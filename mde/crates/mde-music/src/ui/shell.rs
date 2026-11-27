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
use gio::File;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox,
    ListBoxRow, Orientation, Paned,     Popover, Revealer, Scale, ScrolledWindow, Separator, Stack, ToggleButton,
};
use parking_lot::{Mutex, RwLock};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, OnceLock};

const APP_ID: &str = "org.mde.NeighbourhoodMusic";
const CONTROL_PORT: u16 = 3847;
const LAST_SERVER_KEY: &str = "last_server_id";

thread_local! {
    static ACQUIRE_NAV: RefCell<Option<Button>> = const { RefCell::new(None) };
}

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

    let (init_w, init_h) = preferred_window_size();
    let win = ApplicationWindow::builder()
        .application(app)
        .title("Neighbourhood Music")
        .default_width(init_w)
        .default_height(init_h)
        .resizable(true)
        .decorated(true)
        .build();
    win.add_css_class("nm-window");
    let chrome = GtkBox::new(Orientation::Horizontal, 4);
    chrome.add_css_class("nm-window-chrome");
    chrome.set_halign(Align::End);
    chrome.set_margin_top(4);
    chrome.set_margin_end(8);
    let min_btn = Button::with_label("–");
    min_btn.add_css_class("nm-icon");
    min_btn.set_tooltip_text(Some("Minimize"));
    let max_btn = Button::with_label("□");
    max_btn.add_css_class("nm-icon");
    max_btn.set_tooltip_text(Some("Maximize"));
    let close_btn = Button::with_label("✕");
    close_btn.add_css_class("nm-icon");
    close_btn.set_tooltip_text(Some("Close"));
    let win_min = win.clone();
    min_btn.connect_clicked(move |_| win_min.minimize());
    let win_max = win.clone();
    max_btn.connect_clicked(move |_| {
        if win_max.is_maximized() {
            win_max.unmaximize();
        } else {
            win_max.maximize();
        }
    });
    let win_close = win.clone();
    close_btn.connect_clicked(move |_| win_close.close());
    chrome.append(&min_btn);
    chrome.append(&max_btn);
    chrome.append(&close_btn);
    // Soft minimum only — never advertise a min taller than the work area (that
    // pinned the player under the Start bar via WM_NORMAL_HINTS).
    win.set_size_request(640, 360);
    // MDE: compositor SSD title bars only (min/max/close). Mini-player lives in the bar.

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&chrome);

    let repair_banner = build_repair_banner();
    root.append(&repair_banner);

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    // Homogeneous stacks size to the *largest* child — library catalog rows
    // (very long album titles) were blowing the whole window out horizontally.
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);
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
    // Size to the usable work area above the MDE taskbar (never full-screen maximize).
    if should_start_maximized() {
        win.maximize();
    }
    win.present();
    APP_WINDOW.with(|slot| *slot.borrow_mut() = Some(win.clone()));
    {
        let win_c = win.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
            reclamp_window_size(&win_c);
        });
        let win_c = win.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            reclamp_window_size(&win_c);
        });
        let win_c = win.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
            reclamp_window_size(&win_c);
        });
        let win_c = win.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(2800), move || {
            reclamp_window_size(&win_c);
        });
    }

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
        } else if ended {
            match crate::player::eos_action(radio_now, podcast_now) {
                crate::player::EosAction::Pause => {
                    let _ = st.player.pause();
                }
                crate::player::EosAction::NextStation => {
                    if let Some(next) = st.player.next_in_order() {
                        play_track(&st, next);
                    }
                }
                crate::player::EosAction::Advance => {
                    if let Some(next) = st.player.advance() {
                        play_track(&st, next);
                    }
                }
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
            if let Some(url) = track.cover_url.as_deref() {
                if let Some(path) = crate::artwork::cached_image_path(url) {
                    apply_cover_accent(&path);
                }
            }
        }
        if st.player.position_secs() > 2 && st.player.is_playing() {
            radio_fail_count().store(0, std::sync::atomic::Ordering::Relaxed);
        }
        for skip in crate::player::drain_skips() {
            let radio_now = st
                .now
                .read()
                .as_ref()
                .is_some_and(|t| is_radio_track(t));
            let next = match skip {
                crate::player::SkipRequest::Next if radio_now => st.player.next_in_order(),
                crate::player::SkipRequest::Next => st.player.advance(),
                crate::player::SkipRequest::Previous => st.player.previous_track(),
            };
            if let Some(next) = next {
                play_track(&st, next);
            }
        }
        for req in crate::player::drain_window_requests() {
            match req {
                crate::player::WindowRequest::Raise => {
                    APP_WINDOW.with(|slot| {
                        if let Some(win) = slot.borrow().as_ref() {
                            win.present();
                        }
                    });
                }
                crate::player::WindowRequest::Quit => {
                    APP_WINDOW.with(|slot| {
                        if let Some(win) = slot.borrow().as_ref() {
                            if let Some(app) = win.application() {
                                app.quit();
                            } else {
                                win.close();
                            }
                        }
                    });
                }
            }
        }
        let tick = progress_tick().fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if tick % 15 == 0 && st.player.is_playing() {
            if let Some(t) = st.now.read().clone() {
                if !is_radio_track(&t) && !crate::podcasts::is_podcast_track(&t) {
                    if let Some(be) = st.backend.read().clone() {
                        let _ = be.report_progress(&t.id, st.player.position_secs(), false);
                    }
                }
            }
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

fn continuum_until() -> &'static std::sync::atomic::AtomicU64 {
    static UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    &UNTIL
}

fn library_reload_flag() -> &'static std::sync::atomic::AtomicBool {
    static FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    &FLAG
}

fn request_library_reload() {
    library_reload_flag().store(true, Ordering::Relaxed);
}

fn progress_tick() -> &'static std::sync::atomic::AtomicU32 {
    static TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    &TICK
}

fn radio_fail_count() -> &'static std::sync::atomic::AtomicU8 {
    static COUNT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
    &COUNT
}

pub(crate) fn source_eq_key(track: &Track) -> &'static str {
    if is_radio_track(track) {
        "eq_radio"
    } else if crate::podcasts::is_podcast_track(track) {
        "eq_podcast"
    } else if ytm::is_ytm_track(track) {
        "eq_ytm"
    } else {
        "eq_library"
    }
}

fn recall_source_eq(state: &AppState, track: &Track) {
    use std::sync::OnceLock;
    static LAST: OnceLock<Mutex<String>> = OnceLock::new();
    let last = LAST.get_or_init(|| Mutex::new(String::new()));
    let key = source_eq_key(track);
    let mut last = last.lock();
    if *last == key {
        return;
    }
    if !last.is_empty() {
        let gains = state.player.eq_bands();
        if let Ok(text) = serde_json::to_string(&gains) {
            let _ = state.history.set_setting(&last, &text);
        }
    }
    let target = state
        .history
        .get_setting(key)
        .and_then(|text| serde_json::from_str::<[f64; 10]>(&text).ok())
        .unwrap_or([0.0; 10]);
    crate::eq::shared_eq().lock().begin_morph(target);
    *last = key.to_string();
}

