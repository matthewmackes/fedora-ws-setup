//! Stage Audiophile surface.
//!
//! Graphic tab: interactive response graph (log-frequency grid, dB scale, draggable
//! band handles, scroll/keyboard control, live spectrum behind the curve) over ten
//! sliders. Other tabs: Bands, Analyzer (1/3-octave with peak hold), Dynamics,
//! Match and Presets. Everything scrolls vertically and never forces the window wider.

use crate::eq::{shared_eq, BAND_HZ, GAIN_MAX, GAIN_MIN, THIRD_HZ};
use crate::ui::shell::AppState;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DrawingArea, Label, Orientation, ScrolledWindow, Stack,
    ToggleButton,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const F_MIN: f64 = 20.0;
const F_MAX: f64 = 20_000.0;
const PL: f64 = 40.0; // plot margins
const PR: f64 = 14.0;
const PT: f64 = 14.0;
const PB: f64 = 26.0;

const BAND_LABELS: [&str; 10] = ["31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

fn fmt_hz(hz: f64) -> String {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        if (k - k.round()).abs() < 0.05 {
            format!("{:.0} kHz", k)
        } else {
            format!("{:.1} kHz", k)
        }
    } else if (hz - hz.round()).abs() < 0.05 {
        format!("{:.0} Hz", hz)
    } else {
        format!("{:.1} Hz", hz)
    }
}

fn fmt_db(db: f64) -> String {
    if db.abs() < 0.05 {
        "0 dB".to_string()
    } else {
        format!("{:+.1} dB", db)
    }
}

/// Live analyzer data shared by the plot overlay and the Analyzer tab.
pub struct Spectrum {
    pub levels: [f64; 30],
    pub peaks: [f64; 30],
    pub live: bool,
}

impl Spectrum {
    fn new() -> Self {
        Self { levels: [-100.0; 30], peaks: [-100.0; 30], live: false }
    }
}

/// Pull the latest post-EQ audio and update bars (attack fast, release slow, peak hold).
fn update_spectrum(sp: &Rc<RefCell<Spectrum>>) {
    let frozen = shared_eq().lock().frozen();
    if frozen {
        return;
    }
    let (mono, rate) = shared_eq().lock().recent_mono();
    let fresh = crate::eq::third_octave_levels(&mono, rate);
    let mut s = sp.borrow_mut();
    s.live = fresh.iter().any(|v| *v > -90.0);
    for i in 0..30 {
        let target = fresh[i];
        s.levels[i] = if target > s.levels[i] {
            s.levels[i] + (target - s.levels[i]) * 0.6
        } else {
            s.levels[i] + (target - s.levels[i]) * 0.18
        };
        s.peaks[i] = (s.peaks[i] - 0.8).max(s.levels[i]);
    }
}

pub fn build(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 10);
    page.add_css_class("nm-main");
    page.add_css_class("nm-audiophile");
    page.set_hexpand(true);
    page.set_vexpand(true);

    let spectrum = Rc::new(RefCell::new(Spectrum::new()));
    {
        let sp = spectrum.clone();
        let page_w = page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
            if page_w.is_mapped() {
                update_spectrum(&sp);
            }
            glib::ControlFlow::Continue
        });
    }

    let tabs = GtkBox::new(Orientation::Horizontal, 6);
    tabs.set_halign(Align::Center);
    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_hexpand(true);
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_transition_duration(160);
    let mut buttons: Vec<(&'static str, ToggleButton)> = Vec::new();
    for (id, label) in [
        ("graphic", "Graphic"),
        ("bands", "Bands"),
        ("analyzer", "Analyzer"),
        ("dynamics", "Dynamics"),
        ("match", "Match"),
        ("presets", "Presets"),
    ] {
        let b = ToggleButton::with_label(label);
        b.add_css_class("nm-chip");
        if let Some((_, first)) = buttons.first() {
            b.set_group(Some(first));
        }
        tabs.append(&b);
        buttons.push((id, b));
    }
    for (id, b) in &buttons {
        let stack = stack.clone();
        let id = *id;
        b.connect_toggled(move |b| {
            if b.is_active() {
                stack.set_visible_child_name(id);
            }
        });
    }
    page.append(&tabs);

    let add = |name: &str, w: &GtkBox| {
        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_propagate_natural_width(false);
        scroll.set_propagate_natural_height(false);
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        w.set_margin_end(6);
        scroll.set_child(Some(w));
        stack.add_named(&scroll, Some(name));
    };
    add("graphic", &graphic_page(state.clone(), spectrum.clone()));
    add("bands", &bands_page(state.clone()));
    add("analyzer", &analyzer_page(spectrum.clone()));
    add("dynamics", &dynamics_page());
    add("match", &match_page(state.clone(), spectrum.clone()));
    add("presets", &presets_page(state));
    stack.set_visible_child_name("graphic");
    if let Some((_, first)) = buttons.first() {
        first.set_active(true);
    }
    page.append(&stack);
    page
}

fn hint(text: &str) -> Label {
    let l = Label::new(Some(text));
    l.set_wrap(true);
    l.set_xalign(0.0);
    l.set_halign(Align::Fill);
    l.add_css_class("nm-muted");
    l
}

fn card(title: &str) -> GtkBox {
    let c = GtkBox::new(Orientation::Vertical, 8);
    c.add_css_class("nm-info-card");
    if !title.is_empty() {
        let h = Label::new(Some(title));
        h.add_css_class("nm-info-title");
        h.set_halign(Align::Start);
        c.append(&h);
    }
    c
}

// ───────────────────────────────────────────────────────────── plot geometry

#[derive(Clone, Copy)]
struct Geo {
    w: f64,
    h: f64,
}

impl Geo {
    fn pw(&self) -> f64 {
        (self.w - PL - PR).max(10.0)
    }
    fn ph(&self) -> f64 {
        (self.h - PT - PB).max(10.0)
    }
    fn x_of(&self, hz: f64) -> f64 {
        PL + (hz.clamp(F_MIN, F_MAX) / F_MIN).log10() / (F_MAX / F_MIN).log10() * self.pw()
    }
    fn hz_of(&self, x: f64) -> f64 {
        let t = ((x - PL) / self.pw()).clamp(0.0, 1.0);
        F_MIN * (F_MAX / F_MIN).powf(t)
    }
    fn y_of(&self, db: f64) -> f64 {
        PT + (GAIN_MAX - db.clamp(GAIN_MIN, GAIN_MAX)) / (GAIN_MAX - GAIN_MIN) * self.ph()
    }
    fn db_of(&self, y: f64) -> f64 {
        (GAIN_MAX - ((y - PT) / self.ph()).clamp(0.0, 1.0) * (GAIN_MAX - GAIN_MIN)).clamp(GAIN_MIN, GAIN_MAX)
    }
}

