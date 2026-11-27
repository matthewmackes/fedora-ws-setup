//! Per-performer overview — full-bleed art + frosted facts panel.

use crate::model::{Album, Artist, Track};
use crate::performer::{self, PerformerCandidate, PerformerInfo, ResolveOutcome};
use crate::ui::shell::{bind_cover, open_album_detail, play_track, AppState};
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Stack};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub struct PerformerPage {
    root: GtkBox,
    body: GtkBox,
    state: Rc<AppState>,
    content: Stack,
}

impl PerformerPage {
    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn open(&self, artist: Artist) {
        while let Some(c) = self.body.first_child() {
            self.body.remove(&c);
        }
        let loading = super::loading::page();
        loading.set_margin_top(24);
        loading.set_margin_start(24);
        self.body.append(&loading);

        let body = self.body.clone();
        let st = self.state.clone();
        let content = self.content.clone();
        let artist_c = artist.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(performer::resolve_performer(&artist_c.id, &artist_c.name));
        });
        glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
            match rx.try_recv() {
                Ok(outcome) => {
                    render_outcome(&st, &body, &content, &artist, outcome);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    while let Some(c) = body.first_child() {
                        body.remove(&c);
                    }
                    let err = Label::new(Some("Could not load performer info."));
                    err.set_margin_start(24);
                    body.append(&err);
                    glib::ControlFlow::Break
                }
            }
        });
    }
}

pub fn build_performer_page(state: Rc<AppState>, content: Stack) -> PerformerPage {
    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("nm-performer-root");
    root.set_hexpand(true);
    root.set_vexpand(true);
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll.set_propagate_natural_height(false);
    let body = GtkBox::new(Orientation::Vertical, 0);
    body.add_css_class("nm-performer-body");
    scroll.set_child(Some(&body));
    root.append(&scroll);
    PerformerPage {
        root,
        body,
        state,
        content,
    }
}

fn render_outcome(
    state: &Rc<AppState>,
    body: &GtkBox,
    content: &Stack,
    artist: &Artist,
    outcome: ResolveOutcome,
) {
    while let Some(c) = body.first_child() {
        body.remove(&c);
    }
    let back = gtk4::Button::with_label("Back");
    back.add_css_class("nm-chip");
    back.set_halign(gtk4::Align::Start);
    back.set_margin_start(12);
    back.set_margin_top(8);
    let content_back = content.clone();
    back.connect_clicked(move |_| {
        content_back.set_visible_child_name("library");
    });
    body.append(&back);
    match outcome {
        ResolveOutcome::Ready(info) => render_ready(state, body, content, artist, info),
        ResolveOutcome::NeedsConfirm(cands) => render_confirm(state, body, content, artist, cands),
        ResolveOutcome::Empty => render_ready(
            state,
            body,
            content,
            artist,
            PerformerInfo {
                name: artist.name.clone(),
                ..Default::default()
            },
        ),
    }
}

fn render_confirm(
    state: &Rc<AppState>,
    body: &GtkBox,
    content: &Stack,
    artist: &Artist,
    cands: Vec<PerformerCandidate>,
) {
    let wrap = GtkBox::new(Orientation::Vertical, 10);
    wrap.add_css_class("nm-main");
    let h = Label::new(Some("Confirm artist"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    wrap.append(&h);
    let hint = Label::new(Some(&format!(
        "Multiple MusicBrainz matches for “{}”. Pick one — we’ll remember it.",
        artist.name
    )));
    hint.add_css_class("nm-muted");
    hint.set_wrap(true);
    hint.set_halign(Align::Start);
    wrap.append(&hint);
    for c in cands {
        let label = if c.disambiguation.is_empty() {
            format!("{} · {:.0}% · {}", c.name, c.score * 100.0, c.typ)
        } else {
            format!(
                "{} ({}) · {:.0}% · {}",
                c.name,
                c.disambiguation,
                c.score * 100.0,
                c.typ
            )
        };
        let btn = Button::with_label(&label);
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Start);
        let st = state.clone();
        let body = body.clone();
        let content = content.clone();
        let artist = artist.clone();
        let mbid = c.mbid.clone();
        btn.connect_clicked(move |_| {
            performer::remember_mbid(&artist.id, &mbid);
            while let Some(ch) = body.first_child() {
                body.remove(&ch);
            }
            body.append(&super::loading::status_row("Loading…"));
            let (tx, rx) = std::sync::mpsc::channel();
            let mbid2 = mbid.clone();
            std::thread::spawn(move || {
                let _ = tx.send(performer::fetch_artist(&mbid2));
            });
            let st2 = st.clone();
            let body2 = body.clone();
            let content2 = content.clone();
            let artist2 = artist.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
                match rx.try_recv() {
                    Ok(Ok(info)) => {
                        render_ready(&st2, &body2, &content2, &artist2, info);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(_)) => {
                        body2.append(&Label::new(Some("Failed to load artist.")));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                }
            });
        });
        wrap.append(&btn);
    }
    body.append(&wrap);
}

