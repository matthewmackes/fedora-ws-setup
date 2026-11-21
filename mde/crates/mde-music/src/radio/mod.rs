//! Internet radio discovery via [radio-browser.info](https://www.radio-browser.info/).
//!
//! Favorites are stored on this machine and are not sent to the media server.

use crate::model::{RadioStation, Track};
use anyhow::{bail, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PAGE: u32 = 40;

const USER_AGENT: &str = "NeighbourhoodMusic/2026 (MDE; +https://www.radio-browser.info/)";

const MIRRORS: &[&str] = &[
    "https://all.api.radio-browser.info",
    "https://de1.api.radio-browser.info",
    "https://nl1.api.radio-browser.info",
    "https://at1.api.radio-browser.info",
];

static PREFERRED: Mutex<Option<String>> = Mutex::new(None);
static PENDING_PLAY: Mutex<Option<(String, String)>> = Mutex::new(None);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub uuid: String,
    pub name: String,
    pub stream_url: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bitrate: u32,
    #[serde(default)]
    pub votes: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub name: String,
    pub country: String,
    pub tag: String,
    pub offset: u32,
    pub limit: u32,
}

impl Station {
    pub fn from_server(station: &RadioStation) -> Option<Self> {
        let stream_url = station.stream_url.trim();
        if stream_url.is_empty() {
            return None;
        }
        let id = station.id.trim();
        let uuid = if id.is_empty() {
            format!("server:{stream_url}")
        } else {
            format!("server:{id}")
        };
        let name = station.name.trim();
        Some(Self {
            uuid,
            name: if name.is_empty() {
                "Station".into()
            } else {
                name.to_string()
            },
            stream_url: stream_url.to_string(),
            homepage: station.homepage.as_ref().and_then(|h| {
                let h = h.trim();
                if h.is_empty() {
                    None
                } else {
                    Some(h.to_string())
                }
            }),
            ..Default::default()
        })
    }

    pub fn track_id(&self) -> String {
        if let Some(id) = self.uuid.strip_prefix("server:") {
            format!("srv:{id}")
        } else {
            format!("rb:{}", self.uuid)
        }
    }

    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.country.trim().is_empty() {
            parts.push(self.country.trim().to_string());
        }
        if !self.language.trim().is_empty() {
            parts.push(self.language.trim().to_string());
        }
        if !self.codec.trim().is_empty() {
            if self.bitrate > 0 {
                parts.push(format!("{} {} kbps", self.codec.trim(), self.bitrate));
            } else {
                parts.push(self.codec.trim().to_string());
            }
        } else if self.bitrate > 0 {
            parts.push(format!("{} kbps", self.bitrate));
        }
        if !self.tags.trim().is_empty() {
            parts.push(clip(self.tags.trim(), 48));
        }
        if parts.is_empty() {
            if self.uuid.starts_with("server:") {
                "On this server".into()
            } else {
                "Internet radio".into()
            }
        } else {
            parts.join(" · ")
        }
    }

    pub fn to_track(&self) -> Track {
        Track {
            id: self.track_id(),
            title: self.name.clone(),
            artist: "Radio".into(),
            album: self.subtitle(),
            stream_url: self.stream_url.clone(),
            cover_url: raster_cover(self.favicon.as_deref()),
            ..Default::default()
        }
    }
}

pub fn search_body(query: &Query) -> Value {
    let limit = if query.limit == 0 {
        PAGE
    } else {
        query.limit.min(100)
    };
    let mut body = serde_json::json!({
        "hidebroken": true,
        "order": "votes",
        "reverse": true,
        "offset": query.offset,
        "limit": limit,
    });
    let name = query.name.trim();
    if !name.is_empty() {
        body["name"] = Value::String(name.to_string());
    }
    let country = query.country.trim();
    if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
        body["countrycode"] = Value::String(country.to_ascii_uppercase());
    } else if !country.is_empty() {
        body["country"] = Value::String(country.to_string());
    }
    let tag = query.tag.trim();
    if !tag.is_empty() {
        body["tag"] = Value::String(tag.to_string());
    }
    body
}