fn round_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
    cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
    cr.arc(x + r, y + r, r, pi, 3.0 * pi / 2.0);
    cr.close_path();
}

fn nearest_band(geo: Geo, gains: &[f64; 10], x: f64, y: f64) -> usize {
    let centers: Vec<f64> = {
        let eq = shared_eq().lock();
        (0..10).map(|i| eq.center(i)).collect()
    };
    // Prefer a handle the pointer is actually on.
    let mut best: Option<(usize, f64)> = None;
    for i in 0..10 {
        let d = ((geo.x_of(centers[i]) - x).powi(2) + (geo.y_of(gains[i]) - y).powi(2)).sqrt();
        if d <= 28.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((i, d));
        }
    }
    if let Some((i, _)) = best {
        return i;
    }
    (0..10)
        .min_by(|a, b| {
            (geo.x_of(centers[*a]) - x)
                .abs()
                .partial_cmp(&(geo.x_of(centers[*b]) - x).abs())
                .unwrap()
        })
        .unwrap_or(0)
}

struct PlotState {
    hover: Option<usize>,
    active: Option<usize>,
    selected: usize,
}

/// The interactive response graph. `interactive = false` draws only (Match tab).
fn eq_plot(
    state: Rc<AppState>,
    spectrum: Rc<RefCell<Spectrum>>,
    show_spectrum: Rc<Cell<bool>>,
    interactive: bool,
    sketch: Option<Rc<RefCell<Vec<(f64, f64)>>>>,
) -> DrawingArea {
    let area = DrawingArea::new();
    area.add_css_class("nm-eq-plot");
    area.set_content_height(280);
    area.set_hexpand(true);
    area.set_focusable(interactive);
    area.set_can_focus(interactive);
    let ps = Rc::new(RefCell::new(PlotState { hover: None, active: None, selected: 0 }));

    {
        let st = state.clone();
        let ps = ps.clone();
        let sp = spectrum.clone();
        let show = show_spectrum.clone();
        let sk = sketch.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let geo = Geo { w: w as f64, h: h as f64 };
            let gains = st.player.eq_bands();
            let bypass = shared_eq().lock().bypass();
            // Panel
            round_rect(cr, 0.5, 0.5, geo.w - 1.0, geo.h - 1.0, 14.0);
            cr.set_source_rgb(0.045, 0.055, 0.075);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
            cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
            cr.set_font_size(10.0);

            // Live spectrum behind everything (1/3-octave bars).
            if show.get() {
                let s = sp.borrow();
                if s.live {
                    let half = 2f64.powf(1.0 / 6.0);
                    for i in 0..30 {
                        let (x0, x1) = (geo.x_of(THIRD_HZ[i] / half), geo.x_of(THIRD_HZ[i] * half));
                        let frac = ((s.levels[i] + 90.0) / 90.0).clamp(0.0, 1.0);
                        let bh = frac * geo.ph();
                        cr.set_source_rgba(0.5, 0.78, 1.0, 0.10 + 0.14 * frac);
                        cr.rectangle(x0 + 0.5, PT + geo.ph() - bh, (x1 - x0 - 1.0).max(1.0), bh);
                        let _ = cr.fill();
                    }
                }
            }

            // dB grid + labels
            let mut db = GAIN_MIN;
            while db <= GAIN_MAX + 0.01 {
                let y = geo.y_of(db);
                let zero = db.abs() < 0.01;
                cr.set_source_rgba(1.0, 1.0, 1.0, if zero { 0.30 } else { 0.07 });
                cr.set_line_width(if zero { 1.3 } else { 1.0 });
                cr.move_to(PL, y);
                cr.line_to(geo.w - PR, y);
                let _ = cr.stroke();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
                let label = if zero { "0".to_string() } else { format!("{:+.0}", db) };
                let ext = cr.text_extents(&label).ok();
                let tw = ext.map(|e| e.width()).unwrap_or(0.0);
                cr.move_to(PL - 8.0 - tw, y + 3.5);
                let _ = cr.show_text(&label);
                db += 3.0;
            }
            // Frequency grid + labels
            for (hz, label) in [
                (20.0, "20"), (50.0, "50"), (100.0, "100"), (200.0, "200"), (500.0, "500"),
                (1_000.0, "1k"), (2_000.0, "2k"), (5_000.0, "5k"), (10_000.0, "10k"), (20_000.0, "20k"),
            ] {
                let x = geo.x_of(hz);
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
                cr.set_line_width(1.0);
                cr.move_to(x, PT);
                cr.line_to(x, PT + geo.ph());
                let _ = cr.stroke();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
                let tw = cr.text_extents(label).map(|e| e.width()).unwrap_or(0.0);
                let tx = (x - tw / 2.0).clamp(PL - 6.0, geo.w - PR - tw + 6.0);
                cr.move_to(tx, geo.h - 8.0);
                let _ = cr.show_text(label);
            }

            // Response curve + fill to 0 dB
            let n = 220;
            let mut pts: Vec<(f64, f64)> = Vec::with_capacity(n);
            {
                let mut eq = shared_eq().lock();
                for i in 0..n {
                    let t = i as f64 / (n - 1) as f64;
                    let hz = F_MIN * (F_MAX / F_MIN).powf(t);
                    let d = if bypass { 0.0 } else { eq.response_db(hz).clamp(GAIN_MIN, GAIN_MAX) };
                    pts.push((geo.x_of(hz), geo.y_of(d)));
                }
            }
            let y0 = geo.y_of(0.0);
            let accent = if bypass { (0.55, 0.58, 0.62) } else { (0.45, 0.78, 1.0) };
            cr.move_to(pts[0].0, y0);
            for (x, y) in &pts {
                cr.line_to(*x, *y);
            }
            cr.line_to(pts[n - 1].0, y0);
            cr.close_path();
            let g = gtk4::cairo::LinearGradient::new(0.0, PT, 0.0, PT + geo.ph());
            g.add_color_stop_rgba(0.0, accent.0, accent.1, accent.2, 0.34);
            g.add_color_stop_rgba(0.5, accent.0, accent.1, accent.2, 0.06);
            g.add_color_stop_rgba(1.0, accent.0, accent.1, accent.2, 0.34);
            let _ = cr.set_source(&g);
            let _ = cr.fill();
            cr.set_line_width(2.6);
            cr.set_line_join(gtk4::cairo::LineJoin::Round);
            cr.set_source_rgba(accent.0, accent.1, accent.2, 1.0);
            for (i, (x, y)) in pts.iter().enumerate() {
                if i == 0 {
                    cr.move_to(*x, *y);
                } else {
                    cr.line_to(*x, *y);
                }
            }
            let _ = cr.stroke();

            // Sketch (Match tab)
            if let Some(sk) = &sk {
                let sketch = sk.borrow();
                if sketch.len() > 1 {
                    cr.set_source_rgba(1.0, 0.78, 0.35, 0.95);
                    cr.set_line_width(2.0);
                    cr.set_dash(&[6.0, 4.0], 0.0);
                    for (i, (hz, db)) in sketch.iter().enumerate() {
                        let (x, y) = (geo.x_of(*hz), geo.y_of(*db));
                        if i == 0 {
                            cr.move_to(x, y);
                        } else {
                            cr.line_to(x, y);
                        }
                    }
                    let _ = cr.stroke();
                    cr.set_dash(&[], 0.0);
                }
            }

            // Band handles
            let p = ps.borrow();
            let centers: Vec<f64> = {
                let eq = shared_eq().lock();
                (0..10).map(|i| eq.center(i)).collect()
            };
            for i in 0..10 {
                let (x, y) = (geo.x_of(centers[i]), geo.y_of(gains[i]));
                let live = p.active == Some(i);
                let hot = live || p.hover == Some(i);
                let sel = p.selected == i && interactive;
                let r = if hot { 8.5 } else if sel { 7.5 } else { 6.0 };
                if sel || hot {
                    cr.set_source_rgba(accent.0, accent.1, accent.2, 0.25);
                    cr.arc(x, y, r + 5.0, 0.0, std::f64::consts::TAU);
                    let _ = cr.fill();
                }
                cr.set_source_rgb(0.04, 0.05, 0.07);
                cr.arc(x, y, r + 1.5, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
                cr.set_source_rgb(accent.0, accent.1, accent.2);
                cr.arc(x, y, r, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
                if hot {
                    let label = format!("{}  {}", fmt_hz(centers[i]), fmt_db(gains[i]));
                    let tw = cr.text_extents(&label).map(|e| e.width()).unwrap_or(60.0) + 14.0;
                    let bx = (x - tw / 2.0).clamp(PL, geo.w - PR - tw);
                    let by = if y < PT + 40.0 { y + 16.0 } else { y - 34.0 };
                    round_rect(cr, bx, by, tw, 22.0, 7.0);
                    cr.set_source_rgba(0.02, 0.03, 0.05, 0.92);
                    let _ = cr.fill();
                    cr.set_source_rgb(0.95, 0.97, 1.0);
                    cr.move_to(bx + 7.0, by + 15.0);
                    let _ = cr.show_text(&label);
                }
            }
            if bypass {
                cr.set_source_rgba(1.0, 0.8, 0.4, 0.95);
                cr.set_font_size(12.0);
                cr.move_to(PL + 10.0, PT + 20.0);
                let _ = cr.show_text("EQ bypassed — graph shows the curve you would hear");
            }
        });
    }

    if interactive {
        // Drag: the band is locked when the gesture starts, so moving sideways never
        // swaps the band being edited.
        let drag = gtk4::GestureDrag::new();
        {
            let st = state.clone();
            let ps = ps.clone();
            let area_c = area.clone();
            drag.connect_drag_begin(move |_, x, y| {
                let geo = Geo { w: area_c.width() as f64, h: area_c.height() as f64 };
                let gains = st.player.eq_bands();
                let band = nearest_band(geo, &gains, x, y);
                {
                    let mut p = ps.borrow_mut();
                    p.active = Some(band);
                    p.selected = band;
                }
                area_c.grab_focus();
                st.player.set_eq_band(band, snap(geo.db_of(y)));
                area_c.queue_draw();
            });
        }
        {
            let st = state.clone();
            let ps = ps.clone();
            let area_c = area.clone();
            drag.connect_drag_update(move |g, _, dy| {
                let Some((_, sy)) = g.start_point() else { return };
                let geo = Geo { w: area_c.width() as f64, h: area_c.height() as f64 };
                if let Some(band) = ps.borrow().active {
                    st.player.set_eq_band(band, snap(geo.db_of(sy + dy)));
                }
                area_c.queue_draw();
            });
        }
        {
            let ps = ps.clone();
            let area_c = area.clone();
            drag.connect_drag_end(move |_, _, _| {
                ps.borrow_mut().active = None;
                area_c.queue_draw();
            });
        }
        area.add_controller(drag);

        // Double-click resets the band under the pointer.
        let click = gtk4::GestureClick::new();
        {
            let st = state.clone();
            let area_c = area.clone();
            click.connect_pressed(move |_, n, x, y| {
                if n == 2 {
                    let geo = Geo { w: area_c.width() as f64, h: area_c.height() as f64 };
                    let band = nearest_band(geo, &st.player.eq_bands(), x, y);
                    st.player.set_eq_band(band, 0.0);
                    area_c.queue_draw();
                }
            });
        }
        area.add_controller(click);

        let motion = gtk4::EventControllerMotion::new();
        {
            let st = state.clone();
            let ps_m = ps.clone();
            let area_m = area.clone();
            motion.connect_motion(move |_, x, y| {
                let geo = Geo { w: area_m.width() as f64, h: area_m.height() as f64 };
                let band = nearest_band(geo, &st.player.eq_bands(), x, y);
                ps_m.borrow_mut().hover = Some(band);
                area_m.queue_draw();
            });
            let ps_l = ps.clone();
            let area_l = area.clone();
            motion.connect_leave(move |_| {
                ps_l.borrow_mut().hover = None;
                area_l.queue_draw();
            });
        }
        area.add_controller(motion);

        // Scroll wheel: ±0.5 dB on the hovered band.
        let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        {
            let st = state.clone();
            let ps = ps.clone();
            let area_c = area.clone();
            scroll.connect_scroll(move |_, _, dy| {
                let band = ps.borrow().hover.unwrap_or(ps.borrow().selected);
                let now = st.player.eq_bands()[band];
                st.player.set_eq_band(band, snap(now - dy * 0.5));
                area_c.queue_draw();
                glib::Propagation::Stop
            });
        }
        area.add_controller(scroll);

        // Keyboard: ←/→ choose a band, ↑/↓ ±0.5 dB, 0 resets.
        let keys = gtk4::EventControllerKey::new();
        {
            let st = state.clone();
            let ps = ps.clone();
            let area_c = area.clone();
            keys.connect_key_pressed(move |_, key, _, _| {
                use gtk4::gdk::Key;
                let mut p = ps.borrow_mut();
                let band = p.selected;
                let gain = st.player.eq_bands()[band];
                match key {
                    Key::Left => p.selected = band.saturating_sub(1),
                    Key::Right => p.selected = (band + 1).min(9),
                    Key::Up => st.player.set_eq_band(band, snap(gain + 0.5)),
                    Key::Down => st.player.set_eq_band(band, snap(gain - 0.5)),
                    Key::_0 | Key::Home => st.player.set_eq_band(band, 0.0),
                    _ => return glib::Propagation::Proceed,
                }
                drop(p);
                area_c.queue_draw();
                glib::Propagation::Stop
            });
        }
        area.add_controller(keys);
        area.set_tooltip_text(Some(
            "Drag a handle to set a band · scroll to fine-tune · double-click to reset · arrow keys when focused",
        ));
    }

    {
        let area_t = area.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
            if area_t.is_mapped() {
                area_t.queue_draw();
            }
            glib::ControlFlow::Continue
        });
    }
    area
}

