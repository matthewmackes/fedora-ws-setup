use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// User-Agent that matches yt-dlp's `android` player client streams.
pub const ANDROID_YT_UA: &str =
    "com.google.android.youtube/19.29.37 (Linux; U; Android 14) gzip";

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    pub bitrate: Option<u32>,
    pub format: Option<String>,
    pub sample_rate: Option<u32>,
    pub video_url: Option<String>,
    /// When true, GStreamer must use [`ANDROID_YT_UA`] (googlevideo CDN).
    pub needs_android_ua: bool,
}

fn ytdlp_bin() -> PathBuf {
    let local = crate::ytm::repair_dir().join("yt-dlp");
    if local.exists() {
        return local;
    }
    PathBuf::from("yt-dlp")
}

/// yt-dlp ≥2025 needs a JS runtime to solve YouTube `n` challenges.
fn js_runtime_args() -> &'static [String] {
    static ARGS: OnceLock<Vec<String>> = OnceLock::new();
    ARGS.get_or_init(|| {
        let mut out = Vec::new();
        if let Some((kind, path)) = find_js_runtime() {
            tracing::info!(%kind, path = %path.display(), "yt-dlp JS runtime");
            out.push("--js-runtimes".into());
            out.push(format!("{kind}:{}", path.display()));
        } else {
            tracing::warn!("no deno/node JS runtime found for yt-dlp; YTM fallback may fail");
        }
        out
    })
}

fn find_js_runtime() -> Option<(&'static str, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let named: &[(&str, &[&str])] = &[
        (
            "deno",
            &["deno", "/usr/bin/deno", "/usr/local/bin/deno"],
        ),
        (
            "node",
            &[
                "node",
                "nodejs",
                "/usr/bin/node",
                "/usr/bin/nodejs",
                "/usr/local/bin/node",
            ],
        ),
    ];
    for &(kind, candidates) in named {
        for c in candidates {
            let p = PathBuf::from(c);
            if p.is_absolute() {
                if p.is_file() {
                    return Some((kind, p));
                }
            } else if let Ok(resolved) = which_bin(c) {
                return Some((kind, resolved));
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(home.join(".local/share/cursor-agent/versions")) {
        let mut versions: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        versions.sort_by_key(|e| e.file_name());
        for entry in versions.into_iter().rev() {
            let node = entry.path().join("node");
            if node.is_file() {
                return Some(("node", node));
            }
        }
    }
    None
}

fn which_bin(name: &str) -> Result<PathBuf> {
    let out = Command::new("sh")
        .args(["-c", &format!("command -v {name}")])
        .output()
        .context("command -v")?;
    if !out.status.success() {
        anyhow::bail!("not found");
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        anyhow::bail!("empty");
    }
    Ok(PathBuf::from(path))
}

fn ytdlp_base_cmd() -> Command {
    let mut cmd = Command::new(ytdlp_bin());
    for arg in js_runtime_args() {
        cmd.arg(arg);
    }
    // Pull the EJS challenge solver so signature/`n` challenges succeed.
    cmd.args(["--remote-components", "ejs:github"]);
    cmd
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("ytm")
}

fn cached_file_for(video_id: &str) -> Option<PathBuf> {
    let dir = cache_dir();
    let rd = std::fs::read_dir(&dir).ok()?;
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(video_id)
            && !name.ends_with(".json")
            && !name.ends_with(".part")
            && entry.path().is_file()
        {
            if entry.metadata().ok()?.len() > 1024 {
                return Some(entry.path());
            }
        }
    }
    None
}

fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy();
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// A native URL is usable only after a short range request succeeds.
/// Empty URLs and probes that fail (including HTTP 403) must fall back.
pub fn native_probe_accepted(url: &str, probe_ok: bool) -> bool {
    probe_ok && !url.is_empty()
}

pub(crate) fn url_is_playable(url: &str, android_ua: bool) -> bool {
    let mut req = ureq::get(url).set("Range", "bytes=0-2047");
    if android_ua {
        req = req.set("User-Agent", ANDROID_YT_UA);
    }
    match req.call() {
        Ok(resp) => {
            let status = resp.status();
            (200..300).contains(&status)
        }
        Err(_) => false,
    }
}

/// Resolve a URL GStreamer can actually play (android client + UA), else cache to disk.
pub fn resolve_via_ytdlp(video_id: &str) -> Result<ResolvedStream> {
    let id = video_id.strip_prefix("ytm:").unwrap_or(video_id);
    if let Some(path) = cached_file_for(id) {
        tracing::info!(path = %path.display(), "YTM cache hit");
        return Ok(ResolvedStream {
            url: file_uri(&path),
            bitrate: None,
            format: path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_string),
            sample_rate: None,
            video_url: None,
            needs_android_ua: false,
        });
    }

    let page = format!("https://music.youtube.com/watch?v={id}");
    let bin = ytdlp_bin();

    // Prefer android player client — CDN URLs work with ANDROID_YT_UA (no 403).
    let out = ytdlp_base_cmd()
        .args([
            "--extractor-args",
            "youtube:player_client=android",
            "-f",
            "bestaudio/best",
            "-g",
            "--no-playlist",
            &page,
        ])
        .output()
        .with_context(|| format!("spawn {}", bin.display()))?;
    if out.status.success() {
        let stream_url = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !stream_url.is_empty() && url_is_playable(&stream_url, true) {
            return Ok(ResolvedStream {
                url: stream_url,
                bitrate: None,
                format: Some("ytdlp-android".into()),
                sample_rate: None,
                video_url: None,
                needs_android_ua: true,
            });
        }
        tracing::warn!("android CDN URL not playable; downloading to cache");
    } else {
        tracing::warn!(
            "android -g failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    // Reliable fallback: download with yt-dlp's own downloader, play local file.
    let dest = cache_dir().join(id);
    let path = download_best(id, &dest)?;
    Ok(ResolvedStream {
        url: file_uri(&path),
        bitrate: None,
        format: path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_string),
        sample_rate: None,
        video_url: None,
        needs_android_ua: false,
    })
}

/// Download best audio to `dest` (extension chosen by yt-dlp).
pub fn download_best(video_id: &str, dest_template: &Path) -> Result<PathBuf> {
    let id = video_id.strip_prefix("ytm:").unwrap_or(video_id);
    let url = format!("https://music.youtube.com/watch?v={id}");
    let bin = ytdlp_bin();
    if let Some(parent) = dest_template.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let template = format!("{}.%(ext)s", dest_template.display());
    let out = match ytdlp_base_cmd()
        .args([
            "--extractor-args",
            "youtube:player_client=android",
            "-f",
            "bestaudio/best",
            "--no-playlist",
            "--no-part",
            "-o",
            &template,
            &url,
        ])
        .output()
    {
        Ok(out) => out,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("yt-dlp is not installed; acquire needs it to download the track")
        }
        Err(err) => {
            return Err(err).with_context(|| format!("yt-dlp download {}", bin.display()))
        }
    };
    if !out.status.success() {
        let detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let detail = if detail.is_empty() {
            format!("yt-dlp exited {}", out.status)
        } else {
            detail
        };
        anyhow::bail!("yt-dlp download failed: {detail}");
    }
    let parent = dest_template
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = dest_template
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    choose_downloaded(&parent, stem)
}

