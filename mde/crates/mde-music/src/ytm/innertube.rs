//! Anonymous Innertube client for YouTube Music.
//! Client identity rotation + declarative repair overrides.

use crate::model::*;
use crate::ytm::repair;
use crate::ytm::stream::ResolvedStream;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::sync::Mutex;

const INNERTUBE_BASE: &str = "https://music.youtube.com/youtubei/v1";
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct ClientIdentity {
    pub name: String,
    pub client_name: String,
    pub client_version: String,
    pub api_key: String,
}

fn builtin_clients() -> Vec<ClientIdentity> {
    vec![
        ClientIdentity {
            name: "WEB_REMIX".into(),
            client_name: "WEB_REMIX".into(),
            client_version: "1.20241202.01.00".into(),
            api_key: "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30".into(),
        },
        ClientIdentity {
            name: "ANDROID_MUSIC".into(),
            client_name: "ANDROID_MUSIC".into(),
            client_version: "7.27.52".into(),
            api_key: "AIzaSyAOghZGza2MQSZkY_zfZ370N-PUdXEo8AI".into(),
        },
        ClientIdentity {
            name: "IOS".into(),
            client_name: "IOS".into(),
            client_version: "20.10.4".into(),
            api_key: "AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUIn".into(),
        },
    ]
}

pub struct InnertubeClient {
    clients: Mutex<Vec<ClientIdentity>>,
    index: Mutex<usize>,
}

impl Default for InnertubeClient {
    fn default() -> Self {
        Self {
            clients: Mutex::new(builtin_clients()),
            index: Mutex::new(0),
        }
    }
}

impl Clone for InnertubeClient {
    fn clone(&self) -> Self {
        Self {
            clients: Mutex::new(self.clients.lock().unwrap().clone()),
            index: Mutex::new(*self.index.lock().unwrap()),
        }
    }
}

impl InnertubeClient {
    pub fn load_repair_overrides(&mut self) {
        if let Some(pack) = repair::load_active_rules() {
            let mut clients = self.clients.lock().unwrap();
            if !pack.clients.is_empty() {
                *clients = pack
                    .clients
                    .into_iter()
                    .map(|c| ClientIdentity {
                        name: c.name,
                        client_name: c.client_name,
                        client_version: c.client_version,
                        api_key: c.api_key,
                    })
                    .collect();
                *self.index.lock().unwrap() = 0;
                tracing::info!("loaded {} repair client identities", clients.len());
            }
        }
    }

    pub fn rotate_client(&self) {
        let mut idx = self.index.lock().unwrap();
        let n = self.clients.lock().unwrap().len().max(1);
        *idx = (*idx + 1) % n;
        tracing::info!(index = *idx, "rotated YTM client identity");
    }

    fn current(&self) -> ClientIdentity {
        let clients = self.clients.lock().unwrap();
        let idx = *self.index.lock().unwrap();
        clients[idx % clients.len()].clone()
    }

    fn context(&self) -> Value {
        let c = self.current();
        json!({
            "client": {
                "clientName": c.client_name,
                "clientVersion": c.client_version,
                "hl": "en",
                "gl": "US",
                "userAgent": USER_AGENT,
            }
        })
    }

    fn post(&self, endpoint: &str, body: Value) -> Result<Value> {
        let c = self.current();
        let url = format!("{INNERTUBE_BASE}/{endpoint}?prettyPrint=false&key={}", c.api_key);
        let mut payload = body;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("context".into(), self.context());
        }
        let resp = ureq::post(&url)
            .set("User-Agent", USER_AGENT)
            .set("Content-Type", "application/json")
            .set("X-YouTube-Client-Name", "67")
            .set("Origin", "https://music.youtube.com")
            .set("Referer", "https://music.youtube.com/")
            .send_json(payload)
            .with_context(|| format!("innertube POST {endpoint}"))?;
        resp.into_json()
            .with_context(|| format!("innertube JSON {endpoint}"))
    }

    pub fn search(&self, query: &str) -> Result<SearchResults> {
        let body = json!({
            "query": query,
            "params": "EgWKAQIIAWoKEAMQBBAJEAoQBQ%3D%3D"
        });
        let v = self.post("search", body)?;
        Ok(parse_search(&v))
    }

    pub fn home_shelves(&self) -> Result<Vec<HomeShelf>> {
        let v = self.post("browse", json!({ "browseId": "FEmusic_home" }))?;
        let mut shelves = parse_shelves(&v, "Home");
        shelves.extend(self.charts_and_moods().unwrap_or_default());
        Ok(shelves)
    }

    pub fn charts_and_moods(&self) -> Result<Vec<HomeShelf>> {
        let charts = self
            .post("browse", json!({ "browseId": "FEmusic_charts" }))
            .map(|v| parse_shelves(&v, "Charts"))
            .unwrap_or_default();
        let moods = self
            .post("browse", json!({ "browseId": "FEmusic_moods_and_genres" }))
            .map(|v| parse_shelves(&v, "Moods & genres"))
            .unwrap_or_default();
        let mut out = charts;
        out.extend(moods);
        Ok(out)
    }

    pub fn album_tracks(&self, browse_id: &str) -> Result<Vec<Track>> {
        let id = browse_id.strip_prefix("ytm:").unwrap_or(browse_id);
        let v = self.post("browse", json!({ "browseId": id }))?;
        Ok(parse_album_tracks(&v, id))
    }

    pub fn artist_albums(&self, browse_id: &str) -> Result<Vec<Album>> {
        let id = browse_id.strip_prefix("ytm:").unwrap_or(browse_id);
        let v = self.post("browse", json!({ "browseId": id }))?;
        Ok(parse_albums_from_browse(&v))
    }

    pub fn related(&self, video_id: &str) -> Result<Vec<Track>> {
        let id = video_id.strip_prefix("ytm:").unwrap_or(video_id);
        let v = self.post(
            "next",
            json!({
                "videoId": id,
                "isAudioOnly": true,
            }),
        )?;
        Ok(parse_related(&v))
    }

    pub fn resolve_audio(&self, video_id: &str) -> Result<ResolvedStream> {
        let id = video_id.strip_prefix("ytm:").unwrap_or(video_id);
        let v = self.post(
            "player",
            json!({
                "videoId": id,
                "contentCheckOk": true,
                "racyCheckOk": true,
            }),
        )?;
        parse_player_stream(&v, id)
    }
}

