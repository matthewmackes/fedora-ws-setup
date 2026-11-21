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
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox,
    ListBoxRow, Orientation, Paned, Scale, ScrolledWindow, Separator, Stack, ToggleButton,
};
use parking_lot::RwLock;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{mpsc, Arc, OnceLock};

const APP_ID: &str = "org.mde.NeighbourhoodMusic";
const CONTROL_PORT: u16 = 3847;
const LAST_SERVER_KEY: &str = "last_server_id";

pub(crate) struct AppState {
    pub(crate) backend: RwLock<Option<Arc<dyn MediaBackend>>>,
    pub(crate) server: RwLock<Option<ServerTarget>>,
    pub(crate) player: Player,
    pub(crate) history: History,
    pub(crate) now: Arc<RwLock<Option<Track>>>,
    pub(crate) cast_devices: RwLock<Vec<CastDevice>>,
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

    let state = Rc::new(AppState {
        backend: RwLock::new(None),
        server: RwLock::new(None),
        player,
        history,
        now,
        cast_devices: RwLock::new(Vec::new()),
    });

    let win = ApplicationWindow::builder()
        .application(app)
        .title("Neighbourhood Music")
        .default_width(1280)
        .default_height(800)
        .build();
    win.add_css_class("nm-window");
    // MDE standard: compositor SSD title bars. No in-window custom chrome.

    let root = GtkBox::new(Orientation::Vertical, 0);
    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);

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

pub(crate) fn play_track(state: &AppState, track: Track) {
    let radio = is_radio_track(&track);
    let podcast = crate::podcasts::is_podcast_track(&track);
    // Stream over HTTP directly — do not pre-download from the media server.
    let play_uri = if radio || podcast {
        crate::urlfix::normalize_http_url(&track.stream_url)
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

    let body = Paned::new(Orientation::Horizontal);
    body.set_hexpand(true);
    body.set_vexpand(true);

    let content_stack = Stack::new();
    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);

    let home = labeled_scroller("Home");
    let search = build_search(state.clone());
    let (library_page, library_ui) = build_library_page();
    let radio = labeled_scroller("Radio");
    let mfy = labeled_scroller("Made For You");
    let podcast_page = crate::ui::podcast_page::build_podcast_page(state.clone());
    let radio_page = build_radio_page(state.clone());
    let queue_view = labeled_scroller("Queue");
    let lyrics = labeled_scroller("Lyrics");
    let eq = build_eq(state.clone());

    content_stack.add_named(&home.0, Some("home"));
    content_stack.add_named(&search, Some("search"));
    content_stack.add_named(&library_page, Some("library"));
    content_stack.add_named(&radio.0, Some("radio"));
    content_stack.add_named(&mfy.0, Some("mfy"));
    content_stack.add_named(podcast_page.widget(), Some("podcasts"));
    content_stack.add_named(&radio_page.root, Some("stations"));
    content_stack.add_named(&queue_view.0, Some("queue"));
    content_stack.add_named(&lyrics.0, Some("lyrics"));
    content_stack.add_named(&eq, Some("eq"));
    content_stack.set_visible_child_name("home");

    // Wire nav buttons stored in sidebar via names — rebuild sidebar with callbacks
    let sidebar = build_sidebar_nav(state.clone(), content_stack.clone(), root_stack.clone());

    body.set_start_child(Some(&sidebar));
    body.set_end_child(Some(&content_stack));
    body.set_resize_start_child(false);
    body.set_shrink_start_child(false);

    let player_bar = build_player_bar(state.clone(), content_stack.clone());

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
            if let Some(snap) = ui.snapshot.borrow().clone() {
                render_library_home(&st, &ui, &snap);
            }
        });
    }

    // Populate views when backend becomes available — fetch off-thread, apply on main.
    // Reloads when the connected server id changes (Switch server).
    let st = state.clone();
    let home_box = home.1.clone();
    let radio_box = radio.1.clone();
    let mfy_box = mfy.1.clone();
    let podcast_page = podcast_page.clone();
    let radio_page = radio_page.clone();
    let queue_box = queue_view.1.clone();
    let lyrics_box = lyrics.1.clone();
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
                apply_ui_snapshot(
                    &st,
                    incoming.snapshot,
                    &home_box,
                    &library_for_load,
                    &radio_box,
                    &mfy_box,
                    &podcast_page,
                    &radio_page,
                    &queue_box,
                    &lyrics_box,
                );
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
            std::thread::spawn(move || {
                let snapshot = collect_ui_snapshot(be, server_id.clone());
                *slot.lock() = Some(Incoming {
                    server_id,
                    snapshot,
                });
            });
        }
        glib::ControlFlow::Continue
    });

    // Periodic queue/lyrics refresh
    let st2 = state.clone();
    let queue_box2 = queue_view.1.clone();
    let lyrics_box2 = lyrics.1.clone();
    let now_title = Rc::new(RefCell::new(String::new()));
    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        fill_queue(&st2, &queue_box2);
        if let Some(t) = st2.now.read().clone() {
            let key = t.id.clone();
            if *now_title.borrow() != key {
                *now_title.borrow_mut() = key;
                fill_lyrics(&st2, &lyrics_box2, &t);
            }
        }
        glib::ControlFlow::Continue
    });

    outer
}

