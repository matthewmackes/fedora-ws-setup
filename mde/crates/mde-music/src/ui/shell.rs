use crate::acquire::{self, AcquireQueue, LibraryUploader};
use crate::backend::{self, MediaBackend};
use crate::cache;
use crate::cast::{self, CastDevice};
use crate::control::http::{self, ControlState};
use crate::history::History;
use crate::model::*;
use crate::mpris;
use crate::places;
use crate::player::Player;
use crate::radio::{self, Station};
use crate::sync::playlists;
use crate::taste::TasteProfile;
use crate::ytm;
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox,
    ListBoxRow, Orientation, Paned, Popover, Scale, ScrolledWindow, Separator, Stack, ToggleButton,
};
use parking_lot::RwLock;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::{mpsc, Arc, OnceLock};

const APP_ID: &str = "org.mde.NeighbourhoodMusic";
const CONTROL_PORT: u16 = 3847;
const LAST_SERVER_KEY: &str = "last_server_id";

pub(crate) struct AppState {
    pub(crate) backend: RwLock<Option<Arc<dyn MediaBackend>>>,
    pub(crate) ytm: RwLock<Option<Arc<dyn MediaBackend>>>,
    pub(crate) server: RwLock<Option<ServerTarget>>,
    pub(crate) player: Player,
    pub(crate) history: History,
    pub(crate) taste: TasteProfile,
    pub(crate) acquire: Arc<AcquireQueue>,
    pub(crate) now: Arc<RwLock<Option<Track>>>,
    pub(crate) cast_devices: RwLock<Vec<CastDevice>>,
    pub(crate) stage_open: Cell<bool>,
    pub(crate) mini_player: Cell<bool>,
}

struct BackendUploader {
    backend: Arc<dyn MediaBackend>,
}

impl LibraryUploader for BackendUploader {
    fn upload_file(&self, path: &Path, relative: &Path) -> Result<()> {
        self.backend.upload_library_file(path, relative)
    }
}

#[derive(Clone)]
enum NavFrame {
    Home,
    ArtistAlbums { artist: Artist, albums: Vec<Album> },
    Album { album: Album, tracks: Vec<Track> },
    Playlist { playlist: Playlist, tracks: Vec<Track> },
}

pub fn run_app() -> Result<()> {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        if let Err(e) = build(app) {
            eprintln!("Neighbourhood Music failed: {e:#}");
        }
    });
    app.run();
    Ok(())
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    let css = include_str!("../../data/style.css");
    provider.load_from_string(css);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_USER,
    );
}

fn build(app: &Application) -> Result<()> {
    load_css();
    let _ = cache::ensure_dirs();
    let player = Player::new()?;
    mpris::spawn(player.clone());
    let history = History::open()?;
    let now = Arc::new(RwLock::new(None));
    http::spawn(
        ControlState {
            player: player.clone(),
            now: now.clone(),
        },
        CONTROL_PORT,
    );

    let taste = TasteProfile::open()?;
    let acquire = AcquireQueue::new();
    let ytm_be = backend::youtube_music_source().ok();
    ytm::repair::schedule_proactive_checks();

    let state = Rc::new(AppState {
        backend: RwLock::new(None),
        ytm: RwLock::new(ytm_be),
        server: RwLock::new(None),
        player,
        history,
        taste,
        acquire,
        now,
        cast_devices: RwLock::new(Vec::new()),
        stage_open: Cell::new(false),
        mini_player: Cell::new(false),
    });

    let win = ApplicationWindow::builder()
        .application(app)
        .title("Neighbourhood Music")
        .default_width(1280)
        .default_height(800)
        .decorated(true)
        .build();
    win.add_css_class("nm-window");
    // Compositor SSD + in-app chrome for min/max/close and mini-player (Q7).

    let root = GtkBox::new(Orientation::Vertical, 0);
    let chrome = build_window_chrome(win.clone(), state.clone());
    root.append(&chrome);

    let repair_banner = build_repair_banner();
    root.append(&repair_banner);

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_widget_name("nm-root-stack");

    let startup = places::startup_server(
        places::list_servers().unwrap_or_default(),
        state.history.get_setting(LAST_SERVER_KEY).as_deref(),
    );
    if let Some(server) = startup.clone() {
        *state.server.write() = Some(server);
    }

    let picker = build_picker(state.clone(), stack.clone());
    let main = build_main(state.clone(), stack.clone());
    stack.add_named(&picker, Some("picker"));
    stack.add_named(&main, Some("main"));
    if state.server.read().is_some() {
        stack.set_visible_child_name("main");
    } else {
        stack.set_visible_child_name("picker");
    }

    root.append(&stack);
    win.set_child(Some(&root));
    win.present();

    if let Some(server) = startup {
        reconnect_server(state.clone(), stack.clone(), server);
    }

    // GStreamer bus → auto-advance (gapless-ish). Dead radio streams step to the next station.
    let bus = state.player.bus();
    let st = state.clone();
    let radio_skip_at = Rc::new(Cell::new(
        std::time::Instant::now() - std::time::Duration::from_secs(10),
    ));
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        let mut ended = false;
        let mut failed = false;
        while let Some(msg) = bus.pop() {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(_) => ended = true,
                MessageView::Error(e) => {
                    tracing::warn!("gstreamer: {}", e.error());
                    failed = true;
                }
                _ => {}
            }
        }
        let current = st.player.current();
        let radio_now = current.as_ref().is_some_and(is_radio_track);
        let podcast_now = current.as_ref().is_some_and(crate::podcasts::is_podcast_track);
        if failed && radio_now {
            let ready = radio_skip_at.get().elapsed() > std::time::Duration::from_millis(900);
            let fails = radio_fail_count().load(std::sync::atomic::Ordering::Relaxed);
            if ready && fails < 8 {
                radio_fail_count().store(fails + 1, std::sync::atomic::Ordering::Relaxed);
                radio_skip_at.set(std::time::Instant::now());
                if let Some(next) = st.player.next_in_order() {
                    play_track(&st, next);
                }
            }
        } else if ended && podcast_now {
            let _ = st.player.pause();
        } else if ended {
            let next = if radio_now {
                st.player.next_in_order()
            } else {
                st.player.advance()
            };
            if let Some(next) = next {
                play_track(&st, next);
            }
        }
        if let Some(track) = st.player.current() {
            if crate::podcasts::is_podcast_track(&track) {
                let pos = st.player.position_secs();
                let dur = st.player.duration_secs();
                if let Some(seek) = crate::podcasts::take_resume(&track.id, pos, dur) {
                    let _ = st.player.seek_secs(seek);
                }
                crate::podcasts::note_progress(&track.id, pos);
            }
        }
        if st.player.position_secs() > 2 && st.player.is_playing() {
            radio_fail_count().store(0, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some((id, url)) = crate::radio::take_click_url() {
            let url = crate::urlfix::normalize_http_url(&url);
            let same = st.now.read().as_ref().is_some_and(|t| t.id == id);
            let current = st
                .now
                .read()
                .as_ref()
                .map(|t| crate::urlfix::normalize_http_url(&t.stream_url))
                .unwrap_or_default();
            if same && !url.is_empty() && url != current {
                if let Some(t) = st.now.write().as_mut() {
                    t.stream_url = url.clone();
                }
                let _ = st.player.play_uri(&url);
            }
        }
        if st.player.take_scrobble_flag() {
            if let Some(t) = st.player.current() {
                if !is_radio_track(&t) && !crate::podcasts::is_podcast_track(&t) {
                    if let Some(be) = st.backend.read().clone() {
                        let _ = be.scrobble(&t.id, true);
                    }
                    if let Some(srv) = st.server.read().clone() {
                        let _ = st.history.record_listen(&srv.id, &t.id, &t.title, &t.artist);
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    });

    Ok(())
}

fn is_radio_track(track: &Track) -> bool {
    track.id.starts_with("rb:") || track.id.starts_with("srv:")
}

fn radio_fail_count() -> &'static std::sync::atomic::AtomicU8 {
    static COUNT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
    &COUNT
}

pub(crate) fn play_track(state: &AppState, mut track: Track) {
    let radio = is_radio_track(&track);
    let podcast = crate::podcasts::is_podcast_track(&track);
    let is_ytm = ytm::is_ytm_track(&track);
    // Stream over HTTP directly — do not pre-download from the media server.
    let play_uri = if radio || podcast {
        crate::urlfix::normalize_http_url(&track.stream_url)
    } else if is_ytm {
        let resolved = ytm::video_id_of(&track).and_then(|vid| {
            ytm::YtmEngine::new().resolve_stream(&vid).ok()
        });
        if let Some(r) = &resolved {
            track.bit_rate = r.bitrate.or(track.bit_rate);
            track.format = r.format.clone().or(track.format.clone());
            track.sample_rate = r.sample_rate.or(track.sample_rate);
            track.video_url = r.video_url.clone().or(track.video_url.clone());
            track.stream_url = r.url.clone();
        }
        let url = if track.stream_url.is_empty() {
            state
                .ytm
                .read()
                .as_ref()
                .and_then(|b| b.stream_url(&track, state.player.transcode()).ok())
                .unwrap_or_default()
        } else {
            track.stream_url.clone()
        };
        crate::urlfix::normalize_http_url(&url)
    } else {
        crate::urlfix::normalize_http_url(
            &state
                .backend
                .read()
                .as_ref()
                .and_then(|b| b.stream_url(&track, state.player.transcode()).ok())
                .unwrap_or_else(|| track.stream_url.clone()),
        )
    };
    if !play_uri.contains("://") {
        tracing::error!("refusing to play malformed URI: {play_uri}");
        return;
    }
    tracing::debug!("play URI: {play_uri}");

    if radio {
        *state.now.write() = Some(track.clone());
        let _ = state.player.play_uri(&play_uri);
        if let Some(uuid) = track.id.strip_prefix("rb:") {
            crate::radio::note_click(&track.id, uuid);
        }
        return;
    }

    if podcast {
        *state.now.write() = Some(track.clone());
        let _ = state.player.play_uri(&play_uri);
        crate::podcasts::arm_resume(&track.id);
        return;
    }

    // Resolve web cover off the UI thread so Play stays instant.
    {
        let artist = track.artist.clone();
        let album = track.album.clone();
        let server_cover = track.cover_url.clone();
        let now = state.now.clone();
        let track_id = track.id.clone();
        std::thread::spawn(move || {
            let url = crate::artwork::prefer_web_cover(&artist, &album, server_cover);
            if let Some(u) = url {
                let _ = crate::artwork::fetch_image_file(&u);
                let mut g = now.write();
                if let Some(t) = g.as_mut() {
                    if t.id == track_id {
                        t.cover_url = Some(u);
                    }
                }
            }
        });
    }

    if let Some(be) = state.backend.read().as_ref() {
        let _ = be.scrobble(&track.id, false);
        let _ = be.report_progress(&track.id, 0, false);
    }
    *state.now.write() = Some(track.clone());
    let _ = state.player.play_uri(&play_uri);
}

fn activate_server(
    state: &AppState,
    stack: &Stack,
    server: ServerTarget,
    backend: Arc<dyn MediaBackend>,
) {
    let _ = backend.register_session();
    let retry = backend.clone();
    *state.backend.write() = Some(backend);
    crate::podcasts::retry_saved_subscriptions(retry);
    *state.server.write() = Some(server.clone());
    let _ = state.history.set_setting(LAST_SERVER_KEY, &server.id);
    stack.set_visible_child_name("main");
    if let Some(root) = stack.root() {
        show_connected_server(&root, &server);
    }
}

fn reconnect_server(state: Rc<AppState>, stack: Stack, server: ServerTarget) {
    let pending: Arc<
        parking_lot::Mutex<Option<std::result::Result<Arc<dyn MediaBackend>, String>>>,
    > = Arc::new(parking_lot::Mutex::new(None));
    let slot = pending.clone();
    let server_for_connect = server.clone();
    std::thread::spawn(move || {
        let result = backend::connect(&server_for_connect).map_err(|err| format!("{err:#}"));
        *slot.lock() = Some(result);
    });
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        let Some(result) = pending.lock().take() else {
            return glib::ControlFlow::Continue;
        };
        match result {
            Ok(backend) => activate_server(&state, &stack, server.clone(), backend),
            Err(err) => {
                *state.backend.write() = None;
                *state.server.write() = None;
                stack.set_visible_child_name("picker");
                let dialog = gtk4::AlertDialog::builder()
                    .message("Could not reconnect")
                    .detail(err)
                    .modal(true)
                    .build();
                dialog.show(stack.root().and_downcast_ref::<gtk4::Window>());
            }
        }
        glib::ControlFlow::Break
    });
}

