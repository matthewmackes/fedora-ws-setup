//! Podcast discovery from Apple Podcasts and Podcast Index, plus local favorites.
//!
//! Podcast Index is used when `~/.config/neighbourhood-music/podcast-index.json`
//! contains `api_key` and `api_secret`, or when `PODCAST_INDEX_KEY` and
//! `PODCAST_INDEX_SECRET` are set. Apple Podcasts does not need an account.

use crate::backend::MediaBackend;
use crate::model::{Podcast, PodcastEpisode, Track};
use anyhow::{bail, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod people;
pub use people::Guest;

pub const PAGE: usize = 20;

const USER_AGENT: &str = "NeighbourhoodMusic/2026 (MDE; podcast directory)";

const GENRES: &[(&str, u32)] = &[
    ("News", 1489),
    ("Comedy", 1303),
    ("Society", 1324),
    ("Business", 1321),
    ("Arts", 1301),
];

static RESUME: Mutex<Option<(String, u64)>> = Mutex::new(None);
static LAST_PROGRESS: Mutex<(String, u64)> = Mutex::new((String::new(), 0));

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub feed_url: String,
    #[serde(default)]
    pub artwork: Option<String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub show_id: String,
    pub title: String,
    pub description: String,
    pub stream_url: String,
    #[serde(default)]
    pub artwork: Option<String>,
    #[serde(default)]
    pub published: Option<String>,
    #[serde(default)]
    pub duration_secs: Option<u64>,
    #[serde(default)]
    pub guests: Vec<Guest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingSub {
    feed_url: String,
    title: String,
}

pub fn is_podcast_track(track: &Track) -> bool {
    track.id.starts_with("pc:")
}

pub fn genre_names() -> &'static [&'static str] {
    &["News", "Comedy", "Society", "Business", "Arts"]
}

impl Show {
    pub fn from_server(show: &Podcast) -> Self {
        Self {
            id: format!("server:{}", show.id),
            title: if show.title.trim().is_empty() {
                "Podcast".into()
            } else {
                show.title.clone()
            },
            author: String::new(),
            description: show.description.clone(),
            feed_url: show.feed_url.clone().unwrap_or_default(),
            artwork: show.cover_url.clone(),
            source: "This server".into(),
        }
    }

    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.author.trim().is_empty() {
            parts.push(self.author.trim().to_string());
        }
        if !self.source.trim().is_empty() {
            parts.push(self.source.trim().to_string());
        }
        let desc = clip(&strip_html(&self.description), 80);
        if !desc.is_empty() {
            parts.push(desc);
        }
        if parts.is_empty() {
            "Podcast".into()
        } else {
            parts.join(" · ")
        }
    }
}

impl Episode {
    pub fn from_server(show_id: &str, episode: &PodcastEpisode, show_art: Option<String>) -> Option<Self> {
        let stream = episode.stream_url.trim();
        if !stream.contains("://") {
            return None;
        }
        let title = episode.title.trim();
        Some(Self {
            id: format!("server:{show_id}:{}", episode.id),
            show_id: show_id.to_string(),
            title: if title.is_empty() {
                "Episode".into()
            } else {
                title.to_string()
            },
            description: String::new(),
            stream_url: stream.to_string(),
            artwork: episode.cover_url.clone().or(show_art),
            published: episode.published.clone(),
            duration_secs: episode.duration_secs,
            guests: Vec::new(),
        })
    }

    /// Plain-text episode notes (HTML stripped).
    pub fn notes(&self) -> String {
        strip_html(&self.description)
    }

    pub fn to_track(&self, show: &Show) -> Track {
        Track {
            id: format!("pc:{}", self.id),
            title: self.title.clone(),
            artist: if show.author.trim().is_empty() {
                "Podcast".into()
            } else {
                show.author.clone()
            },
            album: show.title.clone(),
            stream_url: self.stream_url.clone(),
            cover_url: self.artwork.clone().or_else(|| show.artwork.clone()),
            duration_secs: self.duration_secs,
            ..Default::default()
        }
    }