#[derive(Clone)]
struct LibraryUi {
    body: GtkBox,
    title: Label,
    back: Button,
    nav_gen: Rc<Cell<u64>>,
    snapshot: Rc<RefCell<Option<UiSnapshot>>>,
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
        nav_gen: Rc::new(Cell::new(0)),
        snapshot: Rc::new(RefCell::new(None)),
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

fn render_library_home(state: &Rc<AppState>, ui: &LibraryUi, snap: &UiSnapshot) {
    ui.back.set_visible(false);
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
                    move |albums| show_album_list(&state, &ui_open, &name, albums)
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
    ui.back.set_visible(true);
    ui.title.set_text(heading);
    clear_box(&ui.body);
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
    let name = album.name.clone();
    let backend = state.backend.read().clone();
    fetch_then(
        ui,
        move || {
            backend
                .and_then(|be| be.album_tracks(&id).ok())
                .unwrap_or_default()
        },
        {
            let state = state.clone();
            let ui = ui.clone();
            move |tracks| show_track_list(&state, &ui, &name, tracks)
        },
    );
}

fn open_playlist(state: &Rc<AppState>, ui: &LibraryUi, playlist: Playlist) {
    ui.title.set_text("Loading…");
    let id = playlist.id.clone();
    let name = playlist.name.clone();
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
            move |tracks| show_track_list(&state, &ui, &name, tracks)
        },
    );
}

