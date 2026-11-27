//! Font catalog + selective iced font loading.
//!
//! Loads the currently selected UI and Mono families in full, plus one Regular
//! preview face for every catalog entry (Settings list previews).

use std::path::PathBuf;

use mde_config::ThemeConfig;
use serde::Deserialize;

use crate::{FONT, FONT_MONO};

const EMBEDDED_CATALOG: &str = include_str!("../../../assets/fonts/CATALOG.toml");

#[derive(Debug, Clone, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    ui: Vec<CatalogEntry>,
    #[serde(default)]
    mono: Vec<CatalogEntry>,
    #[serde(default)]
    nerd: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogEntry {
    family: String,
    dir: String,
    preview: String,
    #[serde(default)]
    #[allow(dead_code)]
    default: bool,
}

/// One choosable typeface in Personalization → Fonts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontChoice {
    pub family: String,
    pub dir: String,
    pub preview: String,
    pub kind: FontKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Ui,
    Mono,
    Nerd,
}

fn load_catalog() -> CatalogFile {
    for path in catalog_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = toml::from_str::<CatalogFile>(&text) {
                return parsed;
            }
        }
    }
    toml::from_str(EMBEDDED_CATALOG).unwrap_or_else(|_| CatalogFile {
        ui: Vec::new(),
        mono: Vec::new(),
        nerd: Vec::new(),
    })
}

fn catalog_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/share/fonts/mde/CATALOG.toml"),
        PathBuf::from("/usr/local/share/fonts/mde/CATALOG.toml"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".local/share/fonts/mde/CATALOG.toml"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("assets/fonts/CATALOG.toml"));
        paths.push(cwd.join("../assets/fonts/CATALOG.toml"));
        paths.push(cwd.join("../../assets/fonts/CATALOG.toml"));
    }
    // Crate-relative: mde/crates/mde-theme → mde/assets/fonts
    paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/CATALOG.toml"));
    paths
}

fn font_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts/mde"),
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts/mde"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".local/share/fonts/mde"));
        roots.push(home.join(".local/share/fonts"));
        roots.push(home.join(".fonts"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("assets/fonts"));
        roots.push(cwd.join("../assets/fonts"));
        roots.push(cwd.join("../../assets/fonts"));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts"));
    roots
}

/// Resolve UI family from theme config (falls back to Inter).
pub fn ui_font(theme: &ThemeConfig) -> &str {
    let name = theme.ui_font.trim();
    if name.is_empty() {
        FONT
    } else {
        name
    }
}

/// Resolve mono family from theme config (falls back to Cascadia Mono).
pub fn mono_font(theme: &ThemeConfig) -> &str {
    let name = theme.mono_font.trim();
    if name.is_empty() {
        FONT_MONO
    } else {
        name
    }
}

/// UI catalog (20 Google + Selawik).
pub fn ui_font_choices() -> Vec<FontChoice> {
    load_catalog()
        .ui
        .into_iter()
        .map(|e| FontChoice {
            family: e.family,
            dir: e.dir,
            preview: e.preview,
            kind: FontKind::Ui,
        })
        .collect()
}

/// Mono catalog: Google/static monos + curated Nerd subset.
pub fn mono_font_choices() -> Vec<FontChoice> {
    let cat = load_catalog();
    let mut out: Vec<FontChoice> = cat
        .mono
        .into_iter()
        .map(|e| FontChoice {
            family: e.family,
            dir: e.dir,
            preview: e.preview,
            kind: FontKind::Mono,
        })
        .collect();
    out.extend(cat.nerd.into_iter().map(|e| FontChoice {
        family: e.family,
        dir: e.dir,
        preview: e.preview,
        kind: FontKind::Nerd,
    }));
    out
}

fn find_entry(family: &str) -> Option<CatalogEntry> {
    let cat = load_catalog();
    cat.ui
        .into_iter()
        .chain(cat.mono)
        .chain(cat.nerd)
        .find(|e| e.family.eq_ignore_ascii_case(family))
}

fn push_unique(found: &mut Vec<PathBuf>, path: PathBuf) {
    if !path.is_file() {
        return;
    }
    let name = path.file_name().map(|n| n.to_os_string());
    if found.iter().any(|existing| existing.file_name().map(|n| n.to_os_string()) == name) {
        return;
    }
    found.push(path);
}

