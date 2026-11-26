//! Native YouTube Music backend (anonymous). Merges into unified catalog.

use super::MediaBackend;
use crate::model::*;
use crate::ytm::{self, YtmEngine};
use anyhow::Result;
use std::sync::Arc;

pub struct YoutubeMusicBackend {
    engine: YtmEngine,
    name: String,
}

impl YoutubeMusicBackend {
    pub fn connect() -> Result<Self> {
        Ok(Self {
            engine: YtmEngine::new(),
            name: "YouTube Music".into(),
        })
    }

    pub fn engine(&self) -> &YtmEngine {
        &self.engine
    }
}

impl MediaBackend for YoutubeMusicBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::YoutubeMusic
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn ping(&self) -> Result<()> {
        self.engine.smoke_test()
    }

    fn home_shelves(&self) -> Result<Vec<HomeShelf>> {
        match self.engine.home_shelves() {
            Ok(s) => Ok(s),
            Err(e) => {
                crate::ytm::repair::note_ytm_failure();
                Err(e)
            }
        }
    }

    fn search(&self, query: &str) -> Result<SearchResults> {
        match self.engine.search(query) {
            Ok(s) => Ok(s),
            Err(e) => {
                crate::ytm::repair::note_ytm_failure();
                Err(e)
            }
        }
    }

    fn artists(&self) -> Result<Vec<Artist>> {
        Ok(Vec::new())
    }

    fn albums(&self) -> Result<Vec<Album>> {
        Ok(self
            .engine
            .home_shelves()
            .unwrap_or_default()
            .into_iter()
            .flat_map(|s| s.albums)
            .collect())
    }

    fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        self.engine.artist_albums(artist_id)
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<Track>> {
        self.engine.album_tracks(album_id)
    }

    fn playlists(&self) -> Result<Vec<Playlist>> {
        Ok(Vec::new())
    }
    fn playlist_tracks(&self, _playlist_id: &str) -> Result<Vec<Track>> {
        Ok(Vec::new())
    }
    fn create_playlist(&self, _name: &str) -> Result<Playlist> {
        anyhow::bail!("YouTube Music playlists require an account (disabled)")
    }
    fn update_playlist(
        &self,
        _playlist_id: &str,
        _name: &str,
        _track_ids: &[String],
    ) -> Result<()> {
        anyhow::bail!("YouTube Music playlists require an account (disabled)")
    }
    fn delete_playlist(&self, _playlist_id: &str) -> Result<()> {
        anyhow::bail!("YouTube Music playlists require an account (disabled)")
    }

    fn similar_tracks(&self, track_id: &str) -> Result<Vec<Track>> {
        self.engine.related(track_id)
    }
    fn instant_mix(&self, item_id: &str) -> Result<Vec<Track>> {
        self.engine.related(item_id)
    }
    fn top_tracks(&self, _artist_id: Option<&str>) -> Result<Vec<Track>> {
        Ok(Vec::new())
    }
    fn made_for_you(&self) -> Result<Vec<HomeShelf>> {
        self.engine.charts_moods()
    }
    fn radio_shelves(&self) -> Result<Vec<HomeShelf>> {
        self.engine.charts_moods()
    }

    fn podcasts(&self) -> Result<Vec<Podcast>> {
        Ok(Vec::new())
    }
    fn podcast_episodes(&self, _podcast_id: &str) -> Result<Vec<PodcastEpisode>> {
        Ok(Vec::new())
    }
    fn radio_stations(&self) -> Result<Vec<RadioStation>> {
        Ok(Vec::new())
    }

    fn stream_url(&self, track: &Track, _mode: TranscodeMode) -> Result<String> {
        let vid = ytm::video_id_of(track).unwrap_or_else(|| track.id.clone());
        match self.engine.resolve_stream(&vid) {
            Ok(s) => Ok(s.url),
            Err(e) => {
                crate::ytm::repair::note_ytm_failure();
                Err(e)
            }
        }
    }

    fn lyrics(&self, _track_id: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn scrobble(&self, _track_id: &str, _submission: bool) -> Result<()> {
        Ok(())
    }
    fn set_favorite(&self, _track_id: &str, _liked: bool) -> Result<()> {
        Ok(())
    }
    fn report_progress(
        &self,
        _track_id: &str,
        _position_ticks_or_secs: u64,
        _is_paused: bool,
    ) -> Result<()> {
        Ok(())
    }
}

pub fn as_arc() -> Result<Arc<dyn MediaBackend>> {
    Ok(Arc::new(YoutubeMusicBackend::connect()?))
}
