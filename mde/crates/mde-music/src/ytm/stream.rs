use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    pub bitrate: Option<u32>,
    pub format: Option<String>,
    pub sample_rate: Option<u32>,
    pub video_url: Option<String>,
}

fn ytdlp_bin() -> PathBuf {
    let local = crate::ytm::repair_dir().join("yt-dlp");
    if local.exists() {
        return local;
    }
    PathBuf::from("yt-dlp")
}

pub fn resolve_via_ytdlp(video_id: &str) -> Result<ResolvedStream> {
    let url = format!("https://music.youtube.com/watch?v={video_id}");
    let bin = ytdlp_bin();
    let out = Command::new(&bin)
        .args([
            "-f",
            "bestaudio/best",
            "-g",
            "--no-playlist",
            &url,
        ])
        .output()
        .with_context(|| format!("spawn {}", bin.display()))?;
    if !out.status.success() {
        anyhow::bail!(
            "yt-dlp failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let stream_url = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if stream_url.is_empty() {
        anyhow::bail!("yt-dlp returned empty URL");
    }
    Ok(ResolvedStream {
        url: stream_url,
        bitrate: None,
        format: Some("ytdlp".into()),
        sample_rate: None,
        video_url: None,
    })
}

/// Download best audio to `dest` (extension chosen by yt-dlp).
pub fn download_best(video_id: &str, dest_template: &Path) -> Result<PathBuf> {
    let url = format!("https://music.youtube.com/watch?v={video_id}");
    let bin = ytdlp_bin();
    let template = format!("{}.%(ext)s", dest_template.display());
    let out = Command::new(&bin)
        .args([
            "-f",
            "bestaudio/best",
            "--no-playlist",
            "-o",
            &template,
            "--write-info-json",
            &url,
        ])
        .output()
        .with_context(|| format!("yt-dlp download {}", bin.display()))?;
    if !out.status.success() {
        anyhow::bail!(
            "yt-dlp download failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // Find produced file next to template stem.
    let parent = dest_template
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = dest_template
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    for entry in std::fs::read_dir(&parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(stem)
            && !name.ends_with(".json")
            && entry.path().is_file()
        {
            return Ok(entry.path());
        }
    }
    anyhow::bail!("download finished but output file not found in {}", parent.display())
}
