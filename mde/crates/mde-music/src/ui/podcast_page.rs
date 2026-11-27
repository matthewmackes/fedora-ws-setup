//! Podcast browser: trending directories, saved shows, and this server's shows.

use crate::model::Track;
use crate::podcasts::people::{self, GuestBio};
use crate::podcasts::{self, Episode, Guest, Show};
use crate::ui::shell::{bind_cover, play_track, AppState};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
enum Browse {
    Trending,
    Favorites,
    Server,
    Genre(String),
    Search(String),
}

struct EpisodeList {
    show: Show,
    episodes: Vec<Episode>,
    shown: usize,
}

struct Incoming {
    gen: u64,
    shows: Option<std::result::Result<Vec<Show>, String>>,
    episodes: Option<std::result::Result<(Show, Vec<Episode>), String>>,
    note: Option<String>,
    /// Updated "This server" catalog after subscribe/refresh (applied on GTK thread).
    server_shows: Option<Vec<Show>>,
}

/// Where a guest's biography lookup stands.
#[derive(Clone)]
enum BioState {
    Pending,
    Found(GuestBio),
    Missing,
}

/// The widgets of one guest card that fill in once a biography arrives.
#[derive(Clone)]
struct GuestSlot {
    name: String,
    photo: gtk4::Image,
    has_feed_photo: bool,
    role: Label,
    bio: Label,
    source: Label,
}

type BioResult = (String, std::result::Result<Option<GuestBio>, String>);

#[derive(Clone)]
pub(crate) struct PodcastPage {
    state: Rc<AppState>,
    root: GtkBox,
    list: ListBox,
    status: Label,
    now: Label,
    more: Button,
    back: Button,
    header: GtkBox,
    favorites: Rc<RefCell<Vec<Show>>>,
    server: Rc<RefCell<Vec<Show>>>,
    shown: Rc<RefCell<Vec<Show>>>,
    episodes: Rc<RefCell<Option<EpisodeList>>>,
    mode: Rc<RefCell<Browse>>,
    generation: Rc<Cell<u64>>,
    chips: Rc<RefCell<Vec<(String, Button)>>>,
    pending: Arc<parking_lot::Mutex<Option<Incoming>>>,
    bios: Rc<RefCell<HashMap<String, BioState>>>,
    slots: Rc<RefCell<Vec<GuestSlot>>>,
    bio_jobs: mpsc::Sender<String>,
    bio_done: Arc<parking_lot::Mutex<Vec<BioResult>>>,
}

impl PodcastPage {
    pub(crate) fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub(crate) fn set_server_shows(&self, shows: Vec<crate::model::Podcast>) {
        *self.server.borrow_mut() = shows.iter().map(Show::from_server).collect();
        if matches!(*self.mode.borrow(), Browse::Server) && self.episodes.borrow().is_none() {
            self.start_shows(false);
        }
    }

    fn bump(&self) -> u64 {
        let gen = self.generation.get().wrapping_add(1);
        self.generation.set(gen);
        gen
    }

    fn set_active_chip(&self, key: &str) {
        for (chip, button) in self.chips.borrow().iter() {
            if chip == key {
                button.add_css_class("nm-chip-on");
            } else {
                button.remove_css_class("nm-chip-on");
            }
        }
    }

    fn refresh_fav_chip(&self) {
        let n = self.favorites.borrow().len();
        let label = if n == 0 {
            "Favorites".to_string()
        } else {
            format!("Favorites ({n})")
        };
        for (key, button) in self.chips.borrow().iter() {
            if key == "favorites" {
                button.set_label(&label);
            }
        }
    }

    fn is_saved(&self, show: &Show) -> bool {
        let feed = feed_key(&show.feed_url);
        self.favorites.borrow().iter().any(|saved| {
            saved.id == show.id || (!feed.is_empty() && feed_key(&saved.feed_url) == feed)
        })
    }

