//! Cover art: server first, then CAA / Fanart / iTunes (hero) / YouTube fallbacks.

use anyhow::{bail, Result};
use md5::Digest;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

static MEM: Mutex<Option<HashMap<String, (Instant, Option<String>)>>> = Mutex::new(None);
const TTL: Duration = Duration::from_secs(86_400);

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("web-art")
}

fn cache_key(artist: &str, album: &str) -> String {
    format!(
        "{}|{}",
        artist.trim().to_ascii_lowercase(),
        album.trim().to_ascii_lowercase()
    )
}

fn disk_path(key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii() && c.is_alphanumeric() { c } else { '_' })
        .collect();
    cache_dir().join(format!("{safe}.url"))
}

fn mem_get(key: &str) -> Option<Option<String>> {
    let mut guard = MEM.lock();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    let map = guard.as_mut().unwrap();
    if let Some((at, val)) = map.get(key) {
        if at.elapsed() < TTL {
            return Some(val.clone());
        }
    }
    None
}

fn mem_set(key: &str, val: Option<String>) {
    let mut guard = MEM.lock();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    guard.as_mut().unwrap().insert(key.to_string(), (Instant::now(), val));
}

/// Resolve a web cover for shelves/lists — Cover Art Archive first (no iTunes burn).
pub fn resolve_cover(artist: &str, album: &str) -> Option<String> {
    let artist = artist.trim();
    let album = album.trim();
    if artist.is_empty() && album.is_empty() {
        return None;
    }
    let key = cache_key(artist, album);
    if let Some(hit) = mem_get(&key) {
        return hit;
    }
    let path = disk_path(&key);
    if let Ok(text) = fs::read_to_string(&path) {
        let url = text.trim().to_string();
        if url.is_empty() {
            mem_set(&key, None);
            return None;
        }
        mem_set(&key, Some(url.clone()));
        return Some(url);
    }

    let url = cover_art_archive(artist, album, 500);
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(&path, url.as_deref().unwrap_or(""));
    mem_set(&key, url.clone());
    url
}

/// Hero / Stage HD — iTunes 1200 when possible, then CAA 1200, then server.
pub fn hero_cover(artist: &str, album: &str, server_cover: Option<String>) -> Option<String> {
    let key = format!("hero|{}", cache_key(artist, album));
    if let Some(hit) = mem_get(&key) {
        return hit.or(server_cover);
    }
    let url = itunes_cover_hd(artist, album)
        .or_else(|| cover_art_archive(artist, album, 1200))
        .or_else(|| resolve_cover(artist, album));
    mem_set(&key, url.clone());
    url.or(server_cover)
}

fn fanart_api_key() -> Option<String> {
    std::env::var("FANART_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            crate::history::History::open()
                .ok()
                .and_then(|h| h.get_setting("fanart_api_key"))
                .filter(|s| !s.trim().is_empty())
        })
}

/// Fanart.tv artist thumb + background (optional API key).
pub fn fanart_for_mbid(mbid: &str) -> Option<(Option<String>, Option<String>)> {
    let key = fanart_api_key()?;
    let url = format!("https://webservice.fanart.tv/v3/music/{mbid}?api_key={key}");
    let resp = ureq::get(&url)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .timeout(Duration::from_secs(6))
        .call()
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    let thumb = v
        .pointer("/artistthumb/0/url")
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .or_else(|| {
            v.pointer("/artistthumb/0/url")
                .and_then(|u| u.as_str())
                .map(str::to_string)
        });
    let bg = v
        .pointer("/artistbackground/0/url")
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .or_else(|| {
            v.pointer("/musicbanner/0/url")
                .and_then(|u| u.as_str())
                .map(str::to_string)
        });
    if thumb.is_none() && bg.is_none() {
        return None;
    }
    Some((thumb, bg))
}

fn itunes_cover_hd(artist: &str, album: &str) -> Option<String> {
    let term = if album.is_empty() {
        artist.to_string()
    } else if artist.is_empty() {
        album.to_string()
    } else {
        format!("{artist} {album}")
    };
    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=album&limit=5",
        urlencoding::encode(&term)
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "NeighbourhoodMusic/2026 (MDE)")
        .timeout(Duration::from_secs(4))
        .call()
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    let results = v.get("results")?.as_array()?;
    let album_lc = album.to_ascii_lowercase();
    let artist_lc = artist.to_ascii_lowercase();
    let best = results
        .iter()
        .find(|r| {
            let an = r
                .get("collectionName")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let ar = r
                .get("artistName")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            (!album_lc.is_empty() && an.contains(&album_lc))
                || (!artist_lc.is_empty() && ar.contains(&artist_lc))
        })
        .or_else(|| results.first())?;
    let art = best.get("artworkUrl100")?.as_str()?;
    Some(
        art.replace("100x100bb", "1200x1200bb")
            .replace("100x100", "1200x1200"),
    )
}