fn collect_family_files(dir_name: &str, preview_only: bool, preview_name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in font_roots() {
        let dir = root.join(dir_name);
        if !dir.is_dir() {
            // Also try flat under /usr/share/fonts without mde/
            continue;
        }
        if preview_only {
            push_unique(&mut found, dir.join(preview_name));
            // Fallback: any Regular face
            if found.is_empty() {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for entry in rd.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let lower = name.to_ascii_lowercase();
                        if (lower.contains("regular") || lower == "selawk.ttf")
                            && (lower.ends_with(".ttf") || lower.ends_with(".otf"))
                            && !lower.contains("nerd font mono")
                            && !lower.contains("nerdfontmono")
                            && !lower.contains("nerdfontpropo")
                        {
                            push_unique(&mut found, entry.path());
                            break;
                        }
                    }
                }
            }
        } else if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                if !ext.eq_ignore_ascii_case("ttf") && !ext.eq_ignore_ascii_case("otf") {
                    continue;
                }
                push_unique(&mut found, path);
            }
        }
        if !found.is_empty() {
            break;
        }
    }
    found
}

fn paths_for_selected(ui: &str, mono: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let cat = load_catalog();
    let all: Vec<CatalogEntry> = cat
        .ui
        .into_iter()
        .chain(cat.mono)
        .chain(cat.nerd)
        .collect();

    for entry in &all {
        let selected = entry.family.eq_ignore_ascii_case(ui)
            || entry.family.eq_ignore_ascii_case(mono);
        let files = collect_family_files(&entry.dir, !selected, &entry.preview);
        for f in files {
            push_unique(&mut found, f);
        }
    }

    // Legacy fallbacks if catalog dirs are missing on a partial install.
    if !all.iter().any(|e| e.family.eq_ignore_ascii_case(ui)) {
        for rel in [
            "mde/Inter/Inter-Regular.ttf",
            "Inter/Inter-Regular.ttf",
            "mde/Selawik/selawk.ttf",
            "Selawik/selawk.ttf",
        ] {
            for root in [PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/share/fonts/mde")] {
                push_unique(&mut found, root.join(rel));
            }
        }
    }
    if !all.iter().any(|e| e.family.eq_ignore_ascii_case(mono)) {
        for rel in [
            "mde/CascadiaMono/CascadiaMono-Regular.ttf",
            "CascadiaMono/CascadiaMono-Regular.ttf",
            "cascadia-mono-fonts/CascadiaMono-Regular.otf",
        ] {
            for root in [PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/share/fonts/mde")] {
                push_unique(&mut found, root.join(rel));
            }
        }
    }

    let _ = find_entry(ui);
    found
}

/// Load selected UI+Mono full sets plus one Regular preview per catalog family.
pub fn font_bytes_selected(ui: &str, mono: &str) -> Vec<Vec<u8>> {
    paths_for_selected(ui, mono)
        .into_iter()
        .filter_map(|path| std::fs::read(path).ok())
        .collect()
}

/// Default load using stock Inter + Cascadia Mono (plus catalog previews).
pub fn font_bytes() -> Vec<Vec<u8>> {
    font_bytes_selected(FONT, FONT_MONO)
}

/// Branding line reflecting the active theme fonts.
pub fn font_branding_for(theme: &ThemeConfig) -> String {
    format!("UI · {} · Mono · {}", ui_font(theme), mono_font(theme))
}

/// Whether `path` exists under any font root for the given catalog dir.
pub fn family_installed(dir: &str) -> bool {
    font_roots().iter().any(|root| {
        let d = root.join(dir);
        d.is_dir()
            && std::fs::read_dir(&d)
                .ok()
                .map(|rd| {
                    rd.flatten().any(|e| {
                        e.path()
                            .extension()
                            .and_then(|x| x.to_str())
                            .map(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
    })
}

/// Absolute path to a preview face when present.
pub fn preview_path(dir: &str, preview: &str) -> Option<PathBuf> {
    for root in font_roots() {
        let p = root.join(dir).join(preview);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_parses() {
        let cat = load_catalog();
        assert!(cat.ui.iter().any(|e| e.family == "Inter"));
        assert!(cat.mono.iter().any(|e| e.family == "Cascadia Mono"));
        assert!(cat.nerd.iter().any(|e| e.family.contains("Nerd")));
    }

    #[test]
    fn defaults_resolve() {
        let t = ThemeConfig::default();
        assert_eq!(ui_font(&t), "Inter");
        assert_eq!(mono_font(&t), "Cascadia Mono");
    }
}