fn build_picker(state: Rc<AppState>, stack: Stack) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 16);
    page.set_valign(Align::Center);
    page.set_halign(Align::Center);
    page.set_margin_top(48);
    page.set_margin_bottom(48);
    page.set_margin_start(48);
    page.set_margin_end(48);

    let brand = Label::new(Some("Neighbourhood Music"));
    brand.add_css_class("nm-brand");
    brand.set_markup("<span size='xx-large' weight='bold' foreground='#1db954'>Neighbourhood Music</span>");
    page.append(&brand);
    page.append(&Label::new(Some(
        "Choose a Jellyfin or Airsonic server from Network Neighbourhood",
    )));

    let list = ListBox::new();
    list.add_css_class("nm-server-list");
    let servers = places::list_servers().unwrap_or_default();
    if servers.is_empty() {
        page.append(&Label::new(Some(
            "No media servers found. Add one below, or save a Video/Audio place in Network Neighbourhood.",
        )));
    }
    for s in servers {
        let row = ListBoxRow::new();
        let box_ = GtkBox::new(Orientation::Horizontal, 12);
        box_.set_margin_top(8);
        box_.set_margin_bottom(8);
        box_.set_margin_start(8);
        box_.set_margin_end(8);
        let text = Label::new(Some(&format!(
            "{} — {} ({})",
            s.name,
            s.kind.label(),
            s.base_url
        )));
        text.set_hexpand(true);
        text.set_halign(Align::Start);
        box_.append(&text);
        let connect = Button::with_label("Connect");
        let st = state.clone();
        let stack2 = stack.clone();
        let srv = s.clone();
        connect.connect_clicked(move |_| {
            match backend::connect(&srv) {
                Ok(be) => activate_server(&st, &stack2, srv.clone(), be),
                Err(e) => {
                    let d = gtk4::AlertDialog::builder()
                        .message("Connection failed")
                        .detail(format!("{e:#}"))
                        .modal(true)
                        .build();
                    d.show(stack2.root().and_downcast_ref::<gtk4::Window>());
                }
            }
        });
        box_.append(&connect);
        row.set_child(Some(&box_));
        list.append(&row);
    }
    let scroll = ScrolledWindow::builder()
        .min_content_height(220)
        .min_content_width(640)
        .child(&list)
        .build();
    page.append(&scroll);

    // Manual add
    page.append(&Label::new(Some("Add server manually")));
    let form = GtkBox::new(Orientation::Horizontal, 8);
    let kind_jelly = ToggleButton::with_label("Jellyfin");
    let kind_air = ToggleButton::with_label("Airsonic");
    kind_jelly.set_active(true);
    kind_air.set_group(Some(&kind_jelly));
    let url_e = Entry::builder().placeholder_text("http://host:8096").build();
    let user_e = Entry::builder().placeholder_text("username").build();
    let pass_e = Entry::builder().placeholder_text("password / token").build();
    pass_e.set_visibility(false);
    let add_btn = Button::with_label("Save & Connect");
    form.append(&kind_jelly);
    form.append(&kind_air);
    form.append(&url_e);
    form.append(&user_e);
    form.append(&pass_e);
    form.append(&add_btn);
    page.append(&form);

    let st = state.clone();
    let stack2 = stack.clone();
    add_btn.connect_clicked(move |_| {
        let kind = if kind_jelly.is_active() {
            BackendKind::Jellyfin
        } else {
            BackendKind::Subsonic
        };
        let srv = ServerTarget {
            id: uuid::Uuid::new_v4().to_string(),
            name: url_e.text().to_string(),
            kind,
            base_url: url_e.text().to_string(),
            username: user_e.text().to_string(),
            password: pass_e.text().to_string(),
            from_neighbourhood: false,
        };
        let _ = places::save_manual_server(srv.clone());
        match backend::connect(&srv) {
            Ok(be) => activate_server(&st, &stack2, srv, be),
            Err(e) => {
                let d = gtk4::AlertDialog::builder()
                    .message("Connection failed")
                    .detail(format!("{e:#}"))
                    .modal(true)
                    .build();
                d.show(None::<&gtk4::Window>);
            }
        }
    });

    page
}

fn build_main(state: Rc<AppState>, root_stack: Stack) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_widget_name("nm-main-outer");

    let body = Paned::new(Orientation::Horizontal);
    body.set_hexpand(true);
    body.set_vexpand(true);
    body.set_widget_name("nm-browse-body");

    let content_stack = Stack::new();
    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);

    let home = labeled_scroller("Home");
    let search = build_search(state.clone());
    let (library_page, library_ui) = build_library_page();
    let radio = build_unified_radio(state.clone());
    let podcast_page = crate::ui::podcast_page::build_podcast_page(state.clone());
    let queue_view = labeled_scroller("Queue");
    let acquire_view = build_acquisitions_page(state.clone());
    let stage = build_stage(state.clone());
    let tools = build_tools_page(state.clone(), content_stack.clone());

    content_stack.add_named(&home.0, Some("home"));
    content_stack.add_named(&search, Some("search"));
    content_stack.add_named(&library_page, Some("library"));
    content_stack.add_named(&radio, Some("radio"));
    content_stack.add_named(podcast_page.widget(), Some("podcasts"));
    content_stack.add_named(&queue_view.0, Some("queue"));
    content_stack.add_named(&acquire_view, Some("acquire"));
    content_stack.add_named(&stage, Some("stage"));
    content_stack.add_named(&tools, Some("tools"));
    content_stack.set_visible_child_name("home");
    LIBRARY_UI.with(|slot| *slot.borrow_mut() = Some(library_ui.clone()));
    CONTENT_STACK.with(|slot| *slot.borrow_mut() = Some(content_stack.clone()));

    let sidebar = build_sidebar_nav(state.clone(), content_stack.clone(), root_stack.clone());

    body.set_start_child(Some(&sidebar));
    body.set_end_child(Some(&content_stack));
    body.set_resize_start_child(false);
    body.set_shrink_start_child(false);

    let player_bar = build_player_bar(state.clone(), content_stack.clone(), body.clone(), outer.clone());

    outer.append(&body);
    outer.append(&player_bar);

    // Status line so Home is never an empty void while loading
    let status = Label::new(Some(if state.server.read().is_some() {
        "Reconnecting…"
    } else {
        "Connect a server to load your library…"
    }));
    status.add_css_class("nm-muted");
    status.set_halign(Align::Start);
    status.set_widget_name("home-status");
    home.1.append(&status);

    {
        let ui = library_ui.clone();
        let st = state.clone();
        library_ui.back.connect_clicked(move |_| {
            ui.nav_gen.set(ui.nav_gen.get().wrapping_add(1));
            library_nav_back(&st, &ui);
        });
    }

    let st = state.clone();
    let home_box = home.1.clone();
    let podcast_page = podcast_page.clone();
    let queue_box = queue_view.1.clone();
    let library_for_load = library_ui.clone();
    let status_label = status.clone();
    struct Incoming {
        server_id: String,
        snapshot: UiSnapshot,
    }
    let pending_slot: Arc<parking_lot::Mutex<Option<Incoming>>> =
        Arc::new(parking_lot::Mutex::new(None));
    let loaded_server: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let inflight_server: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        if let Some(root) = status_label.root() {
            if let Some(server) = st.server.read().clone() {
                show_connected_server(&root, &server);
            }
        }
        if let Some(incoming) = pending_slot.lock().take() {
            let current = st
                .server
                .read()
                .as_ref()
                .map(|s| s.id.clone())
                .unwrap_or_default();
            *inflight_server.borrow_mut() = String::new();
            if incoming.server_id == current && !current.is_empty() {
                *loaded_server.borrow_mut() = current;
                status_label.set_text("Library loaded.");
                apply_ui_snapshot(&st, incoming.snapshot, &home_box, &library_for_load, &podcast_page, &queue_box);
            }
        }
        let server_id = st
            .server
            .read()
            .as_ref()
            .map(|s| s.id.clone())
            .unwrap_or_default();
        if server_id.is_empty()
            || !inflight_server.borrow().is_empty()
            || *loaded_server.borrow() == server_id
        {
            return glib::ControlFlow::Continue;
        }
        if let Some(be) = st.backend.read().clone() {
            *inflight_server.borrow_mut() = server_id.clone();
            status_label.set_text("Loading library…");
            clear_box(&library_for_load.body);
            let loading = Label::new(Some("Loading library…"));
            loading.add_css_class("nm-muted");
            loading.set_halign(Align::Start);
            library_for_load.body.append(&loading);
            let slot = pending_slot.clone();
            let ytm = st.ytm.read().clone();
            std::thread::spawn(move || {
                let snapshot = collect_ui_snapshot(be, ytm, server_id.clone());
                *slot.lock() = Some(Incoming {
                    server_id,
                    snapshot,
                });
            });
        }
        glib::ControlFlow::Continue
    });

    let st2 = state.clone();
    let queue_box2 = queue_view.1.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        fill_queue(&st2, &queue_box2);
        glib::ControlFlow::Continue
    });

    // Continuum: when queue ends, append similar tracks (radio-like).
    let st3 = state.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(800), move || {
        maybe_extend_queue_continuum(&st3);
        glib::ControlFlow::Continue
    });

    install_shortcuts(state.clone(), content_stack.clone());

    outer
}

#[derive(Clone)]
struct LibraryUi {
    body: GtkBox,
    title: Label,
    back: Button,
    crumbs: Label,
    nav_gen: Rc<Cell<u64>>,
    snapshot: Rc<RefCell<Option<UiSnapshot>>>,
    stack: Rc<RefCell<Vec<NavFrame>>>,
}

fn build_library_page() -> (GtkBox, LibraryUi) {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("nm-main");
    page.set_widget_name("library-page");

    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_top(8);
    header.set_margin_start(8);
    header.set_margin_end(8);
    let back = Button::with_label("Back");
    back.add_css_class("nm-nav-btn");
    back.set_visible(false);
    back.set_widget_name("library-back");
    let title = Label::new(Some("Your Library"));
    title.add_css_class("nm-heading");
    title.set_halign(Align::Start);
    title.set_hexpand(true);
    title.set_widget_name("library-title");
    header.append(&back);
    header.append(&title);
    page.append(&header);

    let crumbs = Label::new(Some("Library"));
    crumbs.add_css_class("nm-muted");
    crumbs.set_halign(Align::Start);
    crumbs.set_margin_start(8);
    crumbs.set_widget_name("library-crumbs");
    page.append(&crumbs);

    let body = GtkBox::new(Orientation::Vertical, 8);
    body.set_margin_start(8);
    body.set_margin_end(8);
    body.set_margin_bottom(24);
    let loading = Label::new(Some("Loading library…"));
    loading.add_css_class("nm-muted");
    loading.set_halign(Align::Start);
    body.append(&loading);

    let scroll = ScrolledWindow::builder().child(&body).build();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll.set_widget_name("library-scroll");
    page.append(&scroll);

    let ui = LibraryUi {
        body,
        title,
        back,
        crumbs,
        nav_gen: Rc::new(Cell::new(0)),
        snapshot: Rc::new(RefCell::new(None)),
        stack: Rc::new(RefCell::new(vec![NavFrame::Home])),
    };
    (page, ui)
}

fn clear_box(box_: &GtkBox) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
}

fn fetch_then<T, W, U>(ui: &LibraryUi, work: W, then: U)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    U: FnOnce(T) + 'static,
{
    let gen = ui.nav_gen.get().wrapping_add(1);
    ui.nav_gen.set(gen);
    let nav = ui.nav_gen.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    let mut then = Some(then);
    glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        match rx.try_recv() {
            Ok(value) => {
                if nav.get() == gen {
                    if let Some(then) = then.take() {
                        then(value);
                    }
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

fn row_label_text(row: &ListBoxRow) -> String {
    fn find(widget: &gtk4::Widget) -> Option<String> {
        if let Ok(label) = widget.clone().downcast::<Label>() {
            if label.has_css_class("nm-lib-label") {
                return Some(label.text().to_lowercase());
            }
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(text) = find(&next) {
                return Some(text);
            }
            child = next.next_sibling();
        }
        None
    }
    row.child().and_then(|widget| find(&widget)).unwrap_or_default()
}

fn cover_jobs() -> &'static mpsc::Sender<String> {
    static JOBS: OnceLock<mpsc::Sender<String>> = OnceLock::new();
    JOBS.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<String>();
        let rx = Arc::new(parking_lot::Mutex::new(rx));
        for _ in 0..4 {
            let rx = rx.clone();
            std::thread::spawn(move || loop {
                let url = {
                    let guard = rx.lock();
                    guard.recv()
                };
                let Ok(url) = url else { break };
                let fetch_url = crate::artwork::thumbnail_request_url(&url);
                let path = crate::artwork::fetch_image_file(&fetch_url).ok();
                cover_done().lock().push((url, path));
            });
        }
        tx
    })
}

fn cover_done() -> &'static parking_lot::Mutex<Vec<(String, Option<std::path::PathBuf>)>> {
    static DONE: OnceLock<parking_lot::Mutex<Vec<(String, Option<std::path::PathBuf>)>>> =
        OnceLock::new();
    DONE.get_or_init(|| parking_lot::Mutex::new(Vec::new()))
}

struct CoverHub {
    images: HashMap<String, Vec<gtk4::Image>>,
    queued: HashSet<String>,
}

