use crate::connect::{ConnectProgress, ConnectState};
use crate::dbus;
use crate::media;
use crate::mount;
use crate::place::{DiscoveredHost, MountPolicy, Place, Protocol};
use crate::{APP_ID, APP_NAME};
use gtk4::gdk::{Key, ModifierType};
use gtk4::glib::{self, clone};
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation,
    ScrolledWindow, Stack, StackTransitionType,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::activity::ActivityPanel;
use super::connect_dialog::ConnectDialog;
use super::connect_status::InlineConnectStatus;
use super::discovery::DiscoveryPanel;
use super::settings;
use super::style;
use super::wizard;

enum RefreshEvent {
    Log(String),
    Places(Vec<Place>),
    Discovered(Vec<DiscoveredHost>),
    Done,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Places,
    Settings,
}

struct AppState {
    places: Vec<Place>,
    page: Page,
    selected_id: Option<String>,
    admin_until: Option<Instant>,
    /// True while a BeginAdminSession call is in flight (avoids re-entry without granting early).
    admin_pending: bool,
    pending_mount_id: Option<String>,
    /// Live connect progress keyed by place id (survives card rebuilds).
    connects: HashMap<String, ConnectProgress>,
    /// Place ids currently connecting — allows unlimited parallel mounts.
    connecting: HashMap<String, bool>,
}

