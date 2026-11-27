use crate::model::{RepeatMode, Track, TranscodeMode};
use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// What the UI should do when GStreamer reports end of stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EosAction {
    /// Podcasts stop so the next episode is a choice, not an automatic skip.
    Pause,
    /// Radio steps to the next station in order.
    NextStation,
    /// Library and YouTube Music follow repeat and shuffle.
    Advance,
}

pub fn eos_action(radio: bool, podcast: bool) -> EosAction {
    if podcast {
        EosAction::Pause
    } else if radio {
        EosAction::NextStation
    } else {
        EosAction::Advance
    }
}

/// Media-key / HTTP skip requests. The UI thread drains these and calls `play_track`.
#[derive(Clone, Copy, Debug)]
pub enum SkipRequest {
    Next,
    Previous,
}

fn skip_slot() -> &'static Mutex<Vec<SkipRequest>> {
    static SLOT: OnceLock<Mutex<Vec<SkipRequest>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn request_skip(req: SkipRequest) {
    skip_slot().lock().push(req);
}

pub fn drain_skips() -> Vec<SkipRequest> {
    std::mem::take(&mut *skip_slot().lock())
}

#[derive(Clone, Copy, Debug)]
pub enum WindowRequest {
    Raise,
    Quit,
}

fn window_slot() -> &'static Mutex<Vec<WindowRequest>> {
    static SLOT: OnceLock<Mutex<Vec<WindowRequest>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn request_window(req: WindowRequest) {
    window_slot().lock().push(req);
}

pub fn drain_window_requests() -> Vec<WindowRequest> {
    std::mem::take(&mut *window_slot().lock())
}

#[derive(Clone)]
pub struct Player {
    inner: Arc<Mutex<PlayerInner>>,
    /// When true, souphttpsrc gets the YouTube android User-Agent.
    android_ua: Arc<AtomicBool>,
}

struct PlayerInner {
    pipeline: gst::Element,
    equalizer: Option<gst::Element>,
    queue: Vec<Track>,
    index: usize,
    shuffle: bool,
    repeat: RepeatMode,
    transcode: TranscodeMode,
    volume: f64,
    eq_bands: [f64; 10],
    scrobble_sent: bool,
    play_gen: u64,
    pending_uri: String,
}

impl Player {
    pub fn new() -> Result<Self> {
        gst::init().context("gstreamer init")?;
        let pipeline = gst::ElementFactory::make("playbin3")
            .name("nm-playbin")
            .build()
            .or_else(|_| {
                gst::ElementFactory::make("playbin")
                    .name("nm-playbin")
                    .build()
            })
            .context("create playbin")?;

        crate::eq::ensure_registered();
        let equalizer = gst::ElementFactory::make("nmeq").name("nm-eq").build().ok();
        if let Some(ref eq) = equalizer {
            pipeline.set_property("audio-filter", eq);
            tracing::info!("playback EQ is nmeq (Rust graphic EQ)");
        } else {
            tracing::warn!("nmeq failed to instantiate; playback is unfiltered");
        }

        // YouTube CDN rejects default souphttpsrc UA — stamp android UA when requested.
        let android_ua = Arc::new(AtomicBool::new(false));
        let ua_flag_cb = android_ua.clone();
        pipeline.connect("source-setup", false, move |values| {
            let Ok(source) = values[1].get::<gst::Element>() else {
                return None;
            };
            if ua_flag_cb.load(Ordering::Relaxed) && source.find_property("user-agent").is_some() {
                source.set_property("user-agent", crate::ytm::stream::ANDROID_YT_UA);
            }
            None
        });

        Ok(Self {
            inner: Arc::new(Mutex::new(PlayerInner {
                pipeline,
                equalizer,
                queue: Vec::new(),
                index: 0,
                shuffle: false,
                repeat: RepeatMode::Off,
                transcode: TranscodeMode::Auto,
                volume: 1.0,
                eq_bands: [0.0; 10],
                scrobble_sent: false,
                play_gen: 0,
                pending_uri: String::new(),
            })),
            android_ua,
        })
    }