fn render_ready(
    state: &Rc<AppState>,
    body: &GtkBox,
    content: &Stack,
    artist: &Artist,
    info: PerformerInfo,
) {
    let display_name = if info.name.is_empty() { artist.name.clone() } else { info.name.clone() };
    // Full-bleed backdrop: exact-match fan art (MBID + Wikipedia title when known).
    super::ambient::set_context_ex(
        super::bleed::Mode::Sharp,
        super::ambient::Ctx { artist: artist.name.clone(), album: String::new(), cover_url: artist.cover_url.clone() },
        (!info.mbid.is_empty()).then(|| info.mbid.clone()),
        (!info.wiki_title.is_empty()).then(|| info.wiki_title.clone()),
    );

    // ── Hero ────────────────────────────────────────────────────────────────
    let hero = gtk4::Overlay::new();
    hero.add_css_class("nm-artist-hero");
    hero.set_hexpand(true);
    hero.set_size_request(-1, 400);
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    hero.set_child(Some(&spacer));

    let row = GtkBox::new(Orientation::Horizontal, 28);
    row.set_valign(Align::End);
    row.set_margin_start(32);
    row.set_margin_end(32);
    row.set_margin_bottom(28);

    let portrait = GtkBox::new(Orientation::Vertical, 0);
    portrait.set_size_request(176, 176);
    portrait.set_valign(Align::End);
    portrait.add_css_class("nm-artist-portrait");
    let initial = Label::new(Some(&display_name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()));
    initial.add_css_class("nm-artist-initial");
    initial.set_vexpand(true);
    initial.set_hexpand(true);
    portrait.append(&initial);
    {
        let slot: Arc<parking_lot::Mutex<Option<std::path::PathBuf>>> = Arc::new(parking_lot::Mutex::new(None));
        crate::artimg::portrait_async(&artist.name, slot.clone());
        let portrait = portrait.clone();
        let tries = Rc::new(std::cell::Cell::new(0u32));
        glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
            if let Some(path) = slot.lock().take() {
                while let Some(c) = portrait.first_child() {
                    portrait.remove(&c);
                }
                let pic = gtk4::Picture::new();
                pic.set_content_fit(gtk4::ContentFit::Cover);
                pic.set_can_shrink(true);
                pic.set_size_request(176, 176);
                pic.add_css_class("nm-fade-in");
                pic.set_filename(Some(path));
                portrait.append(&pic);
                return glib::ControlFlow::Break;
            }
            tries.set(tries.get() + 1);
            if tries.get() > 100 || portrait.root().is_none() {
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
    row.append(&portrait);

    let info_col = GtkBox::new(Orientation::Vertical, 8);
    info_col.set_hexpand(true);
    info_col.set_valign(Align::End);
    let kind = Label::new(Some(if info.typ.eq_ignore_ascii_case("person") { "ARTIST" } else { "BAND" }));
    kind.add_css_class("nm-artist-kicker");
    kind.set_halign(Align::Start);
    info_col.append(&kind);
    let name = Label::new(Some(&display_name));
    name.add_css_class("nm-artist-name");
    name.set_halign(Align::Start);
    name.set_wrap(true);
    name.set_xalign(0.0);
    info_col.append(&name);
    let facts = fact_line(&info);
    if !facts.is_empty() {
        let f = Label::new(Some(&facts));
        f.add_css_class("nm-artist-sub");
        f.set_halign(Align::Start);
        f.set_wrap(true);
        f.set_xalign(0.0);
        info_col.append(&f);
    }
    if !info.genres.is_empty() {
        let chips = GtkBox::new(Orientation::Horizontal, 8);
        for g in info.genres.iter().take(5) {
            let c = Label::new(Some(g));
            c.add_css_class("nm-pill");
            chips.append(&c);
        }
        chips.set_halign(Align::Start);
        info_col.append(&chips);
    }

    let actions = GtkBox::new(Orientation::Horizontal, 10);
    actions.set_margin_top(10);
    let play = Button::with_label("▶  Play");
    play.add_css_class("nm-transport");
    play.add_css_class("nm-play");
    let shuffle = Button::with_label("Shuffle");
    shuffle.add_css_class("nm-chip");
    let mix = Button::with_label("Instant Mix");
    mix.add_css_class("nm-chip");
    let retry = Button::with_label("Reload info");
    retry.add_css_class("nm-chip");
    actions.append(&play);
    actions.append(&shuffle);
    actions.append(&mix);
    actions.append(&retry);
    info_col.append(&actions);
    row.append(&info_col);
    hero.add_overlay(&row);
    body.append(&hero);

    // ── Content ─────────────────────────────────────────────────────────────
    let page = GtkBox::new(Orientation::Vertical, 22);
    page.add_css_class("nm-artist-content");
    page.set_margin_start(32);
    page.set_margin_end(32);
    page.set_margin_bottom(32);
    body.append(&page);

    // About
    let about = info_card("About");
    let bio_text = if info.bio.is_empty() {
        format!(
            "No biography was found for {display_name}. The full discography is below; \"Reload info\" retries MusicBrainz and Wikipedia."
        )
    } else {
        info.bio.clone()
    };
    for para in bio_text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let l = Label::new(Some(para.trim()));
        l.add_css_class("nm-bio");
        l.set_wrap(true);
        l.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        l.set_xalign(0.0);
        l.set_halign(Align::Fill);
        l.set_selectable(true);
        about.append(&l);
    }
    if !info.wiki_title.is_empty() {
        let src = Label::new(Some(&format!("Source: Wikipedia — {}", info.wiki_title.replace('_', " "))));
        src.add_css_class("nm-credit");
        src.set_halign(Align::Start);
        about.append(&src);
    }
    page.append(&about);

    // Details
    let rows = detail_rows(&info);
    if !rows.is_empty() || !info.members.is_empty() {
        let card = info_card("Details");
        let grid = gtk4::Grid::new();
        grid.set_column_spacing(24);
        grid.set_row_spacing(10);
        let mut r = 0;
        for (k, v) in rows {
            let kl = Label::new(Some(&k));
            kl.add_css_class("nm-detail-key");
            kl.set_halign(Align::Start);
            kl.set_valign(Align::Start);
            let vl = Label::new(Some(&v));
            vl.add_css_class("nm-detail-val");
            vl.set_halign(Align::Start);
            vl.set_xalign(0.0);
            vl.set_wrap(true);
            vl.set_selectable(true);
            grid.attach(&kl, 0, r, 1, 1);
            grid.attach(&vl, 1, r, 1, 1);
            r += 1;
        }
        if !info.members.is_empty() {
            let kl = Label::new(Some("Members"));
            kl.add_css_class("nm-detail-key");
            kl.set_halign(Align::Start);
            kl.set_valign(Align::Start);
            let flow = gtk4::FlowBox::new();
            flow.set_selection_mode(gtk4::SelectionMode::None);
            flow.set_max_children_per_line(6);
            flow.set_column_spacing(8);
            flow.set_row_spacing(8);
            flow.set_hexpand(true);
            for m in &info.members {
                let c = Label::new(Some(m));
                c.add_css_class("nm-pill");
                flow.insert(&c, -1);
            }
            grid.attach(&kl, 0, r, 1, 1);
            grid.attach(&flow, 1, r, 1, 1);
        }
        card.append(&grid);
        page.append(&card);
    }

    // Popular (filled when data arrives)
    let popular = info_card("Popular");
    popular.set_visible(false);
    page.append(&popular);
    // Albums
    let albums_card = info_card(if artist.id.starts_with("ytm:") { "YouTube Music albums" } else { "Albums" });
    albums_card.set_visible(false);
    page.append(&albums_card);
    // Photos
    let photos_card = info_card("Photos");
    photos_card.set_visible(false);
    page.append(&photos_card);

    // ── Actions ─────────────────────────────────────────────────────────────
    let tops_slot: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let st = state.clone();
        let tops = tops_slot.clone();
        play.connect_clicked(move |_| {
            let t = tops.borrow().clone();
            if let Some(first) = t.first() {
                st.player.set_queue(t.clone(), 0);
                play_track(&st, first.clone());
            }
        });
    }
    {
        let st = state.clone();
        let tops = tops_slot.clone();
        shuffle.connect_clicked(move |_| {
            let mut t = tops.borrow().clone();
            if t.is_empty() {
                return;
            }
            if t.len() > 1 {
                let last = t.pop().unwrap();
                t.insert(0, last);
            }
            st.player.set_shuffle(true);
            st.player.set_queue(t.clone(), 0);
            if let Some(first) = t.first() {
                play_track(&st, first.clone());
            }
        });
    }
    {
        let st = state.clone();
        let id = artist.id.clone();
        mix.connect_clicked(move |_| {
            if let Some(be) = st.backend.read().clone() {
                if let Ok(mix_tracks) = be.instant_mix(&id) {
                    if let Some(first) = mix_tracks.first() {
                        st.player.set_queue(mix_tracks.clone(), 0);
                        play_track(&st, first.clone());
                    }
                }
            }
        });
    }
    {
        let st = state.clone();
        let content = content.clone();
        let body = body.clone();
        let artist = artist.clone();
        retry.connect_clicked(move |_| {
            let page = PerformerPage { root: body.clone(), body: body.clone(), state: st.clone(), content: content.clone() };
            page.open(artist.clone());
        });
    }

    // ── Data: library albums + top tracks ──────────────────────────────────
    let be = state.backend.read().clone();
    let yt = state.ytm.read().clone();
    let artist_id = artist.id.clone();
    let ytm = artist.id.starts_with("ytm:");
    let (tx, rx) = std::sync::mpsc::channel::<(Vec<Album>, Vec<Track>)>();
    std::thread::spawn(move || {
        let backend: Option<Arc<dyn crate::backend::MediaBackend>> = if ytm { yt } else { be };
        let albums = backend.as_ref().and_then(|b| b.artist_albums(&artist_id).ok()).unwrap_or_default();
        let tops = backend.as_ref().and_then(|b| b.top_tracks(Some(&artist_id)).ok()).unwrap_or_default();
        let _ = tx.send((albums, tops));
    });
    {
        let st = state.clone();
        let content_c = content.clone();
        let popular = popular.clone();
        let albums_card = albums_card.clone();
        let tops_slot = tops_slot.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(80), move || match rx.try_recv() {
            Ok((mut albums, tops)) => {
                *tops_slot.borrow_mut() = tops.clone();
                if !tops.is_empty() {
                    popular.set_visible(true);
                    let shown: Vec<Track> = tops.iter().take(10).cloned().collect();
                    crate::ui::shell::show_track_table(&st, &popular, &shown);
                    super::loading::fade_in(&popular);
                }
                albums.sort_by(|a, b| b.year.cmp(&a.year));
                if albums.is_empty() {
                    albums_card.set_visible(false);
                } else {
                    albums_card.set_visible(true);
                    let flow = gtk4::FlowBox::new();
                    flow.set_selection_mode(gtk4::SelectionMode::None);
                    flow.set_homogeneous(true);
                    flow.set_min_children_per_line(2);
                    flow.set_max_children_per_line(8);
                    flow.set_column_spacing(16);
                    flow.set_row_spacing(18);
                    flow.set_valign(Align::Start);
                    for a in albums.iter().take(40) {
                        let btn = Button::new();
                        btn.add_css_class("nm-card");
                        let col = GtkBox::new(Orientation::Vertical, 8);
                        let img = gtk4::Image::new();
                        img.set_pixel_size(176);
                        img.set_icon_name(Some("media-optical-symbolic"));
                        bind_cover(&img, a.cover_url.as_deref());
                        col.append(&img);
                        let title = Label::new(Some(&a.name));
                        title.add_css_class("nm-card-title");
                        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                        title.set_max_width_chars(18);
                        title.set_halign(Align::Start);
                        col.append(&title);
                        if let Some(y) = a.year {
                            let yl = Label::new(Some(&y.to_string()));
                            yl.add_css_class("nm-muted");
                            yl.set_halign(Align::Start);
                            col.append(&yl);
                        }
                        btn.set_child(Some(&col));
                        let st = st.clone();
                        let content = content_c.clone();
                        let album = a.clone();
                        btn.connect_clicked(move |_| open_album_detail(&st, &content, album.clone()));
                        flow.insert(&btn, -1);
                    }
                    albums_card.append(&flow);
                    super::loading::fade_in(&albums_card);
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    // ── Photos (Commons / Fanart / AudioDB / Bandcamp), with attribution ────
    {
        type Photos = Vec<(std::path::PathBuf, String)>;
        let slot: Arc<parking_lot::Mutex<Option<Photos>>> = Arc::new(parking_lot::Mutex::new(None));
        let name = artist.name.clone();
        let mbid = (!info.mbid.is_empty()).then(|| info.mbid.clone());
        let wiki = (!info.wiki_title.is_empty()).then(|| info.wiki_title.clone());
        let slot_t = slot.clone();
        std::thread::spawn(move || {
            let imgs = crate::artimg::artist_images(&name, mbid.as_deref(), wiki.as_deref());
            let mut out: Photos = Vec::new();
            for c in imgs.candidates.iter().filter(|c| {
                matches!(c.kind, crate::artimg::Kind::Fan | crate::artimg::Kind::Backdrop | crate::artimg::Kind::Portrait)
            }) {
                if out.len() >= 8 {
                    break;
                }
                if let Some(p) = crate::artimg::fetch_good(c, crate::artimg::Use::Portrait) {
                    out.push((p, format!("{} · {}", c.source, c.license)));
                }
            }
            *slot_t.lock() = Some(out);
        });
        let photos_card = photos_card.clone();
        let tries = Rc::new(std::cell::Cell::new(0u32));
        glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            if let Some(list) = slot.lock().take() {
                if !list.is_empty() {
                    let strip = GtkBox::new(Orientation::Horizontal, 12);
                    for (path, credit) in list {
                        let pic = gtk4::Picture::new();
                        pic.set_content_fit(gtk4::ContentFit::Cover);
                        pic.set_can_shrink(true);
                        pic.set_size_request(260, 170);
                        pic.set_overflow(gtk4::Overflow::Hidden);
                        pic.add_css_class("nm-photo");
                        pic.set_tooltip_text(Some(&credit));
                        pic.set_filename(Some(path));
                        strip.append(&pic);
                    }
                    let scroll = ScrolledWindow::builder().child(&strip).hexpand(true).min_content_height(180).build();
                    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
                    photos_card.append(&scroll);
                    let credit = Label::new(Some("Hover a photo for its source and licence."));
                    credit.add_css_class("nm-credit");
                    credit.set_halign(Align::Start);
                    photos_card.append(&credit);
                    photos_card.set_visible(true);
                    super::loading::fade_in(&photos_card);
                }
                return glib::ControlFlow::Break;
            }
            tries.set(tries.get() + 1);
            if tries.get() > 200 || photos_card.root().is_none() {
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
}

/// Frosted section card with a heading.
fn info_card(title: &str) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("nm-info-card");
    let h = Label::new(Some(title));
    h.add_css_class("nm-info-title");
    h.set_halign(Align::Start);
    card.append(&h);
    card
}

fn detail_rows(info: &PerformerInfo) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    if !info.typ.is_empty() {
        rows.push(("Type".to_string(), info.typ.clone()));
    }
    if !info.begin.is_empty() {
        let k = if info.typ.eq_ignore_ascii_case("person") { "Born" } else { "Formed" };
        rows.push((k.to_string(), info.begin.clone()));
    }
    if !info.end.is_empty() {
        let k = if info.typ.eq_ignore_ascii_case("person") { "Died" } else { "Disbanded" };
        rows.push((k.to_string(), info.end.clone()));
    }
    let origin = [info.area.as_str(), info.country.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if !origin.is_empty() {
        rows.push(("Origin".to_string(), origin));
    }
    if !info.genres.is_empty() {
        rows.push(("Genres".to_string(), info.genres.join(", ")));
    }
    rows
}

fn fact_line(info: &PerformerInfo) -> String {
    let mut parts = Vec::new();
    if !info.begin.is_empty() {
        let year: String = info.begin.chars().take(4).collect();
        parts.push(if info.end.is_empty() {
            format!("Since {year}")
        } else {
            format!("{year} – {}", info.end.chars().take(4).collect::<String>())
        });
    }
    if !info.area.is_empty() {
        parts.push(info.area.clone());
    } else if !info.country.is_empty() {
        parts.push(info.country.clone());
    }
    parts.join("  ·  ")
}
