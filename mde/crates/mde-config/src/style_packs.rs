//! A087 — official style-pack API.
//!
//! Packs live at `~/.local/share/mde/styles/<id>.toml` (or `…/styles/<id>/pack.toml`).
//! Safe overlays only: accent, opacity, blur, chrome radii, and optional CSS-like
//! notes for documentation — no arbitrary code execution.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ThemeConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StylePack {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub opacity: Option<u8>,
    #[serde(default)]
    pub blur_preset: Option<String>,
    #[serde(default)]
    pub transparency: Option<bool>,
    #[serde(default)]
    pub ssd_corner_radius: Option<u8>,
    #[serde(default)]
    pub ssd_mica: Option<bool>,
    /// Optional documentation / QSS-like notes (ignored at runtime).
    #[serde(default)]
    pub notes: String,
}

impl StylePack {
    pub fn apply_to(&self, theme: &mut ThemeConfig) {
        if let Some(a) = &self.accent {
            if !a.trim().is_empty() {
                theme.accent = a.clone();
            }
        }
        if let Some(o) = self.opacity {
            theme.opacity = o.clamp(1, 100);
        }
        if let Some(b) = &self.blur_preset {
            theme.blur_preset = b.clone();
        }
        if let Some(t) = self.transparency {
            theme.transparency = t;
        }
        if let Some(r) = self.ssd_corner_radius {
            theme.ssd_corner_radius = r;
        }
        if let Some(m) = self.ssd_mica {
            theme.ssd_mica = m;
        }
    }
}

pub fn styles_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mde")
        .join("styles")
}

pub fn list_packs() -> Vec<StylePack> {
    let root = styles_root();
    let mut packs = Vec::new();
    let Ok(rd) = fs::read_dir(&root) else {
        return packs;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            let pack_path = path.join("pack.toml");
            if let Some(mut p) = load_pack(&pack_path) {
                if p.id.is_empty() {
                    p.id = ent.file_name().to_string_lossy().into();
                }
                packs.push(p);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Some(mut p) = load_pack(&path) {
                if p.id.is_empty() {
                    p.id = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                }
                packs.push(p);
            }
        }
    }
    packs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    packs
}

pub fn load_pack(path: &Path) -> Option<StylePack> {
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

pub fn load_named(id: &str) -> Option<StylePack> {
    if id.trim().is_empty() {
        return None;
    }
    let root = styles_root();
    let candidates = [
        root.join(format!("{id}.toml")),
        root.join(id).join("pack.toml"),
    ];
    for c in candidates {
        if let Some(mut p) = load_pack(&c) {
            if p.id.is_empty() {
                p.id = id.into();
            }
            return Some(p);
        }
    }
    list_packs().into_iter().find(|p| p.id == id || p.name == id)
}

/// Apply the active pack onto a theme copy (stock theme fields win when pack missing).
pub fn overlay_theme(theme: &ThemeConfig) -> ThemeConfig {
    let mut out = theme.clone();
    if !theme.style_packs_enabled {
        return out;
    }
    if let Some(pack) = load_named(&theme.style_pack) {
        pack.apply_to(&mut out);
    }
    out
}

/// Ensure a sample pack exists for discoverability.
pub fn ensure_sample_pack() -> std::io::Result<PathBuf> {
    let root = styles_root();
    fs::create_dir_all(&root)?;
    let path = root.join("acrylic-blue.toml");
    if !path.exists() {
        let sample = r##"id = "acrylic-blue"
name = "Acrylic Blue"
description = "Sample MDE style pack — accent + acrylic chrome"
accent = "#0078d4"
opacity = 85
blur_preset = "acrylic"
transparency = true
ssd_corner_radius = 8
notes = "Drop more packs in ~/.local/share/mde/styles/"
"##;
        fs::write(&path, sample)?;
    }
    Ok(path)
}