pub(crate) fn play_track(state: &AppState, mut track: Track) {
    recall_source_eq(state, &track);
    let radio = is_radio_track(&track);
    let podcast = crate::podcasts::is_podcast_track(&track);
    let is_ytm = ytm::is_ytm_track(&track);

    if is_ytm {
        // Resolve off the UI thread — yt-dlp can take several seconds.
        *state.now.write() = Some(track.clone());
        show_player_stage(state);
        let player = state.player.clone();
        let now = state.now.clone();
        let track_id = track.id.clone();
        let vid = ytm::video_id_of(&track).unwrap_or_else(|| track.id.clone());
        std::thread::spawn(move || {
            match ytm::YtmEngine::new().resolve_stream(&vid) {
                Ok(r) => {
                    let uri = if r.url.contains("://") {
                        r.url.clone()
                    } else {
                        crate::urlfix::normalize_http_url(&r.url)
                    };
                    {
                        let mut g = now.write();
                        if let Some(t) = g.as_mut() {
                            if t.id == track_id {
                                t.bit_rate = r.bitrate.or(t.bit_rate);
                                t.format = r.format.clone().or(t.format.clone());
                                t.sample_rate = r.sample_rate.or(t.sample_rate);
                                t.video_url = r.video_url.clone().or(t.video_url.clone());
                                t.stream_url = uri.clone();
                            }
                        }
                    }
                    if !uri.contains("://") {
                        tracing::error!("YTM malformed URI: {uri}");
                        return;
                    }
                    tracing::info!(%uri, android_ua = r.needs_android_ua, "YTM play");
                    let _ = player.play_uri_with_options(&uri, r.needs_android_ua);
                }
                Err(e) => {
                    tracing::warn!(error = %e, video = %vid, "YTM resolve failed");
                    ytm::repair::note_ytm_failure();
                }
            }
        });
        return;
    }

    // Stream over HTTP directly — do not pre-download from the media server.
    let play_uri = if radio || podcast {
        crate::urlfix::normalize_http_url(&track.stream_url)
    } else {
        let mode = state.player.transcode();
        let resolved = state.backend.read().as_ref().and_then(|b| {
            b.stream_url(&track, mode)
                .ok()
                .filter(|u| u.contains("://"))
                .or_else(|| {
                    // History / Continue listening often only has an id.
                    b.stream_url(&track, TranscodeMode::Direct)
                        .ok()
                        .filter(|u| u.contains("://"))
                })
        });
        crate::urlfix::normalize_http_url(
            &resolved.unwrap_or_else(|| track.stream_url.clone()),
        )
    };
    if !play_uri.contains("://") {
        tracing::error!(
            track = %track.id,
            title = %track.title,
            "refusing to play malformed URI: {play_uri}"
        );
        return;
    }
    tracing::debug!("play URI: {play_uri}");

    if radio {
        *state.now.write() = Some(track.clone());
        let _ = state.player.play_uri(&play_uri);
        if let Some(uuid) = track.id.strip_prefix("rb:") {
            crate::radio::note_click(&track.id, uuid);
        }
        show_player_stage(state);
        return;
    }

    if podcast {
        *state.now.write() = Some(track.clone());
        let _ = state.player.play_uri(&play_uri);
        crate::podcasts::arm_resume(&track.id);
        show_player_stage(state);
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
    super::ambient::set_playing(super::ambient::Ctx {
        artist: track.artist.clone(),
        album: track.album.clone(),
        cover_url: track.cover_url.clone(),
    });
    let _ = state.player.play_uri(&play_uri);
    show_player_stage(state);
}

/// Bring the Stage (full player) forward when a track starts — unless Mini mode is on.
fn show_player_stage(state: &AppState) {
    if state.mini_player.get() {
        return;
    }
    state.stage_open.set(true);
    CONTENT_STACK.with(|slot| {
        if let Some(stack) = slot.borrow().as_ref() {
            stack.set_visible_child_name("stage");
        }
    });
}

fn activate_server(
    state: &AppState,
    stack: &Stack,
    server: ServerTarget,
    backend: Arc<dyn MediaBackend>,
) {
    let _ = backend.register_session();
    {
        let sid = server.id.clone();
        let url = server.base_url.clone();
        let be = backend.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::cache::refresh_full(&sid, &url, be) {
                tracing::debug!(error = %e, "library snapshot refresh skipped");
            }
        });
    }
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
                // Don't block startup with a modal — fall back to the picker.
                tracing::warn!(error = %err, "auto-reconnect failed; showing server picker");
                *state.backend.write() = None;
                *state.server.write() = None;
                stack.set_visible_child_name("picker");
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
        .min_content_height(180)
        .min_content_width(420)
        .hexpand(true)
        .child(&list)
        .build();
    page.append(&scroll);

    // Manual add — vertical so the form cannot stretch the window on small screens.
    page.append(&Label::new(Some("Add server manually")));
    let form = GtkBox::new(Orientation::Vertical, 8);
    form.set_hexpand(true);
    let kinds = GtkBox::new(Orientation::Horizontal, 8);
    let kind_jelly = ToggleButton::with_label("Jellyfin");
    let kind_air = ToggleButton::with_label("Airsonic");
    kind_jelly.set_active(true);
    kind_air.set_group(Some(&kind_jelly));
    kinds.append(&kind_jelly);
    kinds.append(&kind_air);
    let url_e = Entry::builder().placeholder_text("http://host:8096").build();
    let user_e = Entry::builder().placeholder_text("username").build();
    let pass_e = Entry::builder().placeholder_text("password / token").build();
    pass_e.set_visibility(false);
    url_e.set_hexpand(true);
    user_e.set_hexpand(true);
    pass_e.set_hexpand(true);
    let add_btn = Button::with_label("Save & Connect");
    form.append(&kinds);
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
    content_stack.set_hhomogeneous(false);
    content_stack.set_vhomogeneous(false);
    content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    content_stack.set_transition_duration(280);

    let home = labeled_scroller("Home · Art & Audio");
    let search = build_search(state.clone());
    let (library_page, library_ui) = build_library_page();
    let radio = build_unified_radio(state.clone());
    let podcast_page = crate::ui::podcast_page::build_podcast_page(state.clone());
    let queue_view = labeled_scroller("Queue");
    let acquire_view = build_acquisitions_page(state.clone());
    let stage = build_stage(state.clone());
    let tools = build_tools_page(state.clone(), content_stack.clone());
    let performer_page = Rc::new(super::performer_page::build_performer_page(
        state.clone(),
        content_stack.clone(),
    ));
    // Wrap tall pages so they scroll inside the work area instead of growing the window.
    let stage_scroll = ScrolledWindow::builder().child(&stage).build();
    stage_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    stage_scroll.set_propagate_natural_height(false);
    stage_scroll.set_vexpand(true);
    let tools_scroll = ScrolledWindow::builder().child(&tools).build();
    tools_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    tools_scroll.set_propagate_natural_height(false);
    tools_scroll.set_vexpand(true);

    let search_scroll = ScrolledWindow::builder().child(&search).build();
    search_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    search_scroll.set_propagate_natural_height(false);
    search_scroll.set_vexpand(true);
    let library_scroll = ScrolledWindow::builder().child(&library_page).build();
    library_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    library_scroll.set_propagate_natural_height(false);
    library_scroll.set_vexpand(true);
    let radio_scroll = ScrolledWindow::builder().child(&radio).build();
    radio_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    radio_scroll.set_propagate_natural_height(false);
    radio_scroll.set_vexpand(true);

    content_stack.add_named(&home.0, Some("home"));
    content_stack.add_named(&search_scroll, Some("search"));
    content_stack.add_named(&library_scroll, Some("library"));
    content_stack.add_named(&radio_scroll, Some("radio"));
    content_stack.add_named(podcast_page.widget(), Some("podcasts"));
    content_stack.add_named(&queue_view.0, Some("queue"));
    content_stack.add_named(&acquire_view, Some("acquire"));
    content_stack.add_named(&stage_scroll, Some("stage"));
    content_stack.add_named(&tools_scroll, Some("tools"));
    content_stack.add_named(performer_page.widget(), Some("performer"));
    content_stack.set_visible_child_name("home");
    LIBRARY_UI.with(|slot| *slot.borrow_mut() = Some(library_ui.clone()));
    CONTENT_STACK.with(|slot| *slot.borrow_mut() = Some(content_stack.clone()));
    PERFORMER_PAGE.with(|slot| *slot.borrow_mut() = Some(performer_page.clone()));

    let sidebar = build_sidebar_nav(state.clone(), content_stack.clone(), root_stack.clone());

    body.set_start_child(Some(&sidebar));
    body.set_end_child(Some(&content_stack));
    body.set_resize_start_child(false);
    body.set_shrink_start_child(false);

    let player_bar = build_player_bar(state.clone(), content_stack.clone(), body.clone(), outer.clone());
    let acquire_badge = Button::with_label("");
    acquire_badge.add_css_class("nm-chip");
    acquire_badge.set_visible(false);
    let content_badge = content_stack.clone();
    acquire_badge.connect_clicked(move |_| {
        content_badge.set_visible_child_name("acquire");
    });
    player_bar.append(&acquire_badge);
    let toast = Revealer::new();
    toast.set_transition_type(gtk4::RevealerTransitionType::SlideUp);
    toast.set_reveal_child(false);
    let toast_label = Label::new(Some(""));
    toast_label.add_css_class("nm-card-title");
    toast_label.set_halign(Align::Start);
    toast_label.set_margin_start(16);
    toast_label.set_margin_end(16);
    toast_label.set_margin_top(8);
    toast_label.set_margin_bottom(8);
    toast.set_child(Some(&toast_label));
    let seen_jobs: Rc<RefCell<HashMap<String, acquire::JobState>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let primed = Rc::new(Cell::new(false));
    let toast_gen = Rc::new(Cell::new(0u32));
    {
        let st = state.clone();
        let badge = acquire_badge.clone();
        let toast = toast.clone();
        let toast_label = toast_label.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(700), move || {
            let jobs = st.acquire.snapshot();
            if !primed.get() {
                let mut seen = seen_jobs.borrow_mut();
                for job in &jobs {
                    seen.insert(job.id.clone(), job.state.clone());
                }
                primed.set(true);
            } else {
                let mut seen = seen_jobs.borrow_mut();
                for job in &jobs {
                    let previous = seen.get(&job.id).cloned();
                    if acquire::became_terminal(previous.as_ref(), &job.state) {
                        toast_label.set_text(&acquire::terminal_message(job));
                        toast.set_reveal_child(true);
                        toast_gen.set(toast_gen.get().wrapping_add(1));
                        let gen = toast_gen.get();
                        let toast_hide = toast.clone();
                        let toast_gen = toast_gen.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_secs(4), move || {
                            if toast_gen.get() == gen {
                                toast_hide.set_reveal_child(false);
                            }
                        });
                    }
                    seen.insert(job.id.clone(), job.state.clone());
                }
            }
            match acquire::badge_text(&jobs) {
                Some(text) => {
                    badge.set_label(&text);
                    badge.set_visible(true);
                    ACQUIRE_NAV.with(|slot| {
                        if let Some(nav) = slot.borrow().as_ref() {
                            nav.set_label(&format!("Acquisitions · {text}"));
                        }
                    });
                }
                None => {
                    badge.set_visible(false);
                    ACQUIRE_NAV.with(|slot| {
                        if let Some(nav) = slot.borrow().as_ref() {
                            nav.set_label("Acquisitions");
                        }
                    });
                }
            }
            glib::ControlFlow::Continue
        });
    }

    // Critical: paned/stack children can report a huge min height (covers, shelves,
    // EQ). Without this scroll shell, GTK advertises that min to the WM and the
    // window cannot shrink above the taskbar.
    let body_scroll = ScrolledWindow::builder().child(&body).build();
    body_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
    body_scroll.set_propagate_natural_width(false);
    body_scroll.set_propagate_natural_height(false);
    body_scroll.set_vexpand(true);
    body_scroll.set_hexpand(true);
    body_scroll.set_widget_name("nm-body-scroll");

    // W676: one full-bleed backdrop behind every page; pages sit on a frosted veil.
    let ambient = super::bleed::Bleed::new(super::bleed::Mode::Ambient);
    super::ambient::install(ambient.clone());
    let bleed_host = gtk4::Overlay::new();
    bleed_host.set_vexpand(true);
    bleed_host.set_hexpand(true);
    bleed_host.add_css_class("nm-bleed-host");
    bleed_host.set_child(Some(ambient.widget()));
    bleed_host.add_overlay(&body_scroll);
    bleed_host.set_clip_overlay(&body_scroll, true);
    {
        let ambient = ambient.clone();
        let adj = body_scroll.vadjustment();
        adj.connect_value_changed(move |a| ambient.set_scroll(a.value()));
    }
    {
        content_stack.connect_visible_child_name_notify(|s| {
            if let Some(name) = s.visible_child_name() {
                super::ambient::page_changed(name.as_str());
            }
        });
    }
    // W683: loudness drives a capped, smoothed backdrop pulse (never flashes).
    glib::timeout_add_local(std::time::Duration::from_millis(66), || {
        let db = crate::eq::snapshot_peak_db();
        let level = ((db + 42.0) / 36.0).clamp(0.0, 1.0);
        super::ambient::set_pulse(level);
        glib::ControlFlow::Continue
    });
    // One-key kill switch for all drift / parallax / pulse (Ctrl+Shift+M).
    {
        let keys = gtk4::EventControllerKey::new();
        keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let off = std::cell::Cell::new(false);
        let space_state = state.clone();
        keys.connect_key_pressed(move |ctrl, key, _, mods| {
            // Spacebar is the universal Play/Pause unless a text field has focus.
            // Capture phase, so a focused button does not swallow it as "click".
            if key == gtk4::gdk::Key::space
                && !mods.intersects(
                    gtk4::gdk::ModifierType::CONTROL_MASK
                        | gtk4::gdk::ModifierType::ALT_MASK
                        | gtk4::gdk::ModifierType::SUPER_MASK,
                )
            {
                let typing = ctrl
                    .widget()
                    .and_then(|w| w.root())
                    .and_then(|r| r.focus())
                    .is_some_and(|w| is_text_input(&w));
                if typing {
                    return glib::Propagation::Proceed;
                }
                let _ = space_state.player.toggle();
                return glib::Propagation::Stop;
            }
            let want = gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::SHIFT_MASK;
            if (key == gtk4::gdk::Key::M || key == gtk4::gdk::Key::m) && mods.contains(want) {
                off.set(!off.get());
                super::bleed::set_global_motion_off(off.get());
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        outer.add_controller(keys);
    }
    outer.append(&bleed_host);
    outer.append(&toast);
    outer.append(&player_bar);
    // Keep a slim breath strip so the bar never visually kisses the Start bar.
    let safe = GtkBox::new(Orientation::Vertical, 0);
    safe.add_css_class("nm-safe-bottom");
    safe.set_height_request(6);
    outer.append(&safe);

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
    let home_box_loading: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
    let loaded_server: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let inflight_server: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        if let Some(root) = status_label.root() {
            if let Some(server) = st.server.read().clone() {
                show_connected_server(&root, &server);
            }
        }
        if library_reload_flag().swap(false, Ordering::Relaxed) {
            *loaded_server.borrow_mut() = String::new();
        }
        if let Some(incoming) = pending_slot.lock().take() {
            if let Some(sk) = home_box_loading.borrow_mut().take() {
                if sk.parent().is_some() {
                    home_box.remove(&sk);
                }
            }
            let current = st
                .server
                .read()
                .as_ref()
                .map(|s| s.id.clone())
                .unwrap_or_default();
            *inflight_server.borrow_mut() = String::new();
            if incoming.server_id == current && !current.is_empty() {
                *loaded_server.borrow_mut() = current;
                if let Some(err) = incoming.snapshot.error.as_ref() {
                    status_label.set_text(err);
                } else {
                    status_label.set_text("Library loaded.");
                }
                apply_ui_snapshot(
                    &st,
                    incoming.snapshot,
                    &home_box,
                    &library_for_load,
                    &podcast_page,
                    &queue_box,
                );
                reclamp_window_size(&status_label);
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
            if !home_box_loading.borrow().is_some() {
                let sk = super::loading::page();
                sk.set_widget_name("home-skeleton");
                home_box.append(&sk);
                *home_box_loading.borrow_mut() = Some(sk);
            }
            clear_box(&library_for_load.body);
            library_for_load.body.append(&super::loading::page());
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
    {
        let st = state.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(25), move || {
            let stepped = {
                let mut eq = crate::eq::shared_eq().lock();
                let stepped = eq.step_morph();
                let gains = eq.gains();
                (stepped, gains)
            };
            if stepped.0 {
                for (i, gain) in stepped.1.iter().enumerate() {
                    st.player.set_eq_band(i, *gain);
                }
            }
            glib::ControlFlow::Continue
        });
    }

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
    body.append(&super::loading::page());

    let scroll = ScrolledWindow::builder().child(&body).build();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll.set_propagate_natural_width(false);
    scroll.set_propagate_natural_height(false);
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

/// Navigate: shows a page skeleton in the body while `work` runs.
fn fetch_then<T, W, U>(ui: &LibraryUi, work: W, then: U)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    U: FnOnce(T) + 'static,
{
    fetch_then_impl(ui, work, then, true);
}

/// Fill part of an already-rendered page: never touches the body, no skeleton.
fn fetch_then_inline<T, W, U>(ui: &LibraryUi, work: W, then: U)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    U: FnOnce(T) + 'static,
{
    fetch_then_impl(ui, work, then, false);
}

fn fetch_then_impl<T, W, U>(ui: &LibraryUi, work: W, then: U, skeleton: bool)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    U: FnOnce(T) + 'static,
{
    let gen = ui.nav_gen.get().wrapping_add(1);
    ui.nav_gen.set(gen);
    let nav = ui.nav_gen.clone();
    // Loading transition: skeleton in the body until the work finishes.
    if skeleton {
        clear_box(&ui.body);
        ui.body.append(&super::loading::page());
    }
    let body_for_fade = ui.body.clone();
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
                        if skeleton {
                            super::loading::fade_in(&body_for_fade);
                        }
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

thread_local! {
    static ACCENT_KEY: RefCell<String> = const { RefCell::new(String::new()) };
    static ACCENT_CSS: RefCell<Option<gtk4::CssProvider>> = const { RefCell::new(None) };
}

fn accent_provider() -> gtk4::CssProvider {
    ACCENT_CSS.with(|slot| {
        let mut guard = slot.borrow_mut();
        if let Some(provider) = guard.clone() {
            return provider;
        }
        let provider = gtk4::CssProvider::new();
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
        *guard = Some(provider.clone());
        provider
    })
}

fn apply_cover_accent(path: &std::path::Path) {
    let key = path.to_string_lossy().to_string();
    if ACCENT_KEY.with(|slot| slot.borrow().as_str() == key) {
        return;
    }
    let Some((r, g, b)) = crate::artwork::accent_from_file(path) else {
        return;
    };
    accent_provider().load_from_string(&crate::artwork::accent_css(r, g, b));
    ACCENT_KEY.with(|slot| *slot.borrow_mut() = key);
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
    pictures: HashMap<String, Vec<gtk4::Picture>>,
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
                pictures: HashMap::new(),
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
                            if let Some(pictures) = hub.pictures.remove(&url) {
                                for picture in pictures {
                                    picture.set_filename(Some(&path));
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

pub(crate) fn bind_picture(picture: &gtk4::Picture, url: Option<&str>) {
    let Some(url) = url.map(str::trim).filter(|url| !url.is_empty()) else {
        return;
    };
    if let Some(path) = crate::artwork::cached_image_path(url) {
        picture.set_filename(Some(path));
        return;
    }
    let hub = cover_hub();
    let mut hub_ref = hub.borrow_mut();
    hub_ref
        .pictures
        .entry(url.to_string())
        .or_default()
        .push(picture.clone());
    if hub_ref.queued.insert(url.to_string()) {
        let _ = cover_jobs().send(url.to_string());
    }
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
        // Ellipsize alone does not cap natural width — long titles still expand parents.
        label.set_max_width_chars(56);
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

fn cover_grid() -> gtk4::FlowBox {
    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(2);
    flow.set_max_children_per_line(6);
    flow.set_column_spacing(12);
    flow.set_row_spacing(12);
    flow.set_hexpand(true);
    flow.add_css_class("nm-cover-grid");
    flow
}

fn filter_cover_grid(flow: &gtk4::FlowBox, query: &str) {
    let mut child = flow.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        let name = widget
            .downcast_ref::<gtk4::FlowBoxChild>()
            .and_then(|row| row.child())
            .map(|inner| inner.widget_name().to_string().to_lowercase())
            .unwrap_or_default();
        widget.set_visible(query.is_empty() || name.contains(query));
    }
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
            show_artist_page(state, ui, artist, albums);
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
                    request_library_reload();
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
        let btn = Button::new();
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Fill);
        let plabel = Label::new(Some(&format!("{} ({count})", playlist.name)));
        plabel.set_halign(Align::Start);
        plabel.set_hexpand(true);
        plabel.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        plabel.set_max_width_chars(48);
        btn.set_child(Some(&plabel));
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
        let flow = cover_grid();
        flow.set_widget_name("library-artists");
        for artist in snap.artists.clone() {
            let card = artist_card(&state, artist);
            flow.append(&card);
        }
        ui.body.append(&flow);
        Some(flow)
    };

    append_section(&ui.body, "Albums");
    let album_list = if snap.albums.is_empty() {
        ui.body.append(&Label::new(Some("No albums yet.")));
        None
    } else {
        let state = state.clone();
        let ui_open = ui.clone();
        let flow = cover_grid();
        flow.set_widget_name("library-albums");
        for album in snap.albums.clone() {
            flow.append(&album_card(&state, album));
        }
        ui.body.append(&flow);
        let _ = ui_open;
        Some(flow)
    };

    if artist_list.is_some() || album_list.is_some() {
        let artists = artist_list.clone();
        let albums = album_list.clone();
        filter.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            if let Some(flow) = &artists {
                filter_cover_grid(flow, &query);
            }
            if let Some(flow) = &albums {
                filter_cover_grid(flow, &query);
            }
        });
    }
}

fn show_artist_page(state: &Rc<AppState>, ui: &LibraryUi, artist: Artist, albums: Vec<Album>) {
    ui.title.set_text(&artist.name);
    clear_box(&ui.body);
    update_crumbs(ui);
    super::ambient::set_context(
        super::bleed::Mode::Sharp,
        super::ambient::Ctx { artist: artist.name.clone(), album: String::new(), cover_url: artist.cover_url.clone() },
    );
    let about = Label::new(Some(&format!(
        "{} album{} · top tracks below, then the discography",
        albums.len(),
        if albums.len() == 1 { "" } else { "s" }
    )));
    about.add_css_class("nm-muted");
    about.set_halign(Align::Start);
    about.set_wrap(true);
    ui.body.append(&about);
    append_section(&ui.body, "Top tracks");
    let tops = GtkBox::new(Orientation::Vertical, 4);
    ui.body.append(&tops);
    append_section(&ui.body, "Albums");
    if albums.is_empty() {
        ui.body.append(&Label::new(Some("No albums.")));
    } else {
        let state_albums = state.clone();
        let ui_open = ui.clone();
        let list = catalog_list(
            albums,
            album_line,
            |album: &Album| album.cover_url.clone(),
            move |album: Album| open_album(&state_albums, &ui_open, album),
        );
        list.set_widget_name("library-artist-albums");
        ui.body.append(&list);
    }
    let id = artist.id.clone();
    let backend = if id.starts_with("ytm:") {
        state.ytm.read().clone()
    } else {
        state.backend.read().clone()
    };
    let st = state.clone();
    let tops_box = tops.clone();
    fetch_then_inline(
        ui,
        move || {
            backend
                .and_then(|be| be.top_tracks(Some(&id)).ok())
                .unwrap_or_default()
        },
        move |tracks: Vec<Track>| {
            if tracks.is_empty() {
                tops_box.append(&Label::new(Some("No top tracks for this artist.")));
            } else {
                show_track_table(&st, &tops_box, &tracks);
            }
        },
    );
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
    static RADIO_PAGE: RefCell<Option<RadioPage>> = const { RefCell::new(None) };
    static APP_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
    static STAGE_STACK: RefCell<Option<Stack>> = const { RefCell::new(None) };
    static PERFORMER_PAGE: RefCell<Option<Rc<super::performer_page::PerformerPage>>> =
        const { RefCell::new(None) };
}

/// Open the performer overview (MusicBrainz facts + Fanart when keyed).
pub(crate) fn open_performer(state: &Rc<AppState>, content: &Stack, artist: Artist) {
    content.set_visible_child_name("performer");
    PERFORMER_PAGE.with(|slot| {
        if let Some(page) = slot.borrow().as_ref() {
            page.open(artist);
        } else {
            tracing::warn!("performer page not mounted");
            let _ = state;
        }
    });
}

pub(crate) fn open_performer_by_name(state: &Rc<AppState>, content: &Stack, name: &str, id: &str) {
    open_performer(
        state,
        content,
        Artist {
            id: id.to_string(),
            name: name.to_string(),
            ..Default::default()
        },
    );
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
                show_playlist_editor(&state, &ui, playlist_meta, tracks);
            }
        },
    );
}

fn write_playlist(state: &Rc<AppState>, ui: &LibraryUi, playlist: &Playlist, ids: &[String]) {
    if let Some(be) = state.backend.read().clone() {
        let _ = be.update_playlist(&playlist.id, &playlist.name, ids);
        request_library_reload();
        open_playlist(state, ui, playlist.clone());
    }
}

fn show_playlist_editor(state: &Rc<AppState>, ui: &LibraryUi, playlist: Playlist, tracks: Vec<Track>) {
    update_crumbs(ui);
    ui.title.set_text(&playlist.name);
    clear_box(&ui.body);
    let actions = GtkBox::new(Orientation::Horizontal, 6);
    let rename = Button::with_label("Rename");
    rename.add_css_class("nm-chip");
    let delete = Button::with_label("Delete");
    delete.add_css_class("nm-chip");
    actions.append(&rename);
    actions.append(&delete);
    ui.body.append(&actions);
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    for (idx, track) in tracks.iter().enumerate() {
        let row = GtkBox::new(Orientation::Horizontal, 4);
        let label = Label::new(Some(&format!("{} — {}", track.title, track.artist)));
        label.set_hexpand(true);
        label.set_halign(Align::Start);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let up = Button::with_label("Up");
        let down = Button::with_label("Down");
        let remove = Button::with_label("Remove");
        for b in [&up, &down, &remove] {
            b.add_css_class("nm-icon");
        }
        {
            let ids = ids.clone();
            let st = state.clone();
            let ui = ui.clone();
            let playlist = playlist.clone();
            up.connect_clicked(move |_| {
                if idx == 0 {
                    return;
                }
                let mut ids = ids.clone();
                ids.swap(idx, idx - 1);
                write_playlist(&st, &ui, &playlist, &ids);
            });
        }
        {
            let ids = ids.clone();
            let st = state.clone();
            let ui = ui.clone();
            let playlist = playlist.clone();
            down.connect_clicked(move |_| {
                let mut ids = ids.clone();
                if idx + 1 >= ids.len() {
                    return;
                }
                ids.swap(idx, idx + 1);
                write_playlist(&st, &ui, &playlist, &ids);
            });
        }
        {
            let ids = ids.clone();
            let st = state.clone();
            let ui = ui.clone();
            let playlist = playlist.clone();
            remove.connect_clicked(move |_| {
                let mut ids = ids.clone();
                if idx < ids.len() {
                    ids.remove(idx);
                }
                write_playlist(&st, &ui, &playlist, &ids);
            });
        }
        row.append(&label);
        row.append(&up);
        row.append(&down);
        row.append(&remove);
        ui.body.append(&row);
    }
    {
        let st = state.clone();
        let ui = ui.clone();
        let playlist = playlist.clone();
        rename.connect_clicked(move |_| {
            let entry = Entry::new();
            entry.set_text(&playlist.name);
            let pop = Popover::new();
            pop.set_parent(&ui.back);
            let save = Button::with_label("Save");
            save.add_css_class("nm-chip");
            let st = st.clone();
            let ui = ui.clone();
            let playlist = playlist.clone();
            let ids = ids.clone();
            let entry_save = entry.clone();
            let pop2 = pop.clone();
            save.connect_clicked(move |_| {
                let name = entry_save.text().to_string();
                if name.trim().is_empty() {
                    return;
                }
                if let Some(be) = st.backend.read().clone() {
                    let _ = be.update_playlist(&playlist.id, name.trim(), &ids);
                    request_library_reload();
                }
                pop2.popdown();
            });
            let col = GtkBox::new(Orientation::Vertical, 4);
            col.append(&entry);
            col.append(&save);
            pop.set_child(Some(&col));
            pop.popup();
        });
    }
    {
        let st = state.clone();
        let playlist = playlist.clone();
        delete.connect_clicked(move |_| {
            if let Some(be) = st.backend.read().clone() {
                let _ = be.delete_playlist(&playlist.id);
                request_library_reload();
            }
        });
    }
}

fn show_immersive_album(state: &Rc<AppState>, ui: &LibraryUi, album: Album, tracks: Vec<Track>) {
    update_crumbs(ui);
    ui.title.set_text(&album.name);
    clear_box(&ui.body);
    ui.body.set_widget_name("library-album-immersive");

    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("nm-album-bleed");
    overlay.set_hexpand(true);
    overlay.set_vexpand(true);
    let backdrop = gtk4::Picture::new();
    backdrop.set_content_fit(gtk4::ContentFit::Cover);
    backdrop.set_can_shrink(true);
    backdrop.set_hexpand(true);
    backdrop.set_vexpand(true);
    backdrop.add_css_class("nm-album-bleed-art");
    // Art comes from the global full-bleed host (HD cover + artist fan art).
    super::ambient::set_context(
        super::bleed::Mode::Sharp,
        super::ambient::Ctx {
            artist: album.artist.clone(),
            album: album.name.clone(),
            cover_url: album.cover_url.clone(),
        },
    );
    overlay.set_child(Some(&backdrop));

    let hero = GtkBox::new(Orientation::Vertical, 12);
    hero.add_css_class("nm-album-hero");
    hero.set_margin_top(148);
    hero.set_margin_start(16);
    hero.set_margin_end(16);
    hero.set_margin_bottom(12);
    hero.set_valign(Align::Fill);
    hero.set_hexpand(true);
    let sub = Label::new(Some(&format!(
        "{}{}",
        album.name,
        album
            .year
            .map(|y| format!(" · {} · {y}", album.artist))
            .unwrap_or_else(|| format!(" · {}", album.artist))
    )));
    sub.add_css_class("nm-hero-title");
    sub.set_halign(Align::Start);
    sub.set_wrap(true);
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
    let overview = Button::with_label("Performer");
    overview.add_css_class("nm-chip");
    {
        let st = state.clone();
        let album_a = album.clone();
        overview.connect_clicked(move |_| {
            if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                open_performer(
                    &st,
                    &stack,
                    Artist {
                        id: album_a
                            .artist_id
                            .clone()
                            .unwrap_or_else(|| format!("name:{}", album_a.artist)),
                        name: album_a.artist.clone(),
                        cover_url: album_a.cover_url.clone(),
                        ..Default::default()
                    },
                );
            }
        });
    }
    actions.append(&play_all);
    actions.append(&shuffle);
    actions.append(&overview);
    hero.append(&actions);

    let frost = GtkBox::new(Orientation::Vertical, 4);
    frost.add_css_class("nm-frost-tracks");
    frost.set_vexpand(true);
    show_track_table(state, &frost, &tracks);
    hero.append(&frost);
    overlay.add_overlay(&hero);
    ui.body.append(&overlay);
}

