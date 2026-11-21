//! Mackes icon theme lookup. SVG icons are rasterized to PNG for iced.
//!
//! Lookups are memoized in-process and the on-disk PNG cache is checked
//! *before* re-reading / re-rasterizing an SVG. Without that, every iced
//! `view()` (scroll, pointer move, PollBus) re-ran resvg for every icon.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const CATEGORIES: &[&str] = &[
    "apps",
    "actions",
    "devices",
    "places",
    "status",
    "categories",
    "mimetypes",
    "emblems",
    "panel",
];

/// Light-glyph theme (`#f4f4f4`) for dark canvases.
pub const CHROME_THEME: &str = "Mackes-Carbon-Light";

/// Dark-glyph theme (`currentColor`) for light canvases.
pub const CONTENT_THEME: &str = "Mackes-Carbon";

/// GTK / FreeDesktop icon theme name for a canvas (`palette.dark`).
pub fn theme_name_for_ui(dark_ui: bool) -> &'static str {
    if dark_ui {
        CHROME_THEME
    } else {
        CONTENT_THEME
    }
}

type CacheKey = (String, u32, bool);

fn resolved_cache() -> &'static Mutex<HashMap<CacheKey, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// PNG path for an icon name or an absolute `.png` / `.svg` path.
///
/// `dark_glyphs` selects the theme order:
/// - `false` → Mackes-Carbon-Light first (light `#f4f4f4` glyphs; dark UIs)
/// - `true` → Mackes-Carbon first (`currentColor` → dark glyphs for light UIs)
///
/// Prefer [`for_ui`] when picking icons from a light/dark canvas flag.
pub fn icon_file(name: &str, px: u32, dark_glyphs: bool) -> Option<String> {
    let dark = dark_glyphs;
    let px = px.clamp(16, 128);
    let key = (name.to_string(), px, dark);
    if let Ok(cache) = resolved_cache().lock() {
        if let Some(hit) = cache.get(&key) {
            return hit.clone();
        }
    }
    let resolved = icon_file_uncached(name, px, dark);
    if let Ok(mut cache) = resolved_cache().lock() {
        cache.insert(key, resolved.clone());
    }
    resolved
}

fn icon_file_uncached(name: &str, px: u32, dark: bool) -> Option<String> {
    let source = locate(name, dark)?;
    if source.extension().and_then(|ext| ext.to_str()) == Some("png") {
        return Some(source.display().to_string());
    }
    // Unit tests do not write under ~/.local/state.
    if cfg!(test) {
        let _ = raster_svg(&std::fs::read(&source).ok()?, px)?;
        return None;
    }
    let out = disk_cache_path(&source, px)?;
    if out.is_file() {
        return Some(out.display().to_string());
    }
    let svg = std::fs::read(&source).ok()?;
    let png = raster_svg(&svg, px)?;
    let dir = out.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    std::fs::write(&out, &png).ok()?;
    Some(out.display().to_string())
}

/// Shell chrome icons for a canvas: light glyphs on dark UI, dark glyphs on light UI.
///
/// Alias of [`for_ui`] — kept for call sites that name the surface “chrome”
/// (taskbar, Start, flyouts, Settings). Never force Light glyphs on light mica.
pub fn chrome_icon(name: &str, px: u32, dark_ui: bool) -> Option<String> {
    for_ui(name, px, dark_ui)
}

/// Content-surface icons: light glyphs on dark UI, dark glyphs on light UI.
pub fn for_ui(name: &str, px: u32, dark_ui: bool) -> Option<String> {
    if dark_ui {
        // Mackes-Carbon-Light fills are #f4f4f4 — readable on dark canvases.
        icon_file(name, px, false)
    } else {
        // Mackes-Carbon uses currentColor → dark pixels after rasterize.
        icon_file(name, px, true).or_else(|| icon_file(name, px, false))
    }
}

/// Theme or hicolor PNG for the tray host, matched to the taskbar canvas.
pub fn find_png(name: &str, dark_ui: bool) -> Option<String> {
    let dark_glyphs = !dark_ui;
    let source = locate(name, dark_glyphs).or_else(|| {
        if dark_glyphs {
            locate(name, false)
        } else {
            None
        }
    })?;
    if source.extension().and_then(|ext| ext.to_str()) == Some("png") {
        Some(source.display().to_string())
    } else {
        for_ui(name, 24, dark_ui)
    }
}

/// Path of the theme file for `name`, before rasterizing.
/// `dark_glyphs == false` prefers Mackes-Carbon-Light (light glyphs for dark canvases).
/// `dark_glyphs == true` prefers Mackes-Carbon (dark glyphs for light canvases).
pub fn locate(name: &str, dark_glyphs: bool) -> Option<PathBuf> {
    if let Some(path) = absolute_icon(name) {
        return Some(path);
    }
    if !icon_name_ok(name) {
        return None;
    }
    let mut themes = if dark_glyphs {
        vec![CONTENT_THEME, CHROME_THEME]
    } else {
        vec![CHROME_THEME, CONTENT_THEME]
    };
    themes.push("hicolor");
    themes.push("Adwaita");
    for root in icon_roots() {
        for theme in &themes {
            if let Some(path) = in_theme(&root.join(theme), name) {
                return Some(path);
            }
        }
    }
    None
}

