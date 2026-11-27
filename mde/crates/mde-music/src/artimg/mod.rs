//! Unified artist / album imagery (W650–W655, W675, W677).
//!
//! Chain (artist): Fanart.tv (keyed) → Deezer → Wikipedia/Wikimedia → TheAudioDB →
//! Last.fm (keyed) → Discogs (token) → Bandcamp `og:image` (polite scrape).
//! Every candidate keeps its source + licence; the quality gate rejects blank,
//! tiny or placeholder images; names must match exactly (normalized) so a common
//! name never borrows another artist's photo.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const UA: &str = "NeighbourhoodMusic/2026 (+local desktop client)";
const POSITIVE_TTL: u64 = 7 * 86_400;
const NEGATIVE_TTL: u64 = 86_400;
/// W677: never ask for more than 1440p.
pub const MAX_EDGE: u32 = 2560;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Portrait,
    Backdrop,
    Logo,
    Banner,
    Thumb,
    Fan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub url: String,
    pub kind: Kind,
    pub source: String,
    pub license: String,
    pub page: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtistImages {
    pub candidates: Vec<Candidate>,
    pub fetched_at: u64,
}

impl ArtistImages {
    pub fn of(&self, kind: Kind) -> Vec<&Candidate> {
        self.candidates.iter().filter(|c| c.kind == kind).collect()
    }
    pub fn portrait(&self) -> Option<&Candidate> {
        self.of(Kind::Portrait).into_iter().next().or_else(|| self.of(Kind::Thumb).into_iter().next())
    }
    /// Backdrops first, then fan photos, then the portrait as a last resort.
    pub fn backdrops(&self) -> Vec<&Candidate> {
        let mut v = self.of(Kind::Backdrop);
        v.extend(self.of(Kind::Fan));
        if v.is_empty() {
            v.extend(self.portrait());
        }
        v
    }
    pub fn logo(&self) -> Option<&Candidate> {
        self.of(Kind::Logo).into_iter().next()
    }
}

// ---------------------------------------------------------------- settings

pub fn source_enabled(name: &str) -> bool {
    crate::history::History::open()
        .ok()
        .and_then(|h| h.get_setting(&format!("art_src_{name}")))
        .map(|v| v != "0")
        .unwrap_or(true)
}

fn setting(key: &str) -> Option<String> {
    std::env::var(key.to_ascii_uppercase())
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            crate::history::History::open()
                .ok()
                .and_then(|h| h.get_setting(key))
                .filter(|s| !s.trim().is_empty())
        })
}

// ---------------------------------------------------------------- http

static LAST_HIT: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);

/// Per-host minimum spacing so we stay polite (W675).
fn throttle(host: &str, gap: Duration) {
    let wait = {
        let mut g = LAST_HIT.lock();
        let map = g.get_or_insert_with(HashMap::new);
        let now = std::time::Instant::now();
        let wait = map
            .get(host)
            .map(|t| gap.saturating_sub(now.duration_since(*t)))
            .unwrap_or_default();
        map.insert(host.to_string(), now + wait);
        wait
    };
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("").to_string()
}

fn get_json(url: &str, gap_ms: u64) -> Option<Value> {
    throttle(&host_of(url), Duration::from_millis(gap_ms));
    ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?
        .into_json()
        .ok()
}

fn get_text(url: &str, gap_ms: u64) -> Option<String> {
    throttle(&host_of(url), Duration::from_millis(gap_ms));
    ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?
        .into_string()
        .ok()
}

// ---------------------------------------------------------------- matching

pub fn norm_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        }
    }
    out.strip_prefix("the").map(str::to_string).filter(|s| !s.is_empty()).unwrap_or(out)
}

pub fn names_match(a: &str, b: &str) -> bool {
    let (a, b) = (norm_name(a), norm_name(b));
    !a.is_empty() && a == b
}

// ---------------------------------------------------------------- sources

fn cand(url: &str, kind: Kind, source: &str, license: &str, page: &str, w: u32, h: u32) -> Candidate {
    Candidate {
        url: url.to_string(),
        kind,
        source: source.to_string(),
        license: license.to_string(),
        page: page.to_string(),
        width: w,
        height: h,
    }
}

fn fanart(mbid: &str) -> Vec<Candidate> {
    let Some(key) = setting("fanart_api_key") else { return Vec::new() };
    let url = format!("https://webservice.fanart.tv/v3/music/{mbid}?api_key={key}");
    let Some(v) = get_json(&url, 500) else { return Vec::new() };
    parse_fanart(&v)
}

