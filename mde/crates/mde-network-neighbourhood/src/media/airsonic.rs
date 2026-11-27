//! Subsonic-API servers (Airsonic, Navidrome, ...): fetch every song once.

use super::views::Entry;
use crate::place::Place;
use anyhow::{bail, Context, Result};
use serde_json::Value;

const PAGE: usize = 500;
const MAX_SONGS: usize = 10_000;

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
        "u={}&p=enc:{}&v=1.16.1&c=NetworkNeighbourhood&f=json",
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
}