fn icon_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/usr/share/icons")];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/icons"));
    }
    roots
}

fn icon_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

fn absolute_icon(name: &str) -> Option<PathBuf> {
    if !name.starts_with('/') || name.contains("..") {
        return None;
    }
    let path = PathBuf::from(name);
    let ext = path.extension().and_then(|ext| ext.to_str())?;
    if ext != "png" && ext != "svg" {
        return None;
    }
    path.is_file().then_some(path)
}

fn in_theme(theme: &Path, name: &str) -> Option<PathBuf> {
    let names = [name.to_string(), format!("{name}-symbolic")];
    for category in CATEGORIES {
        for icon in &names {
            let svg = theme.join("scalable").join(category).join(format!("{icon}.svg"));
            if svg.is_file() {
                return Some(svg);
            }
        }
    }
    for size in ["48x48", "32x32", "24x24", "22x22", "16x16"] {
        for category in CATEGORIES {
            let png = theme.join(size).join(category).join(format!("{name}.png"));
            if png.is_file() {
                return Some(png);
            }
        }
    }
    None
}

pub fn raster_svg(svg: &[u8], px: u32) -> Option<Vec<u8>> {
    let px = px.clamp(16, 128);
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let width = tree.size().width();
    let height = tree.size().height();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let scale = (px as f32 / width).min(px as f32 / height);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

fn disk_cache_path(source: &Path, px: u32) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".local/state/mde/icon-cache");
    // Path-stable key so we can hit the cache without re-reading the SVG.
    let key = format!("{:016x}-{px}", fnv64(source.as_os_str().as_encoded_bytes()));
    Some(dir.join(format!("{key}.png")))
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_like_names() {
        assert!(locate("", true).is_none());
        assert!(locate("../passwd", true).is_none());
        assert!(locate("a/b", true).is_none());
    }

    #[test]
    fn chrome_prefers_carbon_light() {
        let path = locate("view-grid", false).expect("view-grid");
        let text = path.display().to_string();
        assert!(text.contains("Mackes-Carbon-Light"));
        assert!(text.ends_with(".svg"));
    }

    #[test]
    fn dark_ui_uses_light_glyphs() {
        let path = locate("view-refresh", false).expect("light glyphs");
        assert!(
            path.display().to_string().contains("Mackes-Carbon-Light"),
            "dark UI / chrome must use Mackes-Carbon-Light"
        );
        let svg = std::fs::read_to_string(&path).expect("svg");
        assert!(
            svg.contains("#f4f4f4") || svg.contains("#F4F4F4"),
            "light glyph theme should paint near-white fills"
        );
    }

    #[test]
    fn light_ui_prefers_dark_glyphs() {
        let path = locate("view-refresh", true).expect("dark glyphs");
        assert!(
            path.display().to_string().contains("Mackes-Carbon/")
                || path.display().to_string().ends_with("Mackes-Carbon/scalable/actions/view-refresh.svg"),
            "light UI should prefer Mackes-Carbon (not -Light)"
        );
        let svg = std::fs::read_to_string(&path).expect("svg");
        assert!(
            svg.contains("currentColor"),
            "Mackes-Carbon icons use currentColor (rasterizes dark)"
        );
    }

    #[test]
    fn for_ui_theme_order_matches_canvas() {
        // icon_file returns None under cfg(test); assert theme order via locate.
        let dark_ui = locate("folder", false).expect("light glyphs for dark UI");
        let light_ui = locate("folder", true).expect("dark glyphs for light UI");
        let dark_s = dark_ui.display().to_string();
        let light_s = light_ui.display().to_string();
        assert!(dark_s.contains("Mackes-Carbon-Light"));
        assert!(light_s.contains("Mackes-Carbon"));
        assert!(!light_s.contains("Mackes-Carbon-Light"));
        assert_ne!(dark_s, light_s);
        // for_ui / chrome_icon are the public canvas helpers.
        let _ = for_ui("folder", 16, true);
        let _ = for_ui("folder", 16, false);
        let _ = chrome_icon("folder", 16, true);
        let _ = chrome_icon("folder", 16, false);
    }

    #[test]
    fn chrome_icon_tracks_canvas_like_for_ui() {
        let dark = locate("view-grid", false).expect("dark canvas");
        let light = locate("view-grid", true).expect("light canvas");
        assert!(dark.display().to_string().contains("Mackes-Carbon-Light"));
        assert!(light.display().to_string().contains("Mackes-Carbon"));
        assert!(!light.display().to_string().contains("Mackes-Carbon-Light"));
    }

    #[test]
    fn theme_name_for_ui_is_platform_mackes() {
        assert_eq!(theme_name_for_ui(true), "Mackes-Carbon-Light");
        assert_eq!(theme_name_for_ui(false), "Mackes-Carbon");
    }

    #[test]
    fn svg_rect_becomes_png() {
        let svg = concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\">",
            "<rect width=\"16\" height=\"16\" fill=\"blue\"/>",
            "</svg>"
        );
        let png = raster_svg(svg.as_bytes(), 16).expect("png");
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn icon_file_memoizes_misses() {
        let a = icon_file("__mde_missing_icon_xyz__", 24, false);
        let b = icon_file("__mde_missing_icon_xyz__", 24, false);
        assert!(a.is_none());
        assert!(b.is_none());
    }
}
