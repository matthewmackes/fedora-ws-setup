//! Loading transitions: shimmering skeletons while content is fetched and a soft
//! fade-in when it arrives. Animations are CSS, so GTK's reduced-motion setting
//! (`gtk-enable-animations`) turns them off automatically.

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, Spinner};

fn block(w: i32, h: i32, delay: u32) -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 0);
    b.add_css_class("nm-skel");
    b.add_css_class(&format!("nm-skel-d{}", delay % 4));
    b.set_size_request(w, h);
    b.set_halign(Align::Start);
    b
}

/// Spinner + message row, e.g. "Searching library + YouTube Music…".
pub fn status_row(text: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.add_css_class("nm-loading-row");
    let spin = Spinner::new();
    spin.start();
    let label = Label::new(Some(text));
    label.add_css_class("nm-muted");
    row.append(&spin);
    row.append(&label);
    row.set_halign(Align::Start);
    row
}

/// A track-list-shaped skeleton: `n` rows with a cover square and two text bars.
pub fn rows(n: usize) -> GtkBox {
    let col = GtkBox::new(Orientation::Vertical, 10);
    col.add_css_class("nm-skel-rows");
    for i in 0..n {
        let row = GtkBox::new(Orientation::Horizontal, 12);
        row.append(&block(48, 48, i as u32));
        let text = GtkBox::new(Orientation::Vertical, 8);
        text.set_valign(Align::Center);
        text.append(&block(220 + ((i as i32 * 37) % 120), 12, i as u32 + 1));
        text.append(&block(140 + ((i as i32 * 23) % 80), 10, i as u32 + 2));
        row.append(&text);
        col.append(&row);
    }
    col
}

/// A shelf-shaped skeleton: `n` cover cards with a title bar.
pub fn cards(n: usize) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 16);
    row.add_css_class("nm-skel-cards");
    for i in 0..n {
        let card = GtkBox::new(Orientation::Vertical, 8);
        card.append(&block(148, 148, i as u32));
        card.append(&block(110, 12, i as u32 + 1));
        row.append(&card);
    }
    row
}

/// Whole-page skeleton: a hero block, then a shelf, then rows.
pub fn page() -> GtkBox {
    let col = GtkBox::new(Orientation::Vertical, 22);
    col.add_css_class("nm-skel-page");
    col.append(&status_row("Loading…"));
    let hero = block(560, 200, 0);
    hero.set_hexpand(true);
    col.append(&hero);
    col.append(&cards(5));
    col.append(&rows(4));
    col
}

/// Replay the fade-in on `w` (call right after swapping new content in).
pub fn fade_in(w: &impl IsA<gtk4::Widget>) {
    let w = w.as_ref().clone();
    w.remove_css_class("nm-fade-in");
    glib::idle_add_local_once(move || w.add_css_class("nm-fade-in"));
}