fn walk_runs(v: &Value) -> String {
    v.pointer("/runs")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .or_else(|| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
        .unwrap_or_default()
}

fn thumb_url(v: &Value) -> Option<String> {
    v.pointer("/thumbnails")
        .or_else(|| v.pointer("/thumbnail/thumbnails"))
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .map(str::to_string)
}

fn parse_search(v: &Value) -> SearchResults {
    let mut out = SearchResults::default();
    collect_music_items(v, &mut out.tracks, &mut out.albums, &mut out.artists);
    out
}

fn parse_shelves(v: &Value, fallback_title: &str) -> Vec<HomeShelf> {
    let mut shelves = Vec::new();
    let mut tracks = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    collect_music_items(v, &mut tracks, &mut albums, &mut artists);
    if !tracks.is_empty() || !albums.is_empty() {
        shelves.push(HomeShelf {
            id: format!("ytm-{}", fallback_title.to_lowercase().replace(' ', "-")),
            title: fallback_title.into(),
            albums,
            tracks,
            playlists: Vec::new(),
        });
    }
    shelves
}

fn parse_album_tracks(v: &Value, album_id: &str) -> Vec<Track> {
    let mut tracks = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    collect_music_items(v, &mut tracks, &mut albums, &mut artists);
    for (i, t) in tracks.iter_mut().enumerate() {
        t.album_id = Some(format!("ytm:{album_id}"));
        if t.track_num.is_none() {
            t.track_num = Some((i + 1) as u32);
        }
    }
    tracks
}

fn parse_albums_from_browse(v: &Value) -> Vec<Album> {
    let mut tracks = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    collect_music_items(v, &mut tracks, &mut albums, &mut artists);
    albums
}

fn parse_related(v: &Value) -> Vec<Track> {
    let mut tracks = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    collect_music_items(v, &mut tracks, &mut albums, &mut artists);
    tracks
}

fn collect_music_items(
    v: &Value,
    tracks: &mut Vec<Track>,
    albums: &mut Vec<Album>,
    artists: &mut Vec<Artist>,
) {
    match v {
        Value::Object(map) => {
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                push_list_item(renderer, tracks, albums, artists);
            }
            if let Some(renderer) = map.get("musicTwoRowItemRenderer") {
                push_two_row(renderer, tracks, albums, artists);
            }
            if let Some(renderer) = map.get("playlistPanelVideoRenderer") {
                if let Some(t) = track_from_playlist_panel(renderer) {
                    tracks.push(t);
                }
            }
            for val in map.values() {
                collect_music_items(val, tracks, albums, artists);
            }
        }
        Value::Array(arr) => {
            for val in arr {
                collect_music_items(val, tracks, albums, artists);
            }
        }
        _ => {}
    }
}