fn cover_hub() -> Rc<RefCell<CoverHub>> {
    thread_local! {
        static HUB: RefCell<Option<Rc<RefCell<CoverHub>>>> = const { RefCell::new(None) };
    }
    HUB.with(|slot| {
        let mut guard = slot.borrow_mut();
        if guard.is_none() {
            let hub = Rc::new(RefCell::new(CoverHub {
                images: HashMap::new(),
                queued: HashSet::new(),
            }));
            let pump = hub.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
                let ready: Vec<_> = cover_done().lock().drain(..).collect();
                if !ready.is_empty() {
                    let mut hub = pump.borrow_mut();
                    for (url, path) in ready {
                        hub.queued.remove(&url);
                        if let Some(path) = path {
                            if let Some(images) = hub.images.remove(&url) {
                                for image in images {
                                    image.set_from_file(Some(&path));
                                }
                            }
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
            *guard = Some(hub);
        }
        guard.as_ref().unwrap().clone()
    })
}

pub(crate) fn bind_cover(image: &gtk4::Image, url: Option<&str>) {
    let Some(url) = url.map(str::trim).filter(|url| !url.is_empty()) else {
        return;
    };
    if let Some(path) = crate::artwork::cached_image_path(url) {
        image.set_from_file(Some(path));
        return;
    }
    let hub = cover_hub();
    let mut hub_ref = hub.borrow_mut();
    hub_ref
        .images
        .entry(url.to_string())
        .or_default()
        .push(image.clone());
    if hub_ref.queued.insert(url.to_string()) {
        let _ = cover_jobs().send(url.to_string());
    }
}

fn catalog_list<T, L, C, A>(items: Vec<T>, label_of: L, cover_of: C, on_activate: A) -> ListBox
where
    T: Clone + 'static,
    L: Fn(&T) -> String,
    C: Fn(&T) -> Option<String>,
    A: Fn(T) + 'static,
{
    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_activate_on_single_click(true);
    list.add_css_class("nm-library-list");
    list.set_hexpand(true);
    let held = Rc::new(items);
    for item in held.iter() {
        let row = ListBoxRow::new();
        row.add_css_class("nm-lib-row");
        let line = GtkBox::new(Orientation::Horizontal, 10);
        line.set_margin_top(4);
        line.set_margin_bottom(4);
        line.set_margin_start(8);
        line.set_margin_end(8);
        let cover = gtk4::Image::new();
        cover.add_css_class("nm-lib-cover");
        cover.set_pixel_size(48);
        cover.set_size_request(48, 48);
        cover.set_icon_name(Some("media-optical-symbolic"));
        bind_cover(&cover, cover_of(item).as_deref());
        let label = Label::new(Some(&label_of(item)));
        label.add_css_class("nm-lib-label");
        label.set_halign(Align::Start);
        label.set_hexpand(true);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        line.append(&cover);
        line.append(&label);
        row.set_child(Some(&line));
        list.append(&row);
    }
    let held_click = held.clone();
    list.connect_row_activated(move |_, row| {
        let idx = row.index();
        if idx < 0 {
            return;
        }
        if let Some(item) = held_click.get(idx as usize) {
            on_activate(item.clone());
        }
    });
    list
}

fn install_filter(list: &ListBox, query: &Rc<RefCell<String>>) {
    let query = query.clone();
    list.set_filter_func(move |row| {
        let needle = query.borrow().clone();
        needle.is_empty() || row_label_text(row).contains(&needle)
    });
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn artist_line(artist: &Artist) -> String {
    match artist.album_count {
        Some(1) => format!("{}   ·   1 album", artist.name),
        Some(n) => format!("{}   ·   {n} albums", artist.name),
        None => artist.name.clone(),
    }
}

fn album_line(album: &Album) -> String {
    match (album.artist.as_str(), album.year) {
        ("", None) => album.name.clone(),
        ("", Some(year)) => format!("{} ({year})", album.name),
        (artist, None) => format!("{} — {artist}", album.name),
        (artist, Some(year)) => format!("{} — {artist} ({year})", album.name),
    }
}

fn update_crumbs(ui: &LibraryUi) {
    let labels: Vec<String> = ui
        .stack
        .borrow()
        .iter()
        .map(|f| match f {
            NavFrame::Home => "Library".into(),
            NavFrame::ArtistAlbums { artist, .. } => artist.name.clone(),
            NavFrame::Album { album, .. } => album.name.clone(),
            NavFrame::Playlist { playlist, .. } => playlist.name.clone(),
        })
        .collect();
    ui.crumbs.set_text(&labels.join(" › "));
    ui.back.set_visible(ui.stack.borrow().len() > 1);
}

fn library_nav_back(state: &Rc<AppState>, ui: &LibraryUi) {
    {
        let mut stack = ui.stack.borrow_mut();
        if stack.len() > 1 {
            stack.pop();
        }
    }
    let frame = ui.stack.borrow().last().cloned();
    match frame {
        Some(NavFrame::Home) | None => {
            if let Some(snap) = ui.snapshot.borrow().clone() {
                render_library_home(state, ui, &snap);
            }
        }
        Some(NavFrame::ArtistAlbums { artist, albums }) => {
            show_album_list(state, ui, &artist.name, albums);
        }
        Some(NavFrame::Album { album, tracks }) => {
            show_immersive_album(state, ui, album, tracks);
        }
        Some(NavFrame::Playlist { playlist, tracks }) => {
            show_track_list(state, ui, &playlist.name, tracks, None);
        }
    }
    update_crumbs(ui);
}

fn render_library_home(state: &Rc<AppState>, ui: &LibraryUi, snap: &UiSnapshot) {
    *ui.stack.borrow_mut() = vec![NavFrame::Home];
    update_crumbs(ui);
    ui.title.set_text("Your Library");
    clear_box(&ui.body);

    let summary = Label::new(Some(&format!(
        "{} · {} · {}",
        plural(snap.artists.len(), "artist", "artists"),
        plural(snap.albums.len(), "album", "albums"),
        plural(snap.playlists.len(), "playlist", "playlists"),
    )));
    summary.add_css_class("nm-muted");
    summary.set_halign(Align::Start);
    summary.set_widget_name("library-summary");
    ui.body.append(&summary);

    let filter = Entry::new();
    filter.add_css_class("nm-search");
    filter.set_placeholder_text(Some("Filter artists and albums"));
    filter.set_widget_name("library-filter");
    ui.body.append(&filter);

    append_section(&ui.body, "Playlists");
    {
        let create = Button::with_label("+ New playlist");
        create.add_css_class("nm-transport");
        create.set_halign(Align::Start);
        let st = state.clone();
        let ui_c = ui.clone();
        create.connect_clicked(move |_| {
            let entry = Entry::new();
            entry.set_placeholder_text(Some("Playlist name"));
            entry.add_css_class("nm-search");
            let pop = Popover::new();
            pop.set_parent(&ui_c.back);
            let col = GtkBox::new(Orientation::Vertical, 6);
            let save = Button::with_label("Create");
            save.add_css_class("nm-transport");
            let st2 = st.clone();
            let entry2 = entry.clone();
            let pop2 = pop.clone();
            save.connect_clicked(move |_| {
                let name = entry2.text().to_string();
                if name.trim().is_empty() {
                    return;
                }
                if let Some(be) = st2.backend.read().clone() {
                    let _ = be.create_playlist(name.trim());
                }
                pop2.popdown();
            });
            col.append(&entry);
            col.append(&save);
            pop.set_child(Some(&col));
            pop.popup();
        });
        ui.body.append(&create);
    }
    if snap.playlists.is_empty() {
        ui.body.append(&Label::new(Some("No playlists yet.")));
    }
    for playlist in &snap.playlists {
        let count = playlist.song_count.unwrap_or(playlist.tracks.len() as u32);
        let btn = Button::with_label(&format!("{} ({count})", playlist.name));
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Fill);
        let state = state.clone();
        let ui_btn = ui.clone();
        let playlist = playlist.clone();
        btn.connect_clicked(move |_| open_playlist(&state, &ui_btn, playlist.clone()));
        ui.body.append(&btn);
    }

    append_section(&ui.body, "Artists");
    let artist_list = if snap.artists.is_empty() {
        ui.body.append(&Label::new(Some("No artists yet.")));
        None
    } else {
        let state = state.clone();
        let ui_open = ui.clone();
        let list = catalog_list(
            snap.artists.clone(),
            artist_line,
            |artist: &Artist| artist.cover_url.clone(),
            move |artist: Artist| {
            ui_open.title.set_text("Loading…");
            let id = artist.id.clone();
            let name = artist.name.clone();
            let backend = state.backend.read().clone();
            fetch_then(
                &ui_open,
                move || {
                    backend
                        .and_then(|be| be.artist_albums(&id).ok())
                        .unwrap_or_default()
                },
                {
                    let state = state.clone();
                    let ui_open = ui_open.clone();
                    let artist_meta = artist.clone();
                    move |albums: Vec<Album>| {
                        ui_open.stack.borrow_mut().push(NavFrame::ArtistAlbums {
                            artist: artist_meta,
                            albums: albums.clone(),
                        });
                        show_album_list(&state, &ui_open, &name, albums);
                    }
                },
            );
        });
        list.set_widget_name("library-artists");
        ui.body.append(&list);
        Some(list)
    };

    append_section(&ui.body, "Albums");
    let album_list = if snap.albums.is_empty() {
        ui.body.append(&Label::new(Some("No albums yet.")));
        None
    } else {
        let state = state.clone();
        let ui_open = ui.clone();
        let list = catalog_list(
            snap.albums.clone(),
            album_line,
            |album: &Album| album.cover_url.clone(),
            move |album: Album| {
            open_album(&state, &ui_open, album);
        });
        list.set_widget_name("library-albums");
        ui.body.append(&list);
        Some(list)
    };

    if artist_list.is_some() || album_list.is_some() {
        let query = Rc::new(RefCell::new(String::new()));
        if let Some(list) = &artist_list {
            install_filter(list, &query);
        }
        if let Some(list) = &album_list {
            install_filter(list, &query);
        }
        let artists = artist_list.clone();
        let albums = album_list.clone();
        filter.connect_changed(move |entry| {
            *query.borrow_mut() = entry.text().to_lowercase();
            if let Some(list) = &artists {
                list.invalidate_filter();
            }
            if let Some(list) = &albums {
                list.invalidate_filter();
            }
        });
    }
}

fn show_album_list(state: &Rc<AppState>, ui: &LibraryUi, heading: &str, albums: Vec<Album>) {
    ui.title.set_text(heading);
    clear_box(&ui.body);
    update_crumbs(ui);
    if albums.is_empty() {
        ui.body.append(&Label::new(Some("No albums.")));
        return;
    }
    let state = state.clone();
    let ui_open = ui.clone();
    let list = catalog_list(albums, album_line, |album: &Album| album.cover_url.clone(), move |album: Album| {
        open_album(&state, &ui_open, album);
    });
    list.set_widget_name("library-artist-albums");
    ui.body.append(&list);
}

fn open_album(state: &Rc<AppState>, ui: &LibraryUi, album: Album) {
    ui.title.set_text("Loading…");
    let id = album.id.clone();
    let album_meta = album.clone();
    let backend = state.backend.read().clone();
    let ytm = state.ytm.read().clone();
    fetch_then(
        ui,
        move || {
            if id.starts_with("ytm:") {
                ytm.and_then(|be| be.album_tracks(&id).ok())
                    .unwrap_or_default()
            } else {
                backend
                    .and_then(|be| be.album_tracks(&id).ok())
                    .unwrap_or_default()
            }
        },
        {
            let state = state.clone();
            let ui = ui.clone();
            move |tracks: Vec<Track>| {
                ui.stack.borrow_mut().push(NavFrame::Album {
                    album: album_meta.clone(),
                    tracks: tracks.clone(),
                });
                show_immersive_album(&state, &ui, album_meta, tracks);
            }
        },
    );
}

/// Open album detail from Home/Search cards (shared route; no autoplay).
pub(crate) fn open_album_detail(state: &Rc<AppState>, content: &Stack, album: Album) {
    content.set_visible_child_name("library");
    // Find library UI via widget tree is awkward; use a global hook.
    if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
        open_album(state, &ui, album);
    }
}

thread_local! {
    static LIBRARY_UI: RefCell<Option<LibraryUi>> = const { RefCell::new(None) };
    static CONTENT_STACK: RefCell<Option<Stack>> = const { RefCell::new(None) };
}

fn open_playlist(state: &Rc<AppState>, ui: &LibraryUi, playlist: Playlist) {
    ui.title.set_text("Loading…");
    let id = playlist.id.clone();
    let playlist_meta = playlist.clone();
    let backend = state.backend.read().clone();
    fetch_then(
        ui,
        move || {
            backend
                .and_then(|be| be.playlist_tracks(&id).ok())
                .unwrap_or_default()
        },
        {
            let state = state.clone();
            let ui = ui.clone();
            move |tracks: Vec<Track>| {
                ui.stack.borrow_mut().push(NavFrame::Playlist {
                    playlist: playlist_meta.clone(),
                    tracks: tracks.clone(),
                });
                show_track_list(&state, &ui, &playlist_meta.name, tracks, None);
            }
        },
    );
}

fn show_immersive_album(state: &Rc<AppState>, ui: &LibraryUi, album: Album, tracks: Vec<Track>) {
    update_crumbs(ui);
    ui.title.set_text(&album.name);
    clear_box(&ui.body);
    ui.body.set_widget_name("library-album-immersive");

    let hero = GtkBox::new(Orientation::Vertical, 12);
    hero.add_css_class("nm-album-hero");
    let cover = cover_image(album.cover_url.as_deref());
    cover.set_pixel_size(280);
    cover.set_halign(Align::Center);
    hero.append(&cover);
    let sub = Label::new(Some(&format!(
        "{}{}",
        album.artist,
        album.year.map(|y| format!(" · {y}")).unwrap_or_default()
    )));
    sub.add_css_class("nm-muted");
    sub.set_halign(Align::Center);
    hero.append(&sub);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    actions.set_halign(Align::Center);
    let play_all = Button::with_label("Play");
    play_all.add_css_class("nm-transport");
    let shuffle = Button::with_label("Shuffle");
    shuffle.add_css_class("nm-icon");
    let st = state.clone();
    let queued = tracks.clone();
    play_all.connect_clicked(move |_| {
        if let Some(first) = queued.first() {
            st.player.set_queue(queued.clone(), 0);
            play_track(&st, first.clone());
        }
    });
    let st2 = state.clone();
    let queued2 = tracks.clone();
    shuffle.connect_clicked(move |_| {
        st2.player.set_shuffle(true);
        if let Some(first) = queued2.first() {
            st2.player.set_queue(queued2.clone(), 0);
            play_track(&st2, first.clone());
        }
    });
    actions.append(&play_all);
    actions.append(&shuffle);
    hero.append(&actions);
    ui.body.append(&hero);

    let frost = GtkBox::new(Orientation::Vertical, 4);
    frost.add_css_class("nm-frost-tracks");
    show_track_table(state, &frost, &tracks);
    ui.body.append(&frost);
}

fn show_track_list(
    state: &Rc<AppState>,
    ui: &LibraryUi,
    heading: &str,
    tracks: Vec<Track>,
    _album: Option<Album>,
) {
    update_crumbs(ui);
    ui.title.set_text(heading);
    clear_box(&ui.body);
    ui.body.set_widget_name("library-tracks");
    if tracks.is_empty() {
        ui.body.append(&Label::new(Some("No tracks.")));
        return;
    }
    let play_all = Button::with_label("Play");
    play_all.add_css_class("nm-transport");
    play_all.set_halign(Align::Start);
    let st = state.clone();
    let queued = tracks.clone();
    play_all.connect_clicked(move |_| {
        if let Some(first) = queued.first() {
            st.player.set_queue(queued.clone(), 0);
            play_track(&st, first.clone());
        }
    });
    ui.body.append(&play_all);
    show_track_table(state, &ui.body, &tracks);
}

fn show_track_table(state: &Rc<AppState>, parent: &GtkBox, tracks: &[Track]) {
    let header = Label::new(Some("#    Title                        Album              Time     Format"));
    header.add_css_class("nm-muted");
    header.set_halign(Align::Start);
    parent.append(&header);
    let all = tracks.to_vec();
    for (idx, track) in tracks.iter().enumerate() {
        parent.append(&track_row_indexed(state, track.clone(), all.clone(), idx));
    }
}

#[derive(Clone)]
enum RadioMode {
    Top,
    Favorites,
    Server,
    Tag(String),
    Text { name: String, country: String },
}

struct RadioIncoming {
    gen: u64,
    append: bool,
    result: std::result::Result<Vec<Station>, String>,
}

#[derive(Clone)]
struct RadioPage {
    state: Rc<AppState>,
    root: GtkBox,
    list: ListBox,
    status: Label,
    now: Label,
    more: Button,
    favorites: Rc<RefCell<Vec<Station>>>,
    server: Rc<RefCell<Vec<Station>>>,
    shown: Rc<RefCell<Vec<Station>>>,
    mode: Rc<RefCell<RadioMode>>,
    offset: Rc<Cell<u32>>,
    generation: Rc<Cell<u64>>,
    chips: Rc<RefCell<Vec<(String, Button)>>>,
    pending: Arc<parking_lot::Mutex<Option<RadioIncoming>>>,
}

impl RadioPage {
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

    fn is_saved(&self, station: &Station) -> bool {
        self.favorites.borrow().iter().any(|s| s.uuid == station.uuid)
    }

    fn toggle_saved(&self, station: &Station) -> bool {
        let saved = {
            let mut favs = self.favorites.borrow_mut();
            if let Some(i) = favs.iter().position(|s| s.uuid == station.uuid) {
                favs.remove(i);
                false
            } else {
                favs.insert(0, station.clone());
                true
            }
        };
        let snapshot = self.favorites.borrow().clone();
        if let Err(err) = radio::save_favorites(&snapshot) {
            self.status.set_text(&format!(
                "Could not save favorites: {}",
                one_line(&format!("{err:#}"))
            ));
        }
        self.refresh_fav_chip();
        saved
    }

    fn set_server_stations(&self, stations: Vec<RadioStation>) {
        *self.server.borrow_mut() = stations
            .iter()
            .filter_map(Station::from_server)
            .collect();
        if matches!(*self.mode.borrow(), RadioMode::Server) {
            self.start_query(false);
        }
    }

    fn start_query(&self, append: bool) {
        let gen = self.generation.get().wrapping_add(1);
        self.generation.set(gen);
        match self.mode.borrow().clone() {
            RadioMode::Favorites => {
                let stations = self.favorites.borrow().clone();
                let n = stations.len();
                let msg = if n == 0 {
                    "No saved stations yet. Search, then press Save.".to_string()
                } else {
                    format!(
                        "{n} saved station{}",
                        if n == 1 { "" } else { "s" }
                    )
                };
                self.replace_stations(stations, &msg);
                self.more.set_visible(false);
            }
            RadioMode::Server => {
                let stations = self.server.borrow().clone();
                let n = stations.len();
                let msg = if n == 0 {
                    "This server has no internet radio stations.".to_string()
                } else {
                    format!(
                        "{n} station{} on this server",
                        if n == 1 { "" } else { "s" }
                    )
                };
                self.replace_stations(stations, &msg);
                self.more.set_visible(false);
            }
            mode => self.fetch(gen, mode, append),
        }
    }

    fn fetch(&self, gen: u64, mode: RadioMode, append: bool) {
        self.status
            .set_text("Looking up stations on radio-browser.info…");
        if !append {
            self.more.set_visible(false);
        }
        let query = match &mode {
            RadioMode::Tag(tag) => radio::Query {
                tag: tag.clone(),
                offset: self.offset.get(),
                limit: radio::PAGE,
                ..Default::default()
            },
            RadioMode::Text { name, country } => radio::Query {
                name: name.clone(),
                country: country.clone(),
                offset: self.offset.get(),
                limit: radio::PAGE,
                ..Default::default()
            },
            _ => radio::Query {
                offset: self.offset.get(),
                limit: radio::PAGE,
                ..Default::default()
            },
        };
        let pending = self.pending.clone();
        std::thread::spawn(move || {
            let result = radio::search(&query).map_err(|err| format!("{err:#}"));
            let mut slot = pending.lock();
            let keep = slot.as_ref().map(|existing| gen >= existing.gen).unwrap_or(true);
            if keep {
                *slot = Some(RadioIncoming {
                    gen,
                    append,
                    result,
                });
            }
        });
    }

    fn apply_network(&self, incoming: RadioIncoming) {
        let mode = self.mode.borrow().clone();
        match incoming.result {
            Ok(stations) => {
                let full = stations.len() as u32 >= radio::PAGE;
                if incoming.append {
                    if stations.is_empty() {
                        self.status.set_text("No more stations.");
                        self.more.set_visible(false);
                    } else {
                        self.append_stations(&stations);
                        let total = self.shown.borrow().len();
                        self.status
                            .set_text(&format!("{total} stations from radio-browser.info"));
                        self.more.set_visible(full);
                    }
                } else if stations.is_empty() {
                    self.replace_stations(Vec::new(), "No stations matched.");
                    self.more.set_visible(false);
                } else {
                    let msg = describe_stations(&mode, stations.len());
                    self.replace_stations(stations, &msg);
                    self.more.set_visible(full);
                }
            }
            Err(err) => {
                self.status.set_text(&format!(
                    "Radio browser unavailable: {}",
                    one_line(&err)
                ));
                self.more.set_visible(false);
            }
        }
    }

    fn replace_stations(&self, stations: Vec<Station>, status: &str) {
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        self.add_rows(&stations);
        *self.shown.borrow_mut() = stations;
        self.status.set_text(status);
        self.mark_playing();
    }

    fn append_stations(&self, stations: &[Station]) {
        let fresh: Vec<Station> = {
            let mut shown = self.shown.borrow_mut();
            let fresh: Vec<Station> = stations
                .iter()
                .filter(|s| !shown.iter().any(|have| have.uuid == s.uuid))
                .cloned()
                .collect();
            shown.extend(fresh.iter().cloned());
            fresh
        };
        self.add_rows(&fresh);
        self.mark_playing();
    }

    fn add_rows(&self, stations: &[Station]) {
        for station in stations {
            let row = ListBoxRow::new();
            row.add_css_class("nm-lib-row");
            let line = GtkBox::new(Orientation::Horizontal, 10);
            line.set_margin_top(6);
            line.set_margin_bottom(6);
            line.set_margin_start(8);
            line.set_margin_end(8);
            let cover = gtk4::Image::new();
            cover.add_css_class("nm-lib-cover");
            cover.set_pixel_size(48);
            cover.set_size_request(48, 48);
            cover.set_icon_name(Some("media-optical-symbolic"));
            bind_cover(&cover, station.to_track().cover_url.as_deref());
            let text = GtkBox::new(Orientation::Vertical, 2);
            text.set_hexpand(true);
            let name = Label::new(Some(&station.name));
            name.add_css_class("nm-lib-label");
            name.set_halign(Align::Start);
            name.set_hexpand(true);
            name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            let sub = Label::new(Some(&station.subtitle()));
            sub.add_css_class("nm-muted");
            sub.set_halign(Align::Start);
            sub.set_hexpand(true);
            sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            text.append(&name);
            text.append(&sub);
            let saved = self.is_saved(station);
            let save = Button::with_label(if saved { "Saved" } else { "Save" });
            save.add_css_class("nm-save");
            if saved {
                save.add_css_class("saved");
            }
            save.set_valign(Align::Center);
            save.set_tooltip_text(Some("Save this station to favorites"));
            let page = self.clone();
            let stn = station.clone();
            save.connect_clicked(move |btn| {
                let now_saved = page.toggle_saved(&stn);
                btn.set_label(if now_saved { "Saved" } else { "Save" });
                if now_saved {
                    btn.add_css_class("saved");
                } else {
                    btn.remove_css_class("saved");
                }
                if matches!(*page.mode.borrow(), RadioMode::Favorites) {
                    let page = page.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(1), move || {
                        page.start_query(false);
                    });
                }
            });
            if let Some(home) = station.homepage.as_deref().filter(|h| !h.is_empty()) {
                row.set_tooltip_text(Some(home));
            }
            line.append(&cover);
            line.append(&text);
            line.append(&save);
            row.set_child(Some(&line));
            self.list.append(&row);
        }
    }

    fn mark_playing(&self) {
        let current = self.state.player.current();
        let current_id = current.as_ref().map(|t| t.id.clone());
        let shown = self.shown.borrow();
        let mut i = 0;
        while let Some(row) = self.list.row_at_index(i) {
            let on = current_id.as_ref().is_some_and(|id| {
                shown
                    .get(i as usize)
                    .is_some_and(|station| station.track_id() == *id)
            });
            if on {
                row.add_css_class("nm-station-on");
            } else {
                row.remove_css_class("nm-station-on");
            }
            i += 1;
        }
        drop(shown);
        match current {
            Some(track) if is_radio_track(&track) => {
                if self.now.text().as_str() != track.title {
                    self.now.set_text(&track.title);
                }
            }
            _ => {
                if self.now.text().as_str() != "Not tuned" {
                    self.now.set_text("Not tuned");
                }
            }
        }
    }

    fn tune_at(&self, index: usize) {
        let shown = self.shown.borrow().clone();
        let Some(station) = shown.get(index).cloned() else {
            self.status.set_text("No stations to tune.");
            return;
        };
        let tracks: Vec<Track> = shown.iter().map(Station::to_track).collect();
        self.state.player.set_queue(tracks, index);
        radio_fail_count().store(0, std::sync::atomic::Ordering::Relaxed);
        self.now.set_text(&station.name);
        play_track(&self.state, station.to_track());
        self.mark_playing();
        if let Some(row) = self.list.row_at_index(index as i32) {
            let _ = row.grab_focus();
        }
    }

    fn tune_step(&self, delta: i32) {
        let shown = self.shown.borrow().clone();
        if shown.is_empty() {
            self.status.set_text("No stations to tune.");
            return;
        }
        let current = self.state.player.current().map(|t| t.id);
        let idx = current.and_then(|id| shown.iter().position(|s| s.track_id() == id));
        let len = shown.len() as i32;
        let next = match idx {
            Some(i) => (i as i32 + delta).rem_euclid(len) as usize,
            None => {
                if delta < 0 {
                    shown.len() - 1
                } else {
                    0
                }
            }
        };
        drop(shown);
        self.tune_at(next);
    }
}

fn describe_stations(mode: &RadioMode, n: usize) -> String {
    match mode {
        RadioMode::Tag(tag) => format!("{n} {tag} stations from radio-browser.info"),
        RadioMode::Text { name, country } => {
            let what = match (name.is_empty(), country.is_empty()) {
                (false, false) => format!("{name} in {country}"),
                (false, true) => name.clone(),
                (true, false) => country.clone(),
                _ => "stations".into(),
            };
            format!("{n} matches for {what}")
        }
        _ => format!("{n} top stations from radio-browser.info"),
    }
}

fn one_line(err: &str) -> String {
    let line = err.lines().next().unwrap_or(err);
    if line.chars().count() > 140 {
        let mut out: String = line.chars().take(139).collect();
        out.push('…');
        out
    } else {
        line.to_string()
    }
}

fn apply_text_query(page: &RadioPage, name: &str, country: &str) {
    let name = name.trim().to_string();
    let country = country.trim().to_string();
    if name.is_empty() && country.is_empty() {
        *page.mode.borrow_mut() = RadioMode::Top;
        page.set_active_chip("top");
    } else {
        *page.mode.borrow_mut() = RadioMode::Text { name, country };
        page.set_active_chip("");
    }
    page.offset.set(0);
    page.start_query(false);
}

fn build_radio_page(state: Rc<AppState>) -> RadioPage {
    let root = GtkBox::new(Orientation::Vertical, 8);
    root.add_css_class("nm-main");
    root.set_margin_top(8);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_margin_bottom(8);

    let heading = Label::new(Some("Radio stations"));
    heading.add_css_class("nm-heading");
    heading.set_halign(Align::Start);
    let blurb = Label::new(Some(
        "Discover stations on radio-browser.info. Previous and Next tune this list. Save keeps a station on this machine.",
    ));
    blurb.add_css_class("nm-muted");
    blurb.set_halign(Align::Start);
    blurb.set_wrap(true);
    blurb.set_xalign(0.0);

    let tune = GtkBox::new(Orientation::Horizontal, 8);
    tune.add_css_class("nm-tune");
    let prev = Button::with_label("Previous");
    prev.add_css_class("nm-chip");
    prev.set_tooltip_text(Some("Previous station"));
    prev.set_widget_name("nm-radio-prev");
    let now = Label::new(Some("Not tuned"));
    now.add_css_class("nm-card-title");
    now.set_widget_name("nm-radio-now");
    now.set_hexpand(true);
    now.set_halign(Align::Center);
    now.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let next = Button::with_label("Next");
    next.add_css_class("nm-chip");
    next.set_tooltip_text(Some("Next station"));
    next.set_widget_name("nm-radio-next");
    tune.append(&prev);
    tune.append(&now);
    tune.append(&next);

    let search_row = GtkBox::new(Orientation::Horizontal, 8);
    let name_e = Entry::new();
    name_e.add_css_class("nm-search");
    name_e.set_placeholder_text(Some("Find a station"));
    name_e.set_hexpand(true);
    name_e.set_widget_name("nm-radio-search");
    let country_e = Entry::new();
    country_e.add_css_class("nm-search");
    country_e.set_placeholder_text(Some("Country"));
    country_e.set_width_chars(16);
    country_e.set_widget_name("nm-radio-country");
    let search_btn = Button::with_label("Search");
    search_btn.add_css_class("nm-chip");
    search_btn.set_widget_name("nm-radio-search-btn");
    search_row.append(&name_e);
    search_row.append(&country_e);
    search_row.append(&search_btn);

    let chips_box = GtkBox::new(Orientation::Horizontal, 6);
    let chips_scroll = ScrolledWindow::builder().child(&chips_box).build();
    chips_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    chips_scroll.set_min_content_height(48);

    let status = Label::new(Some("Looking up stations…"));
    status.add_css_class("nm-muted");
    status.set_halign(Align::Start);
    status.set_wrap(true);
    status.set_xalign(0.0);
    status.set_widget_name("nm-radio-status");

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_activate_on_single_click(true);
    list.add_css_class("nm-library-list");
    list.set_widget_name("nm-radio-list");
    let list_scroll = ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hexpand(true)
        .build();

    let more = Button::with_label("More stations");
    more.add_css_class("nm-chip");
    more.set_halign(Align::Start);
    more.set_visible(false);
    more.set_widget_name("nm-radio-more");

    root.append(&heading);
    root.append(&blurb);
    root.append(&tune);
    root.append(&search_row);
    root.append(&chips_scroll);
    root.append(&status);
    root.append(&list_scroll);
    root.append(&more);

    let page = RadioPage {
        state,
        root,
        list: list.clone(),
        status: status.clone(),
        now: now.clone(),
        more: more.clone(),
        favorites: Rc::new(RefCell::new(radio::load_favorites())),
        server: Rc::new(RefCell::new(Vec::new())),
        shown: Rc::new(RefCell::new(Vec::new())),
        mode: Rc::new(RefCell::new(RadioMode::Top)),
        offset: Rc::new(Cell::new(0)),
        generation: Rc::new(Cell::new(0)),
        chips: Rc::new(RefCell::new(Vec::new())),
        pending: Arc::new(parking_lot::Mutex::new(None)),
    };

    let ignore = Rc::new(Cell::new(false));
    // Bump this instead of removing a GLib source. timeout_add_local_once
    // drops the source when it runs, and SourceId::remove then panics.
    let search_gen = Rc::new(Cell::new(0u64));
    let mut specs = vec![
        ("top".to_string(), "Top".to_string(), RadioMode::Top),
        (
            "favorites".to_string(),
            "Favorites".to_string(),
            RadioMode::Favorites,
        ),
        (
            "server".to_string(),
            "This server".to_string(),
            RadioMode::Server,
        ),
    ];
    for (label, tag) in [
        ("Jazz", "jazz"),
        ("Classical", "classical"),
        ("Rock", "rock"),
        ("Pop", "pop"),
        ("Electronic", "electronic"),
        ("News", "news"),
        ("Talk", "talk"),
        ("Ambient", "ambient"),
        ("Blues", "blues"),
        ("Metal", "metal"),
    ] {
        specs.push((
            format!("tag:{tag}"),
            label.to_string(),
            RadioMode::Tag(tag.to_string()),
        ));
    }
    for (key, label, mode) in specs {
        let button = Button::with_label(&label);
        button.add_css_class("nm-chip");
        let page_b = page.clone();
        let ignore_b = ignore.clone();
        let search_gen_b = search_gen.clone();
        let name_b = name_e.clone();
        let country_b = country_e.clone();
        let chip_key = key.clone();
        button.connect_clicked(move |_| {
            search_gen_b.set(search_gen_b.get().wrapping_add(1));
            ignore_b.set(true);
            name_b.set_text("");
            country_b.set_text("");
            ignore_b.set(false);
            *page_b.mode.borrow_mut() = mode.clone();
            page_b.offset.set(0);
            page_b.set_active_chip(&chip_key);
            page_b.start_query(false);
        });
        chips_box.append(&button);
        page.chips.borrow_mut().push((key, button));
    }

    let apply_entries: Rc<dyn Fn()> = Rc::new({
        let page = page.clone();
        let name_e = name_e.clone();
        let country_e = country_e.clone();
        move || apply_text_query(&page, &name_e.text(), &country_e.text())
    });
    let schedule: Rc<dyn Fn()> = Rc::new({
        let apply_entries = Rc::clone(&apply_entries);
        let search_gen = search_gen.clone();
        let ignore = ignore.clone();
        move || {
            if ignore.get() {
                return;
            }
            let gen = search_gen.get().wrapping_add(1);
            search_gen.set(gen);
            let apply_entries = Rc::clone(&apply_entries);
            let search_gen = search_gen.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                if search_gen.get() == gen {
                    apply_entries();
                }
            });
        }
    });
    {
        let schedule = Rc::clone(&schedule);
        name_e.connect_changed(move |_| schedule());
    }
    {
        let schedule = Rc::clone(&schedule);
        country_e.connect_changed(move |_| schedule());
    }
    {
        let apply_entries = Rc::clone(&apply_entries);
        let search_gen = search_gen.clone();
        search_btn.connect_clicked(move |_| {
            search_gen.set(search_gen.get().wrapping_add(1));
            apply_entries();
        });
    }
    {
        let apply_entries = Rc::clone(&apply_entries);
        let search_gen = search_gen.clone();
        name_e.connect_activate(move |_| {
            search_gen.set(search_gen.get().wrapping_add(1));
            apply_entries();
        });
    }
    {
        let apply_entries = Rc::clone(&apply_entries);
        let search_gen = search_gen.clone();
        country_e.connect_activate(move |_| {
            search_gen.set(search_gen.get().wrapping_add(1));
            apply_entries();
        });
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
        more.connect_clicked(move |_| {
            page.offset.set(page.shown.borrow().len() as u32);
            page.start_query(true);
        });
    }
    {
        let page = page.clone();
        list.connect_row_activated(move |_, row| {
            let idx = row.index();
            if idx >= 0 {
                page.tune_at(idx as usize);
            }
        });
    }
    {
        let page = page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            if let Some(incoming) = page.pending.lock().take() {
                if incoming.gen == page.generation.get() {
                    page.apply_network(incoming);
                }
            }
            page.mark_playing();
            glib::ControlFlow::Continue
        });
    }

    page.set_active_chip("top");
    page.refresh_fav_chip();
    page.start_query(false);
    page
}

