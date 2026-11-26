use super::MediaBackend;
use crate::model::*;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
pub struct JellyfinBackend {
    name: String,
    base: String,
    user_id: String,
    token: String,
    device_id: String,
}

impl JellyfinBackend {
    pub fn connect(server: &ServerTarget) -> Result<Self> {
        let base = crate::urlfix::normalize_http_url(&server.base_url);
        let device_id = format!("mde-music-{}", hostname::get().unwrap_or_default().to_string_lossy());
        let mut be = Self {
            name: server.name.clone(),
            base: base.clone(),
            user_id: String::new(),
            token: server.password.clone(),
            device_id,
        };

        // If password looks like an API token, try Users/Me; else authenticate by name/password.
        if !server.password.is_empty() && server.username.is_empty() {
            be.token = server.password.clone();
            if let Ok(me) = be.get("/Users/Me") {
                be.user_id = me.get("Id").and_then(|v| v.as_str()).unwrap_or("").into();
            }
        }
        if be.user_id.is_empty() {
            be.authenticate(&server.username, &server.password)?;
        }
        Ok(be)
    }

    fn auth_header(&self) -> String {
        format!(
            "MediaBrowser Client=\"NeighbourhoodMusic\", Device=\"MDE\", DeviceId=\"{}\", Version=\"2026.0\", Token=\"{}\"",
            self.device_id, self.token
        )
    }

    fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let resp = ureq::get(&url)
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
            .call()
            .with_context(|| format!("GET {path}"))?;
        Ok(resp.into_json()?)
    }

    fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let resp = ureq::post(&url)
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
            .set("Content-Type", "application/json")
            .send_json(body)
            .with_context(|| format!("POST {path}"))?;
        if resp.content_type().contains("json") {
            Ok(resp.into_json().unwrap_or(Value::Null))
        } else {
            Ok(Value::Null)
        }
    }

    fn post_empty(&self, path: &str) -> Result<()> {
        let url = format!("{}{path}", self.base);
        ureq::post(&url)
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
            .call()
            .with_context(|| format!("POST {path}"))?;
        Ok(())
    }

    fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        let url = format!("{}/Users/AuthenticateByName", self.base);
        let resp = ureq::post(&url)
            .set(
                "Authorization",
                &format!(
                    "MediaBrowser Client=\"NeighbourhoodMusic\", Device=\"MDE\", DeviceId=\"{}\", Version=\"2026.0\"",
                    self.device_id
                ),
            )
            .set("Content-Type", "application/json")
            .send_json(json!({
                "Username": username,
                "Pw": password,
            }))
            .context("jellyfin auth")?;
        let v: Value = resp.into_json()?;
        self.token = v
            .get("AccessToken")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        self.user_id = v
            .pointer("/User/Id")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if self.token.is_empty() || self.user_id.is_empty() {
            bail!("Jellyfin authentication failed");
        }
        Ok(())
    }

    fn cover(&self, id: &str) -> Option<String> {
        if id.is_empty() {
            return None;
        }
        Some(format!(
            "{}/Items/{id}/Images/Primary?api_key={}&maxHeight=400",
            self.base, self.token
        ))
    }

    fn stream_direct(&self, id: &str) -> String {
        format!(
            "{}/Audio/{id}/stream?static=true&api_key={}",
            self.base, self.token
        )
    }

    fn stream_transcode(&self, id: &str) -> String {
        format!(
            "{}/Audio/{id}/universal?UserId={}&DeviceId={}&MaxStreamingBitrate=192000&Container=mp3&TranscodingContainer=mp3&api_key={}",
            self.base, self.user_id, self.device_id, self.token
        )
    }

    fn parse_track(&self, it: &Value) -> Track {
        let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artists = it
            .get("Artists")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Track {
            title: it.get("Name").and_then(|v| v.as_str()).unwrap_or("Track").into(),
            artist: artists,
            artist_id: it
                .pointer("/ArtistItems/0/Id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            album: it
                .get("Album")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            album_id: it
                .get("AlbumId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            duration_secs: it
                .get("RunTimeTicks")
                .and_then(|v| v.as_u64())
                .map(|t| t / 10_000_000),
            track_num: it
                .get("IndexNumber")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            cover_url: self.cover(it.get("AlbumId").and_then(|v| v.as_str()).unwrap_or(&id)),
            stream_url: self.stream_direct(&id),
            content_type: it
                .pointer("/MediaSources/0/Container")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bit_rate: it
                .pointer("/MediaSources/0/Bitrate")
                .and_then(|v| v.as_u64())
                .map(|n| (n / 1000) as u32),
            sample_rate: None,
            format: it
                .pointer("/MediaSources/0/Container")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            liked: it
                .pointer("/UserData/IsFavorite")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            source: SourceKind::Server,
            video_url: None,
            id,
        }
    }

    fn parse_album(&self, it: &Value) -> Album {
        let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Album {
            name: it.get("Name").and_then(|v| v.as_str()).unwrap_or("Album").into(),
            artist: it
                .get("AlbumArtist")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            artist_id: it
                .pointer("/AlbumArtists/0/Id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            year: it
                .get("ProductionYear")
                .and_then(|v| v.as_i64())
                .map(|y| y as i32),
            cover_url: self.cover(&id),
            track_count: it
                .get("ChildCount")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            source: SourceKind::Server,
            alt_ids: Vec::new(),
            id,
        }
    }

    fn items(&self, query: &str) -> Result<Vec<Value>> {
        let path = format!("/Users/{}/Items?{query}", self.user_id);
        let v = self.get(&path)?;
        Ok(v.get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Walk `StartIndex` until `TotalRecordCount` is covered.
    fn items_paged(&self, query: &str) -> Result<Vec<Value>> {
        const PAGE: u64 = 200;
        let mut start = 0u64;
        let mut out = Vec::new();
        loop {
            let sep = if query.is_empty() { "" } else { "&" };
            let path = format!(
                "/Users/{}/Items?{query}{sep}Limit={PAGE}&StartIndex={start}",
                self.user_id
            );
            let v = self.get(&path)?;
            let batch = v
                .get("Items")
                .and_then(|i| i.as_array())
                .cloned()
                .unwrap_or_default();
            let n = batch.len() as u64;
            let total = v.get("TotalRecordCount").and_then(|t| t.as_u64());
            out.extend(batch);
            if n == 0 {
                break;
            }
            start += n;
            if total.is_some_and(|total| start >= total) || (total.is_none() && n < PAGE) {
                break;
            }
            if start > 100_000 {
                break;
            }
        }
        Ok(out)
    }
}

impl MediaBackend for JellyfinBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Jellyfin
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn ping(&self) -> Result<()> {
        self.get("/System/Info/Public")?;
        Ok(())
    }

    fn home_shelves(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = Vec::new();
        let specs = [
            ("latest", "Recently added", "IncludeItemTypes=MusicAlbum&SortBy=DateCreated&SortOrder=Descending&Limit=20&Recursive=true"),
            ("played", "Recently played", "IncludeItemTypes=Audio&SortBy=DatePlayed&SortOrder=Descending&Limit=30&Recursive=true&Filters=IsPlayed"),
            ("freq", "Heavy rotation", "IncludeItemTypes=Audio&SortBy=PlayCount&SortOrder=Descending&Limit=30&Recursive=true"),
            ("fav", "Liked songs", "IncludeItemTypes=Audio&Filters=IsFavorite&Limit=50&Recursive=true"),
        ];
        for (id, title, q) in specs {
            if let Ok(items) = self.items(q) {
                if id == "latest" {
                    let albums: Vec<_> = items.iter().map(|i| self.parse_album(i)).collect();
                    if !albums.is_empty() {
                        shelves.push(HomeShelf {
                            id: id.into(),
                            title: title.into(),
                            albums,
                            ..Default::default()
                        });
                    }
                } else {
                    let tracks: Vec<_> = items.iter().map(|i| self.parse_track(i)).collect();
                    if !tracks.is_empty() {
                        shelves.push(HomeShelf {
                            id: id.into(),
                            title: title.into(),
                            tracks,
                            ..Default::default()
                        });
                    }
                }
            }
        }
        Ok(shelves)
    }

    fn search(&self, query: &str) -> Result<SearchResults> {
        let q = urlencoding::encode(query);
        let artists = self
            .items(&format!(
                "SearchTerm={q}&IncludeItemTypes=MusicArtist&Recursive=true&Limit=25"
            ))?
            .iter()
            .map(|it| {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Artist {
                    name: it.get("Name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    cover_url: self.cover(&id),
                    album_count: None,
                    id,
                }
            })
            .collect();
        let albums = self
            .items(&format!(
                "SearchTerm={q}&IncludeItemTypes=MusicAlbum&Recursive=true&Limit=25"
            ))?
            .iter()
            .map(|it| self.parse_album(it))
            .collect();
        let tracks = self
            .items(&format!(
                "SearchTerm={q}&IncludeItemTypes=Audio&Recursive=true&Limit=50"
            ))?
            .iter()
            .map(|it| self.parse_track(it))
            .collect();
        let playlists = self
            .items(&format!(
                "SearchTerm={q}&IncludeItemTypes=Playlist&Recursive=true&Limit=20"
            ))?
            .iter()
            .map(|it| Playlist {
                id: it.get("Id").and_then(|v| v.as_str()).unwrap_or("").into(),
                name: it.get("Name").and_then(|v| v.as_str()).unwrap_or("").into(),
                song_count: it.get("ChildCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                cover_url: self.cover(it.get("Id").and_then(|v| v.as_str()).unwrap_or("")),
                ..Default::default()
            })
            .collect();
        Ok(SearchResults {
            artists,
            albums,
            tracks,
            playlists,
        })
    }

    fn artists(&self) -> Result<Vec<Artist>> {
        Ok(self
            .items_paged("IncludeItemTypes=MusicArtist&Recursive=true&SortBy=SortName")?
            .iter()
            .map(|it| {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Artist {
                    name: it.get("Name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    cover_url: self.cover(&id),
                    album_count: it.get("AlbumCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    id,
                }
            })
            .collect())
    }

    fn albums(&self) -> Result<Vec<Album>> {
        Ok(self
            .items_paged("IncludeItemTypes=MusicAlbum&Recursive=true&SortBy=SortName")?
            .iter()
            .map(|it| self.parse_album(it))
            .collect())
    }

    fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        Ok(self
            .items(&format!(
                "ArtistIds={artist_id}&IncludeItemTypes=MusicAlbum&Recursive=true&SortBy=ProductionYear"
            ))?
            .iter()
            .map(|it| self.parse_album(it))
            .collect())
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<Track>> {
        let path = format!(
            "/Users/{}/Items?ParentId={album_id}&IncludeItemTypes=Audio&SortBy=IndexNumber",
            self.user_id
        );
        let v = self.get(&path)?;
        Ok(v.get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|it| self.parse_track(it))
            .collect())
    }

    fn playlists(&self) -> Result<Vec<Playlist>> {
        Ok(self
            .items("IncludeItemTypes=Playlist&Recursive=true&SortBy=SortName")?
            .iter()
            .map(|it| {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Playlist {
                    name: it.get("Name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    song_count: it.get("ChildCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    cover_url: self.cover(&id),
                    revision: it
                        .get("DateLastMediaAdded")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    id,
                    ..Default::default()
                }
            })
            .collect())
    }

    fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let path = format!("/Playlists/{playlist_id}/Items?UserId={}", self.user_id);
        let v = self.get(&path)?;
        Ok(v.get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|it| self.parse_track(it))
            .collect())
    }

    fn create_playlist(&self, name: &str) -> Result<Playlist> {
        let v = self.post_json(
            &format!("/Playlists?Name={}&UserId={}", urlencoding::encode(name), self.user_id),
            json!({}),
        )?;
        let id = v.get("Id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        Ok(Playlist {
            id,
            name: name.into(),
            ..Default::default()
        })
    }

    fn update_playlist(&self, playlist_id: &str, name: &str, track_ids: &[String]) -> Result<()> {
        let _ = name;
        // Replace items: clear then add
        let ids = track_ids.join(",");
        self.post_empty(&format!(
            "/Playlists/{playlist_id}/Items?Ids={ids}&UserId={}",
            self.user_id
        ))?;
        Ok(())
    }

    fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        let url = format!("{}/Items/{playlist_id}", self.base);
        ureq::request("DELETE", &url)
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
            .call()?;
        Ok(())
    }

    fn similar_tracks(&self, track_id: &str) -> Result<Vec<Track>> {
        let path = format!("/Items/{track_id}/Similar?Limit=50&UserId={}", self.user_id);
        let v = self.get(&path)?;
        Ok(v.get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|it| self.parse_track(it))
            .collect())
    }

    fn instant_mix(&self, item_id: &str) -> Result<Vec<Track>> {
        let path = format!(
            "/Items/{item_id}/InstantMix?UserId={}&Limit=50",
            self.user_id
        );
        let v = self.get(&path)?;
        Ok(v.get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|it| self.parse_track(it))
            .collect())
    }

    fn top_tracks(&self, artist_id: Option<&str>) -> Result<Vec<Track>> {
        let mut q = "IncludeItemTypes=Audio&SortBy=PlayCount&SortOrder=Descending&Recursive=true&Limit=50".to_string();
        if let Some(aid) = artist_id {
            q.push_str(&format!("&ArtistIds={aid}"));
        }
        Ok(self.items(&q)?.iter().map(|it| self.parse_track(it)).collect())
    }

    fn made_for_you(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = Vec::new();
        // Instant mix from favorites if any
        if let Ok(favs) = self.items("IncludeItemTypes=Audio&Filters=IsFavorite&Limit=5&Recursive=true") {
            if let Some(first) = favs.first() {
                if let Some(id) = first.get("Id").and_then(|v| v.as_str()) {
                    if let Ok(tracks) = self.instant_mix(id) {
                        shelves.push(HomeShelf {
                            id: "instant-mix".into(),
                            title: "Made for you mix".into(),
                            tracks,
                            ..Default::default()
                        });
                    }
                }
            }
        }
        if let Ok(tracks) = self.top_tracks(None) {
            shelves.push(HomeShelf {
                id: "top".into(),
                title: "Your top songs".into(),
                tracks,
                ..Default::default()
            });
        }
        Ok(shelves)
    }

    fn radio_shelves(&self) -> Result<Vec<HomeShelf>> {
        let mut shelves = self.made_for_you()?;
        if let Ok(items) = self.items("IncludeItemTypes=MusicAlbum&SortBy=Random&Limit=20&Recursive=true") {
            shelves.insert(
                0,
                HomeShelf {
                    id: "radio".into(),
                    title: "Radio for you".into(),
                    albums: items.iter().map(|i| self.parse_album(i)).collect(),
                    ..Default::default()
                },
            );
        }
        Ok(shelves)
    }

    fn podcasts(&self) -> Result<Vec<Podcast>> {
        // Jellyfin: collections typed as podcasts or channels
        let items = self.items(
            "IncludeItemTypes=Folder,CollectionFolder&Recursive=false",
        )?;
        let mut out = Vec::new();
        for it in items {
            let ct = it
                .get("CollectionType")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if ct.eq_ignore_ascii_case("podcasts") || ct.eq_ignore_ascii_case("homevideos") {
                continue;
            }
            let _ = ct;
        }
        // Also search AudioBook / Podcast episode parents
        if let Ok(shows) = self.items("IncludeItemTypes=MusicArtist,Folder&SearchTerm=podcast&Recursive=true&Limit=50") {
            for it in shows {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                out.push(Podcast {
                    title: it.get("Name").and_then(|v| v.as_str()).unwrap_or("Podcast").into(),
                    description: it
                        .get("Overview")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    cover_url: self.cover(&id),
                    episode_count: it.get("ChildCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                    feed_url: None,
                    id,
                });
            }
        }
        // Prefer dedicated podcast library via views
        if let Ok(views) = self.get(&format!("/Users/{}/Views", self.user_id)) {
            if let Some(arr) = views.get("Items").and_then(|i| i.as_array()) {
                for v in arr {
                    let ct = v.get("CollectionType").and_then(|x| x.as_str()).unwrap_or("");
                    if ct == "podcasts" || ct == "audiobooks" {
                        let id = v.get("Id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        out.push(Podcast {
                            title: v.get("Name").and_then(|x| x.as_str()).unwrap_or("Podcasts").into(),
                            description: ct.into(),
                            cover_url: self.cover(&id),
                            episode_count: None,
                            feed_url: None,
                            id,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    fn podcast_episodes(&self, podcast_id: &str) -> Result<Vec<PodcastEpisode>> {
        Ok(self
            .items(&format!(
                "ParentId={podcast_id}&IncludeItemTypes=Audio,AudioBook&Recursive=true&SortBy=DateCreated&SortOrder=Descending&Limit=100"
            ))?
            .iter()
            .map(|it| {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                PodcastEpisode {
                    title: it.get("Name").and_then(|v| v.as_str()).unwrap_or("").into(),
                    podcast_id: podcast_id.into(),
                    duration_secs: it
                        .get("RunTimeTicks")
                        .and_then(|v| v.as_u64())
                        .map(|t| t / 10_000_000),
                    stream_url: self.stream_direct(&id),
                    cover_url: self.cover(&id),
                    published: it
                        .get("PremiereDate")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    id,
                }
            })
            .collect())
    }

    fn radio_stations(&self) -> Result<Vec<RadioStation>> {
        // Live TV channels marked as radio, or internet radio items
        let mut out = Vec::new();
        if let Ok(v) = self.get("/LiveTv/Channels?IsMovie=false&IsNews=false&IsSeries=false&IsSports=false") {
            if let Some(arr) = v.get("Items").and_then(|i| i.as_array()) {
                for ch in arr {
                    if ch.get("Type").and_then(|t| t.as_str()) == Some("Radio")
                        || ch
                            .get("ChannelType")
                            .and_then(|t| t.as_str())
                            .map(|s| s.eq_ignore_ascii_case("Radio"))
                            .unwrap_or(false)
                    {
                        let id = ch.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        out.push(RadioStation {
                            name: ch.get("Name").and_then(|v| v.as_str()).unwrap_or("Radio").into(),
                            stream_url: format!(
                                "{}/LiveTv/LiveStreamFiles/{id}/stream.ts?api_key={}",
                                self.base, self.token
                            ),
                            homepage: None,
                            id,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    fn stream_url(&self, track: &Track, mode: TranscodeMode) -> Result<String> {
        Ok(match mode {
            TranscodeMode::Direct => self.stream_direct(&track.id),
            TranscodeMode::Auto => track.stream_url.clone(),
            TranscodeMode::Transcode => self.stream_transcode(&track.id),
        })
    }

    fn lyrics(&self, track_id: &str) -> Result<Option<String>> {
        match self.get(&format!("/Audio/{track_id}/Lyrics")) {
            Ok(v) => {
                if let Some(s) = v.get("Lyrics").and_then(|l| l.as_str()) {
                    return Ok(Some(s.to_string()));
                }
                // structured lyrics
                if let Some(arr) = v.pointer("/Lyrics").and_then(|l| l.as_array()) {
                    let text = arr
                        .iter()
                        .filter_map(|line| line.get("Text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        return Ok(Some(text));
                    }
                }
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    fn scrobble(&self, track_id: &str, submission: bool) -> Result<()> {
        if submission {
            self.post_empty(&format!(
                "/Users/{}/PlayedItems/{track_id}",
                self.user_id
            ))?;
        } else {
            let _ = track_id;
        }
        Ok(())
    }

    fn set_favorite(&self, track_id: &str, liked: bool) -> Result<()> {
        if liked {
            self.post_empty(&format!(
                "/Users/{}/FavoriteItems/{track_id}",
                self.user_id
            ))?;
        } else {
            let url = format!(
                "{}/Users/{}/FavoriteItems/{track_id}",
                self.base, self.user_id
            );
            ureq::request("DELETE", &url)
                .set("Authorization", &self.auth_header())
                .set("X-Emby-Token", &self.token)
                .call()?;
        }
        Ok(())
    }

    fn report_progress(&self, track_id: &str, position_secs: u64, is_paused: bool) -> Result<()> {
        let ticks = position_secs * 10_000_000;
        self.post_json(
            "/Sessions/Playing/Progress",
            json!({
                "ItemId": track_id,
                "PositionTicks": ticks,
                "IsPaused": is_paused,
                "PlayMethod": "DirectStream",
            }),
        )?;
        Ok(())
    }

    fn upload_library_file(&self, path: &std::path::Path, relative: &std::path::Path) -> Result<()> {
        // Prefer configured ingest directory (Neighbourhood setting), else local staging
        // under the data dir and ask Jellyfin to refresh.
        let root = crate::history::History::open()
            .ok()
            .and_then(|h| h.get_setting("jellyfin_ingest_path"))
            .map(std::path::PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("neighbourhood-music")
                    .join("jellyfin-inbox")
            });
        let dest = root.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &dest)
            .with_context(|| format!("copy {} → {}", path.display(), dest.display()))?;
        // Trigger library scan so the new file appears.
        let _ = self.post_empty("/Library/Refresh");
        tracing::info!(path = %dest.display(), "uploaded to Jellyfin ingest path");
        Ok(())
    }

    fn supports_remote_control(&self) -> bool {
        true
    }

    fn register_session(&self) -> Result<()> {
        // Capabilities ping so Jellyfin can see this client
        let _ = self.get("/Sessions");
        Ok(())
    }
}