    fn toggle_saved(&self, show: &Show) -> bool {
        let saved = {
            let mut favs = self.favorites.borrow_mut();
            let feed = feed_key(&show.feed_url);
            if let Some(i) = favs.iter().position(|saved| {
                saved.id == show.id || (!feed.is_empty() && feed_key(&saved.feed_url) == feed)
            }) {
                favs.remove(i);
                false
            } else {
                favs.insert(0, show.clone());
                true
            }
        };
        let snapshot = self.favorites.borrow().clone();
        if let Err(err) = podcasts::save_favorites(&snapshot) {
            self.status.set_text(&format!("Could not save favorites: {err:#}"));
        }
        self.refresh_fav_chip();
        if saved {
            self.subscribe_show(show);
        } else if matches!(*self.mode.borrow(), Browse::Favorites) && self.episodes.borrow().is_none()
        {
            let page = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(1), move || page.start_shows(false));
        }
        saved
    }

    fn subscribe_show(&self, show: &Show) {
        if !show.feed_url.contains("://") {
            self.status.set_text("Saved on this machine.");
            return;
        }
        let Some(backend) = self.state.backend.read().clone() else {
            podcasts::queue_subscription(show);
            self.status
                .set_text("Saved. Subscription will be tried again next time you connect.");
            return;
        };
        let show = show.clone();
        let pending = self.pending.clone();
        let gen = self.generation.get();
        std::thread::spawn(move || {
            let (note, server_shows) = match backend.subscribe_podcast(&show.feed_url) {
                Ok(()) => {
                    podcasts::forget_subscription(&show.feed_url);
                    let list = backend
                        .podcasts()
                        .unwrap_or_default()
                        .iter()
                        .map(Show::from_server)
                        .collect::<Vec<_>>();
                    (
                        "Saved and subscribed on this server. Open This server or tap Refresh for episodes."
                            .to_string(),
                        Some(list),
                    )
                }
                Err(err) => {
                    podcasts::queue_subscription(&show);
                    (
                        format!(
                            "Saved locally, but the server could not subscribe ({err:#}). Will retry on next connect."
                        ),
                        None,
                    )
                }
            };
            let mut slot = pending.lock();
            match slot.as_mut() {
                Some(item) if item.gen > gen => {}
                Some(item) => {
                    item.note = Some(note);
                    if let Some(shows) = server_shows {
                        item.server_shows = Some(shows);
                    }
                }
                None => {
                    *slot = Some(Incoming {
                        gen,
                        shows: None,
                        episodes: None,
                        note: Some(note),
                        server_shows,
                    });
                }
            }
        });
    }


    fn start_shows(&self, _append: bool) {
        *self.episodes.borrow_mut() = None;
        self.back.set_visible(false);
        self.header.set_visible(false);
        self.more.set_visible(false);
        let gen = self.bump();
        let mode = self.mode.borrow().clone();
        match mode {
            Browse::Favorites => {
                let shows = self.favorites.borrow().clone();
                let n = shows.len();
                let msg = if n == 0 {
                    "No saved shows yet. Search, then press Save.".to_string()
                } else {
                    format!("{n} saved show{}", plural(n))
                };
                self.replace_shows(shows, &msg);
            }
            Browse::Server => {
                let shows = self.server.borrow().clone();
                let n = shows.len();
                let msg = if n == 0 {
                    "This server has no podcasts.".to_string()
                } else {
                    format!("{n} show{} on this server", plural(n))
                };
                self.replace_shows(shows, &msg);
            }
            other => {
                self.status.set_text("Looking up podcasts…");
                let pending = self.pending.clone();
                std::thread::spawn(move || {
                    let result = match &other {
                        Browse::Genre(name) => podcasts::by_genre(name),
                        Browse::Search(query) => podcasts::search(query),
                        _ => podcasts::trending(),
                    }
                    .map_err(|err| format!("{err:#}"));
                    store_shows(&pending, gen, result);
                });
            }
        }
    }

    pub(crate) fn open_show(&self, show: Show) {
        self.back.set_visible(true);
        self.header.set_visible(true);
        *self.episodes.borrow_mut() = None;
        self.fill_header(&show);
        self.clear_list();
        self.more.set_visible(false);
        self.status.set_text("Loading episodes…");
        let gen = self.bump();
        let pending = self.pending.clone();
        let backend = self.state.backend.read().clone();
        std::thread::spawn(move || {
            let result = load_episodes(backend.as_ref(), &show).map(|eps| (show, eps));
            let mut slot = pending.lock();
            let keep = slot.as_ref().map(|item| gen >= item.gen).unwrap_or(true);
            if keep {
                *slot = Some(Incoming {
                    gen,
                    shows: None,
                    episodes: Some(result.map_err(|err| format!("{err:#}"))),
                    note: None,
                    server_shows: None,
                });
            }
        });
    }

    fn fill_header(&self, show: &Show) {
        while let Some(child) = self.header.first_child() {
            self.header.remove(&child);
        }
        self.header.add_css_class("nm-hero-stage");
        let art = gtk4::Image::new();
        art.add_css_class("nm-hero-art");
        art.set_pixel_size(168);
        art.set_size_request(168, 168);
        art.set_valign(Align::Start);
        art.set_icon_name(Some("multimedia-player-symbolic"));
        bind_cover(&art, show.artwork.as_deref());
        let text = GtkBox::new(Orientation::Vertical, 6);
        text.set_hexpand(true);
        let title = Label::new(Some(&show.title));
        title.add_css_class("nm-ep-title");
        title.set_halign(Align::Start);
        title.set_wrap(true);
        title.set_xalign(0.0);
        text.append(&title);
        let by = [show.author.trim(), show.source.trim()]
            .iter()
            .filter(|part| !part.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ");
        if !by.is_empty() {
            let by = Label::new(Some(&by));
            by.add_css_class("nm-ep-meta");
            by.set_halign(Align::Start);
            by.set_xalign(0.0);
            text.append(&by);
        }
        let about = podcasts::strip_html(&show.description);
        if !about.is_empty() {
            let desc = Label::new(Some(&clip_text(&about, 520)));
            desc.add_css_class("nm-ep-desc");
            desc.set_halign(Align::Start);
            desc.set_wrap(true);
            desc.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            desc.set_xalign(0.0);
            text.append(&desc);
        }
        let actions = GtkBox::new(Orientation::Vertical, 6);
        actions.set_valign(Align::Start);
        let save = save_button(self, show);
        let refresh = Button::with_label("Refresh");
        refresh.add_css_class("nm-chip");
        refresh.set_tooltip_text(Some("Reload episodes from the feed and server"));
        let page = self.clone();
        let show_r = show.clone();
        refresh.connect_clicked(move |_| page.refresh_show(&show_r));
        actions.append(&save);
        actions.append(&refresh);
        self.header.append(&art);
        self.header.append(&text);
        self.header.append(&actions);
    }

    fn refresh_show(&self, show: &Show) {
        self.status.set_text("Refreshing episodes…");
        let gen = self.bump();
        let pending = self.pending.clone();
        let backend = self.state.backend.read().clone();
        let show = show.clone();
        std::thread::spawn(move || {
            let refresh_note = if let Some(be) = backend.as_ref() {
                match be.refresh_podcasts() {
                    Ok(()) => None,
                    Err(err) => {
                        tracing::warn!(error = %err, "refreshPodcasts failed");
                        Some(format!("Server refresh failed: {err:#}"))
                    }
                }
            } else {
                Some("No server is connected; reloaded the feed only.".into())
            };
            if refresh_note.is_none() {
                std::thread::sleep(Duration::from_millis(800));
            }
            let result = load_episodes(backend.as_ref(), &show).map(|eps| (show, eps));
            let mut slot = pending.lock();
            let keep = slot.as_ref().map(|item| gen >= item.gen).unwrap_or(true);
            if keep {
                *slot = Some(Incoming {
                    gen,
                    shows: None,
                    episodes: Some(result.map_err(|err| format!("{err:#}"))),
                    note: refresh_note,
                    server_shows: None,
                });
            }
        });
    }

    fn replace_shows(&self, shows: Vec<Show>, status: &str) {
        self.clear_list();
        self.add_show_rows(&shows);
        *self.shown.borrow_mut() = shows;
        self.status.set_text(status);
        self.mark_playing();
    }

    fn show_episodes(&self, show: Show, episodes: Vec<Episode>) {
        let shown = podcasts::PAGE.min(episodes.len());
        self.fill_header(&show);
        self.back.set_visible(true);
        self.header.set_visible(true);
        self.clear_list();
        self.add_episode_rows(&show, &episodes[..shown]);
        self.more.set_label("More episodes");
        self.more.set_visible(shown < episodes.len());
        let n = episodes.len();
        self.status.set_text(&format!(
            "{shown} of {n} episode{} · newest first",
            plural(n)
        ));
        *self.episodes.borrow_mut() = Some(EpisodeList {
            show,
            episodes,
            shown,
        });
        self.mark_playing();
    }

    fn more_episodes(&self) {
        let mut view = self.episodes.borrow_mut();
        let Some(list) = view.as_mut() else {
            return;
        };
        let next = (list.shown + podcasts::PAGE).min(list.episodes.len());
        if next == list.shown {
            self.more.set_visible(false);
            return;
        }
        let start = list.shown;
        list.shown = next;
        let show = list.show.clone();
        let slice: Vec<Episode> = list.episodes[start..next].to_vec();
        let total = list.episodes.len();
        let shown = list.shown;
        drop(view);
        self.add_episode_rows(&show, &slice);
        self.more.set_visible(shown < total);
        self.status
            .set_text(&format!("{shown} of {total} episode{} · newest first", plural(total)));
        self.mark_playing();
    }

    fn clear_list(&self) {
        self.slots.borrow_mut().clear();
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
    }

    fn add_show_rows(&self, shows: &[Show]) {
        for show in shows {
            let row = ListBoxRow::new();
            row.add_css_class("nm-lib-row");
            let line = row_line();
            let art = cover_image(48);
            bind_cover(&art, show.artwork.as_deref());
            let open = open_button(&show.title, &show.subtitle());
            let page = self.clone();
            let chosen = show.clone();
            open.connect_clicked(move |_| page.open_show(chosen.clone()));
            let save = save_button(self, show);
            line.append(&art);
            line.append(&open);
            line.append(&save);
            row.set_child(Some(&line));
            self.list.append(&row);
        }
    }

    fn add_episode_rows(&self, show: &Show, episodes: &[Episode]) {
        let mut existing = 0;
        while self.list.row_at_index(existing).is_some() {
            existing += 1;
        }
        for (offset, episode) in episodes.iter().enumerate() {
            let index = existing as usize + offset;
            let row = self.episode_hero(show, episode, index);
            self.list.append(&row);
        }
    }

    /// One episode, presented as a hero: big art, title, notes, play, and its guests.
    fn episode_hero(&self, show: &Show, episode: &Episode, index: usize) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("nm-lib-row");
        row.add_css_class("nm-ep-row");
        row.set_activatable(false);

        let card = GtkBox::new(Orientation::Horizontal, 20);
        card.add_css_class("nm-ep-hero");

        let art = gtk4::Image::new();
        art.add_css_class("nm-hero-art");
        art.set_pixel_size(280);
        art.set_size_request(280, 280);
        art.set_valign(Align::Start);
        art.set_icon_name(Some("multimedia-player-symbolic"));
        let art_url = episode.artwork.clone().or_else(|| show.artwork.clone());
        bind_cover(&art, art_url.as_deref());

        let body = GtkBox::new(Orientation::Vertical, 8);
        body.set_hexpand(true);

        let mut meta = Vec::new();
        if index == 0 {
            meta.push("LATEST".to_string());
        }
        let subtitle = episode.subtitle_meta();
        if !subtitle.is_empty() {
            meta.push(subtitle);
        }
        if !meta.is_empty() {
            let meta = Label::new(Some(&meta.join(" · ")));
            meta.add_css_class("nm-ep-meta");
            meta.set_halign(Align::Start);
            meta.set_xalign(0.0);
            body.append(&meta);
        }

        let title = Label::new(Some(&episode.title));
        title.add_css_class("nm-ep-title");
        title.set_halign(Align::Start);
        title.set_wrap(true);
        title.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        title.set_xalign(0.0);
        body.append(&title);

        let notes = episode.notes();
        if !notes.is_empty() {
            const SHORT: usize = 420;
            let full = clip_text(&notes, 6000);
            let desc = Label::new(Some(&clip_text(&notes, SHORT)));
            desc.add_css_class("nm-ep-desc");
            desc.set_halign(Align::Start);
            desc.set_wrap(true);
            desc.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            desc.set_xalign(0.0);
            body.append(&desc);
            if notes.chars().count() > SHORT {
                let toggle = Button::with_label("Show full notes");
                toggle.add_css_class("nm-chip");
                toggle.set_halign(Align::Start);
                let short = clip_text(&notes, SHORT);
                let open = Rc::new(Cell::new(false));
                toggle.connect_clicked(move |button| {
                    let now = !open.get();
                    open.set(now);
                    desc.set_text(if now { &full } else { &short });
                    button.set_label(if now { "Show less" } else { "Show full notes" });
                });
                body.append(&toggle);
            }
        }

        let play = Button::with_label("Play episode");
        play.add_css_class("nm-chip");
        play.add_css_class("nm-chip-on");
        play.set_halign(Align::Start);
        play.update_property(&[gtk4::accessible::Property::Label(&format!(
            "Play {}",
            episode.title
        ))]);
        let page = self.clone();
        play.connect_clicked(move |_| page.play_index(index));
        body.append(&play);

        if !episode.guests.is_empty() {
            let heading = Label::new(Some(if episode.guests.len() == 1 {
                "ABOUT THE GUEST"
            } else {
                "ABOUT THE GUESTS"
            }));
            heading.add_css_class("nm-ep-section");
            heading.set_halign(Align::Start);
            body.append(&heading);
            for guest in &episode.guests {
                body.append(&self.guest_card(guest));
            }
        }

        card.append(&art);
        card.append(&body);
        row.set_child(Some(&card));
        row
    }

    fn guest_card(&self, guest: &Guest) -> GtkBox {
        let card = GtkBox::new(Orientation::Horizontal, 14);
        card.add_css_class("nm-guest-card");

        let photo = gtk4::Image::new();
        photo.add_css_class("nm-guest-photo");
        photo.set_pixel_size(72);
        photo.set_size_request(72, 72);
        photo.set_valign(Align::Start);
        photo.set_icon_name(Some("avatar-default-symbolic"));
        let has_feed_photo = guest.img.is_some();
        bind_cover(&photo, guest.img.as_deref());

        let col = GtkBox::new(Orientation::Vertical, 3);
        col.set_hexpand(true);
        let name = Label::new(Some(&guest.name));
        name.add_css_class("nm-guest-name");
        name.set_halign(Align::Start);
        name.set_xalign(0.0);
        name.set_wrap(true);
        let role = Label::new(Some("Guest"));
        role.add_css_class("nm-ep-meta");
        role.set_halign(Align::Start);
        role.set_xalign(0.0);
        let bio = Label::new(Some("Looking up biography…"));
        bio.add_css_class("nm-guest-bio");
        bio.set_halign(Align::Start);
        bio.set_wrap(true);
        bio.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        bio.set_xalign(0.0);
        let source = Label::new(None);
        source.add_css_class("nm-credit");
        source.set_halign(Align::Start);
        source.set_xalign(0.0);
        source.set_wrap(true);
        source.set_visible(false);
        col.append(&name);
        col.append(&role);
        col.append(&bio);
        col.append(&source);
        card.append(&photo);
        card.append(&col);

        let slot = GuestSlot {
            name: guest.name.clone(),
            photo,
            has_feed_photo,
            role,
            bio,
            source,
        };
        let known = self.bios.borrow().get(&bio_key(&guest.name)).cloned();
        match known {
            Some(BioState::Pending) => self.slots.borrow_mut().push(slot),
            Some(state) => paint_guest(&slot, &state),
            None => {
                self.bios
                    .borrow_mut()
                    .insert(bio_key(&guest.name), BioState::Pending);
                self.slots.borrow_mut().push(slot);
                let _ = self.bio_jobs.send(guest.name.clone());
            }
        }
        card
    }

    fn apply_bios(&self) {
        let done: Vec<BioResult> = std::mem::take(&mut *self.bio_done.lock());
        for (name, result) in done {
            let key = bio_key(&name);
            let state = match result {
                Ok(Some(bio)) => BioState::Found(bio),
                Ok(None) => BioState::Missing,
                Err(err) => {
                    tracing::warn!(guest = %name, error = %err, "guest biography lookup failed");
                    // Not cached: forget it so the next view retries.
                    self.bios.borrow_mut().remove(&key);
                    for slot in self.slots.borrow().iter().filter(|s| bio_key(&s.name) == key) {
                        slot.bio.set_text("Biography unavailable right now. Check the connection and Refresh.");
                    }
                    continue;
                }
            };
            for slot in self.slots.borrow().iter().filter(|s| bio_key(&s.name) == key) {
                paint_guest(slot, &state);
            }
            self.bios.borrow_mut().insert(key, state);
        }
    }

    fn play_index(&self, index: usize) {
        let view = self.episodes.borrow().clone();
        let Some(view) = view.as_ref() else {
            return;
        };
        let Some(episode) = view.episodes.get(index) else {
            return;
        };
        let tracks: Vec<Track> = view
            .episodes
            .iter()
            .map(|episode| episode.to_track(&view.show))
            .collect();
        self.state.player.set_queue(tracks, index);
        let track = episode.to_track(&view.show);
        podcasts::arm_resume(&track.id);
        self.now.set_text(&episode.title);
        play_track(&self.state, track);
        self.mark_playing();
    }

    fn tune_step(&self, delta: i32) {
        let current = self.state.player.current();
        if !current.as_ref().is_some_and(podcasts::is_podcast_track) {
            self.status.set_text("Pick an episode to start.");
            return;
        }
        let Some(track) = self.state.player.step_clamped(delta) else {
            self.status.set_text(if delta > 0 {
                "That's the last episode."
            } else {
                "That's the first episode."
            });
            return;
        };
        podcasts::arm_resume(&track.id);
        self.now.set_text(&track.title);
        play_track(&self.state, track);
        self.reveal_playing();
        self.mark_playing();
    }

    fn reveal_playing(&self) {
        let Some(current) = self.state.player.current() else {
            return;
        };
        let mut view = self.episodes.borrow_mut();
        let Some(list) = view.as_mut() else {
            return;
        };
        let Some(index) = list
            .episodes
            .iter()
            .position(|episode| episode.to_track(&list.show).id == current.id)
        else {
            return;
        };
        if index < list.shown {
            return;
        }
        let start = list.shown;
        list.shown = index + 1;
        let show = list.show.clone();
        let slice: Vec<Episode> = list.episodes[start..list.shown].to_vec();
        drop(view);
        self.add_episode_rows(&show, &slice);
        self.more.set_visible(false);
    }

    fn mark_playing(&self) {
        let current = self.state.player.current();
        let current_id = current.as_ref().map(|track| track.id.clone());
        let episodes = self.episodes.borrow();
        let mut i = 0;
        while let Some(row) = self.list.row_at_index(i) {
            let on = if let Some(list) = episodes.as_ref() {
                current_id.as_ref().is_some_and(|id| {
                    list.episodes
                        .get(i as usize)
                        .is_some_and(|episode| episode.to_track(&list.show).id == *id)
                })
            } else {
                false
            };
            if on {
                row.add_css_class("nm-station-on");
            } else {
                row.remove_css_class("nm-station-on");
            }
            i += 1;
        }
        drop(episodes);
        match current {
            Some(track) if podcasts::is_podcast_track(&track) => {
                if self.now.text().as_str() != track.title {
                    self.now.set_text(&track.title);
                }
            }
            _ => {
                if self.now.text().as_str() != "Not playing" {
                    self.now.set_text("Not playing");
                }
            }
        }
    }

    fn apply_incoming(&self, incoming: Incoming) {
        if let Some(shows) = incoming.server_shows {
            *self.server.borrow_mut() = shows;
            if matches!(*self.mode.borrow(), Browse::Server) && self.episodes.borrow().is_none() {
                let n = self.server.borrow().len();
                self.replace_shows(
                    self.server.borrow().clone(),
                    &format!("{n} show{} on this server", plural(n)),
                );
            }
        }
        if let Some(result) = incoming.shows {
            match result {
                Ok(shows) => {
                    let msg = describe(&self.mode.borrow(), shows.len());
                    self.replace_shows(shows, &msg);
                }
                Err(err) => self.status.set_text(&format!("Podcast directory unavailable: {err}")),
            }
        }
        if let Some(result) = incoming.episodes {
            match result {
                Ok((show, episodes)) if episodes.is_empty() => {
                    let extra = incoming
                        .note
                        .as_deref()
                        .map(|note| format!(" {note}"))
                        .unwrap_or_default();
                    self.status
                        .set_text(&format!("This show has no episodes.{extra}"));
                    *self.episodes.borrow_mut() = Some(EpisodeList {
                        show,
                        episodes,
                        shown: 0,
                    });
                }
                Ok((show, episodes)) => {
                    self.show_episodes(show, episodes);
                    if let Some(note) = incoming.note.as_deref() {
                        let current = self.status.text();
                        self.status.set_text(&format!("{current} · {note}"));
                    }
                }
                Err(err) => {
                    let extra = incoming
                        .note
                        .as_deref()
                        .map(|note| format!(" {note}"))
                        .unwrap_or_default();
                    self.status
                        .set_text(&format!("Could not load episodes: {err}.{extra}"));
                }
            }
        } else if let Some(note) = incoming.note {
            self.status.set_text(&note);
        }
    }
}

