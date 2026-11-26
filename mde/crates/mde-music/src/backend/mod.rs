pub mod jellyfin;
pub mod subsonic;
pub mod youtube_music;

use crate::model::*;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

pub trait MediaBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn name(&self) -> &str;
    fn ping(&self) -> Result<()>;

    fn home_shelves(&self) -> Result<Vec<HomeShelf>>;
    fn search(&self, query: &str) -> Result<SearchResults>;
    fn artists(&self) -> Result<Vec<Artist>>;
    /// Full album catalog, paged when the server caps a single response.
    fn albums(&self) -> Result<Vec<Album>> {
        Ok(Vec::new())
    }
    fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>>;
    fn album_tracks(&self, album_id: &str) -> Result<Vec<Track>>;
    fn playlists(&self) -> Result<Vec<Playlist>>;
    fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>>;
    fn create_playlist(&self, name: &str) -> Result<Playlist>;
    fn update_playlist(&self, playlist_id: &str, name: &str, track_ids: &[String]) -> Result<()>;
    fn delete_playlist(&self, playlist_id: &str) -> Result<()>;

    fn similar_tracks(&self, track_id: &str) -> Result<Vec<Track>>;
    fn instant_mix(&self, item_id: &str) -> Result<Vec<Track>>;
    fn top_tracks(&self, artist_id: Option<&str>) -> Result<Vec<Track>>;
    fn made_for_you(&self) -> Result<Vec<HomeShelf>>;
    fn radio_shelves(&self) -> Result<Vec<HomeShelf>>;

    fn podcasts(&self) -> Result<Vec<Podcast>>;
    fn podcast_episodes(&self, podcast_id: &str) -> Result<Vec<PodcastEpisode>>;
    /// Subscribe the server to a podcast feed. Servers that cannot do this return an error.
    fn subscribe_podcast(&self, _feed_url: &str) -> Result<()> {
        anyhow::bail!("this server cannot subscribe to podcasts")
    }
    fn radio_stations(&self) -> Result<Vec<RadioStation>>;

    fn stream_url(&self, track: &Track, mode: TranscodeMode) -> Result<String>;
    fn lyrics(&self, track_id: &str) -> Result<Option<String>>;
    fn scrobble(&self, track_id: &str, submission: bool) -> Result<()>;
    fn set_favorite(&self, track_id: &str, liked: bool) -> Result<()>;
    fn report_progress(&self, track_id: &str, position_ticks_or_secs: u64, is_paused: bool) -> Result<()>;

    /// Upload a local audio file into the server library (API or watched path).
    fn upload_library_file(&self, _path: &Path, _relative: &Path) -> Result<()> {
        anyhow::bail!("upload not supported by this server")
    }

    fn supports_remote_control(&self) -> bool {
        false
    }
    fn register_session(&self) -> Result<()> {
        Ok(())
    }
}

pub fn connect(server: &ServerTarget) -> Result<Arc<dyn MediaBackend>> {
    match server.kind {
        BackendKind::Jellyfin => Ok(Arc::new(jellyfin::JellyfinBackend::connect(server)?)),
        BackendKind::Subsonic => Ok(Arc::new(subsonic::SubsonicBackend::connect(server)?)),
        BackendKind::YoutubeMusic => youtube_music::as_arc(),
    }
}

/// Synthetic always-on YTM source for the unified catalog.
pub fn youtube_music_source() -> Result<Arc<dyn MediaBackend>> {
    youtube_music::as_arc()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the configured Airsonic/Navidrome place when MDE_MUSIC_LIVE=1.
    #[test]
    fn live_library_catalog_when_requested() {
        if std::env::var_os("MDE_MUSIC_LIVE").is_none() {
            return;
        }
        let servers = crate::places::list_servers().expect("list servers");
        let srv = servers
            .into_iter()
            .find(|s| {
                s.kind == BackendKind::Subsonic
                    && s.base_url.starts_with("http://")
                    && !s.username.is_empty()
            })
            .expect("subsonic server");
        let be = connect(&srv).expect("connect");
        let artists = be.artists().expect("artists");
        let albums = be.albums().expect("albums");
        let playlists = be.playlists().expect("playlists");
        eprintln!(
            "catalog artists={} albums={} playlists={}",
            artists.len(),
            albums.len(),
            playlists.len()
        );
        assert!(artists.len() >= 200, "artists {}", artists.len());
        assert!(albums.len() >= 1500, "albums {}", albums.len());
        assert!(!playlists.is_empty(), "playlists");
        let artist = artists
            .iter()
            .find(|a| a.album_count.unwrap_or(0) > 0)
            .expect("artist with albums");
        let artist_albums = be.artist_albums(&artist.id).expect("artist albums");
        assert!(
            !artist_albums.is_empty(),
            "browse albums for {}",
            artist.name
        );
        let tracks = be.album_tracks(&artist_albums[0].id).expect("tracks");
        assert!(
            !tracks.is_empty(),
            "browse tracks for {}",
            artist_albums[0].name
        );
    }
}