fn show_track_list(state: &Rc<AppState>, ui: &LibraryUi, heading: &str, tracks: Vec<Track>) {
    ui.back.set_visible(true);
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
    for track in tracks {
        ui.body.append(&track_row(state, track));
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

    let nav = |name: &'static str, label: &str| {
        let b = Button::with_label(label);
        b.add_css_class("nm-nav-btn");
        let content = content.clone();
        b.connect_clicked(move |_| content.set_visible_child_name(name));
        b
    };
    side.append(&nav("home", "Home"));
    side.append(&nav("search", "Search"));
    side.append(&nav("library", "Your Library"));
    side.append(&nav("radio", "Radio"));
    side.append(&nav("mfy", "Made For You"));
    side.append(&nav("podcasts", "Podcasts"));
    side.append(&nav("stations", "Radio Stations"));
    side.append(&nav("queue", "Queue"));
    side.append(&nav("lyrics", "Lyrics"));
    side.append(&nav("eq", "Equalizer"));

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
    page.append(&entry);
    let results = GtkBox::new(Orientation::Vertical, 6);
    let scroll = ScrolledWindow::builder().child(&results).vexpand(true).build();
    page.append(&scroll);

    let debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
    entry.connect_changed(move |e| {
        let q = e.text().to_string();
        if let Some(id) = debounce.borrow_mut().take() {
            id.remove();
        }
        let st = state.clone();
        let results = results.clone();
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(320), move || {
            while let Some(c) = results.first_child() {
                results.remove(&c);
            }
            if q.trim().len() < 2 {
                return;
            }
            let Some(be) = st.backend.read().clone() else {
                return;
            };
            match be.search(&q) {
                Ok(sr) => {
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
                        let l = Label::new(Some(&a.name));
                        l.set_halign(Align::Start);
                        results.append(&l);
                    }
                }
                Err(err) => {
                    results.append(&Label::new(Some(&format!("Search error: {err:#}"))));
                }
            }
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

fn build_player_bar(state: Rc<AppState>, content: Stack) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 16);
    bar.add_css_class("nm-player");

    let meta = GtkBox::new(Orientation::Vertical, 2);
    meta.set_size_request(220, -1);
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
    let seek = Scale::with_range(Orientation::Horizontal, 0.0, 1000.0, 1.0);
    seek.set_draw_value(false);
    transport.append(&seek);
    bar.append(&transport);

    let right = GtkBox::new(Orientation::Horizontal, 8);
    right.set_halign(Align::End);
    let transcode = Button::with_label("Auto");
    transcode.add_css_class("nm-icon");
    let queue_btn = Button::with_label("Queue");
    queue_btn.add_css_class("nm-icon");
    let cast_btn = Button::with_label("Cast");
    cast_btn.add_css_class("nm-icon");
    let vol = Scale::with_range(Orientation::Horizontal, 0.0, 1.0, 0.01);
    vol.set_value(1.0);
    vol.set_size_request(100, -1);
    right.append(&transcode);
    right.append(&queue_btn);
    right.append(&cast_btn);
    right.append(&vol);
    bar.append(&right);

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
        let st = state.clone();
        transcode.connect_clicked(move |b| {
            let mode = st.player.transcode().next();
            st.player.set_transcode(mode);
            b.set_label(mode.label());
        });
    }
    {
        let st = state.clone();
        vol.connect_value_changed(move |s| st.player.set_volume(s.value()));
    }
    {
        let content = content.clone();
        queue_btn.connect_clicked(move |_| content.set_visible_child_name("queue"));
    }
    {
        let st = state.clone();
        cast_btn.connect_clicked(move |_| {
            let devices = cast::discover(std::time::Duration::from_millis(900));
            *st.cast_devices.write() = devices.clone();
            let Some(track) = st.now.read().clone() else {
                return;
            };
            if let Some(dev) = devices.first() {
                let url = st
                    .backend
                    .read()
                    .as_ref()
                    .and_then(|b| b.stream_url(&track, st.player.transcode()).ok())
                    .unwrap_or(track.stream_url.clone());
                if let Err(e) = cast::cast_url(dev, &url, &track.title) {
                    tracing::warn!("cast: {e:#}");
                }
            }
        });
    }

    // UI tick for metadata / seek
    let st = state.clone();
    let title_l = title.clone();
    let artist_l = artist.clone();
    let seek_s = seek.clone();
    let play_b = play.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if let Some(t) = st.now.read().clone() {
            title_l.set_text(&t.title);
            artist_l.set_text(&format!("{} — {}", t.artist, t.album));
        }
        let dur = st.player.duration_secs().max(1) as f64;
        let pos = st.player.position_secs() as f64;
        seek_s.set_value((pos / dur) * 1000.0);
        play_b.set_label(if st.player.is_playing() { "⏸" } else { "▶" });
        glib::ControlFlow::Continue
    });

    // Seek on release
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

