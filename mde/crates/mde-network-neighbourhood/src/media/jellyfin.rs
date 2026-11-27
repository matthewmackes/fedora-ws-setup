//! Jellyfin: list movies and episodes with genre and date-added.

use super::views::Entry;
use crate::place::Place;
use anyhow::{bail, Context, Result};
use serde_json::Value;

pub fn list_entries(place: &Place, password: &str) -> Result<Vec<Entry>> {
    let base = base_url(place);
    // The password field stores the Jellyfin API key / access token.
    let v = get_json(
        &format!(
            "{base}/Items?Recursive=true&IncludeItemTypes=Movie,Episode&Limit=3000\
             &Fields=Genres,DateCreated,MediaSources&SortBy=DateCreated&SortOrder=Descending"
        ),
        password,
        &place.username,
    )?;
    let Some(items) = v.get("Items").and_then(|i| i.as_array()) else {
        bail!("Jellyfin returned no item list — check the API key");
    };
    Ok(items.iter().map(|it| item_entry(it, &base)).collect())
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
    let mut req = ureq::get(url).timeout(std::time::Duration::from_secs(20)).set(
        "X-Emby-Authorization",
        &format!(
            "MediaBrowser Client=\"NetworkNeighbourhood\", Device=\"MDE\", DeviceId=\"mde-nethood\", Version=\"2026.0\", UserId=\"{user}\""
        ),
    );
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
