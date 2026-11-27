//! MDE single configuration store (`~/.config/mde/config.toml`).
//!
//! Replaces scattered `ws-settings` / `ws-taskbar` / `ws-start` JSON and the
//! legacy Regedit tree. Human-readable TOML with schema versioning, profiles,
//! and a Win11-styled Editor in `mde-control`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod style_packs;
pub mod supervisor;

pub use style_packs::{list_packs, overlay_theme, StylePack};

pub const SCHEMA_VERSION: u32 = 1;
pub const ROOT_DIR_NAME: &str = "mde";
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Top-level document written to disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MdeConfig {
    pub schema: u32,
    #[serde(default)]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    /// Keys not yet modelled as typed fields (advanced / extension bag).
    #[serde(default)]
    pub extras: BTreeMap<String, toml::Value>,
}

impl Default for MdeConfig {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert("default".into(), Profile::default());
        Self {
            schema: SCHEMA_VERSION,
            active_profile: "default".into(),
            profiles,
            extras: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default)]
    pub start: StartConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub displays: DisplaysConfig,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub compositor: CompositorConfig,
    /// Widgets board card visibility (A043 — quiet defaults).
    #[serde(default)]
    pub widgets: WidgetsConfig,
    /// PowerToys-like / community productivity tools (A015–A042).
    #[serde(default)]
    pub tools: ToolsConfig,
    /// A046: virtual desktop names + wallpapers.
    #[serde(default)]
    pub desktops: DesktopsConfig,
    /// Accessibility / a11y (A070–A073, A092, A095).
    #[serde(default)]
    pub accessibility: AccessibilityConfig,
    /// Storage Sense analogue (A090).
    #[serde(default)]
    pub storage: StorageConfig,
    /// Touchpad gesture map (A094).
    #[serde(default)]
    pub gestures: GesturesConfig,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            display_name: "Default".into(),
            shell: ShellConfig::default(),
            start: StartConfig::default(),
            theme: ThemeConfig::default(),
            displays: DisplaysConfig::default(),
            session: SessionConfig::default(),
            notifications: NotificationsConfig::default(),
            clipboard: ClipboardConfig::default(),
            network: NetworkConfig::default(),
            compositor: CompositorConfig::default(),
            widgets: WidgetsConfig::default(),
            tools: ToolsConfig::default(),
            desktops: DesktopsConfig::default(),
            accessibility: AccessibilityConfig::default(),
            storage: StorageConfig::default(),
            gestures: GesturesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellConfig {
    #[serde(default = "default_align")]
    pub taskbar_align: String,
    #[serde(default = "default_search")]
    pub taskbar_search: String,
    #[serde(default)]
    pub taskbar_autohide: bool,
    #[serde(default = "default_true")]
    pub taskbar_badges: bool,
    #[serde(default = "default_true")]
    pub taskbar_all_displays: bool,
    #[serde(default = "default_combine")]
    pub taskbar_combine: String,
    /// When true, show app name labels beside taskbar icons (A005).
    #[serde(default)]
    pub taskbar_labels: bool,
    /// Taskbar density: `comfortable` (48px) or `compact` (36px) — A008.
    #[serde(default = "default_taskbar_size")]
    pub taskbar_size: String,
    /// When true, left-align the pin/start cluster (W146); false keeps center.
    #[serde(default)]
    pub taskbar_left: bool,
    /// When true, taskbar date uses `%Y-%m-%d` instead of `%a %-m/%-d` (W93).
    #[serde(default)]
    pub clock_date_iso: bool,
    /// When true, taskbar clock includes seconds (W237).
    #[serde(default)]
    pub clock_seconds: bool,
    /// A082: always offer End task on jump lists (Win11 developer-style).
    #[serde(default = "default_true")]
    pub end_task_always: bool,
    /// A096: multi-monitor taskbar — `all` | `main_buttons` | `main_only`.
    #[serde(default = "default_taskbar_multi")]
    pub taskbar_multi_mode: String,
    /// A098: up to two IANA timezones shown in Notification Centre header.
    #[serde(default)]
    pub world_clocks: Vec<String>,
    /// When true, show Widgets button at the far left of the taskbar (W226 / A003).
    /// Default OFF for quiet-desktop; Win+W still opens the board.
    #[serde(default = "default_false")]
    pub widgets_btn: bool,
    /// Absolute or `~/…` captures folder; empty → `{PICTURES}/Screenshots` (W505).
    #[serde(default)]
    pub screenshot_dir: String,
    #[serde(default)]
    pub pins: Vec<String>,
}

fn default_align() -> String {
    "center".into()
}
fn default_search() -> String {
    "hide".into()
}
fn default_combine() -> String {
    "always".into()
}
fn default_taskbar_size() -> String {
    "comfortable".into()
}
fn default_taskbar_multi() -> String {
    "all".into()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            taskbar_align: default_align(),
            taskbar_search: default_search(),
            taskbar_autohide: false,
            taskbar_badges: true,
            taskbar_all_displays: true,
            taskbar_combine: default_combine(),
            taskbar_labels: false,
            taskbar_size: default_taskbar_size(),
            taskbar_left: false,
            clock_date_iso: false,
            clock_seconds: false,
            end_task_always: true,
            taskbar_multi_mode: default_taskbar_multi(),
            world_clocks: Vec::new(),
            widgets_btn: false,
            screenshot_dir: String::new(),
            pins: vec![
                "thunar.desktop".into(),
                "org.mozilla.firefox.desktop".into(),
                "terminator.desktop".into(),
                "mde-control.desktop".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartConfig {
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default = "default_true")]
    pub recently_added: bool,
    #[serde(default)]
    pub most_used: bool,
    /// Kept for config compat; Start never shows Recommended (A012 no-promo).
    #[serde(default = "default_false")]
    pub recommended_files: bool,
    /// A012: Tips / tips card opt-in only (off by default).
    #[serde(default = "default_false")]
    pub tips_opt_in: bool,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub pins: Vec<String>,
    /// Desktop ids moved off Start into Settings › Advanced (flat launch list).
    #[serde(default)]
    pub advanced_apps: Vec<String>,
    /// Start pin folder names (`mde-folder:Name` → `Name`) moved into Advanced.
    #[serde(default)]
    pub advanced_groups: Vec<String>,
}