impl Clone for EpisodeList {
    fn clone(&self) -> Self {
        Self {
            show: self.show.clone(),
            episodes: self.episodes.clone(),
            shown: self.shown,
        }
    }
}

fn store_shows(
    pending: &Arc<parking_lot::Mutex<Option<Incoming>>>,
    gen: u64,
    result: std::result::Result<Vec<Show>, String>,
) {
    let mut slot = pending.lock();
    let keep = slot.as_ref().map(|item| gen >= item.gen).unwrap_or(true);
    if keep {
        *slot = Some(Incoming {
            gen,
            shows: Some(result),
            episodes: None,
            note: None,
            server_shows: None,
        });
    }
}

fn load_episodes(
    backend: Option<&Arc<dyn crate::backend::MediaBackend>>,
    show: &Show,
) -> anyhow::Result<Vec<Episode>> {
    let mut server_eps = Vec::new();
    if let Some(id) = show.id.strip_prefix("server:") {
        if let Some(backend) = backend {
            if let Ok(found) = backend.podcast_episodes(id) {
                server_eps = found
                    .into_iter()
                    .filter_map(|episode| Episode::from_server(id, &episode, show.artwork.clone()))
                    .collect();
            }
        }
    }
    // Always try the live feed so undownloaded / skipped server episodes still appear.
    let feed_eps = match podcasts::episodes_for_feed(show) {
        Ok(eps) => eps,
        Err(err) => {
            if server_eps.is_empty() {
                return Err(err);
            }
            tracing::warn!(error = %err, "feed refresh failed; using server episodes only");
            Vec::new()
        }
    };
    let merged = podcasts::merge_episode_lists(server_eps, feed_eps);
    if merged.is_empty() {
        anyhow::bail!(
            "no playable episodes yet — try Refresh, or Save the show so the server can download"
        );
    }
    Ok(merged)
}

