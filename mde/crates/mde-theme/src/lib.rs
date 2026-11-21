//! Theme tokens for iced shell/control and the smithay tab strip.
//! Product decision: no GTK/Qt theme bridges.
//!
//! Color, type, space, radius, elevation, material, and motion live in
//! `mde/fidelity/tokens`. The alpha tints below are placeholders until the
//! backdrop-blur spike passes. They are not Mica.

mod color;
pub mod elevation;
pub mod material;
pub mod motion;
pub mod radius;
pub mod space;
pub mod type_ramp;

pub use color::accent_ramp;

use mde_config::{MdeConfig, ThemeConfig};

pub const PRODUCT: &str = "Mackes Desktop Environment (MDE)";
pub const PRODUCT_SHORT: &str = "MDE";
pub const VERSION: &str = "2026";
pub const VERSION_FULL: &str = "2026.0";
/// Short line under the product name on About.
pub const SUBTITLE: &str = "Rust Wayland desktop for Fedora";
/// One-line intent shown on About.
pub const TAGLINE: &str = "Build to match commodity desktop concepts.";
pub const ACCENT_DEFAULT: &str = "#0078d4";
pub const FONT: &str = "Selawik";
pub const FONT_MONO: &str = "Cascadia Mono";
/// Typography line for About / branding surfaces.
pub const FONT_BRANDING: &str = "UI · Selawik · Mono · Cascadia Mono";
/// Desktop stack shown in About software specs.
pub const DESKTOP_STACK: &str = "Wayland · mde-comp (Smithay) · mde-shell (iced)";

#[cfg(feature = "icons")]
pub mod icons;

/// Selawik and Cascadia Mono files shipped with the platform, when they are installed.
pub fn font_bytes() -> Vec<Vec<u8>> {
    font_paths()
        .into_iter()
        .filter_map(|path| std::fs::read(path).ok())
        .collect()
}

fn font_paths() -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut push_unique = |path: std::path::PathBuf| {
        if !path.is_file() {
            return;
        }
        let name = path.file_name().map(|n| n.to_os_string());
        if found.iter().any(|existing: &std::path::PathBuf| {
            existing.file_name().map(|n| n.to_os_string()) == name
        }) {
            return;
        }
        found.push(path);
    };
    let mut roots = vec![std::path::PathBuf::from("/usr/share/fonts")];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(&home).join(".local/share/fonts"));
        roots.push(std::path::PathBuf::from(&home).join(".fonts"));
    }
    for root in roots {
        for rel in [
            "mde/Selawik/selawk.ttf",
            "mde/Selawik/selawkb.ttf",
            "mde/Selawik/selawksb.ttf",
            "Selawik/selawk.ttf",
            "Selawik/selawkb.ttf",
            "Selawik/selawksb.ttf",
            "cascadia-mono-fonts/CascadiaMono-Regular.otf",
        ] {
            push_unique(root.join(rel));
        }
    }
    found
}
pub const PANEL_HEIGHT: i32 = 48;
/// Windows 11–style corner radius for flyouts and Settings cards.
pub const FLYOUT_RADIUS: f32 = 12.0;
/// Smaller radius for taskbar icon hit targets.
pub const CONTROL_RADIUS: f32 = 8.0;
/// Quick Settings toggle tiles.
pub const TILE_RADIUS: f32 = 8.0;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub accent: String,
    pub bg: String,
    pub bg_elevated: String,
    pub fg: String,
    pub fg_muted: String,
    pub border: String,
    pub danger: String,
    pub dark: bool,
    /// `hc-black` / `hc-white`. Solid fills, no blur.
    #[serde(default)]
    pub high_contrast: bool,
    /// Win11 Transparency effects master.
    #[serde(default = "default_true_flag")]
    pub transparency: bool,
    /// Chrome opacity 0.0–1.0 when transparency is on.
    #[serde(default = "default_opacity_f")]
    pub opacity: f32,
    /// A007: raise opacity when windows cover the desktop.
    #[serde(default = "default_true_flag")]
    pub dynamic_transparency: bool,
    /// A007: `off` | `subtle` | `acrylic` | `thick`.
    #[serde(default = "default_blur_preset_pal")]
    pub blur_preset: String,
    #[serde(default = "default_true_flag")]
    pub accent_on_start_taskbar: bool,
    #[serde(default = "default_true_flag")]
    pub accent_on_titlebars: bool,
    /// A050: SSD title-bar corner radius (px).
    #[serde(default)]
    pub ssd_corner_radius: u8,
    /// A050: mica-like translucent SSD when transparency is on.
    #[serde(default = "default_true_flag")]
    pub ssd_mica: bool,
}

fn default_true_flag() -> bool {
    true
}
fn default_opacity_f() -> f32 {
    0.90
}
fn default_blur_preset_pal() -> String {
    "acrylic".into()
}