const AUDIO_EXT: &[&str] = &["flac", "opus", "m4a", "mp3", "aac", "ogg", "webm", "wav", "mp4"];

/// Pick the downloaded audio file. Images and partials in the same folder are ignored.
pub fn choose_downloaded(parent: &Path, stem: &str) -> Result<PathBuf> {
    let mut best: Option<(usize, PathBuf)> = None;
    for entry in std::fs::read_dir(parent).with_context(|| format!("read {}", parent.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(stem) {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let Some(rank) = AUDIO_EXT.iter().position(|candidate| *candidate == ext) else {
            continue;
        };
        if best.as_ref().map(|(current, _)| rank < *current).unwrap_or(true) {
            best = Some((rank, path));
        }
    }
    best.map(|(_, path)| path).with_context(|| {
        format!(
            "yt-dlp finished but no audio file was found in {}",
            parent.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{choose_downloaded, download_best, native_probe_accepted};

    #[test]
    #[ignore = "downloads from YouTube"]
    fn download_best_writes_an_audio_file() {
        let dir = std::env::temp_dir().join(format!("nm-download-best-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = download_best("CizitNpshbM", &dir.join("tone")).expect("download");
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 1_000, "{len} bytes at {}", path.display());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn native_url_is_kept_only_when_the_probe_succeeds() {
        assert!(!native_probe_accepted("", true));
        assert!(!native_probe_accepted("https://example/audio", false));
        assert!(native_probe_accepted("https://example/audio", true));
    }

    #[test]
    fn download_prefers_flac_over_a_cover_or_webm() {
        let dir = std::env::temp_dir().join(format!(
            "nm-acquire-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01 Title.jpg"), b"cover").unwrap();
        std::fs::write(dir.join("01 Title.webm"), b"webm").unwrap();
        std::fs::write(dir.join("01 Title.flac"), b"flac").unwrap();
        let picked = choose_downloaded(&dir, "01 Title").unwrap();
        assert_eq!(picked.extension().and_then(|e| e.to_str()), Some("flac"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