pub fn parse_fanart(v: &Value) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut take = |field: &str, kind: Kind, w: u32, h: u32| {
        let mut items: Vec<&Value> = v.get(field).and_then(|a| a.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
        items.sort_by_key(|i| {
            std::cmp::Reverse(i.get("likes").and_then(|l| l.as_str()).and_then(|l| l.parse::<i64>().ok()).unwrap_or(0))
        });
        for i in items.into_iter().take(8) {
            if let Some(u) = i.get("url").and_then(|u| u.as_str()) {
                out.push(cand(u, kind, "Fanart.tv", "Fanart.tv terms", "https://fanart.tv", w, h));
            }
        }
    };
    take("artistbackground", Kind::Backdrop, 1920, 1080);
    take("artistthumb", Kind::Portrait, 1000, 1000);
    take("hdmusiclogo", Kind::Logo, 800, 310);
    take("musiclogo", Kind::Logo, 400, 155);
    take("musicbanner", Kind::Banner, 1000, 185);
    out
}

fn deezer(name: &str) -> Vec<Candidate> {
    let url = format!("https://api.deezer.com/search/artist?q={}&limit=5", urlencoding::encode(name));
    let Some(v) = get_json(&url, 250) else { return Vec::new() };
    parse_deezer(&v, name)
}

pub fn parse_deezer(v: &Value, name: &str) -> Vec<Candidate> {
    let Some(items) = v.get("data").and_then(|d| d.as_array()) else { return Vec::new() };
    for a in items {
        let n = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if !names_match(n, name) {
            continue;
        }
        let pic = a.get("picture_xl").and_then(|p| p.as_str()).unwrap_or("");
        // Deezer's silhouette placeholder has an empty hash segment.
        if pic.is_empty() || pic.contains("/artist//") {
            continue;
        }
        let page = a.get("link").and_then(|l| l.as_str()).unwrap_or("https://www.deezer.com");
        return vec![cand(pic, Kind::Portrait, "Deezer", "Deezer terms", page, 1000, 1000)];
    }
    Vec::new()
}

fn wikipedia(name: &str, wikipedia_title: Option<&str>) -> Vec<Candidate> {
    let title = wikipedia_title.map(str::to_string).unwrap_or_else(|| name.replace(' ', "_"));
    let url = format!("https://en.wikipedia.org/api/rest_v1/page/summary/{}", urlencoding::encode(&title));
    let Some(v) = get_json(&url, 250) else { return Vec::new() };
    parse_wikipedia(&v, name, wikipedia_title.is_some())
}

pub fn parse_wikipedia(v: &Value, name: &str, trusted_title: bool) -> Vec<Candidate> {
    let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("");
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if ty == "disambiguation" {
        return Vec::new();
    }
    let base = title.split(" (").next().unwrap_or(title);
    if !trusted_title && !names_match(base, name) {
        return Vec::new();
    }
    let Some(img) = v.get("originalimage") else { return Vec::new() };
    let src = img.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let w = img.get("width").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
    let h = img.get("height").and_then(|h| h.as_u64()).unwrap_or(0) as u32;
    if src.is_empty() {
        return Vec::new();
    }
    let page = v.pointer("/content_urls/desktop/page").and_then(|p| p.as_str()).unwrap_or("https://en.wikipedia.org");
    let kind = if w > h * 12 / 10 { Kind::Fan } else { Kind::Portrait };
    vec![cand(src, kind, "Wikimedia Commons", "Free licence (see page)", page, w, h)]
}

fn audiodb(name: &str) -> Vec<Candidate> {
    let key = setting("audiodb_api_key").unwrap_or_else(|| "2".into());
    let url = format!("https://www.theaudiodb.com/api/v1/json/{key}/search.php?s={}", urlencoding::encode(name));
    let Some(v) = get_json(&url, 700) else { return Vec::new() };
    parse_audiodb(&v, name)
}