pub fn run_app() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let settings = dbus::get_settings_sync().unwrap_or_default();
    style::load(&settings.ui_density);

    let state = Rc::new(RefCell::new(AppState {
        places: Vec::new(),
        page: Page::Places,
        selected_id: None,
        admin_until: None,
        admin_pending: false,
        pending_mount_id: None,
        connects: HashMap::new(),
        connecting: HashMap::new(),
    }));

    let window = ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(1024)
        .default_height(680)
        .build();
    window.add_css_class("nn-window");

    let root = GtkBox::new(Orientation::Vertical, 0);
    window.set_child(Some(&root));

    // Daemon gate
    let daemon_gate = build_daemon_gate();
    root.append(&daemon_gate);

    let main_ui = GtkBox::new(Orientation::Vertical, 0);
    main_ui.set_hexpand(true);
    main_ui.set_vexpand(true);
    root.append(&main_ui);

    let alive = dbus::daemon_alive();
    daemon_gate.set_visible(!alive);
    main_ui.set_visible(alive);

    // Command bar
    let cmd = GtkBox::new(Orientation::Horizontal, 4);
    cmd.add_css_class("nn-commandbar");
    let add_btn = Button::with_label("Add place");
    add_btn.add_css_class("nn-accent");
    let discover_btn = Button::with_label("Browse network");
    let refresh_btn = Button::with_label("Refresh");
    let disconnect_btn = Button::with_label("Disconnect");
    let open_btn = Button::with_label("Open in Files");
    let web_btn = Button::with_label("Open web app");
    web_btn.set_visible(false);
    for b in [
        &add_btn,
        &discover_btn,
        &refresh_btn,
        &open_btn,
        &web_btn,
        &disconnect_btn,
    ] {
        cmd.append(b);
    }
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    cmd.append(&spacer);
    // Match MDE Settings / discovery panel: flat window-close icon that dismisses the window.
    let close_btn = Button::from_icon_name("window-close-symbolic");
    close_btn.add_css_class("flat");
    close_btn.set_tooltip_text(Some("Close"));
    close_btn.set_valign(Align::Center);
    close_btn.connect_clicked({
        let window = window.clone();
        move |_| {
            window.close();
        }
    });
    cmd.append(&close_btn);
    main_ui.append(&cmd);

    let activity = Rc::new(ActivityPanel::new());
    main_ui.append(&activity.revealer);

    // Body: sidebar | content | discovery
    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.set_vexpand(true);
    body.set_hexpand(true);
    main_ui.append(&body);

    let sidebar = GtkBox::new(Orientation::Vertical, 4);
    sidebar.add_css_class("nn-sidebar");
    sidebar.set_size_request(220, -1);
    sidebar.append(&Label::builder()
        .label("NETWORK")
        .halign(Align::Start)
        .css_classes(["nn-nav-title"])
        .build());
    let nav_places = Button::with_label("My places");
    nav_places.add_css_class("nn-nav-btn");
    nav_places.add_css_class("nn-active");
    let nav_add = Button::with_label("Add place");
    nav_add.add_css_class("nn-nav-btn");
    let nav_settings = Button::with_label("Settings");
    nav_settings.add_css_class("nn-nav-btn");
    sidebar.append(&nav_places);
    sidebar.append(&nav_add);
    sidebar.append(&nav_settings);
    body.append(&sidebar);

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::Crossfade);
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    let places_page = GtkBox::new(Orientation::Vertical, 0);
    places_page.set_hexpand(true);
    places_page.set_vexpand(true);
    let places_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    let places_list = GtkBox::new(Orientation::Vertical, 4);
    places_list.set_margin_top(8);
    places_list.set_margin_bottom(16);
    places_scroll.set_child(Some(&places_list));
    places_page.append(&places_scroll);

    let (empty, empty_add) = build_empty_state();
    places_page.append(&empty);

    stack.add_named(&places_page, Some("places"));

    let refresh_settings_style = {
        Rc::new(move || {
            if let Ok(s) = dbus::get_settings_sync() {
                style::load(&s.ui_density);
            }
        })
    };
    let settings_page = settings::build_settings_page(refresh_settings_style);
    stack.add_named(&settings_page, Some("settings"));

    body.append(&stack);

    let discovery = Rc::new(DiscoveryPanel::new());
    body.append(&discovery.revealer);

    let refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let refresh_busy = Rc::new(Cell::new(false));
    let refresh_btn_ref = refresh_btn.clone();
    // Live inline status widgets — updated during connect without rebuilding the list.
    let connect_widgets: Rc<RefCell<HashMap<String, Rc<InlineConnectStatus>>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let apply_places = {
        let state = state.clone();
        let places_list = places_list.clone();
        let empty = empty.clone();
        let places_scroll = places_scroll.clone();
        let window = window.clone();
        let web_btn = web_btn.clone();
        let open_btn = open_btn.clone();
        let disconnect_btn = disconnect_btn.clone();
        let refresh_holder = refresh_holder.clone();
        let connect_widgets = connect_widgets.clone();
        let activity = activity.clone();
        Rc::new(move |places: Vec<Place>| {
            {
                let mut st = state.borrow_mut();
                st.places = places;
            }
            let st = state.borrow();
            let places = st.places.clone();
            let selected = st.selected_id.clone();
            let connects = st.connects.clone();
            drop(st);

            while let Some(c) = places_list.first_child() {
                places_list.remove(&c);
            }
            connect_widgets.borrow_mut().clear();
            let is_empty = places.is_empty();
            empty.set_visible(is_empty);
            places_scroll.set_visible(!is_empty);

            let refresh = refresh_holder
                .borrow()
                .clone()
                .unwrap_or_else(|| Rc::new(|| {}));

            for place in &places {
                let card = build_place_card(
                    place,
                    selected.as_deref() == Some(place.id.as_str()),
                    &window,
                    state.clone(),
                    refresh.clone(),
                    connect_widgets.clone(),
                    activity.clone(),
                    connects.get(&place.id).cloned(),
                );
                places_list.append(&card);
            }

            if let Some(id) = &selected {
                if let Some(p) = places.iter().find(|p| p.id == *id) {
                    web_btn.set_visible(matches!(p.protocol, Protocol::Video | Protocol::Audio));
                    open_btn.set_sensitive(true);
                    disconnect_btn.set_sensitive(true);
                }
            } else {
                web_btn.set_visible(false);
                open_btn.set_sensitive(!places.is_empty());
                disconnect_btn.set_sensitive(!places.is_empty());
            }
        }) as Rc<dyn Fn(Vec<Place>)>
    };

    let do_refresh = {
        let apply_places = apply_places.clone();
        let refresh_busy = refresh_busy.clone();
        let refresh_btn_ref = refresh_btn_ref.clone();
        let activity = activity.clone();
        let discovery = discovery.clone();
        Rc::new(move || {
            if refresh_busy.get() {
                return;
            }
            refresh_busy.set(true);
            refresh_btn_ref.set_sensitive(false);
            refresh_btn_ref.set_label("Refreshing…");
            activity.show("Refresh");
            activity.append("Starting refresh…");

            let (tx, rx) = std::sync::mpsc::channel::<RefreshEvent>();
            std::thread::spawn(move || {
                let tx_log = tx.clone();
                let log = |msg: &str| {
                    let _ = tx_log.send(RefreshEvent::Log(msg.to_string()));
                };

                log("Reloading saved places from daemon…");
                match dbus::list_places_sync() {
                    Ok(places) => {
                        log(&format!("  Loaded {} place(s).", places.len()));
                        let _ = tx.send(RefreshEvent::Places(places));
                    }
                    Err(e) => {
                        log(&format!("  Place reload failed: {e}"));
                        let _ = tx.send(RefreshEvent::Places(Vec::new()));
                    }
                }

                log("Reading neighbourhood discovery database…");
                match dbus::list_discovered_sync() {
                    Ok(cache) => {
                        if let Some(age) = cache.age_secs() {
                            log(&format!(
                                "  Cache: {} host(s), last scan {}s ago.",
                                cache.hosts.len(),
                                age
                            ));
                        } else {
                            log(&format!(
                                "  Cache: {} host(s) (no completed scan yet).",
                                cache.hosts.len()
                            ));
                        }
                        if !cache.last_status.is_empty() {
                            log(&format!("  Status: {}", cache.last_status));
                        }
                        if cache.scanning {
                            log("  Background scanner is currently running…");
                        }
                        if !cache.last_error.is_empty() {
                            log(&format!("  Last error: {}", cache.last_error));
                        }
                        let _ = tx.send(RefreshEvent::Discovered(cache.hosts));
                    }
                    Err(e) => log(&format!("  Cache read failed: {e}")),
                }

                // If the daemon is already scanning, follow that pass. Otherwise kick Discover.
                let already = dbus::get_discovery_status_sync()
                    .ok()
                    .and_then(|st| st.get("scanning").and_then(|v| v.as_bool()))
                    .unwrap_or(false);

                if already {
                    log("Background scanner already running — following its progress…");
                } else {
                    log("Requesting a fresh discovery pass from network-neighbourhoodd…");
                    let _ = std::thread::spawn(|| {
                        let _ = dbus::discover_sync();
                    });
                }

                let tx_poll = tx.clone();
                let mut last_line = String::new();
                let mut saw_scanning = already;
                let started = std::time::Instant::now();
                loop {
                    let st = dbus::get_discovery_status_sync().unwrap_or(serde_json::json!({}));
                    let scanning = st
                        .get("scanning")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if scanning {
                        saw_scanning = true;
                    }
                    if let Some(s) = st.get("last_status").and_then(|v| v.as_str()) {
                        if !s.is_empty() && s != last_line {
                            last_line = s.to_string();
                            let _ = tx_poll.send(RefreshEvent::Log(format!("  {s}")));
                        }
                    }
                    if saw_scanning && !scanning {
                        if let Ok(cache) = dbus::list_discovered_sync() {
                            let _ = tx_poll.send(RefreshEvent::Log(format!(
                                "Scan settled — {} host(s) in database.",
                                cache.hosts.len()
                            )));
                            let _ = tx.send(RefreshEvent::Discovered(cache.hosts));
                        }
                        let _ = tx.send(RefreshEvent::Done);
                        break;
                    }
                    // Discover call returned immediately without flipping scanning (rare).
                    if !saw_scanning && started.elapsed() > Duration::from_secs(8) {
                        if let Ok(cache) = dbus::list_discovered_sync() {
                            let _ = tx.send(RefreshEvent::Discovered(cache.hosts));
                        }
                        let _ = tx.send(RefreshEvent::Done);
                        break;
                    }
                    if started.elapsed() > Duration::from_secs(180) {
                        let _ = tx.send(RefreshEvent::Failed(
                            "Discovery timed out after 3 minutes — try again later.".into(),
                        ));
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(400));
                }
            });

            let apply_places = apply_places.clone();
            let refresh_busy = refresh_busy.clone();
            let refresh_btn_ref = refresh_btn_ref.clone();
            let activity = activity.clone();
            let discovery = discovery.clone();
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let mut keep_going = true;
                while keep_going {
                    match rx.try_recv() {
                        Ok(RefreshEvent::Log(line)) => {
                            activity.append(&line);
                        }
                        Ok(RefreshEvent::Places(places)) => {
                            apply_places(places);
                        }
                        Ok(RefreshEvent::Discovered(hosts)) => {
                            discovery.apply_hosts(hosts);
                        }
                        Ok(RefreshEvent::Done) => {
                            activity.append("Refresh complete.");
                            activity.set_title("Refresh — complete");
                            refresh_busy.set(false);
                            refresh_btn_ref.set_sensitive(true);
                            refresh_btn_ref.set_label("Refresh");
                            return glib::ControlFlow::Break;
                        }
                        Ok(RefreshEvent::Failed(e)) => {
                            activity.append(&format!("Refresh failed: {e}"));
                            activity.set_title("Refresh — failed");
                            refresh_busy.set(false);
                            refresh_btn_ref.set_sensitive(true);
                            refresh_btn_ref.set_label("Refresh");
                            return glib::ControlFlow::Break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            keep_going = false;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            activity.append("Refresh worker exited unexpectedly.");
                            refresh_busy.set(false);
                            refresh_btn_ref.set_sensitive(true);
                            refresh_btn_ref.set_label("Refresh");
                            return glib::ControlFlow::Break;
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
        }) as Rc<dyn Fn()>
    };
    *refresh_holder.borrow_mut() = Some(do_refresh.clone());

    empty_add.connect_clicked(clone!(
        #[strong]
        window,
        #[strong]
        do_refresh,
        #[strong]
        state,
        move |_| {
            ensure_admin(&state);
            wizard::run_add_wizard(&window, do_refresh.clone());
        }
    ));

    add_btn.connect_clicked(clone!(
        #[strong]
        window,
        #[strong]
        do_refresh,
        #[strong]
        state,
        move |_| {
            ensure_admin(&state);
            wizard::run_add_wizard(&window, do_refresh.clone());
        }
    ));
    nav_add.connect_clicked(clone!(
        #[strong]
        window,
        #[strong]
        do_refresh,
        #[strong]
        state,
        move |_| {
            ensure_admin(&state);
            wizard::run_add_wizard(&window, do_refresh.clone());
        }
    ));

    discover_btn.connect_clicked({
        let discovery = discovery.clone();
        move |_| discovery.toggle()
    });

    discovery.connect_activate(&window, {
        let window = window.clone();
        let do_refresh = do_refresh.clone();
        move |host| {
            wizard::run_add_from_discovered(&window, &host, do_refresh.clone());
        }
    });

    refresh_btn.connect_clicked({
        let do_refresh = do_refresh.clone();
        move |_| do_refresh()
    });

    open_btn.connect_clicked({
        let state = state.clone();
        let window = window.clone();
        let do_refresh = do_refresh.clone();
        let connect_widgets = connect_widgets.clone();
        move |_| {
            let id = state.borrow().selected_id.clone().or_else(|| {
                state.borrow().places.first().map(|p| p.id.clone())
            });
            if let Some(id) = id {
                open_place(
                    &window,
                    &state,
                    &id,
                    do_refresh.clone(),
                    connect_widgets.clone(),
                );
            }
        }
    });

    web_btn.connect_clicked({
        let state = state.clone();
        move |_| {
            let st = state.borrow();
            if let Some(id) = &st.selected_id {
                if let Some(p) = st.places.iter().find(|p| p.id == *id) {
                    let _ = media::open_in_browser(p);
                }
            }
        }
    });

    disconnect_btn.connect_clicked({
        let state = state.clone();
        let window = window.clone();
        let do_refresh = do_refresh.clone();
        move |_| {
            let st = state.borrow();
            let place = st
                .selected_id
                .as_ref()
                .and_then(|id| st.places.iter().find(|p| p.id == *id))
                .cloned()
                .or_else(|| st.places.first().cloned());
            drop(st);
            if let Some(p) = place {
                ensure_admin(&state);
                wizard::run_disconnect_chooser(&window, &p, do_refresh.clone());
            }
        }
    });

    nav_places.connect_clicked({
        let stack = stack.clone();
        let state = state.clone();
        let nav_places = nav_places.clone();
        let nav_settings = nav_settings.clone();
        move |_| {
            state.borrow_mut().page = Page::Places;
            stack.set_visible_child_name("places");
            nav_places.add_css_class("nn-active");
            nav_settings.remove_css_class("nn-active");
        }
    });
    nav_settings.connect_clicked({
        let stack = stack.clone();
        let state = state.clone();
        let nav_places = nav_places.clone();
        let nav_settings = nav_settings.clone();
        move |_| {
            state.borrow_mut().page = Page::Settings;
            stack.set_visible_child_name("settings");
            nav_settings.add_css_class("nn-active");
            nav_places.remove_css_class("nn-active");
        }
    });

    // Daemon gate retry
    wire_daemon_gate(&daemon_gate, &main_ui, do_refresh.clone());

    // Shortcuts
    install_shortcuts(
        &window,
        &settings,
        state.clone(),
        do_refresh.clone(),
        discovery.clone(),
        stack.clone(),
        nav_places.clone(),
        nav_settings.clone(),
        connect_widgets.clone(),
    );

    if alive {
        // Fast startup: places + cached neighbourhood DB (no forced full scan).
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let places = dbus::list_places_sync().unwrap_or_default();
            let cache = dbus::list_discovered_sync().unwrap_or_default();
            let _ = tx.send((places, cache));
        });
        let apply_places2 = apply_places.clone();
        let discovery2 = discovery.clone();
        let activity2 = activity.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
            Ok((places, cache)) => {
                apply_places2(places);
                discovery2.apply_hosts(cache.hosts.clone());
                if !cache.hosts.is_empty() || cache.updated_at > 0 {
                    activity2.show("Neighbourhood database");
                    let age = cache
                        .age_secs()
                        .map(|s| format!("{s}s ago"))
                        .unwrap_or_else(|| "never".into());
                    activity2.append(&format!(
                        "Loaded {} discovered host(s) (last scan {age}).",
                        cache.hosts.len()
                    ));
                    if !cache.last_status.is_empty() {
                        activity2.append(&cache.last_status);
                    }
                    if cache.scanning {
                        activity2.append("Background scanner is running…");
                    }
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    window.present();
}

fn ensure_admin(state: &Rc<RefCell<AppState>>) {
    let settings = dbus::get_settings_sync().unwrap_or_default();
    let secs = settings.admin_session_sec.max(60);
    {
        let st = state.borrow();
        if matches!(st.admin_until, Some(until) if Instant::now() < until) {
            return;
        }
        if st.admin_pending {
            return;
        }
    }
    state.borrow_mut().admin_pending = true;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let ok = dbus::call_sync("BeginAdminSession", "").is_ok();
        let _ = tx.send(ok);
    });
    let state = state.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Ok(ok) => {
            let mut st = state.borrow_mut();
            st.admin_pending = false;
            if ok {
                st.admin_until = Some(Instant::now() + Duration::from_secs(secs));
            } else {
                st.admin_until = None;
            }
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => {
            state.borrow_mut().admin_pending = false;
            state.borrow_mut().admin_until = None;
            glib::ControlFlow::Break
        }
    });
}

