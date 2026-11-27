//! Zero-latency EQ (RBJ cookbook). Playback uses a 10-band graphic of peaking filters
//! inside a GStreamer audio-filter (`nmeq`), which replaces `equalizer-10bands`.
//!
//! Post-decode insert: playbin `audio-filter` → `nmeq` (in-place F32). Soft bypass
//! skips the biquads and leaves the dry samples untouched.

mod filter;

pub use filter::ensure_registered;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;

fn peak_bits() -> &'static AtomicU32 {
    // -120.0_f32 — silence until the audio thread publishes a level.
    static PEAK: AtomicU32 = AtomicU32::new(0xC2F0_0000);
    &PEAK
}

fn last_publish_ms() -> &'static AtomicU64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    &LAST
}

fn now_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64 + 1
}

fn cpu_us() -> &'static AtomicU64 {
    static CPU: AtomicU64 = AtomicU64::new(0);
    &CPU
}

/// Post-EQ peak and process time for the UI. The audio thread stores; the UI loads.
pub fn publish_levels(peak_db: f64, process_us: u64) {
    peak_bits().store((peak_db as f32).to_bits(), Ordering::Relaxed);
    cpu_us().store(process_us, Ordering::Relaxed);
    last_publish_ms().store(now_ms(), Ordering::Relaxed);
}

/// Latest post-EQ peak. Reads as silence when the audio thread has stopped publishing
/// (paused / stopped), instead of freezing on the last loud block.
pub fn snapshot_peak_db() -> f64 {
    peak_or_silence(
        last_publish_ms().load(Ordering::Relaxed),
        now_ms(),
        f32::from_bits(peak_bits().load(Ordering::Relaxed)) as f64,
    )
}

fn peak_or_silence(last_ms: u64, now_ms: u64, peak_db: f64) -> f64 {
    if last_ms == 0 || now_ms.saturating_sub(last_ms) > 400 {
        -120.0
    } else {
        peak_db
    }
}

pub fn snapshot_cpu_us() -> u64 {
    cpu_us().load(Ordering::Relaxed)
}

/// ISO octave centres. (The old list reused a legacy 10-band set whose top three
/// bands sat at 12/14/16 kHz and left 31–500 Hz and 2–8 kHz uncovered.)
pub const BAND_HZ: [f64; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

/// Graphic EQ gain range shown and enforced everywhere (dB).
pub const GAIN_MIN: f64 = -12.0;
pub const GAIN_MAX: f64 = 12.0;
/// Octave-spaced bands overlap correctly at Q ≈ √2.
pub const BAND_Q: f64 = 1.414;

/// 1/3-octave analyzer centres, 25 Hz – 20 kHz.
pub const THIRD_HZ: [f64; 30] = [
    25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0,
    630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_500.0, 16_000.0, 20_000.0,
];

fn goertzel_power(samples: &[f32], freq_hz: f64, sample_rate: f64) -> f64 {
    let n = samples.len().max(1) as f64;
    let w = 2.0 * std::f64::consts::PI * freq_hz / sample_rate;
    let coeff = 2.0 * w.cos();
    let mut s1 = 0.0;
    let mut s2 = 0.0;
    for &sample in samples {
        let s0 = sample as f64 + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let _ = n;
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(1e-18)
}

/// Hann-window a mono block (cuts spectral leakage from the rectangular window).
fn hann(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    if n < 2 {
        return samples.to_vec();
    }
    samples
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
            *x * w as f32
        })
        .collect()
}

/// 1/3-octave levels (dBFS) for a mono block: a full-scale sine reads ≈ 0 dB in its band.
/// Run this on the UI side, never inside the audio callback.
pub fn third_octave_levels(mono: &[f32], rate: f64) -> [f64; 30] {
    let mut out = [-100.0; 30];
    if mono.len() < 1024 {
        return out;
    }
    let windowed = hann(mono);
    // Hann coherent gain is 0.5, so a full-scale sine has amplitude N/4.
    let norm = 20.0 * (mono.len() as f64 / 4.0).log10();
    for (i, hz) in THIRD_HZ.iter().enumerate() {
        if *hz < rate / 2.0 {
            out[i] = (10.0 * goertzel_power(&windowed, *hz, rate).log10() - norm).clamp(-100.0, 6.0);
        }
    }
    out
}

