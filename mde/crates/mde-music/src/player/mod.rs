use crate::model::{RepeatMode, Track, TranscodeMode};
use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub struct Player {
    inner: Arc<Mutex<PlayerInner>>,
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

        // Try to insert equalizer via audio-filter
        let equalizer = gst::ElementFactory::make("equalizer-10bands")
            .name("nm-eq")
            .build()
            .ok();
        if let Some(ref eq) = equalizer {
            pipeline.set_property("audio-filter", eq);
        }

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
        // State changes block, and a stuck stream can deadlock the GTK thread
        // if they run there while the player mutex is held. A later play
        // supersedes an in-flight one.
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
        });
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.inner
            .lock()
            .pipeline
            .set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.inner.lock().pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.inner.lock().pipeline.set_state(gst::State::Null)?;
        Ok(())
    }

    pub fn toggle(&self) -> Result<bool> {
        let g = self.inner.lock();
        let (_, state, _) = g.pipeline.state(gst::ClockTime::ZERO);
        if state == gst::State::Playing {
            g.pipeline.set_state(gst::State::Paused)?;
            Ok(false)
        } else {
            g.pipeline.set_state(gst::State::Playing)?;
            Ok(true)
        }
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
        g.eq_bands[band] = db.clamp(-24.0, 12.0);
        if let Some(ref eq) = g.equalizer {
            let prop = format!("band{band}");
            eq.set_property_from_str(&prop, &format!("{}", g.eq_bands[band]));
            // equalizer-10bands uses gdouble properties band0..band9
            eq.set_property(&prop, g.eq_bands[band]);
        }
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
