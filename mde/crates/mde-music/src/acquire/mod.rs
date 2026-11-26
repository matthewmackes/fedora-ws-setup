//! Like → download HQ → MusicBrainz-prefer tags → MB paths → server API upload.

use crate::model::*;
use crate::ytm::{self, stream};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Downloading,
    Tagging,
    Uploading,
    Done,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquireJob {
    pub id: String,
    pub track: Track,
    pub state: JobState,
    pub local_path: Option<String>,
    pub message: String,
}

#[derive(Default)]
pub struct AcquireQueue {
    jobs: Mutex<Vec<AcquireJob>>,
}

impl AcquireQueue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> Vec<AcquireJob> {
        self.jobs.lock().clone()
    }

    pub fn enqueue(&self, track: Track) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.jobs.lock().push(AcquireJob {
            id: id.clone(),
            track,
            state: JobState::Queued,
            local_path: None,
            message: "Queued".into(),
        });
        id
    }

    fn update(&self, id: &str, state: JobState, message: impl Into<String>, path: Option<String>) {
        let mut jobs = self.jobs.lock();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.state = state;
            job.message = message.into();
            if path.is_some() {
                job.local_path = path;
            }
        }
    }
}

pub fn acquired_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("acquired")
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// MusicBrainz-style: `{albumartist}/{date} - {album}/{track} {title}`
pub fn mb_relative_path(track: &Track, year: Option<i32>) -> PathBuf {
    let albumartist = sanitize(if track.artist.is_empty() {
        "Unknown Artist"
    } else {
        &track.artist
    });
    let album = sanitize(if track.album.is_empty() {
        "Unknown Album"
    } else {
        &track.album
    });
    let date = year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "0000".into());
    let track_num = track.track_num.unwrap_or(0);
    let title = sanitize(&track.title);
    PathBuf::from(albumartist)
        .join(format!("{date} - {album}"))
        .join(format!("{track_num:02} {title}"))
}

#[derive(Debug, Clone, Default)]
pub struct MbMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_num: Option<u32>,
    pub year: Option<i32>,
    pub cover_url: Option<String>,
}

/// Prefer high-confidence MusicBrainz; fall back to YTM fields.
pub fn enrich_metadata(track: &Track) -> MbMeta {
    let mut meta = MbMeta {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        track_num: track.track_num,
        year: None,
        cover_url: track.cover_url.clone(),
    };
    if let Ok(Some(mb)) = musicbrainz_lookup(track) {
        if mb.confidence >= 0.75 {
            meta.title = mb.title.unwrap_or(meta.title);
            meta.artist = mb.artist.unwrap_or(meta.artist);
            meta.album = mb.album.unwrap_or(meta.album);
            meta.track_num = mb.track_num.or(meta.track_num);
            meta.year = mb.year.or(meta.year);
            meta.cover_url = mb.cover_url.or(meta.cover_url);
        }
    }
    meta
}

struct MbHit {
    confidence: f64,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    track_num: Option<u32>,
    year: Option<i32>,
    cover_url: Option<String>,
}

fn musicbrainz_lookup(track: &Track) -> Result<Option<MbHit>> {
    let q = format!(
        "recording:\"{}\" AND artist:\"{}\"",
        track.title.replace('"', ""),
        track.artist.replace('"', "")
    );
    let url = format!(
        "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=1",
        urlencoding::encode(&q)
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "NeighbourhoodMusic/2026 (MDE; musicbrainz)")
        .set("Accept", "application/json")
        .call();
    let Ok(resp) = resp else {
        return Ok(None);
    };
    let v: serde_json::Value = resp.into_json()?;
    let rec = v
        .pointer("/recordings/0")
        .ok_or_else(|| anyhow::anyhow!("no recording"))
        .ok();
    let Some(rec) = rec else {
        return Ok(None);
    };
    let score = rec.get("score").and_then(|s| s.as_u64()).unwrap_or(0) as f64 / 100.0;
    let title = rec
        .get("title")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let artist = rec
        .pointer("/artist-credit/0/name")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let album = rec
        .pointer("/releases/0/title")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let year = rec
        .pointer("/releases/0/date")
        .and_then(|t| t.as_str())
        .and_then(|d| d.get(0..4)?.parse().ok());
    let release_id = rec
        .pointer("/releases/0/id")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let cover_url = release_id
        .map(|id| format!("https://coverartarchive.org/release/{id}/front-500"));
    Ok(Some(MbHit {
        confidence: score,
        title,
        artist,
        album,
        track_num: track.track_num,
        year,
        cover_url,
    }))
}