fn snap(db: f64) -> f64 {
    ((db * 10.0).round() / 10.0).clamp(GAIN_MIN, GAIN_MAX)
}

// ─────────────────────────────────────────────────────────────── sliders

/// Ten vertical band sliders with a live dB readout and Hz caption. Shared with Tools.
pub(crate) fn eq_sliders(state: &Rc<AppState>, height: i32) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 4);
    row.set_homogeneous(true);
    row.set_hexpand(true);
    row.add_css_class("nm-eq-sliders");
    let syncing = Rc::new(Cell::new(false));
    let mut scales = Vec::new();
    let mut readouts = Vec::new();
    for i in 0..10 {
        let col = GtkBox::new(Orientation::Vertical, 4);
        let val = Label::new(Some("0.0"));
        val.add_css_class("nm-eq-val");
        let scale = gtk4::Scale::with_range(Orientation::Vertical, GAIN_MIN, GAIN_MAX, 0.5);
        scale.set_inverted(true);
        scale.set_draw_value(false);
        scale.set_height_request(height);
        scale.set_vexpand(true);
        scale.set_halign(Align::Center);
        scale.add_mark(0.0, gtk4::PositionType::Right, None);
        scale.set_value(state.player.eq_bands()[i]);
        scale.update_property(&[gtk4::accessible::Property::Label(&format!(
            "{} gain",
            fmt_hz(BAND_HZ[i])
        ))]);
        let hz = Label::new(Some(BAND_LABELS[i]));
        hz.add_css_class("nm-muted");
        {
            let st = state.clone();
            let syncing = syncing.clone();
            let val = val.clone();
            scale.connect_value_changed(move |s| {
                val.set_text(&format!("{:+.1}", s.value()));
                if !syncing.get() {
                    st.player.set_eq_band(i, s.value());
                }
            });
        }
        // Double-click resets.
        let click = gtk4::GestureClick::new();
        {
            let scale = scale.clone();
            click.connect_pressed(move |_, n, _, _| {
                if n == 2 {
                    scale.set_value(0.0);
                }
            });
        }
        scale.add_controller(click);
        col.append(&val);
        col.append(&scale);
        col.append(&hz);
        row.append(&col);
        scales.push(scale);
        readouts.push(val);
    }
    {
        let st = state.clone();
        let row_t = row.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            if row_t.is_mapped() {
                let gains = st.player.eq_bands();
                syncing.set(true);
                for i in 0..10 {
                    if (scales[i].value() - gains[i]).abs() > 0.01 {
                        scales[i].set_value(gains[i]);
                    }
                    readouts[i].set_text(&format!("{:+.1}", gains[i]));
                }
                syncing.set(false);
            }
            glib::ControlFlow::Continue
        });
    }
    row
}