/// Gain for one block. Advanced off ignores dynamic and spectral flags.
/// Dynamic ducks a hot band by 6 dB. Spectral leans the band slightly toward
/// the measured level. There is no Tube or Iron stage.
pub fn block_gain(
    static_gain: f64,
    advanced: bool,
    dynamic: bool,
    spectral: bool,
    band_db: f64,
) -> f64 {
    if !advanced || (!dynamic && !spectral) {
        return static_gain;
    }
    let mut gain = static_gain;
    if dynamic && band_db > -18.0 {
        gain -= 6.0;
    }
    if spectral {
        let lean = (band_db * 0.1).clamp(-6.0, 6.0);
        gain = gain * 0.8 + lean * 0.2;
    }
    gain.clamp(GAIN_MIN, GAIN_MAX)
}

fn band_level_db(samples: &[f32], freq_hz: f64, sample_rate: f64) -> f64 {
    if samples.is_empty() {
        return -120.0;
    }
    let n = samples.len() as f64;
    let power = goertzel_power(samples, freq_hz, sample_rate) / (n * n);
    10.0 * power.max(1e-18).log10()
}

/// Band gains that follow a file's spectrum, relative to its own average.
pub fn gains_from_samples(samples: &[f32], sample_rate: f64) -> [f64; 10] {
    if samples.len() < 256 {
        return [0.0; 10];
    }
    let step = (samples.len() / 48_000).max(1);
    let taken: Vec<f32> = hann(&samples.iter().step_by(step).copied().take(48_000).collect::<Vec<f32>>());
    let power: Vec<f64> = BAND_HZ
        .iter()
        .map(|hz| goertzel_power(&taken, *hz, sample_rate))
        .collect();
    let mean = power.iter().map(|p| p.log10()).sum::<f64>() / power.len() as f64;
    let mut gains = [0.0; 10];
    for (i, p) in power.iter().enumerate() {
        gains[i] = (10.0 * (p.log10() - mean)).clamp(GAIN_MIN, GAIN_MAX);
    }
    gains
}

/// A preset reduced to the ten graphic bands this player can apply.
pub struct LoadedEqPreset {
    pub name: String,
    pub gains: [f64; 10],
    /// Set when the file was written by this player and stored each band’s frequency.
    pub centers: Option<[f64; 10]>,
}

/// OpenFilter `openfilter.eq.preset` v1. Extra `graphic_centers` is ignored by OpenFilter.
pub fn eq_preset_json(name: &str, gains: &[f64; 10]) -> serde_json::Value {
    let bands: Vec<serde_json::Value> = BAND_HZ
        .iter()
        .zip(gains.iter())
        .map(|(freq, gain)| {
            serde_json::json!({
                "shape": "Bell",
                "freq": freq,
                "gain": gain,
                "q": 1.2,
            })
        })
        .collect();
    serde_json::json!({
        "format": "openfilter.eq.preset",
        "version": 1,
        "name": name,
        "author": "Neighbourhood Music",
        "tags": ["graphic"],
        "global": { "phase_mode": "Zero Latency", "character": "Clean" },
        "graphic_centers": true,
        "bands": bands,
    })
}

fn nearest_band(freq: f64) -> (usize, f64) {
    let mut best = (0usize, f64::MAX);
    let freq = freq.max(1.0).ln();
    for (i, hz) in BAND_HZ.iter().enumerate() {
        let dist = (freq - hz.ln()).abs();
        if dist < best.1 {
            best = (i, dist);
        }
    }
    best
}

/// Reads an OpenFilter preset or the older `{gains:[..]}` file.
/// Unknown keys are ignored. Bell, low shelf, and high shelf map onto the nearest band.
pub fn eq_preset_from_json(value: &serde_json::Value) -> Result<LoadedEqPreset, String> {
    if !value.is_object() {
        return Err("preset is not a JSON object".into());
    }
    let name = value
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("preset")
        .to_string();
    if let Some(gains) = value.get("gains").and_then(|g| g.as_array()) {
        if gains.len() == 10 {
            let mut out = [0.0; 10];
            for (i, n) in gains.iter().enumerate() {
                let gain = n.as_f64().ok_or_else(|| "a gain is not a number".to_string())?;
                out[i] = gain.clamp(GAIN_MIN, GAIN_MAX);
            }
            return Ok(LoadedEqPreset {
                name,
                gains: out,
                centers: None,
            });
        }
    }
    let Some(bands) = value.get("bands").and_then(|b| b.as_array()) else {
        return Err("preset has no bands".into());
    };
    if value
        .get("graphic_centers")
        .and_then(|flag| flag.as_bool())
        .unwrap_or(false)
    {
        let mut gains = [0.0; 10];
        let mut centers = BAND_HZ;
        for (i, band) in bands.iter().take(10).enumerate() {
            if let Some(freq) = band.get("freq").and_then(|f| f.as_f64()) {
                centers[i] = freq.clamp(20.0, 20_000.0);
            }
            gains[i] = band
                .get("gain")
                .and_then(|g| g.as_f64())
                .unwrap_or(0.0)
                .clamp(GAIN_MIN, GAIN_MAX);
        }
        return Ok(LoadedEqPreset {
            name,
            gains,
            centers: Some(centers),
        });
    }
    let mut gains = [0.0; 10];
    let mut best = [f64::MAX; 10];
    for band in bands {
        let shape = band.get("shape").and_then(|s| s.as_str()).unwrap_or("Bell");
        if !matches!(shape, "Bell" | "Low Shelf" | "High Shelf") {
            continue;
        }
        let Some(freq) = band.get("freq").and_then(|f| f.as_f64()) else {
            continue;
        };
        let gain = band
            .get("gain")
            .and_then(|g| g.as_f64())
            .unwrap_or(0.0)
            .clamp(GAIN_MIN, GAIN_MAX);
        let (index, dist) = nearest_band(freq);
        if dist < best[index] {
            best[index] = dist;
            gains[index] = gain;
        }
    }
    Ok(LoadedEqPreset {
        name,
        gains,
        centers: None,
    })
}

