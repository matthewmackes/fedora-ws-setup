//! Jellyfin: list movies and episodes with genre and date-added.

use super::views::Entry;
use crate::place::Place;
use anyhow::{bail, Context, Result};
use serde_json::Value;

/// Saved Jellyfin session: the access token from the first successful login.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: String,
}

fn session_path_in(dir: &std::path::Path, place_id: &str) -> std::path::PathBuf {
    dir.join(format!("{place_id}.jftoken"))
}

/// Where the daemon keeps a place's Jellyfin token (root-only, next to its credentials).
pub fn session_path(place: &Place) -> std::path::PathBuf {
    session_path_in(std::path::Path::new(crate::config::CREDS_DIR), &place.id)
}

fn load_session_from(path: &std::path::Path) -> Option<Session> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn save_session_to(path: &std::path::Path, s: &Session) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(serde_json::to_string(s)?.as_bytes())?;
    Ok(())
}

/// Quick credential check (one sign-in call), for the pre-save test.
pub fn check_login(place: &Place, password: &str) -> Result<String> {
    let base = base_url(place);
    if place.username.is_empty() {
        bail!("enter the Jellyfin username");
    }
    login(&base, &place.username, password)?;
    Ok(format!("signed in as {}", place.username))
}

/// An authenticated connection to a Jellyfin server. The mount keeps it so file
/// browsers can ask for expensive details (cast and crew) on demand.
#[derive(Debug, Clone)]
pub struct Conn {
    pub base: String,
    pub token: String,
    /// Empty when the password field was an API key rather than a login.
    pub user_id: String,
    pub user: String,
}

pub fn list_entries(place: &Place, password: &str) -> Result<Vec<Entry>> {
    let conn = connect(place, password, &session_path(place))?;
    list_with(&conn)
}

/// Connection order: (1) the saved access token, (2) sign in with the password and
/// save the new token for next time, (3) treat the password as an API key.
pub fn connect(place: &Place, password: &str, session_file: &std::path::Path) -> Result<Conn> {
    let base = base_url(place);
    let user = place.username.clone();

    // 1. Saved token from an earlier login: no password sent.
    if let Some(sess) = load_session_from(session_file) {
        let conn = Conn { base: base.clone(), token: sess.token, user_id: sess.user_id, user: user.clone() };
        match get_json(&format!("{base}/Users/{}", conn.user_id), &conn.token, &user) {
            Ok(_) => {
                tracing::info!(place = %place.id, "jellyfin: connected with saved access token");
                return Ok(conn);
            }
            Err(e) => {
                tracing::warn!(place = %place.id, "jellyfin: saved token rejected ({e}); signing in again");
                let _ = std::fs::remove_file(session_file);
            }
        }
    }

    // 2. First login (or token revoked): sign in, then keep the token.
    let mut login_err = None;
    if !user.is_empty() && !password.is_empty() {
        match login(&base, &user, password) {
            Ok((token, user_id)) => {
                let sess = Session { token: token.clone(), user_id: user_id.clone() };
                match save_session_to(session_file, &sess) {
                    Ok(()) => tracing::info!(place = %place.id, "jellyfin: signed in; access token saved for future connections"),
                    Err(e) => tracing::warn!(place = %place.id, "jellyfin: could not save access token: {e}"),
                }
                return Ok(Conn { base, token, user_id, user });
            }
            Err(e) => login_err = Some(e),
        }
    }

    // 3. The password field may itself be an API key.
    match get_json(&format!("{base}/System/Info"), password, &user) {
        Ok(_) => Ok(Conn { base, token: password.to_string(), user_id: String::new(), user }),
        // Report the sign-in failure, which is the more useful message.
        Err(e) => Err(login_err.unwrap_or(e)),
    }
}

/// Everything a file browser can show, except cast and crew (see `person_meta`).
const FIELDS: &str = "Overview,Genres,DateCreated,MediaSources,Studios,ProviderIds,Taglines,OriginalTitle,PremiereDate,Path";

pub fn list_with(c: &Conn) -> Result<Vec<Entry>> {
    let scope = if c.user_id.is_empty() {
        "/Items".to_string()
    } else {
        format!("/Users/{}/Items", c.user_id)
    };
    let query = format!(
        "Recursive=true&IncludeItemTypes=Movie,Episode&Limit=3000&Fields={FIELDS}&SortBy=DateCreated&SortOrder=Descending"
    );
    let v = get_json(&format!("{}{scope}?{query}", c.base), &c.token, &c.user)?;
    let Some(items) = v.get("Items").and_then(|i| i.as_array()) else {
        bail!("Jellyfin returned no item list — check the username and password");
    };
    Ok(items.iter().map(|it| item_entry(it, &c.base, &c.token)).collect())
}

