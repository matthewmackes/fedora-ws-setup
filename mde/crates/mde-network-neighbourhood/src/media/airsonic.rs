//! Subsonic-API servers (Airsonic, Navidrome, ...): fetch every song once.

use super::views::Entry;
use crate::place::Place;
use anyhow::{bail, Context, Result};
use serde_json::Value;

const PAGE: usize = 500;
const MAX_SONGS: usize = 10_000;

/// Quick credential check (one `ping` call), for the pre-save test.
pub fn check_login(place: &Place, password: &str) -> Result<String> {
    let base = base_url(place);
    let auth = auth_query(place, password);
    check_status(&get_json(&format!("{base}/rest/ping.view?{auth}"))?)?;
    Ok(format!("signed in as {}", if place.username.is_empty() { "admin" } else { place.username.as_str() }))
}

pub fn list_entries(place: &Place, password: &str) -> Result<Vec<Entry>> {
    let base = base_url(place);
    let auth = auth_query(place, password);

    // Subsonic's own error (wrong password etc.) arrives inside a 200 response.
    let ping = get_json(&format!("{base}/rest/ping.view?{auth}"))?;
    check_status(&ping)?;

    let mut out = Vec::new();
    let mut offset = 0;
    while out.len() < MAX_SONGS {
        let v = get_json(&format!(
            "{base}/rest/search3.view?{auth}&query=&artistCount=0&albumCount=0&songCount={PAGE}&songOffset={offset}"
        ))?;
        let songs = v
            .pointer("/subsonic-response/searchResult3/song")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();
        if songs.is_empty() {
            break;
        }
        let n = songs.len();
        out.extend(songs.iter().map(|s| song_entry(s, &base, &auth)));
        if n < PAGE {
            break;
        }
        offset += PAGE;
    }

    if out.is_empty() {
        out = via_albums(&base, &auth)?;
    }
    if out.is_empty() {
        bail!("the music server returned no songs");
    }
    Ok(out)
}

/// Fallback for servers whose search3 rejects an empty query.
fn via_albums(base: &str, auth: &str) -> Result<Vec<Entry>> {
    let list = get_json(&format!(
        "{base}/rest/getAlbumList2.view?{auth}&type=newest&size=200"
    ))?;
    let albums = list
        .pointer("/subsonic-response/albumList2/album")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for al in albums {
        let Some(id) = al.get("id").and_then(|v| v.as_str()) else { continue };
        let Ok(v) = get_json(&format!("{base}/rest/getAlbum.view?{auth}&id={id}")) else {
            continue;
        };
        if let Some(songs) = v.pointer("/subsonic-response/album/song").and_then(|s| s.as_array()) {
            out.extend(songs.iter().map(|s| song_entry(s, base, auth)));
        }
    }
    Ok(out)
}

fn song_entry(s: &Value, base: &str, auth: &str) -> Entry {
    let text = |k: &str| s.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let id = text("id");
    Entry {
        title: text("title"),
        artist: text("artist"),
        album: text("album"),
        genre: text("genre"),
        track: s.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        added: chrono::DateTime::parse_from_rfc3339(&text("created"))
            .map(|d| d.timestamp().max(0) as u64)
            .unwrap_or(0),
        size: s.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
        url: format!("{base}/rest/stream.view?{auth}&id={id}"),
        ext: text("suffix"),
        meta: std::sync::Arc::new(super::meta::audio_meta(s)),
    }
}