fn labeled_scroller(title: &str) -> (ScrolledWindow, GtkBox) {
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.add_css_class("nm-main");
    box_.set_margin_top(8);
    box_.set_margin_start(8);
    box_.set_margin_end(8);
    let h = Label::new(Some(title));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    box_.append(&h);
    let scroll = ScrolledWindow::builder().child(&box_).build();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    (scroll, box_)
}

fn server_caption(server: &ServerTarget) -> String {
    let name = server.name.trim();
    let name = if name.is_empty() {
        server.base_url.as_str()
    } else {
        name
    };
    format!("{name} · {}", server.kind.label())
}

fn show_connected_server(root: &impl IsA<gtk4::Widget>, server: &ServerTarget) {
    let root = root.upcast_ref::<gtk4::Widget>();
    let caption = server_caption(server);
    fn walk(widget: &gtk4::Widget, caption: &str) -> bool {
        if widget.widget_name() == "nm-server-name" {
            if let Ok(label) = widget.clone().downcast::<Label>() {
                if label.text().as_str() != caption {
                    label.set_text(caption);
                }
            }
            return true;
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if walk(&next, caption) {
                return true;
            }
            child = next.next_sibling();
        }
        false
    }
    walk(root, &caption);
    if let Some(win) = root.clone().downcast_ref::<ApplicationWindow>() {
        let title = format!("Neighbourhood Music — {}", server.name);
        if win.title().as_deref() != Some(title.as_str()) {
            win.set_title(Some(&title));
        }
    }
}

