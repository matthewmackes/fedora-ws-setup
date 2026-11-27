use super::MediaBackend;
use crate::model::*;
use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use serde_json::Value;
use uuid::Uuid;

/// Subsonic encodes a one-item list as a JSON object and a longer list as an array.
fn json_vec(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(items)) => items.clone(),
        Some(item @ Value::Object(_)) => vec![item.clone()],
        _ => Vec::new(),
    }
}

fn json_id_value(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| {
        v.as_str()
            .map(|s| s.to_string())
            .or_else(|| v.as_i64().map(|n| n.to_string()))
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    })
}

/// Prefer modern OpenSubsonic, then fall back until the server accepts `v=`.
/// Error 30 = "Server must upgrade" → our advertised version is too new.
const API_VERSION_CANDIDATES: &[&str] = &[
    "1.16.1", "1.16.0", "1.15.0", "1.14.0", "1.13.0", "1.12.0", "1.11.0", "1.10.2",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    /// md5(password + salt) — Subsonic ≥ 1.13.0
    Token,
    /// Cleartext / enc:hex — needed for older servers and some LDAP setups
    Password,
}

pub struct SubsonicBackend {
    name: String,
    base: String,
    user: String,
    password: String,
    client: String,
    api_version: String,
    auth_mode: AuthMode,
}

impl SubsonicBackend {
    pub fn connect(server: &ServerTarget) -> Result<Self> {
        let mut be = Self {
            name: server.name.clone(),
            base: crate::urlfix::normalize_http_url(&server.base_url),
            user: if server.username.is_empty() {
                "admin".into()
            } else {
                server.username.clone()
            },
            password: server.password.clone(),
            client: "NeighbourhoodMusic".into(),
            api_version: API_VERSION_CANDIDATES[0].into(),
            auth_mode: AuthMode::Token,
        };
        be.negotiate()?;
        Ok(be)
    }

    /// `getTopSongs` wants an artist name. Resolve an id when possible.
    fn artist_display_name(&self, artist_id: &str) -> Option<String> {
        if !artist_id.chars().any(|c| c.is_ascii_digit()) && artist_id.contains(' ') {
            return Some(artist_id.to_string());
        }
        let v = self
            .get(
                "getArtist.view",
                &format!("id={}", urlencoding::encode(artist_id)),
            )
            .ok()?;
        v.pointer("/subsonic-response/artist/name")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }

    /// Pick an API version + auth mode the server accepts.
    fn negotiate(&mut self) -> Result<()> {
        let mut last_err = None;
        for &ver in API_VERSION_CANDIDATES {
            self.api_version = ver.to_string();
            // Token auth only exists since 1.13.0; older → password.
            let modes: &[AuthMode] = if Self::version_at_least(ver, "1.13.0") {
                &[AuthMode::Token, AuthMode::Password]
            } else {
                &[AuthMode::Password]
            };
            for &mode in modes {
                self.auth_mode = mode;
                match self.ping_raw() {
                    Ok(v) => {
                        if let Some(server_ver) = v
                            .pointer("/subsonic-response/version")
                            .and_then(|x| x.as_str())
                        {
                            // Advertise at most what the server implements.
                            self.api_version = Self::clamp_to_server(server_ver).to_string();
                        }
                        tracing::info!(
                            api_version = %self.api_version,
                            auth = ?self.auth_mode,
                            "Subsonic protocol negotiated"
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        last_err = Some(e);
                        // 20: client too old for this server — keep trying higher isn't possible
                        // while walking downward; skip remaining auth modes for this ver.
                        if msg.contains("[20]") || msg.contains("Client must upgrade") {
                            break;
                        }
                        // 30: client too new — try older version (outer loop).
                        // 41: token auth rejected — try password mode.
                        continue;
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Subsonic handshake failed")))
    }

    fn clamp_to_server(server_ver: &str) -> &str {
        for &c in API_VERSION_CANDIDATES {
            if Self::version_cmp(c, server_ver) <= 0 {
                return c;
            }
        }
        "1.13.0"
    }

    fn version_at_least(a: &str, b: &str) -> bool {
        Self::version_cmp(a, b) >= 0
    }

    /// Compare major.minor (.patch ignored for Subsonic rules).
    fn version_cmp(a: &str, b: &str) -> i32 {
        let parse = |s: &str| -> (i32, i32) {
            let mut parts = s.split('.');
            let maj = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
            let min = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
            (maj, min)
        };
        let (am, an) = parse(a);
        let (bm, bn) = parse(b);
        match am.cmp(&bm).then(an.cmp(&bn)) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    fn auth_qs(&self) -> String {
        let v = &self.api_version;
        let c = urlencoding::encode(&self.client);
        let u = urlencoding::encode(&self.user);
        match self.auth_mode {
            AuthMode::Token => {
                let salt = &Uuid::new_v4().simple().to_string()[..12];
                let mut hasher = Md5::new();
                hasher.update(format!("{}{salt}", self.password).as_bytes());
                let token = hex::encode(hasher.finalize());
                format!("u={u}&t={token}&s={salt}&v={v}&c={c}&f=json")
            }
            AuthMode::Password => {
                // enc:hex avoids some proxy/filter issues with special chars
                let enc = format!("enc:{}", hex::encode(self.password.as_bytes()));
                format!(
                    "u={u}&p={}&v={v}&c={c}&f=json",
                    urlencoding::encode(&enc)
                )
            }
        }
    }

    fn ping_raw(&self) -> Result<Value> {
        self.get_unchecked("ping.view", "")
    }

    fn get_unchecked(&self, method: &str, extra: &str) -> Result<Value> {
        let url = if extra.is_empty() {
            format!("{}/rest/{method}?{}", self.base, self.auth_qs())
        } else {
            format!("{}/rest/{method}?{}&{extra}", self.base, self.auth_qs())
        };
        let resp = ureq::get(&url).call().with_context(|| format!("GET {method}"))?;
        let v: Value = resp.into_json()?;
        if let Some(status) = v.pointer("/subsonic-response/status").and_then(|s| s.as_str()) {
            if status != "ok" {
                let code = v
                    .pointer("/subsonic-response/error/code")
                    .and_then(|c| c.as_i64())
                    .unwrap_or(-1);
                let msg = v
                    .pointer("/subsonic-response/error/message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("subsonic error");
                bail!("[{code}] {msg}");
            }
        }
        Ok(v)
    }

    fn get(&self, method: &str, extra: &str) -> Result<Value> {
        self.get_unchecked(method, extra)
    }

    fn cover(&self, id: &str) -> Option<String> {
        if id.is_empty() {
            return None;
        }
        Some(crate::urlfix::normalize_http_url(&format!(
            "{}/rest/getCoverArt.view?{}&id={}",
            self.base,
            self.auth_qs(),
            urlencoding::encode(id)
        )))
    }

    fn stream(&self, id: &str, max_bit_rate: Option<u32>) -> String {
        let mut url = format!(
            "{}/rest/stream.view?{}&id={}",
            self.base,
            self.auth_qs(),
            urlencoding::encode(id)
        );
        if let Some(br) = max_bit_rate {
            url.push_str(&format!("&maxBitRate={br}"));
        }
        crate::urlfix::normalize_http_url(&url)
    }

    fn parse_track(&self, t: &Value) -> Track {
        let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Track {
            title: t
                .get("title")
                .or_else(|| t.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Track")
                .to_string(),
            artist: t
                .get("artist")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            artist_id: t
                .get("artistId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            album: t
                .get("album")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            album_id: t
                .get("albumId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            duration_secs: t.get("duration").and_then(|v| v.as_u64()),
            track_num: t.get("track").and_then(|v| v.as_u64()).map(|n| n as u32),
            cover_url: self.cover(t.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
            stream_url: self.stream(&id, None),
            content_type: t
                .get("contentType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bit_rate: t.get("bitRate").and_then(|v| v.as_u64()).map(|n| n as u32),
            sample_rate: t
                .get("samplingRate")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            format: t
                .get("suffix")
                .or_else(|| t.get("contentType"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            liked: t.get("starred").is_some(),
            source: SourceKind::Server,
            video_url: None,
            id,
        }
    }

    fn parse_album(&self, a: &Value) -> Album {
        let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Album {
            name: a
                .get("name")
                .or_else(|| a.get("album"))
                .and_then(|v| v.as_str())
                .unwrap_or("Album")
                .to_string(),
            artist: a
                .get("artist")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            artist_id: a
                .get("artistId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            year: a.get("year").and_then(|v| v.as_i64()).map(|y| y as i32),
            cover_url: self.cover(a.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
            track_count: a.get("songCount").and_then(|v| v.as_u64()).map(|n| n as u32),
            source: SourceKind::Server,
            alt_ids: Vec::new(),
            id,
        }
    }

    fn album_list(&self, list_type: &str, size: u32) -> Result<Vec<Album>> {
        self.album_page(list_type, size, 0)
    }

    fn album_page(&self, list_type: &str, size: u32, offset: u32) -> Result<Vec<Album>> {
        let v = self.get(
            "getAlbumList2.view",
            &format!("type={list_type}&size={size}&offset={offset}"),
        )?;
        Ok(json_vec(v.pointer("/subsonic-response/albumList2/album"))
            .iter()
            .map(|a| self.parse_album(a))
            .collect())
    }
}

impl MediaBackend for SubsonicBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Subsonic
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn ping(&self) -> Result<()> {
        self.get("ping.view", "")?;
        Ok(())
    }

    fn home_shelves(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = Vec::new();
        for (id, title, t) in [
            ("newest", "Recently added", "newest"),
            ("recent", "Recently played", "recent"),
            ("frequent", "Heavy rotation", "frequent"),
            ("random", "Jump back in", "random"),
            ("starred", "Liked albums", "starred"),
        ] {
            if let Ok(albums) = self.album_list(t, 20) {
                if !albums.is_empty() {
                    shelves.push(HomeShelf {
                        id: id.into(),
                        title: title.into(),
                        albums,
                        ..Default::default()
                    });
                }
            }
        }
        Ok(shelves)
    }

    fn search(&self, query: &str) -> Result<SearchResults> {
        let v = self.get(
            "search3.view",
            &format!("query={}&artistCount=25&albumCount=25&songCount=50", urlencoding::encode(query)),
        )?;
        let root = v.pointer("/subsonic-response/searchResult3");
        let artists = json_vec(root.and_then(|r| r.get("artist")))
            .iter()
            .map(|a| {
                let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Artist {
                    name: a.get("name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    cover_url: self.cover(a.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
                    album_count: a.get("albumCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    id,
                }
            })
            .collect();
        let albums = json_vec(root.and_then(|r| r.get("album")))
            .iter()
            .map(|a| self.parse_album(a))
            .collect();
        let tracks = json_vec(root.and_then(|r| r.get("song")))
            .iter()
            .map(|t| self.parse_track(t))
            .collect();
        Ok(SearchResults {
            artists,
            albums,
            tracks,
            playlists: vec![],
        })
    }

    fn artists(&self) -> Result<Vec<Artist>> {
        let v = self.get("getArtists.view", "")?;
        let mut out = Vec::new();
        for idx in json_vec(v.pointer("/subsonic-response/artists/index")) {
            for a in json_vec(idx.get("artist")) {
                let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                out.push(Artist {
                    name: a.get("name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    cover_url: self.cover(a.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
                    album_count: a.get("albumCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    id,
                });
            }
        }
        Ok(out)
    }

    fn albums(&self) -> Result<Vec<Album>> {
        const PAGE: u32 = 500;
        let mut offset = 0u32;
        let mut out = Vec::new();
        loop {
            let page = self.album_page("alphabeticalByName", PAGE, offset)?;
            let n = page.len() as u32;
            out.extend(page);
            if n < PAGE {
                break;
            }
            offset = offset.saturating_add(n);
            if offset > 50_000 {
                break;
            }
        }
        Ok(out)
    }

    fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        let v = self.get("getArtist.view", &format!("id={}", urlencoding::encode(artist_id)))?;
        Ok(json_vec(v.pointer("/subsonic-response/artist/album"))
            .iter()
            .map(|a| self.parse_album(a))
            .collect())
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<Track>> {
        let v = self.get("getAlbum.view", &format!("id={}", urlencoding::encode(album_id)))?;
        Ok(json_vec(v.pointer("/subsonic-response/album/song"))
            .iter()
            .map(|t| self.parse_track(t))
            .collect())
    }

    fn playlists(&self) -> Result<Vec<Playlist>> {
        let v = self.get("getPlaylists.view", "")?;
        let arr = json_vec(v.pointer("/subsonic-response/playlists/playlist"));
        Ok(arr
            .iter()
            .map(|p| {
                let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Playlist {
                    name: p.get("name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    song_count: p.get("songCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    cover_url: self.cover(p.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
                    owner: p
                        .get("owner")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    revision: p
                        .get("changed")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    tracks: vec![],
                    id,
                }
            })
            .collect())
    }

    fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let v = self.get(
            "getPlaylist.view",
            &format!("id={}", urlencoding::encode(playlist_id)),
        )?;
        Ok(json_vec(v.pointer("/subsonic-response/playlist/entry"))
            .iter()
            .map(|t| self.parse_track(t))
            .collect())
    }

    fn create_playlist(&self, name: &str) -> Result<Playlist> {
        let v = self.get(
            "createPlaylist.view",
            &format!("name={}", urlencoding::encode(name)),
        )?;
        let id = v
            .pointer("/subsonic-response/playlist/id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        Ok(Playlist {
            id,
            name: name.into(),
            ..Default::default()
        })
    }

    fn update_playlist(&self, playlist_id: &str, name: &str, track_ids: &[String]) -> Result<()> {
        // recreate: clear by creating with songId list
        let mut extra = format!(
            "playlistId={}&name={}",
            urlencoding::encode(playlist_id),
            urlencoding::encode(name)
        );
        for id in track_ids {
            extra.push_str(&format!("&songId={}", urlencoding::encode(id)));
        }
        self.get("createPlaylist.view", &extra)?;
        Ok(())
    }

    fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        self.get(
            "deletePlaylist.view",
            &format!("id={}", urlencoding::encode(playlist_id)),
        )?;
        Ok(())
    }

    fn similar_tracks(&self, track_id: &str) -> Result<Vec<Track>> {
        let v = self.get(
            "getSimilarSongs2.view",
            &format!("id={}&count=50", urlencoding::encode(track_id)),
        )?;
        Ok(json_vec(v.pointer("/subsonic-response/similarSongs2/song"))
            .iter()
            .map(|t| self.parse_track(t))
            .collect())
    }

    fn instant_mix(&self, item_id: &str) -> Result<Vec<Track>> {
        self.similar_tracks(item_id)
    }

    fn top_tracks(&self, artist_id: Option<&str>) -> Result<Vec<Track>> {
        if let Some(aid) = artist_id {
            let name = self.artist_display_name(aid).unwrap_or_else(|| aid.to_string());
            let v = self.get(
                "getTopSongs.view",
                &format!("artist={}&count=50", urlencoding::encode(&name)),
            )?;
            let arr = json_vec(v.pointer("/subsonic-response/topSongs/song"));
            if !arr.is_empty() {
                return Ok(arr.iter().map(|t| self.parse_track(t)).collect());
            }
        }
        Ok(self.album_list("frequent", 20)?.into_iter().flat_map(|a| {
            self.album_tracks(&a.id).unwrap_or_default()
        }).take(50).collect())
    }

    fn made_for_you(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = Vec::new();
        if let Ok(albums) = self.album_list("frequent", 20) {
            shelves.push(HomeShelf {
                id: "daily-mix".into(),
                title: "Daily Mix".into(),
                albums,
                ..Default::default()
            });
        }
        if let Ok(albums) = self.album_list("recent", 20) {
            shelves.push(HomeShelf {
                id: "on-repeat".into(),
                title: "On repeat".into(),
                albums,
                ..Default::default()
            });
        }
        if let Ok(tracks) = self.top_tracks(None) {
            shelves.push(HomeShelf {
                id: "top-songs".into(),
                title: "Your top songs".into(),
                tracks,
                ..Default::default()
            });
        }
        Ok(shelves)
    }

    fn radio_shelves(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = self.made_for_you()?;
        if let Ok(albums) = self.album_list("random", 20) {
            shelves.insert(
                0,
                HomeShelf {
                    id: "radio-random".into(),
                    title: "Station radar".into(),
                    albums,
                    ..Default::default()
                },
            );
        }
        Ok(shelves)
    }

    fn podcasts(&self) -> Result<Vec<Podcast>> {
        // OpenSubsonic / Airsonic-advanced getPodcasts
        let v = match self.get("getPodcasts.view", "includeEpisodes=false") {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };
        let arr = json_vec(v.pointer("/subsonic-response/podcasts/channel"));
        Ok(arr
            .iter()
            .map(|c| {
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Podcast {
                    title: c
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Podcast")
                        .into(),
                    description: c
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    cover_url: self.cover(c.get("coverArt").and_then(|v| v.as_str()).unwrap_or(&id)),
                    episode_count: None,
                    feed_url: c
                        .get("url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| s.contains("://")),
                    id,
                }
            })
            .collect())
    }

    fn podcast_episodes(&self, podcast_id: &str) -> Result<Vec<PodcastEpisode>> {
        let v = self.get(
            "getPodcasts.view",
            &format!("id={}&includeEpisodes=true", urlencoding::encode(podcast_id)),
        )?;
        let channels = json_vec(v.pointer("/subsonic-response/podcasts/channel"));
        let mut eps = Vec::new();
        for ch in channels {
            let ch_id = json_id_value(ch.get("id")).unwrap_or_default();
            if !ch_id.is_empty() && ch_id != podcast_id {
                continue;
            }
            for e in json_vec(ch.get("episode")) {
                let id = json_id_value(e.get("id")).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let status = e
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                // Prefer downloaded media; skip error/skipped shells without a stream.
                let stream_id = json_id_value(e.get("streamId"));
                let stream = if let Some(sid) = stream_id {
                    self.stream(&sid, None)
                } else if status == "completed" || status == "downloading" {
                    self.stream(&id, None)
                } else {
                    // Undownloaded — no playable server stream yet.
                    String::new()
                };
                if !stream.contains("://") {
                    continue;
                }
                eps.push(PodcastEpisode {
                    title: e.get("title").and_then(|v| v.as_str()).unwrap_or("").into(),
                    podcast_id: podcast_id.into(),
                    duration_secs: e
                        .get("duration")
                        .and_then(|v| v.as_u64())
                        .or_else(|| e.get("duration").and_then(|v| v.as_i64()).map(|n| n as u64)),
                    stream_url: stream,
                    cover_url: {
                        let art = json_id_value(e.get("coverArt")).unwrap_or_default();
                        self.cover(&art)
                    },
                    published: e
                        .get("publishDate")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    id,
                });
            }
        }
        Ok(eps)
    }

    fn subscribe_podcast(&self, feed_url: &str) -> Result<()> {
        let url = feed_url.trim();
        if !url.contains("://") {
            anyhow::bail!("podcast feed URL is missing");
        }
        match self.get(
            "createPodcastChannel.view",
            &format!("url={}", urlencoding::encode(url)),
        ) {
            Ok(_) => {
                let _ = self.refresh_podcasts();
                Ok(())
            }
            Err(err) => {
                let msg = err.to_string().to_ascii_lowercase();
                if msg.contains("exist") || msg.contains("already") {
                    let _ = self.refresh_podcasts();
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    fn refresh_podcasts(&self) -> Result<()> {
        self.get("refreshPodcasts.view", "")?;
        Ok(())
    }

    fn radio_stations(&self) -> Result<Vec<RadioStation>> {
        let v = match self.get("getInternetRadioStations.view", "") {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };
        let arr = json_vec(v.pointer("/subsonic-response/internetRadioStations/internetRadioStation"));
        Ok(arr
            .iter()
            .map(|s| RadioStation {
                id: s.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
                name: s.get("name").and_then(|v| v.as_str()).unwrap_or("Station").into(),
                stream_url: s
                    .get("streamUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                homepage: s
                    .get("homePageUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            })
            .collect())
    }

    fn stream_url(&self, track: &Track, mode: TranscodeMode) -> Result<String> {
        let url = match mode {
            TranscodeMode::Direct => self.stream(&track.id, None),
            TranscodeMode::Auto => {
                // Prefer a known stream URL; fall back to Direct when history/resume
                // only has a track id (empty stream_url).
                if track.stream_url.contains("://") {
                    track.stream_url.clone()
                } else {
                    self.stream(&track.id, None)
                }
            }
            TranscodeMode::Transcode => self.stream(&track.id, Some(192)),
        };
        Ok(url)
    }

    fn item_cover_url(&self, item_id: &str) -> Option<String> {
        self.cover(item_id)
    }

    fn lyrics(&self, track_id: &str) -> Result<Option<String>> {
        if let Ok(v) = self.get("getLyrics.view", &format!("id={}", urlencoding::encode(track_id)))
        {
            if let Some(text) = v
                .pointer("/subsonic-response/lyrics/value")
                .and_then(|x| x.as_str())
                .filter(|s| !s.trim().is_empty())
            {
                return Ok(Some(text.to_string()));
            }
        }
        // Older servers only accept artist + title.
        let song = self
            .get("getSong.view", &format!("id={}", urlencoding::encode(track_id)))
            .ok();
        let artist = song
            .as_ref()
            .and_then(|v| v.pointer("/subsonic-response/song/artist").and_then(|x| x.as_str()))
            .unwrap_or("");
        let title = song
            .as_ref()
            .and_then(|v| v.pointer("/subsonic-response/song/title").and_then(|x| x.as_str()))
            .unwrap_or("");
        if artist.is_empty() || title.is_empty() {
            return Ok(None);
        }
        let v = self.get(
            "getLyrics.view",
            &format!(
                "artist={}&title={}",
                urlencoding::encode(artist),
                urlencoding::encode(title)
            ),
        )?;
        Ok(v.pointer("/subsonic-response/lyrics/value")
            .and_then(|x| x.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string()))
    }

    fn scrobble(&self, track_id: &str, submission: bool) -> Result<()> {
        self.get(
            "scrobble.view",
            &format!(
                "id={}&submission={}",
                urlencoding::encode(track_id),
                if submission { "true" } else { "false" }
            ),
        )?;
        Ok(())
    }

    fn set_favorite(&self, track_id: &str, liked: bool) -> Result<()> {
        let method = if liked {
            "star.view"
        } else {
            "unstar.view"
        };
        self.get(method, &format!("id={}", urlencoding::encode(track_id)))?;
        Ok(())
    }

    fn report_progress(&self, track_id: &str, _position: u64, _is_paused: bool) -> Result<()> {
        self.scrobble(track_id, false)
    }

    fn upload_library_file(&self, path: &std::path::Path, relative: &std::path::Path) -> Result<()> {
        let root = crate::history::History::open()
            .ok()
            .and_then(|h| h.get_setting("subsonic_ingest_path"))
            .map(std::path::PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("neighbourhood-music")
                    .join("subsonic-inbox")
            });
        let dest = root.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &dest)
            .with_context(|| format!("copy {} → {}", path.display(), dest.display()))?;
        // Best-effort scan trigger (Navidrome/OpenSubsonic may ignore).
        let _ = self.get("startScan", "");
        tracing::info!(path = %dest.display(), "uploaded to Subsonic ingest path");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_client_to_older_server() {
        assert_eq!(SubsonicBackend::clamp_to_server("1.14.0"), "1.14.0");
        assert_eq!(SubsonicBackend::clamp_to_server("1.15.0"), "1.15.0");
        assert_eq!(SubsonicBackend::clamp_to_server("1.12.0"), "1.12.0");
    }

    #[test]
    fn json_vec_accepts_one_object_or_many() {
        let many = serde_json::json!([{"id": "1"}, {"id": "2"}]);
        assert_eq!(json_vec(Some(&many)).len(), 2);
        let one = serde_json::json!({"id": "1", "name": "Only"});
        assert_eq!(json_vec(Some(&one)).len(), 1);
        assert!(json_vec(None).is_empty());
        assert!(json_vec(Some(&serde_json::json!(null))).is_empty());
    }

    #[test]
    fn version_order() {
        assert!(SubsonicBackend::version_cmp("1.16.1", "1.14.0") > 0);
        assert!(SubsonicBackend::version_cmp("1.13.0", "1.16.1") < 0);
        assert_eq!(SubsonicBackend::version_cmp("1.15.0", "1.15.9"), 0);
    }
}