/// Cast, director and writers for one item. Fetched only when a file browser asks,
/// because including them in the main listing makes it about four times slower.
pub fn person_meta(c: &Conn, item_id: &str) -> Result<super::meta::Meta> {
    let url = if c.user_id.is_empty() {
        format!("{}/Items?Ids={item_id}&Fields=People", c.base)
    } else {
        format!("{}/Users/{}/Items/{item_id}?Fields=People", c.base, c.user_id)
    };
    let v = get_json(&url, &c.token, &c.user)?;
    let item = v.pointer("/Items/0").unwrap_or(&v);
    Ok(super::meta::people_meta(item))
}

/// POST /Users/AuthenticateByName → (access token, user id).
fn login(base: &str, user: &str, password: &str) -> Result<(String, String)> {
    let resp = ureq::post(&format!("{base}/Users/AuthenticateByName"))
        .timeout(std::time::Duration::from_secs(20))
        .set("X-Emby-Authorization", &client_header(""))
        .send_json(serde_json::json!({ "Username": user, "Pw": password }))
        .context("Jellyfin login failed")?;
    let v: Value = resp.into_json()?;
    let token = v.get("AccessToken").and_then(|t| t.as_str());
    let uid = v.pointer("/User/Id").and_then(|t| t.as_str());
    match (token, uid) {
        (Some(t), Some(u)) => Ok((t.to_string(), u.to_string())),
        _ => bail!("Jellyfin login failed: unexpected reply"),
    }
}

fn client_header(user: &str) -> String {
    format!(
        "MediaBrowser Client=\"NetworkNeighbourhood\", Device=\"MDE\", DeviceId=\"mde-nethood\", Version=\"2026.0\", UserId=\"{user}\""
    )
}

/// `token` is the login access token (or API key); it rides in the URL as `api_key`
/// because the mount's read path cannot know it otherwise.
fn item_entry(it: &Value, base: &str, token: &str) -> Entry {
    let text = |k: &str| it.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let id = text("Id");
    let src = it.pointer("/MediaSources/0");
    // Episode names alone collide ("Pilot", "Finale"); prefix series and number.
    let title = match (text("SeriesName").as_str(), text("Type").as_str()) {
        (series, "Episode") if !series.is_empty() => {
            let num = |k: &str| it.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
            format!("{series} - S{:02}E{:02} - {}", num("ParentIndexNumber"), num("IndexNumber"), text("Name"))
        }
        _ => text("Name"),
    };
    Entry {
        title,
        genre: it
            .pointer("/Genres/0")
            .and_then(|g| g.as_str())
            .unwrap_or("")
            .to_string(),
        added: chrono::DateTime::parse_from_rfc3339(&text("DateCreated"))
            .map(|d| d.timestamp().max(0) as u64)
            .unwrap_or(0),
        size: src.and_then(|s| s.get("Size")).and_then(|v| v.as_u64()).unwrap_or(0),
        ext: src
            .and_then(|s| s.get("Container"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        url: if token.is_empty() {
            format!("{base}/Items/{id}/Download")
        } else {
            format!("{base}/Items/{id}/Download?api_key={token}")
        },
        meta: std::sync::Arc::new(super::meta::video_meta(it, base)),
        ..Default::default()
    }
}

fn base_url(place: &Place) -> String {
    if !place.url.is_empty() {
        place.url.trim_end_matches('/').to_string()
    } else {
        let port = place.port.unwrap_or(8096);
        format!("http://{}:{port}", place.host)
    }
}

fn get_json(url: &str, token: &str, user: &str) -> Result<Value> {
    let mut req = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .set("X-Emby-Authorization", &client_header(user));
    if !token.is_empty() {
        req = req
            .set("X-Emby-Token", token)
            .set("Authorization", &format!("MediaBrowser Token=\"{token}\""));
    }
    let resp = req.call().context("Jellyfin request failed")?;
    Ok(resp.into_json()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_item() {
        let it = serde_json::json!({
            "Id":"abc","Name":"Alien","Genres":["Horror"],"DateCreated":"2024-05-06T07:08:09.0000000Z",
            "MediaSources":[{"Size":1234,"Container":"mkv"}]
        });
        let e = item_entry(&it, "http://h:8096", "tok");
        assert_eq!((e.title.as_str(), e.genre.as_str(), e.ext.as_str(), e.size), ("Alien", "Horror", "mkv", 1234));
        assert_eq!(e.url, "http://h:8096/Items/abc/Download?api_key=tok");
        assert!(e.added > 0);
    }

    #[test]
    fn session_roundtrips_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let path = session_path_in(d.path(), "abc");
        assert!(load_session_from(&path).is_none());
        let s = Session { token: "t".into(), user_id: "u".into() };
        save_session_to(&path, &s).unwrap();
        assert_eq!(load_session_from(&path), Some(s));
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