// ─────────────────────────────────────────────────────────────── tabs

fn graphic_page(state: Rc<AppState>, spectrum: Rc<RefCell<Spectrum>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.set_halign(Align::Fill);
    let bypass = ToggleButton::with_label("Bypass");
    bypass.add_css_class("nm-chip");
    bypass.set_active(shared_eq().lock().bypass());
    bypass.set_tooltip_text(Some("Hear the original sound"));
    bypass.update_property(&[gtk4::accessible::Property::Label("Bypass equalizer")]);
    bypass.connect_toggled(|b| shared_eq().lock().set_bypass(b.is_active()));
    let delta = ToggleButton::with_label("Delta");
    delta.add_css_class("nm-chip");
    delta.set_active(shared_eq().lock().delta());
    delta.set_tooltip_text(Some("Hear only what the equalizer adds or removes"));
    delta.update_property(&[gtk4::accessible::Property::Label("Hear only what the equalizer changed")]);
    delta.connect_toggled(|b| shared_eq().lock().set_delta(b.is_active()));
    let flat = Button::with_label("Reset flat");
    flat.add_css_class("nm-chip");
    {
        let st = state.clone();
        flat.connect_clicked(move |_| {
            for i in 0..10 {
                st.player.set_eq_band(i, 0.0);
            }
        });
    }
    let show = Rc::new(Cell::new(true));
    let show_btn = CheckButton::with_label("Live spectrum");
    show_btn.set_active(true);
    {
        let show = show.clone();
        show_btn.connect_toggled(move |b| show.set(b.is_active()));
    }
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let meter = gtk4::LevelBar::for_interval(0.0, 1.0);
    meter.set_size_request(120, -1);
    meter.set_valign(Align::Center);
    meter.add_css_class("nm-peak");
    let peak = Label::new(Some("Peak —"));
    peak.add_css_class("nm-muted");
    peak.set_width_chars(16);
    peak.set_xalign(1.0);
    bar.append(&bypass);
    bar.append(&delta);
    bar.append(&flat);
    bar.append(&show_btn);
    bar.append(&spacer);
    bar.append(&meter);
    bar.append(&peak);
    page.append(&bar);

    let plot = eq_plot(state.clone(), spectrum, show, true, None);
    page.append(&plot);
    page.append(&eq_sliders(&state, 150));
    page.append(&hint(
        "Drag the handles, scroll over the graph, or use the sliders. Double-click resets a band. Gain range ±12 dB.",
    ));

    {
        let peak = peak.clone();
        let meter = meter.clone();
        let page_t = page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
            if page_t.is_mapped() {
                let db = crate::eq::snapshot_peak_db();
                let us = crate::eq::snapshot_cpu_us();
                meter.set_value(((db + 60.0) / 60.0).clamp(0.0, 1.0));
                let hot = if db > -0.5 { " · CLIP" } else if us > 2_000 { " · CPU high" } else { "" };
                peak.set_text(&format!("Peak {db:.1} dBFS{hot}"));
            }
            glib::ControlFlow::Continue
        });
    }
    page
}