/// Extra alpha from blur preset (acrylic-style density without compositor blur).
pub fn blur_preset_boost(preset: &str) -> f32 {
    match preset.to_ascii_lowercase().as_str() {
        "off" | "none" => 0.0,
        "subtle" => 0.02,
        "thick" | "strong" => 0.08,
        _ => 0.04, // acrylic (default)
    }
}

impl Palette {
    pub fn from_theme(theme: &ThemeConfig) -> Self {
        // A087: apply active style-pack overlays before resolving colours.
        let themed = mde_config::overlay_theme(theme);
        color::palette_from_theme(&themed)
    }

    pub fn load() -> Self {
        let theme = MdeConfig::load()
            .map(|c| c.active().theme.clone())
            .unwrap_or_default();
        Self::from_theme(&theme)
    }

    pub fn parse_rgb(hex: &str) -> Option<(f32, f32, f32)> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&h[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&h[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&h[4..6], 16).ok()? as f32 / 255.0;
        Some((r, g, b))
    }

    /// Straight RGBA. `None` when `hex` is not `#rrggbb`.
    pub fn channels(hex: &str) -> Option<[f32; 4]> {
        let (r, g, b) = Self::parse_rgb(hex)?;
        Some([r, g, b, 1.0])
    }
}

/// Darken a straight RGBA colour. Alpha is left alone.
pub fn shade(color: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
        color[3],
    ]
}

/// iced theme whose primary colour is the accent and whose surfaces follow the palette.
#[cfg(feature = "iced")]
pub fn iced_theme(palette: &Palette) -> iced::Theme {
    let fallback_bg = if palette.dark {
        iced::Color::from_rgb(0.125, 0.125, 0.125)
    } else {
        iced::Color::from_rgb(0.953, 0.953, 0.953)
    };
    let fallback_fg = if palette.dark {
        iced::Color::WHITE
    } else {
        iced::Color::from_rgb(0.1, 0.1, 0.1)
    };
    iced::Theme::custom(
        "MDE".into(),
        iced::theme::Palette {
            background: color_or(&palette.bg, fallback_bg),
            text: color_or(&palette.fg, fallback_fg),
            primary: color_or(&palette.accent, iced::Color::from_rgb(0.0, 0.47, 0.83)),
            success: iced::Color::from_rgb(0.06, 0.48, 0.32),
            danger: color_or(&palette.danger, iced::Color::from_rgb(0.77, 0.17, 0.11)),
        },
    )
}

/// Effective chrome alpha: solid when transparency is off / HC; else user opacity
/// (Mica-like default 0.90). Material tint_alpha is a ceiling, not a second slider.
#[cfg(feature = "iced")]
pub fn chrome_alpha(palette: &Palette) -> f32 {
    chrome_alpha_dyn(palette, false)
}

/// A007: when `over_content`, dynamic transparency raises opacity toward solid.
#[cfg(feature = "iced")]
pub fn chrome_alpha_dyn(palette: &Palette, over_content: bool) -> f32 {
    if palette.high_contrast || !palette.transparency {
        return 1.0;
    }
    let mut a = palette.opacity.clamp(0.01, 1.0) + blur_preset_boost(&palette.blur_preset);
    if palette.dynamic_transparency && over_content {
        a = (a + 0.08).min(0.98);
    } else if palette.dynamic_transparency && !over_content {
        a = (a - 0.04).max(0.55);
    }
    a.clamp(0.01, 1.0)
}

/// Taskbar tint. Alpha follows Transparency effects + opacity + A007 presets.
#[cfg(feature = "iced")]
pub fn chrome(palette: &Palette) -> iced::Color {
    chrome_over(palette, false)
}

#[cfg(feature = "iced")]
pub fn chrome_over(palette: &Palette, over_content: bool) -> iced::Color {
    let mat = material::taskbar(palette.dark, palette.high_contrast);
    let (r, g, b) = Palette::parse_rgb(&mat.tint).unwrap_or(elevated_rgb(palette));
    iced::Color::from_rgba(r, g, b, chrome_alpha_dyn(palette, over_content))
}

/// Flyout tint. Slightly more opaque than the taskbar so menus stay readable.
#[cfg(feature = "iced")]
pub fn flyout_fill(palette: &Palette) -> iced::Color {
    let mat = material::flyout(palette.dark, palette.high_contrast);
    let (r, g, b) = Palette::parse_rgb(&mat.tint).unwrap_or(elevated_rgb(palette));
    let alpha = if chrome_alpha(palette) >= 1.0 {
        1.0
    } else {
        (chrome_alpha(palette) + 0.04).min(1.0)
    };
    iced::Color::from_rgba(r, g, b, alpha)
}

/// Hairline edge on glass panels.
#[cfg(feature = "iced")]
pub fn flyout_edge(palette: &Palette) -> iced::Color {
    if palette.dark {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.08)
    }
}