fn show_liked_songs(state: &Rc<AppState>, ui: &LibraryUi) {
    let tracks = state
        .taste
        .liked_tracks(500)
        .unwrap_or_default()
        .into_iter()
        .map(|(id, title, artist, album)| Track {
            id,
            title,
            artist,
            album,
            source: SourceKind::Local,
            ..Default::default()
        })
        .collect();
    show_track_list(state, ui, "Liked Songs", tracks, None);
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

pub(crate) fn show_track_table(state: &Rc<AppState>, parent: &GtkBox, tracks: &[Track]) {
    let header = Label::new(Some("#  Title — Album    Time    Format    Bitrate    Sample"));
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
    fn select_tag(&self, tag: &str) {
        *self.mode.borrow_mut() = RadioMode::Tag(tag.to_string());
        self.offset.set(0);
        self.set_active_chip(&format!("tag:{tag}"));
        self.start_query(false);
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
                    "This server has no internet radio stations. Open Favorites for stations saved on this machine, or Search to add some.".to_string()
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
    // Automatic horizontal is a safety valve if a child still reports a huge min width.
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
    scroll.set_propagate_natural_width(false);
    scroll.set_propagate_natural_height(false);
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

    let brand = Label::new(Some("Neighbourhood"));
    brand.add_css_class("nm-brand");
    brand.set_halign(Align::Start);
    side.append(&brand);
    let brand_sub = Label::new(Some("MUSIC"));
    brand_sub.add_css_class("nm-brand-sub");
    brand_sub.set_halign(Align::Start);
    brand_sub.set_margin_start(12);
    brand_sub.set_margin_bottom(8);
    side.append(&brand_sub);

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
    side.append(&nav("library", "Library"));
    {
        let liked = Button::with_label("Liked Songs");
        liked.add_css_class("nm-nav-btn");
        let content = content.clone();
        let st = state.clone();
        liked.connect_clicked(move |_| {
            content.set_visible_child_name("library");
            if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
                show_liked_songs(&st, &ui);
            }
        });
        side.append(&liked);
    }
    side.append(&nav("podcasts", "Podcasts"));
    let sep1 = Separator::new(Orientation::Horizontal);
    sep1.add_css_class("nm-nav-sep");
    side.append(&sep1);
    let discover = Label::new(Some("DISCOVER"));
    discover.add_css_class("nm-nav-section");
    discover.set_halign(Align::Start);
    discover.set_margin_start(14);
    discover.set_margin_top(8);
    discover.set_margin_bottom(4);
    side.append(&discover);
    side.append(&nav("radio", "Radio"));
    let sep2 = Separator::new(Orientation::Horizontal);
    sep2.add_css_class("nm-nav-sep");
    side.append(&sep2);
    let listen = Label::new(Some("LISTEN"));
    listen.add_css_class("nm-nav-section");
    listen.set_halign(Align::Start);
    listen.set_margin_start(14);
    listen.set_margin_top(8);
    listen.set_margin_bottom(4);
    side.append(&listen);
    side.append(&nav("queue", "Queue"));
    side.append(&nav("stage", "Stage"));
    let acquire_nav = nav("acquire", "Acquisitions");
    ACQUIRE_NAV.with(|slot| *slot.borrow_mut() = Some(acquire_nav.clone()));
    side.append(&acquire_nav);
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
    let hint = Label::new(Some(
        "Type a query, then press Search (or Enter). Searches your library and YouTube Music together — YT marks YouTube Music results.",
    ));
    hint.add_css_class("nm-muted");
    hint.set_halign(Align::Start);
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    page.append(&hint);

    let row = GtkBox::new(Orientation::Horizontal, 8);
    let entry = Entry::new();
    entry.add_css_class("nm-search");
    entry.set_placeholder_text(Some("Artist, song, or album…"));
    entry.set_hexpand(true);
    entry.set_widget_name("nm-search-entry");
    let go = Button::with_label("Search");
    go.add_css_class("nm-transport");
    go.set_widget_name("nm-search-go");
    let charts = Button::with_label("Charts & moods");
    charts.add_css_class("nm-chip");
    row.append(&entry);
    row.append(&go);
    row.append(&charts);
    page.append(&row);

    let results = GtkBox::new(Orientation::Vertical, 6);
    let empty = Label::new(Some(
        "Enter a search, then click Search. Multi-word queries are fine (e.g. “billie eilish”).",
    ));
    empty.add_css_class("nm-muted");
    empty.set_halign(Align::Start);
    empty.set_wrap(true);
    empty.set_xalign(0.0);
    results.append(&empty);
    let scroll = ScrolledWindow::builder().child(&results).vexpand(true).build();
    page.append(&scroll);

    let pending: Arc<parking_lot::Mutex<Option<(String, SearchResults, String)>>> =
        Arc::new(parking_lot::Mutex::new(None));
    let inflight = Arc::new(AtomicU64::new(0));
    {
        let pending = pending.clone();
        let st = state.clone();
        let results = results.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some((_q, sr, note)) = pending.lock().take() {
                render_search_results(&st, &results, &sr, &note);
            }
            glib::ControlFlow::Continue
        });
    }

    let run = {
        let state = state.clone();
        let entry = entry.clone();
        let results = results.clone();
        let pending = pending.clone();
        let inflight = inflight.clone();
        Rc::new(move || {
            let query = entry.text().trim().to_string();
            while let Some(c) = results.first_child() {
                results.remove(&c);
            }
            if query.len() < 2 {
                let empty = Label::new(Some("Enter at least 2 characters, then press Search."));
                empty.add_css_class("nm-muted");
                empty.set_halign(Align::Start);
                results.append(&empty);
                return;
            }
            results.append(&super::loading::status_row("Searching library + YouTube Music…"));
            results.append(&super::loading::rows(5));

            let gen = inflight.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
            let be = state.backend.read().clone();
            let yt = state.ytm.read().clone();
            let slot = pending.clone();
            let inflight_t = inflight.clone();
            std::thread::spawn(move || {
                let mut note = String::new();
                let mut sr = be
                    .as_ref()
                    .and_then(|b| b.search(&query).ok())
                    .unwrap_or_default();
                match yt {
                    Some(yt) => match yt.search(&query) {
                        Ok(ys) => {
                            let n = ys.tracks.len() + ys.albums.len() + ys.artists.len();
                            sr = crate::unify::merge_search(sr, ys);
                            if n == 0 {
                                note = "YouTube Music returned no hits for this query.".into();
                            }
                        }
                        Err(err) => {
                            note = format!("YouTube Music search failed: {err:#}");
                            tracing::warn!(error = %err, "YTM search failed");
                        }
                    },
                    None => note = "YouTube Music is offline for this session.".into(),
                }
                if inflight_t.load(Ordering::SeqCst) != gen {
                    return;
                }
                *slot.lock() = Some((query, sr, note));
            });
        })
    };
    {
        let run = Rc::clone(&run);
        go.connect_clicked(move |_| run());
    }
    {
        let run = Rc::clone(&run);
        entry.connect_activate(move |_| run());
    }
    {
        let state = state.clone();
        let results = results.clone();
        let pending = pending.clone();
        let inflight = inflight.clone();
        charts.connect_clicked(move |_| {
            while let Some(c) = results.first_child() {
                results.remove(&c);
            }
            results.append(&super::loading::status_row("Loading YouTube Music charts and moods…"));
            results.append(&super::loading::cards(5));
            let gen = inflight.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
            let yt = state.ytm.read().clone();
            let slot = pending.clone();
            let inflight_t = inflight.clone();
            std::thread::spawn(move || {
                let (sr, note) = match yt {
                    None => (
                        SearchResults::default(),
                        "YouTube Music is offline for this session.".into(),
                    ),
                    Some(yt) => match yt.made_for_you() {
                        Ok(shelves) => {
                            let mut sr = SearchResults::default();
                            for shelf in shelves {
                                sr.albums.extend(shelf.albums);
                                sr.tracks.extend(shelf.tracks);
                            }
                            let note = if sr.albums.is_empty() && sr.tracks.is_empty() {
                                "YouTube Music charts and moods came back empty.".into()
                            } else {
                                "YouTube Music charts and moods.".into()
                            };
                            (sr, note)
                        }
                        Err(err) => (
                            SearchResults::default(),
                            format!(
                                "YouTube Music charts and moods failed: {err:#}. Use Repair if this keeps failing."
                            ),
                        ),
                    },
                };
                if inflight_t.load(Ordering::SeqCst) != gen {
                    return;
                }
                *slot.lock() = Some(("charts".into(), sr, note));
            });
        });
    }
    page
}