thread_local! {
    /// Places where the user said "Not now" this session; don't nag.
    static KEY_OFFER_DECLINED: RefCell<std::collections::HashSet<String>> =
        RefCell::new(std::collections::HashSet::new());
}

/// After a successful password connect to an SSH place, offer passwordless keys.
fn offer_key_setup(window: &ApplicationWindow, place: &Place) {
    if place.protocol != crate::place::Protocol::Ssh
        || KEY_OFFER_DECLINED.with(|d| d.borrow().contains(&place.id))
    {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let id = place.id.clone();
    std::thread::spawn(move || {
        let _ = tx.send(dbus::has_ssh_key_sync(&id).unwrap_or(true));
    });
    let window = window.clone();
    let place = place.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || match rx.try_recv() {
        Ok(false) => {
            ask_key_setup(&window, &place);
            glib::ControlFlow::Break
        }
        Ok(true) | Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
    });
}

fn ask_key_setup(window: &ApplicationWindow, place: &Place) {
    let alert = gtk4::AlertDialog::builder()
        .modal(true)
        .message("Set up passwordless access?")
        .detail(format!(
            "Connected to {} with a password. Network Neighbourhood can create a \
             private key for this place and install it on the host, so future \
             connections need no password.",
            place.name
        ))
        .buttons(["Not now", "Set up keys"])
        .cancel_button(0)
        .default_button(1)
        .build();
    let window2 = window.clone();
    let place = place.clone();
    alert.choose(Some(window), gio::Cancellable::NONE, move |res| {
        if res.ok() != Some(1) {
            KEY_OFFER_DECLINED.with(|d| d.borrow_mut().insert(place.id.clone()));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let id = place.id.clone();
        std::thread::spawn(move || {
            let _ = tx.send(dbus::setup_ssh_key_sync(&id).map_err(|e| e.to_string()));
        });
        let window3 = window2.clone();
        glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
            Ok(r) => {
                let (msg, detail) = match r {
                    Ok(()) => (
                        "Passwordless access is ready".to_string(),
                        "A key was created, installed and verified. This place will use it from now on.".to_string(),
                    ),
                    Err(e) => (
                        "Couldn't set up keys".to_string(),
                        format!("{e}\n\nYour password connection still works."),
                    ),
                };
                gtk4::AlertDialog::builder()
                    .modal(true)
                    .message(msg)
                    .detail(detail)
                    .build()
                    .show(Some(&window3));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    });
}

fn open_place(
    window: &ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    id: &str,
    refresh: Rc<dyn Fn()>,
    connect_widgets: Rc<RefCell<HashMap<String, Rc<InlineConnectStatus>>>>,
) {
    ensure_admin(state);
    let place = {
        let st = state.borrow();
        st.places.iter().find(|p| p.id == id).cloned()
    };
    let Some(place) = place else { return };
    state.borrow_mut().pending_mount_id = Some(id.to_string());
    state.borrow_mut().selected_id = Some(id.to_string());

    let apply_inline = |widgets: &Rc<RefCell<HashMap<String, Rc<InlineConnectStatus>>>>,
                        place_id: &str,
                        progress: &ConnectProgress| {
        if let Some(w) = widgets.borrow().get(place_id) {
            w.apply(progress);
        }
    };

    // Already mounted → short status, no expand log (decision 8B).
    if mount::is_mounted(&place.mount_point()) {
        let mut progress = ConnectProgress::new(&place.id, &place.mount_point());
        progress.state = ConnectState::AlreadyMounted;
        progress.phrase = format!("Already mounted — {}", place.mount_point());
        progress.mount_point = place.mount_point();
        progress.fraction = 1.0;
        apply_inline(&connect_widgets, &place.id, &progress);
        state
            .borrow_mut()
            .connects
            .insert(place.id.clone(), progress);
        let _ = mount::open_in_files(&place.mount_point());
        return;
    }

    // Unlimited parallel connects (decision 6C) — skip if this place is already connecting.
    if state.borrow().connecting.get(id).copied().unwrap_or(false) {
        return;
    }

    if place.mount_policy == MountPolicy::MountWhenOpened
        || place.mount_policy == MountPolicy::MountNow
        || place.mount_policy == MountPolicy::Automount
    {
        let mut kickoff = ConnectProgress::new(&place.id, &place.mount_point());
        kickoff.state = ConnectState::Connecting;
        kickoff.phrase = format!("Connecting to {}…", place.name);
        kickoff.fraction = 0.05;
        apply_inline(&connect_widgets, &place.id, &kickoff);
        state
            .borrow_mut()
            .connects
            .insert(place.id.clone(), kickoff);
        state
            .borrow_mut()
            .connecting
            .insert(place.id.clone(), true);

        let id = id.to_string();
        let state2 = state.clone();
        let refresh2 = refresh.clone();
        let place_mp = place.mount_point();
        let window2 = window.clone();
        let place2 = place.clone();
        let widgets = connect_widgets.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

        // Modal Connecting dialog (checklist, progress, log, Cancel; diagnosis on failure).
        let dialog = ConnectDialog::new(window, &place.name, &place.host);
        dialog.set_on_cancel({
            let id = id.clone();
            move || {
                let _ = dbus::cancel_connect_sync(&id);
            }
        });
        dialog.set_on_retry({
            let window = window.clone();
            let state = state.clone();
            let refresh = refresh.clone();
            let widgets = connect_widgets.clone();
            let id = id.clone();
            move || {
                state.borrow_mut().connecting.remove(&id);
                open_place(&window, &state, &id, refresh.clone(), widgets.clone());
            }
        });
        dialog.set_on_edit({
            let window = window.clone();
            let place = place.clone();
            let refresh = refresh.clone();
            let state = state.clone();
            let id = id.clone();
            move || {
                state.borrow_mut().connecting.remove(&id);
                wizard::run_edit_credentials(&window, &place, refresh.clone());
            }
        });
        dialog.present();

        std::thread::spawn({
            let id = id.clone();
            move || {
                let result = dbus::mount_place_sync(&id).map_err(|e| e.to_string());
                let _ = tx.send(result);
            }
        });

        glib::timeout_add_local(Duration::from_millis(150), move || {
            if let Ok(progress) = dbus::get_connect_status_sync(&id) {
                if progress.state != ConnectState::Idle {
                    if let Some(w) = widgets.borrow().get(&id) {
                        w.apply(&progress);
                    }
                    dialog.apply(&progress);
                    state2
                        .borrow_mut()
                        .connects
                        .insert(id.clone(), progress);
                }
            }

            match rx.try_recv() {
                Ok(Ok(_log)) => {
                    let progress = dbus::get_connect_status_sync(&id).unwrap_or_else(|_| {
                        let mut p = ConnectProgress::new(&id, &place_mp);
                        p.state = ConnectState::Connected;
                        p.phrase = format!("Connected — {place_mp}");
                        p.mount_point = place_mp.clone();
                        p.fraction = 1.0;
                        p
                    });
                    if let Some(w) = widgets.borrow().get(&id) {
                        w.apply(&progress);
                    }
                    dialog.close();
                    offer_key_setup(&window2, &place2);
                    state2.borrow_mut().connecting.remove(&id);
                    state2.borrow_mut().connects.insert(id.clone(), progress);
                    if mount::is_mounted(&place_mp) {
                        let _ = mount::open_in_files(&place_mp);
                    }
                    // Rebuild once so badge / Connect button flip to Mounted.
                    refresh2();
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    let mut progress = dbus::get_connect_status_sync(&id).unwrap_or_else(|_| {
                        ConnectProgress::new(&id, &place_mp)
                    });
                    if progress.state != ConnectState::Failed
                        && progress.state != ConnectState::Cancelled
                    {
                        progress.state = ConnectState::Failed;
                        progress.phrase = format!("Failed — {e}");
                        progress.diagnosis = crate::diagnose::classify(&e);
                        progress.error = e;
                    }
                    if let Some(w) = widgets.borrow().get(&id) {
                        w.apply(&progress);
                    }
                    dialog.apply(&progress);
                    state2.borrow_mut().connecting.remove(&id);
                    state2.borrow_mut().connects.insert(id.clone(), progress);
                    refresh2();
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let mut progress = ConnectProgress::new(&id, &place_mp);
                    progress.state = ConnectState::Failed;
                    progress.phrase = "Failed — mount worker exited".into();
                    progress.error = "Mount worker exited unexpectedly.".into();
                    if let Some(w) = widgets.borrow().get(&id) {
                        w.apply(&progress);
                    }
                    dialog.apply(&progress);
                    state2.borrow_mut().connecting.remove(&id);
                    state2.borrow_mut().connects.insert(id.clone(), progress);
                    refresh2();
                    glib::ControlFlow::Break
                }
            }
        });
    }
}

fn build_place_card(
    place: &Place,
    selected: bool,
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    refresh: Rc<dyn Fn()>,
    connect_widgets: Rc<RefCell<HashMap<String, Rc<InlineConnectStatus>>>>,
    activity: Rc<ActivityPanel>,
    saved_progress: Option<ConnectProgress>,
) -> GtkBox {
    // Use a box (not a Button) so action buttons inside remain clickable.
    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("nn-card");
    if selected {
        card.add_css_class("nn-active");
    }
    card.set_hexpand(true);

    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.set_halign(Align::Fill);

    let icon = gtk4::Image::from_icon_name(place.protocol.icon_name());
    icon.set_pixel_size(32);
    row.append(&icon);

    let text = GtkBox::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&Label::builder()
        .label(&place.name)
        .halign(Align::Start)
        .build());
    text.append(&Label::builder()
        .label(&place.mount_point())
        .halign(Align::Start)
        .css_classes(["nn-path"])
        .selectable(true)
        .build());
    let remote = if place.url.is_empty() {
        format!(
            "{}://{}/{}",
            place.protocol.as_str(),
            place.host,
            place.path
        )
    } else {
        place.url.clone()
    };
    text.append(&Label::builder()
        .label(&remote)
        .halign(Align::Start)
        .css_classes(["nn-hero-sub"])
        .build());
    row.append(&text);

    let mounted = mount::is_mounted(&place.mount_point());
    let has_error = !place.last_error.is_empty() && !mounted;
    let badge = Label::builder()
        .label(if mounted {
            "Mounted"
        } else if has_error {
            "Error"
        } else {
            "Available"
        })
        .css_classes([if mounted {
            "nn-badge-ok"
        } else if has_error {
            "nn-badge-off"
        } else {
            "nn-badge"
        }])
        .valign(Align::Center)
        .build();
    row.append(&badge);
    card.append(&row);

    // Inline connect status (phrase + chevron log + progress + failure actions).
    let status = Rc::new(InlineConnectStatus::new());
    card.append(&status.root);
    if let Some(progress) = &saved_progress {
        status.apply(progress);
    } else if has_error {
        let mut p = ConnectProgress::new(&place.id, &place.mount_point());
        p.state = ConnectState::Failed;
        p.phrase = format!("Failed — {}", place.last_error);
        p.error = place.last_error.clone();
        status.apply(&p);
    } else if mounted {
        let mut p = ConnectProgress::new(&place.id, &place.mount_point());
        p.state = ConnectState::Connected;
        p.phrase = format!("Connected — {}", place.mount_point());
        p.mount_point = place.mount_point();
        p.fraction = 1.0;
        status.apply(&p);
    } else {
        status.root.set_visible(false);
    }
    connect_widgets
        .borrow_mut()
        .insert(place.id.clone(), status.clone());

    status.connect_cancel({
        let id = place.id.clone();
        let state = state.clone();
        let status = status.clone();
        let refresh = refresh.clone();
        move || {
            let _ = dbus::cancel_connect_sync(&id);
            let mut progress = ConnectProgress::new(&id, "");
            progress.state = ConnectState::Cancelled;
            progress.phrase = "Cancelled".into();
            progress.error = "Connect cancelled — cleaned up".into();
            status.apply(&progress);
            state.borrow_mut().connecting.remove(&id);
            state.borrow_mut().connects.insert(id.clone(), progress);
            refresh();
        }
    });

    let id = place.id.clone();
    status.retry_btn.connect_clicked({
        let window = window.clone();
        let state = state.clone();
        let refresh = refresh.clone();
        let widgets = connect_widgets.clone();
        let id = id.clone();
        move |_| {
            state.borrow_mut().selected_id = Some(id.clone());
            open_place(
                &window,
                &state,
                &id,
                refresh.clone(),
                widgets.clone(),
            );
        }
    });
    status.edit_btn.connect_clicked({
        let window = window.clone();
        let place = place.clone();
        let refresh = refresh.clone();
        move |_| {
            wizard::run_edit_credentials(&window, &place, refresh.clone());
        }
    });
    status.forget_btn.connect_clicked({
        let window = window.clone();
        let place = place.clone();
        let refresh = refresh.clone();
        move |_| {
            wizard::run_disconnect_chooser(&window, &place, refresh.clone());
        }
    });
    status.copy_btn.connect_clicked({
        let status = status.clone();
        move |_| {
            let text = status.error_text();
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&text);
            }
        }
    });
    status.logs_btn.connect_clicked({
        let id = place.id.clone();
        let activity = activity.clone();
        move |_| {
            activity.show("Mount logs");
            match dbus::open_mount_logs_sync(&id) {
                Ok(logs) if !logs.trim().is_empty() => {
                    for line in logs.lines().take(80) {
                        activity.append(line);
                    }
                }
                Ok(_) => activity.append("No journal entries for this mount unit yet."),
                Err(e) => activity.append(&format!("Could not read logs: {e}")),
            }
        }
    });

    let actions = GtkBox::new(Orientation::Horizontal, 6);
    actions.set_halign(Align::Start);

    let open_btn = Button::with_label(if mounted { "Open in Files" } else { "Connect" });
    open_btn.add_css_class("nn-accent");
    let retry_btn = Button::with_label("Retry");
    let creds_btn = Button::with_label("Edit credentials");
    let delete_btn = Button::with_label("Delete");
    delete_btn.add_css_class("destructive-action");

    retry_btn.set_visible(has_error || !mounted);
    for b in [&open_btn, &retry_btn, &creds_btn, &delete_btn] {
        actions.append(b);
    }

    if matches!(place.protocol, Protocol::Video | Protocol::Audio) {
        let open_web = Button::with_label("Open web app");
        let p = place.clone();
        open_web.connect_clicked(move |_| {
            let _ = media::open_in_browser(&p);
        });
        actions.append(&open_web);
    }

    card.append(&actions);

    let id = place.id.clone();
    open_btn.connect_clicked({
        let window = window.clone();
        let state = state.clone();
        let refresh = refresh.clone();
        let widgets = connect_widgets.clone();
        let id = id.clone();
        move |_| {
            state.borrow_mut().selected_id = Some(id.clone());
            open_place(
                &window,
                &state,
                &id,
                refresh.clone(),
                widgets.clone(),
            );
        }
    });
    retry_btn.connect_clicked({
        let window = window.clone();
        let state = state.clone();
        let refresh = refresh.clone();
        let widgets = connect_widgets.clone();
        let id = id.clone();
        move |_| {
            state.borrow_mut().selected_id = Some(id.clone());
            open_place(
                &window,
                &state,
                &id,
                refresh.clone(),
                widgets.clone(),
            );
        }
    });
    creds_btn.connect_clicked({
        let window = window.clone();
        let place = place.clone();
        let refresh = refresh.clone();
        move |_| {
            wizard::run_edit_credentials(&window, &place, refresh.clone());
        }
    });
    delete_btn.connect_clicked({
        let window = window.clone();
        let place = place.clone();
        let refresh = refresh.clone();
        move |_| {
            wizard::run_disconnect_chooser(&window, &place, refresh.clone());
        }
    });

    // Clicking the card body selects + connects (same as Connect).
    let click = gtk4::GestureClick::new();
    click.connect_released({
        let window = window.clone();
        let state = state.clone();
        let refresh = refresh.clone();
        let widgets = connect_widgets.clone();
        let id = id.clone();
        move |gesture, _n, _x, _y| {
            if let Some(widget) = gesture.widget() {
                if widget
                    .pick(_x, _y, gtk4::PickFlags::DEFAULT)
                    .and_then(|w| w.ancestor(Button::static_type()))
                    .is_some()
                {
                    return;
                }
            }
            state.borrow_mut().selected_id = Some(id.clone());
            open_place(
                &window,
                &state,
                &id,
                refresh.clone(),
                widgets.clone(),
            );
        }
    });
    card.add_controller(click);

    card
}