/// Soft hover wash used on taskbar icons and Settings nav.
#[cfg(feature = "iced")]
pub fn hover_wash(palette: &Palette) -> iced::Color {
    if palette.dark {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08)
    } else {
        iced::Color::from_rgba(0.0, 0.0, 0.0, 0.06)
    }
}

/// Muted secondary label colour.
#[cfg(feature = "iced")]
pub fn muted(palette: &Palette) -> iced::Color {
    color_or(
        &palette.fg_muted,
        if palette.dark {
            iced::Color::from_rgb(0.77, 0.77, 0.77)
        } else {
            iced::Color::from_rgb(0.36, 0.36, 0.36)
        },
    )
}

/// Accent with soft glow alpha for selected tiles / running indicators.
#[cfg(feature = "iced")]
pub fn accent(palette: &Palette) -> iced::Color {
    color_or(&palette.accent, iced::Color::from_rgb(0.0, 0.47, 0.83))
}

#[cfg(feature = "iced")]
fn elevated_rgb(palette: &Palette) -> (f32, f32, f32) {
    Palette::parse_rgb(&palette.bg_elevated).unwrap_or(if palette.dark {
        (0.17, 0.17, 0.17)
    } else {
        (1.0, 1.0, 1.0)
    })
}

#[cfg(feature = "iced")]
fn color_or(hex: &str, fallback: iced::Color) -> iced::Color {
    match Palette::parse_rgb(hex) {
        Some((r, g, b)) => iced::Color::from_rgb(r, g, b),
        None => fallback,
    }
}

#[allow(dead_code)]
fn theme_is_dark(theme: &ThemeConfig) -> bool {
    matches!(color::resolved_mode(theme).as_str(), "dark" | "hc-black")
}

pub(crate) fn schedule_is_dark(theme: &ThemeConfig) -> bool {
    if !theme.dark_schedule {
        return false;
    }
    let now = chrono::Local::now();
    let minutes = chrono::Timelike::num_seconds_from_midnight(&now) / 60;
    if theme
        .dark_schedule_mode
        .eq_ignore_ascii_case("sunrise_sunset")
    {
        return approx_solar_dark(minutes as u16, chrono::Datelike::month(&now));
    }
    let Some(from) = parse_hhmm(&theme.dark_from) else {
        return false;
    };
    let Some(until) = parse_hhmm(&theme.dark_until) else {
        return false;
    };
    minutes_in_window(minutes as u16, from, until)
}

/// A051: coarse civil-twilight window (approximate local sunrise/sunset by month).
fn approx_solar_dark(minutes: u16, month: u32) -> bool {
    let (sunrise, sunset) = match month {
        12 | 1 | 2 => (7 * 60, 17 * 60),       // winter
        6 | 7 | 8 => (6 * 60, 20 * 60),       // summer
        _ => (6 * 60 + 30, 19 * 60),          // spring/fall
    };
    minutes < sunrise || minutes >= sunset
}

fn parse_hhmm(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// True when `now` (minutes from local midnight) sits in `[from, until)`.
/// If `from` is later than `until`, the window wraps past midnight.
pub fn minutes_in_window(now: u16, from: u16, until: u16) -> bool {
    if from == until {
        return false;
    }
    if from < until {
        now >= from && now < until
    } else {
        now >= from || now < until
    }
}

pub fn about_line() -> String {
    format!("{PRODUCT} {VERSION_FULL}")
}

/// Rasterized Mackes mark for iced (`mackes.svg` → PNG cache).
pub fn brand_mark(px: u32) -> Option<String> {
    #[cfg(feature = "icons")]
    {
        for path in brand_mark_paths() {
            if path.is_file() {
                if let Some(png) = icons::icon_file(&path.display().to_string(), px, false) {
                    return Some(png);
                }
            }
        }
        None
    }
    #[cfg(not(feature = "icons"))]
    {
        let _ = px;
        None
    }
}

fn brand_mark_paths() -> Vec<std::path::PathBuf> {
    let mut paths = vec![
        std::path::PathBuf::from("/usr/share/mde/brand/mackes.svg"),
        std::path::PathBuf::from("/usr/local/share/mde/brand/mackes.svg"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        paths.push(std::path::PathBuf::from(home).join(".local/share/mde/brand/mackes.svg"));
    }
    // Repo checkout: mde/assets/brand next to crates/
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("assets/brand/mackes.svg"));
        paths.push(cwd.join("../assets/brand/mackes.svg"));
        paths.push(cwd.join("../../assets/brand/mackes.svg"));
    }
    paths
}

