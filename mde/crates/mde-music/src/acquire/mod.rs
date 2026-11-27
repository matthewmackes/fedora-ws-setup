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

fn safe_component(raw: &str, fallback: &str) -> String {
    let cleaned = sanitize(raw);
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '.') {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// MusicBrainz-style: `{albumartist}/{date} - {album}/{track} {title}`
pub fn mb_relative_path(track: &Track, year: Option<i32>) -> PathBuf {
    let albumartist = safe_component(
        if track.artist.is_empty() {
            "Unknown Artist"
        } else {
            &track.artist
        },
        "Unknown Artist",
    );
    let album = safe_component(
        if track.album.is_empty() {
            "Unknown Album"
        } else {
            &track.album
        },
        "Unknown Album",
    );
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

pub const MB_CONFIDENCE: f64 = 0.75;

/// Attach the real audio extension and refuse to climb out of the inbox.
pub fn inbox_relative(relative: &Path, audio_file: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => out.push(part),
            _ => {}
        }
    }
    if let Some(ext) = audio_file.extension() {
        out.set_extension(ext);
    }
    out
}

fn apply_mb(meta: &mut MbMeta, mb: &MbHit) -> bool {
    if mb.confidence < MB_CONFIDENCE {
        return false;
    }
    if let Some(title) = &mb.title {
        meta.title = title.clone();
    }
    if let Some(artist) = &mb.artist {
        meta.artist = artist.clone();
    }
    if let Some(album) = &mb.album {
        meta.album = album.clone();
    }
    meta.track_num = mb.track_num.or(meta.track_num);
    meta.year = mb.year.or(meta.year);
    meta.cover_url = mb.cover_url.clone().or(meta.cover_url.clone());
    true
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
        apply_mb(&mut meta, &mb);
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
    match cmd.status() {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("ffmpeg is not installed; tags were not written");
        }
        Ok(status) if status.success() && tmp.exists() => {
            std::fs::rename(&tmp, path)?;
            Ok(())
        }
        _ => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("ffmpeg could not write tags");
        }
    }
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
    enum End {
        Done,
        Failed { message: String, path: Option<String> },
    }
    let end = (|| -> End {
        queue.update(&job_id, JobState::Downloading, "Downloading best audio…", None);
        let vid = match ytm::video_id_of(&track) {
            Some(id) => id,
            None => {
                return End::Failed {
                    message: "This track has no YouTube Music id, so it cannot be acquired.".into(),
                    path: None,
                };
            }
        };
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
            if let Err(err) = std::fs::create_dir_all(parent) {
                return End::Failed {
                    message: format!("Could not create {}: {err}", parent.display()),
                    path: None,
                };
            }
            let _ = write_folder_art(parent, meta.cover_url.as_deref());
        }
        let file = match stream::download_best(&vid, &abs_base) {
            Ok(file) => file,
            Err(err) => {
                return End::Failed {
                    message: format!("{err:#}"),
                    path: None,
                };
            }
        };
        let saved = file.display().to_string();
        queue.update(
            &job_id,
            JobState::Tagging,
            "Writing metadata…",
            Some(saved.clone()),
        );
        let tag_note = match apply_ffmpeg_tags(&file, &meta) {
            Ok(()) => None,
            Err(err) => Some(err.to_string()),
        };
        let rel_with_ext = inbox_relative(&rel, &file);
        let has_uploader = uploader.is_some();
        if let Some(up) = uploader {
            queue.update(
                &job_id,
                JobState::Uploading,
                "Uploading to server…",
                Some(saved.clone()),
            );
            if let Err(err) = up.upload_file(&file, &rel_with_ext) {
                let tags = tag_note
                    .as_ref()
                    .map(|note| format!(" Tags: {note}"))
                    .unwrap_or_default();
                return End::Failed {
                    message: format!(
                        "Saved {saved} but the server inbox copy failed: {err:#}.{tags}"
                    ),
                    path: Some(saved),
                };
            }
        }
        let mut message = match &tag_note {
            None => "Acquired".to_string(),
            Some(err) => format!("Acquired without tags: {err}"),
        };
        if !has_uploader {
            message = format!("{message}. No server is connected; playable file is {saved}");
        }
        queue.update(&job_id, JobState::Done, message, Some(saved));
        End::Done
    })();
    if let End::Failed { message, path } = end {
        tracing::warn!(error = %message, "acquire failed");
        queue.update(&job_id, JobState::Failed(message.clone()), message, path);
    }
}

/// Sidebar and player-bar copy. `None` when nothing is in flight and nothing failed.
pub fn badge_text(jobs: &[AcquireJob]) -> Option<String> {
    let active = jobs
        .iter()
        .filter(|job| !matches!(job.state, JobState::Done | JobState::Failed(_)))
        .count();
    let failed = jobs
        .iter()
        .filter(|job| matches!(job.state, JobState::Failed(_)))
        .count();
    match (active, failed) {
        (0, 0) => None,
        (n, 0) => Some(format!("{n} acquiring")),
        (0, n) => Some(format!("{n} acquire failed")),
        (active, failed) => Some(format!("{active} acquiring · {failed} failed")),
    }
}