fn default_layout() -> String {
    // A010: More pins is the community-primary Start density.
    "more_pins".into()
}

impl Default for StartConfig {
    fn default() -> Self {
        Self {
            layout: default_layout(),
            recently_added: true,
            most_used: false,
            recommended_files: false,
            tips_opt_in: false,
            folders: Vec::new(),
            pins: Vec::new(),
            advanced_apps: Vec::new(),
            advanced_groups: Vec::new(),
        }
    }
}

impl StartConfig {
    /// Folder display name without the `mde-folder:` prefix.
    pub fn folder_name(id: &str) -> &str {
        id.strip_prefix("mde-folder:").unwrap_or(id)
    }

    pub fn is_advanced_app(&self, id: &str) -> bool {
        self.advanced_apps.iter().any(|a| a == id)
    }

    pub fn is_advanced_group(&self, name_or_id: &str) -> bool {
        let name = Self::folder_name(name_or_id);
        self.advanced_groups
            .iter()
            .any(|g| g.eq_ignore_ascii_case(name))
    }

    /// True when a Start pin should be hidden because it lives in Advanced.
    pub fn pin_is_advanced(&self, pin: &str) -> bool {
        if pin.starts_with("mde-folder:") {
            self.is_advanced_group(pin)
        } else {
            self.is_advanced_app(pin)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeConfig {
    #[serde(default = "default_accent")]
    pub accent: String,
    /// `light` | `dark`
    #[serde(default = "default_light")]
    pub mode: String,
    #[serde(default = "default_true")]
    pub animations: bool,
    /// Win11 “Transparency effects” — shell chrome, flyouts, and translucent surfaces.
    #[serde(default = "default_true")]
    pub transparency: bool,
    /// Chrome opacity percent when transparency is on (1–100). Default 90 (Mica-like).
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    /// A007: more opaque over windows / clearer over empty desktop.
    #[serde(default = "default_true")]
    pub dynamic_transparency: bool,
    /// A007 acrylic-style blur strength preset: `off` | `subtle` | `acrylic` | `thick`.
    #[serde(default = "default_blur_preset")]
    pub blur_preset: String,
    /// Tint Start and taskbar selected/active chrome with the accent colour.
    #[serde(default = "default_true")]
    pub accent_on_start_taskbar: bool,
    /// Accent wash / underline on server-side title bars.
    #[serde(default = "default_true")]
    pub accent_on_titlebars: bool,
    /// Prefer always-visible scrollbars (disables overlay scrollbars where supported).
    #[serde(default)]
    pub always_show_scrollbars: bool,
    /// Image path. Empty leaves the background untouched.
    #[serde(default)]
    pub wallpaper: String,
    /// Daily Bing wallpaper of the day via user systemd timer.
    #[serde(default)]
    pub bing_daily: bool,
    /// A054: Spotlight pack — rotate Bing onto desktop + lock + keep history.
    #[serde(default)]
    pub spotlight: bool,
    /// A054: also apply Spotlight image as lock-screen wallpaper.
    #[serde(default = "default_true")]
    pub spotlight_lock: bool,
    /// Last Bing image title (display only).
    #[serde(default)]
    pub bing_title: String,
    /// Last Bing image copyright credit (display only).
    #[serde(default)]
    pub bing_copyright: String,
    /// When mode is light, use dark colours between `dark_from` and `dark_until`.
    #[serde(default)]
    pub dark_schedule: bool,
    /// A051: `fixed` (HH:MM window) or `sunrise_sunset` (approx local solar).
    #[serde(default = "default_dark_schedule_mode")]
    pub dark_schedule_mode: String,
    /// Local time `HH:MM`.
    #[serde(default = "default_dark_from")]
    pub dark_from: String,
    /// Local time `HH:MM`. Overnight windows wrap past midnight.
    #[serde(default = "default_dark_until")]
    pub dark_until: String,
    /// A045: show file extensions in Thunar / FM defaults.
    #[serde(default = "default_true")]
    pub show_file_extensions: bool,
    /// A045: show hidden files in Thunar / FM defaults.
    #[serde(default)]
    pub show_hidden_files: bool,
    /// A050: SSD title-bar corner radius in px (0 = square).
    #[serde(default = "default_ssd_radius")]
    pub ssd_corner_radius: u8,
    /// A050: mica-like translucent SSD title bars when transparency is on.
    #[serde(default = "default_true")]
    pub ssd_mica: bool,
    /// A087: active style-pack id (empty = stock tokens).
    #[serde(default)]
    pub style_pack: String,
    /// A087: load packs from `~/.local/share/mde/styles/*.toml`.
    #[serde(default = "default_true")]
    pub style_packs_enabled: bool,
    /// UI chrome typeface (Personalization → Fonts). Default Inter.
    #[serde(default = "default_ui_font")]
    pub ui_font: String,
    /// Monospace typeface (Personalization → Fonts). Default Cascadia Mono.
    #[serde(default = "default_mono_font")]
    pub mono_font: String,
}

fn default_accent() -> String {
    "#0078d4".into()
}
fn default_light() -> String {
    "light".into()
}
fn default_dark_from() -> String {
    "20:00".into()
}
fn default_dark_until() -> String {
    "07:00".into()
}
fn default_opacity() -> u8 {
    90
}
fn default_blur_preset() -> String {
    "acrylic".into()
}
fn default_dark_schedule_mode() -> String {
    "fixed".into()
}
fn default_ssd_radius() -> u8 {
    8
}
fn default_ui_font() -> String {
    "Inter".into()
}
fn default_mono_font() -> String {
    "Cascadia Mono".into()
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent: default_accent(),
            mode: default_light(),
            animations: true,
            transparency: true,
            opacity: default_opacity(),
            dynamic_transparency: true,
            blur_preset: default_blur_preset(),
            accent_on_start_taskbar: true,
            accent_on_titlebars: true,
            always_show_scrollbars: false,
            wallpaper: String::new(),
            bing_daily: false,
            spotlight: false,
            spotlight_lock: true,
            bing_title: String::new(),
            bing_copyright: String::new(),
            dark_schedule: false,
            dark_schedule_mode: default_dark_schedule_mode(),
            dark_from: default_dark_from(),
            dark_until: default_dark_until(),
            show_file_extensions: true,
            show_hidden_files: false,
            ssd_corner_radius: default_ssd_radius(),
            ssd_mica: true,
            style_pack: String::new(),
            style_packs_enabled: true,
            ui_font: default_ui_font(),
            mono_font: default_mono_font(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplaysConfig {
    #[serde(default)]
    pub main_display: String,
    #[serde(default)]
    pub profiles: Vec<DisplayProfile>,
    #[serde(default = "default_true")]
    pub allow_fractional_scale: bool,
    /// A061: last Super+P project mode — `pc` | `duplicate` | `extend` | `second`.
    #[serde(default = "default_project_mode")]
    pub project_mode: String,
}

fn default_project_mode() -> String {
    "extend".into()
}

impl Default for DisplaysConfig {
    fn default() -> Self {
        Self {
            main_display: String::new(),
            profiles: Vec::new(),
            allow_fractional_scale: true,
            project_mode: default_project_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayProfile {
    pub name: String,
    #[serde(default)]
    pub outputs: Vec<OutputHead>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputHead {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: String,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub transform: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// A062: preferred refresh rate Hz (0 = compositor default).
    #[serde(default)]
    pub refresh_hz: f64,
    /// A062: request HDR when the output supports it (best-effort).
    #[serde(default)]
    pub hdr: bool,
}

fn default_scale() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionConfig {
    #[serde(default = "default_idle")]
    pub idle_lock_seconds: u64,
    #[serde(default = "default_dpms")]
    pub idle_dpms_seconds: u64,
    #[serde(default)]
    pub xwayland: bool,
    #[serde(default)]
    pub night_light: bool,
    /// A059: night light warmth strength 0–100 (maps to kelvin).
    #[serde(default = "default_night_strength")]
    pub night_light_strength: u8,
    /// A059: schedule night light independently of dark colour schedule.
    #[serde(default)]
    pub night_light_schedule: bool,
    #[serde(default = "default_dark_from")]
    pub night_from: String,
    #[serde(default = "default_dark_until")]
    pub night_until: String,
    /// A066: when Game Mode is on, also enable Do not disturb.
    #[serde(default = "default_true")]
    pub game_mode_dnd: bool,
}

fn default_idle() -> u64 {
    300
}
fn default_dpms() -> u64 {
    330
}
fn default_night_strength() -> u8 {
    50
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            idle_lock_seconds: default_idle(),
            idle_dpms_seconds: default_dpms(),
            xwayland: false,
            night_light: false,
            night_light_strength: default_night_strength(),
            night_light_schedule: false,
            night_from: default_dark_from(),
            night_until: default_dark_until(),
            game_mode_dnd: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub dnd: bool,
    /// Focus assist “priority only” — suppress unknown apps (W86).
    #[serde(default)]
    pub priority: bool,
    #[serde(default = "default_toast")]
    pub toast_seconds: u64,
    /// Quiet hours start `HH:MM` (empty → fall back to theme dark schedule).
    #[serde(default)]
    pub quiet_from: String,
    /// Quiet hours end `HH:MM`.
    #[serde(default)]
    pub quiet_to: String,
}

fn default_toast() -> u64 {
    5
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            dnd: false,
            priority: false,
            toast_seconds: default_toast(),
            quiet_from: String::new(),
            quiet_to: String::new(),
        }
    }
}

/// Widgets board card toggles. Quiet community default: core cards on, news/markets/traffic off (A043).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WidgetsConfig {
    #[serde(default = "default_true")]
    pub weather: bool,
    #[serde(default = "default_true")]
    pub clock: bool,
    #[serde(default = "default_true")]
    pub calendar: bool,
    #[serde(default = "default_true")]
    pub system: bool,
    #[serde(default = "default_true")]
    pub tip: bool,
    #[serde(default = "default_true")]
    pub photos: bool,
    #[serde(default = "default_true")]
    pub background: bool,
    /// Off by default (quiet desktop).
    #[serde(default = "default_false")]
    pub news: bool,
    #[serde(default = "default_false")]
    pub markets: bool,
    #[serde(default = "default_false")]
    pub traffic: bool,
    /// Load third-party cards from `~/.local/share/mde/widgets/*.json` (A043).
    #[serde(default = "default_true")]
    pub third_party: bool,
}

impl Default for WidgetsConfig {
    fn default() -> Self {
        Self {
            weather: true,
            clock: true,
            calendar: true,
            system: true,
            tip: true,
            photos: true,
            background: true,
            news: false,
            markets: false,
            traffic: false,
            third_party: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardConfig {
    #[serde(default = "default_true")]
    pub history: bool,
    #[serde(default = "default_clip_max")]
    pub max_items: u32,
    /// Share text clips via a folder (Syncthing/Nextcloud) — Win11 cloud clipboard analogue.
    #[serde(default)]
    pub cross_device: bool,
    /// Absolute path; empty uses `~/.local/share/mde/clipboard-sync`.
    #[serde(default)]
    pub sync_dir: String,
}

fn default_clip_max() -> u32 {
    // W283: default clipboard history depth (extras clipboard.max_items can override).
    25
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            history: true,
            max_items: default_clip_max(),
            cross_device: false,
            sync_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfig {
    #[serde(default = "default_true")]
    pub wifi: bool,
    #[serde(default = "default_true")]
    pub bluetooth: bool,
    #[serde(default)]
    pub airplane_mode: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            wifi: true,
            bluetooth: true,
            airplane_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositorConfig {
    /// Window layout id. Runtime is floating windows, snap, and workspaces.
    /// The legacy "tabs" value remains the default string so old config files
    /// still load. The server tab strip stays off.
    #[serde(default = "default_layout_mode")]
    pub layout: String,
    /// When true, compositor may paint a top tab strip. Off by default (Win11-like chrome).
    #[serde(default = "default_false")]
    pub server_tab_strip: bool,
    /// Window decorations policy. MDE standard is server-side only (`server`).
    #[serde(default = "default_decorations")]
    pub decorations: String,
    /// A049: Alt+drag move, Alt+right-drag resize (AltSnap).
    #[serde(default = "default_true")]
    pub alt_snap: bool,
}

fn default_layout_mode() -> String {
    "tabs".into()
}

fn default_decorations() -> String {
    "server".into()
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            layout: default_layout_mode(),
            server_tab_strip: false,
            decorations: default_decorations(),
            alt_snap: true,
        }
    }
}

/// A046: named virtual desktops + optional per-desktop wallpaper paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesktopsConfig {
    /// Display names keyed by 0-based index (missing → “Desktop N”).
    #[serde(default)]
    pub names: Vec<String>,
    /// Wallpaper paths keyed by 0-based index (empty → use theme.wallpaper).
    #[serde(default)]
    pub wallpapers: Vec<String>,
}

impl Default for DesktopsConfig {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            wallpapers: Vec::new(),
        }
    }
}

impl DesktopsConfig {
    /// Display name for a 0-based virtual desktop index.
    pub fn name_for(&self, index: usize) -> String {
        self.names
            .get(index)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Desktop {}", index + 1))
    }

    /// Per-desktop wallpaper when set and non-empty.
    pub fn wallpaper_for(&self, index: usize) -> Option<String> {
        self.wallpapers.get(index).and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
    }

    fn ensure_len(&mut self, index: usize) {
        if self.names.len() <= index {
            self.names.resize(index + 1, String::new());
        }
        if self.wallpapers.len() <= index {
            self.wallpapers.resize(index + 1, String::new());
        }
    }

    pub fn set_name(&mut self, index: usize, name: String) {
        self.ensure_len(index);
        self.names[index] = name;
    }

    pub fn set_wallpaper(&mut self, index: usize, path: String) {
        self.ensure_len(index);
        self.wallpapers[index] = path;
    }
}

/// A059: map strength 0–100 to colour temperature (kelvin).
pub fn night_light_kelvin(strength: u8) -> u32 {
    let s = strength.clamp(0, 100) as f32 / 100.0;
    (6500.0 - s * 3300.0).round() as u32
}

fn parse_hhmm_minutes(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// True when local time sits in `[from, until)`; overnight windows wrap past midnight.
pub fn minutes_in_hhmm_window(now_minutes: u16, from: &str, until: &str) -> bool {
    let Some(from) = parse_hhmm_minutes(from) else {
        return false;
    };
    let Some(until) = parse_hhmm_minutes(until) else {
        return false;
    };
    if from == until {
        return false;
    }
    if from < until {
        now_minutes >= from && now_minutes < until
    } else {
        now_minutes >= from || now_minutes < until
    }
}

/// A059: night light schedule uses `session.night_from` / `night_until`.
pub fn night_light_schedule_active(session: &SessionConfig) -> bool {
    if !session.night_light_schedule {
        return false;
    }
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    let secs = now.as_secs() % 86_400;
    let minutes = (secs / 60) as u16;
    minutes_in_hhmm_window(minutes, &session.night_from, &session.night_until)
}

/// Effective night-light on state (manual toggle or schedule).
pub fn night_light_effective(session: &SessionConfig) -> bool {
    session.night_light || night_light_schedule_active(session)
}

/// Community / PowerToys-like tools (A015–A042).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolsConfig {
    /// A015: Command Palette (Super+Alt+Space / enhanced Run).
    #[serde(default = "default_true")]
    pub command_palette: bool,
    /// Enabled palette plugins: apps, files, calc, shell.
    #[serde(default = "default_palette_plugins")]
    pub palette_plugins: Vec<String>,
    /// A016: FancyZones custom layouts.
    #[serde(default = "default_true")]
    pub fancy_zones: bool,
    #[serde(default = "default_zone_layouts")]
    pub zone_layouts: Vec<ZoneLayout>,
    #[serde(default = "default_active_zone")]
    pub active_zone_layout: String,
    /// A017: persist + restore Snap Groups.
    #[serde(default = "default_true")]
    pub snap_groups: bool,
    /// A018: Win+Ctrl+T always-on-top shortcut.
    #[serde(default = "default_true")]
    pub always_on_top_shortcut: bool,
    /// A018: title-bar pin glyph when always-on-top.
    #[serde(default = "default_true")]
    pub always_on_top_indicator: bool,
    /// A019: keep color picker history.
    #[serde(default = "default_true")]
    pub color_picker_history: bool,
    /// A022: key remaps (from → to keysym names).
    #[serde(default)]
    pub keyboard_remaps: Vec<KeyRemap>,
    /// A023: Awake default duration minutes (0 = indefinite).
    #[serde(default)]
    pub awake_minutes: u32,
    /// A024: Find My Mouse (Ctrl×2).
    #[serde(default = "default_true")]
    pub find_my_mouse: bool,
    /// A024: click highlighter overlay.
    #[serde(default)]
    pub mouse_highlighter: bool,
    /// A026: named workspace launch presets.
    #[serde(default)]
    pub workspaces: Vec<WorkspacePreset>,
    /// A030: jump list recent file depth.
    #[serde(default = "default_jump_max")]
    pub jump_list_max: u32,
    /// A042: Advanced Paste transforms in clipboard flyout.
    #[serde(default = "default_true")]
    pub advanced_paste: bool,
    /// A073: Shortcut Guide overlay (hold Super / Super+/).
    #[serde(default = "default_true")]
    pub shortcut_guide: bool,
    /// A076: Image Resizer context action / presets.
    #[serde(default = "default_true")]
    pub image_resizer: bool,
    /// A076: preset labels — `50%`, `1024w`, `1920w`, `Small`, etc.
    #[serde(default = "default_image_presets")]
    pub image_resizer_presets: Vec<String>,
}

fn default_palette_plugins() -> Vec<String> {
    vec![
        "apps".into(),
        "files".into(),
        "calc".into(),
        "shell".into(),
    ]
}

fn default_jump_max() -> u32 {
    8
}

fn default_active_zone() -> String {
    "Columns".into()
}

fn default_image_presets() -> Vec<String> {
    vec![
        "50%".into(),
        "1024w".into(),
        "1920w".into(),
        "Small".into(),
    ]
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            command_palette: true,
            palette_plugins: default_palette_plugins(),
            fancy_zones: true,
            zone_layouts: default_zone_layouts(),
            active_zone_layout: "Columns".into(),
            snap_groups: true,
            always_on_top_shortcut: true,
            always_on_top_indicator: true,
            color_picker_history: true,
            keyboard_remaps: Vec::new(),
            awake_minutes: 0,
            find_my_mouse: true,
            mouse_highlighter: false,
            workspaces: Vec::new(),
            jump_list_max: default_jump_max(),
            advanced_paste: true,
            shortcut_guide: true,
            image_resizer: true,
            image_resizer_presets: default_image_presets(),
        }
    }
}

/// Accessibility (A070 magnifier, A072 text scale, A092 privacy, A095 tablet/OSK).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessibilityConfig {
    /// Prefer compositor scale magnifier over external tools (A070).
    #[serde(default = "default_true")]
    pub magnifier_native: bool,
    /// Magnifier scale factor (typically 1.5–4.0).
    #[serde(default = "default_mag_factor")]
    pub magnifier_factor: f64,
    /// `full` screen zoom or `lens` (follow pointer).
    #[serde(default = "default_mag_mode")]
    pub magnifier_mode: String,
    /// Keep magnifier centred on keyboard focus when possible.
    #[serde(default = "default_true")]
    pub magnifier_follow_focus: bool,
    /// System-wide text scale (also published to gsettings).
    #[serde(default = "default_text_scale")]
    pub text_scale: f64,
    /// Taskbar privacy indicators when mic/camera/location are in use (A092).
    #[serde(default = "default_true")]
    pub privacy_indicators: bool,
    /// Auto-show on-screen keyboard on text focus for touch (A095).
    #[serde(default)]
    pub auto_osk: bool,
    /// Tablet posture: `off` | `auto` | `on`.
    #[serde(default = "default_tablet_mode")]
    pub tablet_mode: String,
}

fn default_mag_factor() -> f64 {
    2.0
}
fn default_mag_mode() -> String {
    "full".into()
}
fn default_text_scale() -> f64 {
    1.0
}
fn default_tablet_mode() -> String {
    "off".into()
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            magnifier_native: true,
            magnifier_factor: default_mag_factor(),
            magnifier_mode: default_mag_mode(),
            magnifier_follow_focus: true,
            text_scale: default_text_scale(),
            privacy_indicators: true,
            auto_osk: false,
            tablet_mode: default_tablet_mode(),
        }
    }
}

/// Storage Sense analogue (A090).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    #[serde(default)]
    pub sense_enabled: bool,
    #[serde(default = "default_true")]
    pub sense_temp: bool,
    #[serde(default = "default_true")]
    pub sense_trash: bool,
    /// Delete Downloads older than N days (0 = never).
    #[serde(default = "default_sense_days")]
    pub sense_downloads_days: u32,
    /// Run cleanup when free space is low.
    #[serde(default = "default_true")]
    pub sense_run_low_disk: bool,
}

