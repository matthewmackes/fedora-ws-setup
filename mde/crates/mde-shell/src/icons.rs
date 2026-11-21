//! Resolve a StatusNotifier icon name to a theme PNG, when one exists.

pub fn find_png(name: &str, dark_ui: bool) -> Option<String> {
    mde_theme::icons::find_png(name, dark_ui)
}

/// ARGB32 network-order pixmap (SNI `IconPixmap`) to a PNG.
pub fn argb32_to_png(width: i32, height: i32, argb: &[u8]) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 || width > 256 || height > 256 {
        return None;
    }
    let width = width as u32;
    let height = height as u32;
    let pixels = (width as usize).checked_mul(height as usize)?;
    let bytes = pixels.checked_mul(4)?;
    if argb.len() < bytes {
        return None;
    }
    let mut rgba = Vec::with_capacity(bytes);
    for px in argb[..bytes].chunks_exact(4) {
        rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
    }
    let image = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Write a pixmap PNG under the user state dir. Tests do not touch that directory.
pub fn cache_pixmap(width: i32, height: i32, argb: &[u8]) -> Option<String> {
    let png = argb32_to_png(width, height, argb)?;
    if cfg!(test) {
        return None;
    }
    let dir = tray_icon_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{:016x}.png", fnv64(&png)));
    if !path.is_file() {
        std::fs::write(&path, &png).ok()?;
    }
    Some(path.display().to_string())
}

fn tray_icon_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".local/state/mde/tray-icons"),
    )
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
        assert!(find_png("", false).is_none());
        assert!(find_png("../passwd", true).is_none());
        assert!(find_png("a/b", false).is_none());
    }

    #[test]
    fn argb_pixmap_becomes_png() {
        let png = argb32_to_png(1, 1, &[255, 10, 20, 30]).expect("png");
        let image = image::load_from_memory(&png).expect("decode");
        assert_eq!(image.to_rgba8().get_pixel(0, 0).0, [10, 20, 30, 255]);
        assert!(argb32_to_png(0, 1, &[0, 0, 0, 0]).is_none());
        assert!(argb32_to_png(1, 1, &[1, 2, 3]).is_none());
        assert!(cache_pixmap(1, 1, &[255, 10, 20, 30]).is_none());
    }
}
