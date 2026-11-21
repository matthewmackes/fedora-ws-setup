//! Community-audit helpers (A045–A066) shared by Settings pages.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mde_config::MdeConfig;

use crate::system::{self, LayoutHead};

/// Map strength 0–100 → night kelvin (6500 cool → 2500 warm).
pub fn night_kelvin(strength: u8) -> u32 {
    let s = strength.min(100) as u32;
    6500u32.saturating_sub(s * 40).max(2500)
}

/// A059: night light with warmth strength (kills prior pid first).
pub fn set_night_light_strength(on: bool, strength: u8) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let pid_path = Path::new(&runtime).join("mde-night-light.pid");
    if let Ok(text) = fs::read_to_string(&pid_path) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        let _ = fs::remove_file(&pid_path);
    }
    if !on {
        return Ok(());
    }
    let k = night_kelvin(strength).to_string();
    let mut child = Command::new("wlsunset");
    child.args(["-t", &k, "-T", "6500"]);
    let spawned = child.spawn().or_else(|_| {
        let mut alt = Command::new("gammastep");
        alt.args(["-O", &k]);
        alt.spawn()
    });
    match spawned {
        Ok(child) => {
            let _ = fs::write(&pid_path, child.id().to_string());
            Ok(())
        }
        Err(e) => Err(format!("night light needs wlsunset or gammastep ({e})")),
    }
}

/// A060/A061: flash identity labels on each display.
pub fn identify_displays(heads: &[LayoutHead]) -> String {
    if heads.is_empty() {
        return "No displays to identify".into();
    }
    for (i, head) in heads.iter().enumerate() {
        let label = format!("{} · {}", i + 1, head.name);
        let _ = Command::new("notify-send")
            .args(["-t", "2500", "-a", "MDE", "Identify display", &label])
            .spawn();
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let _ = fs::write(
                Path::new(&runtime).join("mde-identify.req"),
                format!("{}\n{}", head.name, i + 1),
            );
        }
    }
    format!("Identifying {} display(s)", heads.len())
}

/// A045: push typed theme FM defaults into Thunar / GTK.
pub fn apply_fm_defaults(cfg: &MdeConfig) {
    let t = &cfg.active().theme;
    let _ = system::set_show_file_extensions(t.show_file_extensions);
    let _ = system::set_show_hidden_files(t.show_hidden_files);
}

/// A061: write project layout for four Win11 modes.
pub fn apply_project_mode(heads: &[LayoutHead], mode: &str) -> Result<String, String> {
    if heads.is_empty() {
        return Err("No displays detected".into());
    }
    let mode = mode.to_ascii_lowercase();
    let mut body = String::new();
    match mode.as_str() {
        "pc" | "pc_only" | "internal" => {
            for (i, h) in heads.iter().enumerate() {
                let enabled = if i == 0 { 1 } else { 0 };
                body.push_str(&format!("{} 0 0 {} {enabled}\n", h.name, h.scale));
            }
        }
        "duplicate" | "mirror" => {
            for h in heads {
                body.push_str(&format!("{} 0 0 {} 1\n", h.name, h.scale));
            }
        }
        "second" | "second_only" | "external" => {
            if heads.len() < 2 {
                return Err("Second screen only needs another display".into());
            }
            for (i, h) in heads.iter().enumerate() {
                let enabled = if i == 0 { 0 } else { 1 };
                body.push_str(&format!("{} 0 0 {} {enabled}\n", h.name, h.scale));
            }
        }
        _ => {
            // extend
            let mut x = 0i32;
            for h in heads {
                let w = parse_width(&h.mode).unwrap_or(1920);
                body.push_str(&format!("{} {x} 0 {} 1\n", h.name, h.scale));
                x = x.saturating_add(w.max(1));
            }
        }
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    fs::write(Path::new(&runtime).join("mde-output-layout.txt"), &body)
        .map_err(|e| format!("layout file: {e}"))?;
    let _ = Command::new("mde-comp")
        .args(["ctl", "apply-profile", "layout"])
        .output();
    let _ = identify_displays(heads);
    Ok(format!("Project · {mode}"))
}

fn parse_width(mode: &str) -> Option<i32> {
    let m = mode.split('@').next().unwrap_or(mode);
    let (w, _) = m.split_once('x')?;
    w.parse().ok()
}

/// A062: best-effort refresh/HDR via wlr-randr (when installed).
pub fn apply_head_tuning(head: &LayoutHead) -> String {
    let mut notes = Vec::new();
    if system::which("wlr-randr") {
        if head.refresh_hz > 0.0 {
            let mode = if head.mode.is_empty() {
                format!("{}Hz", head.refresh_hz.round() as i32)
            } else if head.mode.contains('@') {
                head.mode.clone()
            } else {
                format!("{}@{:.0}Hz", head.mode, head.refresh_hz)
            };
            let ok = Command::new("wlr-randr")
                .args(["--output", &head.name, "--mode", &mode])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            notes.push(if ok {
                mode
            } else {
                format!("mode {mode} (best-effort)")
            });
        }
        let hdr_arg = if head.hdr { "on" } else { "off" };
        let _ = Command::new("wlr-randr")
            .args(["--output", &head.name, "--hdr", hdr_arg])
            .status();
        notes.push(format!("HDR {hdr_arg}"));
    } else {
        notes.push(format!(
            "{} scale={} refresh={} hdr={}",
            head.name, head.scale, head.refresh_hz, head.hdr
        ));
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        let path = Path::new(&runtime).join("mde-output-tuning.txt");
        let line = format!(
            "{} scale={} refresh={} hdr={} mode={}\n",
            head.name, head.scale, head.refresh_hz, head.hdr as u8, head.mode
        );
        let body = fs::read_to_string(&path).unwrap_or_default();
        let mut kept = String::new();
        for existing in body.lines() {
            if !existing.starts_with(&format!("{} ", head.name)) {
                kept.push_str(existing);
                kept.push('\n');
            }
        }
        kept.push_str(&line);
        let _ = fs::write(path, kept);
    }
    notes.join(" · ")
}

/// A063: startup desktop entry with impact hint.
#[derive(Debug, Clone)]
pub struct StartupEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub impact: &'static str,
}