fn build_empty_state() -> (GtkBox, Button) {
    let empty = GtkBox::new(Orientation::Vertical, 12);
    empty.set_halign(Align::Center);
    empty.set_valign(Align::Center);
    empty.set_hexpand(true);
    empty.set_vexpand(true);
    empty.set_margin_top(80);

    empty.append(&Label::builder()
        .label("Map a network folder")
        .css_classes(["nn-hero-title"])
        .build());
    empty.append(&Label::builder()
        .label("Add SMB, SSH, NFS, WebDAV, Video, or Audio places as real mounts under /mnt/network.")
        .css_classes(["nn-hero-sub"])
        .wrap(true)
        .justify(gtk4::Justification::Center)
        .build());

    let add = Button::with_label("Add network place");
    add.add_css_class("nn-accent");
    add.set_halign(Align::Center);
    empty.append(&add);

    empty.append(&Label::builder()
        .label("smb://nas/media · sftp://user@host:/data · nfs://storage/export · http://host:8096")
        .css_classes(["nn-example"])
        .justify(gtk4::Justification::Center)
        .build());

    (empty, add)
}

fn build_daemon_gate() -> GtkBox {
    let gate = GtkBox::new(Orientation::Vertical, 16);
    gate.set_halign(Align::Center);
    gate.set_valign(Align::Center);
    gate.set_hexpand(true);
    gate.set_vexpand(true);
    gate.set_margin_top(100);
    gate.append(&Label::builder()
        .label("Network Neighbourhood service is not running")
        .css_classes(["nn-hero-title"])
        .build());
    gate.append(&Label::builder()
        .label("Mounts require network-neighbourhoodd on the system bus. Start or install the service to continue.")
        .css_classes(["nn-hero-sub"])
        .wrap(true)
        .justify(gtk4::Justification::Center)
        .build());
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_halign(Align::Center);
    let start = Button::with_label("Start service");
    start.add_css_class("nn-accent");
    start.set_widget_name("nn-start-daemon");
    let install = Button::with_label("Install");
    install.set_widget_name("nn-install-daemon");
    let retry = Button::with_label("Retry");
    retry.set_widget_name("nn-retry-daemon");
    row.append(&start);
    row.append(&install);
    row.append(&retry);
    gate.append(&row);
    gate
}

