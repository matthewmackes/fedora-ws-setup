//! Stage Audiophile surface: response curve, band drags, and the other EQ tabs.

use crate::eq::{shared_eq, BAND_HZ};
use crate::ui::shell::AppState;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, DrawingArea, Label, Orientation, Stack};
use std::rc::Rc;

pub fn build(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    page.add_css_class("nm-audiophile");
    let tabs = GtkBox::new(Orientation::Horizontal, 6);
    tabs.set_halign(Align::Center);
    let stack = Stack::new();
    stack.set_vexpand(true);
    for (id, label) in [
        ("curve", "Curve"),
        ("analyzer", "Analyzer"),
        ("dynamics", "Dynamics"),
        ("match", "Match"),
        ("presets", "Presets"),
    ] {
        let b = Button::with_label(label);
        b.add_css_class("nm-chip");
        let stack = stack.clone();
        b.connect_clicked(move |_| stack.set_visible_child_name(id));
        tabs.append(&b);
    }
    page.append(&tabs);
    stack.add_named(&curve_page(state.clone()), Some("curve"));
    stack.add_named(&analyzer_page(), Some("analyzer"));
    stack.add_named(&dynamics_page(), Some("dynamics"));
    stack.add_named(&match_page(state.clone()), Some("match"));
    stack.add_named(&presets_page(state), Some("presets"));
    stack.set_visible_child_name("curve");
    page.append(&stack);
    page
}

fn curve_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    let area = DrawingArea::new();
    area.add_css_class("nm-eq-plot");
    area.set_content_height(180);
    area.set_hexpand(true);
    area.set_draw_func(|_, cr, w, h| {
        cr.set_source_rgb(0.05, 0.06, 0.08);
        cr.paint().ok();
        cr.set_source_rgb(0.25, 0.32, 0.4);
        cr.move_to(0.0, h as f64 * 0.5);
        cr.line_to(w as f64, h as f64 * 0.5);
        cr.stroke().ok();
        cr.set_source_rgb(0.55, 0.82, 1.0);
        let mut eq = shared_eq().lock();
        let n = 96.max(8);
        for i in 0..n {
            let t = i as f64 / (n - 1) as f64;
            let hz = 20.0 * (1000.0_f64).powf(t);
            let db = eq.response_db(hz).clamp(-24.0, 12.0);
            let x = t * w as f64;
            let y = (12.0 - db) / 36.0 * h as f64;
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        cr.stroke().ok();
    });
    let area_drag = area.clone();
    let st = state.clone();
    let drag = gtk4::GestureDrag::new();
    drag.connect_drag_update(move |g, _, _| {
        let Some((x, y)) = g.start_point() else { return };
        let (dx, dy) = g.offset().unwrap_or((0.0, 0.0));
        let w = area_drag.width().max(1) as f64;
        let h = area_drag.height().max(1) as f64;
        let t = ((x + dx) / w).clamp(0.0, 1.0);
        let hz = 20.0 * (1000.0_f64).powf(t);
        let band = BAND_HZ
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - hz).abs().partial_cmp(&(*b - hz).abs()).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let db = (12.0 - ((y + dy) / h).clamp(0.0, 1.0) * 36.0).clamp(-24.0, 12.0);
        st.player.set_eq_band(band, db);
        area_drag.queue_draw();
    });
    area.add_controller(drag);
    let plots = Stack::new();
    plots.set_hexpand(true);
    let gl_area = gtk4::GLArea::new();
    gl_area.add_css_class("nm-eq-plot");
    gl_area.set_size_request(-1, 180);
    gl_area.set_hexpand(true);
    let painter = Label::new(Some(super::gl_curve::painter_label(false)));
    painter.add_css_class("nm-muted");
    painter.set_halign(Align::Start);
    let painter_r = painter.clone();
    let plots_r = plots.clone();
    gl_area.connect_render(move |area, _| {
        let ok = super::gl_curve::render_curve(area);
        painter_r.set_text(super::gl_curve::painter_label(ok));
        let name = if ok { "gl" } else { "cpu" };
        if plots_r.visible_child_name().as_deref() != Some(name) {
            plots_r.set_visible_child_name(name);
        }
        glib::Propagation::Stop
    });
    let gl_drag = gl_area.clone();
    let st_gl = state.clone();
    let drag_gl = gtk4::GestureDrag::new();
    drag_gl.connect_drag_update(move |g, _, _| {
        let Some((x, y)) = g.start_point() else { return };
        let (dx, dy) = g.offset().unwrap_or((0.0, 0.0));
        let w = gl_drag.width().max(1) as f64;
        let h = gl_drag.height().max(1) as f64;
        let t = ((x + dx) / w).clamp(0.0, 1.0);
        let hz = 20.0 * (1000.0_f64).powf(t);
        let band = BAND_HZ
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - hz).abs().partial_cmp(&(*b - hz).abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let db = (12.0 - ((y + dy) / h).clamp(0.0, 1.0) * 36.0).clamp(-24.0, 12.0);
        st_gl.player.set_eq_band(band, db);
        gl_drag.queue_render();
    });
    gl_area.add_controller(drag_gl);
    plots.add_named(&area, Some("cpu"));
    plots.add_named(&gl_area, Some("gl"));
    plots.set_visible_child_name("gl");
    page.append(&painter);
    page.append(&plots);
    let gl_timer = gl_area.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        gl_timer.queue_render();
        glib::ControlFlow::Continue
    });
    let controls = GtkBox::new(Orientation::Horizontal, 6);
    let bypass = Button::with_label(if shared_eq().lock().bypass() {
        "Bypassed"
    } else {
        "Bypass"
    });
    bypass.add_css_class("nm-chip");
    bypass.update_property(&[gtk4::accessible::Property::Label("Bypass equalizer")]);
    bypass.connect_clicked(|b| {
        let on = !shared_eq().lock().bypass();
        shared_eq().lock().set_bypass(on);
        b.set_label(if on { "Bypassed" } else { "Bypass" });
    });
    let delta = Button::with_label(if shared_eq().lock().delta() {
        "Delta on"
    } else {
        "Delta"
    });
    delta.add_css_class("nm-chip");
    delta.update_property(&[gtk4::accessible::Property::Label("Hear only what the equalizer changed")]);
    delta.connect_clicked(|b| {
        let on = !shared_eq().lock().delta();
        shared_eq().lock().set_delta(on);
        b.set_label(if on { "Delta on" } else { "Delta" });
    });
    controls.append(&bypass);
    controls.append(&delta);
    let meter = GtkBox::new(Orientation::Horizontal, 2);
    for _ in 0..12 {
        let seg = Label::new(Some("▌"));
        seg.add_css_class("nm-muted");
        meter.append(&seg);
    }
    controls.append(&meter);
    page.append(&controls);
    let peak = Label::new(Some("Peak —"));
    peak.add_css_class("nm-muted");
    peak.set_halign(Align::Start);
    page.append(&peak);
    let peak_l = peak.clone();
    let area_t = area.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        let db = crate::eq::snapshot_peak_db();
        let us = crate::eq::snapshot_cpu_us();
        let hot = ((db + 60.0) / 60.0 * 12.0).clamp(0.0, 12.0) as i32;
        let mut child = meter.first_child();
        let mut index = 0;
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(label) = widget.downcast_ref::<Label>() {
                label.set_text(if index < hot { "█" } else { "▁" });
            }
            index += 1;
        }
        let load = if us > 2_000 {
            format!(" · EQ CPU high ({us} µs)")
        } else {
            String::new()
        };
        let band = shared_eq().lock().selected() + 1;
        peak_l.set_text(&format!("Post peak {db:.1} dBFS · band {band}{load}"));
        area_t.queue_draw();
        glib::ControlFlow::Continue
    });
    page.append(&band_table(state));
    page.append(&crate_note());
    page
}