fn default_sense_days() -> u32 {
    30
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            sense_enabled: false,
            sense_temp: true,
            sense_trash: true,
            sense_downloads_days: default_sense_days(),
            sense_run_low_disk: true,
        }
    }
}

/// Touchpad gesture map (A094). Action ids: `none`, `taskview`, `desktop`,
/// `desktop_prev`, `desktop_next`, `start`, `overview`, `snap`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GesturesConfig {
    #[serde(default = "default_g_up")]
    pub three_up: String,
    #[serde(default = "default_g_down")]
    pub three_down: String,
    #[serde(default = "default_g_left")]
    pub three_left: String,
    #[serde(default = "default_g_right")]
    pub three_right: String,
    #[serde(default = "default_g_four_up")]
    pub four_up: String,
    #[serde(default = "default_g_four_tap")]
    pub four_tap: String,
}

fn default_g_up() -> String {
    "taskview".into()
}
fn default_g_down() -> String {
    "desktop".into()
}
fn default_g_left() -> String {
    "desktop_prev".into()
}
fn default_g_right() -> String {
    "desktop_next".into()
}
fn default_g_four_up() -> String {
    "overview".into()
}
fn default_g_four_tap() -> String {
    "start".into()
}

impl Default for GesturesConfig {
    fn default() -> Self {
        Self {
            three_up: default_g_up(),
            three_down: default_g_down(),
            three_left: default_g_left(),
            three_right: default_g_right(),
            four_up: default_g_four_up(),
            four_tap: default_g_four_tap(),
        }
    }
}