fn check_status(v: &Value) -> Result<()> {
    if v.pointer("/subsonic-response/status").and_then(|s| s.as_str()) == Some("ok") {
        return Ok(());
    }
    let msg = v
        .pointer("/subsonic-response/error/message")
        .and_then(|s| s.as_str())
        .unwrap_or("server did not answer like a Subsonic API");
    bail!("music server error: {msg}")
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn auth_query(place: &Place, password: &str) -> String {
    let user = if place.username.is_empty() { "admin" } else { place.username.as_str() };
    // `enc:` hex keeps special characters in the password from breaking the URL.
    format!(
        "u={}&p=enc:{}&v=1.15.0&c=NetworkNeighbourhood&f=json",
        url::form_urlencoded::byte_serialize(user.as_bytes()).collect::<String>(),
        hex(password)
    )
}

fn base_url(place: &Place) -> String {
    if !place.url.is_empty() {
        place.url.trim_end_matches('/').to_string()
    } else {
        let port = place.port.unwrap_or(4040);
        format!("http://{}:{port}", place.host)
    }
}

fn get_json(url: &str) -> Result<Value> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .context("music server request failed")?;
    Ok(resp.into_json()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::Protocol;

    #[test]
    fn uses_protocol_version_airsonic_accepts() {
        // Airsonic 10.x answers "Incompatible ... Server must upgrade" to anything newer.
        let p = Place::new("m", Protocol::Audio, "h");
        assert!(auth_query(&p, "x").contains("v=1.15.0"));
    }

    #[test]
    fn password_is_hex_encoded() {
        let mut p = Place::new("m", Protocol::Audio, "h");
        p.username = "me@x".into();
        let q = auth_query(&p, "a&b");
        assert!(q.contains("p=enc:612662"));
        assert!(q.contains("u=me%40x"));
    }

    #[test]
    fn parses_song() {
        let s = serde_json::json!({
            "id":"7","title":"T","artist":"A","album":"B","genre":"G","track":3,
            "suffix":"flac","size":99,"created":"2024-01-02T03:04:05.000Z"
        });
        let e = song_entry(&s, "http://h", "u=x");
        assert_eq!((e.track, e.ext.as_str(), e.size), (3, "flac", 99));
        assert!(e.added > 0);
        assert!(e.url.ends_with("&id=7"));
    }

    #[test]
    fn subsonic_error_is_reported() {
        let v = serde_json::json!({"subsonic-response":{"status":"failed","error":{"message":"Wrong username or password"}}});
        assert!(check_status(&v).unwrap_err().to_string().contains("Wrong username"));
    }

    /// Serve canned Subsonic JSON on a local port and run the real client against it.
    fn mock_server(password_ok: bool) -> u16 {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut st) = stream else { break };
                let mut buf = [0u8; 4096];
                let n = st.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /rest/ping.view") {
                    if password_ok {
                        r#"{"subsonic-response":{"status":"ok"}}"#.to_string()
                    } else {
                        r#"{"subsonic-response":{"status":"failed","error":{"code":40,"message":"Wrong username or password"}}}"#.to_string()
                    }
                } else if req.starts_with("GET /rest/search3.view") {
                    r#"{"subsonic-response":{"status":"ok","searchResult3":{"song":[
                        {"id":"1","title":"Smooth Criminal","artist":"Alien Ant Farm","album":"ANThology","genre":"Rock","track":1,"suffix":"mp3","size":100,"created":"2024-01-02T03:04:05.000Z"},
                        {"id":"2","title":"Movies","artist":"Alien Ant Farm","album":"ANThology","genre":"Rock","track":2,"suffix":"mp3","size":200,"created":"2024-01-02T03:04:05.000Z"}]}}}"#.to_string()
                } else {
                    "{}".to_string()
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = st.write_all(resp.as_bytes());
            }
        });
        port
    }

    #[test]
    fn lists_songs_over_http_and_builds_views() {
        let port = mock_server(true);
        let mut p = Place::new("m", Protocol::Audio, "127.0.0.1");
        p.port = Some(port);
        p.username = "mm".into();
        let entries = list_entries(&p, "pw").expect("list");
        assert_eq!(entries.len(), 2);
        assert!(entries[0].url.contains("stream.view"));
        let views = crate::media::views::audio_views(&entries);
        let artist = &views[0].dirs[0];
        assert_eq!(artist.name, "Alien Ant Farm");
        assert_eq!(artist.dirs[0].files[0].name, "01 - Smooth Criminal.mp3");
    }

    #[test]
    fn wrong_password_surfaces_server_message() {
        let port = mock_server(false);
        let mut p = Place::new("m", Protocol::Audio, "127.0.0.1");
        p.port = Some(port);
        let err = list_entries(&p, "bad").unwrap_err().to_string();
        assert!(err.contains("Wrong username or password"), "{err}");
        assert_eq!(crate::diagnose::classify(&err).kind, crate::diagnose::ErrorKind::Auth);
    }
}