fn wire_daemon_gate(gate: &GtkBox, main_ui: &GtkBox, refresh: Rc<dyn Fn()>) {
    let gate = gate.clone();
    let main_ui = main_ui.clone();
    let mut child = gate.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        if let Ok(box_) = c.clone().downcast::<GtkBox>() {
            let mut b = box_.first_child();
            while let Some(btn_w) = b {
                b = btn_w.next_sibling();
                if let Ok(btn) = btn_w.downcast::<Button>() {
                    let name = btn.widget_name().to_string();
                    let gate = gate.clone();
                    let main_ui = main_ui.clone();
                    let refresh = refresh.clone();
                    btn.connect_clicked(move |_| {
                        match name.as_str() {
                            "nn-start-daemon" => {
                                let _ = std::process::Command::new("pkexec")
                                    .args(["systemctl", "start", "network-neighbourhoodd.service"])
                                    .status();
                            }
                            "nn-install-daemon" => {
                                let _ = std::process::Command::new("pkexec")
                                    .args([
                                        "systemctl",
                                        "enable",
                                        "--now",
                                        "network-neighbourhoodd.service",
                                    ])
                                    .status();
                            }
                            _ => {}
                        }
                        let alive = dbus::daemon_alive();
                        gate.set_visible(!alive);
                        main_ui.set_visible(alive);
                        if alive {
                            refresh();
                        }
                    });
                }
            }
        }
    }
}