/// One FancyZones layout: zones as fractions of the output (0.0–1.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneLayout {
    pub name: String,
    #[serde(default)]
    pub zones: Vec<ZoneRect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

fn default_zone_layouts() -> Vec<ZoneLayout> {
    vec![
        ZoneLayout {
            name: "Columns".into(),
            zones: vec![
                ZoneRect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.5,
                    h: 1.0,
                },
                ZoneRect {
                    x: 0.5,
                    y: 0.0,
                    w: 0.5,
                    h: 1.0,
                },
            ],
        },
        ZoneLayout {
            name: "Three columns".into(),
            zones: vec![
                ZoneRect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.33,
                    h: 1.0,
                },
                ZoneRect {
                    x: 0.33,
                    y: 0.0,
                    w: 0.34,
                    h: 1.0,
                },
                ZoneRect {
                    x: 0.67,
                    y: 0.0,
                    w: 0.33,
                    h: 1.0,
                },
            ],
        },
        ZoneLayout {
            name: "Priority grid".into(),
            zones: vec![
                ZoneRect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.66,
                    h: 1.0,
                },
                ZoneRect {
                    x: 0.66,
                    y: 0.0,
                    w: 0.34,
                    h: 0.5,
                },
                ZoneRect {
                    x: 0.66,
                    y: 0.5,
                    w: 0.34,
                    h: 0.5,
                },
            ],
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyRemap {
    /// Source keysym name, e.g. `Caps_Lock` or `a`.
    pub from: String,
    /// Destination keysym name, e.g. `Escape` or `b`.
    pub to: String,
}