fn render_search_results(
    st: &Rc<AppState>,
    results: &GtkBox,
    sr: &SearchResults,
    note: &str,
) {
    while let Some(c) = results.first_child() {
        results.remove(&c);
    }
    super::loading::fade_in(results);
    if !note.is_empty() {
        let l = Label::new(Some(note));
        l.add_css_class("nm-muted");
        l.set_halign(Align::Start);
        l.set_wrap(true);
        results.append(&l);
    }
    let ytm_songs = sr
        .tracks
        .iter()
        .filter(|t| t.source == SourceKind::YoutubeMusic || t.id.starts_with("ytm:"))
        .count();
    append_section(
        results,
        &format!(
            "Songs ({} · {} from YouTube Music)",
            sr.tracks.len().min(40),
            ytm_songs.min(40)
        ),
    );
    if sr.tracks.is_empty() {
        results.append(&Label::new(Some("No songs matched.")));
    }
    for t in sr.tracks.iter().take(40) {
        results.append(&track_row(st, t.clone()));
    }
    append_section(results, "Albums");
    for a in sr.albums.iter().take(20) {
        results.append(&album_card(st, a.clone()));
    }
    append_section(results, "Artists");
    for a in sr.artists.iter().take(20) {
        let label = if a.id.starts_with("ytm:") {
            format!("{} · YT", a.name)
        } else {
            a.name.clone()
        };
        let btn = Button::with_label(&label);
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Start);
        let st2 = st.clone();
        let artist = a.clone();
        btn.connect_clicked(move |_| {
            if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                open_performer(&st2, &stack, artist.clone());
                return;
            }
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
                let ui_open = ui.clone();
                fetch_then(
                    &ui,
                    move || be.and_then(|b| b.artist_albums(&id).ok()).unwrap_or_default(),
                    {
                        let st3 = st2.clone();
                        let artist_meta = artist.clone();
                        move |albums: Vec<Album>| {
                            ui_open.stack.borrow_mut().push(NavFrame::ArtistAlbums {
                                artist: artist_meta.clone(),
                                albums: albums.clone(),
                            });
                            show_artist_page(&st3, &ui_open, artist_meta, albums);
                        }
                    },
                );
            }
        });
        results.append(&btn);
    }
    if !sr.playlists.is_empty() {
        append_section(results, "Playlists");
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

fn build_eq(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Equalizer"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    // Same ten-band sliders as the Audiophile tab (ISO octave centres, ±12 dB).
    page.append(&super::audiophile::eq_sliders(&state, 140));
    let row = GtkBox::new(Orientation::Horizontal, 8);
    let bypass = Button::with_label(if state.player.eq_bypass() {
        "EQ bypassed"
    } else {
        "EQ active"
    });
    bypass.add_css_class("nm-chip");
    let st = state.clone();
    bypass.connect_clicked(move |b| {
        let on = !st.player.eq_bypass();
        st.player.set_eq_bypass(on);
        b.set_label(if on { "EQ bypassed" } else { "EQ active" });
    });
    let flat = Button::with_label("Reset flat");
    flat.add_css_class("nm-chip");
    let st = state.clone();
    flat.connect_clicked(move |_| {
        for i in 0..10 {
            st.player.set_eq_band(i, 0.0);
        }
    });
    row.append(&bypass);
    row.append(&flat);
    page.append(&row);
    page
}

fn build_player_bar(
    state: Rc<AppState>,
    content: Stack,
    browse_body: Paned,
    outer: GtkBox,
) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 12);
    bar.add_css_class("nm-player");
    bar.set_widget_name("nm-player-bar");
    bar.set_vexpand(false);

    let cover_wrap = GtkBox::new(Orientation::Vertical, 0);
    cover_wrap.add_css_class("nm-now-cover-frame");
    let cover = gtk4::Image::new();
    cover.add_css_class("nm-cover-art");
    cover.set_pixel_size(52);
    cover.set_icon_name(Some("media-optical-symbolic"));
    cover.set_widget_name("nm-now-cover");
    cover_wrap.append(&cover);
    bar.append(&cover_wrap);

    let meta = GtkBox::new(Orientation::Vertical, 1);
    meta.set_size_request(180, -1);
    meta.set_valign(Align::Center);
    meta.set_widget_name("nm-now-meta");
    let title = Label::new(Some("Not playing"));
    title.add_css_class("nm-card-title");
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(24);
    let artist = Label::new(Some(""));
    artist.add_css_class("nm-muted");
    artist.set_halign(Align::Start);
    artist.set_cursor_from_name(Some("pointer"));
    artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist.set_max_width_chars(28);
    let performer_btn = Button::with_label("Performer");
    performer_btn.add_css_class("nm-chip");
    performer_btn.set_halign(Align::Start);
    performer_btn.set_visible(false);
    meta.append(&title);
    meta.append(&artist);
    meta.append(&performer_btn);
    bar.append(&meta);
    {
        let st = state.clone();
        let content_perf = content.clone();
        performer_btn.connect_clicked(move |_| {
            if let Some(t) = st.now.read().clone() {
                open_performer_by_name(
                    &st,
                    &content_perf,
                    &t.artist,
                    t.artist_id.as_deref().unwrap_or(&t.artist),
                );
            }
        });
        let st_artist = state.clone();
        let content_artist = content.clone();
        let artist_click = gtk4::GestureClick::new();
        artist_click.connect_released(move |_, _, _, _| {
            if let Some(t) = st_artist.now.read().clone() {
                if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
                    content_artist.set_visible_child_name("library");
                    let artist = Artist {
                        id: t.artist_id.clone().unwrap_or_else(|| format!("name:{}", t.artist)),
                        name: t.artist.clone(),
                        cover_url: t.cover_url.clone(),
                        ..Default::default()
                    };
                    show_artist_page(&st_artist, &ui, artist, Vec::new());
                }
            }
        });
        artist.add_controller(artist_click);
    }
    {
        let content_meta = content.clone();
        let st = state.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| {
            st.stage_open.set(true);
            content_meta.set_visible_child_name("stage");
        });
        meta.add_controller(gesture);
        let content_cover = content.clone();
        let st = state.clone();
        let cover_click = gtk4::GestureClick::new();
        cover_click.connect_released(move |_, _, _, _| {
            st.stage_open.set(true);
            content_cover.set_visible_child_name("stage");
        });
        cover_wrap.add_controller(cover_click);
    }

    let transport = GtkBox::new(Orientation::Vertical, 2);
    transport.set_hexpand(true);
    transport.set_valign(Align::Center);
    let buttons = GtkBox::new(Orientation::Horizontal, 4);
    buttons.set_halign(Align::Center);
    let shuffle = Button::with_label("⇄");
    shuffle.add_css_class("nm-icon");
    shuffle.set_tooltip_text(Some("Shuffle"));
    let prev = Button::with_label("⏮");
    prev.add_css_class("nm-icon");
    let play = Button::with_label("▶");
    play.add_css_class("nm-transport");
    play.add_css_class("nm-play");
    let next = Button::with_label("⏭");
    next.add_css_class("nm-icon");
    let repeat = Button::with_label("↻");
    repeat.add_css_class("nm-icon");
    repeat.set_tooltip_text(Some("Repeat"));
    buttons.append(&shuffle);
    buttons.append(&prev);
    buttons.append(&play);
    buttons.append(&next);
    buttons.append(&repeat);
    transport.append(&buttons);
    // Scrubber lives on Stage and in the Sound flyout so the bar stays cover + title + transport.
    let seek_row = GtkBox::new(Orientation::Horizontal, 6);
    seek_row.set_visible(false);
    seek_row.set_halign(Align::Fill);
    let t_pos = Label::new(Some("0:00"));
    t_pos.add_css_class("nm-time");
    let seek = Scale::with_range(Orientation::Horizontal, 0.0, 1000.0, 1.0);
    seek.set_draw_value(false);
    seek.set_hexpand(true);
    seek.add_css_class("nm-seek");
    let t_dur = Label::new(Some("0:00"));
    t_dur.add_css_class("nm-time");
    seek_row.append(&t_pos);
    seek_row.append(&seek);
    seek_row.append(&t_dur);
    transport.append(&seek_row);
    bar.append(&transport);
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

    let right = GtkBox::new(Orientation::Horizontal, 4);
    right.set_halign(Align::End);
    right.set_valign(Align::Center);
    let stage_btn = Button::with_label("Stage");
    let eq_btn = Button::with_label("Edit EQ");
    stage_btn.add_css_class("nm-chip");
    let sound_btn = Button::with_label("Sound");
    sound_btn.add_css_class("nm-icon");
    let queue_btn = Button::with_label("Queue");
    queue_btn.add_css_class("nm-icon");
    let cast_btn = Button::with_label("Cast");
    cast_btn.add_css_class("nm-icon");
    let mini_btn = Button::with_label("Mini");
    mini_btn.add_css_class("nm-icon");
    right.append(&stage_btn);
    right.append(&eq_btn);
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
        let content = content.clone();
        let st = state.clone();
        eq_btn.connect_clicked(move |_| {
            st.stage_open.set(true);
            content.set_visible_child_name("stage");
            STAGE_STACK.with(|slot| {
                if let Some(stack) = slot.borrow().as_ref() {
                    stack.set_visible_child_name("audiophile");
                }
            });
        });
        eq_btn.add_css_class("nm-chip");
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
        mini_btn.connect_clicked(move |btn| {
            let mini = !st.mini_player.get();
            st.mini_player.set(mini);
            browse_body.set_visible(!mini);
            if mini {
                outer.add_css_class("nm-mini");
            } else {
                outer.remove_css_class("nm-mini");
            }
            if let Some(win) = btn
                .root()
                .and_then(|r| r.downcast::<ApplicationWindow>().ok())
            {
                win.unmaximize();
                if mini {
                    win.set_default_size(420, 120);
                } else {
                    let (w, h) = preferred_window_size();
                    win.set_default_size(w, h);
                }
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
            if on {
                b.add_css_class("nm-icon-on");
            } else {
                b.remove_css_class("nm-icon-on");
            }
            b.set_tooltip_text(Some(if on { "Shuffle on" } else { "Shuffle" }));
        });
    }
    {
        let st = state.clone();
        repeat.connect_clicked(move |b| {
            let mode = st.player.repeat().next();
            st.player.set_repeat(mode);
            b.set_tooltip_text(Some(mode.label()));
            match mode {
                RepeatMode::Off => b.remove_css_class("nm-icon-on"),
                _ => b.add_css_class("nm-icon-on"),
            }
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
    let seek_s = seek.clone();
    let t_pos_l = t_pos.clone();
    let t_dur_l = t_dur.clone();
    let performer_b = performer_btn.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        if let Some(t) = st.now.read().clone() {
            title_l.set_text(&t.title);
            let sub = if t.album.is_empty() {
                t.artist.clone()
            } else {
                format!("{} — {}", t.artist, t.album)
            };
            let mut sub = sub;
            let until = continuum_until().load(std::sync::atomic::Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now < until {
                sub.push_str(" · continuing");
            }
            artist_l.set_text(&sub);
            bind_cover(&cover_i, t.cover_url.as_deref());
            performer_b.set_visible(!t.artist.is_empty());
        } else {
            performer_b.set_visible(false);
        }
        play_b.set_label(if st.player.is_playing() { "⏸" } else { "▶" });
        let pos = st.player.position_secs();
        let dur = st.player.duration_secs();
        t_pos_l.set_text(&format_time(pos));
        t_dur_l.set_text(&format_time(dur));
        if dur > 0 && !seek_s.is_focus() {
            seek_s.set_value((pos as f64 / dur as f64) * 1000.0);
        }
        glib::ControlFlow::Continue
    });

    bar
}

fn format_time(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
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
    let mut problems: Vec<String> = Vec::new();
    let mut home = match be.home_shelves() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("home_shelves: {e:#}");
            problems.push(format!("Home: {e:#}"));
            vec![]
        }
    };
    let radio = be.radio_shelves().unwrap_or_else(|e| {
        tracing::warn!("radio_shelves: {e:#}");
        vec![]
    });
    let mfy = be.made_for_you().unwrap_or_else(|e| {
        tracing::warn!("made_for_you: {e:#}");
        vec![]
    });
    if let Some(yt) = ytm.as_ref() {
        match yt.home_shelves() {
            Ok(mut shelves) => {
                for s in &mut shelves {
                    s.title = format!("{} · YT", s.title);
                }
                home.extend(shelves);
            }
            Err(err) => problems.push(format!(
                "YouTube Music home failed: {err:#}. Use Repair if this keeps failing."
            )),
        }
        match yt.made_for_you() {
            Ok(mut moods) => {
                if moods.is_empty() {
                    problems.push(
                        "YouTube Music charts and moods came back empty.".into(),
                    );
                }
                for s in &mut moods {
                    s.title = format!("{} · YT", s.title);
                }
                home.extend(moods);
            }
            Err(err) => problems.push(format!(
                "YouTube Music charts and moods failed: {err:#}. Use Repair if this keeps failing."
            )),
        }
    }
    let playlists = match playlists::sync_all(&server_id, be.clone()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "playlist sync failed; using server list");
            problems.push(format!("Playlist sync: {e:#}"));
            be.playlists().unwrap_or_else(|e2| {
                problems.push(format!("Playlists: {e2:#}"));
                vec![]
            })
        }
    };
    let artists = be.artists().unwrap_or_else(|e| {
        tracing::warn!("artists: {e:#}");
        problems.push(format!("Artists: {e:#}"));
        vec![]
    });
    let mut albums = be.albums().unwrap_or_else(|e| {
        tracing::warn!("albums: {e:#}");
        problems.push(format!("Albums: {e:#}"));
        vec![]
    });
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
                    // Keep the server cover when it is already a URL. Web lookup
                    // only fills gaps, and the UI downloads bytes off the GTK thread.
                    a.cover_url =
                        crate::artwork::prefer_web_cover(&a.artist, &a.name, a.cover_url.take());
                }
                for t in shelf.tracks.iter_mut().take(8) {
                    t.cover_url =
                        crate::artwork::prefer_web_cover(&t.artist, &t.album, t.cover_url.take());
                }
                shelf
            })
            .collect()
    };

    let error = if !problems.is_empty() {
        Some(problems.join(" · "))
    } else if home.is_empty() && artists.is_empty() && albums.is_empty() {
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

    let recent_tracks = collect_continue_tracks(state);
    let hero_items = build_hero_items(state, &snap, &recent_tracks);
    let hour = chrono::Timelike::hour(&chrono::Local::now());
    let greet = if hour < 12 {
        "Good morning"
    } else if hour < 17 {
        "Good afternoon"
    } else {
        "Good evening"
    };
    let greeting = Label::new(Some(greet));
    greeting.add_css_class("nm-heading");
    greeting.set_halign(Align::Start);
    home.append(&greeting);
    if let Ok(artists) = state.taste.top_artists(1) {
        if let Some((name, _)) = artists.first() {
            if !name.is_empty() {
                let because = Label::new(Some(&format!("Because you listen to {name}")));
                because.add_css_class("nm-muted");
                because.set_halign(Align::Start);
                home.append(&because);
            }
        }
    }
    let hero_banner = Label::new(Some("Featured"));
    hero_banner.add_css_class("nm-section-title");
    hero_banner.set_halign(Align::Start);
    home.append(&hero_banner);
    if hero_items.is_empty() {
        let empty = Label::new(Some("Connect a library to see the art-forward hero."));
        empty.add_css_class("nm-muted");
        empty.set_halign(Align::Start);
        home.append(&empty);
    } else {
        home.append(&build_home_hero(state, hero_items));
    }

    // Continue listening — medium cards with progress ring styling.
    if !recent_tracks.is_empty() {
        append_section(home, "Continue listening");
        let row = GtkBox::new(Orientation::Horizontal, 10);
        for track in recent_tracks.iter().take(12) {
            row.append(&continue_card(state, track.clone()));
        }
        let scroll = ScrolledWindow::builder()
            .child(&row)
            .hexpand(true)
            .min_content_height(210)
            .build();
        scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
        scroll.set_propagate_natural_width(false);
        scroll.set_propagate_natural_height(false);
        scroll.set_width_request(120);
        home.append(&scroll);
    }

    if let Some(err) = &snap.error {
        let l = Label::new(Some(err));
        l.add_css_class("nm-muted");
        l.set_halign(Align::Start);
        l.set_wrap(true);
        l.set_max_width_chars(72);
        home.append(&l);
    }

    // Prefer New Music / New Podcasts shelves near the top when labeled that way.
    let mut rest = Vec::new();
    for shelf in snap.home.iter().chain(snap.mfy.iter()) {
        let t = shelf.title.to_ascii_lowercase();
        if t.contains("new") || t.contains("recent") {
            fill_shelf(state, home, shelf);
        } else {
            rest.push(shelf);
        }
    }
    if !snap.podcasts.is_empty() {
        append_section(home, "New podcasts");
        let row = GtkBox::new(Orientation::Horizontal, 10);
        for p in snap.podcasts.iter().take(12) {
            let btn = Button::new();
            btn.add_css_class("nm-card");
            let col = GtkBox::new(Orientation::Vertical, 8);
            col.append(&cover_frame(p.cover_url.as_deref()));
            let title = Label::new(Some(&p.title));
            title.add_css_class("nm-card-title");
            title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            title.set_max_width_chars(18);
            col.append(&title);
            btn.set_child(Some(&col));
            let page = podcast_page.clone();
            let show = crate::podcasts::Show::from_server(p);
            btn.connect_clicked(move |_| {
                CONTENT_STACK.with(|slot| {
                    if let Some(stack) = slot.borrow().as_ref() {
                        stack.set_visible_child_name("podcasts");
                    }
                });
                page.open_show(show.clone());
            });
            row.append(&btn);
        }
        let scroll = ScrolledWindow::builder()
            .child(&row)
            .hexpand(true)
            .min_content_height(200)
            .build();
        scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
        home.append(&scroll);
    }
    for shelf in rest {
        fill_shelf(state, home, shelf);
    }
    for shelf in &snap.radio {
        fill_shelf(state, home, shelf);
    }

    podcast_page.set_server_shows(snap.podcasts.clone());
    RADIO_PAGE.with(|slot| {
        if let Some(page) = slot.borrow().as_ref() {
            page.set_server_stations(snap.stations.clone());
        }
    });
    fill_queue(state, queue);
}

