//! Community / PowerToys-like helpers (A015–A042).
//! Pure logic + state files under `~/.local/state/mde/`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(home).join(".local/state/mde");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ── A015 Command Palette ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PaletteHit {
    pub kind: &'static str,
    pub label: String,
    pub action: String,
}

/// Ranked palette results for apps / files / calc / shell plugins.
pub fn palette_search(
    query: &str,
    plugins: &[String],
    apps: &[(String, String)],
    files: &[(String, String)],
    limit: usize,
) -> Vec<PaletteHit> {
    let q = query.trim();
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    let ql = q.to_lowercase();
    let has = |name: &str| plugins.iter().any(|p| p.eq_ignore_ascii_case(name));

    if has("calc") {
        if let Some(v) = eval_simple_calc(q) {
            out.push(PaletteHit {
                kind: "calc",
                label: format!("= {v}"),
                action: format!("calc:{v}"),
            });
        }
    }
    if has("apps") {
        for (id, name) in apps {
            let nl = name.to_lowercase();
            let il = id.to_lowercase();
            if nl.contains(&ql) || il.contains(&ql) {
                out.push(PaletteHit {
                    kind: "app",
                    label: name.clone(),
                    action: format!("app:{id}"),
                });
                if out.len() >= limit {
                    return out;
                }
            }
        }
    }
    if has("files") {
        for (path, label) in files {
            if label.to_lowercase().contains(&ql) || path.to_lowercase().contains(&ql) {
                out.push(PaletteHit {
                    kind: "file",
                    label: label.clone(),
                    action: format!("file:{path}"),
                });
                if out.len() >= limit {
                    return out;
                }
            }
        }
    }
    if has("shell") {
        out.push(PaletteHit {
            kind: "shell",
            label: format!("Run: {q}"),
            action: format!("shell:{q}"),
        });
    }
    out.truncate(limit);
    out
}

fn eval_simple_calc(expr: &str) -> Option<String> {
    let e = expr.trim().replace(' ', "");
    if e.is_empty() || !e.chars().all(|c| c.is_ascii_digit() || "+-*/.()".contains(c)) {
        return None;
    }
    // Tiny left-to-right +−×÷ evaluator (no precedence — good enough for MVP).
    let mut nums: Vec<f64> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let mut cur = String::new();
    for c in e.chars() {
        if c.is_ascii_digit() || c == '.' {
            cur.push(c);
        } else if "+-*/".contains(c) {
            nums.push(cur.parse().ok()?);
            cur.clear();
            ops.push(c);
        } else {
            return None;
        }
    }
    if !cur.is_empty() {
        nums.push(cur.parse().ok()?);
    }
    if nums.is_empty() || ops.len() != nums.len().saturating_sub(1) {
        return None;
    }
    let mut acc = nums[0];
    for (i, op) in ops.iter().enumerate() {
        let n = nums[i + 1];
        acc = match op {
            '+' => acc + n,
            '-' => acc - n,
            '*' => acc * n,
            '/' => {
                if n == 0.0 {
                    return None;
                }
                acc / n
            }
            _ => return None,
        };
    }
    if (acc - acc.round()).abs() < 1e-9 {
        Some(format!("{}", acc.round() as i64))
    } else {
        Some(format!("{acc:.6}").trim_end_matches('0').trim_end_matches('.').to_string())
    }
}

// ── A017 Snap Groups ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapGroupMember {
    pub app_id: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapGroup {
    pub id: String,
    pub label: String,
    pub members: Vec<SnapGroupMember>,
    #[serde(default)]
    pub saved_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SnapGroupsFile {
    #[serde(default)]
    groups: Vec<SnapGroup>,
}

fn snap_groups_path() -> PathBuf {
    state_dir().join("snap-groups.json")
}

pub fn load_snap_groups() -> Vec<SnapGroup> {
    let Ok(text) = std::fs::read_to_string(snap_groups_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<SnapGroupsFile>(&text)
        .map(|f| f.groups)
        .unwrap_or_default()
}

pub fn save_snap_group(group: SnapGroup) {
    let mut file = std::fs::read_to_string(snap_groups_path())
        .ok()
        .and_then(|t| serde_json::from_str::<SnapGroupsFile>(&t).ok())
        .unwrap_or_default();
    file.groups.retain(|g| g.id != group.id);
    file.groups.insert(0, group);
    file.groups.truncate(12);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(snap_groups_path(), text);
    }
}

pub fn write_restore_group(group: &SnapGroup) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = PathBuf::from(runtime).join("mde-snap-group-restore.req");
    let mut body = String::new();
    for m in &group.members {
        body.push_str(&format!(
            "{}|{}|{}|{}|{}|{}\n",
            m.app_id, m.title, m.x, m.y, m.w, m.h
        ));
    }
    std::fs::write(&path, body).map_err(|e| format!("restore group · {e}"))
}

// ── A019 Color history ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSample {
    pub hex: String,
    #[serde(default)]
    pub rgb: String,
    #[serde(default)]
    pub hsl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ColorHistoryFile {
    #[serde(default)]
    colors: Vec<ColorSample>,
}

