//! Native YouTube Music pipeline (no ytmdesktop runtime).
//! Methods inspired by yt-dlp / ytmdesktop Innertube clients.

pub mod innertube;
pub mod repair;
pub mod stream;

use crate::model::*;
use anyhow::Result;
use innertube::InnertubeClient;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
}

pub fn repair_dir() -> PathBuf {
    config_dir().join("ytm-repair")
}

/// High-level anonymous YTM access used by the backend + acquire path.
#[derive(Clone, Default)]
pub struct YtmEngine {
    client: InnertubeClient,
}

impl YtmEngine {
    pub fn new() -> Self {
        let client = InnertubeClient::default();
        client.load_repair_overrides();
        Self { client }
    }

    /// Re-read repair rules so the next request uses the rotated client.
    pub fn reload_rules(&self) {
        self.client.load_repair_overrides();
    }

    pub fn search(&self, query: &str) -> Result<SearchResults> {
        self.client.search(query)
    }

    pub fn home_shelves(&self) -> Result<Vec<HomeShelf>> {
        self.client.home_shelves()
    }

    pub fn album_tracks(&self, album_id: &str) -> Result<Vec<Track>> {
        self.client.album_tracks(album_id)
    }

    pub fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        self.client.artist_albums(artist_id)
    }

    pub fn related(&self, video_id: &str) -> Result<Vec<Track>> {
        self.client.related(video_id)
    }

    pub fn charts_moods(&self) -> Result<Vec<HomeShelf>> {
        self.client.charts_and_moods()
    }

    /// Native Innertube first. A URL that does not return audio falls back to yt-dlp.
    /// No Electron or ytmdesktop process is involved.
    pub fn resolve_stream(&self, video_id: &str) -> Result<stream::ResolvedStream> {
        let mut native_err = None;
        match self.client.resolve_audio(video_id) {
            Ok(mut stream) => {
                stream.needs_android_ua = stream.url.contains("googlevideo.com");
                let probe_ok = stream::url_is_playable(&stream.url, stream.needs_android_ua);
                if stream::native_probe_accepted(&stream.url, probe_ok) {
                    tracing::info!("playing native Innertube stream");
                    return Ok(stream);
                }
                native_err = Some("native URL did not return audio".to_string());
                tracing::warn!("native Innertube URL was not playable; falling back to yt-dlp");
            }
            Err(err) => {
                native_err = Some(format!("{err:#}"));
                tracing::warn!(%err, "native Innertube failed; falling back to yt-dlp");
            }
        }
        match stream::resolve_via_ytdlp(video_id) {
            Ok(stream) => Ok(stream),
            Err(ytdlp_err) => Err(anyhow::anyhow!(
                "native: {}; yt-dlp: {ytdlp_err:#}",
                native_err.unwrap_or_else(|| "not tried".into())
            )),
        }
    }

    pub fn smoke_test(&self) -> Result<()> {
        const PROBE_IDS: &[&str] = &["dQw4w9WgXcQ", "kJQP7kiw5Fk", "fJ9rUzIMcZQ"];
        let _ = self.home_shelves()?;
        let mut last_err = None;
        for id in PROBE_IDS {
            match self.resolve_stream(id) {
                Ok(stream) if !stream.url.is_empty() => {
                    if stream.url.starts_with("file:") {
                        return Ok(());
                    }
                    let mut req = ureq::get(&stream.url).set("Range", "bytes=0-4095");
                    if stream.needs_android_ua {
                        req = req.set("User-Agent", stream::ANDROID_YT_UA);
                    }
                    req.call()
                        .map_err(|e| anyhow::anyhow!("smoke play buffer: {e}"))?;
                    return Ok(());
                }
                Ok(_) => last_err = Some(anyhow::anyhow!("smoke: empty stream url for {id}")),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("smoke: no playable probe")))
    }
}

pub fn is_ytm_track(track: &Track) -> bool {
    track.source == SourceKind::YoutubeMusic || track.id.starts_with("ytm:")
}

pub fn video_id_of(track: &Track) -> Option<String> {
    track
        .id
        .strip_prefix("ytm:")
        .map(str::to_string)
        .or_else(|| {
            if track.id.len() == 11 {
                Some(track.id.clone())
            } else {
                None
            }
        })
}
