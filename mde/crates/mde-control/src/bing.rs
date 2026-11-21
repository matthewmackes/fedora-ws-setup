//! Bing wallpaper of the day for Settings → Background.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use mde_config::MdeConfig;
use serde::Deserialize;

use crate::system;

const ARCHIVE: &str = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=en-US";
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";
const TIMER_NAME: &str = "mde-bing-wallpaper.timer";
const SERVICE_NAME: &str = "mde-bing-wallpaper.service";

#[derive(Debug, Deserialize)]
struct Archive {
    images: Vec<ArchiveImage>,
}

#[derive(Debug, Deserialize)]
struct ArchiveImage {
    urlbase: String,
    startdate: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    copyright: String,
}

#[derive(Debug, Clone)]
pub struct BingMeta {
    pub path: PathBuf,
    pub title: String,
    pub copyright: String,
}

/// Soft network/remote failure — keep current wallpaper; timer should exit 0.
#[derive(Debug)]
pub enum RefreshError {
    Soft(String),
    Hard(String),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::Soft(s) | RefreshError::Hard(s) => write!(f, "{s}"),
        }
    }
}

fn pictures_bing_dir() -> Result<PathBuf, RefreshError> {
    let home = std::env::var("HOME").map_err(|_| RefreshError::Hard("HOME is unset".into()))?;
    let dir = PathBuf::from(home).join("Pictures").join("Bing");
    fs::create_dir_all(&dir).map_err(|e| RefreshError::Hard(format!("create {dir:?}: {e}")))?;
    Ok(dir)
}

fn systemd_user_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is unset".to_string())?;
    let dir = PathBuf::from(home).join(".config/systemd/user");
    fs::create_dir_all(dir.join("timers.target.wants"))
        .map_err(|e| format!("systemd user dir: {e}"))?;
    Ok(dir)
}

fn control_bin() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.into_os_string().into_string().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mde-control".into())
}

fn curl_bytes(url: &str, timeout_secs: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            timeout_secs,
            "-A",
            UA,
            url,
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "curl failed ({})",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn fetch_meta() -> Result<ArchiveImage, RefreshError> {
    let body = curl_bytes(ARCHIVE, "20").map_err(RefreshError::Soft)?;
    let archive: Archive =
        serde_json::from_slice(&body).map_err(|e| RefreshError::Soft(format!("Bing JSON: {e}")))?;
    let image = archive
        .images
        .into_iter()
        .next()
        .ok_or_else(|| RefreshError::Soft("Bing returned no images".into()))?;
    if image.urlbase.trim().is_empty() || image.startdate.trim().is_empty() {
        return Err(RefreshError::Soft("Bing image metadata incomplete".into()));
    }
    Ok(image)
}

/// Strip trailing `_1920x1080` / `_UHD` from urlbase path, then build UHD URL + filename stem.
fn uhd_url_and_name(urlbase: &str) -> Result<(String, String), RefreshError> {
    let base = urlbase.split('&').next().unwrap_or(urlbase).trim();
    let stripped = {
        let re_end = ["_UHD", "_1920x1200", "_1920x1080", "_1366x768", "_1280x720", "_1024x768"];
        let mut s = base.to_string();
        for suf in re_end {
            if let Some(rest) = s.strip_suffix(suf) {
                s = rest.to_string();
                break;
            }
        }
        s
    };
    let id = stripped
        .rsplit("id=")
        .next()
        .unwrap_or(stripped.as_str())
        .rsplit('/')
        .next()
        .unwrap_or(stripped.as_str());
    if id.is_empty() {
        return Err(RefreshError::Soft("Bing urlbase has no id".into()));
    }
    let name = format!("{id}_UHD.jpg");
    let url = format!("https://www.bing.com{stripped}_UHD.jpg");
    Ok((url, name))
}

fn jpeg_looks_valid(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if meta.len() < 20_000 {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff
}

fn download_uhd(url: &str, dest: &Path) -> Result<(), RefreshError> {
    if jpeg_looks_valid(dest) {
        return Ok(());
    }
    let bytes = curl_bytes(url, "60").map_err(RefreshError::Soft)?;
    if bytes.len() < 20_000 || bytes.get(0..3) != Some(&[0xff, 0xd8, 0xff]) {
        return Err(RefreshError::Soft("Bing download was not a JPEG".into()));
    }
    let tmp = dest.with_extension("jpg.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| RefreshError::Hard(format!("write {}: {e}", tmp.display())))?;
        f.write_all(&bytes)
            .map_err(|e| RefreshError::Hard(format!("write {}: {e}", tmp.display())))?;
    }
    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        RefreshError::Hard(format!("rename {}: {e}", dest.display()))
    })?;
    Ok(())
}

