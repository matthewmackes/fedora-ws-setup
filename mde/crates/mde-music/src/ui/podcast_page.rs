//! Podcasts: a magazine-style library.
//!
//! Four screens in one stack. **Home** leads with Continue listening, the newest
//! episodes from your shows, and your shows as cover tiles. **Discover** is the
//! directory (search, trending, genres). **Show** has a masthead, filterable episode
//! cards, and web background (reviews, related shows). **Episode** is a full page with
//! playback controls, notes, chapters, a synced transcript, and guest biographies.

use crate::model::Track;
use crate::podcasts::library::{self, HistoryEntry};
use crate::podcasts::people::{self, GuestBio};
use crate::podcasts::web::{self, Chapter, ShowExtras, TranscriptLine};
use crate::podcasts::{self, Episode, Guest, Show};
use crate::ui::shell::{bind_cover, play_track, AppState};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, FlowBox, Label, ListBox, ListBoxRow, Orientation,
    Overlay, ProgressBar, ScrolledWindow, SelectionMode, Stack, StackTransitionType,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const TILE: i32 = 176;
const CARD: i32 = 208;
const NEW_DAYS: i64 = 7;
const SPEEDS: [f64; 6] = [1.0, 1.25, 1.5, 1.75, 2.0, 0.8];

#[derive(Clone)]
enum Browse {
    Trending,
    Genre(String),
    Search(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Unplayed,
    Downloaded,
    InProgress,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sleep {
    Off,
    At(Instant, u32),
    EndOfEpisode,
}

struct EpisodeList {
    show: Show,
    episodes: Vec<Episode>,
}

#[derive(Clone)]
struct EpCtx {
    show: Show,
    list: Vec<Episode>,
    index: usize,
}

/// Work finished on a background thread, applied on the GTK thread.
enum Msg {
    Shows {
        gen: u64,
        result: Result<Vec<Show>, String>,
    },
    Episodes {
        gen: u64,
        result: Result<(Show, Vec<Episode>), String>,
        note: Option<String>,
    },
    Note(String),
    ServerShows(Vec<Show>),
    Extras(String, Box<ShowExtras>),
    Latest(Vec<(Show, Episode)>),
    Chapters(String, Vec<Chapter>),
    Transcript(String, Result<Vec<TranscriptLine>, String>),
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

/// One episode card on the show page, kept so its buttons can follow playback.
struct CardSlot {
    id: String,
    index: usize,
    episode: Episode,
    play: Button,
    played: Button,
    download: Button,
    title: Label,
    new_tag: Label,
    progress: ProgressBar,
}

/// Live widgets of the open episode page.
struct EpUi {
    id: String,
    play: Button,
    played: Button,
    download: Button,
    speed: Button,
    sleep: Button,
    chapters_chip: Button,
    chapters_list: ListBox,
    chapters_note: Label,
    chapters: Vec<(u64, ListBoxRow)>,
    transcript_list: ListBox,
    transcript_note: Label,
    transcript: Vec<(Option<u64>, ListBoxRow)>,
    transcript_requested: bool,
    highlighted: (Option<usize>, Option<usize>),
}

#[derive(Clone)]
pub(crate) struct PodcastPage {
    state: Rc<AppState>,
    root: GtkBox,
    stack: Stack,
    nav: Rc<RefCell<Vec<String>>>,
    banner: Label,
    banner_gen: Rc<Cell<u64>>,
    // home
    cont_wrap: GtkBox,
    cont_row: GtkBox,
    cont_sig: Rc<RefCell<String>>,
    new_wrap: GtkBox,
    new_row: GtkBox,
    shows_grid: FlowBox,
    shows_empty: Label,
    library_shows: Rc<RefCell<Vec<Show>>>,
    latest: Rc<RefCell<Vec<(Show, Episode)>>>,
    // discover
    d_status: Label,
    d_grid: FlowBox,
    d_shows: Rc<RefCell<Vec<Show>>>,
    chips: Rc<RefCell<Vec<(String, Button)>>>,
    mode: Rc<RefCell<Browse>>,
    dir_gen: Rc<Cell<u64>>,
    // show
    s_masthead: GtkBox,
    s_list: ListBox,
    s_status: Label,
    s_more: Button,
    s_about: GtkBox,
    s_filters: Rc<RefCell<Vec<(Filter, Button)>>>,
    filter: Rc<Cell<Filter>>,
    shown_n: Rc<Cell<usize>>,
    episodes: Rc<RefCell<Option<EpisodeList>>>,
    current_show: Rc<RefCell<Option<Show>>>,
    cards: Rc<RefCell<Vec<CardSlot>>>,
    ep_gen: Rc<Cell<u64>>,
    extras: Rc<RefCell<HashMap<String, ShowExtras>>>,
    // episode page
    e_body: GtkBox,
    ctx: Rc<RefCell<Option<EpCtx>>>,
    ep_ui: Rc<RefCell<Option<EpUi>>>,
    // playback helpers
    speed: Rc<Cell<f64>>,
    speed_applied: Rc<RefCell<String>>,
    sleep: Rc<Cell<Sleep>>,
    auto_played: Rc<RefCell<String>>,
    // shared
    favorites: Rc<RefCell<Vec<Show>>>,
    server: Rc<RefCell<Vec<Show>>>,
    inbox: Arc<parking_lot::Mutex<Vec<Msg>>>,
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
        self.rebuild_library();
    }

    // ── navigation ──────────────────────────────────────────────────────────

    fn current_page(&self) -> String {
        self.stack
            .visible_child_name()
            .map(|n| n.to_string())
            .unwrap_or_default()
    }

    fn go(&self, name: &str) {
        let cur = self.current_page();
        if cur != name {
            if !cur.is_empty() {
                self.nav.borrow_mut().push(cur);
            }
            self.stack.set_visible_child_name(name);
        }
        if name == "home" {
            self.nav.borrow_mut().clear();
            self.refresh_home();
        }
    }

    fn back(&self) {
        let prev = self.nav.borrow_mut().pop().unwrap_or_else(|| "home".into());
        self.stack.set_visible_child_name(&prev);
        if prev == "home" {
            self.refresh_home();
        }
        self.refresh_all();
    }

    fn set_note(&self, text: &str) {
        let gen = self.banner_gen.get().wrapping_add(1);
        self.banner_gen.set(gen);
        self.banner.set_text(text);
        self.banner.set_visible(!text.is_empty());
        if !text.is_empty() {
            let banner = self.banner.clone();
            let counter = self.banner_gen.clone();
            glib::timeout_add_local_once(Duration::from_secs(8), move || {
                if counter.get() == gen {
                    banner.set_visible(false);
                }
            });
        }
    }

    // ── saved shows ─────────────────────────────────────────────────────────

    fn is_saved(&self, show: &Show) -> bool {
        let feed = podcasts::feed_key(&show.feed_url);
        self.favorites.borrow().iter().any(|saved| {
            saved.id == show.id
                || (!feed.is_empty() && podcasts::feed_key(&saved.feed_url) == feed)
        })
    }

    fn toggle_saved(&self, show: &Show) -> bool {
        let saved = {
            let mut favs = self.favorites.borrow_mut();
            let feed = podcasts::feed_key(&show.feed_url);
            if let Some(i) = favs.iter().position(|saved| {
                saved.id == show.id
                    || (!feed.is_empty() && podcasts::feed_key(&saved.feed_url) == feed)
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
            self.set_note(&format!("Could not save favorites: {err:#}"));
        }
        if saved {
            self.subscribe_show(show);
        }
        self.rebuild_library();
        saved
    }

    fn subscribe_show(&self, show: &Show) {
        if !show.feed_url.contains("://") {
            self.set_note("Saved on this machine.");
            return;
        }
        let Some(backend) = self.state.backend.read().clone() else {
            podcasts::queue_subscription(show);
            self.set_note("Saved. Subscription will be tried again next time you connect.");
            return;
        };
        let show = show.clone();
        let inbox = self.inbox.clone();
        std::thread::spawn(move || match backend.subscribe_podcast(&show.feed_url) {
            Ok(()) => {
                podcasts::forget_subscription(&show.feed_url);
                let list = backend
                    .podcasts()
                    .unwrap_or_default()
                    .iter()
                    .map(Show::from_server)
                    .collect::<Vec<_>>();
                let mut inbox = inbox.lock();
                inbox.push(Msg::ServerShows(list));
                inbox.push(Msg::Note("Saved and subscribed on this server.".into()));
            }
            Err(err) => {
                podcasts::queue_subscription(&show);
                inbox.lock().push(Msg::Note(format!(
                    "Saved locally, but the server could not subscribe ({err:#}). Will retry on next connect."
                )));
            }
        });
    }

    // ── home ────────────────────────────────────────────────────────────────

    /// Favorites plus this server's shows, without duplicates.
    fn collect_library(&self) -> Vec<Show> {
        let mut out: Vec<Show> = self.favorites.borrow().clone();
        for show in self.server.borrow().iter() {
            let key = podcasts::feed_key(&show.feed_url);
            let dup = out.iter().any(|have| {
                have.id == show.id
                    || (!key.is_empty() && podcasts::feed_key(&have.feed_url) == key)
            });
            if !dup {
                out.push(show.clone());
            }
        }
        out
    }

    fn rebuild_library(&self) {
        *self.library_shows.borrow_mut() = self.collect_library();
        self.render_shows_grid();
        self.start_latest();
        // Saved state on visible Save buttons is rebuilt with the pages themselves.
        if self.current_page() == "discover" {
            self.render_discover_grid();
        }
    }

    fn refresh_home(&self) {
        self.render_continue(true);
        self.render_new_row();
        self.render_shows_grid();
    }

    fn render_continue(&self, force: bool) {
        let items = library::continue_listening();
        let sig: String = items
            .iter()
            .map(|(e, pos)| format!("{}@{}", e.track.id, pos / 30))
            .collect::<Vec<_>>()
            .join(",");
        if !force && *self.cont_sig.borrow() == sig {
            return;
        }
        *self.cont_sig.borrow_mut() = sig;
        clear_box(&self.cont_row);
        self.cont_wrap.set_visible(!items.is_empty());
        for (entry, pos) in items.into_iter().take(12) {
            let card = self.continue_card(entry, pos);
            self.cont_row.append(&card);
        }
    }

    fn continue_card(&self, entry: HistoryEntry, pos: u64) -> Button {
        let dur = entry.track.duration_secs.unwrap_or(0);
        let frac = if dur > 0 { pos as f64 / dur as f64 } else { 0.0 };
        let left = if dur > pos {
            format!("{} left", podcasts::format_duration(dur - pos))
        } else {
            format!("at {}", clock(pos))
        };
        let card = media_card(
            entry.track.cover_url.as_deref().or(entry.show.artwork.as_deref()),
            &entry.show.title,
            &entry.track.title,
            &left,
            Some(frac),
            false,
        );
        let page = self.clone();
        card.connect_clicked(move |_| {
            let track = entry.track.clone();
            page.state.player.set_queue(vec![track.clone()], 0);
            podcasts::arm_resume(&track.id);
            play_track(&page.state, track);
            page.speed_applied.borrow_mut().clear();
            page.refresh_all();
        });
        card
    }

    fn render_new_row(&self) {
        clear_box(&self.new_row);
        let played = library::played_set();
        let mut items: Vec<(Show, Episode)> = self
            .latest
            .borrow()
            .iter()
            .filter(|(_, ep)| !played.contains(&format!("pc:{}", ep.id)))
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            podcasts::publish_key(b.1.published.as_deref())
                .cmp(&podcasts::publish_key(a.1.published.as_deref()))
        });
        self.new_wrap.set_visible(!items.is_empty());
        for (show, ep) in items.into_iter().take(12) {
            let sub = ep.subtitle_meta();
            let card = media_card(
                ep.artwork.as_deref().or(show.artwork.as_deref()),
                &show.title,
                &ep.title,
                &sub,
                None,
                is_recent(&ep),
            );
            let page = self.clone();
            card.connect_clicked(move |_| page.open_episode(show.clone(), vec![ep.clone()], 0));
            self.new_row.append(&card);
        }
    }

    fn render_shows_grid(&self) {
        while let Some(child) = self.shows_grid.first_child() {
            self.shows_grid.remove(&child);
        }
        let shows = self.library_shows.borrow().clone();
        self.shows_empty.set_visible(shows.is_empty());
        let latest = self.latest.borrow().clone();
        let played = library::played_set();
        for show in &shows {
            let newest = latest.iter().find(|(s, _)| s.id == show.id).map(|(_, e)| e);
            let sub = match newest.and_then(|e| e.published.as_deref()).and_then(podcasts::short_date) {
                Some(date) => format!("Latest {date}"),
                None if !show.author.trim().is_empty() => show.author.trim().to_string(),
                None => show.source.clone(),
            };
            let fresh = newest.is_some_and(|e| {
                is_recent(e) && !played.contains(&format!("pc:{}", e.id))
            });
            let tile = show_tile(show, &sub, fresh);
            self.shows_grid.append(&tile);
        }
    }

    /// Newest episode per library show, from cache first and then the live feeds.
    fn start_latest(&self) {
        let shows: Vec<Show> = self.library_shows.borrow().iter().take(16).cloned().collect();
        if shows.is_empty() {
            return;
        }
        let inbox = self.inbox.clone();
        std::thread::spawn(move || {
            let mut found: Vec<(Show, Episode)> = Vec::new();
            for show in shows {
                let cached = library::latest_cached(&show.id);
                let episode = match cached {
                    Some(entry) if library::is_fresh(&entry) => entry.episode,
                    _ => {
                        if !show.feed_url.contains("://") {
                            continue;
                        }
                        let ep = podcasts::episodes_for_feed(&show)
                            .ok()
                            .and_then(|eps| eps.into_iter().next());
                        library::store_latest(&show, ep.clone());
                        ep
                    }
                };
                if let Some(ep) = episode {
                    found.push((show, ep));
                    inbox.lock().push(Msg::Latest(found.clone()));
                }
            }
        });
    }

    // ── discover ────────────────────────────────────────────────────────────

    fn set_active_chip(&self, key: &str) {
        for (chip, button) in self.chips.borrow().iter() {
            if chip == key {
                button.add_css_class("nm-chip-on");
            } else {
                button.remove_css_class("nm-chip-on");
            }
        }
    }

    fn start_directory(&self) {
        let gen = self.dir_gen.get().wrapping_add(1);
        self.dir_gen.set(gen);
        let mode = self.mode.borrow().clone();
        self.d_status.set_text("Looking up podcasts…");
        let inbox = self.inbox.clone();
        std::thread::spawn(move || {
            let result = match &mode {
                Browse::Genre(name) => podcasts::by_genre(name),
                Browse::Search(query) => podcasts::search(query),
                Browse::Trending => podcasts::trending(),
            }
            .map_err(|err| format!("{err:#}"));
            inbox.lock().push(Msg::Shows { gen, result });
        });
    }

    fn render_discover_grid(&self) {
        while let Some(child) = self.d_grid.first_child() {
            self.d_grid.remove(&child);
        }
        for show in self.d_shows.borrow().iter() {
            let tile = show_tile(show, &show.subtitle_short(), false);
            let save = save_button(self, show);
            save.add_css_class("nm-tile-save");
            save.set_halign(Align::End);
            save.set_valign(Align::Start);
            save.set_margin_top(8);
            save.set_margin_end(8);
            tile.add_overlay(&save);
            self.d_grid.append(&tile);
        }
    }

    // ── show page ───────────────────────────────────────────────────────────

    pub(crate) fn open_show(&self, show: Show) {
        self.go("show");
        *self.current_show.borrow_mut() = Some(show.clone());
        self.filter.set(Filter::All);
        for (f, button) in self.s_filters.borrow().iter() {
            if *f == Filter::All {
                button.add_css_class("nm-chip-on");
            } else {
                button.remove_css_class("nm-chip-on");
            }
        }
        self.shown_n.set(podcasts::PAGE);
        *self.episodes.borrow_mut() = None;
        self.clear_cards();
        self.s_more.set_visible(false);
        self.s_status.set_text("Loading episodes…");
        let cached = self.extras.borrow().get(&show.id).cloned();
        self.fill_masthead(&show, cached.as_ref());
        clear_box(&self.s_about);
        match cached {
            Some(extras) => self.fill_about(&show, &extras),
            None => {
                let wait = Label::new(Some("Loading background, reviews and related shows…"));
                wait.add_css_class("nm-muted");
                wait.set_halign(Align::Start);
                self.s_about.append(&wait);
                let inbox = self.inbox.clone();
                let for_web = show.clone();
                std::thread::spawn(move || {
                    let extras = web::show_extras(&for_web);
                    inbox
                        .lock()
                        .push(Msg::Extras(for_web.id.clone(), Box::new(extras)));
                });
            }
        }
        self.load_episodes_for(show, None);
    }

    fn load_episodes_for(&self, show: Show, refresh_note: Option<String>) {
        let gen = self.ep_gen.get().wrapping_add(1);
        self.ep_gen.set(gen);
        let inbox = self.inbox.clone();
        let backend = self.state.backend.read().clone();
        std::thread::spawn(move || {
            let result = load_episodes(backend.as_ref(), &show)
                .map(|eps| (show, eps))
                .map_err(|err| format!("{err:#}"));
            inbox.lock().push(Msg::Episodes {
                gen,
                result,
                note: refresh_note,
            });
        });
    }

    fn refresh_show(&self, show: &Show) {
        self.s_status.set_text("Refreshing episodes…");
        let gen = self.ep_gen.get().wrapping_add(1);
        self.ep_gen.set(gen);
        let inbox = self.inbox.clone();
        let backend = self.state.backend.read().clone();
        let show = show.clone();
        std::thread::spawn(move || {
            let note = if let Some(be) = backend.as_ref() {
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
            if note.is_none() {
                std::thread::sleep(Duration::from_millis(800));
            }
            let result = load_episodes(backend.as_ref(), &show)
                .map(|eps| (show, eps))
                .map_err(|err| format!("{err:#}"));
            inbox.lock().push(Msg::Episodes { gen, result, note });
        });
    }

    /// The magazine masthead: small art, a large title, a rule, the byline and deck.
    fn fill_masthead(&self, show: &Show, extras: Option<&ShowExtras>) {
        clear_box(&self.s_masthead);
        let art = gtk4::Image::new();
        art.add_css_class("nm-mast-art");
        art.set_pixel_size(112);
        art.set_size_request(112, 112);
        art.set_valign(Align::Start);
        art.set_icon_name(Some("multimedia-player-symbolic"));
        bind_cover(&art, show.artwork.as_deref());

        let col = GtkBox::new(Orientation::Vertical, 6);
        col.set_hexpand(true);
        let mut kicker = vec!["PODCAST".to_string()];
        if let Some(extras) = extras {
            if !extras.genre.is_empty() {
                kicker.push(extras.genre.to_uppercase());
            }
        }
        col.append(&styled_label(&kicker.join("  ·  "), "nm-ep-meta"));
        let title = styled_label(&show.title, "nm-mast-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        col.append(&title);
        let rule = gtk4::Separator::new(Orientation::Horizontal);
        rule.add_css_class("nm-mast-rule");
        col.append(&rule);
        let by = [show.author.trim(), show.source.trim()]
            .iter()
            .filter(|p| !p.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("  ·  ");
        if !by.is_empty() {
            col.append(&styled_label(&by, "nm-mast-by"));
        }
        let deck_text = extras
            .and_then(|e| e.long_description.clone())
            .unwrap_or_else(|| podcasts::strip_html(&show.description));
        if !deck_text.is_empty() {
            let deck = styled_label(&clip_text(&deck_text, 480), "nm-mast-deck");
            deck.set_wrap(true);
            deck.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            col.append(&deck);
        }

        let actions = GtkBox::new(Orientation::Vertical, 6);
        actions.set_valign(Align::Start);
        actions.append(&save_button(self, show));
        let refresh = Button::with_label("Refresh");
        refresh.add_css_class("nm-chip");
        refresh.set_tooltip_text(Some("Reload episodes from the feed and server"));
        let page = self.clone();
        let for_refresh = show.clone();
        refresh.connect_clicked(move |_| page.refresh_show(&for_refresh));
        actions.append(&refresh);

        self.s_masthead.append(&art);
        self.s_masthead.append(&col);
        self.s_masthead.append(&actions);
    }

    /// Web background below the episodes: rating, summary, reviews, related shows.
    fn fill_about(&self, show: &Show, extras: &ShowExtras) {
        clear_box(&self.s_about);
        let about = &self.s_about;
        if let Some((stars, count)) = extras.rating {
            about.append(&styled_label("LISTENER RATING", "nm-ep-section"));
            about.append(&styled_label(
                &format!("{}  {stars:.1} from {count} recent ratings", star_text(stars.round() as u8)),
                "nm-ep-desc",
            ));
        }
        if !extras.categories.is_empty() {
            about.append(&styled_label(
                &extras.categories.join("  ·  ").to_uppercase(),
                "nm-ep-meta",
            ));
        }
        if let Some((text, url)) = &extras.summary {
            about.append(&styled_label("ABOUT THE SHOW", "nm-ep-section"));
            let body = styled_label(&clip_text(text, 900), "nm-ep-desc");
            body.set_wrap(true);
            body.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            about.append(&body);
            if !url.is_empty() {
                let src = Label::new(None);
                src.add_css_class("nm-credit");
                src.set_halign(Align::Start);
                src.set_markup(&format!(
                    "<a href=\"{}\">Wikipedia</a>",
                    glib::markup_escape_text(url)
                ));
                about.append(&src);
            }
        }
        if !extras.reviews.is_empty() {
            about.append(&styled_label("WHAT LISTENERS SAY", "nm-ep-section"));
            for review in &extras.reviews {
                let card = GtkBox::new(Orientation::Vertical, 3);
                card.add_css_class("nm-guest-card");
                card.append(&styled_label(
                    &format!("{}  {}", star_text(review.stars), review.title),
                    "nm-guest-name",
                ));
                let body = styled_label(&clip_text(&review.body, 320), "nm-guest-bio");
                body.set_wrap(true);
                body.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                card.append(&body);
                card.append(&styled_label(&format!("— {}", review.author), "nm-credit"));
                about.append(&card);
            }
            about.append(&styled_label("Reviews from Apple Podcasts listeners.", "nm-credit"));
        }
        if !extras.related.is_empty() {
            about.append(&styled_label("YOU MIGHT ALSO LIKE", "nm-ep-section"));
            let row = GtkBox::new(Orientation::Horizontal, 12);
            for other in extras.related.iter().take(10) {
                let tile = show_tile_sized(other, &other.subtitle_short(), false, 140);
                let button = Button::new();
                button.add_css_class("nm-card");
                button.set_has_frame(false);
                button.set_child(Some(&tile));
                button.update_property(&[gtk4::accessible::Property::Label(&other.title)]);
                let page = self.clone();
                let other = other.clone();
                button.connect_clicked(move |_| page.open_show(other.clone()));
                row.append(&button);
            }
            let scroll = ScrolledWindow::builder().child(&row).build();
            scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
            scroll.set_min_content_height(180);
            about.append(&scroll);
        }
        let _ = show;
    }

    fn clear_cards(&self) {
        self.cards.borrow_mut().clear();
        while let Some(row) = self.s_list.row_at_index(0) {
            self.s_list.remove(&row);
        }
    }

    fn passes(&self, ep: &Episode, played: &std::collections::BTreeSet<String>, progress: &std::collections::BTreeMap<String, u64>) -> bool {
        let id = format!("pc:{}", ep.id);
        match self.filter.get() {
            Filter::All => true,
            Filter::Unplayed => !played.contains(&id),
            Filter::Downloaded => library::downloaded_path(ep).is_some(),
            Filter::InProgress => {
                !played.contains(&id) && progress.get(&id).is_some_and(|p| *p >= 5)
            }
        }
    }

    /// Rebuild the episode cards for the active filter, newest first.
    fn render_episodes(&self) {
        self.clear_cards();
        let (show, episodes) = {
            let view = self.episodes.borrow();
            let Some(list) = view.as_ref() else {
                return;
            };
            (list.show.clone(), list.episodes.clone())
        };
        let played = library::played_set();
        let progress = podcasts::load_progress_map();
        let matching: Vec<usize> = episodes
            .iter()
            .enumerate()
            .filter(|(_, ep)| self.passes(ep, &played, &progress))
            .map(|(i, _)| i)
            .collect();
        let shown = self.shown_n.get().min(matching.len());
        for &i in matching.iter().take(shown) {
            let (row, slot) = self.episode_card(&show, &episodes, i, &played);
            self.s_list.append(&row);
            self.cards.borrow_mut().push(slot);
        }
        self.s_more.set_visible(shown < matching.len());
        let total = episodes.len();
        self.s_status.set_text(&if matching.len() == total {
            format!("{shown} of {total} episode{} · newest first", plural(total))
        } else {
            format!(
                "{shown} of {} matching · {total} episode{} in all",
                matching.len(),
                plural(total)
            )
        });
        self.refresh_cards();
    }

    /// One episode card: artwork with a play button, status, summary and actions.
    fn episode_card(
        &self,
        show: &Show,
        episodes: &[Episode],
        index: usize,
        played: &std::collections::BTreeSet<String>,
    ) -> (ListBoxRow, CardSlot) {
        let episode = &episodes[index];
        let id = format!("pc:{}", episode.id);
        let row = ListBoxRow::new();
        row.add_css_class("nm-lib-row");
        row.add_css_class("nm-ep-row");
        row.set_activatable(false);

        let card = GtkBox::new(Orientation::Horizontal, 16);
        card.add_css_class("nm-ep-card");

        let overlay = Overlay::new();
        overlay.set_valign(Align::Start);
        let art = gtk4::Image::new();
        art.add_css_class("nm-card-art");
        art.set_pixel_size(150);
        art.set_size_request(150, 150);
        art.set_icon_name(Some("multimedia-player-symbolic"));
        let art_url = episode.artwork.clone().or_else(|| show.artwork.clone());
        bind_cover(&art, art_url.as_deref());
        overlay.set_child(Some(&art));
        let play = Button::with_label("Play");
        play.add_css_class("nm-fab");
        play.set_halign(Align::Start);
        play.set_valign(Align::End);
        play.set_margin_start(8);
        play.set_margin_bottom(8);
        overlay.add_overlay(&play);

        let body = GtkBox::new(Orientation::Vertical, 5);
        body.set_hexpand(true);

        let meta_row = GtkBox::new(Orientation::Horizontal, 8);
        let new_tag = styled_label("NEW", "nm-new-tag");
        new_tag.set_visible(false);
        meta_row.append(&new_tag);
        let mut meta = Vec::new();
        if index == 0 {
            meta.push("LATEST".to_string());
        }
        let sub = episode.subtitle_meta();
        if !sub.is_empty() {
            meta.push(sub);
        }
        if !episode.guests.is_empty() {
            let names: Vec<&str> = episode.guests.iter().map(|g| g.name.as_str()).collect();
            meta.push(format!("with {}", names.join(", ")));
        }
        let meta_label = fitted_label(&meta.join(" · "), "nm-ep-meta", 1);
        meta_row.append(&meta_label);
        body.append(&meta_row);

        let title = styled_label(&episode.title, "nm-ep-card-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        title.set_lines(2);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        body.append(&title);

        let notes = episode.notes();
        if !notes.is_empty() {
            let desc = styled_label(&clip_text(&notes, 220), "nm-ep-desc");
            desc.set_wrap(true);
            desc.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            desc.set_lines(2);
            desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            body.append(&desc);
        }

        let progress = ProgressBar::new();
        progress.add_css_class("nm-ep-progress");
        progress.set_visible(false);
        body.append(&progress);

        let actions = GtkBox::new(Orientation::Horizontal, 8);
        actions.set_margin_top(4);
        let played_btn = Button::with_label("Mark played");
        played_btn.add_css_class("nm-chip");
        let download = Button::with_label("Download");
        download.add_css_class("nm-chip");
        let details = Button::with_label("Details");
        details.add_css_class("nm-chip");
        actions.append(&played_btn);
        actions.append(&download);
        actions.append(&details);
        body.append(&actions);

        card.append(&overlay);
        card.append(&body);
        row.set_child(Some(&card));

        {
            let page = self.clone();
            let show = show.clone();
            let list = episodes.to_vec();
            play.connect_clicked(move |_| page.toggle_episode(&show, &list, index));
        }
        {
            let page = self.clone();
            let id = id.clone();
            played_btn.connect_clicked(move |_| {
                library::set_played(&id, !library::is_played(&id));
                page.refresh_all();
                page.render_new_row();
            });
        }
        {
            let page = self.clone();
            let ep = episode.clone();
            download.connect_clicked(move |_| {
                page.toggle_download(&ep);
            });
        }
        {
            let page = self.clone();
            let show = show.clone();
            let list = episodes.to_vec();
            details.connect_clicked(move |_| page.open_episode(show.clone(), list.clone(), index));
        }
        let _ = played;
        (
            row,
            CardSlot {
                id,
                index,
                episode: episode.clone(),
                play,
                played: played_btn,
                download,
                title,
                new_tag,
                progress,
            },
        )
    }

    fn toggle_download(&self, ep: &Episode) {
        if library::is_downloading(ep) {
            return;
        }
        if library::downloaded_path(ep).is_some() {
            library::remove_download(ep);
            self.set_note("Download removed.");
        } else {
            library::start_download(ep);
            self.set_note(&format!("Downloading “{}” for offline listening…", ep.title));
        }
        self.refresh_all();
    }

    // ── playback ────────────────────────────────────────────────────────────

    fn track_for(show: &Show, ep: &Episode) -> Track {
        let mut track = ep.to_track(show);
        track.stream_url = library::playable_url(ep);
        track
    }

    fn play_from(&self, show: &Show, episodes: &[Episode], index: usize) {
        let Some(episode) = episodes.get(index) else {
            return;
        };
        let tracks: Vec<Track> = episodes.iter().map(|e| Self::track_for(show, e)).collect();
        self.state.player.set_queue(tracks, index);
        let track = Self::track_for(show, episode);
        library::remember(&episode.to_track(show), show);
        play_track(&self.state, track);
        self.speed_applied.borrow_mut().clear();
        self.refresh_all();
    }

    fn toggle_episode(&self, show: &Show, episodes: &[Episode], index: usize) {
        let Some(episode) = episodes.get(index) else {
            return;
        };
        let id = format!("pc:{}", episode.id);
        if self
            .state
            .player
            .current()
            .is_some_and(|track| track.id == id)
        {
            let _ = self.state.player.toggle();
            self.refresh_all();
        } else {
            self.play_from(show, episodes, index);
        }
    }

    fn skip(&self, delta: i64) {
        let current = self.state.player.current();
        if !current.as_ref().is_some_and(podcasts::is_podcast_track) {
            return;
        }
        let pos = self.state.player.position_secs() as i64;
        let dur = self.state.player.duration_secs() as i64;
        let mut target = (pos + delta).max(0);
        if dur > 0 {
            target = target.min(dur - 1).max(0);
        }
        let _ = self.state.player.seek_secs(target as u64);
    }

    fn cycle_speed(&self) {
        let now = self.speed.get();
        let at = SPEEDS.iter().position(|s| (*s - now).abs() < 0.01).unwrap_or(0);
        let next = SPEEDS[(at + 1) % SPEEDS.len()];
        self.speed.set(next);
        if self
            .state
            .player
            .current()
            .is_some_and(|t| podcasts::is_podcast_track(&t))
        {
            let _ = self.state.player.set_rate(next);
            if let Some(track) = self.state.player.current() {
                *self.speed_applied.borrow_mut() = track.id;
            }
        }
        self.refresh_episode_ui();
    }

    fn cycle_sleep(&self) {
        let next = match self.sleep.get() {
            Sleep::Off => Sleep::At(Instant::now() + Duration::from_secs(15 * 60), 15),
            Sleep::At(_, 15) => Sleep::At(Instant::now() + Duration::from_secs(30 * 60), 30),
            Sleep::At(_, 30) => Sleep::At(Instant::now() + Duration::from_secs(60 * 60), 60),
            Sleep::At(_, _) => Sleep::EndOfEpisode,
            Sleep::EndOfEpisode => Sleep::Off,
        };
        self.sleep.set(next);
        self.refresh_episode_ui();
    }

    fn sleep_label(&self) -> String {
        match self.sleep.get() {
            Sleep::Off => "Sleep: off".into(),
            Sleep::At(until, _) => {
                let left = until.saturating_duration_since(Instant::now()).as_secs();
                format!("Sleep: {} min", left.div_ceil(60))
            }
            Sleep::EndOfEpisode => "Sleep: end of episode".into(),
        }
    }

    /// Per-tick playback chores: speed, sleep timer, auto-played.
    fn playback_tick(&self) {
        let Some(track) = self.state.player.current() else {
            return;
        };
        if !podcasts::is_podcast_track(&track) {
            return;
        }
        let playing = self.state.player.is_playing();
        let dur = self.state.player.duration_secs();
        let pos = self.state.player.position_secs();
        if playing && dur > 0 && *self.speed_applied.borrow() != track.id {
            *self.speed_applied.borrow_mut() = track.id.clone();
            if (self.speed.get() - 1.0).abs() > 0.01 {
                let _ = self.state.player.set_rate(self.speed.get());
            }
        }
        match self.sleep.get() {
            Sleep::At(until, _) if Instant::now() >= until => {
                let _ = self.state.player.pause();
                self.sleep.set(Sleep::Off);
                self.set_note("Sleep timer: paused.");
                self.refresh_all();
            }
            Sleep::EndOfEpisode if playing && dur > 0 && pos + 2 >= dur => {
                let _ = self.state.player.pause();
                self.sleep.set(Sleep::Off);
                self.set_note("Sleep timer: episode finished.");
                self.refresh_all();
            }
            _ => {}
        }
        if dur > 60 && pos + 25 >= dur && *self.auto_played.borrow() != track.id {
            *self.auto_played.borrow_mut() = track.id.clone();
            library::set_played(&track.id, true);
        }
    }

    // ── refreshing live state ───────────────────────────────────────────────

    fn refresh_all(&self) {
        self.refresh_cards();
        self.refresh_episode_ui();
    }

    fn refresh_cards(&self) {
        let played = library::played_set();
        let progress = podcasts::load_progress_map();
        let current = self.state.player.current();
        let playing = self.state.player.is_playing();
        for card in self.cards.borrow().iter() {
            let is_cur = current.as_ref().is_some_and(|t| t.id == card.id);
            card.play.set_label(if is_cur && playing {
                "Pause"
            } else if is_cur {
                "Resume"
            } else {
                "Play"
            });
            let done = played.contains(&card.id);
            card.played
                .set_label(if done { "Mark unplayed" } else { "Mark played" });
            if done {
                card.title.remove_css_class("nm-ep-unplayed");
                card.title.add_css_class("nm-ep-done");
            } else {
                card.title.add_css_class("nm-ep-unplayed");
                card.title.remove_css_class("nm-ep-done");
            }
            card.new_tag.set_visible(!done && is_recent(&card.episode));
            if let Some(pos) = progress.get(&card.id).filter(|p| **p >= 5) {
                if let Some(dur) = card.episode.duration_secs.filter(|d| *d > 0) {
                    card.progress.set_fraction((*pos as f64 / dur as f64).clamp(0.0, 1.0));
                    card.progress.set_visible(!done);
                }
            } else {
                card.progress.set_visible(false);
            }
            card.download.set_label(&download_label(&card.episode));
            if is_cur {
                card.play.add_css_class("nm-fab-on");
            } else {
                card.play.remove_css_class("nm-fab-on");
            }
            let _ = card.index;
        }
    }

    fn refresh_episode_ui(&self) {
        let ctx = self.ctx.borrow().clone();
        let mut guard = self.ep_ui.borrow_mut();
        let (Some(ui), Some(ctx)) = (guard.as_mut(), ctx) else {
            return;
        };
        let episode = &ctx.list[ctx.index];
        let current = self.state.player.current();
        let is_cur = current.as_ref().is_some_and(|t| t.id == ui.id);
        let playing = self.state.player.is_playing();
        ui.play.set_label(if is_cur && playing {
            "Pause"
        } else if is_cur {
            "Resume"
        } else {
            "Play episode"
        });
        ui.played.set_label(if library::is_played(&ui.id) {
            "Mark unplayed"
        } else {
            "Mark played"
        });
        ui.download.set_label(&download_label(episode));
        ui.speed.set_label(&format!("Speed {:.2}×", self.speed.get()).replace(".00×", ".0×"));
        ui.sleep.set_label(&self.sleep_label());
        if is_cur {
            let pos = self.state.player.position_secs();
            let mark = |rows: Vec<(u64, &ListBoxRow)>, last: &mut Option<usize>| {
                let at = rows.iter().rposition(|(start, _)| *start <= pos);
                if *last != at {
                    if let Some(old) = *last {
                        rows[old].1.remove_css_class("nm-station-on");
                    }
                    if let Some(new) = at {
                        rows[new].1.add_css_class("nm-station-on");
                    }
                    *last = at;
                }
            };
            let chapters: Vec<(u64, &ListBoxRow)> =
                ui.chapters.iter().map(|(s, r)| (*s, r)).collect();
            let mut last = ui.highlighted.0;
            mark(chapters, &mut last);
            ui.highlighted.0 = last;
            let lines: Vec<(u64, &ListBoxRow)> = ui
                .transcript
                .iter()
                .filter_map(|(s, r)| s.map(|s| (s, r)))
                .collect();
            let mut last = ui.highlighted.1;
            mark(lines, &mut last);
            ui.highlighted.1 = last;
        }
    }

    // ── episode page ────────────────────────────────────────────────────────

    pub(crate) fn open_episode(&self, show: Show, list: Vec<Episode>, index: usize) {
        let Some(episode) = list.get(index).cloned() else {
            return;
        };
        self.go("episode");
        *self.ctx.borrow_mut() = Some(EpCtx {
            show: show.clone(),
            list: list.clone(),
            index,
        });
        self.slots.borrow_mut().clear();
        clear_box(&self.e_body);
        let id = format!("pc:{}", episode.id);

        let top = GtkBox::new(Orientation::Horizontal, 24);
        top.add_css_class("nm-ep-page");
        let art = gtk4::Image::new();
        art.add_css_class("nm-hero-art");
        art.set_pixel_size(300);
        art.set_size_request(300, 300);
        art.set_valign(Align::Start);
        art.set_icon_name(Some("multimedia-player-symbolic"));
        let art_url = episode.artwork.clone().or_else(|| show.artwork.clone());
        bind_cover(&art, art_url.as_deref());

        let col = GtkBox::new(Orientation::Vertical, 10);
        col.set_hexpand(true);
        let kicker_row = GtkBox::new(Orientation::Horizontal, 8);
        if is_recent(&episode) && !library::is_played(&id) {
            kicker_row.append(&styled_label("NEW", "nm-new-tag"));
        }
        let mut kicker = vec![show.title.to_uppercase()];
        let sub = episode.subtitle_meta();
        if !sub.is_empty() {
            kicker.push(sub);
        }
        let kicker_label = styled_label(&kicker.join("  ·  "), "nm-ep-meta");
        kicker_label.set_wrap(true);
        kicker_row.append(&kicker_label);
        col.append(&kicker_row);
        let title = styled_label(&episode.title, "nm-ep-page-title");
        title.set_wrap(true);
        title.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        col.append(&title);
        let rule = gtk4::Separator::new(Orientation::Horizontal);
        rule.add_css_class("nm-mast-rule");
        col.append(&rule);

        let controls = GtkBox::new(Orientation::Horizontal, 8);
        let play = Button::with_label("Play episode");
        play.add_css_class("nm-chip");
        play.add_css_class("nm-chip-on");
        play.add_css_class("nm-play-big");
        let back15 = Button::with_label("−15 s");
        back15.add_css_class("nm-chip");
        let fwd30 = Button::with_label("+30 s");
        fwd30.add_css_class("nm-chip");
        let speed = Button::with_label("Speed");
        speed.add_css_class("nm-chip");
        speed.set_tooltip_text(Some("Playback speed"));
        let sleep = Button::with_label("Sleep: off");
        sleep.add_css_class("nm-chip");
        sleep.set_tooltip_text(Some("Pause after 15, 30 or 60 minutes, or at the end of the episode"));
        for w in [&play, &back15, &fwd30, &speed, &sleep] {
            controls.append(w);
        }
        let controls_scroll = ScrolledWindow::builder().child(&controls).build();
        controls_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
        controls_scroll.set_min_content_height(44);
        col.append(&controls_scroll);

        let more = GtkBox::new(Orientation::Horizontal, 8);
        let played = Button::with_label("Mark played");
        played.add_css_class("nm-chip");
        let download = Button::with_label("Download");
        download.add_css_class("nm-chip");
        more.append(&played);
        more.append(&download);
        col.append(&more);
        top.append(&art);
        top.append(&col);
        self.e_body.append(&top);

        // Tabs: Notes · Chapters · Transcript · Guests
        let tabs = GtkBox::new(Orientation::Horizontal, 6);
        tabs.set_margin_top(10);
        let pages = Stack::new();
        pages.set_transition_type(StackTransitionType::Crossfade);
        pages.set_transition_duration(160);
        let mut tab_buttons: Vec<(&'static str, Button)> = Vec::new();

        // Notes
        let notes_box = GtkBox::new(Orientation::Vertical, 8);
        let notes_text = web::block_text(&episode.description);
        let paragraphs: Vec<&str> = notes_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if paragraphs.is_empty() {
            notes_box.append(&styled_label("This episode has no show notes.", "nm-muted"));
        }
        for para in paragraphs.iter().take(80) {
            let label = styled_label(para, "nm-ep-desc");
            label.set_wrap(true);
            label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            label.set_selectable(true);
            notes_box.append(&label);
        }
        pages.add_named(&notes_box, Some("notes"));
        tab_buttons.push(("notes", tab_chip("Notes")));

        // Chapters
        let chapters_box = GtkBox::new(Orientation::Vertical, 6);
        let chapters_note = styled_label("Looking for chapters…", "nm-muted");
        chapters_note.set_halign(Align::Start);
        let chapters_list = ListBox::new();
        chapters_list.set_selection_mode(SelectionMode::None);
        chapters_list.set_activate_on_single_click(true);
        chapters_list.add_css_class("nm-library-list");
        chapters_box.append(&chapters_note);
        chapters_box.append(&chapters_list);
        pages.add_named(&chapters_box, Some("chapters"));
        let chapters_chip = tab_chip("Chapters");
        tab_buttons.push(("chapters", chapters_chip.clone()));

        // Transcript
        let transcript_box = GtkBox::new(Orientation::Vertical, 6);
        let transcript_note = styled_label(
            if episode.transcript_url.is_some() {
                "Loading transcript…"
            } else {
                "This episode's feed does not include a transcript."
            },
            "nm-muted",
        );
        transcript_note.set_halign(Align::Start);
        transcript_note.set_wrap(true);
        let transcript_list = ListBox::new();
        transcript_list.set_selection_mode(SelectionMode::None);
        transcript_list.set_activate_on_single_click(true);
        transcript_list.add_css_class("nm-library-list");
        transcript_box.append(&transcript_note);
        transcript_box.append(&transcript_list);
        pages.add_named(&transcript_box, Some("transcript"));
        tab_buttons.push(("transcript", tab_chip("Transcript")));

        // Guests
        if !episode.guests.is_empty() {
            let guests_box = GtkBox::new(Orientation::Vertical, 10);
            for guest in &episode.guests {
                guests_box.append(&self.guest_card(guest));
            }
            pages.add_named(&guests_box, Some("guests"));
            tab_buttons.push((
                "guests",
                tab_chip(if episode.guests.len() == 1 { "Guest" } else { "Guests" }),
            ));
        }

        for (name, button) in &tab_buttons {
            tabs.append(button);
            let pages = pages.clone();
            let name = *name;
            let all: Vec<Button> = tab_buttons.iter().map(|(_, b)| b.clone()).collect();
            let page = self.clone();
            button.connect_clicked(move |clicked| {
                pages.set_visible_child_name(name);
                for b in &all {
                    if b == clicked {
                        b.add_css_class("nm-chip-on");
                    } else {
                        b.remove_css_class("nm-chip-on");
                    }
                }
                if name == "transcript" {
                    page.request_transcript();
                }
            });
        }
        if let Some((_, first)) = tab_buttons.first() {
            first.add_css_class("nm-chip-on");
        }
        self.e_body.append(&tabs);
        let pages_frame = GtkBox::new(Orientation::Vertical, 0);
        pages_frame.add_css_class("nm-ep-tabs");
        pages_frame.append(&pages);
        self.e_body.append(&pages_frame);

        *self.ep_ui.borrow_mut() = Some(EpUi {
            id: id.clone(),
            play: play.clone(),
            played: played.clone(),
            download: download.clone(),
            speed: speed.clone(),
            sleep: sleep.clone(),
            chapters_chip,
            chapters_list: chapters_list.clone(),
            chapters_note,
            chapters: Vec::new(),
            transcript_list: transcript_list.clone(),
            transcript_note,
            transcript: Vec::new(),
            transcript_requested: false,
            highlighted: (None, None),
        });

        {
            let page = self.clone();
            play.connect_clicked(move |_| {
                let ctx = page.ctx.borrow().clone();
                if let Some(c) = ctx {
                    page.toggle_episode(&c.show, &c.list, c.index);
                }
            });
        }
        {
            let page = self.clone();
            back15.connect_clicked(move |_| page.skip(-15));
        }
        {
            let page = self.clone();
            fwd30.connect_clicked(move |_| page.skip(30));
        }
        {
            let page = self.clone();
            speed.connect_clicked(move |_| page.cycle_speed());
        }
        {
            let page = self.clone();
            sleep.connect_clicked(move |_| page.cycle_sleep());
        }
        {
            let page = self.clone();
            let id = id.clone();
            played.connect_clicked(move |_| {
                library::set_played(&id, !library::is_played(&id));
                page.refresh_all();
                page.render_new_row();
            });
        }
        {
            let page = self.clone();
            let ep = episode.clone();
            download.connect_clicked(move |_| page.toggle_download(&ep));
        }
        {
            let page = self.clone();
            chapters_list.connect_row_activated(move |_, row| {
                let start = page
                    .ep_ui
                    .borrow()
                    .as_ref()
                    .and_then(|ui| ui.chapters.get(row.index() as usize).map(|(s, _)| *s));
                if let Some(start) = start {
                    page.seek_episode(start);
                }
            });
        }
        {
            let page = self.clone();
            transcript_list.connect_row_activated(move |_, row| {
                let start = page
                    .ep_ui
                    .borrow()
                    .as_ref()
                    .and_then(|ui| ui.transcript.get(row.index() as usize).and_then(|(s, _)| *s));
                if let Some(start) = start {
                    page.seek_episode(start);
                }
            });
        }

        // Chapters are cheap to look for; the transcript waits until its tab opens.
        let inbox = self.inbox.clone();
        let for_chapters = episode.clone();
        std::thread::spawn(move || {
            let list = web::chapters(&for_chapters);
            inbox
                .lock()
                .push(Msg::Chapters(format!("pc:{}", for_chapters.id), list));
        });
        self.refresh_episode_ui();
    }

    fn request_transcript(&self) {
        let ctx = self.ctx.borrow().clone();
        let Some(ctx) = ctx else {
            return;
        };
        {
            let mut guard = self.ep_ui.borrow_mut();
            let Some(ui) = guard.as_mut() else {
                return;
            };
            if ui.transcript_requested {
                return;
            }
            ui.transcript_requested = true;
        }
        let episode = ctx.list[ctx.index].clone();
        if episode.transcript_url.is_none() {
            return;
        }
        let inbox = self.inbox.clone();
        std::thread::spawn(move || {
            let result = web::transcript(&episode).map_err(|err| format!("{err:#}"));
            inbox
                .lock()
                .push(Msg::Transcript(format!("pc:{}", episode.id), result));
        });
    }

    fn seek_episode(&self, secs: u64) {
        let ctx = self.ctx.borrow().clone();
        let Some(ctx) = ctx else {
            return;
        };
        let id = format!("pc:{}", ctx.list[ctx.index].id);
        if self.state.player.current().is_some_and(|t| t.id == id) {
            let _ = self.state.player.seek_secs(secs);
            if !self.state.player.is_playing() {
                let _ = self.state.player.play();
            }
            self.refresh_all();
        } else {
            self.play_from(&ctx.show, &ctx.list, ctx.index);
            podcasts::resume_at(&id, secs);
        }
    }

    fn fill_chapters(&self, id: &str, chapters: Vec<Chapter>) {
        let mut guard = self.ep_ui.borrow_mut();
        let Some(ui) = guard.as_mut() else {
            return;
        };
        if ui.id != id {
            return;
        }
        if chapters.is_empty() {
            ui.chapters_note
                .set_text("No chapters in this episode's feed or show notes.");
            return;
        }
        ui.chapters_note.set_visible(false);
        ui.chapters_chip
            .set_label(&format!("Chapters ({})", chapters.len()));
        for chapter in chapters {
            let row = timed_row(Some(chapter.start), "", &chapter.title);
            ui.chapters_list.append(&row);
            ui.chapters.push((chapter.start, row));
        }
    }

    fn fill_transcript(&self, id: &str, result: Result<Vec<TranscriptLine>, String>) {
        let mut guard = self.ep_ui.borrow_mut();
        let Some(ui) = guard.as_mut() else {
            return;
        };
        if ui.id != id {
            return;
        }
        match result {
            Err(err) => ui
                .transcript_note
                .set_text(&format!("Transcript unavailable: {err}")),
            Ok(lines) => {
                ui.transcript_note.set_visible(false);
                for line in lines {
                    let row = timed_row(line.start, &line.speaker, &line.text);
                    ui.transcript_list.append(&row);
                    ui.transcript.push((line.start, row));
                }
            }
        }
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
        let name = styled_label(&guest.name, "nm-guest-name");
        name.set_wrap(true);
        let role = styled_label("Guest", "nm-ep-meta");
        let bio = styled_label("Looking up biography…", "nm-guest-bio");
        bio.set_wrap(true);
        bio.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let source = styled_label("", "nm-credit");
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
                    self.bios.borrow_mut().remove(&key);
                    for slot in self.slots.borrow().iter().filter(|s| bio_key(&s.name) == key) {
                        slot.bio.set_text("Biography unavailable right now. Check the connection.");
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

    // ── incoming work ───────────────────────────────────────────────────────

    fn drain_inbox(&self) {
        let msgs: Vec<Msg> = std::mem::take(&mut *self.inbox.lock());
        for msg in msgs {
            match msg {
                Msg::Shows { gen, result } => {
                    if gen != self.dir_gen.get() {
                        continue;
                    }
                    match result {
                        Ok(shows) => {
                            let status = describe(&self.mode.borrow(), shows.len());
                            *self.d_shows.borrow_mut() = shows;
                            self.d_status.set_text(&status);
                            self.render_discover_grid();
                        }
                        Err(err) => self
                            .d_status
                            .set_text(&format!("Podcast directory unavailable: {err}")),
                    }
                }
                Msg::Episodes { gen, result, note } => {
                    if gen != self.ep_gen.get() {
                        continue;
                    }
                    let extra = note.map(|n| format!(" {n}")).unwrap_or_default();
                    match result {
                        Ok((show, episodes)) if episodes.is_empty() => {
                            self.s_status
                                .set_text(&format!("This show has no episodes.{extra}"));
                            *self.episodes.borrow_mut() = Some(EpisodeList { show, episodes });
                            self.clear_cards();
                        }
                        Ok((show, episodes)) => {
                            *self.episodes.borrow_mut() = Some(EpisodeList { show, episodes });
                            self.render_episodes();
                            if !extra.is_empty() {
                                let current = self.s_status.text();
                                self.s_status.set_text(&format!("{current} ·{extra}"));
                            }
                        }
                        Err(err) => self
                            .s_status
                            .set_text(&format!("Could not load episodes: {err}.{extra}")),
                    }
                }
                Msg::Note(text) => self.set_note(&text),
                Msg::ServerShows(list) => {
                    *self.server.borrow_mut() = list;
                    self.rebuild_library();
                }
                Msg::Extras(show_id, extras) => {
                    self.extras.borrow_mut().insert(show_id.clone(), (*extras).clone());
                    let show = self
                        .current_show
                        .borrow()
                        .clone()
                        .filter(|s| s.id == show_id);
                    if let Some(show) = show {
                        self.fill_masthead(&show, Some(&extras));
                        self.fill_about(&show, &extras);
                    }
                }
                Msg::Latest(list) => {
                    *self.latest.borrow_mut() = list;
                    if self.current_page() == "home" {
                        self.render_new_row();
                        self.render_shows_grid();
                    }
                }
                Msg::Chapters(id, list) => self.fill_chapters(&id, list),
                Msg::Transcript(id, result) => self.fill_transcript(&id, result),
            }
        }
    }
}

impl Clone for EpisodeList {
    fn clone(&self) -> Self {
        Self {
            show: self.show.clone(),
            episodes: self.episodes.clone(),
        }
    }
}

// ── free helpers ────────────────────────────────────────────────────────────

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
        Browse::Trending => format!("{n} trending shows"),
    }
}

fn is_recent(ep: &Episode) -> bool {
    let published = podcasts::publish_key(ep.published.as_deref());
    published > 0 && chrono::Utc::now().timestamp() - published < NEW_DAYS * 86_400
}

fn download_label(ep: &Episode) -> String {
    if library::is_downloading(ep) {
        "Downloading…".into()
    } else if library::downloaded_path(ep).is_some() {
        "Remove download".into()
    } else {
        "Download".into()
    }
}

fn star_text(stars: u8) -> String {
    let n = stars.min(5) as usize;
    format!("{}{}", "★".repeat(n), "☆".repeat(5 - n))
}

fn clock(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
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
    let end = cut
        .rfind(". ")
        .map(|i| i + 1)
        .or_else(|| cut.rfind(' '))
        .unwrap_or(cut.len());
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
        BioState::Missing => slot
            .bio
            .set_text(&format!("No public biography found for {}.", slot.name)),
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

fn clear_box(b: &GtkBox) {
    while let Some(child) = b.first_child() {
        b.remove(&child);
    }
}

fn styled_label(text: &str, class: &str) -> Label {
    let label = Label::new(Some(text));
    label.add_css_class(class);
    label.set_halign(Align::Start);
    label.set_xalign(0.0);
    label
}

/// A label that takes its column's width and ellipsizes instead of growing it.
fn fitted_label(text: &str, class: &str, lines: i32) -> Label {
    let label = styled_label(text, class);
    label.set_halign(Align::Fill);
    label.set_hexpand(true);
    label.set_max_width_chars(1);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    if lines > 1 {
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        label.set_lines(lines);
    }
    label
}

fn tab_chip(text: &str) -> Button {
    let button = Button::with_label(text);
    button.add_css_class("nm-chip");
    button
}

/// A chapter or transcript line: timestamp, optional speaker, text.
fn timed_row(start: Option<u64>, speaker: &str, text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("nm-lib-row");
    let line = GtkBox::new(Orientation::Horizontal, 12);
    line.set_margin_top(6);
    line.set_margin_bottom(6);
    line.set_margin_start(10);
    line.set_margin_end(10);
    let stamp = styled_label(&start.map(clock).unwrap_or_default(), "nm-ep-meta");
    stamp.set_width_chars(7);
    stamp.set_valign(Align::Start);
    let col = GtkBox::new(Orientation::Vertical, 2);
    col.set_hexpand(true);
    if !speaker.trim().is_empty() {
        col.append(&styled_label(speaker.trim(), "nm-ep-meta"));
    }
    let body = styled_label(text, "nm-ep-desc");
    body.set_wrap(true);
    body.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    col.append(&body);
    line.append(&stamp);
    line.append(&col);
    row.set_child(Some(&line));
    row.set_activatable(start.is_some());
    row
}

fn show_tile(show: &Show, sub: &str, fresh: bool) -> Overlay {
    show_tile_sized(show, sub, fresh, TILE)
}

/// A cover tile with the title and a line of detail over the artwork.
fn show_tile_sized(show: &Show, sub: &str, fresh: bool, px: i32) -> Overlay {
    let overlay = Overlay::new();
    overlay.add_css_class("nm-tile");
    let art = gtk4::Image::new();
    art.add_css_class("nm-tile-art");
    art.set_pixel_size(px);
    art.set_size_request(px, px);
    art.set_icon_name(Some("multimedia-player-symbolic"));
    bind_cover(&art, show.artwork.as_deref());
    overlay.set_child(Some(&art));
    let shade = GtkBox::new(Orientation::Vertical, 1);
    shade.add_css_class("nm-tile-shade");
    shade.set_valign(Align::End);
    let title = fitted_label(&show.title, "nm-tile-title", 2);
    let subtitle = fitted_label(sub, "nm-tile-sub", 1);
    shade.append(&title);
    shade.append(&subtitle);
    overlay.add_overlay(&shade);
    if fresh {
        let tag = styled_label("NEW", "nm-new-tag");
        tag.set_halign(Align::Start);
        tag.set_valign(Align::Start);
        tag.set_margin_top(8);
        tag.set_margin_start(8);
        overlay.add_overlay(&tag);
    }
    overlay.update_property(&[gtk4::accessible::Property::Label(&show.title)]);
    overlay
}

/// A horizontal-row card: artwork (with progress), show, bold title, detail.
fn media_card(
    art_url: Option<&str>,
    show_title: &str,
    title: &str,
    detail: &str,
    progress: Option<f64>,
    fresh: bool,
) -> Button {
    let button = Button::new();
    button.add_css_class("nm-card");
    button.set_has_frame(false);
    let col = GtkBox::new(Orientation::Vertical, 4);
    col.set_size_request(CARD, -1);
    let overlay = Overlay::new();
    let art = gtk4::Image::new();
    art.add_css_class("nm-card-art");
    art.set_pixel_size(CARD);
    art.set_size_request(CARD, CARD);
    art.set_icon_name(Some("multimedia-player-symbolic"));
    bind_cover(&art, art_url);
    overlay.set_child(Some(&art));
    if let Some(frac) = progress {
        let bar = ProgressBar::new();
        bar.add_css_class("nm-ep-progress");
        bar.set_fraction(frac.clamp(0.0, 1.0));
        bar.set_valign(Align::End);
        bar.set_margin_start(10);
        bar.set_margin_end(10);
        bar.set_margin_bottom(10);
        overlay.add_overlay(&bar);
    }
    if fresh {
        let tag = styled_label("NEW", "nm-new-tag");
        tag.set_valign(Align::Start);
        tag.set_margin_top(8);
        tag.set_margin_start(8);
        overlay.add_overlay(&tag);
    }
    col.append(&overlay);
    let meta = fitted_label(&show_title.to_uppercase(), "nm-ep-meta", 1);
    let name = fitted_label(title, "nm-ep-card-title", 2);
    name.add_css_class("nm-ep-unplayed");
    let sub = fitted_label(detail, "nm-muted", 1);
    col.append(&meta);
    col.append(&name);
    col.append(&sub);
    button.set_child(Some(&col));
    button.update_property(&[gtk4::accessible::Property::Label(title)]);
    button
}

fn save_button(page: &PodcastPage, show: &Show) -> Button {
    let saved = page.is_saved(show);
    let button = Button::with_label(if saved { "Saved" } else { "Save" });
    button.add_css_class("nm-save");
    if saved {
        button.add_css_class("saved");
    }
    button.set_valign(Align::Center);
    button.set_tooltip_text(Some("Save this show to your library"));
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

fn section_header(text: &str) -> Label {
    let label = styled_label(text, "nm-ep-section");
    label.set_margin_top(6);
    label
}

fn horizontal_row() -> (ScrolledWindow, GtkBox) {
    let row = GtkBox::new(Orientation::Horizontal, 14);
    let scroll = ScrolledWindow::builder().child(&row).build();
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    scroll.set_min_content_height(CARD + 110);
    (scroll, row)
}

fn flow_grid() -> FlowBox {
    let flow = FlowBox::new();
    flow.add_css_class("nm-tile-grid");
    flow.set_selection_mode(SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(2);
    flow.set_max_children_per_line(10);
    flow.set_row_spacing(14);
    flow.set_column_spacing(14);
    flow.set_valign(Align::Start);
    flow.set_halign(Align::Start);
    flow.set_activate_on_single_click(true);
    flow
}

fn page_scroll(child: &GtkBox) -> ScrolledWindow {
    let scroll = ScrolledWindow::builder()
        .child(child)
        .vexpand(true)
        .hexpand(true)
        .build();
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll.set_propagate_natural_height(false);
    scroll.set_min_content_height(120);
    scroll
}

fn back_button(label: &str) -> Button {
    let back = Button::with_label(label);
    back.add_css_class("nm-chip");
    back.set_halign(Align::Start);
    back
}

pub(crate) fn build_podcast_page(state: Rc<AppState>) -> PodcastPage {
    let root = GtkBox::new(Orientation::Vertical, 6);
    root.add_css_class("nm-main");
    root.set_margin_top(8);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_margin_bottom(8);

    let banner = styled_label("", "nm-banner");
    banner.set_wrap(true);
    banner.set_visible(false);

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::Crossfade);
    stack.set_transition_duration(220);
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    // ── home ──
    let home = GtkBox::new(Orientation::Vertical, 10);
    let masthead = GtkBox::new(Orientation::Horizontal, 12);
    let heading = styled_label("Podcasts", "nm-mast-title");
    heading.set_hexpand(true);
    let discover_btn = Button::with_label("Discover shows");
    discover_btn.add_css_class("nm-chip");
    discover_btn.add_css_class("nm-chip-on");
    discover_btn.set_valign(Align::Center);
    discover_btn.set_widget_name("nm-podcast-discover");
    masthead.append(&heading);
    masthead.append(&discover_btn);
    home.append(&masthead);
    let rule = gtk4::Separator::new(Orientation::Horizontal);
    rule.add_css_class("nm-mast-rule");
    home.append(&rule);

    let cont_wrap = GtkBox::new(Orientation::Vertical, 6);
    cont_wrap.append(&section_header("CONTINUE LISTENING"));
    let (cont_scroll, cont_row) = horizontal_row();
    cont_wrap.append(&cont_scroll);
    cont_wrap.set_visible(false);
    home.append(&cont_wrap);

    let new_wrap = GtkBox::new(Orientation::Vertical, 6);
    new_wrap.append(&section_header("NEW EPISODES FROM YOUR SHOWS"));
    let (new_scroll, new_row) = horizontal_row();
    new_wrap.append(&new_scroll);
    new_wrap.set_visible(false);
    home.append(&new_wrap);

    home.append(&section_header("YOUR SHOWS"));
    let shows_empty = styled_label(
        "No shows yet. Open Discover shows to find podcasts and press Save.",
        "nm-muted",
    );
    shows_empty.set_wrap(true);
    home.append(&shows_empty);
    let shows_grid = flow_grid();
    home.append(&shows_grid);
    let home_scroll = page_scroll(&home);
    stack.add_named(&home_scroll, Some("home"));

    // ── discover ──
    let discover = GtkBox::new(Orientation::Vertical, 8);
    let d_back = back_button("‹ Podcasts");
    d_back.set_widget_name("nm-podcast-back");
    discover.append(&d_back);
    discover.append(&styled_label("Discover", "nm-mast-title"));
    let search_row = GtkBox::new(Orientation::Horizontal, 8);
    let search = Entry::new();
    search.add_css_class("nm-search");
    search.set_placeholder_text(Some("Show, host, or topic"));
    search.set_hexpand(true);
    search.set_widget_name("nm-podcast-search");
    let search_btn = Button::with_label("Search");
    search_btn.add_css_class("nm-chip");
    search_btn.set_widget_name("nm-podcast-search-btn");
    search_row.append(&search);
    search_row.append(&search_btn);
    discover.append(&search_row);
    let chips_box = GtkBox::new(Orientation::Horizontal, 6);
    let chips_scroll = ScrolledWindow::builder().child(&chips_box).build();
    chips_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    chips_scroll.set_min_content_height(48);
    discover.append(&chips_scroll);
    let d_status = styled_label("Looking up podcasts…", "nm-muted");
    d_status.set_wrap(true);
    d_status.set_widget_name("nm-podcast-status");
    discover.append(&d_status);
    let d_grid = flow_grid();
    discover.append(&d_grid);
    stack.add_named(&page_scroll(&discover), Some("discover"));

    // ── show ──
    let show_page = GtkBox::new(Orientation::Vertical, 10);
    let s_back = back_button("‹ Back");
    show_page.append(&s_back);
    let s_masthead = GtkBox::new(Orientation::Horizontal, 18);
    s_masthead.add_css_class("nm-show-header");
    show_page.append(&s_masthead);
    let filter_row = GtkBox::new(Orientation::Horizontal, 6);
    let filters = [
        (Filter::All, "All"),
        (Filter::Unplayed, "Unplayed"),
        (Filter::Downloaded, "Downloaded"),
        (Filter::InProgress, "In progress"),
    ];
    let s_filters: Rc<RefCell<Vec<(Filter, Button)>>> = Rc::new(RefCell::new(Vec::new()));
    for (f, label) in filters {
        let button = Button::with_label(label);
        button.add_css_class("nm-chip");
        if f == Filter::All {
            button.add_css_class("nm-chip-on");
        }
        filter_row.append(&button);
        s_filters.borrow_mut().push((f, button));
    }
    show_page.append(&filter_row);
    let s_status = styled_label("", "nm-muted");
    s_status.set_wrap(true);
    s_status.set_widget_name("nm-podcast-episodes-status");
    show_page.append(&s_status);
    let s_list = ListBox::new();
    s_list.set_selection_mode(SelectionMode::None);
    s_list.add_css_class("nm-library-list");
    s_list.set_widget_name("nm-podcast-list");
    show_page.append(&s_list);
    let s_more = Button::with_label("More episodes");
    s_more.add_css_class("nm-chip");
    s_more.set_halign(Align::Start);
    s_more.set_visible(false);
    s_more.set_widget_name("nm-podcast-more");
    show_page.append(&s_more);
    let s_about = GtkBox::new(Orientation::Vertical, 8);
    s_about.set_margin_top(12);
    show_page.append(&s_about);
    stack.add_named(&page_scroll(&show_page), Some("show"));

    // ── episode ──
    let episode_page = GtkBox::new(Orientation::Vertical, 10);
    let e_back = back_button("‹ Back");
    episode_page.append(&e_back);
    let e_body = GtkBox::new(Orientation::Vertical, 10);
    episode_page.append(&e_body);
    stack.add_named(&page_scroll(&episode_page), Some("episode"));

    root.append(&banner);
    root.append(&stack);

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
        stack: stack.clone(),
        nav: Rc::new(RefCell::new(Vec::new())),
        banner,
        banner_gen: Rc::new(Cell::new(0)),
        cont_wrap,
        cont_row,
        cont_sig: Rc::new(RefCell::new(String::new())),
        new_wrap,
        new_row,
        shows_grid: shows_grid.clone(),
        shows_empty,
        library_shows: Rc::new(RefCell::new(Vec::new())),
        latest: Rc::new(RefCell::new(Vec::new())),
        d_status,
        d_grid: d_grid.clone(),
        d_shows: Rc::new(RefCell::new(Vec::new())),
        chips: Rc::new(RefCell::new(Vec::new())),
        mode: Rc::new(RefCell::new(Browse::Trending)),
        dir_gen: Rc::new(Cell::new(0)),
        s_masthead,
        s_list: s_list.clone(),
        s_status,
        s_more: s_more.clone(),
        s_about,
        s_filters: s_filters.clone(),
        filter: Rc::new(Cell::new(Filter::All)),
        shown_n: Rc::new(Cell::new(podcasts::PAGE)),
        episodes: Rc::new(RefCell::new(None)),
        current_show: Rc::new(RefCell::new(None)),
        cards: Rc::new(RefCell::new(Vec::new())),
        ep_gen: Rc::new(Cell::new(0)),
        extras: Rc::new(RefCell::new(HashMap::new())),
        e_body,
        ctx: Rc::new(RefCell::new(None)),
        ep_ui: Rc::new(RefCell::new(None)),
        speed: Rc::new(Cell::new(1.0)),
        speed_applied: Rc::new(RefCell::new(String::new())),
        sleep: Rc::new(Cell::new(Sleep::Off)),
        auto_played: Rc::new(RefCell::new(String::new())),
        favorites: Rc::new(RefCell::new(podcasts::load_favorites())),
        server: Rc::new(RefCell::new(Vec::new())),
        inbox: Arc::new(parking_lot::Mutex::new(Vec::new())),
        bios: Rc::new(RefCell::new(HashMap::new())),
        slots: Rc::new(RefCell::new(Vec::new())),
        bio_jobs,
        bio_done,
    };

    // Directory chips and search.
    let ignore = Rc::new(Cell::new(false));
    let search_gen = Rc::new(Cell::new(0u64));
    let mut specs: Vec<(String, String, Browse)> =
        vec![("trending".into(), "Trending".into(), Browse::Trending)];
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
            page_b.start_directory();
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
            page.start_directory();
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

    // Navigation.
    {
        let page = page.clone();
        discover_btn.connect_clicked(move |_| {
            page.go("discover");
            if page.d_shows.borrow().is_empty() {
                page.start_directory();
            }
        });
    }
    for back in [&d_back, &s_back, &e_back] {
        let page = page.clone();
        back.connect_clicked(move |_| page.back());
    }
    {
        let page = page.clone();
        d_grid.connect_child_activated(move |_, child| {
            let show = page.d_shows.borrow().get(child.index() as usize).cloned();
            if let Some(show) = show {
                page.open_show(show);
            }
        });
    }
    {
        let page = page.clone();
        shows_grid.connect_child_activated(move |_, child| {
            let show = page.library_shows.borrow().get(child.index() as usize).cloned();
            if let Some(show) = show {
                page.open_show(show);
            }
        });
    }
    {
        let page = page.clone();
        s_more.connect_clicked(move |_| {
            page.shown_n.set(page.shown_n.get() + podcasts::PAGE);
            page.render_episodes();
        });
    }
    for (f, button) in s_filters.borrow().iter() {
        let page = page.clone();
        let f = *f;
        button.connect_clicked(move |_| {
            page.filter.set(f);
            for (other, b) in page.s_filters.borrow().iter() {
                if *other == f {
                    b.add_css_class("nm-chip-on");
                } else {
                    b.remove_css_class("nm-chip-on");
                }
            }
            page.shown_n.set(podcasts::PAGE);
            page.render_episodes();
        });
    }

    // Heartbeat: incoming work, playback chores, live button state.
    {
        let page = page.clone();
        let beat = Cell::new(0u32);
        glib::timeout_add_local(Duration::from_millis(200), move || {
            page.drain_inbox();
            page.apply_bios();
            page.playback_tick();
            let n = beat.get().wrapping_add(1);
            beat.set(n);
            if n % 5 == 0 {
                page.refresh_all();
                if page.current_page() == "home" {
                    page.render_continue(false);
                }
            }
            glib::ControlFlow::Continue
        });
    }

    page.set_active_chip("trending");
    stack.set_visible_child_name("home");
    *page.library_shows.borrow_mut() = page.collect_library();
    page.refresh_home();
    page.start_latest();
    page
}

impl Show {
    /// Short second line for cover tiles.
    fn subtitle_short(&self) -> String {
        if !self.author.trim().is_empty() {
            self.author.trim().to_string()
        } else {
            podcasts::clip(&podcasts::strip_html(&self.description), 40)
        }
    }
}