fn install_shortcuts(
    window: &ApplicationWindow,
    settings: &crate::config::Settings,
    state: Rc<RefCell<AppState>>,
    refresh: Rc<dyn Fn()>,
    discovery: Rc<DiscoveryPanel>,
    stack: Stack,
    nav_places: Button,
    nav_settings: Button,
    connect_widgets: Rc<RefCell<HashMap<String, Rc<InlineConnectStatus>>>>,
) {
    let shortcuts = settings.shortcuts.clone();
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(clone!(
        #[strong]
        window,
        #[strong]
        state,
        #[strong]
        refresh,
        #[strong]
        discovery,
        #[strong]
        stack,
        #[strong]
        nav_places,
        #[strong]
        nav_settings,
        #[strong]
        connect_widgets,
        move |_, key, _, mods| {
            let hit = |action: &str| -> bool {
                shortcuts
                    .get(action)
                    .map(|spec| match_shortcut(spec, key, mods))
                    .unwrap_or(false)
            };
            if hit("refresh") || key == Key::F5 {
                refresh();
                return glib::Propagation::Stop;
            }
            if hit("add") || (key == Key::n && mods.contains(ModifierType::CONTROL_MASK)) {
                ensure_admin(&state);
                wizard::run_add_wizard(&window, refresh.clone());
                return glib::Propagation::Stop;
            }
            if hit("discover") || (key == Key::f && mods.contains(ModifierType::CONTROL_MASK)) {
                discovery.toggle();
                return glib::Propagation::Stop;
            }
            if hit("settings") || (key == Key::comma && mods.contains(ModifierType::CONTROL_MASK)) {
                state.borrow_mut().page = Page::Settings;
                stack.set_visible_child_name("settings");
                nav_settings.add_css_class("nn-active");
                nav_places.remove_css_class("nn-active");
                return glib::Propagation::Stop;
            }
            if hit("disconnect") || (key == Key::d && mods.contains(ModifierType::CONTROL_MASK)) {
                let st = state.borrow();
                if let Some(p) = st
                    .selected_id
                    .as_ref()
                    .and_then(|id| st.places.iter().find(|x| x.id == *id))
                    .cloned()
                {
                    drop(st);
                    wizard::run_disconnect_chooser(&window, &p, refresh.clone());
                }
                return glib::Propagation::Stop;
            }
            if hit("open") || (key == Key::o && mods.contains(ModifierType::CONTROL_MASK)) {
                if let Some(id) = state.borrow().selected_id.clone() {
                    open_place(
                        &window,
                        &state,
                        &id,
                        refresh.clone(),
                        connect_widgets.clone(),
                    );
                }
                return glib::Propagation::Stop;
            }
            if hit("properties") {
                let st = state.borrow();
                if let Some(p) = st
                    .selected_id
                    .as_ref()
                    .and_then(|id| st.places.iter().find(|x| x.id == *id))
                    .cloned()
                {
                    drop(st);
                    wizard::run_edit_credentials(&window, &p, refresh.clone());
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    window.add_controller(controller);
}

fn match_shortcut(spec: &str, key: Key, mods: ModifierType) -> bool {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("F5") {
        return key == Key::F5;
    }
    let ctrl = spec.contains("<Control>") || spec.contains("Ctrl");
    let alt = spec.contains("<Alt>") || spec.contains("Alt");
    if ctrl != mods.contains(ModifierType::CONTROL_MASK) {
        return false;
    }
    if alt != mods.contains(ModifierType::ALT_MASK) {
        return false;
    }
    let last = spec
        .rsplit('>')
        .next()
        .unwrap_or(spec)
        .trim_matches(|c| c == '<' || c == '>')
        .to_ascii_lowercase();
    match last.as_str() {
        "n" => key == Key::n,
        "d" => key == Key::d,
        "o" => key == Key::o,
        "f" => key == Key::f,
        "comma" | "," => key == Key::comma,
        "return" => key == Key::Return,
        _ => false,
    }
}