pub struct GraphicEq {
    gains: [f64; 10],
    bypass: bool,
    rate: f64,
    bands: Vec<StereoPeak>,
    built: [f64; 10],
    built_rate: f64,
    built_centers: [f64; 10],
    peak_db: f64,
    morph_from: [f64; 10],
    morph_to: [f64; 10],
    morph_left: u8,
    advanced: bool,
    dynamic: [bool; 10],
    spectral: [bool; 10],
    delta: bool,
    last_us: u64,
    centers: [f64; 10],
    selected: usize,
    recent: Vec<f32>,
    spectrum: [f64; 10],
    frozen: bool,
    /// Audio callbacks since the last spectrum pass. The display does not need every block.
    spectrum_skip: u32,
}

impl GraphicEq {
    pub fn new() -> Self {
        Self {
            gains: [0.0; 10],
            bypass: false,
            rate: 48_000.0,
            bands: Vec::new(),
            built: [1.0; 10],
            built_rate: 0.0,
            built_centers: [0.0; 10],
            peak_db: -120.0,
            morph_from: [0.0; 10],
            morph_to: [0.0; 10],
            morph_left: 0,
            advanced: false,
            dynamic: [false; 10],
            spectral: [false; 10],
            delta: false,
            last_us: 0,
            centers: BAND_HZ,
            selected: 0,
            recent: Vec::new(),
            spectrum: [-80.0; 10],
            frozen: false,
            spectrum_skip: 0,
        }
    }