fn build_sidebar_nav(state: Rc<AppState>, content: Stack, root_stack: Stack) -> GtkBox {
    let side = GtkBox::new(Orientation::Vertical, 4);
    side.add_css_class("nm-sidebar");
    side.set_vexpand(true);

    let brand = Label::new(Some("Neighbourhood Music"));
    brand.add_css_class("nm-brand");
    side.append(&brand);

    let server_lbl = Label::new(Some("No server"));
    server_lbl.add_css_class("nm-muted");
    server_lbl.set_widget_name("nm-server-name");
    server_lbl.set_halign(Align::Start);
    server_lbl.set_hexpand(true);
    server_lbl.set_margin_start(12);
    server_lbl.set_margin_end(12);
    server_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    server_lbl.set_max_width_chars(28);
    if let Some(s) = state.server.read().as_ref() {
        server_lbl.set_text(&server_caption(s));
    }
    side.append(&server_lbl);

    let nav_buttons: Rc<RefCell<Vec<Button>>> = Rc::new(RefCell::new(Vec::new()));
    let set_active = {
        let nav_buttons = nav_buttons.clone();
        Rc::new(move |active: &Button| {
            for b in nav_buttons.borrow().iter() {
                b.remove_css_class("active");
            }
            active.add_css_class("active");
        }) as Rc<dyn Fn(&Button)>
    };

    let nav = |name: &'static str, label: &str| {
        let b = Button::with_label(label);
        b.add_css_class("nm-nav-btn");
        let content = content.clone();
        let set_active = set_active.clone();
        let btn = b.clone();
        b.connect_clicked(move |_| {
            content.set_visible_child_name(name);
            set_active(&btn);
        });
        nav_buttons.borrow_mut().push(b.clone());
        b
    };
    // Slim primary destinations
    side.append(&nav("home", "Home"));
    side.append(&nav("search", "Search"));
    side.append(&nav("library", "Your Library"));
    side.append(&nav("podcasts", "Podcasts"));
    side.append(&Separator::new(Orientation::Horizontal));
    let discover = Label::new(Some("Discover"));
    discover.add_css_class("nm-muted");
    discover.set_halign(Align::Start);
    discover.set_margin_start(12);
    side.append(&discover);
    side.append(&nav("radio", "Radio"));
    side.append(&Separator::new(Orientation::Horizontal));
    let listen = Label::new(Some("Listen"));
    listen.add_css_class("nm-muted");
    listen.set_halign(Align::Start);
    listen.set_margin_start(12);
    side.append(&listen);
    side.append(&nav("queue", "Queue"));
    side.append(&nav("stage", "Stage"));
    side.append(&nav("acquire", "Acquisitions"));
    side.append(&Separator::new(Orientation::Horizontal));
    side.append(&nav("tools", "Tools"));

    side.append(&Separator::new(Orientation::Horizontal));
    let switch = Button::with_label("Switch server");
    switch.add_css_class("nm-nav-btn");
    switch.connect_clicked(move |_| {
        root_stack.set_visible_child_name("picker");
    });
    side.append(&switch);

    let refresh = Button::with_label("Warm web artwork cache");
    refresh.add_css_class("nm-nav-btn");
    let st = state.clone();
    refresh.connect_clicked(move |_| {
        if let Some(be) = st.backend.read().clone() {
            std::thread::spawn(move || {
                if let Ok(shelves) = be.home_shelves() {
                    for shelf in shelves {
                        for a in shelf.albums {
                            let _ = crate::artwork::resolve_cover(&a.artist, &a.name);
                        }
                        for t in shelf.tracks {
                            let _ = crate::artwork::resolve_cover(&t.artist, &t.album);
                        }
                    }
                }
            });
        }
    });
    side.append(&refresh);

    if let Some(first) = nav_buttons.borrow().first() {
        first.add_css_class("active");
    }
    side
}