#[derive(Clone)]
struct HeroItem {
    title: String,
    subtitle: String,
    cover: Option<String>,
    kind: &'static str,
    album: Option<Album>,
    track: Option<Track>,
}

fn collect_continue_tracks(state: &Rc<AppState>) -> Vec<Track> {
    let Some(srv) = state.server.read().clone() else {
        return Vec::new();
    };
    let Ok(recent) = state.history.recent(&srv.id, 12) else {
        return Vec::new();
    };
    let be = state.backend.read().clone();
    let mut out = Vec::new();
    for (id, title, artist, _) in recent {
        let ytm = id.starts_with("ytm:");
        let cover = if ytm {
            None
        } else {
            be.as_ref().and_then(|b| b.item_cover_url(&id))
        };
        let mut track = Track {
            id: id.clone(),
            title: title.clone(),
            artist: artist.clone(),
            stream_url: String::new(),
            cover_url: cover,
            source: if ytm {
                SourceKind::YoutubeMusic
            } else {
                SourceKind::Server
            },
            ..Default::default()
        };
        if let Some(b) = be.as_ref() {
            if let Ok(url) = b.stream_url(&track, TranscodeMode::Direct) {
                if url.contains("://") {
                    track.stream_url = url;
                }
            }
        }
        out.push(track);
    }
    out
}

fn build_hero_items(
    state: &Rc<AppState>,
    snap: &UiSnapshot,
    recent: &[Track],
) -> Vec<HeroItem> {
    let mut items = Vec::new();
    if let Some(t) = state.now.read().clone().or_else(|| recent.first().cloned()) {
        let cover = crate::artwork::hero_cover(
            &t.artist,
            &t.album,
            t.cover_url.clone(),
        );
        items.push(HeroItem {
            title: t.title.clone(),
            subtitle: format!("{} · Continue", t.artist),
            cover,
            kind: "continue",
            album: None,
            track: Some(t),
        });
    }
    for shelf in snap.home.iter().chain(snap.mfy.iter()) {
        let t = shelf.title.to_ascii_lowercase();
        if !(t.contains("new") || t.contains("recent")) {
            continue;
        }
        if let Some(a) = shelf.albums.first() {
            items.push(HeroItem {
                title: a.name.clone(),
                subtitle: format!("{} · New music", a.artist),
                cover: crate::artwork::hero_cover(&a.artist, &a.name, a.cover_url.clone()),
                kind: "new_music",
                album: Some(a.clone()),
                track: None,
            });
        }
        if items.len() >= 6 {
            break;
        }
    }
    for p in snap.podcasts.iter().take(2) {
        items.push(HeroItem {
            title: p.title.clone(),
            subtitle: "New podcast".into(),
            cover: p.cover_url.clone(),
            kind: "podcast",
            album: None,
            track: None,
        });
    }
    // Always have a hero — fall back to first library albums so Home never looks "unchanged".
    if items.is_empty() {
        for shelf in snap.home.iter().chain(snap.mfy.iter()) {
            for a in shelf.albums.iter().take(3) {
                items.push(HeroItem {
                    title: a.name.clone(),
                    subtitle: format!("{} · Featured", a.artist),
                    cover: crate::artwork::hero_cover(&a.artist, &a.name, a.cover_url.clone()),
                    kind: "new_music",
                    album: Some(a.clone()),
                    track: None,
                });
            }
            if !items.is_empty() {
                break;
            }
        }
    }
    if items.is_empty() {
        if let Some(a) = snap.albums.first() {
            items.push(HeroItem {
                title: a.name.clone(),
                subtitle: format!("{} · Library", a.artist),
                cover: a.cover_url.clone(),
                kind: "new_music",
                album: Some(a.clone()),
                track: None,
            });
        }
    }
    items.truncate(8);
    items
}

fn build_home_hero(state: &Rc<AppState>, items: Vec<HeroItem>) -> GtkBox {
    let wrap = GtkBox::new(Orientation::Vertical, 8);
    wrap.add_css_class("nm-hero");
    let idx = Rc::new(Cell::new(0usize));
    let stage = GtkBox::new(Orientation::Vertical, 10);
    stage.add_css_class("nm-hero-stage");
    stage.set_hexpand(true);

    let cover = gtk4::Image::new();
    cover.add_css_class("nm-hero-art");
    cover.set_pixel_size(280);
    cover.set_halign(Align::Center);
    cover.set_icon_name(Some("media-optical-symbolic"));
    let title = Label::new(None);
    title.add_css_class("nm-hero-title");
    title.set_halign(Align::Center);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(40);
    let sub = Label::new(None);
    sub.add_css_class("nm-hero-sub");
    sub.set_halign(Align::Center);
    let play = Button::with_label("Play");
    play.add_css_class("nm-transport");
    play.add_css_class("nm-play");
    play.set_halign(Align::Center);
    let open_btn = Button::with_label("Open");
    open_btn.add_css_class("nm-chip");
    let nav = GtkBox::new(Orientation::Horizontal, 8);
    nav.set_halign(Align::Center);
    let prev = Button::with_label("‹");
    prev.add_css_class("nm-icon");
    let next = Button::with_label("›");
    next.add_css_class("nm-icon");
    nav.append(&prev);
    nav.append(&play);
    nav.append(&open_btn);
    nav.append(&next);
    stage.append(&cover);
    stage.append(&title);
    stage.append(&sub);
    stage.append(&nav);
    wrap.append(&stage);

    let items_r = Rc::new(items);
    let paint = {
        let items = items_r.clone();
        let idx = idx.clone();
        let cover = cover.clone();
        let title = title.clone();
        let sub = sub.clone();
        Rc::new(move || {
            let i = idx.get() % items.len().max(1);
            if let Some(it) = items.get(i) {
                title.set_text(&it.title);
                sub.set_text(&it.subtitle);
                bind_cover(&cover, it.cover.as_deref());
                let (artist, album) = match (&it.album, &it.track) {
                    (Some(a), _) => (a.artist.clone(), a.name.clone()),
                    (_, Some(t)) => (t.artist.clone(), t.album.clone()),
                    _ => (String::new(), String::new()),
                };
                if !artist.is_empty() {
                    super::ambient::set_context(
                        super::bleed::Mode::Sharp,
                        super::ambient::Ctx { artist, album, cover_url: it.cover.clone() },
                    );
                }
            }
        }) as Rc<dyn Fn()>
    };
    paint();
    let hold = Rc::new(Cell::new(0u64));
    let hold_for = {
        let hold = hold.clone();
        move || {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            hold.set(now.saturating_add(20));
        }
    };

    {
        let idx = idx.clone();
        let items = items_r.clone();
        let paint = paint.clone();
        let hold_for = hold_for.clone();
        prev.connect_clicked(move |_| {
            hold_for();
            let n = items.len().max(1);
            idx.set((idx.get() + n - 1) % n);
            paint();
        });
    }
    {
        let idx = idx.clone();
        let items = items_r.clone();
        let paint = paint.clone();
        let hold_for = hold_for.clone();
        next.connect_clicked(move |_| {
            hold_for();
            let n = items.len().max(1);
            idx.set((idx.get() + 1) % n);
            paint();
        });
    }
    {
        let st = state.clone();
        let items = items_r.clone();
        let idx = idx.clone();
        let hold_for = hold_for.clone();
        play.connect_clicked(move |_| {
            hold_for();
            let Some(it) = items.get(idx.get()) else {
                return;
            };
            if let Some(t) = &it.track {
                st.player.set_queue(vec![t.clone()], 0);
                play_track(&st, t.clone());
            } else if let Some(album) = &it.album {
                if let Some(ui) = LIBRARY_UI.with(|s| s.borrow().clone()) {
                    if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                        stack.set_visible_child_name("library");
                    }
                    open_album(&st, &ui, album.clone());
                }
            } else if it.kind == "podcast" {
                if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                    stack.set_visible_child_name("podcasts");
                }
            }
        });
    }
    {
        let st = state.clone();
        let items = items_r.clone();
        let idx = idx.clone();
        let hold_for = hold_for.clone();
        open_btn.connect_clicked(move |_| {
            hold_for();
            let Some(it) = items.get(idx.get()) else {
                return;
            };
            if let Some(album) = &it.album {
                if let Some(ui) = LIBRARY_UI.with(|s| s.borrow().clone()) {
                    if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                        stack.set_visible_child_name("library");
                    }
                    open_album(&st, &ui, album.clone());
                }
            } else if let Some(t) = &it.track {
                if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                    open_performer_by_name(
                        &st,
                        &stack,
                        &t.artist,
                        t.artist_id.as_deref().unwrap_or(&t.artist),
                    );
                }
            } else if it.kind == "podcast" {
                if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                    stack.set_visible_child_name("podcasts");
                }
            }
        });
    }

    // Slow auto-advance while playing (Netflix feel).
    let idx_r = idx.clone();
    let items_r2 = items_r.clone();
    let paint_r = paint.clone();
    let st = state.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(12), move || {
        if items_r2.is_empty() {
            return glib::ControlFlow::Break;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now < hold.get() {
            return glib::ControlFlow::Continue;
        }
        let interval_ok = if st.player.is_playing() {
            // Slowed while playing — fire every tick but only advance every ~2 callbacks via counter
            true
        } else {
            true
        };
        if interval_ok {
            idx_r.set((idx_r.get() + 1) % items_r2.len());
            paint_r();
        }
        glib::ControlFlow::Continue
    });

    wrap
}

