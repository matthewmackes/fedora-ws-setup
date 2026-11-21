//! Color tokens. Semantic colors for light and dark match the existing product
//! palette so current screenshots do not move. High-contrast roles are structural
//! (solid black or white) until a measured spec replaces them.
//! `status = "unmeasured"` means these are not Win11 reference values.

use crate::Palette;
use mde_config::ThemeConfig;
use std::path::PathBuf;

#[derive(Clone, Debug)]
struct ThemeColors {
    bg: String,
    bg_elevated: String,
    fg: String,
    fg_muted: String,
    border: String,
    danger: String,
    dark: bool,
    high_contrast: bool,
}

pub fn palette_from_theme(theme: &ThemeConfig) -> Palette {
    let mode = resolved_mode(theme);
    let colors = colors_for(&mode);
    let opacity = if theme.transparency {
        (theme.opacity.clamp(1, 100) as f32) / 100.0
    } else {
        1.0
    };
    Palette {
        accent: theme.accent.clone(),
        bg: colors.bg,
        bg_elevated: colors.bg_elevated,
        fg: colors.fg,
        fg_muted: colors.fg_muted,
        border: colors.border,
        danger: colors.danger,
        dark: colors.dark,
        high_contrast: colors.high_contrast,
        transparency: theme.transparency && !colors.high_contrast,
        opacity,
        dynamic_transparency: theme.dynamic_transparency,
        blur_preset: theme.blur_preset.clone(),
        accent_on_start_taskbar: theme.accent_on_start_taskbar,
        accent_on_titlebars: theme.accent_on_titlebars,
        ssd_corner_radius: theme.ssd_corner_radius,
        ssd_mica: theme.ssd_mica,
    }
}

pub fn resolved_mode(theme: &ThemeConfig) -> String {
    if let Some(forced) = fidelity_theme() {
        return forced;
    }
    let mode = theme.mode.to_ascii_lowercase();
    match mode.as_str() {
        "dark" | "hc-black" | "hc-white" => mode,
        _ => {
            if theme.dark_schedule && crate::schedule_is_dark(theme) {
                "dark".into()
            } else {
                "light".into()
            }
        }
    }
}

pub fn fidelity_theme() -> Option<String> {
    if std::env::var("MDE_FIDELITY").ok().as_deref() != Some("1") {
        return None;
    }
    std::env::var("MDE_FIDELITY_THEME")
        .ok()
        .map(|v| v.to_ascii_lowercase())
        .filter(|v| matches!(v.as_str(), "light" | "dark" | "hc-black" | "hc-white"))
}

fn colors_for(mode: &str) -> ThemeColors {
    if let Some(loaded) = load_mode(mode) {
        return loaded;
    }
    fallback(mode)
}

fn fallback(mode: &str) -> ThemeColors {
    match mode {
        "dark" => ThemeColors {
            bg: "#202020".into(),
            bg_elevated: "#2c2c2c".into(),
            fg: "#ffffff".into(),
            fg_muted: "#c5c5c5".into(),
            border: "#3f3f3f".into(),
            danger: "#ff99a4".into(),
            dark: true,
            high_contrast: false,
        },
        "hc-black" => ThemeColors {
            bg: "#000000".into(),
            bg_elevated: "#000000".into(),
            fg: "#ffffff".into(),
            fg_muted: "#ffffff".into(),
            border: "#ffffff".into(),
            danger: "#ff5555".into(),
            dark: true,
            high_contrast: true,
        },
        "hc-white" => ThemeColors {
            bg: "#ffffff".into(),
            bg_elevated: "#ffffff".into(),
            fg: "#000000".into(),
            fg_muted: "#000000".into(),
            border: "#000000".into(),
            danger: "#c42b1c".into(),
            dark: false,
            high_contrast: true,
        },
        _ => ThemeColors {
            bg: "#f3f3f3".into(),
            bg_elevated: "#f9f9f9".into(),
            fg: "#1a1a1a".into(),
            fg_muted: "#5d5d5d".into(),
            border: "#e5e5e5".into(),
            danger: "#c42b1c".into(),
            dark: false,
            high_contrast: false,
        },
    }
}

fn load_mode(mode: &str) -> Option<ThemeColors> {
    let path = token_path("color.toml");
    let text = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let table = value.get(mode)?.as_table()?;
    let get = |key: &str| table.get(key).and_then(|v| v.as_str()).map(|s| s.to_string());
    Some(ThemeColors {
        bg: get("fill_solid")?,
        bg_elevated: get("fill_subtle")?,
        fg: get("text_primary")?,
        fg_muted: get("text_secondary")?,
        border: get("stroke_default")?,
        danger: get("danger")?,
        dark: mode == "dark" || mode == "hc-black",
        high_contrast: mode.starts_with("hc-"),
    })
}

pub fn token_path(name: &str) -> PathBuf {
    token_root().join(name)
}

pub fn token_root() -> PathBuf {
    if let Ok(root) = std::env::var("MDE_FIDELITY_ROOT") {
        return PathBuf::from(root).join("tokens");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fidelity/tokens")
}

pub fn token_status(file: &str) -> String {
    let Ok(text) = std::fs::read_to_string(token_path(file)) else {
        return "missing".into();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return "invalid".into();
    };
    value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unmeasured")
        .to_string()
}

/// HSL deltas from `accent.toml`. The base hex is returned unchanged.
#[derive(Clone, Debug)]
pub struct AccentRamp {
    pub base: String,
    pub secondary: String,
    pub tertiary: String,
}

pub fn accent_ramp(hex: &str) -> AccentRamp {
    let (dl_secondary, dl_tertiary) = accent_deltas();
    let base = normalize_hex(hex).unwrap_or_else(|| "#0078d4".into());
    let secondary = shift_lightness(&base, dl_secondary).unwrap_or_else(|| base.clone());
    let tertiary = shift_lightness(&base, dl_tertiary).unwrap_or_else(|| base.clone());
    AccentRamp {
        base,
        secondary,
        tertiary,
    }
}

fn accent_deltas() -> (f32, f32) {
    let Ok(text) = std::fs::read_to_string(token_path("accent.toml")) else {
        return (0.12, 0.24);
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return (0.12, 0.24);
    };
    let deltas = value.get("deltas");
    let read = |key: &str, fallback: f32| {
        deltas
            .and_then(|d| d.get(key))
            .and_then(|v| v.as_float())
            .map(|v| v as f32)
            .unwrap_or(fallback)
    };
    (read("secondary_l", 0.12), read("tertiary_l", 0.24))
}

fn normalize_hex(hex: &str) -> Option<String> {
    let (r, g, b) = Palette::parse_rgb(hex)?;
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    ))
}

fn shift_lightness(hex: &str, delta: f32) -> Option<String> {
    let (r, g, b) = Palette::parse_rgb(hex)?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let (r, g, b) = hsl_to_rgb(h, s, (l + delta).clamp(0.0, 1.0));
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    ))
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min) < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s < 1e-6 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    (
        hue(p, q, h + 1.0 / 3.0),
        hue(p, q, h),
        hue(p, q, h - 1.0 / 3.0),
    )
}

fn hue(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}