fn collect_ui_snapshot(be: Arc<dyn MediaBackend>, server_id: String) -> UiSnapshot {
    let home = be.home_shelves().unwrap_or_else(|e| {
        tracing::warn!("home_shelves: {e:#}");
        vec![]
    });
    let radio = be.radio_shelves().unwrap_or_default();
    let mfy = be.made_for_you().unwrap_or_default();
    let playlists = playlists::sync_all(&server_id, be.clone()).unwrap_or_else(|_| {
        be.playlists().unwrap_or_default()
    });
    let artists = be.artists().unwrap_or_default();
    let albums = be.albums().unwrap_or_default();
    let podcasts = be.podcasts().unwrap_or_default();
    let stations = be.radio_stations().unwrap_or_default();

    // Enrich covers from the web in this worker thread (not on GTK main).
    let enrich_shelf = |shelves: Vec<HomeShelf>| -> Vec<HomeShelf> {
        shelves
            .into_iter()
            .map(|mut shelf| {
                // Cap web lookups so first paint stays quick; rest keep server art.
                for a in shelf.albums.iter_mut().take(8) {
                    a.cover_url = crate::artwork::prefer_web_cover(
                        &a.artist,
                        &a.name,
                        a.cover_url.take(),
                    );
                    if let Some(ref u) = a.cover_url {
                        let _ = crate::artwork::fetch_image_file(u);
                    }
                }
                for t in shelf.tracks.iter_mut().take(8) {
                    t.cover_url = crate::artwork::prefer_web_cover(
                        &t.artist,
                        &t.album,
                        t.cover_url.take(),
                    );
                    if let Some(ref u) = t.cover_url {
                        let _ = crate::artwork::fetch_image_file(u);
                    }
                }
                shelf
            })
            .collect()
    };

    let error = if home.is_empty()
        && radio.is_empty()
        && mfy.is_empty()
        && artists.is_empty()
        && albums.is_empty()
    {
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
    radio: &GtkBox,
    mfy: &GtkBox,
    podcast_page: &crate::ui::podcast_page::PodcastPage,
    radio_page: &RadioPage,
    queue: &GtkBox,
    lyrics: &GtkBox,
) {
    clear_after_heading(home);
    clear_after_heading(radio);
    clear_after_heading(mfy);
    library
        .nav_gen
        .set(library.nav_gen.get().wrapping_add(1));
    *library.snapshot.borrow_mut() = Some(snap.clone());
    render_library_home(state, library, &snap);

    if let Some(err) = &snap.error {
        let l = Label::new(Some(err));
        l.add_css_class("nm-muted");
        l.set_halign(Align::Start);
        home.append(&l);
    }

    for shelf in &snap.home {
        fill_shelf(state, home, shelf);
    }
    for shelf in &snap.radio {
        fill_shelf(state, radio, shelf);
    }
    for shelf in &snap.mfy {
        fill_shelf(state, mfy, shelf);
    }

    podcast_page.set_server_shows(snap.podcasts);
    radio_page.set_server_stations(snap.stations);

    fill_queue(state, queue);
    let _ = lyrics;
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
    col.append(&title);
    col.append(&sub);
    btn.set_child(Some(&col));
    let st = state.clone();
    btn.connect_clicked(move |_| {
        if let Some(be) = st.backend.read().clone() {
            if let Ok(tracks) = be.album_tracks(&album.id) {
                if let Some(first) = tracks.first() {
                    st.player.set_queue(tracks.clone(), 0);
                    play_track(&st, first.clone());
                }
            }
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
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.add_css_class("nm-track-row");
    let label = Label::new(Some(&format!("{} — {}", track.title, track.artist)));
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    let play = Button::with_label("Play");
    play.add_css_class("nm-icon");
    let st = state.clone();
    let t = track.clone();
    play.connect_clicked(move |_| {
        st.player.set_queue(vec![t.clone()], 0);
        play_track(&st, t.clone());
    });
    let next = Button::with_label("Next");
    next.add_css_class("nm-icon");
    let st2 = state.clone();
    next.connect_clicked(move |_| st2.player.add_next(track.clone()));
    row.append(&label);
    row.append(&play);
    row.append(&next);
    row
}

fn fill_queue(state: &AppState, box_: &GtkBox) {
    clear_after_heading(box_);
    for (i, t) in state.player.queue().into_iter().enumerate() {
        let mark = if i == state.player.index() { "▶ " } else { "" };
        let l = Label::new(Some(&format!("{mark}{} — {}", t.title, t.artist)));
        l.set_halign(Align::Start);
        box_.append(&l);
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
        .unwrap_or_else(|| "No lyrics available from this server.".into());
    let l = Label::new(Some(&text));
    l.set_wrap(true);
    l.set_halign(Align::Start);
    box_.append(&l);
}