fn continue_card(state: &Rc<AppState>, track: Track) -> Button {
    let btn = Button::new();
    btn.add_css_class("nm-card");
    btn.add_css_class("nm-continue-card");
    btn.set_width_request(168);
    let col = GtkBox::new(Orientation::Vertical, 8);
    let frame = GtkBox::new(Orientation::Vertical, 0);
    frame.add_css_class("nm-cover-frame");
    frame.add_css_class("nm-progress-ring");
    frame.append(&cover_image(track.cover_url.as_deref()));
    col.append(&frame);
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
        btn.set_width_request(168);
        let col = GtkBox::new(Orientation::Vertical, 8);
        col.append(&cover_frame(p.cover_url.as_deref()));
        let title = Label::new(Some(&p.name));
        title.add_css_class("nm-card-title");
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title.set_max_width_chars(18);
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
        .min_content_height(196)
        .build();
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    scroll.set_propagate_natural_width(false);
    scroll.set_propagate_natural_height(false);
    // Keep min size tiny so 12×160px cards cannot force the window wider than the monitor.
    scroll.set_width_request(120);
    parent.append(&scroll);
}

fn cover_image(url: Option<&str>) -> gtk4::Image {
    let image = gtk4::Image::new();
    image.add_css_class("nm-cover-art");
    image.set_pixel_size(148);
    image.set_icon_name(Some("media-optical-symbolic"));
    bind_cover(&image, url);
    image
}

fn cover_frame(url: Option<&str>) -> GtkBox {
    cover_frame_titled(url, None)
}

fn cover_frame_titled(url: Option<&str>, title: Option<&str>) -> GtkBox {
    let frame = GtkBox::new(Orientation::Vertical, 0);
    frame.add_css_class("nm-cover-frame");
    frame.set_halign(Align::Center);
    let letter = title
        .and_then(|t| t.chars().find(|c| c.is_alphanumeric()))
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "♪".into());
    if url.map(str::trim).filter(|u| !u.is_empty()).is_none() {
        let mark = Label::new(Some(&letter));
        mark.add_css_class("nm-art-fallback");
        mark.set_size_request(148, 148);
        frame.append(&mark);
    } else {
        frame.append(&cover_image(url));
    }
    frame
}

fn album_card(state: &Rc<AppState>, album: Album) -> GtkBox {
    let wrap = GtkBox::new(Orientation::Vertical, 4);
    wrap.add_css_class("nm-card-wrap");
    wrap.set_widget_name(&format!("{} {}", album.artist, album.name));
    let btn = Button::new();
    btn.add_css_class("nm-card");
    btn.set_width_request(168);
    let col = GtkBox::new(Orientation::Vertical, 8);
    col.append(&cover_frame_titled(album.cover_url.as_deref(), Some(&album.name)));
    let title = Label::new(Some(&album.name));
    title.add_css_class("nm-card-title");
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    let sub = Label::new(Some(&album.artist));
    sub.add_css_class("nm-card-sub");
    sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    sub.set_max_width_chars(18);
    let badge = Label::new(Some(album.source.badge()));
    badge.add_css_class("nm-badge");
    col.append(&badge);
    col.append(&title);
    col.append(&sub);
    btn.set_child(Some(&col));
    let st = state.clone();
    let album_play = album.clone();
    btn.connect_clicked(move |_| {
        // Open album detail — never autoplay from discovery cards.
        if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
            stack.set_visible_child_name("library");
        }
        if let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) {
            open_album(&st, &ui, album.clone());
        }
    });
    let play = Button::with_label("Play");
    play.add_css_class("nm-chip");
    play.add_css_class("nm-card-play");
    play.set_halign(Align::Center);
    let st_play = state.clone();
    play.connect_clicked(move |_| {
        let (tx, rx) = std::sync::mpsc::channel();
        let id = album_play.id.clone();
        let backend = if id.starts_with("ytm:") {
            st_play.ytm.read().clone()
        } else {
            st_play.backend.read().clone()
        };
        std::thread::spawn(move || {
            let tracks = backend
                .and_then(|b| b.album_tracks(&id).ok())
                .unwrap_or_default();
            let _ = tx.send(tracks);
        });
        let st = st_play.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
            match rx.try_recv() {
                Ok(tracks) => {
                    if let Some(first) = tracks.first().cloned() {
                        st.player.set_queue(tracks, 0);
                        play_track(&st, first);
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    });
    wrap.append(&btn);
    wrap.append(&play);
    wrap
}

fn artist_card(state: &Rc<AppState>, artist: Artist) -> Button {
    let btn = Button::new();
    btn.add_css_class("nm-card");
    btn.add_css_class("nm-artist-card");
    btn.set_width_request(148);
    btn.set_widget_name(&artist.name);
    let col = GtkBox::new(Orientation::Vertical, 8);
    let holder = GtkBox::new(Orientation::Vertical, 0);
    holder.append(&cover_frame_titled(artist.cover_url.as_deref(), Some(&artist.name)));
    col.append(&holder);
    // W665: swap in a real artist photo when a source has one (keyless Deezer/Wikimedia, Fanart if keyed).
    {
        let slot: Arc<parking_lot::Mutex<Option<std::path::PathBuf>>> = Arc::new(parking_lot::Mutex::new(None));
        crate::artimg::portrait_async(&artist.name, slot.clone());
        let holder = holder.clone();
        let tries = Rc::new(Cell::new(0u32));
        glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            if let Some(path) = slot.lock().take() {
                while let Some(c) = holder.first_child() {
                    holder.remove(&c);
                }
                // Fixed 148 px round frame so the photo never stretches with the card.
                let frame = GtkBox::new(Orientation::Vertical, 0);
                frame.set_size_request(148, 148);
                frame.set_halign(Align::Center);
                frame.set_hexpand(false);
                frame.set_overflow(gtk4::Overflow::Hidden);
                frame.add_css_class("nm-artist-photo");
                frame.add_css_class("nm-fade-in");
                let pic = gtk4::Picture::new();
                pic.set_content_fit(gtk4::ContentFit::Cover);
                pic.set_can_shrink(true);
                pic.set_size_request(148, 148);
                pic.set_filename(Some(path));
                frame.append(&pic);
                holder.append(&frame);
                return glib::ControlFlow::Break;
            }
            tries.set(tries.get() + 1);
            if tries.get() > 120 || holder.root().is_none() && tries.get() > 5 {
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
    let title = Label::new(Some(&artist.name));
    title.add_css_class("nm-card-title");
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(16);
    col.append(&title);
    btn.set_child(Some(&col));
    let st = state.clone();
    btn.connect_clicked(move |_| {
        // Rich artist profile (hero, bio, details, popular, albums, photos).
        if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
            open_performer(&st, &stack, artist.clone());
            return;
        }
        if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
            stack.set_visible_child_name("library");
        }
        let Some(ui) = LIBRARY_UI.with(|slot| slot.borrow().clone()) else {
            return;
        };
        ui.title.set_text("Loading…");
        let id = artist.id.clone();
        let backend = if id.starts_with("ytm:") {
            st.ytm.read().clone()
        } else {
            st.backend.read().clone()
        };
        let artist_meta = artist.clone();
        fetch_then(
            &ui,
            move || backend.and_then(|be| be.artist_albums(&id).ok()).unwrap_or_default(),
            {
                let st = st.clone();
                let ui = ui.clone();
                move |albums: Vec<Album>| {
                    ui.stack.borrow_mut().push(NavFrame::ArtistAlbums {
                        artist: artist_meta.clone(),
                        albums: albums.clone(),
                    });
                    show_artist_page(&st, &ui, artist_meta, albums);
                }
            },
        );
    });
    btn
}

fn track_card(state: &Rc<AppState>, track: Track) -> Button {
    let btn = Button::new();
    btn.add_css_class("nm-card");
    btn.set_width_request(168);
    let col = GtkBox::new(Orientation::Vertical, 8);
    col.append(&cover_frame_titled(
        track.cover_url.as_deref(),
        Some(&track.title),
    ));
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
        .unwrap_or_else(|| "—".into());
    let rate = track
        .bit_rate
        .map(|b| format!("{b} kbps"))
        .unwrap_or_else(|| "—".into());
    let sample = track
        .sample_rate
        .map(|hz| format!("{hz} Hz"))
        .unwrap_or_else(|| "—".into());
    let ytm = track.source == SourceKind::YoutubeMusic || track.id.starts_with("ytm:");
    let label = Label::new(Some(&format!(
        "{num:<4} {} — {}    {dur}    {fmt}    {rate}    {sample}",
        track.title, track.album
    )));
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(64);

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
            let liked = st.taste.is_liked(&t.id);
            if liked {
                let _ = st.taste.unlike(&t.id);
                if let Some(be) = st.backend.read().as_ref() {
                    let _ = be.set_favorite(&t.id, false);
                }
                b.set_label("♡");
            } else {
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
                    if st_l.taste.is_liked(&t_l.id) {
                        let _ = st_l.taste.unlike(&t_l.id);
                        if let Some(be) = st_l.backend.read().as_ref() {
                            let _ = be.set_favorite(&t_l.id, false);
                        }
                    } else {
                        let _ = st_l.taste.like(&t_l);
                        if let Some(be) = st_l.backend.read().as_ref() {
                            let _ = be.set_favorite(&t_l.id, true);
                        }
                        if ytm::is_ytm_track(&t_l) {
                            let uploader = st_l.backend.read().clone().map(|be| {
                                Arc::new(BackendUploader { backend: be }) as Arc<dyn LibraryUploader>
                            });
                            acquire::spawn_like_acquire(st_l.acquire.clone(), t_l.clone(), uploader);
                        }
                    }
                }),
            );
            let st_pl = st.clone();
            let t_pl = t.clone();
            add_btn(
                "Add to Playlist",
                Box::new(move || {
                    if let Some(be) = st_pl.backend.read().clone() {
                        if let Ok(lists) = be.playlists() {
                            if let Some(first) = lists.first() {
                                let mut ids = be
                                    .playlist_tracks(&first.id)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|tr| tr.id)
                                    .collect::<Vec<_>>();
                                if !ids.iter().any(|id| id == &t_pl.id) {
                                    ids.push(t_pl.id.clone());
                                }
                                if let Some(server) = st_pl.server.read().clone() {
                                    let _ = playlists::upsert_local(
                                        &server.id,
                                        &first.id,
                                        &first.name,
                                        ids,
                                        be.clone(),
                                    );
                                } else {
                                    let _ = be.update_playlist(&first.id, &first.name, &ids);
                                }
                            } else if let Ok(created) = be.create_playlist("Liked from Neighbourhood") {
                                let _ = be.update_playlist(&created.id, &created.name, &[t_pl.id.clone()]);
                            }
                            request_library_reload();
                        }
                    }
                }),
            );
            let st_al = st.clone();
            let t_al = t.clone();
            add_btn(
                "Go to Album",
                Box::new(move || {
                    let album = Album {
                        id: t_al.album_id.clone().unwrap_or_default(),
                        name: t_al.album.clone(),
                        artist: t_al.artist.clone(),
                        artist_id: t_al.artist_id.clone(),
                        cover_url: t_al.cover_url.clone(),
                        source: t_al.source,
                        ..Default::default()
                    };
                    if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                        open_album_detail(&st_al, &stack, album);
                    }
                }),
            );
            let st_ar = st.clone();
            let t_ar = t.clone();
            add_btn(
                "Go to Artist",
                Box::new(move || {
                    if let Some(stack) = CONTENT_STACK.with(|s| s.borrow().clone()) {
                        open_performer_by_name(
                            &st_ar,
                            &stack,
                            &t_ar.artist,
                            t_ar.artist_id.as_deref().unwrap_or(""),
                        );
                    }
                }),
            );
            pop.set_child(Some(&box_));
            pop.popup();
        });
    }

    {
        let badge = Label::new(Some(track.source.badge()));
        badge.add_css_class("nm-badge");
        row.append(&badge);
    }
    row.append(&label);
    row.append(&like);
    row.append(&play);
    row.append(&next);
    row.append(&more);
    row
}