pub fn parse_audiodb(v: &Value, name: &str) -> Vec<Candidate> {
    let Some(a) = v.pointer("/artists/0") else { return Vec::new() };
    if !names_match(a.get("strArtist").and_then(|n| n.as_str()).unwrap_or(""), name) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut add = |f: &str, k: Kind, w: u32, h: u32| {
        if let Some(u) = a.get(f).and_then(|u| u.as_str()).filter(|u| !u.is_empty()) {
            out.push(cand(u, k, "TheAudioDB", "TheAudioDB terms", "https://www.theaudiodb.com", w, h));
        }
    };
    add("strArtistFanart", Kind::Backdrop, 1920, 1080);
    add("strArtistFanart2", Kind::Backdrop, 1920, 1080);
    add("strArtistFanart3", Kind::Backdrop, 1920, 1080);
    add("strArtistThumb", Kind::Portrait, 1000, 1000);
    add("strArtistLogo", Kind::Logo, 800, 310);
    add("strArtistBanner", Kind::Banner, 1000, 185);
    out
}

fn lastfm(name: &str) -> Vec<Candidate> {
    let Some(key) = setting("lastfm_api_key") else { return Vec::new() };
    let url = format!(
        "https://ws.audioscrobbler.com/2.0/?method=artist.getinfo&artist={}&api_key={key}&format=json",
        urlencoding::encode(name)
    );
    let Some(v) = get_json(&url, 250) else { return Vec::new() };
    let Some(img) = v.pointer("/artist/image").and_then(|i| i.as_array()) else { return Vec::new() };
    // Last.fm serves a star placeholder; the hash 2a96cbd8… is that placeholder.
    img.iter()
        .rev()
        .filter_map(|i| i.get("#text").and_then(|t| t.as_str()))
        .find(|u| !u.is_empty() && !u.contains("2a96cbd8b46e442fc41c2b86b821562f"))
        .map(|u| vec![cand(u, Kind::Portrait, "Last.fm", "Last.fm terms", "https://www.last.fm", 300, 300)])
        .unwrap_or_default()
}

fn discogs(name: &str) -> Vec<Candidate> {
    let Some(token) = setting("discogs_token") else { return Vec::new() };
    let url = format!(
        "https://api.discogs.com/database/search?q={}&type=artist&token={token}",
        urlencoding::encode(name)
    );
    let Some(v) = get_json(&url, 1100) else { return Vec::new() };
    let Some(items) = v.get("results").and_then(|r| r.as_array()) else { return Vec::new() };
    for r in items {
        let title = r.get("title").and_then(|t| t.as_str()).unwrap_or("");
        let img = r.get("cover_image").and_then(|i| i.as_str()).unwrap_or("");
        if names_match(title, name) && !img.is_empty() && !img.contains("spacer.gif") {
            return vec![cand(img, Kind::Portrait, "Discogs", "Discogs terms", "https://www.discogs.com", 600, 600)];
        }
    }
    Vec::new()
}