    pub fn set_frozen(&mut self, on: bool) {
        self.frozen = on;
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub fn spectrum_db(&self) -> [f64; 10] {
        self.spectrum
    }

    /// Recent post-EQ mono samples and the sample rate, for UI-side analysis.
    /// Copying ~2k floats under the lock is cheap; the FFT-ish work happens outside it.
    pub fn recent_mono(&self) -> (Vec<f32>, f64) {
        (self.recent.chunks(2).map(|f| f[0]).collect(), self.rate)
    }

    /// Downsampled post-EQ waveform for the Stage artwork view.
    pub fn waveform(&self, points: usize) -> Vec<f32> {
        let points = points.max(2);
        if self.recent.is_empty() {
            return vec![0.0; points];
        }
        let step = (self.recent.len() / points).max(1);
        self.recent.iter().step_by(step).take(points).copied().collect()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select_step(&mut self, delta: i32) {
        let next = self.selected as i32 + delta;
        self.selected = next.rem_euclid(10) as usize;
    }

    pub fn nudge_selected(&mut self, delta_db: f64) -> f64 {
        let index = self.selected;
        let next = (self.gains[index] + delta_db).clamp(GAIN_MIN, GAIN_MAX);
        self.set_gain(index, next);
        next
    }

    pub fn set_center(&mut self, index: usize, hz: f64) {
        if index < 10 {
            self.centers[index] = hz.clamp(20.0, 20_000.0);
            self.built_rate = 0.0;
        }
    }

    pub fn center(&self, index: usize) -> f64 {
        self.centers.get(index).copied().unwrap_or(1_000.0)
    }

    pub fn set_delta(&mut self, on: bool) {
        self.delta = on;
    }

    pub fn delta(&self) -> bool {
        self.delta
    }

    pub fn last_us(&self) -> u64 {
        self.last_us
    }

    pub fn set_advanced(&mut self, on: bool) {
        self.advanced = on;
    }

    pub fn advanced(&self) -> bool {
        self.advanced
    }

    pub fn set_dynamic_band(&mut self, index: usize, on: bool) {
        if index < 10 {
            self.dynamic[index] = on;
        }
    }

    pub fn set_spectral_band(&mut self, index: usize, on: bool) {
        if index < 10 {
            self.spectral[index] = on;
        }
    }

    pub fn spectral_band(&self, index: usize) -> bool {
        self.spectral.get(index).copied().unwrap_or(false)
    }

    pub fn dynamic_band(&self, index: usize) -> bool {
        self.dynamic.get(index).copied().unwrap_or(false)
    }

    pub fn begin_morph(&mut self, target: [f64; 10]) {
        self.morph_from = self.gains;
        self.morph_to = target;
        self.morph_left = 8;
    }

    /// One step of an 8-step crossfade (~200 ms if called every 25 ms).
    pub fn step_morph(&mut self) -> bool {
        if self.morph_left == 0 {
            return false;
        }
        self.morph_left -= 1;
        let t = 1.0 - (self.morph_left as f64 / 8.0);
        for i in 0..10 {
            self.gains[i] = self.morph_from[i] * (1.0 - t) + self.morph_to[i] * t;
        }
        self.built_rate = 0.0;
        true
    }

    pub fn set_gain(&mut self, index: usize, db: f64) {
        if index < 10 {
            self.gains[index] = db.clamp(GAIN_MIN, GAIN_MAX);
        }
    }

    pub fn gains(&self) -> [f64; 10] {
        self.gains
    }

    pub fn set_bypass(&mut self, on: bool) {
        self.bypass = on;
    }

    pub fn bypass(&self) -> bool {
        self.bypass
    }

    pub fn set_rate(&mut self, rate: f64) {
        if rate > 1.0 {
            self.rate = rate;
        }
    }

    /// Bring the filters up to date. Coefficients are retuned in place so the filter
    /// memory survives a gain change (rebuilding reset it and clicked on every drag step).
    fn ensure(&mut self) {
        let rate_changed = (self.built_rate - self.rate).abs() >= 1.0;
        if self.bands.len() == 10 && self.built == self.gains && !rate_changed && self.built_centers == self.centers {
            return;
        }
        if self.bands.len() != 10 {
            self.bands = self
                .centers
                .iter()
                .zip(self.gains)
                .map(|(hz, gain)| StereoPeak::new(self.rate, *hz, BAND_Q, gain))
                .collect();
        } else {
            for i in 0..10 {
                self.bands[i].retune(self.rate, self.centers[i], BAND_Q, self.gains[i]);
            }
        }
        self.built = self.gains;
        self.built_rate = self.rate;
        self.built_centers = self.centers;
    }

    pub fn process_interleaved(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }
        if self.bypass {
            self.note_peak(samples);
            publish_levels(self.peak_db, 0);
            return;
        }
        let started = std::time::Instant::now();
        let dry = if self.delta { samples.to_vec() } else { Vec::new() };
        let armed = self.advanced
            && (self.dynamic.iter().any(|on| *on) || self.spectral.iter().any(|on| *on));
        if !armed {
            self.ensure();
            for band in &mut self.bands {
                band.process_interleaved(samples);
            }
        } else {
            self.ensure();
            let mono: Vec<f32> = samples.chunks(2).map(|frame| frame[0]).collect();
            for index in 0..10 {
                let band_db = band_level_db(&mono, self.centers[index], self.rate);
                let gain = block_gain(
                    self.gains[index],
                    true,
                    self.dynamic[index],
                    self.spectral[index],
                    band_db,
                );
                // Retune the persistent band (state kept) instead of building a new one.
                self.bands[index].retune(self.rate, self.centers[index], BAND_Q, gain.clamp(GAIN_MIN, GAIN_MAX));
                self.bands[index].process_interleaved(samples);
            }
            // The next plain block must restore the static gains.
            self.built = [f64::NAN; 10];
        }
        if self.delta {
            for (sample, dry) in samples.iter_mut().zip(dry) {
                *sample = (*sample - dry).clamp(-1.0, 1.0);
            }
        }
        self.note_peak(samples);
        self.capture_spectrum(samples);
        self.last_us = started.elapsed().as_micros() as u64;
        publish_levels(self.peak_db, self.last_us);
        if self.last_us > 2_000 {
            tracing::warn!(us = self.last_us, "EQ process over budget");
        }
    }

    pub fn response_db(&mut self, freq_hz: f64) -> f64 {
        self.ensure();
        self.bands.iter().map(|b| b.response_db(freq_hz, self.rate)).sum()
    }

    pub fn peak_db(&self) -> f64 {
        self.peak_db
    }

    fn capture_spectrum(&mut self, samples: &[f32]) {
        if self.frozen || samples.is_empty() {
            return;
        }
        self.recent.extend_from_slice(samples);
        if self.recent.len() > 4_096 {
            let drop_n = self.recent.len() - 4_096;
            self.recent.drain(0..drop_n);
        }
        // 512 samples, every 8 callbacks. A 256-frame stereo block stays a filter, not an FFT.
        self.spectrum_skip = self.spectrum_skip.saturating_add(1);
        if self.spectrum_skip < 8 || self.recent.len() < 512 {
            return;
        }
        self.spectrum_skip = 0;
        let start = self.recent.len() - 512;
        let window = &self.recent[start..];
        let mono: Vec<f32> = window.chunks(2).map(|frame| frame[0]).collect();
        let mut levels = [0.0; 10];
        for (i, hz) in BAND_HZ.iter().enumerate() {
            let power = goertzel_power(&mono, *hz, self.rate);
            levels[i] = 10.0 * power.log10();
        }
        let peak = levels.into_iter().fold(f64::MIN, f64::max);
        for (i, level) in levels.iter().enumerate() {
            self.spectrum[i] = (level - peak).clamp(-80.0, 0.0);
        }
    }

    fn note_peak(&mut self, samples: &[f32]) {
        let mut peak = 0.0f32;
        for s in samples {
            peak = peak.max(s.abs());
        }
        let db = if peak > 1e-8 {
            20.0 * (peak as f64).log10()
        } else {
            -120.0
        };
        self.peak_db = self.peak_db.max(db) - 0.4;
        if self.peak_db < -120.0 {
            self.peak_db = -120.0;
        }
    }

    pub fn process_mono(&mut self, samples: &mut [f32]) {
        if self.bypass || samples.is_empty() {
            return;
        }
        self.ensure();
        for band in &mut self.bands {
            band.process_mono(samples);
        }
    }
}

pub fn shared_eq() -> &'static Mutex<GraphicEq> {
    static EQ: OnceLock<Mutex<GraphicEq>> = OnceLock::new();
    EQ.get_or_init(|| Mutex::new(GraphicEq::new()))
}