/// Launch a set of apps into saved positions (A026).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacePreset {
    pub name: String,
    #[serde(default)]
    pub apps: Vec<WorkspaceApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceApp {
    /// Desktop id or executable.
    pub id: String,
    /// Optional zone index into the active FancyZones layout (−1 = free).
    #[serde(default = "default_zone_idx")]
    pub zone: i32,
}

fn default_zone_idx() -> i32 {
    -1
}

/// Flat key path used by the Config Editor (e.g. `shell.taskbar_align`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigKey {
    pub path: String,
    pub category: String,
    pub label: String,
    pub advanced: bool,
}

impl MdeConfig {
    pub fn config_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("no config dir")?;
        Ok(base.join(ROOT_DIR_NAME))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE_NAME))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.is_file() {
            return Self::load_from(&path);
        }
        let mut cfg = Self::default();
        cfg.ensure_community_profiles();
        let _ = cfg.migrate_legacy();
        cfg.save()?;
        Ok(cfg)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut cfg: Self = toml::from_str(&text).context("parse config.toml")?;
        cfg.ensure_community_profiles();
        Ok(cfg)
    }

    /// A100: seed Community Classic + Stock Win11 named profiles if missing.
    pub fn ensure_community_profiles(&mut self) {
        if !self.profiles.contains_key("community-classic") {
            let mut p = Profile::default();
            p.display_name = "Community Classic".into();
            p.shell.taskbar_align = "left".into();
            p.shell.taskbar_left = true;
            p.shell.widgets_btn = false;
            p.shell.taskbar_search = "hide".into();
            p.start.layout = "more_pins".into();
            p.start.tips_opt_in = false;
            self.profiles.insert("community-classic".into(), p);
        }
        if !self.profiles.contains_key("stock-win11") {
            let mut p = Profile::default();
            p.display_name = "Stock Win11".into();
            p.shell.taskbar_align = "center".into();
            p.shell.taskbar_left = false;
            p.shell.widgets_btn = true;
            p.shell.taskbar_search = "icon".into();
            p.start.layout = "more_pins".into();
            self.profiles.insert("stock-win11".into(), p);
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)?;
        let path = dir.join(CONFIG_FILE_NAME);
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).context("serialize config")?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn active(&self) -> &Profile {
        self.profiles
            .get(&self.active_profile)
            .or_else(|| self.profiles.values().next())
            .expect("at least one profile")
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        let key = if self.profiles.contains_key(&self.active_profile) {
            self.active_profile.clone()
        } else {
            let k = self
                .profiles
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "default".into());
            self.active_profile = k.clone();
            if !self.profiles.contains_key(&k) {
                self.profiles.insert(k.clone(), Profile::default());
            }
            k
        };
        self.profiles.get_mut(&key).expect("profile")
    }

    /// Names of Start pin folders from `extras.start.pin_folders`.
    pub fn start_pin_folder_names(&self) -> Vec<String> {
        let Some(arr) = self.extras.get("start.pin_folders").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        arr.iter()
            .filter_map(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }

    /// Apps inside a Start pin folder (by display name).
    pub fn start_pin_folder_apps(&self, name: &str) -> Vec<String> {
        let Some(arr) = self.extras.get("start.pin_folders").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        for item in arr {
            let n = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if n.eq_ignore_ascii_case(name) {
                return item
                    .get("apps")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
            }
        }
        Vec::new()
    }

    /// Folder names currently present as `mde-folder:…` pins on Start (in use).
    pub fn start_folders_in_use(&self) -> Vec<String> {
        self.active()
            .start
            .pins
            .iter()
            .filter(|p| p.starts_with("mde-folder:"))
            .map(|p| StartConfig::folder_name(p).to_string())
            .collect()
    }

    /// Import a full document (Editor import).
    pub fn import_toml(text: &str) -> Result<Self> {
        let cfg: Self = toml::from_str(text).context("import toml")?;
        Ok(cfg)
    }

    pub fn export_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn create_profile(&mut self, name: &str, display_name: &str) -> Result<()> {
        if self.profiles.contains_key(name) {
            anyhow::bail!("profile {name} already exists");
        }
        let mut p = self.active().clone();
        p.display_name = display_name.into();
        self.profiles.insert(name.into(), p);
        Ok(())
    }

    /// Catalog of editable keys for the Config Editor.
    pub fn catalog() -> Vec<ConfigKey> {
        vec![
            key("shell.taskbar_align", "Shell", "Taskbar alignment", false),
            key("shell.taskbar_left", "Shell", "Left-align taskbar pins", false),
            key("shell.taskbar_search", "Shell", "Search button style", false),
            key("shell.taskbar_autohide", "Shell", "Auto-hide taskbar", false),
            key("shell.taskbar_combine", "Shell", "Combine taskbar buttons", true),
            key("shell.taskbar_labels", "Shell", "Show taskbar labels", false),
            key("shell.taskbar_size", "Shell", "Taskbar size (compact/comfortable)", false),
            key("shell.taskbar_badges", "Shell", "Taskbar notification badges", false),
            key("shell.widgets_btn", "Shell", "Taskbar Widgets button", false),
            key("shell.clock_seconds", "Shell", "Taskbar clock seconds", false),
            key(
                "shell.screenshot_dir",
                "Shell",
                "Screenshots folder (empty = Pictures/Screenshots)",
                false,
            ),
            key("start.layout", "Start", "Start layout", false),
            key("start.recently_added", "Start", "Show recently added", false),
            key(
                "start.advanced_apps",
                "Start",
                "Apps moved to Settings › Advanced",
                true,
            ),
            key(
                "start.advanced_groups",
                "Start",
                "Start folders moved to Settings › Advanced",
                true,
            ),
            key("theme.accent", "Theme", "Accent colour", false),
            key("theme.ui_font", "Theme", "UI font family", false),
            key("theme.mono_font", "Theme", "Monospace font family", false),
            key("theme.mode", "Theme", "Colour mode", false),
            key("theme.animations", "Theme", "Animations", false),
            key("theme.transparency", "Theme", "Transparency effects", false),
            key("theme.opacity", "Theme", "Chrome opacity (1–100)", false),
            key(
                "theme.dynamic_transparency",
                "Theme",
                "Dynamic taskbar opacity over windows",
                false,
            ),
            key("theme.blur_preset", "Theme", "Acrylic blur preset", false),
            key(
                "theme.accent_on_start_taskbar",
                "Theme",
                "Accent colour on Start and taskbar",
                false,
            ),
            key(
                "theme.accent_on_titlebars",
                "Theme",
                "Accent colour on title bars",
                false,
            ),
            key(
                "theme.always_show_scrollbars",
                "Theme",
                "Always show scrollbars",
                false,
            ),
            key("theme.wallpaper", "Theme", "Wallpaper image path", false),
            key("theme.bing_daily", "Theme", "Daily Bing wallpaper", false),
            key("theme.spotlight", "Theme", "Spotlight wallpaper pack", false),
            key("theme.spotlight_lock", "Theme", "Spotlight on lock screen", false),
            key("theme.dark_schedule", "Theme", "Dark colour schedule", false),
            key(
                "theme.dark_schedule_mode",
                "Theme",
                "Dark schedule mode (fixed/sunrise_sunset)",
                false,
            ),
            key("theme.dark_from", "Theme", "Dark from (HH:MM)", false),
            key("theme.dark_until", "Theme", "Dark until (HH:MM)", false),
            key(
                "theme.show_file_extensions",
                "Theme",
                "Show file extensions (file manager)",
                false,
            ),
            key(
                "theme.show_hidden_files",
                "Theme",
                "Show hidden files (file manager)",
                false,
            ),
            key(
                "theme.ssd_corner_radius",
                "Theme",
                "SSD title-bar corner radius",
                false,
            ),
            key("theme.ssd_mica", "Theme", "Mica SSD title bars", false),
            key("displays.main_display", "Displays", "Main display", false),
            key(
                "displays.allow_fractional_scale",
                "Displays",
                "Allow fractional scale",
                true,
            ),
            key(
                "displays.project_mode",
                "Displays",
                "Project mode (pc/duplicate/extend/second)",
                false,
            ),
            key("session.idle_lock_seconds", "Session", "Lock after (seconds)", false),
            key("session.xwayland", "Session", "Enable XWayland", true),
            key("session.night_light", "Session", "Night light", false),
            key(
                "session.night_light_strength",
                "Session",
                "Night light strength (0–100)",
                false,
            ),
            key(
                "session.night_light_schedule",
                "Session",
                "Night light schedule",
                false,
            ),
            key("session.game_mode_dnd", "Session", "Game Mode enables DND", false),
            key(
                "compositor.alt_snap",
                "Compositor",
                "Alt+drag move/resize (AltSnap)",
                false,
            ),
            key("notifications.dnd", "Notifications", "Do not disturb", false),
            key(
                "notifications.toast_seconds",
                "Notifications",
                "Banner duration (seconds; extras notifications.toast_secs overrides)",
                false,
            ),
            key(
                "notifications.quiet_from",
                "Notifications",
                "Quiet hours from (HH:MM; empty uses theme.dark_from)",
                false,
            ),
            key(
                "notifications.quiet_to",
                "Notifications",
                "Quiet hours until (HH:MM; empty uses theme.dark_until)",
                false,
            ),
            key("clipboard.history", "Clipboard", "Clipboard history", false),
            key(
                "clipboard.max_items",
                "Clipboard",
                "History max items (extras clipboard.max_items overrides; default 25)",
                false,
            ),
            key(
                "clipboard.cross_device",
                "Clipboard",
                "Cross-device clipboard sync",
                false,
            ),
            key(
                "clipboard.sync_dir",
                "Clipboard",
                "Cross-device sync folder",
                true,
            ),
            key("network.wifi", "Network", "Wi-Fi", false),
            key("network.bluetooth", "Network", "Bluetooth", false),
            key("network.airplane_mode", "Network", "Airplane mode", false),
            key("compositor.layout", "Compositor", "Window layout", true),
            key(
                "compositor.decorations",
                "Compositor",
                "Window decorations (server)",
                true,
            ),
            key(
                "tools.command_palette",
                "Tools",
                "Command Palette (Super+Alt+Space)",
                false,
            ),
            key("tools.fancy_zones", "Tools", "FancyZones layouts", false),
            key("tools.snap_groups", "Tools", "Snap Groups restore", false),
            key(
                "tools.always_on_top_shortcut",
                "Tools",
                "Always on top (Win+Ctrl+T)",
                false,
            ),
            key(
                "tools.always_on_top_indicator",
                "Tools",
                "Always on top title-bar pin",
                false,
            ),
            key(
                "tools.color_picker_history",
                "Tools",
                "Color picker history",
                false,
            ),
            key("tools.find_my_mouse", "Tools", "Find My Mouse (Ctrl×2)", false),
            key(
                "tools.mouse_highlighter",
                "Tools",
                "Mouse click highlighter",
                false,
            ),
            key("tools.awake_minutes", "Tools", "Awake default minutes (0=∞)", false),
            key("tools.jump_list_max", "Tools", "Jump list recent depth", false),
            key(
                "tools.advanced_paste",
                "Tools",
                "Advanced Paste transforms",
                false,
            ),
            key(
                "tools.shortcut_guide",
                "Tools",
                "Shortcut Guide overlay (Super+/)",
                false,
            ),
            key(
                "tools.image_resizer",
                "Tools",
                "Image Resizer",
                false,
            ),
            key(
                "shell.end_task_always",
                "Shell",
                "Always show End task on jump lists",
                false,
            ),
            key(
                "shell.taskbar_multi_mode",
                "Shell",
                "Multi-monitor taskbar (all|main_buttons|main_only)",
                false,
            ),
            key(
                "shell.world_clocks",
                "Shell",
                "World clock timezones in Notification Centre",
                false,
            ),
            key(
                "theme.style_pack",
                "Theme",
                "Active style pack id",
                false,
            ),
            key(
                "theme.style_packs_enabled",
                "Theme",
                "Enable style packs",
                false,
            ),
            key(
                "accessibility.magnifier_native",
                "Accessibility",
                "Compositor-native magnifier",
                false,
            ),
            key(
                "accessibility.magnifier_factor",
                "Accessibility",
                "Magnifier scale factor",
                false,
            ),
            key(
                "accessibility.text_scale",
                "Accessibility",
                "System-wide text scale",
                false,
            ),
            key(
                "accessibility.privacy_indicators",
                "Accessibility",
                "Taskbar privacy indicators",
                false,
            ),
            key(
                "accessibility.auto_osk",
                "Accessibility",
                "Auto on-screen keyboard",
                false,
            ),
            key(
                "accessibility.tablet_mode",
                "Accessibility",
                "Tablet mode (off|auto|on)",
                false,
            ),
            key(
                "storage.sense_enabled",
                "Storage",
                "Storage Sense automatic cleanup",
                false,
            ),
            key(
                "gestures.three_up",
                "Gestures",
                "Three-finger swipe up",
                false,
            ),
        ]
    }

    /// Migrate legacy JSON trees into this document.
    pub fn migrate_legacy(&mut self) -> Result<bool> {
        let mut changed = false;
        let cfg = dirs::config_dir().context("config dir")?;

        // Touch extras first so it does not overlap profile borrows.
        if let Some(v) = read_json(&cfg.join("ws-settings/settings.json")) {
            if let Some(b) = v.get("night_light").and_then(|x| x.as_bool()) {
                self.extras
                    .insert("legacy.night_light".into(), toml::Value::Boolean(b));
                changed = true;
            }
        }

        {
            let profile = self.active_mut();
            if let Some(v) = read_json(&cfg.join("ws-settings/settings.json")) {
                if let Some(s) = v.get("taskbar_align").and_then(|x| x.as_str()) {
                    profile.shell.taskbar_align = s.into();
                    changed = true;
                }
                if let Some(s) = v.get("taskbar_search").and_then(|x| x.as_str()) {
                    profile.shell.taskbar_search = s.into();
                    changed = true;
                }
                if let Some(b) = v.get("taskbar_autohide").and_then(|x| x.as_bool()) {
                    profile.shell.taskbar_autohide = b;
                    changed = true;
                }
                if let Some(s) = v.get("screenshot_dir").and_then(|x| x.as_str()) {
                    if !s.is_empty() {
                        profile.shell.screenshot_dir = s.into();
                        changed = true;
                    }
                }
                if let Some(s) = v.get("main_display").and_then(|x| x.as_str()) {
                    profile.displays.main_display = s.into();
                    changed = true;
                }
            }

            if let Some(v) = read_json(&cfg.join("ws-start/settings.json")) {
                if let Some(s) = v.get("layout").and_then(|x| x.as_str()) {
                    profile.start.layout = s.into();
                    changed = true;
                }
                if let Some(b) = v.get("recently_added").and_then(|x| x.as_bool()) {
                    profile.start.recently_added = b;
                    changed = true;
                }
                if let Some(b) = v.get("most_used").and_then(|x| x.as_bool()) {
                    profile.start.most_used = b;
                    changed = true;
                }
            }

            let pins = cfg.join("ws-taskbar/pins");
            if pins.is_file() {
                if let Ok(text) = fs::read_to_string(&pins) {
                    profile.shell.pins = text
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(str::to_string)
                        .collect();
                    changed = true;
                }
            }

            let start_pins = cfg.join("ws-start/pins");
            if start_pins.is_file() {
                if let Ok(text) = fs::read_to_string(&start_pins) {
                    profile.start.pins = text
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(str::to_string)
                        .collect();
                    changed = true;
                }
            }
        }

        Ok(changed)
    }
}

