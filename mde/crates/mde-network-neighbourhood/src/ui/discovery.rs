use crate::dbus;
use crate::place::DiscoveredHost;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation,
    Revealer, ScrolledWindow, Spinner, TextBuffer, TextView,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub struct DiscoveryPanel {
    pub revealer: Revealer,
    list: ListBox,
    status: Label,
    spinner: Spinner,
    log_buffer: TextBuffer,
    hosts: Rc<RefCell<Vec<DiscoveredHost>>>,
    scanning: Rc<Cell<bool>>,
}

impl DiscoveryPanel {
    pub fn new() -> Self {
        let revealer = Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideLeft)
            .reveal_child(false)
            .build();

        let panel = GtkBox::new(Orientation::Vertical, 8);
        panel.set_width_request(320);
        panel.add_css_class("nn-sidebar");
        panel.set_margin_start(4);

        let header = GtkBox::new(Orientation::Horizontal, 8);
        header.set_margin_start(12);
        header.set_margin_end(8);
        header.set_margin_top(12);
        let title = Label::builder()
            .label("Browse network")
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let spinner = Spinner::new();
        let close = Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        header.append(&title);
        header.append(&spinner);
        header.append(&close);
        panel.append(&header);

        let status = Label::builder()
            .label("Loading neighbourhood database…")
            .halign(Align::Start)
            .margin_start(12)
            .wrap(true)
            .css_classes(["nn-hero-sub"])
            .build();
        panel.append(&status);

        let refresh_btn = Button::with_label("Scan now");
        refresh_btn.set_margin_start(12);
        refresh_btn.set_margin_end(12);
        refresh_btn.add_css_class("nn-accent");
        panel.append(&refresh_btn);