    /// Date and length only (the hero shows the notes separately).
    pub fn subtitle_meta(&self) -> String {
        let mut parts = Vec::new();
        if let Some(date) = self.published.as_deref().and_then(short_date) {
            parts.push(date);
        }
        if let Some(secs) = self.duration_secs {
            parts.push(format_duration(secs));
        }
        parts.join(" · ")
    }
}

pub fn trending() -> Result<Vec<Show>> {
    let mut shows = apple_chart(None)?;
    if let Some(extra) = podcast_index_trending(None) {
        shows = merge_shows(shows, extra);
    }
    Ok(shows)
}

pub fn by_genre(name: &str) -> Result<Vec<Show>> {
    let genre_id = GENRES
        .iter()
        .find(|(label, _)| label.eq_ignore_ascii_case(name.trim()))
        .map(|(_, id)| *id);
    let mut shows = apple_chart(genre_id)?;
    if let Some(extra) = podcast_index_trending(Some(name.trim())) {
        shows = merge_shows(shows, extra);
    }
    Ok(shows)
}

pub fn search(query: &str) -> Result<Vec<Show>> {
    let query = query.trim();
    if query.is_empty() {
        return trending();
    }
    // Apple and Podcast Index both match title, host, and description in their
    // own indexes. The result JSON often omits the description text.
    let mut shows = apple_search(query)?;
    if let Some(extra) = podcast_index_search(query) {
        shows = merge_shows(shows, extra);
    }
    Ok(shows)
}

/// Max episodes kept after a feed parse (newest first). Huge shows like The Daily
/// publish multi‑MB feeds; we only need a recent window for browsing.
pub const FEED_EPISODE_CAP: usize = 200;

pub fn episodes_for_feed(show: &Show) -> Result<Vec<Episode>> {
    let feed = show.feed_url.trim();
    if !feed.contains("://") {
        bail!("this show has no podcast feed");
    }
    // The Daily et al. ship 10–20 MB feeds. Read enough for recent items, then
    // truncate on a complete </item> so the XML still parses.
    let bytes = http_feed_bytes(feed, 8_000_000)?;
    let channel = rss::Channel::read_from(Cursor::new(&bytes)).context("read podcast feed")?;
    let show_art = channel
        .itunes_ext()
        .and_then(|ext| ext.image())
        .map(|url| url.to_string())
        .or_else(|| show.artwork.clone());
    let mut episodes = Vec::new();
    for item in channel.items() {
        let Some(stream) = item.enclosure().map(|enc| enc.url().trim().to_string()) else {
            continue;
        };
        if !stream.contains("://") {
            continue;
        }
        let guid = item.guid().map(|g| g.value().trim().to_string()).unwrap_or_default();
        let id = if guid.is_empty() {
            format!("feed:{}", stable_id(&stream))
        } else {
            format!("feed:{}", stable_id(&guid))
        };
        let itunes = item.itunes_ext();
        let artwork = itunes
            .and_then(|ext| ext.image())
            .map(|url| url.to_string())
            .or_else(|| show_art.clone());
        let duration = itunes
            .and_then(|ext| ext.duration())
            .map(|d| d.to_string())
            .as_deref()
            .and_then(parse_duration);
        let description = item
            .itunes_ext()
            .and_then(|ext| ext.summary())
            .map(|s| s.to_string())
            .or_else(|| item.description().map(|s| s.to_string()))
            .unwrap_or_default();
        let title = item.title().unwrap_or("Episode").trim().to_string();
        let guests = guests_for(item, &title, &description, &show.author);
        episodes.push(Episode {
            id,
            show_id: show.id.clone(),
            title,
            description,
            stream_url: stream,
            artwork,
            published: item.pub_date().map(|s| s.to_string()),
            duration_secs: duration,
            guests,
        });
        if episodes.len() >= FEED_EPISODE_CAP * 2 {
            // RSS order is usually newest-first; stop early on huge feeds.
            break;
        }
    }
    sort_newest(&mut episodes);
    episodes.truncate(FEED_EPISODE_CAP);
    Ok(episodes)
}