fn fill_queue(state: &Rc<AppState>, box_: &GtkBox) {
    clear_after_heading(box_);
    let clear = Button::with_label("Clear queue");
    clear.add_css_class("nm-chip");
    clear.set_halign(Align::Start);
    let st_clear = state.clone();
    let box_clear = box_.clone();
    clear.connect_clicked(move |_| {
        st_clear.player.clear_queue();
        fill_queue(&st_clear, &box_clear);
    });
    box_.append(&clear);
    let queue = state.player.queue();
    let idx = state.player.index();
    for (i, t) in queue.iter().enumerate() {
        let mark = if i == idx { "▶ " } else { "" };
        let btn = Button::with_label(&format!("{mark}{} — {}", t.title, t.artist));
        btn.add_css_class("nm-nav-btn");
        btn.set_halign(Align::Start);
        let row = GtkBox::new(Orientation::Horizontal, 4);
        let st = state.clone();
        let track = t.clone();
        let all = queue.clone();
        btn.connect_clicked(move |_| {
            st.player.set_queue(all.clone(), i);
            play_track(&st, track.clone());
        });
        let up = Button::with_label("Up");
        let down = Button::with_label("Down");
        up.add_css_class("nm-icon");
        down.add_css_class("nm-icon");
        {
            let st = state.clone();
            let box_q = box_.clone();
            let all = queue.clone();
            up.connect_clicked(move |_| {
                if i == 0 {
                    return;
                }
                let mut q = all.clone();
                q.swap(i, i - 1);
                let idx = st.player.index();
                st.player.set_queue(q, idx);
                fill_queue(&st, &box_q);
            });
        }
        {
            let st = state.clone();
            let box_q = box_.clone();
            let all = queue.clone();
            down.connect_clicked(move |_| {
                let mut q = all.clone();
                if i + 1 >= q.len() {
                    return;
                }
                q.swap(i, i + 1);
                let idx = st.player.index();
                st.player.set_queue(q, idx);
                fill_queue(&st, &box_q);
            });
        }
        row.append(&btn);
        row.append(&up);
        row.append(&down);
        box_.append(&row);
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

/// Extra margin inside the EWMH work area so SSD chrome + player never sit under the Start bar.
const WORKAREA_INSET_PX: i32 = 16;

fn primary_monitor_geometry() -> Option<(i32, i32)> {
    let display = gtk4::gdk::Display::default()?;
    let monitors = display.monitors();
    let monitor = monitors
        .item(0)
        .and_then(|obj| obj.downcast::<gtk4::gdk::Monitor>().ok())?;
    let geo = monitor.geometry();
    Some((geo.width(), geo.height()))
}

/// Prefer `_NET_WORKAREA` (excludes MDE taskbar). Fallback: monitor minus 48px bar.
fn x11_workarea() -> Option<(i32, i32, i32, i32)> {
    let out = std::process::Command::new("xprop")
        .args(["-root", "_NET_WORKAREA"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // _NET_WORKAREA(CARDINAL) = x, y, w, h, …
    let nums: Vec<i32> = text
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() >= 4 {
        Some((nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

fn usable_monitor_size() -> Option<(i32, i32)> {
    if let Some((_x, _y, ww, wh)) = x11_workarea() {
        let uw = (ww - WORKAREA_INSET_PX * 2).max(640);
        let uh = (wh - WORKAREA_INSET_PX * 2).max(400);
        return Some((uw, uh));
    }
    let (mw, mh) = primary_monitor_geometry()?;
    // MDE comfortable taskbar is 48px; keep extra slack for SSD titlebar.
    let uw = (mw - 48).max(640);
    let uh = (mh - 48 - 48).max(400);
    Some((uw, uh))
}

fn preferred_window_size() -> (i32, i32) {
    let Some((uw, uh)) = usable_monitor_size() else {
        return (1024, 640);
    };
    // Stay strictly inside the work area — content must scroll, not grow the window.
    let w = ((uw as f64) * 0.90).clamp(720.0, uw as f64) as i32;
    let h = ((uh as f64) * 0.92).clamp(400.0, uh as f64) as i32;
    (w.min(uw), h.min(uh))
}

fn preferred_window_position(w: i32, h: i32) -> (i32, i32) {
    if let Some((wx, wy, ww, wh)) = x11_workarea() {
        let x = wx + ((ww - w) / 2).max(WORKAREA_INSET_PX);
        let y = wy + ((wh - h) / 2).max(WORKAREA_INSET_PX);
        // Prefer upper half so the player stays clear of the taskbar even if WM chrome grows.
        let y = y.min(wy + WORKAREA_INSET_PX + 24);
        return (x, y);
    }
    (24, 24)
}

fn should_start_maximized() -> bool {
    false
}

/// GTK4 often will not shrink a mapped window when content requests a larger min size.
/// Nudge the WM with wmctrl when available (X11 / Xwayland).
fn force_wm_geometry(w: i32, h: i32) {
    let (x, y) = preferred_window_position(w, h);
    let out = match std::process::Command::new("wmctrl").args(["-lG"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return,
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(line) = text
        .lines()
        .find(|l| l.contains("Neighbourhood Music") || l.contains("Network Music Interface"))
    else {
        return;
    };
    let id = line.split_whitespace().next().unwrap_or("");
    if id.is_empty() {
        return;
    }
    let _ = std::process::Command::new("wmctrl")
        .args([
            "-i",
            "-r",
            id,
            "-b",
            "remove,maximized_vert,maximized_horz,fullscreen",
        ])
        .status();
    let _ = std::process::Command::new("wmctrl")
        .args([
            "-i",
            "-r",
            id,
            "-e",
            &format!("0,{x},{y},{w},{h}"),
        ])
        .status();
    tracing::debug!(%id, x, y, w, h, "forced music window into work area");
}

/// After library sync builds heavy widgets, force the window back into the work area.
fn reclamp_window_size(widget: &impl IsA<gtk4::Widget>) {
    let Some(win) = widget
        .root()
        .and_then(|r| r.downcast::<ApplicationWindow>().ok())
    else {
        return;
    };
    if win.is_maximized() {
        win.unmaximize();
    }
    let (w, h) = preferred_window_size();
    win.set_default_size(w, h);
    force_wm_geometry(w, h);
}

fn short_banner_text(raw: &str) -> String {
    let line = raw.lines().find(|l| !l.trim().is_empty()).unwrap_or(raw);
    let trimmed = line.trim();
    if trimmed.chars().count() <= 96 {
        trimmed.to_string()
    } else {
        let cut: String = trimmed.chars().take(93).collect();
        format!("{cut}…")
    }
}

fn build_repair_banner() -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("nm-repair-banner");
    bar.set_visible(false);
    bar.set_widget_name("nm-repair-banner");
    let label = Label::new(Some(""));
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(72);
    label.set_wrap(false);
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
        let repairing = ytm::repair::is_repairing();
        let failed = matches!(
            status.phase,
            ytm::repair::RepairPhase::Failed(_) | ytm::repair::RepairPhase::RolledBack
        );
        bar_t.set_visible(repairing || failed);
        if repairing {
            if status.message.is_empty() {
                label_t.set_text("Repairing YouTube Music access…");
            } else {
                label_t.set_text(&format!(
                    "YouTube Music: {}",
                    short_banner_text(&status.message)
                ));
            }
        } else if failed {
            let msg = if status.message.is_empty() {
                "YouTube Music needs attention — try Repair now.".into()
            } else {
                format!("YouTube Music: {}", short_banner_text(&status.message))
            };
            label_t.set_text(&msg);
        }
        glib::ControlFlow::Continue
    });
    bar
}

fn stage_cover_px() -> i32 {
    let h = usable_monitor_size().map(|(_, h)| h).unwrap_or(640);
    // Keep Stage art inside short work areas (e.g. 730px usable).
    ((h as f64) * 0.42).clamp(180.0, 320.0) as i32
}

fn build_stage(state: Rc<AppState>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 10);
    page.add_css_class("nm-main");
    page.add_css_class("nm-stage");
    let tabs = GtkBox::new(Orientation::Horizontal, 8);
    tabs.set_halign(Align::Center);
    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);
    let art = GtkBox::new(Orientation::Vertical, 14);
    art.set_valign(Align::Center);
    art.set_halign(Align::Center);
    let cover_wrap = GtkBox::new(Orientation::Vertical, 0);
    cover_wrap.add_css_class("nm-stage-cover-frame");
    cover_wrap.set_halign(Align::Center);
    let cover = gtk4::Image::new();
    cover.add_css_class("nm-stage-cover-art");
    cover.set_pixel_size(stage_cover_px());
    cover.set_halign(Align::Center);
    cover.set_widget_name("nm-stage-cover");
    cover_wrap.append(&cover);
    art.append(&cover_wrap);
    let stage_title = Label::new(Some(""));
    stage_title.add_css_class("nm-stage-title");
    stage_title.set_halign(Align::Center);
    stage_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    stage_title.set_max_width_chars(42);
    let stage_artist = Label::new(Some(""));
    stage_artist.add_css_class("nm-stage-artist");
    stage_artist.set_halign(Align::Center);
    stage_artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    stage_artist.set_max_width_chars(42);
    art.append(&stage_title);
    art.append(&stage_artist);
    let seek = Scale::with_range(Orientation::Horizontal, 0.0, 1000.0, 1.0);
    seek.set_draw_value(false);
    seek.add_css_class("nm-seek");
    seek.set_width_request(320);
    let times = Label::new(Some("0:00 / 0:00"));
    times.add_css_class("nm-time");
    times.set_halign(Align::Center);
    art.append(&seek);
    art.append(&times);
    let wave = gtk4::DrawingArea::new();
    wave.set_content_width(360);
    wave.set_content_height(64);
    wave.set_draw_func(|_, cr, w, h| {
        cr.set_source_rgb(0.05, 0.06, 0.08);
        cr.paint().ok();
        let samples = crate::eq::shared_eq().lock().waveform(160);
        cr.set_source_rgb(0.55, 0.82, 1.0);
        let mid = h as f64 / 2.0;
        for (i, sample) in samples.iter().enumerate() {
            let x = i as f64 / samples.len().max(1) as f64 * w as f64;
            let y = mid - (*sample as f64).clamp(-1.0, 1.0) * mid * 0.9;
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        cr.stroke().ok();
        let spectrum = crate::eq::shared_eq().lock().spectrum_db();
        cr.set_source_rgba(0.9, 0.75, 0.4, 0.8);
        for (i, db) in spectrum.iter().enumerate() {
            let x = (i as f64 + 0.2) / 10.0 * w as f64;
            let bar_h = ((db + 80.0) / 80.0).clamp(0.0, 1.0) * (h as f64 * 0.35);
            cr.rectangle(x, h as f64 - bar_h - 2.0, (w as f64 / 12.0).max(2.0), bar_h);
        }
        cr.fill().ok();
    });
    art.append(&wave);
    let wave_r = wave.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        wave_r.queue_draw();
        glib::ControlFlow::Continue
    });
    let lyrics_box = GtkBox::new(Orientation::Vertical, 4);
    let queue_box = GtkBox::new(Orientation::Vertical, 4);
    let credits = Label::new(Some("Play a track to see title, artist, album, format, and bitrate."));
    credits.set_wrap(true);
    stack.add_named(&art, Some("art"));
    stack.add_named(&lyrics_box, Some("lyrics"));
    stack.add_named(&queue_box, Some("queue"));
    stack.add_named(&credits, Some("credits"));
    let watch = GtkBox::new(Orientation::Vertical, 8);
    watch.set_margin_top(12);
    watch.set_margin_start(16);
    watch.set_margin_end(16);
    let watch_note = Label::new(Some(
        "Audio stays the default. Show video only when this track has a video address.",
    ));
    watch_note.set_wrap(true);
    watch_note.add_css_class("nm-muted");
    watch_note.set_halign(Align::Start);
    let watch_status = Label::new(Some(""));
    watch_status.set_wrap(true);
    watch_status.set_halign(Align::Start);
    let video = gtk4::Video::new();
    video.set_autoplay(true);
    video.set_hexpand(true);
    video.set_vexpand(true);
    video.set_size_request(480, 270);
    video.add_css_class("nm-eq-plot");
    let show_video = Button::with_label("Show video");
    show_video.add_css_class("nm-chip");
    show_video.set_halign(Align::Start);
    let back_audio = Button::with_label("Back to audio");
    back_audio.add_css_class("nm-chip");
    back_audio.set_halign(Align::Start);
    let watch_actions = GtkBox::new(Orientation::Horizontal, 8);
    watch_actions.append(&show_video);
    watch_actions.append(&back_audio);
    watch.append(&watch_note);
    watch.append(&watch_status);
    watch.append(&watch_actions);
    watch.append(&video);
    let audio_paused_for_watch = Rc::new(Cell::new(false));
    {
        let st = state.clone();
        let status = watch_status.clone();
        let video_w = video.clone();
        let paused = audio_paused_for_watch.clone();
        show_video.connect_clicked(move |_| {
            let url = st
                .now
                .read()
                .as_ref()
                .and_then(|track| track.video_url.clone())
                .filter(|url| crate::model::watchable_video(Some(url)));
            let Some(url) = url else {
                status.set_text("This track has no video. Audio keeps playing.");
                return;
            };
            let _ = st.player.pause();
            paused.set(true);
            video_w.set_file(Some(&File::for_uri(&url)));
            status.set_text("Playing the video. Audio is paused until you go back.");
        });
    }
    {
        let st = state.clone();
        let status = watch_status.clone();
        let video_w = video.clone();
        let paused = audio_paused_for_watch.clone();
        let stack_back = stack.clone();
        back_audio.connect_clicked(move |_| {
            video_w.set_file(None::<&File>);
            if paused.replace(false) {
                let _ = st.player.play();
            }
            status.set_text("Audio resumed.");
            stack_back.set_visible_child_name("art");
        });
    }
    stack.add_named(&watch, Some("watch"));
    {
        let video_w = video.clone();
        let st = state.clone();
        let paused = audio_paused_for_watch.clone();
        let status = watch_status.clone();
        stack.connect_visible_child_notify(move |stack| {
            if stack.visible_child_name().as_deref() == Some("watch") {
                return;
            }
            video_w.set_file(None::<&File>);
            if paused.replace(false) {
                let _ = st.player.play();
                status.set_text("Audio resumed.");
            }
        });
    }
    let audiophile = super::audiophile::build(state.clone());
    stack.add_named(&audiophile, Some("audiophile"));
    STAGE_STACK.with(|slot| *slot.borrow_mut() = Some(stack.clone()));
    stack.set_visible_child_name("art");
    for (name, label) in [
        ("art", "Artwork"),
        ("lyrics", "Lyrics"),
        ("queue", "Queue"),
        ("credits", "Credits"),
        ("watch", "Watch"),
        ("audiophile", "Audiophile"),
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
    let stage_title_l = stage_title.clone();
    let stage_artist_l = stage_artist.clone();
    let lyrics_b = lyrics_box.clone();
    let queue_b = queue_box.clone();
    let credits_l = credits.clone();
    let last = Rc::new(RefCell::new(String::new()));
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if let Some(t) = st.now.read().clone() {
            bind_cover(&cover_i, t.cover_url.as_deref());
            stage_title_l.set_text(&t.title);
            stage_artist_l.set_text(&format!("{} — {}", t.artist, t.album));
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
        if !seek_s.is_focus() {
            seek_s.set_value((pos / dur) * 1000.0);
        }
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
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);
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
    RADIO_PAGE.with(|slot| *slot.borrow_mut() = Some(stations.clone()));
    let tag_box = tags.1.clone();
    for tag in ["jazz", "classical", "rock", "pop", "electronic", "news", "ambient", "metal"] {
        let b = Button::with_label(tag);
        b.add_css_class("nm-chip");
        let stack_t = stack.clone();
        b.connect_clicked(move |_| {
            RADIO_PAGE.with(|slot| {
                if let Some(page) = slot.borrow().as_ref() {
                    page.select_tag(tag);
                }
            });
            stack_t.set_visible_child_name("stations");
        });
        tag_box.append(&b);
    }
    let fav_box = favorites.1.clone();
    let st_fav = state.clone();
    for station in radio::load_favorites() {
        let b = Button::with_label(&station.name);
        b.add_css_class("nm-nav-btn");
        b.set_halign(Align::Start);
        let st = st_fav.clone();
        let station = station.clone();
        b.connect_clicked(move |_| play_track(&st, station.to_track()));
        fav_box.append(&b);
    }
    if fav_box.first_child().is_none() {
        fav_box.append(&Label::new(Some(
            "No saved stations yet. Search on Stations, then press Save.",
        )));
    }
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

fn settings_row(state: &Rc<AppState>, title: &str, key: &'static str, hint: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Vertical, 4);
    let label = Label::new(Some(title));
    label.add_css_class("nm-section-title");
    label.set_halign(Align::Start);
    let entry = Entry::new();
    entry.set_placeholder_text(Some(hint));
    entry.add_css_class("nm-search");
    if let Some(v) = state.history.get_setting(key) {
        entry.set_text(&v);
    }
    let st = state.clone();
    entry.connect_activate(move |e| {
        let _ = st.history.set_setting(key, e.text().trim());
    });
    let save = Button::with_label("Save");
    save.add_css_class("nm-chip");
    save.set_halign(Align::Start);
    let st = state.clone();
    let entry_s = entry.clone();
    save.connect_clicked(move |_| {
        let _ = st.history.set_setting(key, entry_s.text().trim());
    });
    row.append(&label);
    row.append(&entry);
    row.append(&save);
    row
}

fn build_tools_page(state: Rc<AppState>, content: Stack) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 8);
    page.add_css_class("nm-main");
    let h = Label::new(Some("Tools"));
    h.add_css_class("nm-heading");
    h.set_halign(Align::Start);
    page.append(&h);
    page.append(&settings_row(
        &state,
        "Fanart API key",
        "fanart_api_key",
        "Optional fanart.tv key for artist backgrounds",
    ));
    page.append(&settings_row(
        &state,
        "Jellyfin ingest folder",
        "jellyfin_ingest_path",
        "Folder the server watches for acquired uploads",
    ));
    page.append(&settings_row(
        &state,
        "Subsonic ingest folder",
        "subsonic_ingest_path",
        "Folder the server watches for acquired uploads",
    ));
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
    let cache_btn = Button::with_label(&format!(
        "Clear web artwork cache ({:.1} MB)",
        cache::cache_bytes() as f64 / 1_048_576.0
    ));
    cache_btn.add_css_class("nm-chip");
    cache_btn.set_halign(Align::Start);
    cache_btn.connect_clicked(move |b| {
        let _ = cache::clear_web_art();
        b.set_label(&format!(
            "Clear web artwork cache ({:.1} MB)",
            cache::cache_bytes() as f64 / 1_048_576.0
        ));
    });
    page.append(&cache_btn);
    let keys = Label::new(Some(
        "Keys: Space play/pause · N/P next/prev · ←/→ seek · ↑/↓ volume · L like · S stage · Q queue · / search · Esc home · [ ] EQ band · -/= gain · B bypass",
    ));
    keys.add_css_class("nm-muted");
    keys.set_wrap(true);
    keys.set_halign(Align::Start);
    page.append(&keys);
    page
}

fn audio_sinks() -> Vec<(String, String)> {
    let Ok(out) = std::process::Command::new("wpctl").args(["status"]).output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut sinks = Vec::new();
    let mut in_sinks = false;
    for line in text.lines() {
        if line.contains("Sinks") {
            in_sinks = true;
            continue;
        }
        if in_sinks && (line.contains("Sources") || line.contains("Filters")) {
            break;
        }
        if !in_sinks {
            continue;
        }
        let Some(dot) = line.find('.') else { continue };
        let id: String = line[..dot].chars().filter(|c| c.is_ascii_digit()).collect();
        let name = line[dot + 1..].trim().trim_start_matches('*').trim();
        if !id.is_empty() && !name.is_empty() {
            sinks.push((id, name.to_string()));
        }
    }
    sinks
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
    box_.append(&Label::new(Some("Output")));
    let outputs = GtkBox::new(Orientation::Vertical, 2);
    box_.append(&outputs);
    let outputs_fill = outputs.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(10), move || {
        for (id, name) in audio_sinks() {
            let b = Button::with_label(&name);
            b.add_css_class("nm-nav-btn");
            b.set_halign(Align::Start);
            b.connect_clicked(move |_| {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-default", &id])
                    .status();
            });
            outputs_fill.append(&b);
        }
    });
    let open_eq = Button::with_label("Open Audiophile");
    open_eq.add_css_class("nm-chip");
    open_eq.connect_clicked(|_| {
        STAGE_STACK.with(|slot| {
            if let Some(stack) = slot.borrow().as_ref() {
                if let Some(content) = stack.parent() {
                    if let Some(content) = content.downcast_ref::<Stack>() {
                        content.set_visible_child_name("stage");
                    }
                }
                stack.set_visible_child_name("audiophile");
            }
        });
        CONTENT_STACK.with(|slot| {
            if let Some(stack) = slot.borrow().as_ref() {
                stack.set_visible_child_name("stage");
            }
        });
    });
    let bypass = Button::with_label(if state.player.eq_bypass() {
        "EQ bypassed"
    } else {
        "EQ active"
    });
    bypass.add_css_class("nm-chip");
    let st_b = state.clone();
    bypass.connect_clicked(move |b| {
        let on = !st_b.player.eq_bypass();
        st_b.player.set_eq_bypass(on);
        b.set_label(if on { "EQ bypassed" } else { "EQ active" });
    });
    box_.append(&open_eq);
    box_.append(&bypass);
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
        let local = Label::new(Some(
            "This computer stays on the system output. A second device is another zone and is not clock-synced.",
        ));
        local.set_wrap(true);
        local.add_css_class("nm-muted");
        local.set_max_width_chars(36);
        box_fill.append(&local);
        if devices.is_empty() {
            box_fill.append(&Label::new(Some("No cast devices found.")));
            return;
        }
        let active: Rc<RefCell<Option<CastDevice>>> = Rc::new(RefCell::new(None));
        for dev in devices {
            let row = GtkBox::new(Orientation::Vertical, 2);
            let btn = Button::with_label(&format!("{} ({})", dev.name, dev.protocol.label()));
            btn.add_css_class("nm-nav-btn");
            let controls = GtkBox::new(Orientation::Horizontal, 4);
            let status_b = Button::with_label("Status");
            status_b.add_css_class("nm-chip");
            let stop_b = Button::with_label("Stop");
            stop_b.add_css_class("nm-chip");
            let vol = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
            vol.set_value(40.0);
            vol.set_hexpand(true);
            let set_vol = Button::with_label("Volume");
            set_vol.add_css_class("nm-chip");
            controls.append(&status_b);
            controls.append(&stop_b);
            controls.append(&vol);
            controls.append(&set_vol);
            let st3 = st2.clone();
            let btn_status = btn.clone();
            let dev_play = dev.clone();
            let active_play = active.clone();
            btn.connect_clicked(move |_| {
                let Some(track) = st3.now.read().clone() else {
                    btn_status.set_label("Nothing is playing to cast.");
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
                if url.is_empty() {
                    btn_status.set_label(&format!("{} — failed: no stream URL", dev_play.name));
                    return;
                }
                btn_status.set_label("Sending…");
                let previous = active_play.borrow().clone();
                let dev_t = dev_play.clone();
                let title = track.title.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let mut prefix = String::new();
                    if let Some(prev) = previous {
                        if prev.id != dev_t.id {
                            match cast::cast_stop(&prev) {
                                Ok(()) => prefix = format!("Stopped {}. ", prev.name),
                                Err(err) => {
                                    prefix = format!("Previous device did not stop: {err:#}. ")
                                }
                            }
                        }
                    }
                    let sent = match cast::cast_url(&dev_t, &url, &title) {
                        Ok(()) => true,
                        Err(err) => {
                            let _ = tx.send((
                                false,
                                format!("{prefix}{} — failed: {err:#}", dev_t.name),
                            ));
                            return;
                        }
                    };
                    let word = if prefix.is_empty() { "sent" } else { "transferred" };
                    let _ = tx.send((sent, format!("{prefix}{} — {word}", dev_t.name)));
                });
                let label = btn_status.clone();
                let active_done = active_play.clone();
                let dev_done = dev_play.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                    match rx.try_recv() {
                        Ok((sent, text)) => {
                            if sent {
                                *active_done.borrow_mut() = Some(dev_done.clone());
                            }
                            label.set_label(&text);
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    }
                });
            });
            let dev_status = dev.clone();
            let label_status = btn.clone();
            status_b.connect_clicked(move |_| {
                label_status.set_label("Checking…");
                let dev_t = dev_status.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let text = match cast::cast_status(&dev_t) {
                        Ok(state) => format!("{} — {state}", dev_t.name),
                        Err(err) => format!("{} — status failed: {err:#}", dev_t.name),
                    };
                    let _ = tx.send(text);
                });
                let label = label_status.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                    match rx.try_recv() {
                        Ok(text) => {
                            label.set_label(&text);
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    }
                });
            });
            let dev_stop = dev.clone();
            let label_stop = btn.clone();
            let active_stop = active.clone();
            stop_b.connect_clicked(move |_| {
                label_stop.set_label("Stopping…");
                let dev_t = dev_stop.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let text = match cast::cast_stop(&dev_t) {
                        Ok(()) => format!("{} — stopped", dev_t.name),
                        Err(err) => format!("{} — stop failed: {err:#}", dev_t.name),
                    };
                    let _ = tx.send(text);
                });
                let label = label_stop.clone();
                let active_clear = active_stop.clone();
                let id = dev_stop.id.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                    match rx.try_recv() {
                        Ok(text) => {
                            if active_clear.borrow().as_ref().is_some_and(|d| d.id == id)
                                && text.contains("stopped")
                            {
                                *active_clear.borrow_mut() = None;
                            }
                            label.set_label(&text);
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    }
                });
            });
            let dev_vol = dev.clone();
            let label_vol = btn.clone();
            let vol_s = vol.clone();
            set_vol.connect_clicked(move |_| {
                let volume = vol_s.value().round() as u8;
                label_vol.set_label("Setting volume…");
                let dev_t = dev_vol.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let text = match cast::cast_volume(&dev_t, volume) {
                        Ok(()) => format!("{} — volume {volume}", dev_t.name),
                        Err(err) => format!("{} — volume failed: {err:#}", dev_t.name),
                    };
                    let _ = tx.send(text);
                });
                let label = label_vol.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                    match rx.try_recv() {
                        Ok(text) => {
                            label.set_label(&text);
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    }
                });
            });
            row.append(&btn);
            row.append(&controls);
            box_fill.append(&row);
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
    let liked: Vec<String> = state
        .taste
        .top_artists(8)
        .unwrap_or_default()
        .into_iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    std::thread::spawn(move || {
        let mut extra = be
            .similar_tracks(&id)
            .or_else(|_| be.instant_mix(&id))
            .unwrap_or_default();
        extra.sort_by_key(|track| {
            let artist = track.artist.to_lowercase();
            if liked.iter().any(|name| artist.contains(name.as_str()) || name.contains(&artist)) {
                0
            } else {
                1
            }
        });
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        continuum_until().store(now.saturating_add(12), std::sync::atomic::Ordering::Relaxed);
    });
}