fn note_to_hz(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(hz) = text.parse::<f64>() {
        return Some(hz);
    }
    let bytes = text.as_bytes();
    let letter = bytes[0].to_ascii_uppercase();
    let semis = match letter {
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

fn band_table(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 4);
    let head = Label::new(Some("Band table — type a gain in dB, or a note such as A4 or C#3 to retune that band"));
    head.set_wrap(true);
    head.set_halign(Align::Start);
    head.add_css_class("nm-muted");
    page.append(&head);
    for index in 0..10 {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let hz = shared_eq().lock().center(index);
        let label = Label::new(Some(&format!("{hz:.0} Hz")));
        label.set_width_chars(10);
        let gain = gtk4::Entry::new();
        gain.set_placeholder_text(Some("gain dB"));
        gain.set_width_chars(8);
        gain.set_text(&format!("{:.1}", state.player.eq_bands()[index]));
        gain.update_property(&[gtk4::accessible::Property::Label(&format!("Band {} gain", index + 1))]);
        let note = gtk4::Entry::new();
        note.set_placeholder_text(Some("A4"));
        note.set_width_chars(6);
        note.update_property(&[gtk4::accessible::Property::Label(&format!("Band {} note", index + 1))]);
        let st_g = state.clone();
        let label_g = label.clone();
        gain.connect_activate(move |e| {
            if let Ok(db) = e.text().parse::<f64>() {
                st_g.player.set_eq_band(index, db);
            }
        });
        let st_n = state.clone();
        note.connect_activate(move |e| {
            if let Some(hz) = note_to_hz(&e.text()) {
                shared_eq().lock().set_center(index, hz);
                label_g.set_text(&format!("{hz:.0} Hz"));
                let _ = &st_n;
            }
        });
        row.append(&label);
        row.append(&gain);
        row.append(&note);
        page.append(&row);
    }
    page
}

fn dynamics_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 6);
    let switch = gtk4::CheckButton::with_label("Enable advanced processing");
    switch.set_active(shared_eq().lock().advanced());
    switch.connect_toggled(|b| {
        shared_eq().lock().set_advanced(b.is_active());
    });
    page.append(&switch);
    let note = Label::new(Some(
        "Dynamic bands duck when that frequency is hot. Spectral bands lean toward the measured level. Neither runs until advanced processing is on.",
    ));
    note.set_wrap(true);
    note.add_css_class("nm-muted");
    note.set_halign(Align::Start);
    page.append(&note);
    for (index, hz) in crate::eq::BAND_HZ.iter().enumerate() {
        let band = gtk4::CheckButton::with_label(&format!("{hz:.0} Hz dynamic"));
        band.set_active(shared_eq().lock().dynamic_band(index));
        let switch_b = switch.clone();
        band.set_sensitive(switch_b.is_active());
        let band_gate = band.clone();
        switch.connect_toggled(move |b| band_gate.set_sensitive(b.is_active()));
        let index_d = index;
        band.connect_toggled(move |b| {
            shared_eq().lock().set_dynamic_band(index_d, b.is_active());
        });
        page.append(&band);
        let spectral = gtk4::CheckButton::with_label(&format!("{hz:.0} Hz spectral"));
        spectral.set_active(shared_eq().lock().spectral_band(index));
        spectral.set_sensitive(switch.is_active());
        let spectral_gate = spectral.clone();
        switch.connect_toggled(move |b| spectral_gate.set_sensitive(b.is_active()));
        spectral.connect_toggled(move |b| {
            shared_eq().lock().set_spectral_band(index, b.is_active());
        });
        page.append(&spectral);
    }
    page
}