fn guests_for(item: &rss::Item, title: &str, description: &str, author: &str) -> Vec<Guest> {
    let tagged = people::guests_from_extensions(item);
    if !tagged.is_empty() {
        return tagged;
    }
    people::guests_from_text(title, &strip_html(description), author)
}

/// Merge server-downloaded episodes with the live RSS window (prefer server streams).
pub fn merge_episode_lists(server: Vec<Episode>, feed: Vec<Episode>) -> Vec<Episode> {
    let mut out: Vec<Episode> = Vec::new();
    let mut seen = std::collections::HashMap::new();
    for ep in server.into_iter().chain(feed) {
        let key = ep.title.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        if let Some(&at) = seen.get(&key) {
            // Server copies carry no notes or guests; borrow them from the feed copy.
            let kept: &mut Episode = &mut out[at];
            if kept.description.trim().is_empty() {
                kept.description = ep.description;
            }
            if kept.guests.is_empty() {
                kept.guests = ep.guests;
            }
            continue;
        }
        seen.insert(key, out.len());
        out.push(ep);
    }
    sort_newest(&mut out);
    out.truncate(FEED_EPISODE_CAP);
    out
}

pub fn sort_newest(episodes: &mut [Episode]) {
    episodes.sort_by(|a, b| publish_key(b.published.as_deref()).cmp(&publish_key(a.published.as_deref())));
}

fn publish_key(value: Option<&str>) -> i64 {
    let Some(value) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0;
    };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return dt.timestamp();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(value) {
        return dt.timestamp();
    }
    0
}

pub fn load_favorites() -> Vec<Show> {
    load_favorites_from(&favorites_path())
}

pub fn load_favorites_from(path: &Path) -> Vec<Show> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<FavoriteFile>(&text) {
        Ok(file) => file
            .shows
            .into_iter()
            .filter(|show| !show.id.trim().is_empty() && !show.title.trim().is_empty())
            .collect(),
        Err(err) => {
            tracing::warn!("podcast favorites: {err}");
            Vec::new()
        }
    }
}

pub fn save_favorites(shows: &[Show]) -> Result<()> {
    save_favorites_to(&favorites_path(), shows)
}

pub fn save_favorites_to(path: &Path, shows: &[Show]) -> Result<()> {
    write_json(path, &FavoriteFile { shows: shows.to_vec() })
}

pub fn queue_subscription(show: &Show) {
    let feed = show.feed_url.trim();
    if !feed.contains("://") {
        return;
    }
    let mut pending = load_pending();
    if pending.iter().any(|item| item.feed_url == feed) {
        return;
    }
    pending.insert(
        0,
        PendingSub {
            feed_url: feed.to_string(),
            title: show.title.clone(),
        },
    );
    let _ = save_pending(&pending);
}

pub fn forget_subscription(feed_url: &str) {
    let feed = feed_url.trim();
    let pending: Vec<PendingSub> = load_pending()
        .into_iter()
        .filter(|item| item.feed_url != feed)
        .collect();
    let _ = save_pending(&pending);
}

pub fn retry_saved_subscriptions(backend: Arc<dyn MediaBackend>) {
    std::thread::spawn(move || {
        let pending = load_pending();
        if pending.is_empty() {
            return;
        }
        let mut left = Vec::new();
        for item in pending {
            match backend.subscribe_podcast(&item.feed_url) {
                Ok(()) => tracing::info!(title = %item.title, "podcast subscribed on server"),
                Err(err) => {
                    tracing::debug!(title = %item.title, "podcast subscribe retry: {err:#}");
                    left.push(item);
                }
            }
        }
        let _ = save_pending(&left);
    });
}

pub fn arm_resume(track_id: &str) {
    let Some(pos) = progress_for(track_id) else {
        return;
    };
    if pos >= 5 {
        *RESUME.lock() = Some((track_id.to_string(), pos));
    }
}