fn build_search(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Search"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    let entry = Entry::new();
    entry.add_css_class("nm-search");
    entry.set_placeholder_text(Some("What do you want to listen to?"));
    entry.set_widget_name("nm-search-entry");
    page.append(&entry);
    let results = GtkBox::new(Orientation::Vertical, 6);
    let scroll = ScrolledWindow::builder().child(&results).vexpand(true).build();
    page.append(&scroll);

    let debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
    let pending: Arc<parking_lot::Mutex<Option<(String, SearchResults)>>> =
        Arc::new(parking_lot::Mutex::new(None));
    {
        let pending = pending.clone();
        let st = state.clone();
        let results = results.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some((q, sr)) = pending.lock().take() {
                while let Some(c) = results.first_child() {
                    results.remove(&c);
                }
                let _ = q;
                append_section(&results, "Songs");
                for t in sr.tracks.iter().take(40) {
                    results.append(&track_row(&st, t.clone()));
                }
                append_section(&results, "Albums");
                for a in sr.albums.iter().take(20) {
                    results.append(&album_card(&st, a.clone()));
                }
                append_section(&results, "Artists");
                for a in sr.artists.iter().take(20) {
                    let btn = Button::with_label(&a.name);
                    btn.add_css_class("nm-nav-btn");
                    btn.set_halign(Align::Start);
                    let st2 = st.clone();
                    let artist = a.clone();
                    btn.connect_clicked(move |_| {
                        if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
                            if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                                stack.set_visible_child_name("library");
                            }
                            let be = if artist.id.starts_with("ytm:") {
                                st2.ytm.read().clone()
                            } else {
                                st2.backend.read().clone()
                            };
                            let id = artist.id.clone();
                            let name = artist.name.clone();
                            let ui_open = ui.clone();
                            fetch_then(
                                &ui,
                                move || {
                                    be.and_then(|b| b.artist_albums(&id).ok())
                                        .unwrap_or_default()
                                },
                                {
                                    let st3 = st2.clone();
                                    let artist_meta = artist.clone();
                                    move |albums: Vec<Album>| {
                                        ui_open.stack.borrow_mut().push(NavFrame::ArtistAlbums {
                                            artist: artist_meta,
                                            albums: albums.clone(),
                                        });
                                        show_album_list(&st3, &ui_open, &name, albums);
                                    }
                                },
                            );
                        }
                    });
                    results.append(&btn);
                }
                if !sr.playlists.is_empty() {
                    append_section(&results, "Playlists");
                    for p in sr.playlists.iter().take(20) {
                        let btn = Button::with_label(&p.name);
                        btn.add_css_class("nm-nav-btn");
                        let st2 = st.clone();
                        let pl = p.clone();
                        btn.connect_clicked(move |_| {
                            if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
                                open_playlist(&st2, &ui, pl.clone());
                            }
                        });
                        results.append(&btn);
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }
    entry.connect_changed(move |e| {
        let q = e.text().to_string();
        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }
        let st = state.clone();
        let pending = pending.clone();
        let results = results.clone();
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(320), move || {
            while let Some(c) = results.first_child() {
                results.remove(&c);
            }
            if q.trim().len() < 2 {
                return;
            }
            results.append(&Label::new(Some("Searching…")));
            let be = st.backend.read().clone();
            let yt = st.ytm.read().clone();
            let slot = pending.clone();
            let query = q.clone();
            std::thread::spawn(move || {
                let mut sr = be
                    .as_ref()
                    .and_then(|b| b.search(&query).ok())
                    .unwrap_or_default();
                if let Some(yt) = yt {
                    if let Ok(ys) = yt.search(&query) {
                        sr = crate::unify::merge_search(sr, ys);
                    }
                }
                *slot.lock() = Some((query, sr));
            });
        });
        *debounce.borrow_mut() = Some(id);
    });
    page
}

fn build_eq(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Equalizer"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    let row = GtkBox::new(Orientation::Horizontal, 8);
    let bands = ["60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k"];
    for (i, name) in bands.iter().enumerate() {
        let col = GtkBox::new(Orientation::Vertical, 4);
        let scale = Scale::with_range(Orientation::Vertical, -24.0, 12.0, 0.5);
        scale.set_inverted(true);
        scale.set_value(0.0);
        scale.set_vexpand(true);
        scale.set_height_request(180);
        let st = state.clone();
        scale.connect_value_changed(move |s| {
            st.player.set_eq_band(i, s.value());
        });
        col.append(&scale);
        col.append(&Label::new(Some(name)));
        row.append(&col);
    }
    page.append(&row);
    page
}

fn build_player_bar(
    state: Rc<AppState>,
    content: Stack,
    browse_body: Paned,
    outer: GtkBox,
) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 16);
    bar.add_css_class("nm-player");
    bar.set_widget_name("nm-player-bar");

    let cover = gtk4::Image::new();
    cover.set_pixel_size(48);
    cover.set_icon_name(Some("media-optical-symbolic"));
    cover.set_widget_name("nm-now-cover");
    bar.append(&cover);

    let meta = GtkBox::new(Orientation::Vertical, 2);
    meta.set_size_request(220, -1);
    meta.set_widget_name("nm-now-meta");
    let title = Label::new(Some("Not playing"));
    title.add_css_class("nm-card-title");
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(28);
    let artist = Label::new(Some(""));
    artist.add_css_class("nm-muted");
    artist.set_halign(Align::Start);
    artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist.set_max_width_chars(36);
    meta.append(&title);
    meta.append(&artist);
    bar.append(&meta);
    {
        let content = content.clone();
        let st = state.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| {
            st.stage_open.set(true);
            content.set_visible_child_name("stage");
        });
        meta.add_controller(gesture);
    }

    let transport = GtkBox::new(Orientation::Vertical, 4);
    transport.set_hexpand(true);
    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(Align::Center);
    let shuffle = Button::with_label("Shuffle");
    shuffle.add_css_class("nm-icon");
    let prev = Button::with_label("⏮");
    prev.add_css_class("nm-icon");
    let play = Button::with_label("▶");
    play.add_css_class("nm-transport");
    let next = Button::with_label("⏭");
    next.add_css_class("nm-icon");
    let repeat = Button::with_label("Repeat");
    repeat.add_css_class("nm-icon");
    buttons.append(&shuffle);
    buttons.append(&prev);
    buttons.append(&play);
    buttons.append(&next);
    buttons.append(&repeat);
    transport.append(&buttons);
    // Slim bar: scrubber lives in Stage / Sound flyout (Q8).
    bar.append(&transport);

    let right = GtkBox::new(Orientation::Horizontal, 8);
    right.set_halign(Align::End);
    let stage_btn = Button::with_label("Stage");
    stage_btn.add_css_class("nm-icon");
    let sound_btn = Button::with_label("Sound");
    sound_btn.add_css_class("nm-icon");
    let queue_btn = Button::with_label("Queue");
    queue_btn.add_css_class("nm-icon");
    let cast_btn = Button::with_label("Cast");
    cast_btn.add_css_class("nm-icon");
    let mini_btn = Button::with_label("Mini");
    mini_btn.add_css_class("nm-icon");
    right.append(&stage_btn);
    right.append(&sound_btn);
    right.append(&queue_btn);
    right.append(&cast_btn);
    right.append(&mini_btn);
    bar.append(&right);

    {
        let content = content.clone();
        let st = state.clone();
        stage_btn.connect_clicked(move |_| {
            st.stage_open.set(true);
            content.set_visible_child_name("stage");
        });
    }
    {
        let st = state.clone();
        sound_btn.connect_clicked(move |btn| {
            show_sound_flyout(&st, btn);
        });
    }
    {
        let st = state.clone();
        let browse_body = browse_body.clone();
        let outer = outer.clone();
        mini_btn.connect_clicked(move |_| {
            let mini = !st.mini_player.get();
            st.mini_player.set(mini);
            browse_body.set_visible(!mini);
            // Keep player bar; hide chrome extras via class.
            if mini {
                outer.add_css_class("nm-mini");
            } else {
                outer.remove_css_class("nm-mini");
            }
        });
    }

    // Actions
    {
        let st = state.clone();
        play.connect_clicked(move |b| {
            let playing = st.player.toggle().unwrap_or(false);
            b.set_label(if playing { "⏸" } else { "▶" });
        });
    }
    {
        let st = state.clone();
        next.connect_clicked(move |_| {
            let current = st.player.current();
            let radio = current.as_ref().is_some_and(is_radio_track);
            let podcast = current.as_ref().is_some_and(crate::podcasts::is_podcast_track);
            if radio {
                radio_fail_count().store(0, std::sync::atomic::Ordering::Relaxed);
            }
            let next = if podcast {
                st.player.step_clamped(1)
            } else if radio {
                st.player.next_in_order()
            } else {
                st.player.advance()
            };
            if let Some(t) = next {
                play_track(&st, t);
            }
        });
    }
    {
        let st = state.clone();
        prev.connect_clicked(move |_| {
            let current = st.player.current();
            let radio = current.as_ref().is_some_and(is_radio_track);
            let podcast = current.as_ref().is_some_and(crate::podcasts::is_podcast_track);
            if radio {
                radio_fail_count().store(0, std::sync::atomic::Ordering::Relaxed);
            }
            let prev = if podcast {
                st.player.step_clamped(-1)
            } else if radio {
                st.player.prev_in_order()
            } else {
                st.player.previous_track()
            };
            if let Some(t) = prev {
                play_track(&st, t);
            }
        });
    }
    {
        let st = state.clone();
        shuffle.connect_clicked(move |b| {
            let on = !st.player.shuffle();
            st.player.set_shuffle(on);
            b.set_label(if on { "Shuffle ●" } else { "Shuffle" });
        });
    }
    {
        let st = state.clone();
        repeat.connect_clicked(move |b| {
            let mode = st.player.repeat().next();
            st.player.set_repeat(mode);
            b.set_label(mode.label());
        });
    }
    {
        let content = content.clone();
        queue_btn.connect_clicked(move |_| content.set_visible_child_name("queue"));
    }
    {
        let st = state.clone();
        cast_btn.connect_clicked(move |btn| show_cast_picker(&st, btn));
    }

    let st = state.clone();
    let title_l = title.clone();
    let artist_l = artist.clone();
    let cover_i = cover.clone();
    let play_b = play.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if let Some(t) = st.now.read().clone() {
            title_l.set_text(&t.title);
            artist_l.set_text(&format!("{} — {}", t.artist, t.album));
            bind_cover(&cover_i, t.cover_url.as_deref());
        }
        play_b.set_label(if st.player.is_playing() { "⏸" } else { "▶" });
        glib::ControlFlow::Continue
    });

    bar
}

#[derive(Clone)]
struct UiSnapshot {
    home: Vec<HomeShelf>,
    radio: Vec<HomeShelf>,
    mfy: Vec<HomeShelf>,
    playlists: Vec<Playlist>,
    artists: Vec<Artist>,
    albums: Vec<Album>,
    podcasts: Vec<Podcast>,
    stations: Vec<RadioStation>,
    error: Option<String>,
}

fn collect_ui_snapshot(
    be: Arc<dyn MediaBackend>,
    ytm: Option<Arc<dyn MediaBackend>>,
    server_id: String,
) -> UiSnapshot {
    let mut home = be.home_shelves().unwrap_or_else(|e| {
        tracing::warn!("home_shelves: {e:#}");
        vec![]
    });
    let radio = be.radio_shelves().unwrap_or_default();
    let mfy = be.made_for_you().unwrap_or_default();
    if let Some(yt) = ytm.as_ref() {
        if let Ok(mut shelves) = yt.home_shelves() {
            for s in &mut shelves {
                s.title = format!("{} · YT", s.title);
            }
            home.extend(shelves);
        }
        if let Ok(mut moods) = yt.made_for_you() {
            for s in &mut moods {
                s.title = format!("{} · YT", s.title);
            }
            home.extend(moods);
        }
    }
    let playlists = playlists::sync_all(&server_id, be.clone()).unwrap_or_else(|_| {
        be.playlists().unwrap_or_default()
    });
    let artists = be.artists().unwrap_or_default();
    let mut albums = be.albums().unwrap_or_default();
    if let Some(yt) = ytm.as_ref() {
        albums = crate::unify::merge_albums(albums, yt.albums().unwrap_or_default());
    }
    let podcasts = be.podcasts().unwrap_or_default();
    let stations = be.radio_stations().unwrap_or_default();

    let enrich_shelf = |shelves: Vec<HomeShelf>| -> Vec<HomeShelf> {
        shelves
            .into_iter()
            .map(|mut shelf| {
                for a in shelf.albums.iter_mut().take(8) {
                    a.cover_url =
                        crate::artwork::prefer_web_cover(&a.artist, &a.name, a.cover_url.take());
                    if let Some(ref u) = a.cover_url {
                        let _ = crate::artwork::fetch_image_file(u);
                    }
                }
                for t in shelf.tracks.iter_mut().take(8) {
                    t.cover_url =
                        crate::artwork::prefer_web_cover(&t.artist, &t.album, t.cover_url.take());
                    if let Some(ref u) = t.cover_url {
                        let _ = crate::artwork::fetch_image_file(u);
                    }
                }
                shelf
            })
            .collect()
    };

    let error = if home.is_empty() && artists.is_empty() && albums.is_empty() {
        Some("Library returned no shelves — check server music library.".into())
    } else {
        None
    };

    UiSnapshot {
        home: enrich_shelf(home),
        radio: enrich_shelf(radio),
        mfy: enrich_shelf(mfy),
        playlists,
        artists,
        albums,
        podcasts,
        stations,
        error,
    }
}