/// Polite robots.txt check (only the `User-agent: *` group, simple prefix rules).
pub fn robots_allows(robots: &str, path: &str) -> bool {
    let mut in_star = false;
    for line in robots.lines() {
        let l = line.split('#').next().unwrap_or("").trim();
        if let Some(ua) = l.to_ascii_lowercase().strip_prefix("user-agent:") {
            in_star = ua.trim() == "*";
        } else if in_star {
            if let Some(rule) = l.to_ascii_lowercase().strip_prefix("disallow:") {
                let rule = rule.trim();
                if !rule.is_empty() && path.starts_with(rule) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn parse_og_image(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("property=\"og:image\"") {
        let start = from + i;
        let tag_start = lower[..start].rfind('<')?;
        let tag_end = lower[start..].find('>').map(|e| start + e)?;
        let tag = &html[tag_start..tag_end];
        if let Some(c) = tag.to_ascii_lowercase().find("content=\"") {
            let rest = &tag[c + 9..];
            if let Some(end) = rest.find('"') {
                let u = &rest[..end];
                if u.starts_with("http") {
                    return Some(u.replace("&amp;", "&"));
                }
            }
        }
        from = tag_end;
    }
    None
}

fn bandcamp(name: &str) -> Vec<Candidate> {
    let robots = get_text("https://bandcamp.com/robots.txt", 1000).unwrap_or_default();
    if !robots_allows(&robots, "/search") {
        return Vec::new();
    }
    let url = format!("https://bandcamp.com/search?q={}&item_type=b", urlencoding::encode(name));
    let Some(html) = get_text(&url, 1500) else { return Vec::new() };
    // First result's page link, then that page's og:image.
    let Some(i) = html.find("class=\"itemurl\"") else { return Vec::new() };
    let rest = &html[i..];
    let Some(t) = rest.find("http") else { return Vec::new() };
    let link: String = rest[t..].chars().take_while(|c| !c.is_whitespace() && *c != '<').collect();
    let link = link.split('?').next().unwrap_or("").to_string();
    if !link.contains(".bandcamp.com") {
        return Vec::new();
    }
    let Some(page) = get_text(&link, 1500) else { return Vec::new() };
    let title_ok = page
        .split("<title>")
        .nth(1)
        .and_then(|t| t.split("</title>").next())
        .is_some_and(|t| t.to_ascii_lowercase().contains(&name.to_ascii_lowercase()));
    if !title_ok {
        return Vec::new();
    }
    parse_og_image(&page)
        .map(|u| vec![cand(&u, Kind::Fan, "Bandcamp", "Artist-provided press image", &link, 700, 700)])
        .unwrap_or_default()
}

// ---------------------------------------------------------------- resolver

fn cache_path(name: &str) -> PathBuf {
    let safe: String = norm_name(name).chars().take(80).collect();
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("artist-images")
        .join(format!("{safe}.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn cached(name: &str) -> Option<ArtistImages> {
    let text = std::fs::read_to_string(cache_path(name)).ok()?;
    let a: ArtistImages = serde_json::from_str(&text).ok()?;
    let ttl = if a.candidates.is_empty() { NEGATIVE_TTL } else { POSITIVE_TTL };
    (now().saturating_sub(a.fetched_at) < ttl).then_some(a)
}

fn store(name: &str, a: &ArtistImages) {
    let p = cache_path(name);
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(t) = serde_json::to_string(a) {
        let _ = std::fs::write(p, t);
    }
}

static INFLIGHT: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// Resolve every source for an artist. Blocking: call from a worker thread.
pub fn artist_images(name: &str, mbid: Option<&str>, wikipedia_title: Option<&str>) -> ArtistImages {
    let name = name.trim();
    if name.is_empty() {
        return ArtistImages::default();
    }
    if let Some(hit) = cached(name) {
        return hit;
    }
    let key = norm_name(name);
    {
        let mut g = INFLIGHT.lock();
        let set = g.get_or_insert_with(Default::default);
        if !set.insert(key.clone()) {
            drop(g);
            // Another thread is resolving: wait briefly for its result.
            for _ in 0..40 {
                std::thread::sleep(Duration::from_millis(250));
                if let Some(hit) = cached(name) {
                    return hit;
                }
            }
            return ArtistImages::default();
        }
    }
    let mut all: Vec<Candidate> = Vec::new();
    if let (Some(m), true) = (mbid, source_enabled("fanart")) {
        all.extend(fanart(m));
    }
    if source_enabled("audiodb") {
        all.extend(audiodb(name));
    }
    if source_enabled("deezer") {
        all.extend(deezer(name));
    }
    if source_enabled("wikimedia") {
        all.extend(wikipedia(name, wikipedia_title));
    }
    if source_enabled("lastfm") && !all.iter().any(|c| c.kind == Kind::Portrait) {
        all.extend(lastfm(name));
    }
    if source_enabled("discogs") && !all.iter().any(|c| c.kind == Kind::Portrait) {
        all.extend(discogs(name));
    }
    if source_enabled("bandcamp") && all.len() < 2 {
        all.extend(bandcamp(name));
    }
    let mut seen = std::collections::HashSet::new();
    all.retain(|c| seen.insert(c.url.clone()));
    let out = ArtistImages { candidates: all, fetched_at: now() };
    store(name, &out);
    INFLIGHT.lock().as_mut().map(|s| s.remove(&key));
    out
}

// ---------------------------------------------------------------- quality gate (W653)

#[derive(Debug, PartialEq, Eq)]
pub enum Use {
    Backdrop,
    Portrait,
}

/// Decide whether downloaded image bytes are good enough for `use_`.
pub fn quality_ok(bytes: &[u8], use_: Use) -> bool {
    let Ok(img) = image::load_from_memory(bytes) else { return false };
    let (w, h) = (img.width(), img.height());
    let short = w.min(h);
    let ok_size = match use_ {
        Use::Backdrop => short >= 600 && w >= h,
        Use::Portrait => short >= 400,
    };
    if !ok_size {
        return false;
    }
    // Reject near-solid images (blank placeholders).
    let small = img.resize(16, 16, image::imageops::FilterType::Triangle).to_luma8();
    let px: Vec<f64> = small.pixels().map(|p| p[0] as f64).collect();
    let mean = px.iter().sum::<f64>() / px.len() as f64;
    let var = px.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / px.len() as f64;
    var.sqrt() >= 4.0
}

/// Download a candidate (via the shared image cache) and return a local path
/// only if it passes the gate for `use_`.
pub fn fetch_good(c: &Candidate, use_: Use) -> Option<PathBuf> {
    let path = crate::artwork::fetch_image_file(&cap_url(&c.url)).ok()?;
    let bytes = std::fs::read(&path).ok()?;
    if quality_ok(&bytes, use_) {
        Some(path)
    } else {
        let _ = std::fs::remove_file(&path);
        None
    }
}

/// W677: ask CDNs for at most 1440p-class variants where the URL allows it.
pub fn cap_url(url: &str) -> String {
    // Wikimedia thumbnails: …/3840px-Name.jpg → clamp to 1920 px.
    if url.contains("upload.wikimedia.org") || url.contains("thumb.wikimedia.org") {
        if let Some(i) = url.rfind('/') {
            let (head, file) = url.split_at(i + 1);
            if let Some(px) = file.find("px-") {
                if let Ok(n) = file[..px].parse::<u32>() {
                    if n > 1920 {
                        return format!("{head}1920{}", &file[px..]);
                    }
                }
            }
        }
    }
    // Apple/iTunes: …/1200x1200bb.jpg → clamp to 1440
    if url.contains("mzstatic.com") {
        return regex_free_replace_size(url, 1440);
    }
    url.to_string()
}

fn regex_free_replace_size(url: &str, edge: u32) -> String {
    let Some(slash) = url.rfind('/') else { return url.to_string() };
    let (head, file) = url.split_at(slash + 1);
    let Some(x) = file.find('x') else { return url.to_string() };
    let digits_before = file[..x].chars().all(|c| c.is_ascii_digit()) && !file[..x].is_empty();
    if !digits_before {
        return url.to_string();
    }
    let after: String = file[x + 1..].chars().skip_while(|c| c.is_ascii_digit()).collect();
    format!("{head}{edge}x{edge}{after}")
}


// ---------------------------------------------------------------- background portraits (W655, W665)

type PortraitSlot = std::sync::Arc<Mutex<Option<PathBuf>>>;

static PORTRAIT_QUEUE: std::sync::OnceLock<std::sync::mpsc::Sender<(String, PortraitSlot)>> =
    std::sync::OnceLock::new();

/// Queue a portrait lookup on a small shared worker pool (2 threads). The slot is
/// filled with a quality-gated local file when one is found.
pub fn portrait_async(name: &str, slot: PortraitSlot) {
    let tx = PORTRAIT_QUEUE.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(String, PortraitSlot)>();
        let rx = std::sync::Arc::new(Mutex::new(rx));
        for _ in 0..2 {
            let rx = rx.clone();
            std::thread::spawn(move || loop {
                let job = rx.lock().recv();
                let Ok((name, slot)) = job else { break };
                let imgs = artist_images(&name, None, None);
                if let Some(c) = imgs.portrait() {
                    if let Some(p) = fetch_good(c, Use::Portrait) {
                        *slot.lock() = Some(p);
                    }
                }
            });
        }
        tx
    });
    let _ = tx.send((name.to_string(), slot));
}

pub fn resolve_for_ui(name: &str, mbid: Option<&str>) -> Result<ArtistImages> {
    Ok(artist_images(name, mbid, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn name_matching_is_strict_but_ignores_punctuation_and_the() {
        assert!(names_match("Daft Punk", "daft  punk"));
        assert!(names_match("The Beatles", "Beatles"));
        assert!(!names_match("Genesis", "Genesis Owusu"));
        assert!(!names_match("", ""));
    }

    #[test]
    fn deezer_rejects_wrong_artist_and_silhouette() {
        let v = json!({"data":[
            {"name":"Daft Punkers","picture_xl":"https://x/artist/abc/1000x1000.jpg"},
            {"name":"Daft Punk","picture_xl":"https://cdn/images/artist//1000x1000.jpg"}]});
        assert!(parse_deezer(&v, "Daft Punk").is_empty());
        let ok = json!({"data":[{"name":"Daft Punk","picture_xl":"https://cdn/images/artist/638e/1000x1000.jpg","link":"https://deezer/x"}]});
        let c = parse_deezer(&ok, "Daft Punk");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, Kind::Portrait);
    }

    #[test]
    fn wikipedia_needs_a_matching_title_and_skips_disambiguation() {
        let v = json!({"title":"Daft Punk","type":"standard","originalimage":{"source":"https://u/a.jpg","width":2000,"height":1200}});
        let c = parse_wikipedia(&v, "Daft Punk", false);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, Kind::Fan, "wide image is a fan/backdrop candidate");
        assert!(parse_wikipedia(&v, "Someone Else", false).is_empty());
        assert_eq!(parse_wikipedia(&v, "Someone Else", true).len(), 1);
        let d = json!({"title":"Daft","type":"disambiguation","originalimage":{"source":"https://u/a.jpg","width":9,"height":9}});
        assert!(parse_wikipedia(&d, "Daft", false).is_empty());
    }

    #[test]
    fn fanart_sorts_by_likes_and_splits_kinds() {
        let v = json!({
            "artistbackground":[{"url":"https://f/a.jpg","likes":"1"},{"url":"https://f/b.jpg","likes":"9"}],
            "hdmusiclogo":[{"url":"https://f/l.png","likes":"2"}],
            "artistthumb":[{"url":"https://f/t.jpg","likes":"3"}]});
        let c = parse_fanart(&v);
        let imgs = ArtistImages { candidates: c, fetched_at: 0 };
        assert_eq!(imgs.backdrops()[0].url, "https://f/b.jpg");
        assert_eq!(imgs.logo().unwrap().url, "https://f/l.png");
        assert_eq!(imgs.portrait().unwrap().url, "https://f/t.jpg");
    }

    #[test]
    fn audiodb_requires_matching_artist() {
        let v = json!({"artists":[{"strArtist":"Other","strArtistFanart":"https://a/f.jpg"}]});
        assert!(parse_audiodb(&v, "Daft Punk").is_empty());
        let v = json!({"artists":[{"strArtist":"Daft Punk","strArtistFanart":"https://a/f.jpg","strArtistThumb":"https://a/t.jpg"}]});
        assert_eq!(parse_audiodb(&v, "Daft Punk").len(), 2);
    }

    #[test]
    fn robots_and_og_image() {
        let r = "User-agent: *\nDisallow: /private\nUser-agent: bot\nDisallow: /";
        assert!(robots_allows(r, "/search"));
        assert!(!robots_allows(r, "/private/x"));
        let html = r#"<head><meta property="og:image" content="https://f4.bcbits.com/img/a.jpg?x=1&amp;y=2"></head>"#;
        assert_eq!(parse_og_image(html).unwrap(), "https://f4.bcbits.com/img/a.jpg?x=1&y=2");
    }

    #[test]
    fn quality_gate_rejects_small_and_blank() {
        let enc = |w: u32, h: u32, f: &dyn Fn(u32, u32) -> [u8; 3]| {
            let mut img = image::RgbImage::new(w, h);
            for (x, y, p) in img.enumerate_pixels_mut() {
                *p = image::Rgb(f(x, y));
            }
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
            buf.into_inner()
        };
        let noisy = |x: u32, y: u32| [(x * 255 / 400 % 256) as u8, (y * 255 / 300 % 256) as u8, (((x / 50) + (y / 50)) % 2 * 200) as u8];
        let solid = |_: u32, _: u32| [10, 10, 10];
        assert!(quality_ok(&enc(1280, 720, &noisy), Use::Backdrop));
        assert!(!quality_ok(&enc(400, 300, &noisy), Use::Backdrop), "too small");
        assert!(!quality_ok(&enc(700, 1200, &noisy), Use::Backdrop), "portrait-oriented");
        assert!(!quality_ok(&enc(1280, 720, &solid), Use::Backdrop), "blank");
        assert!(quality_ok(&enc(500, 500, &noisy), Use::Portrait));
    }

    #[test]
    fn caps_apple_art_at_1440() {
        let u = "https://is1-ssl.mzstatic.com/image/thumb/Music/a/b/3000x3000bb.jpg";
        assert!(cap_url(u).ends_with("/1440x1440bb.jpg"), "{}", cap_url(u));
        assert_eq!(cap_url("https://x/y.jpg"), "https://x/y.jpg");
        let w = "https://thumb.wikimedia.org/wikipedia/commons/thumb/d/df/A.jpg/3840px-A.jpg?utm=1";
        assert!(cap_url(w).contains("/1920px-A.jpg"), "{}", cap_url(w));
        let small = "https://thumb.wikimedia.org/x/1280px-A.jpg";
        assert_eq!(cap_url(small), small);
    }
}