/// Seek target once the pipeline knows the episode length. `None` means keep waiting.
pub fn take_resume(track_id: &str, position: u64, duration: u64) -> Option<u64> {
    let mut guard = RESUME.lock();
    let Some((id, pos)) = guard.clone() else {
        return None;
    };
    if id != track_id {
        return None;
    }
    if duration == 0 {
        return None;
    }
    *guard = None;
    if position > 2 || pos + 15 >= duration {
        None
    } else {
        Some(pos)
    }
}

pub fn note_progress(track_id: &str, position: u64) {
    if !track_id.starts_with("pc:") || position < 5 {
        return;
    }
    {
        let mut last = LAST_PROGRESS.lock();
        if last.0 == track_id && position >= last.1 && position - last.1 < 5 {
            return;
        }
        *last = (track_id.to_string(), position);
    }
    let mut map = load_progress_map();
    map.insert(track_id.to_string(), position);
    let _ = write_json(&progress_path(), &map);
}

fn progress_for(track_id: &str) -> Option<u64> {
    load_progress_map().get(track_id).copied()
}

fn load_progress_map() -> std::collections::BTreeMap<String, u64> {
    let Ok(text) = fs::read_to_string(progress_path()) else {
        return std::collections::BTreeMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn apple_chart(genre: Option<u32>) -> Result<Vec<Show>> {
    let url = match genre {
        Some(id) => format!("https://itunes.apple.com/us/rss/toppodcasts/limit=40/genre={id}/json"),
        None => "https://itunes.apple.com/us/rss/toppodcasts/limit=40/json".into(),
    };
    let value = http_json(&url)?;
    let ids = chart_ids(&value);
    if ids.is_empty() {
        bail!("Apple Podcasts returned no shows");
    }
    apple_lookup(&ids)
}

fn chart_ids(value: &Value) -> Vec<String> {
    let entries = match value.pointer("/feed/entry") {
        Some(Value::Array(items)) => items.clone(),
        Some(one @ Value::Object(_)) => vec![one.clone()],
        _ => Vec::new(),
    };
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .pointer("/id/attributes/im:id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

fn apple_lookup(ids: &[String]) -> Result<Vec<Show>> {
    let mut shows = Vec::new();
    for chunk in ids.chunks(40) {
        let url = format!(
            "https://itunes.apple.com/lookup?id={}&entity=podcast",
            chunk.join(",")
        );
        let value = http_json(&url)?;
        shows.extend(parse_apple_results(&value));
    }
    if shows.is_empty() {
        bail!("Apple Podcasts lookup returned no feeds");
    }
    Ok(shows)
}

fn apple_search(query: &str) -> Result<Vec<Show>> {
    let url = format!(
        "https://itunes.apple.com/search?term={}&media=podcast&entity=podcast&limit=40",
        urlencoding::encode(query)
    );
    let value = http_json(&url)?;
    Ok(parse_apple_results(&value))
}

pub fn parse_apple_results(value: &Value) -> Vec<Show> {
    let Some(results) = value.get("results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    results.iter().filter_map(parse_apple_show).collect()
}

fn parse_apple_show(value: &Value) -> Option<Show> {
    let wrapper = value.get("wrapperType").and_then(|v| v.as_str()).unwrap_or("");
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if wrapper == "podcastEpisode" || kind == "podcast-episode" {
        return None;
    }
    let id_num = value.get("collectionId").and_then(json_id);
    let id_num = id_num.filter(|s| !s.is_empty())?;
    let feed = text_field(value, "feedUrl");
    if !feed.contains("://") {
        return None;
    }
    let title = text_field(value, "collectionName");
    let title = if title.is_empty() {
        text_field(value, "trackName")
    } else {
        title
    };
    if title.is_empty() {
        return None;
    }
    let artwork = nonempty(text_field(value, "artworkUrl600"))
        .or_else(|| nonempty(text_field(value, "artworkUrl100")));
    Some(Show {
        id: format!("apple:{id_num}"),
        title,
        author: text_field(value, "artistName"),
        description: text_field(value, "description"),
        feed_url: feed,
        artwork,
        source: "Apple Podcasts".into(),
    })
}

fn podcast_index_trending(category: Option<&str>) -> Option<Vec<Show>> {
    let (key, secret) = index_credentials()?;
    let mut url = "https://api.podcastindex.org/api/1.0/podcasts/trending?max=20&lang=en".to_string();
    if let Some(category) = category.filter(|c| !c.is_empty()) {
        url.push_str("&cat=");
        url.push_str(&urlencoding::encode(category));
    }
    let value = index_get(&url, &key, &secret).ok()?;
    Some(parse_index_feeds(&value))
}

fn podcast_index_search(query: &str) -> Option<Vec<Show>> {
    let (key, secret) = index_credentials()?;
    let url = format!(
        "https://api.podcastindex.org/api/1.0/search/byterm?q={}&max=20",
        urlencoding::encode(query)
    );
    let value = index_get(&url, &key, &secret).ok()?;
    Some(parse_index_feeds(&value))
}

fn parse_index_feeds(value: &Value) -> Vec<Show> {
    let feeds = value
        .get("feeds")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    feeds.iter().filter_map(parse_index_show).collect()
}

fn parse_index_show(value: &Value) -> Option<Show> {
    let id = value.get("id").and_then(json_id).filter(|s| !s.is_empty())?;
    let feed = text_field(value, "url");
    if !feed.contains("://") {
        return None;
    }
    let title = text_field(value, "title");
    if title.is_empty() {
        return None;
    }
    Some(Show {
        id: format!("pindex:{id}"),
        title,
        author: text_field(value, "author"),
        description: text_field(value, "description"),
        feed_url: feed,
        artwork: nonempty(text_field(value, "artwork"))
            .or_else(|| nonempty(text_field(value, "image"))),
        source: "Podcast Index".into(),
    })
}

fn merge_shows(primary: Vec<Show>, extra: Vec<Show>) -> Vec<Show> {
    let mut out = primary;
    for show in extra {
        let key = feed_key(&show.feed_url);
        if out.iter().any(|have| feed_key(&have.feed_url) == key) {
            if let Some(existing) = out.iter_mut().find(|have| feed_key(&have.feed_url) == key) {
                if existing.description.trim().is_empty() && !show.description.trim().is_empty() {
                    existing.description = show.description;
                }
                if existing.author.trim().is_empty() {
                    existing.author = show.author;
                }
            }
            continue;
        }
        out.push(show);
    }
    out
}

fn feed_key(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn index_credentials() -> Option<(String, String)> {
    if let (Ok(key), Ok(secret)) = (
        std::env::var("PODCAST_INDEX_KEY"),
        std::env::var("PODCAST_INDEX_SECRET"),
    ) {
        let key = key.trim().to_string();
        let secret = secret.trim().to_string();
        if !key.is_empty() && !secret.is_empty() {
            return Some((key, secret));
        }
    }
    let text = fs::read_to_string(index_config_path()).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let key = value
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let secret = value
        .get("api_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if key.is_empty() || secret.is_empty() {
        None
    } else {
        Some((key, secret))
    }
}

fn index_get(url: &str, key: &str, secret: &str) -> Result<Value> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    let mut hasher = Sha1::new();
    hasher.update(format!("{key}{secret}{now}").as_bytes());
    let auth = hex::encode(hasher.finalize());
    let value = http()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("X-Auth-Key", key)
        .set("X-Auth-Date", &now)
        .set("Authorization", &auth)
        .set("Accept", "application/json")
        .call()
        .context("podcast index")?
        .into_json()?;
    Ok(value)
}

fn http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .timeout_connect(Duration::from_secs(5))
        .user_agent(USER_AGENT)
        .build()
}

fn http_json(url: &str) -> Result<Value> {
    Ok(http()
        .get(url)
        .set("Accept", "application/json")
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?)
}

fn http_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let resp = http()
        .get(url)
        .set("Accept", "application/rss+xml, application/xml, text/xml, */*")
        .call()
        .with_context(|| format!("GET feed {url}"))?;
    let mut bytes = Vec::new();
    resp.into_reader().take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Fetch a podcast feed and, if truncated by `limit`, cut on the last complete
/// `</item>` and close the RSS document so the XML parser still succeeds.
fn http_feed_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = http_bytes(url, limit)?;
    if (bytes.len() as u64) < limit {
        return Ok(bytes);
    }
    let needle = b"</item>";
    if let Some(pos) = bytes
        .windows(needle.len())
        .rposition(|w| w == needle)
        .map(|p| p + needle.len())
    {
        bytes.truncate(pos);
        if !bytes.windows(b"</channel>".len()).any(|w| w == b"</channel>") {
            bytes.extend_from_slice(b"</channel>");
        }
        if !bytes.windows(b"</rss>".len()).any(|w| w == b"</rss>") {
            bytes.extend_from_slice(b"</rss>");
        }
        tracing::info!(
            %url,
            bytes = bytes.len(),
            "truncated oversized podcast feed at last </item>"
        );
        return Ok(bytes);
    }
    bail!(
        "podcast feed is larger than {} bytes and could not be truncated safely",
        limit
    );
}

fn json_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| value.as_i64().map(|n| n.to_string()))
        .or_else(|| value.as_u64().map(|n| n.to_string()))
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn stable_id(value: &str) -> String {
    let digest = md5::Md5::digest(value.as_bytes());
    hex::encode(digest)
}

pub fn parse_duration(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(secs) = value.parse::<u64>() {
        return Some(secs);
    }
    let parts: Vec<&str> = value.split(':').collect();
    let nums: Option<Vec<u64>> = parts.iter().map(|p| p.parse().ok()).collect();
    let nums = nums?;
    Some(match nums.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    })
}

fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{} h {} min", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{} min", secs / 60)
    } else {
        format!("{secs} sec")
    }
}