        let log_buffer = TextBuffer::new(None::<&gtk4::TextTagTable>);
        let log_view = TextView::with_buffer(&log_buffer);
        log_view.set_editable(false);
        log_view.set_monospace(true);
        log_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        log_view.add_css_class("nn-log");
        let log_scroll = ScrolledWindow::builder()
            .child(&log_view)
            .min_content_height(100)
            .max_content_height(160)
            .hexpand(true)
            .margin_start(8)
            .margin_end(8)
            .build();
        panel.append(&log_scroll);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::Single);
        let scroll = ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hexpand(true)
            .build();
        panel.append(&scroll);

        revealer.set_child(Some(&panel));

        let hosts = Rc::new(RefCell::new(Vec::new()));
        let scanning = Rc::new(Cell::new(false));

        close.connect_clicked({
            let revealer = revealer.clone();
            move |_| revealer.set_reveal_child(false)
        });

        let panel = Self {
            revealer,
            list,
            status,
            spinner,
            log_buffer,
            hosts,
            scanning,
        };

        // Seed from background database immediately (async).
        {
            let list = panel.list.clone();
            let status = panel.status.clone();
            let hosts = panel.hosts.clone();
            let log_buffer = panel.log_buffer.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let cache = dbus::list_discovered_sync().unwrap_or_default();
                let _ = tx.send(cache);
            });
            glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
                Ok(cache) => {
                    fill_host_list(&list, &cache.hosts);
                    *hosts.borrow_mut() = cache.hosts.clone();
                    let age = cache
                        .age_secs()
                        .map(|s| format!("{s}s ago"))
                        .unwrap_or_else(|| "never".into());
                    status.set_text(&format!(
                        "{} device(s) in database (scanned {age})",
                        cache.hosts.len()
                    ));
                    let mut end = log_buffer.end_iter();
                    log_buffer.insert(
                        &mut end,
                        &format!(
                            "Loaded cache: {} host(s). {}\n",
                            cache.hosts.len(),
                            cache.last_status
                        ),
                    );
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    status.set_text(
                        "No discovery database yet — Scan now or wait for background scanner.",
                    );
                    glib::ControlFlow::Break
                }
            });
        }

        refresh_btn.connect_clicked({
            let list = panel.list.clone();
            let status = panel.status.clone();
            let spinner = panel.spinner.clone();
            let hosts = panel.hosts.clone();
            let scanning = panel.scanning.clone();
            let refresh_btn = refresh_btn.clone();
            let log_buffer = panel.log_buffer.clone();
            move |_| {
                if scanning.get() {
                    return;
                }
                scanning.set(true);
                spinner.start();
                refresh_btn.set_sensitive(false);
                status.set_text("Scanning via neighbourhoodd…");
                log_buffer.set_text("Requesting discovery pass…\n");

                enum Ev {
                    Log(String),
                    Done(Vec<DiscoveredHost>),
                    Err(String),
                }
                let (tx, rx) = std::sync::mpsc::channel::<Ev>();
                std::thread::spawn(move || {
                    let (done_tx, done_rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = done_tx.send(dbus::discover_sync());
                    });
                    loop {
                        if let Ok(st) = dbus::get_discovery_status_sync() {
                            if let Some(s) = st.get("last_status").and_then(|v| v.as_str()) {
                                if !s.is_empty() {
                                    let _ = tx.send(Ev::Log(s.to_string()));
                                }
                            }
                        }
                        match done_rx.try_recv() {
                            Ok(Ok(h)) => {
                                let _ = tx.send(Ev::Done(h));
                                break;
                            }
                            Ok(Err(e)) => {
                                let _ = tx.send(Ev::Err(e.to_string()));
                                break;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                std::thread::sleep(Duration::from_millis(400));
                            }
                            Err(_) => {
                                let _ = tx.send(Ev::Err("worker exited".into()));
                                break;
                            }
                        }
                    }
                });

                let list = list.clone();
                let status = status.clone();
                let spinner = spinner.clone();
                let hosts = hosts.clone();
                let scanning = scanning.clone();
                let refresh_btn = refresh_btn.clone();
                let log_buffer = log_buffer.clone();
                glib::timeout_add_local(Duration::from_millis(80), move || {
                    loop {
                        match rx.try_recv() {
                            Ok(Ev::Log(line)) => {
                                let mut end = log_buffer.end_iter();
                                log_buffer.insert(&mut end, &line);
                                if !line.ends_with('\n') {
                                    let mut end = log_buffer.end_iter();
                                    log_buffer.insert(&mut end, "\n");
                                }
                                status.set_text(&line);
                            }
                            Ok(Ev::Done(discovered)) => {
                                fill_host_list(&list, &discovered);
                                let n = discovered.len();
                                *hosts.borrow_mut() = discovered;
                                status.set_text(&format!(
                                    "{n} device(s) found — double-click to add"
                                ));
                                spinner.stop();
                                refresh_btn.set_sensitive(true);
                                scanning.set(false);
                                return glib::ControlFlow::Break;
                            }
                            Ok(Ev::Err(e)) => {
                                status.set_text(&format!("Scan failed: {e}"));
                                let mut end = log_buffer.end_iter();
                                log_buffer.insert(&mut end, &format!("ERROR: {e}\n"));
                                spinner.stop();
                                refresh_btn.set_sensitive(true);
                                scanning.set(false);
                                return glib::ControlFlow::Break;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                return glib::ControlFlow::Continue;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                status.set_text("Scan failed.");
                                spinner.stop();
                                refresh_btn.set_sensitive(true);
                                scanning.set(false);
                                return glib::ControlFlow::Break;
                            }
                        }
                    }
                });
            }
        });

        panel
    }

    pub fn toggle(&self) {
        self.revealer
            .set_reveal_child(!self.revealer.reveals_child());
    }

    pub fn apply_hosts(&self, hosts: Vec<DiscoveredHost>) {
        fill_host_list(&self.list, &hosts);
        let n = hosts.len();
        self.status
            .set_text(&format!("{n} device(s) — double-click to add"));
        *self.hosts.borrow_mut() = hosts;
    }

    pub fn connect_activate<F: Fn(DiscoveredHost) + 'static>(
        &self,
        window: &ApplicationWindow,
        f: F,
    ) {
        let hosts = self.hosts.clone();
        let _window = window.clone();
        self.list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            if let Some(h) = hosts.borrow().get(idx).cloned() {
                f(h);
            }
        });
    }
}

fn fill_host_list(list: &ListBox, discovered: &[DiscoveredHost]) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    for h in discovered {
        let row = ListBoxRow::new();
        let box_ = GtkBox::new(Orientation::Vertical, 2);
        box_.set_margin_start(10);
        box_.set_margin_end(10);
        box_.set_margin_top(8);
        box_.set_margin_bottom(8);
        box_.append(
            &Label::builder()
                .label(&h.name)
                .halign(Align::Start)
                .build(),
        );
        box_.append(
            &Label::builder()
                .label(&format!(
                    "{} · {} · {}",
                    h.protocol.group_label(),
                    h.host,
                    h.source
                ))
                .halign(Align::Start)
                .css_classes(["nn-hero-sub"])
                .build(),
        );
        row.set_child(Some(&box_));
        list.append(&row);
    }
}