fn color_history_path() -> PathBuf {
    state_dir().join("color-history.json")
}

pub fn load_color_history(limit: usize) -> Vec<ColorSample> {
    let Ok(text) = std::fs::read_to_string(color_history_path()) else {
        return Vec::new();
    };
    let mut colors = serde_json::from_str::<ColorHistoryFile>(&text)
        .map(|f| f.colors)
        .unwrap_or_default();
    colors.truncate(limit);
    colors
}

pub fn push_color_history(hex: &str) {
    let hex = normalize_hex(hex);
    if hex.is_empty() {
        return;
    }
    let (r, g, b) = parse_hex_rgb(&hex).unwrap_or((0, 0, 0));
    let sample = ColorSample {
        hex: hex.clone(),
        rgb: format!("rgb({r}, {g}, {b})"),
        hsl: rgb_to_hsl_string(r, g, b),
    };
    let mut file = std::fs::read_to_string(color_history_path())
        .ok()
        .and_then(|t| serde_json::from_str::<ColorHistoryFile>(&t).ok())
        .unwrap_or_default();
    file.colors.retain(|c| !c.hex.eq_ignore_ascii_case(&hex));
    file.colors.insert(0, sample);
    file.colors.truncate(24);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(color_history_path(), text);
    }
}

fn normalize_hex(s: &str) -> String {
    let t = s.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    if t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", t.to_ascii_uppercase())
    } else {
        String::new()
    }
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let t = hex.trim().trim_start_matches('#');
    if t.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&t[0..2], 16).ok()?,
        u8::from_str_radix(&t[2..4], 16).ok()?,
        u8::from_str_radix(&t[4..6], 16).ok()?,
    ))
}

fn rgb_to_hsl_string(r: u8, g: u8, b: u8) -> String {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return format!("hsl(0, 0%, {}%)", (l * 100.0).round() as i32);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - rf).abs() < 1e-6 {
        ((gf - bf) / d) % 6.0
    } else if (max - gf).abs() < 1e-6 {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    };
    let h_deg = (h * 60.0).rem_euclid(360.0);
    format!(
        "hsl({}, {}%, {}%)",
        h_deg.round() as i32,
        (s * 100.0).round() as i32,
        (l * 100.0).round() as i32
    )
}

// ── A023 Awake ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AwakeState {
    pub active: bool,
    /// Unix secs when awake ends; 0 = indefinite.
    pub until_unix: u64,
}

fn awake_path() -> PathBuf {
    state_dir().join("awake.json")
}

pub fn load_awake() -> AwakeState {
    let Ok(text) = std::fs::read_to_string(awake_path()) else {
        return AwakeState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_awake(state: &AwakeState) {
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(awake_path(), text);
    }
}

pub fn awake_is_active(state: &AwakeState, now_unix: u64) -> bool {
    if !state.active {
        return false;
    }
    if state.until_unix == 0 {
        return true;
    }
    now_unix < state.until_unix
}

// ── A030 Jump list pin forever ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct JumpPinsFile {
    /// desktop_id → pinned absolute paths
    #[serde(default)]
    pins: std::collections::BTreeMap<String, Vec<String>>,
}

fn jump_pins_path() -> PathBuf {
    state_dir().join("jump-pins.json")
}

pub fn jump_pinned_files(desktop_id: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(jump_pins_path()) else {
        return Vec::new();
    };
    let file: JumpPinsFile = serde_json::from_str(&text).unwrap_or_default();
    file.pins.get(desktop_id).cloned().unwrap_or_default()
}

pub fn jump_pin_forever(desktop_id: &str, path: &str) {
    let desktop_id = desktop_id.trim();
    let path = path.trim();
    if desktop_id.is_empty() || path.is_empty() {
        return;
    }
    let mut file = std::fs::read_to_string(jump_pins_path())
        .ok()
        .and_then(|t| serde_json::from_str::<JumpPinsFile>(&t).ok())
        .unwrap_or_default();
    let entry = file.pins.entry(desktop_id.into()).or_default();
    if !entry.iter().any(|p| p == path) {
        entry.insert(0, path.into());
    }
    entry.truncate(16);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(jump_pins_path(), text);
    }
}

pub fn jump_unpin_forever(desktop_id: &str, path: &str) {
    let mut file = std::fs::read_to_string(jump_pins_path())
        .ok()
        .and_then(|t| serde_json::from_str::<JumpPinsFile>(&t).ok())
        .unwrap_or_default();
    if let Some(entry) = file.pins.get_mut(desktop_id) {
        entry.retain(|p| p != path);
    }
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(jump_pins_path(), text);
    }
}

pub fn jump_clear_recent_for_app(desktop_id: &str) {
    // Clear forever-pins for this app; dismiss.json handles Start recommended.
    let mut file = std::fs::read_to_string(jump_pins_path())
        .ok()
        .and_then(|t| serde_json::from_str::<JumpPinsFile>(&t).ok())
        .unwrap_or_default();
    file.pins.remove(desktop_id);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(jump_pins_path(), text);
    }
}