pub fn list_startup_entries() -> Vec<StartupEntry> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".config/autostart"));
    }
    dirs.push(PathBuf::from("/etc/xdg/autostart"));
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let id = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("app.desktop")
                .to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            let name = desktop_key(&text, "Name").unwrap_or_else(|| id.clone());
            let hidden = desktop_key(&text, "Hidden")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let x_gnome = desktop_key(&text, "X-GNOME-Autostart-enabled")
                .map(|v| !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true);
            let enabled = !hidden && x_gnome;
            let impact = startup_impact(&name, &id);
            out.push(StartupEntry {
                id,
                name,
                path,
                enabled,
                impact,
            });
            if out.len() >= 48 {
                break;
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn startup_impact(name: &str, id: &str) -> &'static str {
    let s = format!("{name} {id}").to_lowercase();
    if s.contains("dropbox")
        || s.contains("onedrive")
        || s.contains("syncthing")
        || s.contains("steam")
        || s.contains("teams")
        || s.contains("slack")
        || s.contains("discord")
    {
        "High"
    } else if s.contains("nm-applet")
        || s.contains("blueman")
        || s.contains("kdeconnect")
        || s.contains("agent")
        || s.contains("polkit")
    {
        "Medium"
    } else {
        "Low"
    }
}

pub fn set_startup_enabled(path: &Path, enabled: bool) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut saw_hidden = false;
    let mut saw_gnome = false;
    for line in &mut lines {
        if line.starts_with("Hidden=") {
            *line = format!("Hidden={}", if enabled { "false" } else { "true" });
            saw_hidden = true;
        }
        if line.starts_with("X-GNOME-Autostart-enabled=") {
            *line = format!(
                "X-GNOME-Autostart-enabled={}",
                if enabled { "true" } else { "false" }
            );
            saw_gnome = true;
        }
    }
    if !saw_hidden {
        lines.push(format!("Hidden={}", if enabled { "false" } else { "true" }));
    }
    if !saw_gnome {
        lines.push(format!(
            "X-GNOME-Autostart-enabled={}",
            if enabled { "true" } else { "false" }
        ));
    }
    // Prefer writing user override under ~/.config/autostart.
    let dest = if path.starts_with("/etc/") {
        let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
        let user_dir = PathBuf::from(home).join(".config/autostart");
        fs::create_dir_all(&user_dir).map_err(|e| e.to_string())?;
        user_dir.join(path.file_name().unwrap_or_default())
    } else {
        path.to_path_buf()
    };
    fs::write(&dest, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

fn desktop_key(desktop: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in desktop.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// A064: common defaults by type/protocol.
#[derive(Debug, Clone)]
pub struct DefaultSlot {
    pub label: &'static str,
    pub mime: &'static str,
    pub current: String,
}

pub fn default_app_slots() -> Vec<DefaultSlot> {
    let slots = [
        ("Web browser", "x-scheme-handler/http"),
        ("Email", "x-scheme-handler/mailto"),
        ("Music", "audio/mpeg"),
        ("Photos", "image/jpeg"),
        ("Video", "video/mp4"),
        ("PDF", "application/pdf"),
        ("Text", "text/plain"),
        ("Archive", "application/zip"),
    ];
    slots
        .into_iter()
        .map(|(label, mime)| DefaultSlot {
            label,
            mime,
            current: xdg_mime_default(mime),
        })
        .collect()
}

pub fn xdg_mime_default(mime: &str) -> String {
    let out = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                "(none)".into()
            } else {
                s
            }
        }
        _ => "(unavailable)".into(),
    }
}

pub fn set_xdg_mime_default(mime: &str, desktop_id: &str) -> Result<(), String> {
    let status = Command::new("xdg-mime")
        .args(["default", desktop_id, mime])
        .status()
        .map_err(|e| format!("xdg-mime: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdg-mime failed ({status})"))
    }
}