    pub fn bus(&self) -> gst::Bus {
        self.inner.lock().pipeline.bus().expect("playbin bus")
    }

    pub fn set_queue(&self, tracks: Vec<Track>, start: usize) {
        let mut g = self.inner.lock();
        g.queue = tracks;
        g.index = start.min(g.queue.len().saturating_sub(1));
        g.scrobble_sent = false;
    }

    pub fn queue(&self) -> Vec<Track> {
        self.inner.lock().queue.clone()
    }

    pub fn current(&self) -> Option<Track> {
        let g = self.inner.lock();
        g.queue.get(g.index).cloned()
    }

    pub fn index(&self) -> usize {
        self.inner.lock().index
    }

    pub fn play_uri(&self, uri: &str) -> Result<()> {
        self.play_uri_with_options(uri, false)
    }

    /// Play `uri`. Set `android_ua` for googlevideo streams from yt-dlp's android client.
    pub fn play_uri_with_options(&self, uri: &str, android_ua: bool) -> Result<()> {
        // State changes block, and a stuck stream can deadlock the GTK thread
        // if they run there while the player mutex is held. A later play
        // supersedes an in-flight one.
        self.android_ua.store(android_ua, Ordering::Relaxed);
        let (pipeline, volume, gen) = {
            let mut g = self.inner.lock();
            g.play_gen = g.play_gen.wrapping_add(1);
            g.pending_uri = uri.to_string();
            (g.pipeline.clone(), g.volume, g.play_gen)
        };
        let this = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            let current = {
                let g = this.inner.lock();
                if g.play_gen != gen {
                    return;
                }
                g.pending_uri.clone()
            };
            if current.is_empty() {
                return;
            }
            let _ = pipeline.set_state(gst::State::Null);
            if this.inner.lock().play_gen != gen {
                return;
            }
            pipeline.set_property("uri", current);
            pipeline.set_property("volume", volume);
            let _ = pipeline.set_state(gst::State::Playing);
            crate::mpris::note_changed();
            crate::mpris::note_seek_us(0);
        });
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.inner
            .lock()
            .pipeline
            .set_state(gst::State::Playing)?;
        crate::mpris::note_changed();
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.inner.lock().pipeline.set_state(gst::State::Paused)?;
        crate::mpris::note_changed();
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.inner.lock().pipeline.set_state(gst::State::Null)?;
        crate::mpris::note_changed();
        Ok(())
    }

    pub fn toggle(&self) -> Result<bool> {
        let g = self.inner.lock();
        let (_, state, _) = g.pipeline.state(gst::ClockTime::ZERO);
        let playing = if state == gst::State::Playing {
            g.pipeline.set_state(gst::State::Paused)?;
            false
        } else {
            g.pipeline.set_state(gst::State::Playing)?;
            true
        };
        drop(g);
        crate::mpris::note_changed();
        Ok(playing)
    }

    pub fn is_playing(&self) -> bool {
        let g = self.inner.lock();
        let (_, state, _) = g.pipeline.state(gst::ClockTime::ZERO);
        state == gst::State::Playing
    }

    pub fn seek_secs(&self, secs: u64) -> Result<()> {
        let g = self.inner.lock();
        g.pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_seconds(secs),
        )?;
        drop(g);
        crate::mpris::note_seek_us((secs as i64) * 1_000_000);
        Ok(())
    }

    /// Change playback speed (1.0 = normal) from the current position.
    pub fn set_rate(&self, rate: f64) -> Result<()> {
        let g = self.inner.lock();
        let pos = g
            .pipeline
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        g.pipeline.seek(
            rate.clamp(0.25, 4.0),
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            pos,
            gst::SeekType::None,
            gst::ClockTime::ZERO,
        )?;
        Ok(())
    }

    pub fn position_secs(&self) -> u64 {
        let g = self.inner.lock();
        g.pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.seconds())
            .unwrap_or(0)
    }

    pub fn duration_secs(&self) -> u64 {
        let g = self.inner.lock();
        g.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.seconds())
            .unwrap_or(0)
    }

    pub fn set_volume(&self, vol: f64) {
        let mut g = self.inner.lock();
        g.volume = vol.clamp(0.0, 1.0);
        g.pipeline.set_property("volume", g.volume);
    }

    pub fn volume(&self) -> f64 {
        self.inner.lock().volume
    }

    pub fn set_eq_band(&self, band: usize, db: f64) {
        let mut g = self.inner.lock();
        if band >= 10 {
            return;
        }
        g.eq_bands[band] = db.clamp(crate::eq::GAIN_MIN, crate::eq::GAIN_MAX);
        crate::eq::shared_eq()
            .lock()
            .set_gain(band, g.eq_bands[band]);
    }

    pub fn set_eq_bypass(&self, on: bool) {
        crate::eq::shared_eq().lock().set_bypass(on);
    }

    pub fn eq_bypass(&self) -> bool {
        crate::eq::shared_eq().lock().bypass()
    }

    pub fn eq_bands(&self) -> [f64; 10] {
        self.inner.lock().eq_bands
    }

    pub fn set_shuffle(&self, on: bool) {
        self.inner.lock().shuffle = on;
    }
    pub fn shuffle(&self) -> bool {
        self.inner.lock().shuffle
    }
    pub fn set_repeat(&self, mode: RepeatMode) {
        self.inner.lock().repeat = mode;
    }
    pub fn repeat(&self) -> RepeatMode {
        self.inner.lock().repeat
    }
    pub fn set_transcode(&self, mode: TranscodeMode) {
        self.inner.lock().transcode = mode;
    }
    pub fn transcode(&self) -> TranscodeMode {
        self.inner.lock().transcode
    }

    pub fn add_next(&self, track: Track) {
        let mut g = self.inner.lock();
        let insert_at = (g.index + 1).min(g.queue.len());
        g.queue.insert(insert_at, track);
    }

    pub fn enqueue(&self, track: Track) {
        self.inner.lock().queue.push(track);
    }

    pub fn clear_queue(&self) {
        let mut g = self.inner.lock();
        g.queue.clear();
        g.index = 0;
    }

    /// Advance according to repeat/shuffle. Returns next track if any.
    pub fn advance(&self) -> Option<Track> {
        let mut g = self.inner.lock();
        g.scrobble_sent = false;
        if g.queue.is_empty() {
            return None;
        }
        match g.repeat {
            RepeatMode::One => {
                return g.queue.get(g.index).cloned();
            }
            RepeatMode::All | RepeatMode::Off => {}
        }
        if g.shuffle && g.queue.len() > 1 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            g.index.hash(&mut h);
            std::time::SystemTime::now().hash(&mut h);
            let next = (h.finish() as usize) % g.queue.len();
            g.index = if next == g.index {
                (next + 1) % g.queue.len()
            } else {
                next
            };
        } else if g.index + 1 < g.queue.len() {
            g.index += 1;
        } else if g.repeat == RepeatMode::All {
            g.index = 0;
        } else {
            return None;
        }
        g.queue.get(g.index).cloned()
    }

    pub fn previous_track(&self) -> Option<Track> {
        let mut g = self.inner.lock();
        g.scrobble_sent = false;
        if g.queue.is_empty() {
            return None;
        }
        if self_position_helper(&g) > 3 {
            return g.queue.get(g.index).cloned();
        }
        if g.index > 0 {
            g.index -= 1;
        } else if g.repeat == RepeatMode::All {
            g.index = g.queue.len() - 1;
        }
        g.queue.get(g.index).cloned()
    }

    /// Next queue item, wrapping, ignoring shuffle. Used to step through radio stations.
    pub fn next_in_order(&self) -> Option<Track> {
        let mut g = self.inner.lock();
        if g.queue.is_empty() {
            return None;
        }
        g.scrobble_sent = false;
        g.index = (g.index + 1) % g.queue.len();
        g.queue.get(g.index).cloned()
    }

    /// Move one slot in the queue and stop at either end.
    pub fn step_clamped(&self, delta: i32) -> Option<Track> {
        let mut g = self.inner.lock();
        if g.queue.is_empty() || delta == 0 {
            return None;
        }
        let next = g.index as i32 + delta;
        if next < 0 || next >= g.queue.len() as i32 {
            return None;
        }
        g.scrobble_sent = false;
        g.index = next as usize;
        g.queue.get(g.index).cloned()
    }

    /// Previous queue item, wrapping, ignoring shuffle.
    pub fn prev_in_order(&self) -> Option<Track> {
        let mut g = self.inner.lock();
        if g.queue.is_empty() {
            return None;
        }
        g.scrobble_sent = false;
        if g.index == 0 {
            g.index = g.queue.len() - 1;
        } else {
            g.index -= 1;
        }
        g.queue.get(g.index).cloned()
    }

    pub fn take_scrobble_flag(&self) -> bool {
        let mut g = self.inner.lock();
        if g.scrobble_sent {
            return false;
        }
        let pos = self_position_helper(&g);
        let dur = g
            .pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.seconds())
            .unwrap_or(0);
        if dur > 0 && pos >= dur / 2 {
            g.scrobble_sent = true;
            true
        } else {
            false
        }
    }
}

