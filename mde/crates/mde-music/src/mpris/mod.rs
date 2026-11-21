//! MPRIS2 player interface for media keys / AVRCP / lock-screen controls.

use crate::model::Track;
use crate::player::Player;
use anyhow::Result;
use std::collections::HashMap;
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection};

pub struct MprisHub {
    _conn: Connection,
}

struct Root;
struct PlayerIface {
    player: Player,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Neighbourhood Music"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "neighbourhood-music"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["http".into(), "https".into()]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec!["audio/mpeg".into(), "audio/flac".into(), "audio/ogg".into()]
    }
    fn raise(&self) {}
    fn quit(&self) {}
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerIface {
    fn next(&self) {
        let _ = self.player.advance();
    }
    fn previous(&self) {
        let _ = self.player.previous_track();
    }
    fn pause(&self) {
        let _ = self.player.pause();
    }
    fn play_pause(&self) {
        let _ = self.player.toggle();
    }
    fn stop(&self) {
        let _ = self.player.stop();
    }
    fn play(&self) {
        let _ = self.player.play();
    }
    fn seek(&self, offset: i64) {
        let pos = self.player.position_secs() as i64 + offset / 1_000_000;
        let _ = self.player.seek_secs(pos.max(0) as u64);
    }
    fn set_position(&self, _track_id: zvariant::ObjectPath<'_>, position: i64) {
        let _ = self.player.seek_secs((position / 1_000_000).max(0) as u64);
    }
    fn open_uri(&self, uri: &str) {
        let _ = self.player.play_uri(uri);
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        if self.player.is_playing() {
            "Playing".into()
        } else if self.player.current().is_some() {
            "Paused".into()
        } else {
            "Stopped".into()
        }
    }

    #[zbus(property)]
    fn loop_status(&self) -> String {
        match self.player.repeat() {
            crate::model::RepeatMode::Off => "None",
            crate::model::RepeatMode::All => "Playlist",
            crate::model::RepeatMode::One => "Track",
        }
        .into()
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.player.shuffle()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, zvariant::Value<'_>> {
        track_metadata(self.player.current().as_ref())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.player.volume()
    }

    #[zbus(property)]
    fn set_volume(&self, vol: f64) {
        self.player.set_volume(vol);
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        (self.player.position_secs() as i64) * 1_000_000
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;
}

fn track_metadata(track: Option<&Track>) -> HashMap<String, zvariant::Value<'static>> {
    let mut m = HashMap::new();
    let Some(t) = track else {
        return m;
    };
    m.insert(
        "mpris:trackid".into(),
        zvariant::Value::new(format!("/org/mde/NeighbourhoodMusic/{}", t.id)),
    );
    if let Some(d) = t.duration_secs {
        m.insert("mpris:length".into(), zvariant::Value::new((d as i64) * 1_000_000));
    }
    if let Some(ref art) = t.cover_url {
        m.insert("mpris:artUrl".into(), zvariant::Value::new(art.clone()));
    }
    m.insert("xesam:title".into(), zvariant::Value::new(t.title.clone()));
    m.insert(
        "xesam:artist".into(),
        zvariant::Value::new(vec![t.artist.clone()]),
    );
    m.insert("xesam:album".into(), zvariant::Value::new(t.album.clone()));
    m
}

pub async fn start(player: Player) -> Result<MprisHub> {
    let root = Root;
    let iface = PlayerIface { player };
    let conn = Builder::session()?
        .name("org.mpris.MediaPlayer2.NeighbourhoodMusic")?
        .serve_at("/org/mpris/MediaPlayer2", root)?
        .serve_at("/org/mpris/MediaPlayer2", iface)?
        .build()
        .await?;
    Ok(MprisHub { _conn: conn })
}

pub fn spawn(player: Player) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        if let Ok(rt) = rt {
            let _ = rt.block_on(start(player));
            // Keep the connection alive
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
    });
}