fn bands_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);
    let c = card("Bands");
    c.append(&hint(
        "Set each band's gain and centre. The centre accepts Hz (1000, 2.5k) or a note name (A4, C#3).",
    ));
    let grid = gtk4::Grid::new();
    grid.set_column_spacing(14);
    grid.set_row_spacing(8);
    for (col, title) in ["Band", "Centre", "Gain (dB)", ""].iter().enumerate() {
        let l = Label::new(Some(title));
        l.add_css_class("nm-detail-key");
        l.set_halign(Align::Start);
        grid.attach(&l, col as i32, 0, 1, 1);
    }
    let mut spins = Vec::new();
    for i in 0..10 {
        let r = i as i32 + 1;
        let name = Label::new(Some(&format!("{}", i + 1)));
        name.set_halign(Align::Start);
        let centre = gtk4::Entry::new();
        centre.set_width_chars(9);
        centre.set_text(&fmt_hz(shared_eq().lock().center(i as usize)));
        centre.set_tooltip_text(Some("Press Enter to apply"));
        centre.update_property(&[gtk4::accessible::Property::Label(&format!("Band {} centre", i + 1))]);
        {
            let centre_c = centre.clone();
            centre.connect_activate(move |e| {
                if let Some(hz) = parse_freq(&e.text()) {
                    shared_eq().lock().set_center(i as usize, hz.clamp(F_MIN, F_MAX));
                    centre_c.set_text(&fmt_hz(shared_eq().lock().center(i as usize)));
                    centre_c.remove_css_class("error");
                } else {
                    centre_c.add_css_class("error");
                }
            });
        }
        let spin = gtk4::SpinButton::with_range(GAIN_MIN, GAIN_MAX, 0.5);
        spin.set_digits(1);
        spin.set_value(state.player.eq_bands()[i as usize]);
        spin.update_property(&[gtk4::accessible::Property::Label(&format!("Band {} gain", i + 1))]);
        let syncing = Rc::new(Cell::new(false));
        {
            let st = state.clone();
            let syncing = syncing.clone();
            spin.connect_value_changed(move |s| {
                if !syncing.get() {
                    st.player.set_eq_band(i as usize, s.value());
                }
            });
        }
        let reset = Button::with_label("Reset");
        reset.add_css_class("nm-chip");
        {
            let spin = spin.clone();
            reset.connect_clicked(move |_| spin.set_value(0.0));
        }
        grid.attach(&name, 0, r, 1, 1);
        grid.attach(&centre, 1, r, 1, 1);
        grid.attach(&spin, 2, r, 1, 1);
        grid.attach(&reset, 3, r, 1, 1);
        spins.push((spin, syncing, centre));
    }
    c.append(&grid);
    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let defaults = Button::with_label("Restore standard centres");
    defaults.add_css_class("nm-chip");
    {
        let entries: Vec<gtk4::Entry> = spins.iter().map(|(_, _, e)| e.clone()).collect();
        defaults.connect_clicked(move |_| {
            for (i, hz) in BAND_HZ.iter().enumerate() {
                shared_eq().lock().set_center(i, *hz);
                entries[i].set_text(&fmt_hz(*hz));
            }
        });
    }
    actions.append(&defaults);
    c.append(&actions);
    page.append(&c);
    {
        let st = state.clone();
        let page_t = page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            if page_t.is_mapped() {
                let gains = st.player.eq_bands();
                for (i, (spin, syncing, _)) in spins.iter().enumerate() {
                    if (spin.value() - gains[i]).abs() > 0.01 && !spin.has_focus() {
                        syncing.set(true);
                        spin.set_value(gains[i]);
                        syncing.set(false);
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }
    page
}

fn parse_freq(text: &str) -> Option<f64> {
    let t = text.trim().to_ascii_lowercase().replace("hz", "");
    let t = t.trim();
    if let Some(k) = t.strip_suffix('k') {
        return k.trim().parse::<f64>().ok().map(|v| v * 1000.0).filter(|v| *v > 0.0);
    }
    if let Ok(v) = t.parse::<f64>() {
        return (v > 0.0).then_some(v);
    }
    note_to_hz(t)
}

fn note_to_hz(text: &str) -> Option<f64> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let semis = match bytes[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut acc = 0;
    if bytes.get(i) == Some(&b'#') {
        acc = 1;
        i += 1;
    } else if bytes.get(i) == Some(&b'b') {
        acc = -1;
        i += 1;
    }
    let octave: i32 = text.get(i..)?.parse().ok()?;
    let midi = (octave + 1) * 12 + semis + acc;
    Some(440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0))
}

fn analyzer_page(spectrum: Rc<RefCell<Spectrum>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);
    let c = card("Spectrum analyzer");
    c.append(&hint("Post-EQ level per 1/3 octave, in dBFS, with peak hold. Play something to see it move."));
    let controls = GtkBox::new(Orientation::Horizontal, 14);
    let freeze = CheckButton::with_label("Freeze");
    freeze.set_active(shared_eq().lock().frozen());
    freeze.connect_toggled(|b| shared_eq().lock().set_frozen(b.is_active()));
    let tilt_l = Label::new(Some("Tilt"));
    tilt_l.add_css_class("nm-muted");
    let tilt = gtk4::Scale::with_range(Orientation::Horizontal, -6.0, 6.0, 0.5);
    tilt.set_value(0.0);
    tilt.set_hexpand(true);
    tilt.set_draw_value(true);
    tilt.set_value_pos(gtk4::PositionType::Right);
    tilt.set_format_value_func(|_, v| format!("{v:+.1} dB/oct"));
    tilt.add_mark(0.0, gtk4::PositionType::Bottom, None);
    tilt.set_tooltip_text(Some("Display tilt around 1 kHz — compensates the natural pink-noise slope. Does not change the sound."));
    controls.append(&freeze);
    controls.append(&tilt_l);
    controls.append(&tilt);
    c.append(&controls);

    let area = DrawingArea::new();
    area.add_css_class("nm-eq-plot");
    area.set_content_height(300);
    area.set_hexpand(true);
    {
        let sp = spectrum.clone();
        let tilt = tilt.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (w as f64, h as f64);
            let (pl, pr, pt, pb) = (40.0, 12.0, 12.0, 26.0);
            let (pw, ph) = ((w - pl - pr).max(10.0), (h - pt - pb).max(10.0));
            round_rect(cr, 0.5, 0.5, w - 1.0, h - 1.0, 14.0);
            cr.set_source_rgb(0.045, 0.055, 0.075);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
            cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
            cr.set_font_size(10.0);
            let y_of = |db: f64| pt + (0.0 - db.clamp(-90.0, 0.0)) / 90.0 * ph;
            for db in (-90..=0).step_by(15) {
                let y = y_of(db as f64);
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
                cr.move_to(pl, y);
                cr.line_to(w - pr, y);
                let _ = cr.stroke();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
                let label = format!("{db}");
                let tw = cr.text_extents(&label).map(|e| e.width()).unwrap_or(0.0);
                cr.move_to(pl - 8.0 - tw, y + 3.5);
                let _ = cr.show_text(&label);
            }
            let s = sp.borrow();
            let tilt_db = tilt.value();
            let slot = pw / 30.0;
            for i in 0..30 {
                let octaves = (THIRD_HZ[i] / 1000.0).log2();
                let lvl = (s.levels[i] + tilt_db * octaves).clamp(-90.0, 0.0);
                let peak = (s.peaks[i] + tilt_db * octaves).clamp(-90.0, 0.0);
                let x = pl + slot * i as f64 + 1.5;
                let bw = (slot - 3.0).max(2.0);
                let top = y_of(lvl);
                let g = gtk4::cairo::LinearGradient::new(0.0, pt + ph, 0.0, pt);
                g.add_color_stop_rgb(0.0, 0.18, 0.72, 0.45);
                g.add_color_stop_rgb(0.62, 0.45, 0.85, 0.40);
                g.add_color_stop_rgb(0.85, 0.95, 0.78, 0.30);
                g.add_color_stop_rgb(1.0, 0.95, 0.35, 0.30);
                let _ = cr.set_source(&g);
                cr.rectangle(x, top, bw, pt + ph - top);
                let _ = cr.fill();
                if peak > -89.0 {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
                    cr.rectangle(x, y_of(peak) - 1.5, bw, 2.0);
                    let _ = cr.fill();
                }
            }
            for (idx, label) in [(1, "31"), (4, "63"), (7, "125"), (10, "250"), (13, "500"), (16, "1k"), (19, "2k"), (22, "4k"), (25, "8k"), (28, "16k")] {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
                let tw = cr.text_extents(label).map(|e| e.width()).unwrap_or(0.0);
                cr.move_to(pl + slot * (idx as f64 + 0.5) - tw / 2.0, h - 8.0);
                let _ = cr.show_text(label);
            }
            if !s.live {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.55);
                cr.set_font_size(13.0);
                cr.move_to(pl + 16.0, pt + ph / 2.0);
                let _ = cr.show_text("No audio yet — start playing a track");
            }
        });
    }
    c.append(&area);
    {
        let area_t = area.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            if area_t.is_mapped() {
                area_t.queue_draw();
            }
            glib::ControlFlow::Continue
        });
    }
    page.append(&c);
    page
}