fn apply_ui_snapshot(
    state: &Rc<AppState>,
    snap: UiSnapshot,
    home: &GtkBox,
    library: &LibraryUi,
    podcast_page: &crate::ui::podcast_page::PodcastPage,
    queue: &GtkBox,
) {
    clear_after_heading(home);
    library
        .nav_gen
        .set(library.nav_gen.get().wrapping_add(1));
    *library.snapshot.borrow_mut() = Some(snap.clone());
    render_library_home(state, library, &snap);

    // Continue listening from history
    if let Some(srv) = state.server.read().as_ref() {
        if let Ok(recent) = state.history.recent(&srv.id, 12) {
            if !recent.is_empty() {
                append_section(home, "Continue listening");
                let row = GtkBox::new(Orientation::Horizontal, 8);
                for (id, title, artist, _) in recent {
                    let l = Label::new(Some(&format!("{title} — {artist}")));
                    l.add_css_class("nm-card-sub");
                    let btn = Button::new();
                    btn.add_css_class("nm-card");
                    btn.set_child(Some(&l));
                    let st = state.clone();
                    btn.connect_clicked(move |_| {
                        let t = Track {
                            id: id.clone(),
                            title: title.clone(),
                            artist: artist.clone(),
                            stream_url: String::new(),
                            ..Default::default()
                        };
                        st.player.set_queue(vec![t.clone()], 0);
                        play_track(&st, t);
                    });
                    row.append(&btn);
                }
                home.append(&row);
            }
        }
    }

    if let Some(err) = &snap.error {
        let l = Label::new(Some(err));
        l.add_css_class("nm-muted");
        l.set_halign(Align::Start);
        home.append(&l);
    }

    for shelf in &snap.home {
        fill_shelf(state, home, shelf);
    }
    for shelf in snap.mfy.iter().chain(snap.radio.iter()) {
        fill_shelf(state, home, shelf);
    }

    podcast_page.set_server_shows(snap.podcasts);
    fill_queue(state, queue);
}

fn clear_after_heading(box_: &GtkBox) {
    // Keep first child (heading), remove the rest
    let mut child = box_.first_child().and_then(|w| w.next_sibling());
    while let Some(w) = child {
        let next = w.next_sibling();
        box_.remove(&w);
        child = next;
    }
}

fn append_section(parent: &GtkBox, title: &str) {
    let l = Label::new(Some(title));
    l.add_css_class("nm-section-title");
    l.set_halign(Align::Start);
    parent.append(&l);
}

fn fill_shelf(state: &Rc<AppState>, parent: &GtkBox, shelf: &HomeShelf) {
    append_section(parent, &shelf.title);
    let row = GtkBox::new(Orientation::Horizontal, 10);
    for a in shelf.albums.iter().take(12) {
        row.append(&album_card(state, a.clone()));
    }
    for t in shelf.tracks.iter().take(12) {
        row.append(&track_card(state, t.clone()));
    }
    for p in shelf.playlists.iter().take(12) {
        let btn = Button::new();
        btn.add_css_class("nm-card");
        let col = GtkBox::new(Orientation::Vertical, 4);
        col.append(&cover_image(p.cover_url.as_deref()));
        let title = Label::new(Some(&p.name));
        title.add_css_class("nm-card-title");
        col.append(&title);
        btn.set_child(Some(&col));
        let st = state.clone();
        let pl = p.clone();
        btn.connect_clicked(move |_| {
            if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
                if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                    stack.set_visible_child_name("library");
                }
                open_playlist(&st, &ui, pl.clone());
            }
        });
        row.append(&btn);
    }
    if shelf.albums.is_empty() && shelf.tracks.is_empty() && shelf.playlists.is_empty() {
        row.append(&Label::new(Some("(empty)")));
    }
    let scroll = ScrolledWindow::builder()
        .child(&row)
        .hexpand(true)
        .min_content_height(200)
        .build();
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    parent.append(&scroll);
}

fn cover_image(url: Option<&str>) -> gtk4::Image {
    let image = gtk4::Image::new();
    image.set_pixel_size(140);
    image.set_icon_name(Some("media-optical-symbolic"));
    bind_cover(&image, url);
    image
}

fn album_card(state: &Rc<AppState>, album: Album) -> Button {
    let btn = Button::new();
    btn.add_css_class("nm-card");
    let col = GtkBox::new(Orientation::Vertical, 4);
    col.append(&cover_image(album.cover_url.as_deref()));
    let title = Label::new(Some(&album.name));
    title.add_css_class("nm-card-title");
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    let sub = Label::new(Some(&album.artist));
    sub.add_css_class("nm-card-sub");
    sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    sub.set_max_width_chars(18);
    if album.source == SourceKind::YoutubeMusic || album.id.starts_with("ytm:") {
        let badge = Label::new(Some("YT"));
        badge.add_css_class("nm-badge");
        col.append(&badge);
    }
    col.append(&title);
    col.append(&sub);
    btn.set_child(Some(&col));
    let st = state.clone();
    btn.connect_clicked(move |_| {
        // Open album detail — never autoplay from discovery cards.
        if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
            stack.set_visible_child_name("library");
        }
        if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
            open_album(&st, &ui, album.clone());
        }
    });
    btn
}

fn track_card(state: &Rc<AppState>, track: Track) -> Button {
    let btn = Button::new();
    btn.add_css_class("nm-card");
    let col = GtkBox::new(Orientation::Vertical, 4);
    col.append(&cover_image(track.cover_url.as_deref()));
    let title = Label::new(Some(&track.title));
    title.add_css_class("nm-card-title");
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    let sub = Label::new(Some(&track.artist));
    sub.add_css_class("nm-card-sub");
    sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    sub.set_max_width_chars(18);
    col.append(&title);
    col.append(&sub);
    btn.set_child(Some(&col));
    let st = state.clone();
    btn.connect_clicked(move |_| {
        st.player.set_queue(vec![track.clone()], 0);
        play_track(&st, track.clone());
    });
    btn
}

fn track_row(state: &Rc<AppState>, track: Track) -> GtkBox {
    track_row_indexed(state, track.clone(), vec![track], 0)
}

fn track_row_indexed(state: &Rc<AppState>, track: Track, album_tracks: Vec<Track>, index: usize) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.add_css_class("nm-track-row");
    let num = track.track_num.unwrap_or((index + 1) as u32);
    let dur = track
        .duration_secs
        .map(|s| format!("{}:{:02}", s / 60, s % 60))
        .unwrap_or_else(|| "--:--".into());
    let fmt = track
        .format
        .clone()
        .or_else(|| track.content_type.clone())
        .or_else(|| track.bit_rate.map(|b| format!("{b} kbps")))
        .unwrap_or_else(|| "—".into());
    let label = Label::new(Some(&format!(
        "{num:<4} {} — {}    {}    {dur}    {fmt}",
        track.title, track.artist, track.album
    )));
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let play = Button::with_label("Play");
    play.add_css_class("nm-icon");
    let like = Button::with_label(if state.taste.is_liked(&track.id) || track.liked {
        "♥"
    } else {
        "♡"
    });
    like.add_css_class("nm-icon");
    let next = Button::with_label("Next");
    next.add_css_class("nm-icon");
    let more = Button::with_label("⋯");
    more.add_css_class("nm-icon");

    let play_from = {
        let st = state.clone();
        let queue = album_tracks.clone();
        let idx = index;
        Rc::new(move || {
            if let Some(t) = queue.get(idx) {
                st.player.set_queue(queue.clone(), idx);
                play_track(&st, t.clone());
            }
        }) as Rc<dyn Fn()>
    };
    {
        let play_from = play_from.clone();
        play.connect_clicked(move |_| play_from());
    }
    {
        let play_from = play_from.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| play_from());
        label.add_controller(gesture);
    }
    {
        let st = state.clone();
        let t = track.clone();
        like.connect_clicked(move |b| {
            let _ = st.taste.like(&t);
            if let Some(be) = st.backend.read().as_ref() {
                let _ = be.set_favorite(&t.id, true);
            }
            b.set_label("♥");
            if ytm::is_ytm_track(&t) {
                let uploader = st.backend.read().clone().map(|be| {
                    Arc::new(BackendUploader { backend: be }) as Arc<dyn LibraryUploader>
                });
                acquire::spawn_like_acquire(st.acquire.clone(), t.clone(), uploader);
            }
        });
    }
    {
        let st2 = state.clone();
        let t = track.clone();
        next.connect_clicked(move |_| st2.player.add_next(t.clone()));
    }
    {
        let st = state.clone();
        let t = track.clone();
        let queue = album_tracks.clone();
        let idx = index;
        more.connect_clicked(move |btn| {
            let pop = Popover::new();
            pop.set_parent(btn);
            let box_ = GtkBox::new(Orientation::Vertical, 4);
            let add_btn = |label: &str, action: Box<dyn Fn()>| {
                let b = Button::with_label(label);
                b.add_css_class("nm-nav-btn");
                b.connect_clicked(move |_| action());
                box_.append(&b);
            };
            let st_p = st.clone();
            let q = queue.clone();
            let i = idx;
            add_btn(
                "Play",
                Box::new(move || {
                    if let Some(t) = q.get(i) {
                        st_p.player.set_queue(q.clone(), i);
                        play_track(&st_p, t.clone());
                    }
                }),
            );
            let st_n = st.clone();
            let t_n = t.clone();
            add_btn("Play Next", Box::new(move || st_n.player.add_next(t_n.clone())));
            let st_q = st.clone();
            let t_q = t.clone();
            add_btn("Add to Queue", Box::new(move || st_q.player.enqueue(t_q.clone())));
            let st_l = st.clone();
            let t_l = t.clone();
            add_btn(
                "Like",
                Box::new(move || {
                    let _ = st_l.taste.like(&t_l);
                    if ytm::is_ytm_track(&t_l) {
                        let uploader = st_l.backend.read().clone().map(|be| {
                            Arc::new(BackendUploader { backend: be }) as Arc<dyn LibraryUploader>
                        });
                        acquire::spawn_like_acquire(st_l.acquire.clone(), t_l.clone(), uploader);
                    }
                }),
            );
            pop.set_child(Some(&box_));
            pop.popup();
        });
    }

    row.append(&label);
    row.append(&like);
    row.append(&play);
    row.append(&next);
    row.append(&more);
    row
}

fn fill_queue(state: &AppState, box_: &GtkBox) {
    clear_after_heading(box_);
    let queue = state.player.queue();
    let idx = state.player.index();
    for (i, t) in queue.iter().enumerate() {
        let mark = if i == idx { "▶ " } else { "" };
        let btn = Button::with_label(&format!("{mark}{} — {}", t.title, t.artist));
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Start);
        let player = state.player.clone();
        let track = t.clone();
        let all = queue.clone();
        // Capture stream via play_uri using resolved URL already on track when possible.
        let uri = track.stream_url.clone();
        btn.connect_clicked(move |_| {
            player.set_queue(all.clone(), i);
            if !uri.is_empty() {
                let _ = player.play_uri(&uri);
            }
        });
        box_.append(&btn);
    }
}

fn fill_lyrics(state: &AppState, box_: &GtkBox, track: &Track) {
    clear_after_heading(box_);
    let text = state
        .backend
        .read()
        .as_ref()
        .and_then(|b| b.lyrics(&track.id).ok())
        .flatten()
        .or_else(|| {
            state
                .ytm
                .read()
                .as_ref()
                .and_then(|b| b.lyrics(&track.id).ok())
                .flatten()
        })
        .unwrap_or_else(|| "No lyrics available.".into());
    let l = Label::new(Some(&text));
    l.set_wrap(true);
    l.set_halign(Align::Start);
    box_.append(&l);
}

fn build_window_chrome(win: ApplicationWindow, state: Rc<AppState>) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("nm-titlebar");
    let title = Label::new(Some("Neighbourhood Music"));
    title.add_css_class("nm-titlebar-title");
    title.set_halign(Align::Start);
    title.set_hexpand(true);
    let min = Button::with_label("—");
    min.add_css_class("nm-icon");
    let max = Button::with_label("□");
    max.add_css_class("nm-icon");
    let close = Button::with_label("✕");
    close.add_css_class("nm-close");
    let win_m = win.clone();
    min.connect_clicked(move |_| win_m.minimize());
    let win_x = win.clone();
    max.connect_clicked(move |_| {
        if win_x.is_maximized() {
            win_x.unmaximize();
        } else {
            win_x.maximize();
        }
    });
    let win_c = win.clone();
    close.connect_clicked(move |_| win_c.close());
    let mini = Button::with_label("Mini player");
    mini.add_css_class("nm-nav-btn");
    let st = state.clone();
    let win_mini = win.clone();
    mini.connect_clicked(move |_| {
        let on = !st.mini_player.get();
        st.mini_player.set(on);
        if on {
            win_mini.set_default_size(420, 96);
        } else {
            win_mini.set_default_size(1280, 800);
        }
    });
    bar.append(&title);
    bar.append(&mini);
    bar.append(&min);
    bar.append(&max);
    bar.append(&close);
    bar
}

fn build_repair_banner() -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("nm-repair-banner");
    bar.set_visible(false);
    bar.set_widget_name("nm-repair-banner");
    let label = Label::new(Some(""));
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    let cancel = Button::with_label("Cancel");
    cancel.add_css_class("nm-nav-btn");
    cancel.connect_clicked(|_| ytm::repair::request_cancel());
    let repair = Button::with_label("Repair now");
    repair.add_css_class("nm-transport");
    repair.connect_clicked(|_| ytm::repair::spawn_auto_repair());
    bar.append(&label);
    bar.append(&repair);
    bar.append(&cancel);
    let label_t = label.clone();
    let bar_t = bar.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let status = ytm::repair::current_status();
        let active = ytm::repair::is_repairing()
            || matches!(
                status.phase,
                ytm::repair::RepairPhase::Failed(_) | ytm::repair::RepairPhase::RolledBack
            );
        bar_t.set_visible(active || !status.message.is_empty());
        if !status.message.is_empty() {
            label_t.set_text(&format!("YouTube Music: {}", status.message));
        } else if ytm::repair::is_repairing() {
            label_t.set_text("Repairing YouTube Music access…");
        }
        glib::ControlFlow::Continue
    });
    bar
}

