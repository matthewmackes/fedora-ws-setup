use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Jellyfin,
    Subsonic,
}

impl BackendKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jellyfin => "Jellyfin",
            Self::Subsonic => "Airsonic / Subsonic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTarget {
    pub id: String,
    pub name: String,
    pub kind: BackendKind,
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub from_neighbourhood: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub album_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub year: Option<i32>,
    pub cover_url: Option<String>,
    pub track_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub album: String,
    pub album_id: Option<String>,
    pub duration_secs: Option<u64>,
    pub track_num: Option<u32>,
    pub cover_url: Option<String>,
    pub stream_url: String,
    pub content_type: Option<String>,
    pub bit_rate: Option<u32>,
    pub liked: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub song_count: Option<u32>,
    pub cover_url: Option<String>,
    pub owner: Option<String>,
    pub revision: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Podcast {
    pub id: String,
    pub title: String,
    pub description: String,
    pub cover_url: Option<String>,
    pub episode_count: Option<u32>,
    /// Original feed URL when the server knows it. Used to subscribe and refresh.
    #[serde(default)]
    pub feed_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodcastEpisode {
    pub id: String,
    pub title: String,
    pub podcast_id: String,
    pub duration_secs: Option<u64>,
    pub stream_url: String,
    pub cover_url: Option<String>,
    pub published: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RadioStation {
    pub id: String,
    pub name: String,
    pub stream_url: String,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResults {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HomeShelf {
    pub id: String,
    pub title: String,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscodeMode {
    Direct,
    #[default]
    Auto,
    Transcode,
}

impl TranscodeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Direct => "Direct",
            Self::Auto => "Auto",
            Self::Transcode => "Transcode",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Direct => Self::Auto,
            Self::Auto => Self::Transcode,
            Self::Transcode => Self::Direct,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Repeat off",
            Self::All => "Repeat all",
            Self::One => "Repeat one",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcode_cycles() {
        assert_eq!(TranscodeMode::Direct.next(), TranscodeMode::Auto);
        assert_eq!(TranscodeMode::Auto.next(), TranscodeMode::Transcode);
        assert_eq!(TranscodeMode::Transcode.next(), TranscodeMode::Direct);
    }

    #[test]
    fn repeat_cycles() {
        assert_eq!(RepeatMode::Off.next(), RepeatMode::All);
        assert_eq!(RepeatMode::One.next(), RepeatMode::Off);
    }
}