fn dynamics_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);
    let c = card("Dynamic & spectral bands");
    let master_row = GtkBox::new(Orientation::Horizontal, 12);
    let sw = gtk4::Switch::new();
    sw.set_active(shared_eq().lock().advanced());
    sw.set_valign(Align::Center);
    sw.update_property(&[gtk4::accessible::Property::Label("Enable advanced processing")]);
    let ml = Label::new(Some("Enable advanced processing"));
    ml.add_css_class("nm-detail-val");
    master_row.append(&sw);
    master_row.append(&ml);
    c.append(&master_row);
    c.append(&hint(
        "Dynamic: a band ducks 6 dB when that frequency is loud. Spectral: a band leans toward the measured level. Neither does anything until advanced processing is on.",
    ));
    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(2);
    flow.set_max_children_per_line(5);
    flow.set_column_spacing(10);
    flow.set_row_spacing(10);
    let mut cells: Vec<GtkBox> = Vec::new();
    for (i, hz) in BAND_HZ.iter().enumerate() {
        let cell = GtkBox::new(Orientation::Vertical, 6);
        cell.add_css_class("nm-dyn-cell");
        let title = Label::new(Some(&fmt_hz(*hz)));
        title.add_css_class("nm-info-title");
        let dynamic = ToggleButton::with_label("Dynamic");
        dynamic.add_css_class("nm-chip");
        dynamic.set_active(shared_eq().lock().dynamic_band(i));
        dynamic.connect_toggled(move |b| shared_eq().lock().set_dynamic_band(i, b.is_active()));
        let spectral = ToggleButton::with_label("Spectral");
        spectral.add_css_class("nm-chip");
        spectral.set_active(shared_eq().lock().spectral_band(i));
        spectral.connect_toggled(move |b| shared_eq().lock().set_spectral_band(i, b.is_active()));
        cell.append(&title);
        cell.append(&dynamic);
        cell.append(&spectral);
        cell.set_sensitive(sw.is_active());
        flow.insert(&cell, -1);
        cells.push(cell);
    }
    sw.connect_active_notify(move |s| {
        shared_eq().lock().set_advanced(s.is_active());
        for cell in &cells {
            cell.set_sensitive(s.is_active());
        }
    });
    c.append(&flow);
    page.append(&c);
    page
}

/// Linear interpolation of a sketched curve (sorted by frequency) at `hz`.
fn sketch_db_at(pts: &[(f64, f64)], hz: f64) -> Option<f64> {
    if pts.len() < 2 {
        return None;
    }
    let lx = hz.log10();
    let mut sorted = pts.to_vec();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if lx <= sorted[0].0.log10() {
        return Some(sorted[0].1);
    }
    for w in sorted.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (la, lb) = (a.0.log10(), b.0.log10());
        if lx >= la && lx <= lb {
            let t = if (lb - la).abs() < 1e-9 { 0.0 } else { (lx - la) / (lb - la) };
            return Some(a.1 + (b.1 - a.1) * t);
        }
    }
    sorted.last().map(|p| p.1)
}