/// Download today's UHD Bing image into ~/Pictures/Bing/ (does not mutate config).
pub fn download_today() -> Result<BingMeta, RefreshError> {
    let image = fetch_meta()?;
    let (url, name) = uhd_url_and_name(&image.urlbase)?;
    let dest = pictures_bing_dir()?.join(name);
    download_uhd(&url, &dest)?;
    Ok(BingMeta {
        path: dest,
        title: image.title,
        copyright: image.copyright,
    })
}

/// Download, update config wallpaper + Bing caption fields, save, and apply via swaybg.
pub fn refresh_and_apply(cfg: &mut MdeConfig) -> Result<String, RefreshError> {
    let meta = download_today()?;
    let path = meta.path.to_string_lossy().into_owned();
    {
        let theme = &mut cfg.active_mut().theme;
        theme.wallpaper = path.clone();
        theme.bing_title = meta.title;
        theme.bing_copyright = meta.copyright;
    }
    cfg.save()
        .map_err(|e| RefreshError::Hard(format!("save config: {e}")))?;
    system::set_wallpaper(&path).map_err(RefreshError::Hard)?;
    if cfg.active().theme.spotlight {
        let p = std::path::Path::new(&path);
        let _ = crate::community::record_spotlight(cfg, p);
    }
    Ok(path)
}

/// CLI / timer entry: no-op when daily is off; soft failures exit success for the timer.
pub fn run_cli() -> i32 {
    let mut cfg = match MdeConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mde-control --bing-wallpaper: load config: {e}");
            return 1;
        }
    };
    if !cfg.active().theme.bing_daily && !cfg.active().theme.spotlight {
        return 0;
    }
    match refresh_and_apply(&mut cfg) {
        Ok(path) => {
            eprintln!("Bing wallpaper: {path}");
            0
        }
        Err(RefreshError::Soft(msg)) => {
            eprintln!("Bing wallpaper: {msg} (keeping current)");
            0
        }
        Err(RefreshError::Hard(msg)) => {
            eprintln!("Bing wallpaper: {msg}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::uhd_url_and_name;

    #[test]
    fn uhd_name_from_urlbase() {
        let (url, name) =
            uhd_url_and_name("/th?id=OHR.KnowledgeSculpture_EN-US9746703110").unwrap();
        assert_eq!(name, "OHR.KnowledgeSculpture_EN-US9746703110_UHD.jpg");
        assert_eq!(
            url,
            "https://www.bing.com/th?id=OHR.KnowledgeSculpture_EN-US9746703110_UHD.jpg"
        );
    }

    #[test]
    fn strips_existing_size_suffix() {
        let (_, name) = uhd_url_and_name("/th?id=OHR.Foo_EN-US_1920x1080").unwrap();
        assert_eq!(name, "OHR.Foo_EN-US_UHD.jpg");
    }
}

pub fn set_daily_timer(enabled: bool) -> Result<(), String> {
    let units = systemd_user_dir()?;
    let service_path = units.join(SERVICE_NAME);
    let timer_path = units.join(TIMER_NAME);
    let bin = control_bin();

    let service = format!(
        "[Unit]\nDescription=Bing wallpaper of the day (MDE)\n\n[Service]\nType=oneshot\nExecStart={bin} --bing-wallpaper\n"
    );
    let timer = "\
[Unit]
Description=Bing wallpaper of the day (MDE)

[Timer]
OnStartupSec=2min
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
";

    if enabled {
        fs::write(&service_path, service).map_err(|e| format!("write {SERVICE_NAME}: {e}"))?;
        fs::write(&timer_path, timer).map_err(|e| format!("write {TIMER_NAME}: {e}"))?;
        let link = units.join("timers.target.wants").join(TIMER_NAME);
        if link.exists() || link.is_symlink() {
            let _ = fs::remove_file(&link);
        }
        std::os::unix::fs::symlink(format!("../{TIMER_NAME}"), &link)
            .map_err(|e| format!("symlink timer: {e}"))?;
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let status = Command::new("systemctl")
            .args(["--user", "enable", "--now", TIMER_NAME])
            .status()
            .map_err(|e| format!("systemctl enable: {e}"))?;
        if !status.success() {
            return Err(format!("systemctl enable --now {TIMER_NAME} failed"));
        }
    } else {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", TIMER_NAME])
            .status();
    }
    Ok(())
}