fn write_folder_art(dir: &Path, cover_url: Option<&str>) -> Result<()> {
    let Some(url) = cover_url.filter(|u| !u.is_empty()) else {
        return Ok(());
    };
    let dest = dir.join("cover.jpg");
    if dest.exists() {
        return Ok(());
    }
    let resp = ureq::get(url)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .call()?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&dest)?;
    std::io::copy(&mut reader, &mut file)?;
    Ok(())
}

/// Tag via ffmpeg metadata when available (keeps deps light).
fn apply_ffmpeg_tags(path: &Path, meta: &MbMeta) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("m4a");
    let tmp = path.with_extension(format!("tagged.{ext}"));
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-codec", "copy"])
        .arg("-metadata")
        .arg(format!("title={}", meta.title))
        .arg("-metadata")
        .arg(format!("artist={}", meta.artist))
        .arg("-metadata")
        .arg(format!("album={}", meta.album))
        .arg("-metadata")
        .arg(format!("track={}", meta.track_num.unwrap_or(0)));
    if let Some(y) = meta.year {
        cmd.arg("-metadata").arg(format!("date={y}"));
    }
    cmd.arg(&tmp);
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if ok && tmp.exists() {
        std::fs::rename(&tmp, path)?;
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(())
}

pub trait LibraryUploader: Send + Sync {
    fn upload_file(&self, path: &Path, relative: &Path) -> Result<()>;
}

/// Run acquire for a liked YTM (or any) track.
pub fn run_job(
    queue: Arc<AcquireQueue>,
    job_id: String,
    uploader: Option<Arc<dyn LibraryUploader>>,
) {
    let track = {
        let jobs = queue.jobs.lock();
        match jobs.iter().find(|j| j.id == job_id) {
            Some(j) => j.track.clone(),
            None => return,
        }
    };
    let result = (|| -> Result<PathBuf> {
        queue.update(&job_id, JobState::Downloading, "Downloading best audio…", None);
        let vid = ytm::video_id_of(&track).context("not a YTM track id")?;
        let meta = enrich_metadata(&track);
        let rel = mb_relative_path(
            &Track {
                title: meta.title.clone(),
                artist: meta.artist.clone(),
                album: meta.album.clone(),
                track_num: meta.track_num,
                ..track.clone()
            },
            meta.year,
        );
        let abs_base = acquired_root().join(&rel);
        if let Some(parent) = abs_base.parent() {
            std::fs::create_dir_all(parent)?;
            let _ = write_folder_art(parent, meta.cover_url.as_deref());
        }
        let file = stream::download_best(&vid, &abs_base)?;
        queue.update(
            &job_id,
            JobState::Tagging,
            "Writing metadata…",
            Some(file.display().to_string()),
        );
        let _ = apply_ffmpeg_tags(&file, &meta);
        if let Some(up) = uploader {
            queue.update(&job_id, JobState::Uploading, "Uploading to server…", None);
            // relative with real extension
            let mut rel_with_ext = rel.clone();
            if let Some(ext) = file.extension() {
                rel_with_ext.set_extension(ext);
            }
            up.upload_file(&file, &rel_with_ext)?;
        }
        Ok(file)
    })();
    match result {
        Ok(path) => queue.update(
            &job_id,
            JobState::Done,
            "Acquired",
            Some(path.display().to_string()),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "acquire failed");
            queue.update(&job_id, JobState::Failed(format!("{e:#}")), format!("{e:#}"), None);
        }
    }
}

pub fn spawn_like_acquire(queue: Arc<AcquireQueue>, track: Track, uploader: Option<Arc<dyn LibraryUploader>>) {
    let id = queue.enqueue(track);
    std::thread::spawn(move || run_job(queue, id, uploader));
}