#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    pub fn peaking(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * (freq_hz / sample_rate).clamp(0.0001, 0.49);
        let alpha = w0.sin() / (2.0 * q.max(0.05));
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha / a;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Replace the coefficients, keeping `z1`/`z2` so audio does not click.
    pub fn retune_peaking(&mut self, sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) {
        let fresh = Biquad::peaking(sample_rate, freq_hz, q, gain_db);
        self.b0 = fresh.b0;
        self.b1 = fresh.b1;
        self.b2 = fresh.b2;
        self.a1 = fresh.a1;
        self.a2 = fresh.a2;
    }

    pub fn low_shelf(sample_rate: f64, freq_hz: f64, gain_db: f64) -> Self {
        shelf(sample_rate, freq_hz, gain_db, true)
    }

    pub fn high_shelf(sample_rate: f64, freq_hz: f64, gain_db: f64) -> Self {
        shelf(sample_rate, freq_hz, gain_db, false)
    }

    /// Magnitude of this section at `freq_hz`, in dB.
    pub fn response_db(&self, freq_hz: f64, sample_rate: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * (freq_hz / sample_rate).clamp(0.0001, 0.49);
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = -(self.b1 * s1 + self.b2 * s2);
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = -(self.a1 * s1 + self.a2 * s2);
        let mag = ((num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im).max(1e-24)).sqrt();
        20.0 * mag.max(1e-12).log10()
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }
}

fn shelf(sample_rate: f64, freq_hz: f64, gain_db: f64, low: bool) -> Biquad {
    let a = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * (freq_hz / sample_rate).clamp(0.0001, 0.49);
    let cos = w0.cos();
    let alpha = w0.sin() / 2.0 * ((a + 1.0 / a) * (1.0 / 1.0 - 1.0) + 2.0).sqrt();
    let (b0, b1, b2, a0, a1, a2) = if low {
        (
            a * ((a + 1.0) - (a - 1.0) * cos + 2.0 * a.sqrt() * alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - 2.0 * a.sqrt() * alpha),
            (a + 1.0) + (a - 1.0) * cos + 2.0 * a.sqrt() * alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - 2.0 * a.sqrt() * alpha,
        )
    } else {
        (
            a * ((a + 1.0) + (a - 1.0) * cos + 2.0 * a.sqrt() * alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - 2.0 * a.sqrt() * alpha),
            (a + 1.0) - (a - 1.0) * cos + 2.0 * a.sqrt() * alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - 2.0 * a.sqrt() * alpha,
        )
    };
    Biquad {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
        z1: 0.0,
        z2: 0.0,
    }
}