// ── A032 Per-app notification prefs ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppNotifPref {
    pub app: String,
    #[serde(default = "default_true_pref")]
    pub banners: bool,
    #[serde(default = "default_true_pref")]
    pub sound: bool,
    #[serde(default)]
    pub priority: bool,
}

fn default_true_pref() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppNotifPrefsFile {
    #[serde(default)]
    apps: Vec<AppNotifPref>,
}

fn app_notif_prefs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/mde/app-notif-prefs.json")
}

pub fn load_app_notif_prefs() -> Vec<AppNotifPref> {
    let Ok(text) = std::fs::read_to_string(app_notif_prefs_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<AppNotifPrefsFile>(&text)
        .map(|f| f.apps)
        .unwrap_or_default()
}

pub fn save_app_notif_prefs(apps: &[AppNotifPref]) {
    let path = app_notif_prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = AppNotifPrefsFile {
        apps: apps.to_vec(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
}

pub fn upsert_app_notif_pref(pref: AppNotifPref) {
    let mut apps = load_app_notif_prefs();
    if let Some(existing) = apps
        .iter_mut()
        .find(|a| a.app.eq_ignore_ascii_case(&pref.app))
    {
        *existing = pref;
    } else {
        apps.push(pref);
    }
    save_app_notif_prefs(&apps);
}

pub fn pref_for_app(app: &str) -> AppNotifPref {
    let low = app.trim().to_lowercase();
    load_app_notif_prefs()
        .into_iter()
        .find(|a| a.app.eq_ignore_ascii_case(&low))
        .unwrap_or(AppNotifPref {
            app: low,
            banners: true,
            sound: true,
            priority: false,
        })
}

// ── A042 Advanced Paste transforms ──────────────────────────────────────────

pub fn transform_clip(kind: &str, text: &str) -> Result<String, String> {
    let t = text;
    match kind {
        "plain" => Ok(strip_html_lite(t)),
        "upper" => Ok(t.to_uppercase()),
        "lower" => Ok(t.to_lowercase()),
        "markdown" => Ok(html_to_markdown_lite(t)),
        "html" => Ok(markdown_to_html_lite(t)),
        "json" => format_json(t),
        "trim" => Ok(t.lines().map(str::trim).collect::<Vec<_>>().join("\n")),
        other => Err(format!("unknown transform · {other}")),
    }
}

fn strip_html_lite(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn html_to_markdown_lite(s: &str) -> String {
    let plain = strip_html_lite(s);
    // Already markdown-ish? leave alone.
    if !s.contains('<') {
        return s.to_string();
    }
    plain
}

fn markdown_to_html_lite(s: &str) -> String {
    let mut out = String::from("<p>");
    for (i, line) in s.lines().enumerate() {
        if i > 0 {
            out.push_str("<br/>");
        }
        let mut escaped = String::new();
        for c in line.chars() {
            match c {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                _ => escaped.push(c),
            }
        }
        // **bold**
        let mut rendered = String::new();
        let chars: Vec<char> = escaped.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if let Some(end) = chars[i + 2..].windows(2).position(|w| w == ['*', '*']) {
                    rendered.push_str("<b>");
                    for c in &chars[i + 2..i + 2 + end] {
                        rendered.push(*c);
                    }
                    rendered.push_str("</b>");
                    i += 4 + end;
                    continue;
                }
            }
            rendered.push(chars[i]);
            i += 1;
        }
        out.push_str(&rendered);
    }
    out.push_str("</p>");
    out
}

fn format_json(s: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(s.trim()).map_err(|e| format!("JSON · {e}"))?;
    serde_json::to_string_pretty(&v).map_err(|e| format!("JSON · {e}"))
}

// ── A016 zone apply helper ──────────────────────────────────────────────────

/// Write a custom zone geometry request for the focused window.
pub fn write_zone_snap(zone_idx: usize, x: f32, y: f32, w: f32, h: f32) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = PathBuf::from(runtime).join("mde-snap.req");
    // Custom zone: `zone <idx> <x> <y> <w> <h>` fractions — compositor may
    // fall back to preset if unsupported; shell also writes absolute hint.
    let body = format!("zone {zone_idx} {x:.4} {y:.4} {w:.4} {h:.4}\n");
    std::fs::write(&path, body).map_err(|e| format!("zone snap · {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_add() {
        assert_eq!(eval_simple_calc("2+3").as_deref(), Some("5"));
        assert_eq!(eval_simple_calc("10/4").as_deref(), Some("2.5"));
    }

    #[test]
    fn transform_json() {
        let out = transform_clip("json", r#"{"a":1}"#).unwrap();
        assert!(out.contains("\"a\""));
    }

    #[test]
    fn hex_hsl() {
        let s = rgb_to_hsl_string(255, 0, 0);
        assert!(s.starts_with("hsl("));
    }
}