/// A065: uninstall / repair helpers.
pub fn uninstall_desktop_app(id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return "No app selected".into();
    }
    // Flatpak style ids often look like org.foo.Bar
    if id.contains('.') && !id.ends_with(".desktop") {
        let status = Command::new("flatpak")
            .args(["uninstall", "-y", id])
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return format!("Uninstalled flatpak {id}");
        }
    }
    let stem = id.strip_suffix(".desktop").unwrap_or(id);
    // Best-effort: open software centre or suggest rpm/dnf.
    if system::which("gnome-software") {
        let _ = Command::new("gnome-software")
            .args(["--details", stem])
            .spawn();
        return format!("Opened Software for {stem}");
    }
    if system::which("plasma-discover") {
        let _ = Command::new("plasma-discover")
            .args([format!("appstream://{stem}")])
            .spawn();
        return format!("Opened Discover for {stem}");
    }
    format!("Remove {stem} with dnf remove / flatpak uninstall")
}

pub fn reset_app_data(id: &str) -> String {
    let stem = id.strip_suffix(".desktop").unwrap_or(id);
    let Ok(home) = std::env::var("HOME") else {
        return "HOME unset".into();
    };
    let mut cleared = 0usize;
    for rel in [
        format!(".config/{stem}"),
        format!(".local/share/{stem}"),
        format!(".cache/{stem}"),
    ] {
        let p = PathBuf::from(&home).join(&rel);
        if p.is_dir() {
            let bak = p.with_extension("mde-bak");
            let _ = fs::rename(&p, &bak);
            cleared += 1;
        }
    }
    if cleared == 0 {
        format!("No user data found for {stem}")
    } else {
        format!("Reset {cleared} data folder(s) for {stem} (renamed *.mde-bak)")
    }
}

/// A054: Spotlight history under ~/Pictures/Spotlight.
pub fn spotlight_history() -> Vec<PathBuf> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let dir = PathBuf::from(home).join("Pictures/Spotlight");
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
        })
        .collect();
    paths.sort();
    paths.reverse();
    paths.truncate(12);
    paths
}

pub fn spotlight_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let dir = PathBuf::from(home).join("Pictures/Spotlight");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Copy current wallpaper into Spotlight history + optional lock wallpaper file.
pub fn record_spotlight(cfg: &mut MdeConfig, src: &Path) -> Result<(), String> {
    let dir = spotlight_dir()?;
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("spotlight.jpg");
    let dest = dir.join(name);
    fs::copy(src, &dest).map_err(|e| e.to_string())?;
    if cfg.active().theme.spotlight_lock {
        let lock = dir.join("lock-current");
        let _ = fs::copy(&dest, &lock);
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let _ = fs::write(
                Path::new(&runtime).join("mde-lock-wallpaper.txt"),
                dest.display().to_string(),
            );
        }
    }
    Ok(())
}

/// A066: enable Game Mode + optional DND.
pub fn set_game_mode_with_dnd(cfg: &mut MdeConfig, on: bool) -> Result<String, String> {
    system::set_gamemode(on)?;
    if on && cfg.active().session.game_mode_dnd {
        cfg.active_mut().notifications.dnd = true;
        let _ = cfg.save();
        Ok("Game Mode on · Do not disturb on".into())
    } else if !on && cfg.active().session.game_mode_dnd {
        cfg.active_mut().notifications.dnd = false;
        let _ = cfg.save();
        Ok("Game Mode off · Do not disturb off".into())
    } else if on {
        Ok("Game Mode on".into())
    } else {
        Ok("Game Mode off".into())
    }
}

/// A058: pick account picture via zenity (same as shell).
pub fn pick_account_picture() -> String {
    let out = Command::new("zenity")
        .args([
            "--file-selection",
            "--title=Choose account picture",
            "--file-filter=Images | *.png *.jpg *.jpeg *.webp",
        ])
        .output();
    let Ok(o) = out else {
        return "zenity is required to pick a picture".into();
    };
    if !o.status.success() {
        return "Cancelled".into();
    }
    let src = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if src.is_empty() || !Path::new(&src).is_file() {
        return "No image selected".into();
    }
    let Ok(home) = std::env::var("HOME") else {
        return "HOME unset".into();
    };
    let face = PathBuf::from(&home).join(".face");
    let avatar = PathBuf::from(&home)
        .join(".config/mde")
        .join("avatar.png");
    if let Some(parent) = avatar.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::copy(&src, &face);
    let _ = fs::copy(&src, &avatar);
    "Account picture updated".into()
}

/// A046 helpers.
pub fn desktop_display_name(cfg: &MdeConfig, index: usize) -> String {
    cfg.active()
        .desktops
        .names
        .get(index)
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("Desktop {}", index + 1))
}

pub fn set_desktop_name(cfg: &mut MdeConfig, index: usize, name: String) {
    let names = &mut cfg.active_mut().desktops.names;
    while names.len() <= index {
        names.push(String::new());
    }
    names[index] = name;
}

pub fn set_desktop_wallpaper(cfg: &mut MdeConfig, index: usize, path: String) {
    let walls = &mut cfg.active_mut().desktops.wallpapers;
    while walls.len() <= index {
        walls.push(String::new());
    }
    walls[index] = path;
}