pub fn search(query: &Query) -> Result<Vec<Station>> {
    let body = search_body(query);
    let value = post_json("/json/stations/search", &body)?;
    if let Some(msg) = value.get("error").and_then(|v| v.as_str()) {
        bail!("{msg}");
    }
    if !value.is_array() {
        bail!("unexpected radio-browser response");
    }
    Ok(parse_stations(&value))
}

/// Ask radio-browser for the URL it wants players to open, and count the click.
pub fn register_click(uuid: &str) -> Option<String> {
    let uuid = uuid.trim();
    if uuid.is_empty() || uuid.starts_with("server:") {
        return None;
    }
    for base in mirrors() {
        let url = format!("{base}/json/url/{}", urlencoding::encode(uuid));
        let Ok(resp) = http().get(&url).call() else {
            continue;
        };
        let Ok(value) = resp.into_json::<Value>() else {
            continue;
        };
        remember_base(base);
        let play = value.get("url").and_then(|u| u.as_str()).unwrap_or("").trim();
        if !play.is_empty() {
            return Some(play.to_string());
        }
    }
    None
}

/// Resolve the play URL in the background. The UI applies it if that station is still current.
pub fn note_click(track_id: &str, uuid: &str) {
    let track_id = track_id.to_string();
    let uuid = uuid.to_string();
    std::thread::spawn(move || {
        if let Some(url) = register_click(&uuid) {
            *PENDING_PLAY.lock() = Some((track_id, url));
        }
    });
}

pub fn take_click_url() -> Option<(String, String)> {
    PENDING_PLAY.lock().take()
}

pub fn parse_stations(value: &Value) -> Vec<Station> {
    value
        .as_array()
        .map(|arr| arr.iter().filter_map(parse_station).collect())
        .unwrap_or_default()
}

pub fn favorites_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("radio-favorites.json")
}

pub fn load_favorites() -> Vec<Station> {
    load_favorites_from(&favorites_path())
}

pub fn load_favorites_from(path: &Path) -> Vec<Station> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<FavoriteFile>(&text) {
        Ok(file) => file
            .stations
            .into_iter()
            .filter(|s| !s.uuid.trim().is_empty() && s.stream_url.contains("://"))
            .collect(),
        Err(err) => {
            tracing::warn!("radio favorites: {err}");
            Vec::new()
        }
    }
}

pub fn save_favorites(stations: &[Station]) -> Result<()> {
    save_favorites_to(&favorites_path(), stations)
}

pub fn save_favorites_to(path: &Path, stations: &[Station]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = FavoriteFile {
        stations: stations.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&file)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Default)]
struct FavoriteFile {
    #[serde(default)]
    stations: Vec<Station>,
}

fn parse_station(value: &Value) -> Option<Station> {
    if value.get("lastcheckok").and_then(|v| v.as_i64()) == Some(0) {
        return None;
    }
    let uuid = text(value.get("stationuuid"));
    if uuid.is_empty() {
        return None;
    }
    let resolved = text(value.get("url_resolved"));
    let raw = text(value.get("url"));
    let stream_url = if !resolved.is_empty() { resolved } else { raw };
    if !stream_url.contains("://") {
        return None;
    }
    let name = text(value.get("name"));
    let homepage = nonempty(text(value.get("homepage")));
    let favicon = nonempty(text(value.get("favicon")));
    Some(Station {
        uuid,
        name: if name.is_empty() {
            "Untitled station".into()
        } else {
            name
        },
        stream_url,
        homepage,
        favicon,
        tags: text(value.get("tags")),
        country: text(value.get("country")),
        language: text(value.get("language")),
        codec: text(value.get("codec")),
        bitrate: number_u32(value.get("bitrate")),
        votes: number_i64(value.get("votes")),
    })
}