/// A toast only when a job we were already watching reaches done or failed.
pub fn became_terminal(previous: Option<&JobState>, next: &JobState) -> bool {
    let was_terminal = previous.is_some_and(|state| matches!(state, JobState::Done | JobState::Failed(_)));
    let now_terminal = matches!(next, JobState::Done | JobState::Failed(_));
    previous.is_some() && now_terminal && !was_terminal
}

pub fn terminal_message(job: &AcquireJob) -> String {
    match &job.state {
        JobState::Done => format!("Acquired {}", job.track.title),
        JobState::Failed(err) => format!("Acquire failed for {}: {err}", job.track.title),
        _ => job.message.clone(),
    }
}

pub fn spawn_like_acquire(queue: Arc<AcquireQueue>, track: Track, uploader: Option<Arc<dyn LibraryUploader>>) {
    let id = queue.enqueue(track);
    std::thread::spawn(move || run_job(queue, id, uploader));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(state: JobState) -> AcquireJob {
        AcquireJob {
            id: "1".into(),
            track: Track {
                title: "Song".into(),
                ..Track::default()
            },
            state,
            local_path: None,
            message: String::new(),
        }
    }

    #[test]
    fn badge_counts_active_and_failed() {
        assert!(badge_text(&[]).is_none());
        assert_eq!(
            badge_text(&[job(JobState::Downloading), job(JobState::Done)]).as_deref(),
            Some("1 acquiring")
        );
        assert_eq!(
            badge_text(&[job(JobState::Failed("no".into()))]).as_deref(),
            Some("1 acquire failed")
        );
    }

    #[test]
    fn toast_only_after_a_watched_job_finishes() {
        assert!(!became_terminal(None, &JobState::Done));
        assert!(!became_terminal(Some(&JobState::Queued), &JobState::Downloading));
        assert!(became_terminal(Some(&JobState::Uploading), &JobState::Done));
        assert!(became_terminal(
            Some(&JobState::Tagging),
            &JobState::Failed("ffmpeg".into())
        ));
        assert!(!became_terminal(Some(&JobState::Done), &JobState::Done));
    }

    #[test]
    fn low_confidence_keeps_youtube_music_tags() {
        let mut meta = MbMeta {
            title: "YTM".into(),
            artist: "Channel".into(),
            album: "Singles".into(),
            ..MbMeta::default()
        };
        let weak = MbHit {
            confidence: 0.4,
            title: Some("Other".into()),
            artist: Some("Someone".into()),
            album: Some("LP".into()),
            track_num: Some(4),
            year: Some(1999),
            cover_url: None,
        };
        assert!(!apply_mb(&mut meta, &weak));
        assert_eq!(meta.title, "YTM");
        let strong = MbHit {
            confidence: 0.9,
            ..weak
        };
        assert!(apply_mb(&mut meta, &strong));
        assert_eq!(meta.title, "Other");
        assert_eq!(meta.year, Some(1999));
    }

    struct Inbox {
        root: PathBuf,
    }

    impl LibraryUploader for Inbox {
        fn upload_file(&self, path: &Path, relative: &Path) -> Result<()> {
            let dest = self.root.join(relative);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &dest)?;
            Ok(())
        }
    }

    fn write_wav(path: &Path) {
        let samples: Vec<i16> = (0..4_800)
            .map(|i| ((i as f32 * 0.08).sin() * 8_000.0) as i16)
            .collect();
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&(48_000u32 * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn tagged_acquire_lands_as_a_playable_inbox_file() {
        let dir = std::env::temp_dir().join(format!(
            "nm-inbox-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("song.wav");
        write_wav(&src);
        let meta = MbMeta {
            title: "Inbox Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: Some(1),
            ..MbMeta::default()
        };
        apply_ffmpeg_tags(&src, &meta).expect("ffmpeg tags");
        let track = Track {
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            track_num: Some(1),
            ..Track::default()
        };
        let relative = inbox_relative(&mb_relative_path(&track, None), &src);
        let root = dir.join("inbox");
        Inbox { root: root.clone() }
            .upload_file(&src, &relative)
            .unwrap();
        let landed = root.join(&relative);
        assert_eq!(landed.extension().and_then(|ext| ext.to_str()), Some("wav"));
        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format_tags=title",
                "-of",
                "default=nw=1:nk=1",
                landed.to_str().unwrap(),
            ])
            .output()
            .expect("ffprobe");
        assert!(probe.status.success(), "ffprobe failed");
        let title = String::from_utf8_lossy(&probe.stdout);
        assert!(title.contains("Inbox Song"), "{title}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inbox_path_uses_the_audio_extension_and_stays_inside() {
        let relative = PathBuf::from("Artist").join("0000 - Album").join("01 Title");
        let audio = PathBuf::from("/tmp/01 Title.m4a");
        let inbox = inbox_relative(&relative, &audio);
        assert_eq!(
            inbox,
            PathBuf::from("Artist")
                .join("0000 - Album")
                .join("01 Title.m4a")
        );
        let climbed = mb_relative_path(
            &Track {
                artist: "..".into(),
                album: ".".into(),
                title: "Song".into(),
                ..Track::default()
            },
            None,
        );
        let text = climbed.to_string_lossy().to_string();
        assert!(!text.contains(".."));
        assert!(text.starts_with("Unknown Artist"));
    }
}