fn cover_art_archive(artist: &str, album: &str, size: u32) -> Option<String> {
    let query = if artist.is_empty() {
        album.to_string()
    } else if album.is_empty() {
        format!("artist:{artist}")
    } else {
        format!("{album} AND artist:{artist}")
    };
    let url = format!(
        "https://musicbrainz.org/ws/2/release/?query={}&fmt=json&limit=1",
        urlencoding::encode(&query)
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "NeighbourhoodMusic/2026 (MDE; https://localhost)")
        .timeout(Duration::from_secs(5))
        .call()
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    let mbid = v.pointer("/releases/0/id").and_then(|x| x.as_str())?;
    let caa = format!("https://coverartarchive.org/release/{mbid}/front-{size}");
    match ureq::head(&caa)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .timeout(Duration::from_secs(4))
        .call()
    {
        Ok(r) if (200..400).contains(&r.status()) => Some(caa),
        _ => Some(format!("https://coverartarchive.org/release/{mbid}/front")),
    }
}

/// Stable identity for a cover. Subsonic puts a fresh salt and token on every
/// URL, and the old 120-character filename cut off `id=`, so every album
/// collided or missed the cache.
fn stable_art_key(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    let host = parsed.host_str().unwrap_or("");
    let path = parsed.path();
    if let Some((_, id)) = parsed.query_pairs().find(|(k, _)| k == "id") {
        return format!("{host}{path}?id={id}");
    }
    format!("{host}{path}")
}

fn image_cache_path(url: &str) -> PathBuf {
    let digest = md5::Md5::digest(stable_art_key(url).as_bytes());
    cache_dir().join(format!("{}.img", hex::encode(digest)))
}

fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.len() >= 32
        && (bytes.starts_with(b"\xFF\xD8")
            || bytes.starts_with(b"\x89PNG")
            || bytes.starts_with(b"GIF8")
            || (bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"))
}

/// Average the colorful pixels, then keep the result visible on the dark shell.
pub fn accent_from_samples(pixels: &[(u8, u8, u8)]) -> (u8, u8, u8) {
    if pixels.is_empty() {
        return (90, 150, 200);
    }
    let mut count = 0u64;
    let mut r = 0u64;
    let mut g = 0u64;
    let mut b = 0u64;
    for &(pr, pg, pb) in pixels {
        let max = pr.max(pg).max(pb);
        let min = pr.min(pg).min(pb);
        if max < 28 || min > 232 {
            continue;
        }
        r += pr as u64;
        g += pg as u64;
        b += pb as u64;
        count += 1;
    }
    if count == 0 {
        for &(pr, pg, pb) in pixels {
            r += pr as u64;
            g += pg as u64;
            b += pb as u64;
        }
        count = pixels.len() as u64;
    }
    lift_accent(
        (r / count) as u8,
        (g / count) as u8,
        (b / count) as u8,
    )
}

fn lift_accent(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let lum = relative_luminance(r, g, b);
    if lum < 0.08 {
        mix(r, g, b, 70, 120, 170, 0.55)
    } else if lum > 0.85 {
        mix(r, g, b, 40, 70, 110, 0.45)
    } else {
        (r, g, b)
    }
}

fn mix(r: u8, g: u8, b: u8, tr: u8, tg: u8, tb: u8, toward: f64) -> (u8, u8, u8) {
    let blend = |a: u8, b: u8| (a as f64 * (1.0 - toward) + b as f64 * toward).round() as u8;
    (blend(r, tr), blend(g, tg), blend(b, tb))
}

fn channel_linear(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

pub fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * channel_linear(r) + 0.7152 * channel_linear(g) + 0.0722 * channel_linear(b)
}

fn composite(fg: u8, bg: u8, alpha: f64) -> u8 {
    (fg as f64 * alpha + bg as f64 * (1.0 - alpha)).round() as u8
}

/// Light text on the tinted scrim. The scrim stays mostly the dark shell.
pub fn scrim_contrast(r: u8, g: u8, b: u8) -> f64 {
    let sr = composite(r, 9, 0.28);
    let sg = composite(g, 11, 0.28);
    let sb = composite(b, 15, 0.28);
    let text = relative_luminance(243, 245, 247);
    let bg = relative_luminance(sr, sg, sb);
    (text.max(bg) + 0.05) / (text.min(bg) + 0.05)
}

pub fn accent_css(r: u8, g: u8, b: u8) -> String {
    format!(
        "window.nm-window .nm-player {{ border-top: 2px solid rgb({r},{g},{b}); }}\n\
         window.nm-window .nm-stage {{ background-image: linear-gradient(180deg, rgba({r},{g},{b},0.28), rgba(8,10,14,0.15)); }}\n\
         window.nm-window .nm-album-bleed, window.nm-window .nm-performer-panel {{ box-shadow: inset 0 0 0 1px rgba({r},{g},{b},0.55); }}\n"
    )
}

pub fn accent_from_file(path: &std::path::Path) -> Option<(u8, u8, u8)> {
    let image = image::open(path).ok()?;
    let small = image.resize(24, 24, image::imageops::FilterType::Triangle);
    let pixels: Vec<(u8, u8, u8)> = small
        .to_rgb8()
        .pixels()
        .map(|pixel| (pixel[0], pixel[1], pixel[2]))
        .collect();
    Some(accent_from_samples(&pixels))
}

/// Return a local path only if the image was already downloaded.
pub fn cached_image_path(url: &str) -> Option<PathBuf> {
    let path = image_cache_path(url);
    path.exists().then_some(path)
}

/// Download remote artwork bytes (web CDN or server) into the web-art cache as a local file.
pub fn fetch_image_file(url: &str) -> Result<PathBuf> {
    let path = image_cache_path(url);
    if path.exists() {
        return Ok(path);
    }
    fs::create_dir_all(cache_dir())?;
    let resp = ureq::get(url)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .timeout(Duration::from_secs(8))
        .call()?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)?;
    if !looks_like_image(&bytes) {
        bail!("cover response is not an image");
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(tmp, &path)?;
    Ok(path)
}

/// Smaller Subsonic cover for list rows. The cache key ignores `size` and auth.
pub fn thumbnail_request_url(url: &str) -> String {
    if url.contains("getCoverArt") && !url.contains("size=") {
        format!("{url}&size=240")
    } else {
        url.to_string()
    }
}

/// Server art first; web (CAA) only when missing.
pub fn prefer_web_cover(artist: &str, album: &str, server_cover: Option<String>) -> Option<String> {
    if server_cover
        .as_ref()
        .is_some_and(|u| u.contains("://"))
    {
        return server_cover;
    }
    resolve_cover(artist, album).or(server_cover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_cache_key_keeps_id_and_ignores_auth() {
        let a = "http://172.20.0.2:4040/rest/getCoverArt.view?u=admin&t=aaaabbbbccccddddeeeeffffaaaabbbb&s=0123456789ab&v=1.15.0&c=NeighbourhoodMusic&f=json&id=al-1";
        let b = "http://172.20.0.2:4040/rest/getCoverArt.view?u=admin&t=11112222333344445555666677778888&s=zzzzzzzzzzzz&v=1.15.0&c=NeighbourhoodMusic&f=json&id=al-1&size=240";
        let c = "http://172.20.0.2:4040/rest/getCoverArt.view?u=admin&t=11112222333344445555666677778888&s=zzzzzzzzzzzz&v=1.15.0&c=NeighbourhoodMusic&f=json&id=al-2";
        assert_eq!(stable_art_key(a), stable_art_key(b));
        assert_ne!(stable_art_key(a), stable_art_key(c));
        assert!(stable_art_key(a).contains("id=al-1"));
        assert_eq!(image_cache_path(a), image_cache_path(b));
    }

    #[test]
    fn accent_follows_the_cover_and_keeps_light_text() {
        let red = accent_from_samples(&[(180, 30, 40); 16]);
        assert!(red.0 > red.1 && red.0 > red.2, "{red:?}");
        for sample in [
            accent_from_samples(&[(0, 0, 0); 8]),
            accent_from_samples(&[(255, 255, 255); 8]),
            red,
            accent_from_samples(&[(20, 180, 90); 8]),
        ] {
            let contrast = scrim_contrast(sample.0, sample.1, sample.2);
            assert!(contrast >= 4.5, "{sample:?} contrast {contrast}");
        }
        let css = accent_css(red.0, red.1, red.2);
        assert!(css.contains("nm-player"));
        assert!(css.contains("nm-stage"));
    }

    #[test]
    fn accent_from_a_png_matches_its_color() {
        let path = std::env::temp_dir().join(format!("nm-accent-{}.png", std::process::id()));
        let image = image::RgbImage::from_pixel(16, 16, image::Rgb([20, 40, 180]));
        image.save(&path).unwrap();
        let (r, g, b) = accent_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(b > r && b > g, "{r} {g} {b}");
    }
}