/// One peaking band applied independently to left and right.
#[derive(Clone, Debug)]
pub struct StereoPeak {
    left: Biquad,
    right: Biquad,
}

impl StereoPeak {
    pub fn new(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> Self {
        Self {
            left: Biquad::peaking(sample_rate, freq_hz, q, gain_db),
            right: Biquad::peaking(sample_rate, freq_hz, q, gain_db),
        }
    }

    pub fn process_interleaved(&mut self, samples: &mut [f32]) {
        let mut i = 0;
        while i + 1 < samples.len() {
            samples[i] = self.left.process(samples[i]);
            samples[i + 1] = self.right.process(samples[i + 1]);
            i += 2;
        }
    }

    pub fn process_mono(&mut self, samples: &mut [f32]) {
        for sample in samples {
            *sample = self.left.process(*sample);
        }
    }

    pub fn response_db(&self, freq_hz: f64, sample_rate: f64) -> f64 {
        self.left.response_db(freq_hz, sample_rate)
    }

    /// New coefficients, same filter memory.
    pub fn retune(&mut self, sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) {
        self.left.retune_peaking(sample_rate, freq_hz, q, gain_db);
        self.right.retune_peaking(sample_rate, freq_hz, q, gain_db);
    }
}

fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    (sum / samples.len() as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The meter atomics are process-global; serialize the tests that touch them.
    static METER_GUARD: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

    fn tone(freq: f64, sr: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sr).sin() as f32)
            .collect()
    }

    /// Audio EQ Cookbook bell, evaluated independently of `Biquad::response_db`.
    /// OpenFilter's static bell uses this same specification. Tolerance is ±0.5 dB.
    fn cookbook_peaking_db(sample_rate: f64, f0: f64, q: f64, gain_db: f64, freq: f64) -> f64 {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * f0 / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha / a;
        let w = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let num_re = b0 + b1 * c1 + b2 * c2;
        let num_im = -(b1 * s1 + b2 * s2);
        let den_re = a0 + a1 * c1 + a2 * c2;
        let den_im = -(a1 * s1 + a2 * s2);
        let mag = ((num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im)).sqrt();
        20.0 * mag.log10()
    }

    #[test]
    fn default_eq_block_stays_under_two_milliseconds() {
        let mut eq = GraphicEq::new();
        eq.set_rate(48_000.0);
        let mut block = vec![0.2f32; 512];
        let mut worst = 0u64;
        for _ in 0..40 {
            eq.process_interleaved(&mut block);
            worst = worst.max(eq.last_us());
        }
        assert!(worst < 2_000, "worst default block was {worst} µs");
    }

    #[test]
    fn short_spectrum_window_still_finds_the_tone() {
        let sr = 48_000.0;
        let mut eq = GraphicEq::new();
        eq.set_rate(sr);
        let mut block: Vec<f32> = (0..256)
            .flat_map(|i| {
                let s = (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32;
                [s, s]
            })
            .collect();
        for _ in 0..16 {
            eq.process_interleaved(&mut block);
        }
        let spec = eq.spectrum_db();
        assert!(
            spec[4] > spec[0],
            "1 kHz band {} vs 60 Hz {}",
            spec[4],
            spec[0]
        );
    }

    #[test]
    fn oracle_peaking_matches_cookbook_within_half_db() {
        let sr = 48_000.0;
        let filt = Biquad::peaking(sr, 1_000.0, 1.0, 12.0);
        for freq in [100.0, 1_000.0, 4_000.0] {
            let got = filt.response_db(freq, sr);
            let expect = cookbook_peaking_db(sr, 1_000.0, 1.0, 12.0, freq);
            let err = (got - expect).abs();
            assert!(err < 0.5, "{freq} Hz engine {got} cookbook {expect} err {err}");
        }
        let mut tone_in = tone(1_000.0, sr, 48_000);
        let dry = rms(&tone_in[24_000..]);
        let mut band = StereoPeak::new(sr, 1_000.0, 1.0, 12.0);
        band.process_mono(&mut tone_in);
        let wet = rms(&tone_in[24_000..]);
        let measured = 20.0 * (wet / dry).log10();
        assert!(
            (measured - 12.0).abs() < 1.0,
            "processed 1 kHz sine was {measured} dB, expected 12 ± 1"
        );
    }

    #[test]
    fn peaking_boosts_target_more_than_distant_tone() {
        let sr = 48_000.0;
        let mut near = tone(1000.0, sr, 48_000);
        let mut far = tone(100.0, sr, 48_000);
        // Interleave mono as stereo pairs.
        let near_st: Vec<f32> = near.iter().flat_map(|s| [*s, *s]).collect();
        let far_st: Vec<f32> = far.iter().flat_map(|s| [*s, *s]).collect();
        let dry_near = rms(&near_st[24_000..]);
        let dry_far = rms(&far_st[24_000..]);
        let mut eq = StereoPeak::new(sr, 1000.0, 1.0, 12.0);
        near = near_st;
        far = far_st;
        eq.process_interleaved(&mut near);
        let mut eq = StereoPeak::new(sr, 1000.0, 1.0, 12.0);
        eq.process_interleaved(&mut far);
        let wet_near = rms(&near[24_000..]);
        let wet_far = rms(&far[24_000..]);
        let near_db = 20.0 * (wet_near / dry_near).log10();
        let far_db = 20.0 * (wet_far / dry_far).log10();
        assert!(near_db > 6.0, "1 kHz boost was {near_db} dB");
        assert!(far_db.abs() < 3.0, "100 Hz moved {far_db} dB");
    }

    #[test]
    fn low_shelf_boosts_bass_more_than_treble() {
        let sr = 48_000.0;
        let bass = tone(80.0, sr, 24_000);
        let treble = tone(8000.0, sr, 24_000);
        let mut filt = Biquad::low_shelf(sr, 200.0, 9.0);
        let wet_b: Vec<f32> = bass.iter().map(|s| filt.process(*s)).collect();
        let mut filt = Biquad::low_shelf(sr, 200.0, 9.0);
        let wet_t: Vec<f32> = treble.iter().map(|s| filt.process(*s)).collect();
        let b = 20.0 * (rms(&wet_b[8000..]) / rms(&bass[8000..])).log10();
        let t = 20.0 * (rms(&wet_t[8000..]) / rms(&treble[8000..])).log10();
        assert!(b > t + 3.0, "bass {b} dB vs treble {t} dB");
    }

    #[test]
    fn openfilter_preset_roundtrip_and_upstream_bell() {
        let gains = [1.0, 0.0, -2.0, 0.0, 3.0, 0.0, 0.0, 1.5, 0.0, -1.0];
        let saved = eq_preset_json("Air", &gains);
        let back = eq_preset_from_json(&saved).expect("round trip");
        assert_eq!(back.name, "Air");
        for i in 0..10 {
            assert!((back.gains[i] - gains[i]).abs() < 0.01, "band {i}");
        }
        assert!(back.centers.is_some());
        let upstream = serde_json::json!({
            "format": "openfilter.eq.preset",
            "version": 1,
            "name": "Vocal",
            "mystery": { "kept": false },
            "global": { "character": "Tube", "phase_mode": "Natural", "drive": 30 },
            "bands": [
                { "shape": "Bell", "freq": 1100, "gain": -1.5, "q": 2.2, "dyn": "Dynamic" },
                { "shape": "Low Cut", "slope": 24, "freq": 80 },
                { "shape": "NotAShape", "freq": 100, "gain": 9 }
            ]
        });
        let loaded = eq_preset_from_json(&upstream).expect("upstream");
        assert!((loaded.gains[5] - -1.5).abs() < 0.01, "1 kHz {:?}", loaded.gains);
        assert!(loaded.gains.iter().enumerate().all(|(i, g)| i == 5 || *g == 0.0));
        assert!(loaded.centers.is_none());
    }

    #[test]
    fn advanced_off_ignores_dynamic_and_spectral() {
        assert_eq!(block_gain(8.0, false, true, true, 0.0), 8.0);
        assert_eq!(block_gain(8.0, true, false, false, 0.0), 8.0);
        assert!(block_gain(12.0, true, true, false, -6.0) < 12.0);
        let spectral = block_gain(0.0, true, false, true, -3.0);
        assert!(spectral.abs() > 0.0 && spectral.abs() < 6.0, "{spectral}");
    }

    #[test]
    fn dynamic_band_ducks_a_loud_tone_only_when_advanced() {
        let sr = 48_000.0;
        let tone: Vec<f32> = (0..4_000)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32 * 0.6)
            .flat_map(|s| [s, s])
            .collect();
        let mut plain = GraphicEq::new();
        plain.set_rate(sr);
        plain.set_gain(5, 12.0);
        plain.set_dynamic_band(5, true);
        let mut ducked = GraphicEq::new();
        ducked.set_rate(sr);
        ducked.set_gain(5, 12.0);
        ducked.set_dynamic_band(5, true);
        ducked.set_advanced(true);
        let mut dry = tone.clone();
        plain.process_interleaved(&mut dry);
        let mut wet = tone;
        ducked.process_interleaved(&mut wet);
        let plain_rms = rms(&dry[2_000..]);
        let ducked_rms = rms(&wet[2_000..]);
        assert!(
            ducked_rms < plain_rms * 0.8,
            "ducked {ducked_rms} vs static {plain_rms}"
        );
    }

    #[test]
    fn meter_reads_silence_when_the_audio_thread_goes_quiet() {
        assert_eq!(peak_or_silence(0, 5_000, -3.0), -120.0, "never published");
        assert_eq!(peak_or_silence(1_000, 1_200, -3.0), -3.0, "fresh");
        assert_eq!(peak_or_silence(1_000, 1_500, -3.0), -120.0, "stale after 400 ms");
    }

    #[test]
    fn meter_snapshot_roundtrips_without_the_eq_lock() {
        let _g = METER_GUARD.lock();
        publish_levels(-12.5, 40);
        let db = snapshot_peak_db();
        assert!((db - -12.5).abs() < 0.05, "{db}");
        assert_eq!(snapshot_cpu_us(), 40);
    }

    #[test]
    fn file_match_boosts_the_tone_that_is_present() {
        let sr = 48_000.0;
        let samples: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32)
            .collect();
        let gains = gains_from_samples(&samples, sr);
        let low = gains[0];
        let mid = gains[5];
        assert!(mid > low + 3.0, "1 kHz {mid} vs 31.5 Hz {low}");
    }

    #[test]
    fn boosting_a_band_raises_a_tone_at_that_band_by_about_the_gain() {
        let sr = 48_000.0;
        let n = 24_000;
        for (index, hz) in BAND_HZ.iter().enumerate() {
            let tone: Vec<f32> = (0..n)
                .map(|i| (2.0 * std::f64::consts::PI * hz * i as f64 / sr).sin() as f32 * 0.25)
                .flat_map(|s| [s, s])
                .collect();
            let mut eq = GraphicEq::new();
            eq.set_rate(sr);
            eq.set_gain(index, 9.0);
            let mut out = tone.clone();
            eq.process_interleaved(&mut out);
            let gain_db = 20.0 * (rms(&out[n..]) / rms(&tone[n..])).log10();
            // Neighbouring-band overlap is zero here (only this band is set).
            assert!((gain_db - 9.0).abs() < 0.7, "band {index} ({hz} Hz) gave {gain_db:.2} dB");
        }
    }

    #[test]
    fn changing_gain_keeps_filter_memory_so_audio_does_not_click() {
        let sr = 48_000.0;
        let tone: Vec<f32> = (0..9_600)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32 * 0.5)
            .flat_map(|s| [s, s])
            .collect();
        let mut eq = GraphicEq::new();
        eq.set_rate(sr);
        eq.set_gain(5, 6.0);
        let mut a = tone[..9_600].to_vec();
        eq.process_interleaved(&mut a);
        eq.set_gain(5, 7.0); // a drag step
        let mut b = tone[9_600..].to_vec();
        eq.process_interleaved(&mut b);
        // The jump across the block boundary must stay small relative to the signal.
        let step = (b[0] - a[a.len() - 2]).abs();
        assert!(step < 0.35, "discontinuity {step}");
    }

    #[test]
    fn third_octave_analyzer_reads_a_full_scale_tone_near_zero_dbfs() {
        let sr = 48_000.0;
        let mut eq = GraphicEq::new();
        eq.set_rate(sr);
        let tone: Vec<f32> = (0..4_096)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32)
            .flat_map(|s| [s, s])
            .collect();
        for chunk in tone.chunks(512) {
            let mut c = chunk.to_vec();
            eq.process_interleaved(&mut c);
        }
        let (mono, rate) = eq.recent_mono();
        let bands = third_octave_levels(&mono, rate);
        let k = THIRD_HZ.iter().position(|h| *h == 1_000.0).unwrap();
        assert!(bands[k] > -4.0 && bands[k] < 3.0, "1 kHz band reads {:.1} dBFS", bands[k]);
        assert!(bands[k] > bands[3] + 25.0, "50 Hz band should be far below");
    }
}