fn build_stage(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    page.add_css_class("nm-stage");
    let tabs = GtkBox::new(Orientation::Horizontal, 8);
    let stack = Stack::new();
    stack.set_vexpand(true);
    let art = GtkBox::new(Orientation::Vertical, 8);
    let cover = gtk4::Image::new();
    cover.set_pixel_size(320);
    cover.set_halign(Align::Center);
    cover.set_widget_name("nm-stage-cover");
    art.append(&cover);
    let seek = Scale::with_range(Orientation::Horizontal, 0.0, 1000.0, 1.0);
    seek.set_draw_value(false);
    let times = Label::new(Some("0:00 / 0:00"));
    times.add_css_class("nm-muted");
    art.append(&seek);
    art.append(&times);
    let lyrics_box = GtkBox::new(Orientation::Vertical, 4);
    let queue_box = GtkBox::new(Orientation::Vertical, 4);
    let credits = Label::new(Some("Credits appear when metadata is available."));
    credits.set_wrap(true);
    stack.add_named(&art, Some("art"));
    stack.add_named(&lyrics_box, Some("lyrics"));
    stack.add_named(&queue_box, Some("queue"));
    stack.add_named(&credits, Some("credits"));
    stack.set_visible_child_name("art");
    for (name, label) in [
        ("art", "Artwork"),
        ("lyrics", "Lyrics"),
        ("queue", "Queue"),
        ("credits", "Credits"),
    ] {
        let b = Button::with_label(label);
        b.add_css_class("nm-chip");
        let stack = stack.clone();
        b.connect_clicked(move |_| stack.set_visible_child_name(name));
        tabs.append(&b);
    }
    page.append(&tabs);
    page.append(&stack);
    {
        let st = state.clone();
        seek.connect_change_value(move |_, _, value| {
            let dur = st.player.duration_secs();
            if dur > 0 {
                let secs = ((value / 1000.0) * dur as f64) as u64;
                let _ = st.player.seek_secs(secs);
            }
            glib::Propagation::Proceed
        });
    }
    let st = state.clone();
    let cover_i = cover.clone();
    let seek_s = seek.clone();
    let times_l = times.clone();
    let lyrics_b = lyrics_box.clone();
    let queue_b = queue_box.clone();
    let credits_l = credits.clone();
    let last = Rc::new(RefCell::new(String::new()));
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if let Some(t) = st.now.read().clone() {
            bind_cover(&cover_i, t.cover_url.as_deref());
            if *last.borrow() != t.id {
                *last.borrow_mut() = t.id.clone();
                fill_lyrics(&st, &lyrics_b, &t);
                credits_l.set_text(&format!(
                    "{}\nArtist: {}\nAlbum: {}\nFormat: {}\nBitrate: {}\nSource: {:?}",
                    t.title,
                    t.artist,
                    t.album,
                    t.format.clone().unwrap_or_else(|| "—".into()),
                    t.bit_rate.map(|b| format!("{b} kbps")).unwrap_or_else(|| "—".into()),
                    t.source
                ));
            }
        }
        fill_queue(&st, &queue_b);
        let dur = st.player.duration_secs().max(1) as f64;
        let pos = st.player.position_secs() as f64;
        seek_s.set_value((pos / dur) * 1000.0);
        times_l.set_text(&format!(
            "{}:{:02} / {}:{:02}",
            (pos as u64) / 60,
            (pos as u64) % 60,
            (dur as u64) / 60,
            (dur as u64) % 60
        ));
        glib::ControlFlow::Continue
    });
    page
}

fn build_acquisitions_page(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Acquisitions"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    let list = GtkBox::new(Orientation::Vertical, 4);
    let scroll = ScrolledWindow::builder().child(&list).vexpand(true).build();
    page.append(&scroll);
    let st = state.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(700), move || {
        while let Some(c) = list.first_child() {
            list.remove(&c);
        }
        for job in st.acquire.snapshot() {
            let line = format!(
                "{} — {} [{}] {}",
                job.track.title,
                job.track.artist,
                match &job.state {
                    acquire::JobState::Queued => "queued",
                    acquire::JobState::Downloading => "downloading",
                    acquire::JobState::Tagging => "tagging",
                    acquire::JobState::Uploading => "uploading",
                    acquire::JobState::Done => "done",
                    acquire::JobState::Failed(_) => "failed",
                },
                job.message
            );
            let l = Label::new(Some(&line));
            l.set_halign(Align::Start);
            list.append(&l);
        }
        if st.acquire.snapshot().is_empty() {
            list.append(&Label::new(Some("Like a YouTube Music track to acquire it.")));
        }
        glib::ControlFlow::Continue
    });
    page
}

fn build_unified_radio(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Radio"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    let tabs = GtkBox::new(Orientation::Horizontal, 8);
    let stack = Stack::new();
    stack.set_vexpand(true);
    let for_you = labeled_scroller("For You");
    let stations = build_radio_page(state.clone());
    let tags = labeled_scroller("Tags");
    let favorites = labeled_scroller("Favorites");
    stack.add_named(&for_you.0, Some("foryou"));
    stack.add_named(&stations.root, Some("stations"));
    stack.add_named(&tags.0, Some("tags"));
    stack.add_named(&favorites.0, Some("favorites"));
    stack.set_visible_child_name("foryou");
    for (name, label) in [
        ("foryou", "For You"),
        ("stations", "Stations"),
        ("tags", "Tags"),
        ("favorites", "Favorites"),
    ] {
        let b = Button::with_label(label);
        b.add_css_class("nm-chip");
        let stack = stack.clone();
        b.connect_clicked(move |_| stack.set_visible_child_name(name));
        tabs.append(&b);
    }
    page.append(&tabs);
    page.append(&stack);
    // Populate For You from server radio + YTM moods when available.
    let st = state.clone();
    let box_ = for_you.1.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
        if let Some(be) = st.backend.read().clone() {
            if let Ok(shelves) = be.radio_shelves() {
                for s in shelves {
                    fill_shelf(&st, &box_, &s);
                }
            }
        }
        if let Some(yt) = st.ytm.read().clone() {
            if let Ok(shelves) = yt.radio_shelves() {
                for s in shelves {
                    fill_shelf(&st, &box_, &s);
                }
            }
        }
    });
    page
}

fn build_tools_page(state: Rc<AppState>, content: Stack) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Tools"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    let eq = build_eq(state.clone());
    page.append(&eq);
    let repair = Button::with_label("Repair YouTube Music extractors");
    repair.add_css_class("nm-transport");
    repair.connect_clicked(|_| ytm::repair::spawn_auto_repair());
    page.append(&repair);
    let stage = Button::with_label("Open Stage");
    stage.add_css_class("nm-nav-btn");
    stage.connect_clicked(move |_| content.set_visible_child_name("stage"));
    page.append(&stage);
    page
}

fn show_sound_flyout(state: &Rc<AppState>, anchor: &Button) {
    let pop = Popover::new();
    pop.set_parent(anchor);
    let box_ = GtkBox::new(Orientation::Vertical, 8);
    box_.set_margin_top(8);
    box_.set_margin_bottom(8);
    box_.set_margin_start(12);
    box_.set_margin_end(12);
    let vol = Scale::with_range(Orientation::Horizontal, 0.0, 1.0, 0.01);
    vol.set_value(state.player.volume());
    vol.set_size_request(180, -1);
    let st = state.clone();
    vol.connect_value_changed(move |s| st.player.set_volume(s.value()));
    box_.append(&Label::new(Some("Volume")));
    box_.append(&vol);
    let seek = Scale::with_range(Orientation::Horizontal, 0.0, 1000.0, 1.0);
    seek.set_draw_value(false);
    seek.set_size_request(180, -1);
    let st2 = state.clone();
    seek.connect_change_value(move |_, _, value| {
        let dur = st2.player.duration_secs();
        if dur > 0 {
            let secs = ((value / 1000.0) * dur as f64) as u64;
            let _ = st2.player.seek_secs(secs);
        }
        glib::Propagation::Proceed
    });
    box_.append(&Label::new(Some("Seek")));
    box_.append(&seek);
    let transcode = Button::with_label(state.player.transcode().label());
    transcode.add_css_class("nm-icon");
    let st3 = state.clone();
    transcode.connect_clicked(move |b| {
        let mode = st3.player.transcode().next();
        st3.player.set_transcode(mode);
        b.set_label(mode.label());
    });
    box_.append(&transcode);
    let bands = GtkBox::new(Orientation::Horizontal, 4);
    for i in 0..10 {
        let scale = Scale::with_range(Orientation::Vertical, -24.0, 12.0, 0.5);
        scale.set_inverted(true);
        scale.set_value(state.player.eq_bands()[i]);
        scale.set_height_request(100);
        let st = state.clone();
        scale.connect_value_changed(move |s| st.player.set_eq_band(i, s.value()));
        bands.append(&scale);
    }
    box_.append(&Label::new(Some("EQ")));
    box_.append(&bands);
    pop.set_child(Some(&box_));
    pop.popup();
}

fn show_cast_picker(state: &Rc<AppState>, anchor: &Button) {
    let pop = Popover::new();
    pop.set_parent(anchor);
    let box_ = GtkBox::new(Orientation::Vertical, 4);
    box_.append(&Label::new(Some("Discovering devices…")));
    pop.set_child(Some(&box_));
    pop.popup();
    let devices_slot: Arc<parking_lot::Mutex<Vec<CastDevice>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));
    let slot = devices_slot.clone();
    std::thread::spawn(move || {
        let devices = cast::discover(std::time::Duration::from_millis(1200));
        *slot.lock() = devices;
    });
    let st2 = state.clone();
    let box_fill = box_.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(1300), move || {
        while let Some(c) = box_fill.first_child() {
            box_fill.remove(&c);
        }
        let devices = devices_slot.lock().clone();
        *st2.cast_devices.write() = devices.clone();
        if devices.is_empty() {
            box_fill.append(&Label::new(Some("No cast devices found.")));
            return;
        }
        for dev in devices {
            let btn = Button::with_label(&format!("{} ({})", dev.name, dev.protocol.label()));
            btn.add_css_class("nm-nav-btn");
            let st3 = st2.clone();
            btn.connect_clicked(move |_| {
                let Some(track) = st3.now.read().clone() else {
                    return;
                };
                let url = if ytm::is_ytm_track(&track) {
                    st3.ytm
                        .read()
                        .as_ref()
                        .and_then(|b| b.stream_url(&track, st3.player.transcode()).ok())
                        .unwrap_or(track.stream_url.clone())
                } else {
                    st3.backend
                        .read()
                        .as_ref()
                        .and_then(|b| b.stream_url(&track, st3.player.transcode()).ok())
                        .unwrap_or(track.stream_url.clone())
                };
                if let Err(e) = cast::cast_url(&dev, &url, &track.title) {
                    tracing::warn!("cast: {e:#}");
                }
            });
            box_fill.append(&btn);
        }
    });
}

fn maybe_extend_queue_continuum(state: &AppState) {
    let queue = state.player.queue();
    if queue.is_empty() || !state.player.is_playing() {
        return;
    }
    let idx = state.player.index();
    if idx + 1 < queue.len() {
        return;
    }
    let Some(current) = queue.get(idx).cloned() else {
        return;
    };
    if is_radio_track(&current) || crate::podcasts::is_podcast_track(&current) {
        return;
    }
    let be = if ytm::is_ytm_track(&current) {
        state.ytm.read().clone()
    } else {
        state.backend.read().clone()
    };
    let Some(be) = be else { return };
    let id = current.id.clone();
    let player = state.player.clone();
    std::thread::spawn(move || {
        let extra = be
            .similar_tracks(&id)
            .or_else(|_| be.instant_mix(&id))
            .unwrap_or_default();
        if extra.is_empty() {
            return;
        }
        let mut q = player.queue();
        let existing: HashSet<_> = q.iter().map(|t| t.id.clone()).collect();
        for t in extra.into_iter().take(20) {
            if !existing.contains(&t.id) {
                q.push(t);
            }
        }
        let idx = player.index();
        player.set_queue(q, idx);
    });
}

fn install_shortcuts(state: Rc<AppState>, content: Stack) {
    let controller = gtk4::EventControllerKey::new();
    let st = state.clone();
    let content_keys = content.clone();
    controller.connect_key_pressed(move |_, key, _code, _mods| {
        use gtk4::gdk::Key;
        match key {
            Key::space => {
                let _ = st.player.toggle();
                glib::Propagation::Stop
            }
            Key::n | Key::N => {
                if let Some(t) = st.player.advance() {
                    play_track(&st, t);
                }
                glib::Propagation::Stop
            }
            Key::p | Key::P => {
                if let Some(t) = st.player.previous_track() {
                    play_track(&st, t);
                }
                glib::Propagation::Stop
            }
            Key::l | Key::L => {
                if let Some(t) = st.now.read().clone() {
                    let _ = st.taste.like(&t);
                    if ytm::is_ytm_track(&t) {
                        let uploader = st.backend.read().clone().map(|be| {
                            Arc::new(BackendUploader { backend: be }) as Arc<dyn LibraryUploader>
                        });
                        acquire::spawn_like_acquire(st.acquire.clone(), t, uploader);
                    }
                }
                glib::Propagation::Stop
            }
            Key::s | Key::S => {
                content_keys.set_visible_child_name("stage");
                glib::Propagation::Stop
            }
            Key::slash => {
                content_keys.set_visible_child_name("search");
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    content.add_controller(controller);
}
