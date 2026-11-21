use super::{MediaItem, MediaLibrary};
use crate::place::Place;
use anyhow::{Context, Result};
use serde_json::Value;

pub fn list_libraries(place: &Place, password: &str) -> Result<Vec<MediaLibrary>> {
    let base = base_url(place);
    let user = if place.username.is_empty() {
        "admin"
    } else {
        place.username.as_str()
    };
    // Plaintext p= is accepted by Airsonic / Navidrome for local setups.
    let auth = format!("u={user}&p={password}&v=1.16.1&c=NetworkNeighbourhood&f=json");

    let mut items = Vec::new();

    if let Ok(artists) = get_json(&format!("{base}/rest/getArtists.view?{auth}")) {
        if let Some(index) = artists
            .pointer("/subsonic-response/artists/index")
            .and_then(|v| v.as_array())
        {
            for idx in index {
                if let Some(artists) = idx.get("artist").and_then(|v| v.as_array()) {
                    for a in artists.iter().take(100) {
                        let name = a
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Artist")
                            .to_string();
                        let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        items.push(MediaItem {
                            name,
                            stream_url: format!("{base}/app/#/artist/{id}"),
                            size: Some(0),
                        });
                    }
                }
            }
        }
    }

    if let Ok(albums) = get_json(&format!(
        "{base}/rest/getAlbumList2.view?{auth}&type=newest&size=50"
    )) {
        if let Some(arr) = albums
            .pointer("/subsonic-response/albumList2/album")
            .and_then(|v| v.as_array())
        {
            for al in arr {
                let name = al
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Album")
                    .to_string();
                let id = al.get("id").and_then(|v| v.as_str()).unwrap_or("");
                items.push(MediaItem {
                    name: format!("Album — {name}"),
                    stream_url: format!("{base}/rest/stream.view?{auth}&id={id}"),
                    size: None,
                });
            }
        }
    }

    if items.is_empty() {
        items.push(MediaItem {
            name: "Open in Browser.url".into(),
            stream_url: base.clone(),
            size: Some(64),
        });
    }

    Ok(vec![MediaLibrary {
        name: place.name.clone(),
        items,
    }])
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
    let resp = ureq::get(url).call().context("airsonic http")?;
    Ok(resp.into_json()?)
}