/// Curated open-source licenses shipping with or underpinning MDE.
pub fn licenses_at_play() -> Vec<(&'static str, &'static str)> {
    vec![
        ("MDE platform (mde-comp, mde-shell, mde-control, …)", "MIT"),
        ("smithay-drm-extras", "MIT"),
        ("Mackes-Carbon / Mackes-Carbon-Light icons", "Apache-2.0"),
        ("Selawik UI font", "SIL OFL 1.1"),
        ("Cascadia Mono", "SIL OFL 1.1"),
        ("iced, Smithay, and other Rust crates", "per-crate (see Cargo)"),
        ("Base OS packages", "Fedora project licenses"),
    ]
}

/// Frozen clock when `MDE_FIDELITY=1` and `MDE_FIDELITY_NOW` parses.
pub fn now() -> chrono::DateTime<chrono::Local> {
    if fidelity_active() {
        if let Ok(text) = std::env::var("MDE_FIDELITY_NOW") {
            if let Ok(fixed) = chrono::DateTime::parse_from_rfc3339(&text) {
                return fixed.with_timezone(&chrono::Local);
            }
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&text, "%Y-%m-%dT%H:%M:%S") {
                return naive.and_local_timezone(chrono::Local).single().unwrap_or_else(chrono::Local::now);
            }
        }
    }
    chrono::Local::now()
}

pub fn fidelity_active() -> bool {
    std::env::var("MDE_FIDELITY").ok().as_deref() == Some("1")
}

/// Reference scoring must not run on unmeasured token files.
pub fn enforce_measured() -> Result<(), String> {
    if !fidelity_active() {
        return Ok(());
    }
    for file in [
        "color.toml",
        "type.toml",
        "space.toml",
        "radius.toml",
        "elevation.toml",
        "material.toml",
        "motion.toml",
        "accent.toml",
    ] {
        let status = color::token_status(file);
        if status != "measured" {
            return Err(format!("{file} status is {status}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_palette() {
        let p = Palette::from_theme(&ThemeConfig::default());
        assert!(!p.dark);
        assert_eq!(p.fg, "#1a1a1a");
    }

    #[test]
    fn overnight_dark_window() {
        assert!(minutes_in_window(21 * 60, 20 * 60, 7 * 60));
        assert!(minutes_in_window(6 * 60, 20 * 60, 7 * 60));
        assert!(!minutes_in_window(12 * 60, 20 * 60, 7 * 60));
        assert!(!minutes_in_window(12 * 60, 8 * 60, 8 * 60));
    }

    #[test]
    fn parse_accent() {
        let (r, g, b) = Palette::parse_rgb("#0078d4").unwrap();
        assert!(r < 0.1 && g > 0.4 && b > 0.8);
        let px = Palette::channels("#0078d4").unwrap();
        assert!((px[2] - b).abs() < 0.001);
    }

    #[test]
    fn dark_palette_keeps_accent() {
        let mut theme = ThemeConfig::default();
        theme.mode = "dark".into();
        theme.accent = "#8764b8".into();
        let p = Palette::from_theme(&theme);
        assert!(p.dark);
        assert!(theme_is_dark(&theme));
        assert!(!p.high_contrast);
        assert_eq!(p.accent, "#8764b8");
        assert_eq!(p.bg, "#202020");
        assert_eq!(p.bg_elevated, "#2c2c2c");
    }

    #[test]
    fn high_contrast_palettes_differ() {
        let mut theme = ThemeConfig::default();
        theme.mode = "hc-black".into();
        let black = Palette::from_theme(&theme);
        theme.mode = "hc-white".into();
        let white = Palette::from_theme(&theme);
        assert!(black.high_contrast && black.dark);
        assert!(white.high_contrast && !white.dark);
        assert_ne!(black.bg, white.bg);
        assert_ne!(black.fg, white.fg);
        assert_eq!(black.bg, "#000000");
        assert_eq!(white.fg, "#000000");
    }

    #[test]
    fn accent_round_trip_and_ramp() {
        let ramp = color::accent_ramp("#0078d4");
        assert_eq!(ramp.base, "#0078d4");
        assert_ne!(ramp.secondary, ramp.base);
        assert_ne!(ramp.tertiary, ramp.secondary);
        let px = Palette::channels(&ramp.base).unwrap();
        assert!(px[2] > 0.8);
    }

    #[test]
    fn unmeasured_tokens_fail_fidelity_enforcement() {
        let prev = std::env::var("MDE_FIDELITY").ok();
        std::env::set_var("MDE_FIDELITY", "1");
        let err = enforce_measured().unwrap_err();
        assert!(err.contains("unmeasured") || err.contains("missing"), "{err}");
        match prev {
            Some(v) => std::env::set_var("MDE_FIDELITY", v),
            None => std::env::remove_var("MDE_FIDELITY"),
        }
    }

    #[test]
    fn animations_off_is_zero_duration() {
        let motion = motion::action("start-open", false);
        assert_eq!(motion.duration_ms, 0);
    }
}