fn describe(mode: &Browse, n: usize) -> String {
    match mode {
        Browse::Genre(name) => format!("{n} {name} shows"),
        Browse::Search(query) => format!("{n} matches for {query}"),
        Browse::Favorites => format!("{n} saved show{}", plural(n)),
        Browse::Server => format!("{n} show{} on this server", plural(n)),
        Browse::Trending => format!("{n} trending shows"),
    }
}

fn bio_key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn clip_text(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    let end = cut.rfind(". ").map(|i| i + 1).or_else(|| cut.rfind(' ')).unwrap_or(cut.len());
    format!("{}…", cut[..end].trim_end())
}

fn paint_guest(slot: &GuestSlot, state: &BioState) {
    match state {
        BioState::Found(bio) => {
            if !bio.descriptor.is_empty() {
                let mut d = bio.descriptor.clone();
                if let Some(first) = d.get(..1) {
                    d = format!("{}{}", first.to_uppercase(), &d[1..]);
                }
                slot.role.set_text(&format!("Guest · {d}"));
            }
            slot.bio.set_text(&bio.bio);
            slot.source.set_markup(&format!(
                "<a href=\"{}\">Wikipedia — {}</a>",
                glib::markup_escape_text(&bio.url),
                glib::markup_escape_text(&bio.article)
            ));
            slot.source.set_visible(true);
            if !slot.has_feed_photo {
                bind_cover(&slot.photo, bio.photo.as_deref());
            }
        }
        BioState::Missing => slot.bio.set_text(&format!(
            "No public biography found for {}.",
            slot.name
        )),
        BioState::Pending => {}
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn feed_key(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn row_line() -> GtkBox {
    let line = GtkBox::new(Orientation::Horizontal, 10);
    line.set_margin_top(6);
    line.set_margin_bottom(6);
    line.set_margin_start(8);
    line.set_margin_end(8);
    line
}

fn cover_image(px: i32) -> gtk4::Image {
    let art = gtk4::Image::new();
    art.add_css_class("nm-lib-cover");
    art.set_pixel_size(px);
    art.set_size_request(px, px);
    art.set_icon_name(Some("multimedia-player-symbolic"));
    art
}

fn open_button(title: &str, subtitle: &str) -> Button {
    let button = Button::new();
    button.add_css_class("nm-open");
    button.set_hexpand(true);
    button.set_has_frame(false);
    button.set_child(Some(&text_column(title, subtitle)));
    button.update_property(&[gtk4::accessible::Property::Label(title)]);
    button
}

fn text_column(title: &str, subtitle: &str) -> GtkBox {
    let text = GtkBox::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    let name = Label::new(Some(title));
    name.add_css_class("nm-lib-label");
    name.set_halign(Align::Start);
    name.set_hexpand(true);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name.set_xalign(0.0);
    let sub = Label::new(Some(subtitle));
    sub.add_css_class("nm-muted");
    sub.set_halign(Align::Start);
    sub.set_hexpand(true);
    sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    sub.set_xalign(0.0);
    text.append(&name);
    text.append(&sub);
    text
}

fn save_button(page: &PodcastPage, show: &Show) -> Button {
    let saved = page.is_saved(show);
    let button = Button::with_label(if saved { "Saved" } else { "Save" });
    button.add_css_class("nm-save");
    if saved {
        button.add_css_class("saved");
    }
    button.set_valign(Align::Center);
    button.set_tooltip_text(Some("Save this show"));
    let page = page.clone();
    let show = show.clone();
    button.connect_clicked(move |button| {
        let now = page.toggle_saved(&show);
        button.set_label(if now { "Saved" } else { "Save" });
        if now {
            button.add_css_class("saved");
        } else {
            button.remove_css_class("saved");
        }
    });
    button
}

pub(crate) fn build_podcast_page(state: Rc<AppState>) -> PodcastPage {
    let root = GtkBox::new(Orientation::Vertical, 8);
    root.add_css_class("nm-main");
    root.set_margin_top(8);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_margin_bottom(8);

    let heading = Label::new(Some("Podcasts"));
    heading.add_css_class("nm-heading");
    heading.set_halign(Align::Start);
    let blurb = Label::new(Some(
        "Search Apple Podcasts / Podcast Index, or open This server for subscribed shows. Save subscribes on your media server. Refresh reloads episodes (needed for large feeds like The Daily).",
    ));
    blurb.add_css_class("nm-muted");
    blurb.set_halign(Align::Start);
    blurb.set_wrap(true);
    blurb.set_xalign(0.0);

    let tune = GtkBox::new(Orientation::Horizontal, 8);
    tune.add_css_class("nm-tune");
    let prev = Button::with_label("Previous");
    prev.add_css_class("nm-chip");
    prev.set_widget_name("nm-podcast-prev");
    let now = Label::new(Some("Not playing"));
    now.add_css_class("nm-card-title");
    now.set_widget_name("nm-podcast-now");
    now.set_hexpand(true);
    now.set_halign(Align::Center);
    now.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let next = Button::with_label("Next");
    next.add_css_class("nm-chip");
    next.set_widget_name("nm-podcast-next");
    tune.append(&prev);
    tune.append(&now);
    tune.append(&next);

    let search_row = GtkBox::new(Orientation::Horizontal, 8);
    let search = Entry::new();
    search.add_css_class("nm-search");
    search.set_placeholder_text(Some("Show, host, or description"));
    search.set_hexpand(true);
    search.set_widget_name("nm-podcast-search");
    let search_btn = Button::with_label("Search");
    search_btn.add_css_class("nm-chip");
    search_btn.set_widget_name("nm-podcast-search-btn");
    search_row.append(&search);
    search_row.append(&search_btn);

    let chips_box = GtkBox::new(Orientation::Horizontal, 6);
    let chips_scroll = ScrolledWindow::builder().child(&chips_box).build();
    chips_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    chips_scroll.set_min_content_height(48);

    let back = Button::with_label("Back");
    back.add_css_class("nm-chip");
    back.set_halign(Align::Start);
    back.set_visible(false);
    back.set_widget_name("nm-podcast-back");

    let header = GtkBox::new(Orientation::Horizontal, 12);
    header.set_visible(false);

    let status = Label::new(Some("Looking up podcasts…"));
    status.add_css_class("nm-muted");
    status.set_halign(Align::Start);
    status.set_wrap(true);
    status.set_xalign(0.0);
    status.set_widget_name("nm-podcast-status");

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_activate_on_single_click(true);
    list.add_css_class("nm-library-list");
    list.set_widget_name("nm-podcast-list");
    let list_scroll = ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hexpand(true)
        .build();

    let more = Button::with_label("More episodes");
    more.add_css_class("nm-chip");
    more.set_halign(Align::Start);
    more.set_visible(false);
    more.set_widget_name("nm-podcast-more");

    root.append(&heading);
    root.append(&blurb);
    root.append(&tune);
    root.append(&search_row);
    root.append(&chips_scroll);
    root.append(&back);
    root.append(&header);
    root.append(&status);
    root.append(&list_scroll);
    root.append(&more);

    let (bio_jobs, bio_rx) = mpsc::channel::<String>();
    let bio_done: Arc<parking_lot::Mutex<Vec<BioResult>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));
    {
        let done = bio_done.clone();
        std::thread::spawn(move || {
            while let Ok(name) = bio_rx.recv() {
                let result = people::lookup_bio(&name).map_err(|err| format!("{err:#}"));
                done.lock().push((name, result));
            }
        });
    }

    let page = PodcastPage {
        state,
        root,
        list: list.clone(),
        status: status.clone(),
        now: now.clone(),
        more: more.clone(),
        back: back.clone(),
        header: header.clone(),
        favorites: Rc::new(RefCell::new(podcasts::load_favorites())),
        server: Rc::new(RefCell::new(Vec::new())),
        shown: Rc::new(RefCell::new(Vec::new())),
        episodes: Rc::new(RefCell::new(None)),
        mode: Rc::new(RefCell::new(Browse::Trending)),
        generation: Rc::new(Cell::new(0)),
        chips: Rc::new(RefCell::new(Vec::new())),
        pending: Arc::new(parking_lot::Mutex::new(None)),
        bios: Rc::new(RefCell::new(HashMap::new())),
        slots: Rc::new(RefCell::new(Vec::new())),
        bio_jobs,
        bio_done,
    };

    let ignore = Rc::new(Cell::new(false));
    let search_gen = Rc::new(Cell::new(0u64));
    let mut specs = vec![
        ("trending".into(), "Trending".into(), Browse::Trending),
        ("favorites".into(), "Favorites".into(), Browse::Favorites),
        ("server".into(), "This server".into(), Browse::Server),
    ];
    for name in podcasts::genre_names() {
        specs.push((
            format!("genre:{name}"),
            (*name).to_string(),
            Browse::Genre((*name).to_string()),
        ));
    }
    for (key, label, mode) in specs {
        let button = Button::with_label(&label);
        button.add_css_class("nm-chip");
        let page_b = page.clone();
        let ignore_b = ignore.clone();
        let search_gen_b = search_gen.clone();
        let search_b = search.clone();
        let chip_key = key.clone();
        button.connect_clicked(move |_| {
            search_gen_b.set(search_gen_b.get().wrapping_add(1));
            ignore_b.set(true);
            search_b.set_text("");
            ignore_b.set(false);
            *page_b.mode.borrow_mut() = mode.clone();
            page_b.set_active_chip(&chip_key);
            page_b.start_shows(false);
        });
        chips_box.append(&button);
        page.chips.borrow_mut().push((key, button));
    }

    let run_search: Rc<dyn Fn()> = Rc::new({
        let page = page.clone();
        let search = search.clone();
        move || {
            let query = search.text().trim().to_string();
            if query.is_empty() {
                *page.mode.borrow_mut() = Browse::Trending;
                page.set_active_chip("trending");
            } else {
                *page.mode.borrow_mut() = Browse::Search(query);
                page.set_active_chip("");
            }
            page.start_shows(false);
        }
    });
    {
        let run_search = Rc::clone(&run_search);
        let search_gen = search_gen.clone();
        let ignore = ignore.clone();
        search.connect_changed(move |_| {
            if ignore.get() {
                return;
            }
            let gen = search_gen.get().wrapping_add(1);
            search_gen.set(gen);
            let run_search = Rc::clone(&run_search);
            let search_gen = search_gen.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                if search_gen.get() == gen {
                    run_search();
                }
            });
        });
    }
    {
        let run_search = Rc::clone(&run_search);
        let search_gen = search_gen.clone();
        search_btn.connect_clicked(move |_| {
            search_gen.set(search_gen.get().wrapping_add(1));
            run_search();
        });
    }
    {
        let run_search = Rc::clone(&run_search);
        let search_gen = search_gen.clone();
        search.connect_activate(move |_| {
            search_gen.set(search_gen.get().wrapping_add(1));
            run_search();
        });
    }
    {
        let page = page.clone();
        back.connect_clicked(move |_| page.start_shows(false));
    }
    {
        let page = page.clone();
        prev.connect_clicked(move |_| page.tune_step(-1));
    }
    {
        let page = page.clone();
        next.connect_clicked(move |_| page.tune_step(1));
    }
    {
        let page = page.clone();
        more.connect_clicked(move |_| page.more_episodes());
    }
    {
        let page = page.clone();
        list.connect_row_activated(move |_, row| {
            let index = row.index();
            if index < 0 {
                return;
            }
            let index = index as usize;
            if page.episodes.borrow().is_some() {
                page.play_index(index);
            } else if let Some(show) = page.shown.borrow().get(index).cloned() {
                page.open_show(show);
            }
        });
    }
    {
        let page = page.clone();
        glib::timeout_add_local(Duration::from_millis(200), move || {
            if let Some(incoming) = page.pending.lock().take() {
                if incoming.gen == page.generation.get() || incoming.note.is_some() {
                    page.apply_incoming(incoming);
                }
            }
            page.apply_bios();
            page.mark_playing();
            glib::ControlFlow::Continue
        });
    }

    page.set_active_chip("trending");
    page.refresh_fav_chip();
    page.start_shows(false);
    page
}
