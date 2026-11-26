//! Read Network Neighbourhood media places + optional local manual servers.

use crate::model::{BackendKind, ServerTarget};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const SYSTEM_PLACES: &str = "/etc/network-neighbourhood/places.json";
const SYSTEM_CREDS: &str = "/etc/network-neighbourhood/creds";

#[derive(Debug, Deserialize)]
struct PlacesFile {
    #[serde(default)]
    places: Vec<RawPlace>,
}

#[derive(Debug, Deserialize)]
struct RawPlace {
    id: String,
    name: String,
    protocol: String,
    host: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: String,
    #[serde(default)]
    credential_file: String,
    #[serde(default)]
    credential_scope: String,
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize, serde::Serialize, Clone)]
struct ManualServers {
    #[serde(default)]
    servers: Vec<ServerTarget>,
}

fn manual_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("servers.json")
}

fn user_creds_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("network-neighbourhood")
        .join("creds")
}

fn read_cred(place: &RawPlace) -> String {
    let path = if place.credential_file.starts_with('/') {
        PathBuf::from(&place.credential_file)
    } else if place.credential_scope == "user" {
        if place.credential_file.is_empty() {
            user_creds_dir().join(format!("{}.cred", place.id))
        } else {
            user_creds_dir().join(&place.credential_file)
        }
    } else if place.credential_file.is_empty() {
        Path::new(SYSTEM_CREDS).join(format!("{}.cred", place.id))
    } else {
        Path::new(SYSTEM_CREDS).join(&place.credential_file)
    };
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn base_url(place: &RawPlace, default_port: u16) -> String {
    if !place.url.is_empty() {
        return crate::urlfix::normalize_http_url(&place.url);
    }
    let port = place.port.unwrap_or(default_port);
    crate::urlfix::normalize_http_url(&format!("http://{}:{port}", place.host))
}

pub fn list_servers() -> Result<Vec<ServerTarget>> {
    let mut out = Vec::new();

    if Path::new(SYSTEM_PLACES).exists() {
        let text = match fs::read_to_string(SYSTEM_PLACES) {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!("skipping {SYSTEM_PLACES}: {err}");
                String::new()
            }
        };
        let file: PlacesFile = if text.is_empty() {
            PlacesFile { places: Vec::new() }
        } else {
            serde_json::from_str(&text).context("parse places.json")?
        };
        for p in file.places {
            let kind = match p.protocol.as_str() {
                "video" | "jellyfin" => BackendKind::Jellyfin,
                "audio" | "airsonic" | "navidrome" => BackendKind::Subsonic,
                _ => continue,
            };
            let default_port = match kind {
                BackendKind::Jellyfin => 8096,
                BackendKind::Subsonic => 4040,
                BackendKind::YoutubeMusic => 443,
            };
            out.push(ServerTarget {
                id: p.id.clone(),
                name: p.name.clone(),
                kind,
                base_url: base_url(&p, default_port),
                username: p.username.clone(),
                password: read_cred(&p),
                from_neighbourhood: true,
            });
        }
    }

    let mp = manual_path();
    if mp.exists() {
        let text = fs::read_to_string(&mp)?;
        let manual: ManualServers = serde_json::from_str(&text).unwrap_or(ManualServers {
            servers: vec![],
        });
        for s in manual.servers {
            if !out.iter().any(|e| e.id == s.id) {
                out.push(s);
            }
        }
    }

    Ok(out)
}

/// Server to open with the window. A remembered id wins. Otherwise the first
/// saved server that has a username and an explicit port.
pub fn startup_server(mut servers: Vec<ServerTarget>, saved_id: Option<&str>) -> Option<ServerTarget> {
    if let Some(id) = saved_id.map(str::trim).filter(|id| !id.is_empty()) {
        if let Some(pos) = servers.iter().position(|server| server.id == id) {
            return Some(servers.swap_remove(pos));
        }
    }
    servers.sort_by_key(startup_rank);
    let first = servers.into_iter().next()?;
    (startup_rank(&first) < 100).then_some(first)
}

fn startup_rank(server: &ServerTarget) -> u8 {
    let url = crate::urlfix::normalize_http_url(&server.base_url);
    let http = url.starts_with("http://") || url.starts_with("https://");
    if !http || server.username.trim().is_empty() {
        return 100;
    }
    if url.matches(':').count() >= 2 {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, url: &str) -> ServerTarget {
        ServerTarget {
            id: id.into(),
            name: url.into(),
            kind: BackendKind::Subsonic,
            base_url: url.into(),
            username: "admin".into(),
            password: String::new(),
            from_neighbourhood: false,
        }
    }

    #[test]
    fn startup_prefers_remembered_id_then_ported_url() {
        let bare = server("bare", "http://172.20.0.2");
        let ported = server("ported", "http://172.20.0.2:4040");
        let chosen = startup_server(vec![bare.clone(), ported.clone()], None).unwrap();
        assert_eq!(chosen.id, "ported");
        let remembered = startup_server(vec![ported, bare.clone()], Some("bare")).unwrap();
        assert_eq!(remembered.id, "bare");
        assert!(startup_server(
            vec![ServerTarget {
                username: String::new(),
                ..bare
            }],
            None
        )
        .is_none());
    }
}

pub fn save_manual_server(server: ServerTarget) -> Result<()> {
    let path = manual_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut manual = if path.exists() {
        serde_json::from_str(&fs::read_to_string(&path)?)?
    } else {
        ManualServers { servers: vec![] }
    };
    if let Some(existing) = manual.servers.iter_mut().find(|s| s.id == server.id) {
        *existing = server;
    } else {
        manual.servers.push(server);
    }
    fs::write(path, serde_json::to_string_pretty(&manual)?)?;
    Ok(())
}