fn match_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 6);
    let note = Label::new(Some(
        "Drag a target shape, or match the spectrum of an audio file. ffmpeg reads the file.",
    ));
    note.set_wrap(true);
    note.add_css_class("nm-muted");
    note.set_halign(Align::Start);
    page.append(&note);
    let area = DrawingArea::new();
    area.add_css_class("nm-eq-plot");
    area.set_content_height(160);
    area.set_hexpand(true);
    let sketch: Rc<std::cell::RefCell<Vec<(f64, f64)>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sketch_draw = sketch.clone();
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(0.05, 0.06, 0.08);
        cr.paint().ok();
        cr.set_source_rgb(0.9, 0.75, 0.4);
        let pts = sketch_draw.borrow();
        for (i, (x, y)) in pts.iter().enumerate() {
            let px = x * w as f64;
            let py = y * h as f64;
            if i == 0 {
                cr.move_to(px, py);
            } else {
                cr.line_to(px, py);
            }
        }
        cr.stroke().ok();
    });
    let area_d = area.clone();
    let sketch_d = sketch.clone();
    let st = state.clone();
    let drag = gtk4::GestureDrag::new();
    drag.connect_drag_begin(move |_, _, _| sketch_d.borrow_mut().clear());
    let sketch_u = sketch.clone();
    let area_u = area.clone();
    drag.connect_drag_update(move |g, _, _| {
        let Some((x, y)) = g.start_point() else { return };
        let (dx, dy) = g.offset().unwrap_or((0.0, 0.0));
        let w = area_u.width().max(1) as f64;
        let h = area_u.height().max(1) as f64;
        sketch_u.borrow_mut().push((((x + dx) / w).clamp(0.0, 1.0), ((y + dy) / h).clamp(0.0, 1.0)));
        area_u.queue_draw();
    });
    let sketch_e = sketch.clone();
    drag.connect_drag_end(move |_, _, _| {
        let pts = sketch_e.borrow().clone();
        if pts.len() < 2 {
            return;
        }
        for (index, hz) in BAND_HZ.iter().enumerate() {
            let t = (hz.log10() - 20.0_f64.log10()) / (20_000.0_f64.log10() - 20.0_f64.log10());
            let y = pts
                .iter()
                .min_by(|a, b| (a.0 - t).abs().partial_cmp(&(b.0 - t).abs()).unwrap())
                .map(|p| p.1)
                .unwrap_or(0.5);
            let db = (12.0 - y * 36.0).clamp(-24.0, 12.0);
            st.player.set_eq_band(index, db);
        }
        area_d.queue_draw();
    });
    area.add_controller(drag);
    page.append(&area);
    let status = Label::new(Some(""));
    status.add_css_class("nm-muted");
    status.set_halign(Align::Start);
    let pick = Button::with_label("Match audio file");
    pick.add_css_class("nm-chip");
    pick.set_halign(Align::Start);
    let st = state.clone();
    let status_b = status.clone();
    pick.connect_clicked(move |_| {
        let dialog = gtk4::FileChooserDialog::new(
            Some("Match audio file"),
            None::<&gtk4::Window>,
            gtk4::FileChooserAction::Open,
            &[
                ("Cancel", gtk4::ResponseType::Cancel),
                ("Match", gtk4::ResponseType::Accept),
            ],
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
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                glib::ControlFlow::Break
                            }
                        }
                    });
                }
            }
            dialog.close();
        });
        dialog.show();
    });
    page.append(&pick);
    page.append(&status);
    page
}