/// True when `widget` (or an ancestor) is somewhere the user types text.
fn is_text_input(widget: &gtk4::Widget) -> bool {
    let mut cur = Some(widget.clone());
    while let Some(w) = cur {
        if w.is::<gtk4::Editable>() || w.is::<gtk4::TextView>() {
            return true;
        }
        cur = w.parent();
    }
    false
}

fn install_shortcuts(state: Rc<AppState>, content: Stack) {
    let controller = gtk4::EventControllerKey::new();
    let st = state.clone();
    let content_keys = content.clone();
    controller.connect_key_pressed(move |_, key, _code, _mods| {
        use gtk4::gdk::Key;
        match key {
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
                    if st.taste.is_liked(&t.id) {
                        let _ = st.taste.unlike(&t.id);
                        if let Some(be) = st.backend.read().as_ref() {
                            let _ = be.set_favorite(&t.id, false);
                        }
                    } else {
                        let _ = st.taste.like(&t);
                        if let Some(be) = st.backend.read().as_ref() {
                            let _ = be.set_favorite(&t.id, true);
                        }
                        if ytm::is_ytm_track(&t) {
                            let uploader = st.backend.read().clone().map(|be| {
                                Arc::new(BackendUploader { backend: be }) as Arc<dyn LibraryUploader>
                            });
                            acquire::spawn_like_acquire(st.acquire.clone(), t, uploader);
                        }
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
            Key::Left => {
                let pos = st.player.position_secs().saturating_sub(10);
                let _ = st.player.seek_secs(pos);
                glib::Propagation::Stop
            }
            Key::Right => {
                let _ = st.player.seek_secs(st.player.position_secs().saturating_add(10));
                glib::Propagation::Stop
            }
            Key::Up => {
                st.player.set_volume((st.player.volume() + 0.05).min(1.0));
                glib::Propagation::Stop
            }
            Key::Down => {
                st.player.set_volume((st.player.volume() - 0.05).max(0.0));
                glib::Propagation::Stop
            }
            Key::q | Key::Q => {
                content_keys.set_visible_child_name("queue");
                glib::Propagation::Stop
            }
            Key::Escape => {
                content_keys.set_visible_child_name("home");
                glib::Propagation::Stop
            }
            Key::bracketleft => {
                crate::eq::shared_eq().lock().select_step(-1);
                glib::Propagation::Stop
            }
            Key::bracketright => {
                crate::eq::shared_eq().lock().select_step(1);
                glib::Propagation::Stop
            }
            Key::equal | Key::plus => {
                let db = crate::eq::shared_eq().lock().nudge_selected(0.5);
                let index = crate::eq::shared_eq().lock().selected();
                st.player.set_eq_band(index, db);
                glib::Propagation::Stop
            }
            Key::minus => {
                let db = crate::eq::shared_eq().lock().nudge_selected(-0.5);
                let index = crate::eq::shared_eq().lock().selected();
                st.player.set_eq_band(index, db);
                glib::Propagation::Stop
            }
            Key::b | Key::B => {
                let on = !st.player.eq_bypass();
                st.player.set_eq_bypass(on);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    content.add_controller(controller);
}