fn match_page(state: Rc<AppState>, spectrum: Rc<RefCell<Spectrum>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);
    let c = card("Match");
    c.append(&hint(
        "Draw the shape you want on the graph and the ten bands follow it, or match the average spectrum of an audio file (ffmpeg reads the first 20 seconds).",
    ));
    let sketch: Rc<RefCell<Vec<(f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let plot = eq_plot(state.clone(), spectrum, Rc::new(Cell::new(false)), false, Some(sketch.clone()));
    let drag = gtk4::GestureDrag::new();
    {
        let sketch = sketch.clone();
        drag.connect_drag_begin(move |_, _, _| sketch.borrow_mut().clear());
    }
    {
        let sketch = sketch.clone();
        let plot_c = plot.clone();
        drag.connect_drag_update(move |g, _, _| {
            let Some((sx, sy)) = g.start_point() else { return };
            let (dx, dy) = g.offset().unwrap_or((0.0, 0.0));
            let geo = Geo { w: plot_c.width() as f64, h: plot_c.height() as f64 };
            sketch.borrow_mut().push((geo.hz_of(sx + dx), snap(geo.db_of(sy + dy))));
            plot_c.queue_draw();
        });
    }
    let status = Label::new(Some("Drag across the graph to draw a target."));
    status.add_css_class("nm-muted");
    status.set_halign(Align::Start);
    {
        let sketch = sketch.clone();
        let st = state.clone();
        let status = status.clone();
        let plot_c = plot.clone();
        drag.connect_drag_end(move |_, _, _| {
            let pts = sketch.borrow().clone();
            if pts.len() < 2 {
                return;
            }
            for i in 0..10 {
                let hz = shared_eq().lock().center(i);
                if let Some(db) = sketch_db_at(&pts, hz) {
                    st.player.set_eq_band(i, snap(db));
                }
            }
            status.set_text("Applied your sketch to the ten bands.");
            plot_c.queue_draw();
        });
    }
    plot.add_controller(drag);
    c.append(&plot);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let pick = Button::with_label("Match audio file…");
    pick.add_css_class("nm-chip");
    let clear = Button::with_label("Clear sketch");
    clear.add_css_class("nm-chip");
    {
        let sketch = sketch.clone();
        let plot_c = plot.clone();
        clear.connect_clicked(move |_| {
            sketch.borrow_mut().clear();
            plot_c.queue_draw();
        });
    }
    {
        let st = state.clone();
        let status_b = status.clone();
        pick.connect_clicked(move |_| {
            let dialog = gtk4::FileChooserDialog::new(
                Some("Match audio file"),
                None::<&gtk4::Window>,
                gtk4::FileChooserAction::Open,
                &[("Cancel", gtk4::ResponseType::Cancel), ("Match", gtk4::ResponseType::Accept)],
            );
            let st = st.clone();
            let status_b = status_b.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk4::ResponseType::Accept {
                    if let Some(path) = dialog.file().and_then(|f| f.path()) {
                        status_b.set_text("Reading file…");
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(decode_f32(&path));
                        });
                        let st = st.clone();
                        let status_t = status_b.clone();
                        glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                            match rx.try_recv() {
                                Ok(Ok(samples)) => {
                                    let gains = crate::eq::gains_from_samples(&samples, 48_000.0);
                                    for (i, gain) in gains.iter().enumerate() {
                                        st.player.set_eq_band(i, *gain);
                                    }
                                    status_t.set_text("Matched the file onto the ten bands.");
                                    glib::ControlFlow::Break
                                }
                                Ok(Err(err)) => {
                                    status_t.set_text(&err);
                                    glib::ControlFlow::Break
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => glib::ControlFlow::Break,
                            }
                        });
                    }
                }
                dialog.close();
            });
            dialog.show();
        });
    }
    actions.append(&pick);
    actions.append(&clear);
    c.append(&actions);
    c.append(&status);
    page.append(&c);
    page
}

fn decode_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let output = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-t", "20", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
        .output()
        .map_err(|_| "ffmpeg is not installed; file match needs it.".to_string())?;
    if !output.status.success() || output.stdout.len() < 1024 {
        return Err("Could not decode that file.".into());
    }
    let mut samples = Vec::with_capacity(output.stdout.len() / 4);
    for chunk in output.stdout.chunks_exact(4) {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(samples)
}

#[cfg(test)]
mod plot_tests {
    use super::*;

    #[test]
    fn frequency_and_note_parsing() {
        assert_eq!(parse_freq("1000"), Some(1000.0));
        assert_eq!(parse_freq("2.5k"), Some(2500.0));
        assert_eq!(parse_freq("440 Hz"), Some(440.0));
        let a4 = parse_freq("A4").unwrap();
        assert!((a4 - 440.0).abs() < 0.01, "{a4}");
        let c3s = parse_freq("C#3").unwrap();
        assert!((c3s - 138.59).abs() < 0.1, "{c3s}");
        assert_eq!(parse_freq("nonsense"), None);
        assert_eq!(parse_freq("-5"), None);
    }

    #[test]
    fn plot_geometry_round_trips() {
        let g = Geo { w: 800.0, h: 300.0 };
        for hz in [20.0, 100.0, 1_000.0, 20_000.0] {
            assert!((g.hz_of(g.x_of(hz)) - hz).abs() / hz < 1e-6, "{hz}");
        }
        for db in [-12.0, -3.0, 0.0, 6.5, 12.0] {
            assert!((g.db_of(g.y_of(db)) - db).abs() < 1e-6, "{db}");
        }
        assert!(g.y_of(12.0) < g.y_of(-12.0), "boost is drawn above cut");
    }

    #[test]
    fn sketch_interpolates_between_points() {
        let pts = vec![(100.0, -6.0), (10_000.0, 6.0)];
        let mid = sketch_db_at(&pts, 1_000.0).unwrap();
        assert!(mid.abs() < 0.01, "{mid}");
        assert_eq!(sketch_db_at(&pts, 20.0), Some(-6.0));
        assert_eq!(sketch_db_at(&pts, 20_000.0), Some(6.0));
        assert_eq!(sketch_db_at(&pts[..1], 500.0), None);
    }

    #[test]
    fn builtin_presets_stay_inside_the_gain_range() {
        for (name, gains) in BUILTIN_PRESETS {
            for g in gains {
                assert!((GAIN_MIN..=GAIN_MAX).contains(&g), "{name}: {g}");
            }
        }
        assert!(BUILTIN_PRESETS[0].1.iter().all(|g| *g == 0.0), "first preset is Flat");
    }

    #[test]
    fn snap_stays_in_range_and_rounds() {
        assert_eq!(snap(99.0), GAIN_MAX);
        assert_eq!(snap(-99.0), GAIN_MIN);
        assert!((snap(3.04) - 3.0).abs() < 1e-9);
    }
}

