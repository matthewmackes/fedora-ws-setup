use super::{MediaItem, MediaLibrary};
use crate::place::Place;
use anyhow::{Context, Result};
use serde_json::Value;

pub fn list_libraries(place: &Place, password: &str) -> Result<Vec<MediaLibrary>> {
    let base = base_url(place);
    let token = if password.is_empty() {
        String::new()
    } else {
        password.to_string()
    };

    // password field stores API key / access token for Jellyfin
    let views = get_json(
        &format!("{base}/Library/MediaFolders"),
        &token,
        &place.username,
    )
    .or_else(|_| get_json(&format!("{base}/Users/Public"), &token, &place.username))?;

    let mut libs = Vec::new();

    // Prefer Items endpoint with user's views
    if let Ok(items) = get_json(
        &format!("{base}/Items?Recursive=false&IncludeItemTypes=Movie,Series,Folder"),
        &token,
        &place.username,
    ) {
        if let Some(arr) = items.get("Items").and_then(|v| v.as_array()) {
            let mut lib_items = Vec::new();
            for it in arr.iter().take(200) {
                let name = it
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Item")
                    .to_string();
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("");
                let stream = format!("{base}/Items/{id}/Download");
                lib_items.push(MediaItem {
                    name: format!("{name}"),
                    stream_url: stream,
                    size: it.get("Size").and_then(|v| v.as_u64()),
                });
            }
            libs.push(MediaLibrary {
                name: "Library".into(),
                items: lib_items,
            });
        }
    }

    if libs.is_empty() {
        // Fallback: expose base as single placeholder library
        let _ = views;
        libs.push(MediaLibrary {
            name: place.name.clone(),
            items: vec![MediaItem {
                name: "Open in Browser.url".into(),
                stream_url: base,
                size: Some(64),
            }],
        });
    }
    Ok(libs)
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
    let mut req = ureq::get(url).set(
        "X-Emby-Authorization",
        &format!(
            "MediaBrowser Client=\"NetworkNeighbourhood\", Device=\"MDE\", DeviceId=\"mde-nethood\", Version=\"2026.0\", UserId=\"{user}\""
        ),
    );
    if !token.is_empty() {
        req = req.set("X-Emby-Token", token).set("Authorization", &format!("MediaBrowser Token=\"{token}\""));
    }
    let resp = req.call().context("jellyfin http")?;
    Ok(resp.into_json()?)
}