fn decode_f32(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
        ])
        .arg(path)
        .args(["-t", "8", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
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

fn analyzer_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 6);
    let note = Label::new(Some(
        "Post-EQ level in each graphic band. Tilt is display-only. Delta listen stays on the Curve tab.",
    ));
    note.set_wrap(true);
    note.add_css_class("nm-muted");
    note.set_halign(Align::Start);
    page.append(&note);
    let freeze = gtk4::CheckButton::with_label("Freeze");
    freeze.connect_toggled(|b| shared_eq().lock().set_frozen(b.is_active()));
    page.append(&freeze);
    let tilt = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, -3.0, 3.0, 0.5);
    tilt.set_value(0.0);
    tilt.set_hexpand(true);
    page.append(&Label::new(Some("Tilt dB/octave")));
    page.append(&tilt);
    let bars = GtkBox::new(Orientation::Horizontal, 6);
    bars.set_hexpand(true);
    let mut labels = Vec::new();
    for hz in crate::eq::BAND_HZ {
        let col = GtkBox::new(Orientation::Vertical, 2);
        let bar = Label::new(Some("▁"));
        bar.add_css_class("nm-hero-title");
        bar.set_halign(Align::Center);
        let cap = Label::new(Some(&format!("{hz:.0}")));
        cap.add_css_class("nm-muted");
        col.append(&bar);
        col.append(&cap);
        bars.append(&col);
        labels.push(bar);
    }
    page.append(&bars);
    let tilt_s = tilt.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        let spectrum = shared_eq().lock().spectrum_db();
        let tilt_db = tilt_s.value();
        for (i, bar) in labels.iter().enumerate() {
            let octaves = (crate::eq::BAND_HZ[i] / 1000.0).log2();
            let db = (spectrum[i] + tilt_db * octaves).clamp(-80.0, 0.0);
            let steps = ((db + 80.0) / 10.0).round() as usize;
            bar.set_text(&"▇".repeat(steps.max(1).min(8)));
        }
        glib::ControlFlow::Continue
    });
    page
}

fn presets_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 6);
    let note = Label::new(Some(
        "Presets are Audiophile .ofeq files. Bell, low shelf, and high shelf land on the nearest band. Other shapes are skipped.",
    ));
    note.set_wrap(true);
    note.add_css_class("nm-muted");
    note.set_halign(Align::Start);
    page.append(&note);
    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("Preset name"));
    let save = Button::with_label("Save graphic EQ");
    save.add_css_class("nm-chip");
    save.set_halign(Align::Start);
    let list = GtkBox::new(Orientation::Vertical, 4);
    let st = state.clone();
    let entry_s = entry.clone();
    let list_s = list.clone();
    save.connect_clicked(move |_| {
        let name = entry_s.text().to_string();
        if name.trim().is_empty() {
            return;
        }
        let gains = st.player.eq_bands();
        if save_preset(name.trim(), &gains).is_ok() {
            remember_last(&current_eq_key(&st), name.trim());
            refill_presets(&list_s, &st);
        }
    });
    page.append(&entry);
    page.append(&save);
    page.append(&list);
    refill_presets(&list, &state);
    page
}

fn note_page(text: &str) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 6);
    let l = Label::new(Some(text));
    l.set_wrap(true);
    l.set_halign(Align::Start);
    l.add_css_class("nm-muted");
    page.append(&l);
    page
}

fn crate_note() -> Label {
    let l = Label::new(Some(
        "Drag the curve to move the nearest band. Tools still has the ten sliders and bypass.",
    ));
    l.set_wrap(true);
    l.set_halign(Align::Start);
    l.add_css_class("nm-muted");
    l
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
        list.append(&Label::new(Some("No presets saved yet.")));
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
        let b = Button::with_label(&preset.name);
        b.add_css_class("nm-nav-btn");
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
        list.append(&Label::new(Some("No presets saved yet.")));
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
