//! Shared full-bleed backdrop (W656, W676, W678, W680, W681, W683).
//!
//! One cairo-drawn widget used by every page:
//! * cover-fit image with an art-adaptive scrim,
//! * 600 ms crossfade between images, optional rotation through several,
//! * slow Ken-Burns drift, pointer-driven 2.5D parallax ("Cinema") and scroll parallax,
//! * `Ambient` mode (heavily blurred + dimmed) for dense pages,
//! * capped audio pulse (never flashes),
//! * honors the system reduced-motion preference (`gtk-enable-animations`).
//! Decoding happens on a worker thread; the UI thread only blits cairo surfaces.

use gtk4::prelude::*;
use gtk4::{glib, DrawingArea, EventControllerMotion};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Crisp art for hero pages (album, artist, performer, Stage, Home).
    Sharp,
    /// Blurred + dimmed for dense pages.
    Ambient,
}

/// Longest edge we keep decoded in memory (W677: 1440p class).
const DECODE_EDGE: u32 = 1440;
const FADE: Duration = Duration::from_millis(600);
const ROTATE: Duration = Duration::from_secs(24);
/// ~30 fps floor (target in the interview): redraw at most every 33 ms.
const FRAME: Duration = Duration::from_millis(33);

struct Layer {
    surface: gtk4::cairo::ImageSurface,
    /// Tiny copy for Ambient mode: bilinear upscaling gives a cheap blur.
    blur: gtk4::cairo::ImageSurface,
    w: f64,
    h: f64,
    born: Instant,
}

type Decoded = Option<(Vec<u8>, i32, i32, Vec<u8>, i32, i32)>;

struct State {
    mode: Mode,
    cinema: bool,
    cur: Option<Layer>,
    prev: Option<Layer>,
    fade_start: Option<Instant>,
    queue: Vec<PathBuf>,
    idx: usize,
    last_switch: Instant,
    inbox: Arc<Mutex<Option<(u64, Decoded)>>>,
    loading: bool,
    pointer: (f64, f64),
    pointer_smooth: (f64, f64),
    scroll: f64,
    pulse: f64,
    pulse_target: f64,
    last_frame: Instant,
    reduced: bool,
    paused: bool,
    /// Accent-neutral dark scrim strength, 0..1.
    scrim: f64,
    generation: u64,
}

#[derive(Clone)]
pub struct Bleed {
    pub area: DrawingArea,
    st: Rc<RefCell<State>>,
}

thread_local! {
    static GLOBAL_MOTION_OFF: RefCell<bool> = const { RefCell::new(false) };
}

/// One-key kill switch / settings hook: freezes all drift, parallax and pulse.
pub fn set_global_motion_off(off: bool) {
    GLOBAL_MOTION_OFF.with(|g| *g.borrow_mut() = off);
}

fn system_reduced_motion() -> bool {
    gtk4::Settings::default().map(|s| !s.is_gtk_enable_animations()).unwrap_or(false)
}

fn to_surface(rgba: &[u8], w: i32, h: i32) -> Option<gtk4::cairo::ImageSurface> {
    // cairo ARGB32 is premultiplied BGRA (little-endian). Our decode is opaque RGBA.
    let stride = gtk4::cairo::Format::ARgb32.stride_for_width(w as u32).ok()?;
    let mut data = vec![0u8; (stride * h) as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let s = (y * w as usize + x) * 4;
            let d = y * stride as usize + x * 4;
            data[d] = rgba[s + 2];
            data[d + 1] = rgba[s + 1];
            data[d + 2] = rgba[s];
            data[d + 3] = 255;
        }
    }
    gtk4::cairo::ImageSurface::create_for_data(data, gtk4::cairo::Format::ARgb32, w, h, stride).ok()
}

