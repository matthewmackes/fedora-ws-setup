//! Jellyfin: list movies and episodes with genre and date-added.

use super::views::Entry;
use crate::place::Place;
use anyhow::{bail, Context, Result};
use serde_json::Value;

pub fn list_entries(place: &Place, password: &str) -> Result<Vec<Entry>> {
    let base = base_url(place);
    // Sign in with username + password when both are set; the password field may
    // instead hold an API key / access token, so fall back to using it as one.
    let mut login_err = None;
    let (token, scope) = if !place.username.is_empty() && !password.is_empty() {
        match login(&base, &place.username, password) {
            Ok((token, user_id)) => (token, format!("/Users/{user_id}/Items")),
            Err(e) => {
                login_err = Some(e);
                (password.to_string(), "/Items".to_string())
            }
        }
    } else {
        (password.to_string(), "/Items".to_string())
    };

    let query = "Recursive=true&IncludeItemTypes=Movie,Episode&Limit=3000\
                 &Fields=Genres,DateCreated,MediaSources&SortBy=DateCreated&SortOrder=Descending";
    let v = match get_json(&format!("{base}{scope}?{query}"), &token, &place.username) {
        Ok(v) => v,
        // Report the sign-in failure, which is the more useful message.
        Err(e) => return Err(login_err.unwrap_or(e)),
    };
    let Some(items) = v.get("Items").and_then(|i| i.as_array()) else {
        bail!("Jellyfin returned no item list — check the username and password");
    };
    Ok(items.iter().map(|it| item_entry(it, &base)).collect())
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

fn item_entry(it: &Value, base: &str) -> Entry {
    let text = |k: &str| it.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let id = text("Id");
    let src = it.pointer("/MediaSources/0");
    Entry {
        title: text("Name"),
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
        url: format!("{base}/Items/{id}/Download"),
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
        let e = item_entry(&it, "http://h:8096");
        assert_eq!((e.title.as_str(), e.genre.as_str(), e.ext.as_str(), e.size), ("Alien", "Horror", "mkv", 1234));
        assert_eq!(e.url, "http://h:8096/Items/abc/Download");
        assert!(e.added > 0);
    }
}
