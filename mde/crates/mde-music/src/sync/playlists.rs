//! Bidirectional playlist sync with last-write-wins revisions.

use crate::backend::MediaBackend;
use crate::model::Playlist;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPlaylist {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub track_ids: Vec<String>,
    pub revision: String,
    pub updated_unix: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    playlists: Vec<LocalPlaylist>,
}

fn store_path(server_id: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join(server_id)
        .join("playlists.json")
}

fn load(server_id: &str) -> Store {
    let path = store_path(server_id);
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(server_id: &str, store: &Store) -> Result<()> {
    let path = store_path(server_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn sync_all(server_id: &str, backend: Arc<dyn MediaBackend>) -> Result<Vec<Playlist>> {
    let remote = backend.playlists()?;
    let mut store = load(server_id);

    // Push local-only or newer locals
    for local in store.playlists.clone() {
        let remote_match = remote.iter().find(|p| p.id == local.id);
        match remote_match {
            None => {
                // recreate on server if missing
                if let Ok(created) = backend.create_playlist(&local.name) {
                    let _ = backend.update_playlist(&created.id, &local.name, &local.track_ids);
                }
            }
            Some(r) => {
                let remote_rev = r.revision.clone().unwrap_or_default();
                if local.revision != remote_rev && local.updated_unix >= now() - 86400 * 365 {
                    // last-write-wins: if local revision differs and local is newer stamp, push
                    if local.revision > remote_rev {
                        let _ = backend.update_playlist(&local.id, &local.name, &local.track_ids);
                    }
                }
            }
        }
    }

    // Pull remotes into local
    let mut merged = Vec::new();
    for r in remote {
        let tracks = backend.playlist_tracks(&r.id).unwrap_or_default();
        let track_ids: Vec<_> = tracks.iter().map(|t| t.id.clone()).collect();
        let revision = r.revision.clone().unwrap_or_else(|| r.id.clone());
        if let Some(existing) = store.playlists.iter_mut().find(|p| p.id == r.id) {
            if revision >= existing.revision {
                existing.name = r.name.clone();
                existing.track_ids = track_ids;
                existing.revision = revision.clone();
                existing.updated_unix = now();
            }
        } else {
            store.playlists.push(LocalPlaylist {
                id: r.id.clone(),
                server_id: server_id.into(),
                name: r.name.clone(),
                track_ids,
                revision: revision.clone(),
                updated_unix: now(),
            });
        }
        let mut full = r;
        full.tracks = tracks;
        full.revision = Some(revision);
        merged.push(full);
    }

    save(server_id, &store)?;
    Ok(merged)
}

pub fn upsert_local(
    server_id: &str,
    id: &str,
    name: &str,
    track_ids: Vec<String>,
    backend: Arc<dyn MediaBackend>,
) -> Result<()> {
    let mut store = load(server_id);
    let revision = format!("{}", now());
    if let Some(p) = store.playlists.iter_mut().find(|p| p.id == id) {
        p.name = name.into();
        p.track_ids = track_ids.clone();
        p.revision = revision.clone();
        p.updated_unix = now();
    } else {
        store.playlists.push(LocalPlaylist {
            id: id.into(),
            server_id: server_id.into(),
            name: name.into(),
            track_ids: track_ids.clone(),
            revision: revision.clone(),
            updated_unix: now(),
        });
    }
    save(server_id, &store)?;
    backend.update_playlist(id, name, &track_ids)?;
    Ok(())
}
