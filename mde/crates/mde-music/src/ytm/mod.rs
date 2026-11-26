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
        let mut client = InnertubeClient::default();
        client.load_repair_overrides();
        Self { client }
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

    /// Resolve best audio URL; falls back to yt-dlp when native fails.
    pub fn resolve_stream(&self, video_id: &str) -> Result<stream::ResolvedStream> {
        match self.client.resolve_audio(video_id) {
            Ok(s) => Ok(s),
            Err(native_err) => {
                tracing::warn!(%native_err, "native YTM resolve failed; trying yt-dlp");
                stream::resolve_via_ytdlp(video_id).map_err(|e| {
                    anyhow::anyhow!("native: {native_err:#}; yt-dlp: {e:#}")
                })
            }
        }
    }

    pub fn smoke_test(&self) -> Result<()> {
        let sr = self.search("music")?;
        let track = sr
            .tracks
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("smoke: no search hits"))?;
        let id = track
            .id
            .strip_prefix("ytm:")
            .unwrap_or(&track.id)
            .to_string();
        let _ = self.home_shelves()?;
        let stream = self.resolve_stream(&id)?;
        if stream.url.is_empty() {
            anyhow::bail!("smoke: empty stream url");
        }
        // Short buffered play probe via ureq HEAD/GET of a few KB.
        let _ = ureq::get(&stream.url)
            .set("Range", "bytes=0-4095")
            .call()
            .map_err(|e| anyhow::anyhow!("smoke play buffer: {e}"))?;
        Ok(())
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