fn short_date(value: &str) -> Option<String> {
    let value = value.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt.format("%Y-%m-%d").to_string());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(value) {
        return Some(dt.format("%Y-%m-%d").to_string());
    }
    if value.len() >= 10 && value.as_bytes().get(4) == Some(&b'-') {
        return Some(value[..10].to_string());
    }
    None
}

pub fn strip_html(value: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
}

fn favorites_path() -> PathBuf {
    config_dir().join("podcast-favorites.json")
}

fn pending_path() -> PathBuf {
    config_dir().join("podcast-subscribe.json")
}

fn progress_path() -> PathBuf {
    config_dir().join("podcast-progress.json")
}

fn index_config_path() -> PathBuf {
    config_dir().join("podcast-index.json")
}

#[derive(Serialize, Deserialize, Default)]
struct FavoriteFile {
    #[serde(default)]
    shows: Vec<Show>,
}

#[derive(Serialize, Deserialize, Default)]
struct PendingFile {
    #[serde(default)]
    pending: Vec<PendingSub>,
}

fn load_pending() -> Vec<PendingSub> {
    let Ok(text) = fs::read_to_string(pending_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<PendingFile>(&text)
        .map(|file| file.pending)
        .unwrap_or_default()
}

fn save_pending(pending: &[PendingSub]) -> Result<()> {
    write_json(&pending_path(), &PendingFile { pending: pending.to_vec() })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_result_keeps_feed_and_skips_episodes() {
        let value = serde_json::json!({
            "results": [
                {
                    "wrapperType": "track",
                    "kind": "podcast",
                    "collectionId": 123,
                    "collectionName": "Daily News",
                    "artistName": "Ada",
                    "feedUrl": "https://example.com/feed.xml",
                    "artworkUrl600": "https://example.com/art.jpg",
                    "description": "World news <b>today</b>"
                },
                {
                    "wrapperType": "podcastEpisode",
                    "kind": "podcast-episode",
                    "collectionId": 123,
                    "trackName": "One episode"
                }
            ]
        });
        let shows = parse_apple_results(&value);
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].id, "apple:123");
        assert_eq!(shows[0].feed_url, "https://example.com/feed.xml");
        assert!(shows[0].subtitle().contains("Ada"));
        assert!(shows[0].subtitle().contains("World news today"));
    }

    #[test]
    fn feed_episodes_are_newest_first() {
        let xml = br#"<?xml version="1.0"?>
        <rss version="2.0">
          <channel>
            <title>Show</title>
            <item>
              <title>Older</title>
              <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
              <enclosure url="https://example.com/old.mp3" type="audio/mpeg"/>
              <itunes:duration xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">10:00</itunes:duration>
            </item>
            <item>
              <title>Newer</title>
              <guid>ep-2</guid>
              <pubDate>Wed, 01 Jan 2025 00:00:00 GMT</pubDate>
              <enclosure url="https://example.com/new.mp3" type="audio/mpeg"/>
              <itunes:duration xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">1:02:03</itunes:duration>
            </item>
          </channel>
        </rss>"#;
        let channel = rss::Channel::read_from(Cursor::new(&xml[..])).unwrap();
        assert_eq!(channel.items().len(), 2);
        let show = Show {
            id: "apple:1".into(),
            title: "Show".into(),
            feed_url: "https://example.com/feed.xml".into(),
            ..Default::default()
        };
        // Parse through the same item walk as episodes_for_feed.
        let mut episodes = Vec::new();
        for item in channel.items() {
            episodes.push(Episode {
                id: item.guid().map(|g| g.value().to_string()).unwrap_or_else(|| "x".into()),
                title: item.title().unwrap_or("").into(),
                published: item.pub_date().map(|s| s.to_string()),
                stream_url: item.enclosure().unwrap().url().into(),
                show_id: show.id.clone(),
                ..Default::default()
            });
        }
        sort_newest(&mut episodes);
        assert_eq!(episodes[0].title, "Newer");
        assert_eq!(parse_duration("10:00"), Some(600));
        assert_eq!(parse_duration("1:02:03"), Some(3723));
        let track = episodes[0].to_track(&show);
        assert!(track.id.starts_with("pc:"));
        assert!(is_podcast_track(&track));
    }

    #[test]
    fn live_apple_podcasts_when_requested() {
        if std::env::var_os("MDE_MUSIC_LIVE").is_none() {
            return;
        }
        let top = trending().expect("trending");
        assert!(top.len() >= 5, "expected trending shows, got {}", top.len());
        assert!(
            top.iter().any(|show| show.feed_url.contains("://")),
            "trending shows should include a feed"
        );
        let news = by_genre("News").expect("news");
        assert!(!news.is_empty(), "news chart was empty");
        let found = search("NPR").expect("search");
        assert!(
            found.iter().any(|show| show.title.to_ascii_lowercase().contains("npr")
                || show.author.to_ascii_lowercase().contains("npr")),
            "NPR search returned {:?}",
            found.iter().take(3).map(|show| &show.title).collect::<Vec<_>>()
        );
        let show = top
            .iter()
            .find(|show| show.feed_url.contains("://"))
            .unwrap();
        let episodes = episodes_for_feed(show).expect("episodes");
        assert!(episodes.len() >= 1, "feed had no episodes");
        assert!(episodes[0].stream_url.contains("://"));
    }

    #[test]
    fn favorites_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("podcast-favorites.json");
        let show = Show {
            id: "apple:9".into(),
            title: "Test".into(),
            author: "Host".into(),
            feed_url: "https://example.com/feed.xml".into(),
            source: "Apple Podcasts".into(),
            ..Default::default()
        };
        save_favorites_to(&path, &[show.clone()]).unwrap();
        assert_eq!(load_favorites_from(&path), vec![show]);
    }
}