fn push_list_item(
    r: &Value,
    tracks: &mut Vec<Track>,
    albums: &mut Vec<Album>,
    artists: &mut Vec<Artist>,
) {
    let video_id = r
        .pointer("/playlistItemData/videoId")
        .or_else(|| r.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(|x| x.as_str());
    let browse = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(|x| x.as_str());
    let title = r
        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")
        .map(walk_runs)
        .unwrap_or_default();
    let subtitle = r
        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text")
        .map(walk_runs)
        .unwrap_or_default();
    let cover = thumb_url(r.pointer("/thumbnail/musicThumbnailRenderer/thumbnail").unwrap_or(r));
    if let Some(vid) = video_id {
        if tracks.iter().any(|t| t.id == format!("ytm:{vid}")) {
            return;
        }
        let artist = subtitle.split('•').next().unwrap_or(&subtitle).trim().to_string();
        tracks.push(Track {
            id: format!("ytm:{vid}"),
            title,
            artist,
            album: String::new(),
            stream_url: String::new(),
            cover_url: cover,
            source: SourceKind::YoutubeMusic,
            format: Some("ytm".into()),
            ..Default::default()
        });
        return;
    }
    if let Some(bid) = browse {
        let page_type = r
            .pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if page_type.contains("ARTIST") {
            if artists.iter().any(|a| a.id == format!("ytm:{bid}")) {
                return;
            }
            artists.push(Artist {
                id: format!("ytm:{bid}"),
                name: title,
                cover_url: cover,
                album_count: None,
            });
        } else {
            if albums.iter().any(|a| a.id == format!("ytm:{bid}")) {
                return;
            }
            let artist = subtitle.split('•').next().unwrap_or("").trim().to_string();
            albums.push(Album {
                id: format!("ytm:{bid}"),
                name: title,
                artist,
                cover_url: cover,
                source: SourceKind::YoutubeMusic,
                ..Default::default()
            });
        }
    }
}

fn push_two_row(
    r: &Value,
    tracks: &mut Vec<Track>,
    albums: &mut Vec<Album>,
    artists: &mut Vec<Artist>,
) {
    let title = r.pointer("/title").map(walk_runs).unwrap_or_default();
    let subtitle = r.pointer("/subtitle").map(walk_runs).unwrap_or_default();
    let cover = thumb_url(r.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail").unwrap_or(r));
    if let Some(vid) = r
        .pointer("/navigationEndpoint/watchEndpoint/videoId")
        .and_then(|x| x.as_str())
    {
        if tracks.iter().any(|t| t.id == format!("ytm:{vid}")) {
            return;
        }
        tracks.push(Track {
            id: format!("ytm:{vid}"),
            title,
            artist: subtitle,
            stream_url: String::new(),
            cover_url: cover,
            source: SourceKind::YoutubeMusic,
            format: Some("ytm".into()),
            ..Default::default()
        });
        return;
    }
    if let Some(bid) = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(|x| x.as_str())
    {
        let page_type = r
            .pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if page_type.contains("ARTIST") {
            artists.push(Artist {
                id: format!("ytm:{bid}"),
                name: title,
                cover_url: cover,
                album_count: None,
            });
        } else {
            albums.push(Album {
                id: format!("ytm:{bid}"),
                name: title,
                artist: subtitle,
                cover_url: cover,
                source: SourceKind::YoutubeMusic,
                ..Default::default()
            });
        }
    }
}

fn track_from_playlist_panel(r: &Value) -> Option<Track> {
    let vid = r.get("videoId")?.as_str()?;
    let title = r.pointer("/title").map(walk_runs).unwrap_or_default();
    let artist = r
        .pointer("/longBylineText")
        .or_else(|| r.pointer("/shortBylineText"))
        .map(walk_runs)
        .unwrap_or_default();
    Some(Track {
        id: format!("ytm:{vid}"),
        title,
        artist,
        stream_url: String::new(),
        cover_url: thumb_url(r),
        source: SourceKind::YoutubeMusic,
        format: Some("ytm".into()),
        ..Default::default()
    })
}

fn parse_player_stream(v: &Value, video_id: &str) -> Result<ResolvedStream> {
    let formats = v
        .pointer("/streamingData/adaptiveFormats")
        .or_else(|| v.pointer("/streamingData/formats"))
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut best: Option<(u32, String, String, Option<u32>)> = None;
    for f in &formats {
        let mime = f.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
        if !mime.starts_with("audio/") {
            continue;
        }
        let br = f
            .get("bitrate")
            .and_then(|b| b.as_u64())
            .unwrap_or(0) as u32;
        let url = f
            .get("url")
            .and_then(|u| u.as_str())
            .map(str::to_string);
        let Some(url) = url else { continue };
        let fmt = mime
            .split(';')
            .next()
            .unwrap_or("audio")
            .replace("audio/", "");
        let sample_rate = f
            .get("audioSampleRate")
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse::<u32>().ok())
            .or_else(|| f.get("audioSampleRate").and_then(|s| s.as_u64()).map(|n| n as u32));
        if best.as_ref().map(|(b, _, _, _)| br > *b).unwrap_or(true) {
            best = Some((br, url, fmt, sample_rate));
        }
    }
    let video_url = formats
        .iter()
        .filter_map(|f| {
            let mime = f.get("mimeType")?.as_str()?;
            if !mime.starts_with("video/") {
                return None;
            }
            f.get("url")?.as_str().map(str::to_string)
        })
        .max_by_key(|u| u.len());
    let Some((bitrate, url, format, sample_rate)) = best else {
        anyhow::bail!("no clear audio URL for {video_id} (may need yt-dlp / repair)");
    };
    Ok(ResolvedStream {
        url,
        bitrate: Some(bitrate / 1000),
        format: Some(format),
        sample_rate,
        video_url,
    })
}
