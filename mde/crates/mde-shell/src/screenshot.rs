//! Unified screenshot capture (W500–W515).
//!
//! Modes: screen / area / window / snip. Saves under Screenshots (or configured
//! dir), copies PNG to the clipboard, and prefers grim+slurp with ashpd portal
//! and optional tool fallbacks. The Win11 snip overlay uses [`CaptureOutcome`]
//! and opens swappy from the thumbnail toast instead of auto-launching it.

use std::path::{Path, PathBuf};
use std::process::Command;

use mde_protocol::ScreenshotMode;

use crate::which_bin;

/// Result of a capture used by the snip overlay / toast (W515).
#[derive(Debug, Clone)]
pub enum CaptureOutcome {
    Saved { path: PathBuf, label: String },
    Cancelled,
    Failed(String),
}

impl CaptureOutcome {
    pub fn status_line(&self) -> String {
        match self {
            Self::Saved { path, label } => {
                format!("Screenshot · {label} · {}", path.display())
            }
            Self::Cancelled => "Screenshot · cancelled".into(),
            Self::Failed(e) => format!("Screenshot · {e}"),
        }
    }
}

/// Resolve save directory: `shell.screenshot_dir` if set, else `{PICTURES}/Screenshots`.
pub fn screenshot_dir() -> PathBuf {
    if let Ok(cfg) = mde_config::MdeConfig::load() {
        let custom = cfg.active().shell.screenshot_dir.trim();
        if !custom.is_empty() {
            let p = PathBuf::from(shellexpand_home(custom));
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
    }
    default_screenshot_dir()
}

pub fn default_screenshot_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let pictures = Command::new("xdg-user-dir")
        .arg("PICTURES")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| format!("{home}/Pictures"));
    let dir = PathBuf::from(pictures).join("Screenshots");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn shellexpand_home(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    if s == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| s.to_string());
    }
    s.to_string()
}

fn stamp_path(dir: &Path) -> PathBuf {
    let stamp = mde_theme::now().format("%Y%m%d-%H%M%S");
    dir.join(format!("Screenshot-{stamp}.png"))
}

fn wl_copy_png(path: &Path) {
    if !which_bin("wl-copy") {
        return;
    }
    let _ = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                let bytes = std::fs::read(path)?;
                use std::io::Write;
                stdin.write_all(&bytes)?;
            }
            child.wait()?;
            Ok(())
        });
}

fn notify_saved(path: &Path) {
    if !which_bin("notify-send") {
        return;
    }
    let label = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Screenshot");
    let _ = Command::new("notify-send")
        .args(["-i", "camera-photo", "Screenshot saved", label])
        .spawn();
}

fn finish_ok(label: &str, path: &Path) -> CaptureOutcome {
    wl_copy_png(path);
    notify_saved(path);
    CaptureOutcome::Saved {
        path: path.to_path_buf(),
        label: label.into(),
    }
}

fn grim_to(path: &Path, geom: Option<&str>) -> Result<(), String> {
    if !which_bin("grim") {
        return Err("grim not installed".into());
    }
    let mut cmd = Command::new("grim");
    if let Some(g) = geom {
        cmd.args(["-g", g]);
    }
    cmd.arg(path);
    match cmd.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(format!("grim failed writing {}", path.display())),
        Err(e) => Err(format!("grim spawn failed: {e}")),
    }
}

/// Region select via slurp. Distinguishes cancel vs seat failure (W512).
fn slurp_geom() -> Result<String, SlurpErr> {
    if !which_bin("slurp") {
        return Err(SlurpErr::Missing);
    }
    match Command::new("slurp").output() {
        Ok(o) if o.status.success() => {
            let g = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if g.is_empty() {
                Err(SlurpErr::Cancelled)
            } else {
                Ok(g)
            }
        }
        Ok(o) => {
            let code = o.status.code().unwrap_or(1);
            if code == 1 {
                Err(SlurpErr::Cancelled)
            } else {
                Err(SlurpErr::Failed(format!("slurp exit {code}")))
            }
        }
        Err(e) => Err(SlurpErr::Failed(e.to_string())),
    }
}

