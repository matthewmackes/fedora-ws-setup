//! Cover art from public web APIs (iTunes / Cover Art Archive), not the media server.

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

/// Resolve a high-res cover URL from the public web. Prefer iTunes CDN, then Cover Art Archive.
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

    let url = itunes_cover(artist, album).or_else(|| cover_art_archive(artist, album));
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(&path, url.as_deref().unwrap_or(""));
    mem_set(&key, url.clone());
    url
}

fn itunes_cover(artist: &str, album: &str) -> Option<String> {
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
    // Bump to higher resolution on Apple's CDN
    Some(
        art.replace("100x100bb", "600x600bb")
            .replace("100x100", "600x600"),
    )
}

fn cover_art_archive(artist: &str, album: &str) -> Option<String> {
    let query = format!("{album} AND artist:{artist}");
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
    let mbid = v
        .pointer("/releases/0/id")
        .and_then(|x| x.as_str())?;
    // CAA redirects to archive.org CDN
    let caa = format!("https://coverartarchive.org/release/{mbid}/front-500");
    // HEAD to confirm
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

pub fn prefer_web_cover(artist: &str, album: &str, server_cover: Option<String>) -> Option<String> {
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
}
