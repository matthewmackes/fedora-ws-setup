//! Aggressive LAN library + artwork cache.

use crate::backend::MediaBackend;
use crate::model::{Album, Artist, Playlist, Track};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LibrarySnapshot {
    pub server_id: String,
    pub fetched_unix: u64,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

fn cache_root() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
}

pub fn is_lan_host(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if host == "localhost" || host.ends_with(".local") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
            std::net::IpAddr::V6(v6) => v6.is_loopback() || {
                let o = v6.octets();
                (o[0] & 0xfe) == 0xfc || (o[0] == 0xfe && (o[1] & 0xc0) == 0x80)
            },
        };
    }
    // Heuristic: try UDP connect to discover local iface reachability
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect(format!("{host}:9")).map(|_| true))
        .unwrap_or(false)
}

fn snapshot_path(server_id: &str) -> PathBuf {
    cache_root().join(server_id).join("library.json")
}

pub fn load_snapshot(server_id: &str) -> Option<LibrarySnapshot> {
    let path = snapshot_path(server_id);
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_snapshot(snap: &LibrarySnapshot) -> Result<()> {
    let path = snapshot_path(&snap.server_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string(snap)?)?;
    Ok(())
}

pub fn artwork_path(server_id: &str, key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii() && c.is_alphanumeric() { c } else { '_' })
        .collect();
    cache_root().join(server_id).join("art").join(safe)
}

pub fn cache_artwork(server_id: &str, key: &str, url: &str) -> Result<PathBuf> {
    let path = artwork_path(server_id, key);
    if path.exists() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let resp = ureq::get(url).call()?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)?;
    fs::write(&path, bytes)?;
    Ok(path)
}

/// Full catalog pull when on LAN.
pub fn refresh_full(server_id: &str, base_url: &str, backend: Arc<dyn MediaBackend>) -> Result<LibrarySnapshot> {
    if !is_lan_host(base_url) {
        if let Some(existing) = load_snapshot(server_id) {
            return Ok(existing);
        }
    }
    let artists = backend.artists().unwrap_or_default();
    let playlists = backend.playlists().unwrap_or_default();
    let mut albums = Vec::new();
    let mut tracks = Vec::new();
    // Cap aggressive crawl to keep UI responsive on huge libraries
    for a in artists.iter().take(200) {
        if let Ok(al) = backend.artist_albums(&a.id) {
            for album in al {
                if let Ok(ts) = backend.album_tracks(&album.id) {
                    tracks.extend(ts);
                }
                if let Some(ref url) = album.cover_url {
                    let _ = cache_artwork(server_id, &album.id, url);
                }
                albums.push(album);
            }
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let snap = LibrarySnapshot {
        server_id: server_id.into(),
        fetched_unix: now,
        artists,
        albums,
        tracks,
        playlists,
    };
    save_snapshot(&snap)?;
    Ok(snap)
}

pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(cache_root())?;
    Ok(())
}

pub fn audio_cache_dir(server_id: &str) -> PathBuf {
    cache_root().join(server_id).join("audio")
}

pub fn cache_audio_file(server_id: &str, track_id: &str, url: &str) -> Result<PathBuf> {
    let dir = audio_cache_dir(server_id);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{track_id}.bin"));
    if path.exists() {
        return Ok(path);
    }
    if !is_lan_host(url) && !url.contains("127.0.0.1") {
        // Still allow caching when stream URL host is LAN even if API was remote
    }
    let resp = ureq::get(url).call()?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)?;
    fs::write(&path, bytes)?;
    Ok(path)
}

pub fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