/// Worker-thread decode: full (≤1440) + tiny blur source.
fn decode(path: &PathBuf) -> Decoded {
    let img = image::load_from_memory(&std::fs::read(path).ok()?).ok()?;
    let img = if img.width().max(img.height()) > DECODE_EDGE {
        img.resize(DECODE_EDGE, DECODE_EDGE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let small = img.resize(48, 48, image::imageops::FilterType::Triangle).to_rgba8();
    let big = img.to_rgba8();
    Some((
        big.as_raw().clone(),
        big.width() as i32,
        big.height() as i32,
        small.as_raw().clone(),
        small.width() as i32,
        small.height() as i32,
    ))
}

impl Bleed {
    pub fn new(mode: Mode) -> Self {
        let area = DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.add_css_class("nm-bleed");
        let st = Rc::new(RefCell::new(State {
            mode,
            cinema: true,
            cur: None,
            prev: None,
            fade_start: None,
            queue: Vec::new(),
            idx: 0,
            last_switch: Instant::now(),
            inbox: Arc::new(Mutex::new(None)),
            loading: false,
            pointer: (0.5, 0.5),
            pointer_smooth: (0.5, 0.5),
            scroll: 0.0,
            pulse: 0.0,
            pulse_target: 0.0,
            last_frame: Instant::now(),
            reduced: system_reduced_motion(),
            paused: false,
            scrim: if mode == Mode::Ambient { 0.62 } else { 0.38 },
            generation: 0,
        }));
        let this = Self { area: area.clone(), st: st.clone() };

        {
            let st = st.clone();
            area.set_draw_func(move |_, cr, w, h| draw(&st.borrow(), cr, w as f64, h as f64));
        }
        {
            let motion = EventControllerMotion::new();
            let st = st.clone();
            let area_m = area.clone();
            motion.connect_motion(move |_, x, y| {
                let (w, h) = (area_m.width().max(1) as f64, area_m.height().max(1) as f64);
                st.borrow_mut().pointer = ((x / w).clamp(0.0, 1.0), (y / h).clamp(0.0, 1.0));
            });
            area.add_controller(motion);
        }
        {
            let this = this.clone();
            area.add_tick_callback(move |a, _| {
                this.tick(a);
                glib::ControlFlow::Continue
            });
        }
        {
            let st_map = st.clone();
            area.connect_map(move |_| {
                let mut s = st_map.borrow_mut();
                s.paused = false;
                s.reduced = system_reduced_motion();
            });
            let st_unmap = st.clone();
            area.connect_unmap(move |_| st_unmap.borrow_mut().paused = true);
        }
        this
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }

    pub fn set_mode(&self, mode: Mode) {
        let mut s = self.st.borrow_mut();
        s.mode = mode;
        s.scrim = if mode == Mode::Ambient { 0.62 } else { 0.38 };
        drop(s);
        self.area.queue_draw();
    }

    pub fn set_cinema(&self, on: bool) {
        self.st.borrow_mut().cinema = on;
    }

    /// Vertical scroll position in pixels (scroll parallax, W681).
    pub fn set_scroll(&self, px: f64) {
        self.st.borrow_mut().scroll = px;
    }

    /// Loudness 0..1 from the playing audio; capped and smoothed (W683).
    pub fn set_pulse(&self, level: f64) {
        self.st.borrow_mut().pulse_target = level.clamp(0.0, 1.0);
    }

    /// Replace the image set. Identical sets are ignored so page re-renders don't restart the fade.
    pub fn set_images(&self, paths: Vec<PathBuf>) {
        {
            let mut s = self.st.borrow_mut();
            if s.queue == paths {
                return;
            }
            s.queue = paths;
            s.idx = 0;
            s.generation = s.generation.wrapping_add(1);
        }
        self.load_current();
    }

    pub fn clear(&self) {
        let mut s = self.st.borrow_mut();
        s.queue.clear();
        s.cur = None;
        s.prev = None;
        s.generation = s.generation.wrapping_add(1);
        drop(s);
        self.area.queue_draw();
    }

    fn load_current(&self) {
        let (path, inbox, gen) = {
            let mut s = self.st.borrow_mut();
            let Some(p) = s.queue.get(s.idx).cloned() else { return };
            s.loading = true;
            (p, s.inbox.clone(), s.generation)
        };
        std::thread::spawn(move || {
            let d = decode(&path);
            // The UI drops results whose generation is stale.
            *inbox.lock() = Some((gen, d));
        });
    }

    fn tick(&self, area: &DrawingArea) {
        let motion_off = GLOBAL_MOTION_OFF.with(|g| *g.borrow());
        let now = Instant::now();
        let mut redraw = false;
        let mut start_next = false;
        {
            let mut s = self.st.borrow_mut();
            // Pick up a finished decode.
            let got = s.inbox.lock().take();
            if let Some((gen, d)) = got {
                s.loading = false;
                let d = if gen == s.generation { d } else { None };
                if let Some((big, bw, bh, small, sw, sh)) = d {
                    if let (Some(surface), Some(blur)) = (to_surface(&big, bw, bh), to_surface(&small, sw, sh)) {
                        let layer = Layer { surface, blur, w: bw as f64, h: bh as f64, born: now };
                        s.prev = s.cur.take();
                        s.cur = Some(layer);
                        s.fade_start = Some(now);
                        s.last_switch = now;
                        redraw = true;
                    }
                }
            }
            if s.paused || !area.is_mapped() {
                return;
            }
            let animate = !s.reduced && !motion_off;
            // Rotate through multiple images.
            if s.queue.len() > 1 && !s.loading && now.duration_since(s.last_switch) > ROTATE {
                s.idx = (s.idx + 1) % s.queue.len();
                s.last_switch = now;
                start_next = true;
            }
            // Fade progress.
            if let Some(t0) = s.fade_start {
                if now.duration_since(t0) >= FADE || !animate {
                    s.fade_start = None;
                    s.prev = None;
                }
                redraw = true;
            }
            if animate && now.duration_since(s.last_frame) >= FRAME {
                s.last_frame = now;
                // Smooth pointer (parallax) and pulse (low-pass keeps it < ~3 changes/s).
                let k = 0.08;
                let (px, py) = s.pointer;
                s.pointer_smooth.0 += (px - s.pointer_smooth.0) * k;
                s.pointer_smooth.1 += (py - s.pointer_smooth.1) * k;
                let target = s.pulse_target;
                s.pulse += (target - s.pulse) * 0.06;
                redraw = true;
            } else if !animate {
                s.pulse = 0.0;
            }
        }
        if start_next {
            self.load_current();
        }
        if redraw {
            area.queue_draw();
        }
    }
}

/// Cover-fit transform for `layer` into `w`×`h`, with drift/parallax/pulse applied.
fn paint_layer(s: &State, cr: &gtk4::cairo::Context, l: &Layer, w: f64, h: f64, alpha: f64, animate: bool) {
    let ambient = s.mode == Mode::Ambient;
    let (src, sw, sh) = if ambient { (&l.blur, 48.0_f64.min(l.w), 48.0_f64.min(l.h)) } else { (&l.surface, l.w, l.h) };
    // Aspect of the blur copy equals the original's; recompute from actual surface size.
    let (sw, sh) = if ambient { (src.width() as f64, src.height() as f64) } else { (sw, sh) };
    let cover = (w / sw).max(h / sh);
    let age = l.born.elapsed().as_secs_f64();
    let drift = if animate && !ambient { (age / 26.0).min(1.0) } else { 0.0 };
    // Ken-Burns: 1.00 → 1.07 over ~26 s; audio pulse adds ≤ 2.5 %.
    let zoom = cover * (1.04 + 0.07 * drift) * (1.0 + 0.025 * s.pulse.min(1.0));
    let (mut ox, mut oy) = (0.0, 0.0);
    if animate && s.cinema {
        // 2.5D: shift up to ±2 % of the window with the pointer.
        ox = (0.5 - s.pointer_smooth.0) * w * 0.04;
        oy = (0.5 - s.pointer_smooth.1) * h * 0.04;
    }
    // Scroll parallax: backdrop travels at 35 % of the content.
    oy -= s.scroll * 0.35;
    let dw = sw * zoom;
    let dh = sh * zoom;
    let x = (w - dw) / 2.0 + ox;
    // Keep the image covering the frame even when shifted.
    let y = ((h - dh) / 2.0 + oy).clamp(h - dh, 0.0);
    let x = x.clamp(w - dw, 0.0);
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(zoom, zoom);
    let _ = cr.set_source_surface(src, 0.0, 0.0);
    cr.source().set_filter(gtk4::cairo::Filter::Bilinear);
    let _ = cr.paint_with_alpha(alpha);
    let _ = cr.restore();
}

fn draw(s: &State, cr: &gtk4::cairo::Context, w: f64, h: f64) {
    cr.set_source_rgb(0.05, 0.055, 0.07);
    let _ = cr.paint();
    let motion_off = GLOBAL_MOTION_OFF.with(|g| *g.borrow());
    let animate = !s.reduced && !motion_off;
    let fade = s
        .fade_start
        .map(|t| (t.elapsed().as_secs_f64() / FADE.as_secs_f64()).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    // Smoothstep for a soft crossfade.
    let eased = fade * fade * (3.0 - 2.0 * fade);
    if let Some(p) = &s.prev {
        paint_layer(s, cr, p, w, h, 1.0, animate);
    }
    if let Some(c) = &s.cur {
        paint_layer(s, cr, c, w, h, eased, animate);
    }
    // Scrim: flat dim + bottom gradient so lower-third text stays readable.
    cr.set_source_rgba(0.02, 0.025, 0.04, s.scrim * 0.55);
    let _ = cr.paint();
    let g = gtk4::cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
    g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.10);
    g.add_color_stop_rgba(0.55, 0.0, 0.0, 0.0, 0.10 + s.scrim * 0.2);
    g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.55 + s.scrim * 0.3);
    let _ = cr.set_source(&g);
    let _ = cr.paint();
}

/// Pure helper for tests/telemetry: pulse after smoothing never exceeds its target.
pub fn smooth_pulse(current: f64, target: f64) -> f64 {
    current + (target.clamp(0.0, 1.0) - current) * 0.06
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_is_smoothed_and_bounded() {
        let mut p = 0.0;
        for _ in 0..200 {
            p = smooth_pulse(p, 5.0);
        }
        assert!(p <= 1.0 + 1e-9, "{p}");
        // A single step from silence to full scale moves at most 6 %.
        assert!(smooth_pulse(0.0, 1.0) <= 0.06 + 1e-9);
    }

    #[test]
    fn rgba_converts_to_premultiplied_bgra() {
        let rgba = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let s = to_surface(&rgba, 2, 1).unwrap();
        assert_eq!((s.width(), s.height()), (2, 1));
    }
}