/// Starting points for the ten ISO bands (31.5 Hz … 16 kHz), all inside ±12 dB.
pub const BUILTIN_PRESETS: [(&str, [f64; 10]); 8] = [
    ("Flat", [0.0; 10]),
    ("Bass boost", [6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Treble boost", [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0]),
    ("Vocal presence", [-2.0, -1.0, -1.0, 0.0, 2.0, 3.0, 3.0, 2.0, 0.0, -1.0]),
    ("Loudness", [5.0, 3.0, 0.0, -1.0, -1.0, 0.0, 0.0, 2.0, 4.0, 5.0]),
    ("Rock", [4.0, 3.0, 1.0, -1.0, -2.0, 1.0, 3.0, 4.0, 4.0, 3.0]),
    ("Acoustic", [3.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 2.0]),
    ("Late night", [-3.0, -2.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, -2.0, -3.0]),
];

fn presets_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 12);

    let builtin = card("Starting points");
    builtin.append(&hint("One tap loads a shape onto the ten bands. Fine-tune it afterwards on the Graphic tab."));
    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(2);
    flow.set_max_children_per_line(4);
    flow.set_column_spacing(10);
    flow.set_row_spacing(10);
    for (name, gains) in BUILTIN_PRESETS {
        let b = Button::with_label(name);
        b.add_css_class("nm-chip");
        let st = state.clone();
        b.connect_clicked(move |_| {
            for (i, g) in gains.iter().enumerate() {
                st.player.set_eq_band(i, *g);
            }
            remember_last(&current_eq_key(&st), name);
        });
        flow.insert(&b, -1);
    }
    builtin.append(&flow);
    page.append(&builtin);

    let mine = card("Your presets");
    mine.append(&hint(
        "Saved as Audiophile .ofeq files. Bell, low shelf and high shelf land on the nearest band; other shapes are skipped.",
    ));
    let save_row = GtkBox::new(Orientation::Horizontal, 8);
    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("Name this setting"));
    entry.set_hexpand(true);
    let save = Button::with_label("Save current EQ");
    save.add_css_class("nm-chip");
    save_row.append(&entry);
    save_row.append(&save);
    mine.append(&save_row);
    let list = GtkBox::new(Orientation::Vertical, 6);
    mine.append(&list);
    page.append(&mine);

    let save_now = {
        let st = state.clone();
        let entry = entry.clone();
        let list = list.clone();
        Rc::new(move || {
            let name = entry.text().to_string();
            if name.trim().is_empty() {
                entry.add_css_class("error");
                return;
            }
            entry.remove_css_class("error");
            if save_preset(name.trim(), &st.player.eq_bands()).is_ok() {
                remember_last(&current_eq_key(&st), name.trim());
                entry.set_text("");
                refill_presets(&list, &st);
            }
        })
    };
    {
        let f = save_now.clone();
        save.connect_clicked(move |_| f());
        let f = save_now.clone();
        entry.connect_activate(move |_| f());
    }
    refill_presets(&list, &state);
    page
}

fn preset_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("neighbourhood-music")
        .join("eq-presets")
}

fn preset_file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{safe}.ofeq")
}

fn save_preset(name: &str, gains: &[f64; 10]) -> anyhow::Result<()> {
    let dir = preset_dir();
    std::fs::create_dir_all(&dir)?;
    let body = crate::eq::eq_preset_json(name, gains);
    std::fs::write(
        dir.join(preset_file_name(name)),
        serde_json::to_string_pretty(&body)?,
    )?;
    Ok(())
}

fn last_used_path() -> std::path::PathBuf {
    preset_dir().join("last-used.json")
}

fn remember_last(kind: &str, name: &str) {
    let path = last_used_path();
    let mut map = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = map.as_object_mut() {
        obj.insert(kind.to_string(), serde_json::Value::String(name.to_string()));
        if let Ok(text) = serde_json::to_string_pretty(&map) {
            let _ = std::fs::create_dir_all(preset_dir());
            let _ = std::fs::write(path, text);
        }
    }
}

fn last_for(kind: &str) -> Option<String> {
    let text = std::fs::read_to_string(last_used_path()).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    value.get(kind).and_then(|n| n.as_str()).map(str::to_string)
}

fn current_eq_key(state: &AppState) -> &'static str {
    state
        .now
        .read()
        .as_ref()
        .map(super::shell::source_eq_key)
        .unwrap_or("eq_library")
}

fn eq_key_label(key: &str) -> &'static str {
    match key {
        "eq_ytm" => "YouTube Music",
        "eq_radio" => "Radio",
        "eq_podcast" => "Podcasts",
        _ => "Library",
    }
}

fn apply_preset(state: &AppState, preset: &crate::eq::LoadedEqPreset) {
    if let Some(centers) = preset.centers {
        let mut eq = shared_eq().lock();
        for (i, hz) in centers.iter().enumerate() {
            eq.set_center(i, *hz);
        }
    }
    for (i, gain) in preset.gains.iter().enumerate() {
        state.player.set_eq_band(i, *gain);
    }
}

fn refill_presets(list: &GtkBox, state: &Rc<AppState>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let key = current_eq_key(state);
    let remembered = last_for(key).unwrap_or_else(|| "none yet".into());
    let caption = Label::new(Some(&format!(
        "Last preset for {}: {remembered}",
        eq_key_label(key)
    )));
    caption.add_css_class("nm-muted");
    caption.set_halign(Align::Start);
    list.append(&caption);
    let dir = preset_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        list.append(&hint("Nothing saved yet — set the curve you like and save it above."));
        return;
    };
    let mut any = false;
    for entry in rd.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "ofeq" && ext != "json" {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("last-used.json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let Ok(preset) = crate::eq::eq_preset_from_json(&value) else { continue };
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let b = Button::with_label(&format!("Load “{}”", preset.name));
        b.add_css_class("nm-chip");
        b.set_hexpand(true);
        b.set_halign(Align::Start);
        let st = state.clone();
        let loaded = preset.name.clone();
        b.connect_clicked(move |_| {
            apply_preset(&st, &preset);
            remember_last(&current_eq_key(&st), &loaded);
        });
        let remove = Button::with_label("Delete");
        remove.add_css_class("nm-chip");
        let list_r = list.clone();
        let st_r = state.clone();
        let path_r = path.clone();
        remove.connect_clicked(move |_| {
            let _ = std::fs::remove_file(&path_r);
            refill_presets(&list_r, &st_r);
        });
        row.append(&b);
        row.append(&remove);
        list.append(&row);
        any = true;
    }
    if !any {
        list.append(&hint("Nothing saved yet — set the curve you like and save it above."));
    }
}

#[cfg(test)]
mod tests {
    use super::note_to_hz;

    #[test]
    fn a4_and_csharp3() {
        let a4 = note_to_hz("A4").unwrap();
        assert!((a4 - 440.0).abs() < 0.01, "{a4}");
        let cs3 = note_to_hz("C#3").unwrap();
        assert!((cs3 - 138.59).abs() < 0.2, "{cs3}");
    }
}