fn self_position_helper(g: &PlayerInner) -> u64 {
    g.pipeline
        .query_position::<gst::ClockTime>()
        .map(|t| t.seconds())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Track::default()
        }
    }

    #[test]
    fn eos_pauses_podcasts_and_steps_radio() {
        assert_eq!(eos_action(false, true), EosAction::Pause);
        assert_eq!(eos_action(true, false), EosAction::NextStation);
        assert_eq!(eos_action(false, false), EosAction::Advance);
    }

    #[test]
    fn radio_wraps_and_eq_insert_still_reaches_eos_and_seek() {
        let player = Player::new().expect("player");
        player.set_queue(vec![track("one"), track("two")], 0);
        assert_eq!(player.next_in_order().unwrap().id, "two");
        assert_eq!(player.next_in_order().unwrap().id, "one");

        let pipeline = gstreamer::parse::launch(
            "audiotestsrc num-buffers=12 ! audio/x-raw,format=F32LE,rate=48000,channels=2 ! nmeq ! fakesink",
        )
        .expect("nmeq pipeline");
        pipeline
            .set_state(gst::State::Playing)
            .expect("play nmeq");
        let bus = pipeline.bus().expect("bus");
        let msg = bus
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(5),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .expect("pipeline produced no message");
        if let gst::MessageView::Error(err) = msg.view() {
            panic!("nmeq pipeline: {}", err.error());
        }
        let _ = pipeline.set_state(gst::State::Null);

        let path = std::env::temp_dir().join(format!(
            "nm-seek-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let encode = gstreamer::parse::launch(&format!(
            "audiotestsrc num-buffers=120 ! audio/x-raw,rate=48000,channels=2 ! wavenc ! filesink location=\"{}\"",
            path.display()
        ))
        .expect("wav pipeline");
        encode.set_state(gst::State::Playing).expect("encode");
        let encoded = encode
            .bus()
            .expect("encode bus")
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(5),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .expect("encode produced no message");
        if let gst::MessageView::Error(err) = encoded.view() {
            panic!("wav encode: {}", err.error());
        }
        let _ = encode.set_state(gst::State::Null);

        player
            .play_uri(&format!("file://{}", path.display()))
            .expect("play file");
        std::thread::sleep(std::time::Duration::from_millis(500));
        player.seek_secs(1).expect("seek through nmeq");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let pos = player.position_secs();
        let _ = std::fs::remove_file(&path);
        assert!(pos >= 1, "position after seek was {pos}");
    }
}
