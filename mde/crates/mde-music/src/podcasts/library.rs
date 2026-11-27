//! Listening state kept on this machine: played flags, resume history for
//! "Continue listening", offline downloads, and the newest-episode cache.

use super::{config_dir, http, progress_for, stable_id, write_json, Episode, Show};
use crate::model::Track;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_MAX: usize = 40;
/// Newest-episode lookups are reused for this long before the feed is read again.
pub const LATEST_TTL_SECS: i64 = 3600;

static DOWNLOADING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub track: Track,
    pub show: Show,
    pub updated: i64,
}

#[derive(Serialize, Deserialize, Default)]
struct LibraryFile {
    #[serde(default)]
    played: BTreeSet<String>,
    #[serde(default)]
    history: Vec<HistoryEntry>,
}

#[derive(Serialize, Deserialize, Default)]
struct LatestFile {
    #[serde(default)]
    shows: BTreeMap<String, LatestEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LatestEntry {
    pub fetched: i64,
    pub show: Show,
    pub episode: Option<Episode>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn library_path() -> PathBuf {
    config_dir().join("podcast-library.json")
}

fn latest_path() -> PathBuf {
    config_dir().join("podcast-latest.json")
}

fn load() -> LibraryFile {
    fs::read_to_string(library_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(file: &LibraryFile) {
    let _ = write_json(&library_path(), file);
}

pub fn is_played(track_id: &str) -> bool {
    load().played.contains(track_id)
}

pub fn played_set() -> BTreeSet<String> {
    load().played
}

pub fn set_played(track_id: &str, played: bool) {
    let mut file = load();
    if played {
        file.played.insert(track_id.to_string());
    } else {
        file.played.remove(track_id);
    }
    save(&file);
}

/// Record that an episode was started, so it can come back under Continue listening.
pub fn remember(track: &Track, show: &Show) {
    let mut file = load();
    file.history.retain(|e| e.track.id != track.id);
    file.history.insert(
        0,
        HistoryEntry {
            track: track.clone(),
            show: show.clone(),
            updated: now(),
        },
    );
    file.history.truncate(HISTORY_MAX);
    save(&file);
}

/// Episodes that were started but not finished, most recent first.
pub fn continue_listening() -> Vec<(HistoryEntry, u64)> {
    let file = load();
    file.history
        .into_iter()
        .filter(|e| !file.played.contains(&e.track.id))
        .filter_map(|e| {
            let pos = progress_for(&e.track.id)?;
            if pos < 5 {
                return None;
            }
            if let Some(dur) = e.track.duration_secs {
                if pos + 30 >= dur {
                    return None;
                }
            }
            Some((e, pos))
        })
        .collect()
}

pub fn latest_cached(show_id: &str) -> Option<LatestEntry> {
    let text = fs::read_to_string(latest_path()).ok()?;
    let file: LatestFile = serde_json::from_str(&text).ok()?;
    file.shows.get(show_id).cloned()
}

pub fn store_latest(show: &Show, episode: Option<Episode>) {
    let mut file: LatestFile = fs::read_to_string(latest_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    file.shows.insert(
        show.id.clone(),
        LatestEntry {
            fetched: now(),
            show: show.clone(),
            episode,
        },
    );
    let _ = write_json(&latest_path(), &file);
}

pub fn is_fresh(entry: &LatestEntry) -> bool {
    now() - entry.fetched < LATEST_TTL_SECS
}

// ── offline downloads ───────────────────────────────────────────────────────

fn download_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("podcast-downloads")
}

fn extension_of(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    for ext in ["m4a", "aac", "ogg", "opus", "wav", "flac"] {
        if path.ends_with(&format!(".{ext}")) {
            return ext;
        }
    }
    "mp3"
}

fn download_target(episode: &Episode) -> PathBuf {
    download_dir().join(format!(
        "{}.{}",
        stable_id(&episode.id),
        extension_of(&episode.stream_url)
    ))
}

pub fn downloaded_path(episode: &Episode) -> Option<PathBuf> {
    let path = download_target(episode);
    path.exists().then_some(path)
}

pub fn is_downloading(episode: &Episode) -> bool {
    DOWNLOADING.lock().contains(&episode.id)
}

pub fn remove_download(episode: &Episode) {
    let _ = fs::remove_file(download_target(episode));
}

/// Download an episode on a background thread; progress is "exists / is_downloading".
pub fn start_download(episode: &Episode) {
    if downloaded_path(episode).is_some() || !DOWNLOADING.lock().insert(episode.id.clone()) {
        return;
    }
    let episode = episode.clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let target = download_target(&episode);
            fs::create_dir_all(download_dir())?;
            let part = target.with_extension("part");
            let resp = http()
                .get(&episode.stream_url)
                .timeout(std::time::Duration::from_secs(3600))
                .call()?;
            let mut out = fs::File::create(&part)?;
            std::io::copy(&mut resp.into_reader().take(2_000_000_000), &mut out)?;
            fs::rename(&part, &target)?;
            Ok(())
        })();
        if let Err(err) = result {
            tracing::warn!(episode = %episode.title, "podcast download failed: {err:#}");
        }
        DOWNLOADING.lock().remove(&episode.id);
    });
}

/// `file://` URI for a track whose episode is on disk, else its stream URL.
pub fn playable_url(episode: &Episode) -> String {
    match downloaded_path(episode) {
        Some(path) => format!("file://{}", path.display()),
        None => episode.stream_url.clone(),
    }
}