enum SlurpErr {
    Missing,
    Cancelled,
    Failed(String),
}

/// Open the region annotator (toast click / explicit edit).
pub fn open_swappy(path: &Path) {
    if which_bin("swappy") {
        let _ = Command::new("swappy")
            .args(["-f", &path.to_string_lossy()])
            .spawn();
        return;
    }
    let _ = Command::new("xdg-open").arg(path).spawn();
}

fn portal_screenshot_blocking(out: &Path) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let response = ashpd::desktop::screenshot::Screenshot::request()
            .interactive(false)
            .modal(true)
            .send()
            .await
            .map_err(|e| format!("portal request: {e}"))?
            .response()
            .map_err(|e| format!("portal cancelled or failed: {e}"))?;
        let uri = response.uri().as_str();
        let src = uri.strip_prefix("file://").unwrap_or(uri);
        std::fs::copy(src, out).map_err(|e| format!("copy portal file: {e}"))?;
        Ok(())
    })
}

fn fallback_tools(out: &Path) -> Option<CaptureOutcome> {
    if which_bin("gnome-screenshot") {
        if Command::new("gnome-screenshot")
            .args(["-f", &out.to_string_lossy()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(finish_ok("gnome-screenshot", out));
        }
    }
    if which_bin("spectacle") {
        if Command::new("spectacle")
            .args(["-b", "-o", &out.to_string_lossy()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(finish_ok("spectacle", out));
        }
    }
    None
}

/// Run a typed screenshot capture; returns structured outcome for the GUI toast.
pub fn capture(mode: ScreenshotMode, geom: Option<&str>) -> CaptureOutcome {
    let dir = screenshot_dir();
    let out = stamp_path(&dir);

    match mode {
        ScreenshotMode::Screen => capture_screen(&out),
        // Overlay owns the editor via toast — never auto-open swappy here.
        ScreenshotMode::Area | ScreenshotMode::Snip => capture_region(&out),
        ScreenshotMode::Window => {
            if let Some(g) = geom.filter(|s| !s.is_empty()) {
                match grim_to(&out, Some(g)) {
                    Ok(()) => finish_ok("window", &out),
                    Err(e) => CaptureOutcome::Failed(e),
                }
            } else {
                CaptureOutcome::Failed("no focused window geometry".into())
            }
        }
    }
}

/// Legacy status-string wrapper (diagnostics / non-GUI callers).
pub fn spawn_screenshot(mode: ScreenshotMode, geom: Option<&str>) -> String {
    capture(mode, geom).status_line()
}

fn capture_screen(out: &Path) -> CaptureOutcome {
    match grim_to(out, None) {
        Ok(()) => finish_ok("screen", out),
        Err(grim_err) => match portal_screenshot_blocking(out) {
            Ok(()) => finish_ok("portal", out),
            Err(portal_err) => {
                if let Some(msg) = fallback_tools(out) {
                    return msg;
                }
                CaptureOutcome::Failed(format!(
                    "failed — {grim_err}; portal: {portal_err}; install grim (preferred), or gnome-screenshot / spectacle"
                ))
            }
        },
    }
}

fn capture_region(out: &Path) -> CaptureOutcome {
    match slurp_geom() {
        Ok(g) => match grim_to(out, Some(&g)) {
            Ok(()) => finish_ok("region", out),
            Err(e) => CaptureOutcome::Failed(e),
        },
        Err(SlurpErr::Cancelled) => CaptureOutcome::Cancelled,
        Err(SlurpErr::Missing) => match portal_screenshot_blocking(out) {
            Ok(()) => finish_ok("portal", out),
            Err(e) => CaptureOutcome::Failed(format!("region needs slurp ({e}); install grim+slurp")),
        },
        Err(SlurpErr::Failed(e)) => CaptureOutcome::Failed(format!(
            "region failed ({e}) — not falling back to fullscreen"
        )),
    }
}