fn text(value: Option<&Value>) -> String {
    value
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

fn number_u32(value: Option<&Value>) -> u32 {
    match value {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn number_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().unwrap_or_else(|| n.as_u64().unwrap_or(0) as i64),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn raster_cover(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    if lower.contains(".svg") {
        return None;
    }
    Some(url.to_string())
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .timeout_connect(Duration::from_secs(5))
        .user_agent(USER_AGENT)
        .build()
}

fn mirrors() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(saved) = PREFERRED.lock().clone() {
        out.push(saved);
    }
    for mirror in MIRRORS {
        if !out.iter().any(|have| have == mirror) {
            out.push((*mirror).to_string());
        }
    }
    out
}

fn remember_base(base: String) {
    *PREFERRED.lock() = Some(base);
}

fn post_json(path: &str, body: &Value) -> Result<Value> {
    let payload = serde_json::to_vec(body)?;
    let mut last = String::from("radio-browser request failed");
    for base in mirrors() {
        let url = format!("{base}{path}");
        match http()
            .post(&url)
            .set("User-Agent", USER_AGENT)
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .send_bytes(&payload)
        {
            Ok(resp) => {
                remember_base(base);
                return Ok(resp.into_json()?);
            }
            Err(err) => {
                last = err.to_string();
                tracing::debug!("radio-browser {url}: {err}");
            }
        }
    }
    bail!(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_body_maps_country_code_name_and_tag() {
        let body = search_body(&Query {
            name: " bbc ".into(),
            country: "de".into(),
            tag: " jazz ".into(),
            offset: 40,
            limit: 40,
        });
        assert_eq!(body["name"], "bbc");
        assert_eq!(body["countrycode"], "DE");
        assert!(body.get("country").is_none());
        assert_eq!(body["tag"], "jazz");
        assert_eq!(body["hidebroken"], true);
        assert_eq!(body["order"], "votes");
        assert_eq!(body["reverse"], true);
        assert_eq!(body["offset"], 40);

        let named = search_body(&Query {
            country: "Germany".into(),
            ..Default::default()
        });
        assert_eq!(named["country"], "Germany");
        assert!(named.get("countrycode").is_none());
    }

    #[test]
    fn parse_prefers_resolved_url_and_skips_broken() {
        let value = serde_json::json!([
            {
                "stationuuid": "abc",
                "name": "Test FM",
                "url": "http://example/raw",
                "url_resolved": "https://example/live",
                "homepage": "https://example",
                "favicon": "https://example/logo.svg",
                "tags": "jazz,talk",
                "country": "Germany",
                "language": "german",
                "codec": "MP3",
                "bitrate": 128,
                "votes": 12,
                "lastcheckok": 1
            },
            {
                "stationuuid": "dead",
                "name": "Down",
                "url": "http://example/down",
                "lastcheckok": 0
            },
            {
                "stationuuid": "nourl",
                "name": "Silent",
                "url": ""
            }
        ]);
        let stations = parse_stations(&value);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].stream_url, "https://example/live");
        assert_eq!(stations[0].bitrate, 128);
        assert_eq!(stations[0].track_id(), "rb:abc");
        assert!(stations[0].to_track().cover_url.is_none());
        assert!(stations[0].subtitle().contains("Germany"));
        assert!(stations[0].subtitle().contains("128 kbps"));
    }

    #[test]
    fn server_station_ids_and_favorites_roundtrip() {
        let server = RadioStation {
            id: "9".into(),
            name: "Local".into(),
            stream_url: "http://10.0.0.8/stream".into(),
            homepage: Some("  ".into()),
        };
        let station = Station::from_server(&server).unwrap();
        assert_eq!(station.track_id(), "srv:9");
        assert_eq!(station.subtitle(), "On this server");
        assert!(station.homepage.is_none());
        assert!(Station::from_server(&RadioStation {
            stream_url: "  ".into(),
            ..Default::default()
        })
        .is_none());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("radio-favorites.json");
        save_favorites_to(&path, &[station.clone()]).unwrap();
        let loaded = load_favorites_from(&path);
        assert_eq!(loaded, vec![station]);

        let junk = dir.path().join("junk.json");
        fs::write(&junk, "{\"stations\":[{\"uuid\":\"x\",\"name\":\"n\",\"stream_url\":\"not-a-url\"}]}").unwrap();
        assert!(load_favorites_from(&junk).is_empty());
    }

    #[test]
    fn live_radio_browser_when_requested() {
        if std::env::var_os("MDE_MUSIC_LIVE").is_none() {
            return;
        }
        let top = search(&Query {
            limit: 5,
            ..Default::default()
        })
        .expect("top stations");
        assert!(top.len() >= 1, "expected stations, got {}", top.len());
        assert!(top[0].stream_url.contains("://"));
        let jazz = search(&Query {
            tag: "jazz".into(),
            limit: 3,
            ..Default::default()
        })
        .expect("jazz");
        assert!(!jazz.is_empty());
    }
}