fn key(path: &str, category: &str, label: &str, advanced: bool) -> ConfigKey {
    ConfigKey {
        path: path.into(),
        category: category.into(),
        label: label.into(),
        advanced,
    }
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_toml() {
        let cfg = MdeConfig::default();
        let text = cfg.export_toml().unwrap();
        let back = MdeConfig::import_toml(&text).unwrap();
        assert_eq!(cfg.schema, back.schema);
        assert_eq!(cfg.active().theme.accent, back.active().theme.accent);
    }

    #[test]
    fn save_load_tmp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = MdeConfig::default();
        cfg.active_mut().theme.accent = "#ff0000".into();
        cfg.save_to(&path).unwrap();
        let loaded = MdeConfig::load_from(&path).unwrap();
        assert_eq!(loaded.active().theme.accent, "#ff0000");
    }

    #[test]
    fn catalog_has_shell_keys() {
        let cat = MdeConfig::catalog();
        assert!(cat.iter().any(|k| k.path == "shell.taskbar_align"));
        assert!(cat.iter().any(|k| k.advanced));
    }

    #[test]
    fn profile_clone() {
        let mut cfg = MdeConfig::default();
        cfg.create_profile("work", "Work").unwrap();
        assert!(cfg.profiles.contains_key("work"));
        assert_eq!(cfg.profiles["work"].display_name, "Work");
    }

    #[test]
    fn shell_defaults_community_quiet() {
        // A001: center remains default; A003: Widgets button off by default.
        let shell = ShellConfig::default();
        assert_eq!(shell.taskbar_align, "center");
        assert!(!shell.taskbar_left);
        assert!(!shell.widgets_btn);
        assert_eq!(shell.taskbar_search, "hide");
    }
}
