//! Guided "Add place": Type -> Where -> Sign in -> What to open -> Check & save.
//! Standard ports are assumed; the logic lives in `crate::guide`.

use crate::dbus;
use crate::guide::{self, Draft, Step};
use crate::place::{DiscoveredHost, Protocol};
use crate::probe::{self, ProbeStep};
use gtk4::prelude::*;
use gtk4::{
    glib, Align, ApplicationWindow, Box as GtkBox, Button, CheckButton, Entry, Expander, Grid,
    Image, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, Spinner, Window,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Starting point when the user picked a discovered host.
pub struct Preset {
    pub protocol: Protocol,
    pub host: String,
    pub port: Option<u16>,
}

struct Ctx {
    win: Window,
    root: GtkBox,
    draft: RefCell<Draft>,
    window: ApplicationWindow,
    refresh: Rc<dyn Fn()>,
    found: RefCell<Vec<DiscoveredHost>>,
}

pub fn run(window: &ApplicationWindow, refresh: Rc<dyn Fn()>, preset: Option<Preset>) {
    let win = Window::builder()
        .transient_for(window)
        .modal(true)
        .title("Add a place")
        .default_width(560)
        .default_height(560)
        .build();
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(16);
    root.set_margin_start(24);
    root.set_margin_end(24);
    win.set_child(Some(&root));

    let mut draft = Draft::default();
    let mut first = Step::Kind;
    if let Some(p) = preset {
        draft.protocol = Some(p.protocol);
        draft.host = p.host;
        draft.port = p.port;
        first = if draft.host.is_empty() { Step::Host } else { next_after(&draft, Step::Host) };
    }
    let ctx = Rc::new(Ctx {
        win: win.clone(),
        root,
        draft: RefCell::new(draft),
        window: window.clone(),
        refresh,
        found: RefCell::new(Vec::new()),
    });

    // Load discovered hosts once, in the background.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(dbus::list_discovered_sync().map(|c| c.hosts).unwrap_or_default());
    });
    {
        let ctx = ctx.clone();
        glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
            Ok(hosts) => {
                *ctx.found.borrow_mut() = hosts;
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    show(&ctx, first);
    win.present();
}

fn next_after(d: &Draft, step: Step) -> Step {
    let steps = guide::steps_for(d.protocol);
    let i = steps.iter().position(|s| *s == step).unwrap_or(0);
    steps[(i + 1).min(steps.len() - 1)]
}

fn prev_before(d: &Draft, step: Step) -> Step {
    let steps = guide::steps_for(d.protocol);
    let i = steps.iter().position(|s| *s == step).unwrap_or(0);
    steps[i.saturating_sub(1)]
}

fn clear(b: &GtkBox) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

fn label(text: &str, css: &[&str]) -> Label {
    Label::builder()
        .label(text)
        .halign(Align::Start)
        .xalign(0.0)
        .wrap(true)
        .css_classes(css.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .build()
}

/// Header + body + footer scaffold. Returns (body, footer, next button).
fn frame(ctx: &Rc<Ctx>, step: Step, heading: &str, sub: &str) -> (GtkBox, Button) {
    clear(&ctx.root);
    let draft = ctx.draft.borrow().clone();
    let steps = guide::steps_for(draft.protocol);
    let idx = steps.iter().position(|s| *s == step).unwrap_or(0);
    // The total is only known once the type is chosen.
    let counter = if draft.protocol.is_some() {
        format!("Step {} of {}", idx + 1, steps.len())
    } else {
        "Step 1".to_string()
    };
    ctx.root.append(&label(&counter, &["nn-hero-sub"]));
    ctx.root.append(&label(heading, &["title-2"]));
    if !sub.is_empty() {
        ctx.root.append(&label(sub, &["nn-hero-sub"]));
    }
    let body = GtkBox::new(Orientation::Vertical, 10);
    body.set_vexpand(true);
    ctx.root.append(&body);

    let footer = GtkBox::new(Orientation::Horizontal, 8);
    let cancel = Button::with_label("Cancel");
    cancel.connect_clicked({
        let win = ctx.win.clone();
        move |_| win.close()
    });
    footer.append(&cancel);
    if step == Step::Kind {
        let adv = Button::with_label("Paste a URI instead…");
        adv.add_css_class("flat");
        adv.connect_clicked({
            let ctx = ctx.clone();
            move |_| {
                ctx.win.close();
                super::wizard::run_add_advanced(&ctx.window, ctx.refresh.clone());
            }
        });
        footer.append(&adv);
    }
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    footer.append(&spacer);
    if idx > 0 {
        let back = Button::with_label("Back");
        back.connect_clicked({
            let ctx = ctx.clone();
            move |_| {
                let to = prev_before(&ctx.draft.borrow(), step);
                show(&ctx, to);
            }
        });
        footer.append(&back);
    }
    let next = Button::with_label("Next");
    next.add_css_class("nn-accent");
    if step != Step::Kind {
        footer.append(&next);
    }
    ctx.root.append(&footer);
    (body, next)
}

fn show(ctx: &Rc<Ctx>, step: Step) {
    match step {
        Step::Kind => step_kind(ctx),
        Step::Host => step_host(ctx),
        Step::Login => step_login(ctx),
        Step::Target => step_target(ctx),
        Step::Check => step_check(ctx),
    }
}

fn step_kind(ctx: &Rc<Ctx>) {
    let (body, _) = frame(
        ctx,
        Step::Kind,
        "What do you want to connect to?",
        "Pick one. Standard ports are used automatically.",
    );
    let grid = Grid::builder()
        .row_spacing(10)
        .column_spacing(10)
        .column_homogeneous(true)
        .build();
    for (i, p) in guide::KINDS.iter().copied().enumerate() {
        let inner = GtkBox::new(Orientation::Vertical, 4);
        inner.set_halign(Align::Start);
        let icon = Image::from_icon_name(p.icon_name());
        icon.set_pixel_size(32);
        icon.set_halign(Align::Start);
        inner.append(&icon);
        inner.append(&label(guide::title(p), &["heading"]));
        inner.append(&label(guide::blurb(p), &["nn-hero-sub"]));
        let btn = Button::builder().child(&inner).css_classes(["nn-kind"]).build();
        btn.connect_clicked({
            let ctx = ctx.clone();
            move |_| {
                ctx.draft.borrow_mut().protocol = Some(p);
                show(&ctx, Step::Host);
            }
        });
        grid.attach(&btn, (i % 2) as i32, (i / 2) as i32, 1, 1);
    }
    body.append(&grid);
}

fn step_host(ctx: &Rc<Ctx>) {
    let d = ctx.draft.borrow().clone();
    let p = d.protocol.unwrap_or(Protocol::Smb);
    let (body, next) = frame(
        ctx,
        Step::Host,
        guide::where_heading(p),
        "Type a name or IP address. Pick one below if it was found on your network.",
    );
    let host = Entry::builder()
        .text(&d.host)
        .placeholder_text("nas.local or 192.168.1.20")
        .hexpand(true)
        .activates_default(true)
        .build();
    body.append(&host);
    body.append(&label(
        &format!("Standard port {} will be used.", p.default_port()),
        &["nn-hero-sub"],
    ));
    let port = Entry::builder()
        .text(d.port.map(|n| n.to_string()).unwrap_or_default())
        .placeholder_text(&p.default_port().to_string())
        .build();
    let ex = Expander::builder().label("Use a different port").child(&port).build();
    ex.set_expanded(d.port.is_some());
    body.append(&ex);

    body.append(&label("Found on your network", &["heading"]));
    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_activate_on_single_click(true);
    let found = guide::host_candidates(&ctx.found.borrow(), p);
    if found.is_empty() {
        body.append(&label(
            "Nothing found yet. Type the address above, or use Browse network to scan.",
            &["nn-hero-sub"],
        ));
    } else {
        let hosts: Rc<Vec<DiscoveredHost>> = Rc::new(found);
        for h in hosts.iter() {
            let row = ListBoxRow::new();
            let line = GtkBox::new(Orientation::Horizontal, 8);
            line.set_margin_top(6);
            line.set_margin_bottom(6);
            line.set_margin_start(10);
            line.set_margin_end(10);
            line.append(&label(&guide::host_label(h), &[]));
            if h.reachable {
                let dot = Label::new(Some("online"));
                dot.add_css_class("nn-badge-ok");
                dot.set_hexpand(true);
                dot.set_halign(Align::End);
                line.append(&dot);
            }
            row.set_child(Some(&line));
            list.append(&row);
        }
        list.connect_row_activated({
            let host = host.clone();
            let hosts = hosts.clone();
            move |_, row| {
                if let Some(h) = hosts.get(row.index() as usize) {
                    host.set_text(&h.host);
                }
            }
        });
        body.append(&ScrolledWindow::builder().child(&list).min_content_height(120).vexpand(true).build());
    }

    let err = label("", &["nn-conflict"]);
    body.append(&err);
    next.connect_clicked({
        let ctx = ctx.clone();
        let host = host.clone();
        let port = port.clone();
        let err = err.clone();
        move |_| {
            let parsed = guide::parse_host_input(&host.text());
            if parsed.host.is_empty() {
                err.set_text("Enter a host name or IP address.");
                return;
            }
            let typed_port = port.text().trim().to_string();
            let chosen = if typed_port.is_empty() {
                parsed.port
            } else {
                match typed_port.parse::<u16>() {
                    Ok(n) if n > 0 => Some(n),
                    _ => {
                        err.set_text("That port isn't a number between 1 and 65535.");
                        return;
                    }
                }
            };
            let to = {
                let mut d = ctx.draft.borrow_mut();
                d.host = parsed.host;
                d.port = chosen.filter(|n| Some(*n) != d.protocol.map(|p| p.default_port()));
                if !parsed.user.is_empty() {
                    d.user = parsed.user;
                }
                if !parsed.path.is_empty() && d.target.is_empty() {
                    d.target = parsed.path;
                }
                next_after(&d, Step::Host)
            };
            show(&ctx, to);
        }
    });
    host.grab_focus();
}

fn step_login(ctx: &Rc<Ctx>) {
    let d = ctx.draft.borrow().clone();
    let p = d.protocol.unwrap_or(Protocol::Smb);
    let sub = match p {
        Protocol::Ssh => "Your account on that computer. I'll offer to set up passwordless keys after it connects.",
        Protocol::Video => "Your Jellyfin username and password. The first sign-in saves an access token for next time.",
        Protocol::Audio => "Your music server username and password.",
        Protocol::Smb => "The account that can open the shared folder, or connect as a guest.",
        _ => "Leave both empty if no sign-in is needed.",
    };
    let (body, next) = frame(ctx, Step::Login, &format!("Sign in to {}", d.host), sub);

    let guest = CheckButton::with_label("Connect as guest (no password)");
    guest.set_active(d.guest);
    if p == Protocol::Smb {
        body.append(&guest);
    }
    let default_user = if d.user.is_empty() && p == Protocol::Ssh {
        std::env::var("USER").unwrap_or_default()
    } else {
        d.user.clone()
    };
    let user = Entry::builder().text(&default_user).placeholder_text("Username").hexpand(true).build();
    let pass = Entry::builder()
        .text(&d.password)
        .placeholder_text("Password")
        .visibility(false)
        .hexpand(true)
        .activates_default(true)
        .build();
    body.append(&label("Username", &["heading"]));
    body.append(&user);
    body.append(&label("Password", &["heading"]));
    body.append(&pass);
    guest.connect_toggled({
        let user = user.clone();
        let pass = pass.clone();
        move |g| {
            user.set_sensitive(!g.is_active());
            pass.set_sensitive(!g.is_active());
        }
    });
    user.set_sensitive(!d.guest);
    pass.set_sensitive(!d.guest);

    let err = label("", &["nn-conflict"]);
    body.append(&err);
    next.connect_clicked({
        let ctx = ctx.clone();
        let (guest, user, pass, err) = (guest.clone(), user.clone(), pass.clone(), err.clone());
        move |_| {
            let is_guest = p == Protocol::Smb && guest.is_active();
            if !is_guest && user.text().trim().is_empty() && p != Protocol::WebDav {
                err.set_text("Enter a username (or choose guest).");
                return;
            }
            let to = {
                let mut d = ctx.draft.borrow_mut();
                d.guest = is_guest;
                d.user = user.text().trim().to_string();
                d.password = if is_guest { String::new() } else { pass.text().to_string() };
                next_after(&d, Step::Login)
            };
            show(&ctx, to);
        }
    });
    if d.guest || default_user.is_empty() {
        user.grab_focus();
    } else {
        pass.grab_focus();
    }
}

fn step_target(ctx: &Rc<Ctx>) {
    let d = ctx.draft.borrow().clone();
    let p = d.protocol.unwrap_or(Protocol::Smb);
    let (heading, hint, example) = guide::target_prompt(p).unwrap_or(("Which folder?", "", ""));
    let (body, next) = frame(ctx, Step::Target, heading, hint);

    let target = Entry::builder()
        .text(&d.target)
        .placeholder_text(example)
        .hexpand(true)
        .activates_default(true)
        .build();
    body.append(&target);

    if p == Protocol::Smb {
        body.append(&label("Shared folders on this host", &["heading"]));
        let status = label("Looking…", &["nn-hero-sub"]);
        let list = ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.set_activate_on_single_click(true);
        body.append(&status);
        body.append(&ScrolledWindow::builder().child(&list).min_content_height(120).vexpand(true).build());

        let (tx, rx) = std::sync::mpsc::channel();
        let (h, u, pw) = (d.host.clone(), d.user.clone(), d.password.clone());
        std::thread::spawn(move || {
            let _ = tx.send(dbus::list_smb_shares_sync(&h, &u, &pw).map_err(|e| e.to_string()));
        });
        let shares: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        list.connect_row_activated({
            let target = target.clone();
            let shares = shares.clone();
            move |_, row| {
                if let Some(s) = shares.borrow().get(row.index() as usize) {
                    target.set_text(s);
                }
            }
        });
        glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
            Ok(Ok(found)) if !found.is_empty() => {
                status.set_text("Click one to select it.");
                for s in &found {
                    let row = ListBoxRow::new();
                    let l = label(s, &[]);
                    l.set_margin_top(6);
                    l.set_margin_bottom(6);
                    l.set_margin_start(10);
                    row.set_child(Some(&l));
                    list.append(&row);
                }
                *shares.borrow_mut() = found;
                glib::ControlFlow::Break
            }
            Ok(Ok(_)) => {
                status.set_text("No shared folders listed. Type the share name above.");
                glib::ControlFlow::Break
            }
            Ok(Err(_)) => {
                status.set_text("Couldn't list the shares. Type the share name above.");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    let err = label("", &["nn-conflict"]);
    body.append(&err);
    next.connect_clicked({
        let ctx = ctx.clone();
        let (target, err) = (target.clone(), err.clone());
        move |_| {
            let t = target.text().trim().to_string();
            if t.is_empty() && guide::target_required(p) {
                err.set_text("Enter or pick a folder.");
                return;
            }
            let to = {
                let mut d = ctx.draft.borrow_mut();
                d.target = t;
                next_after(&d, Step::Target)
            };
            show(&ctx, to);
        }
    });
    target.grab_focus();
}

fn step_check(ctx: &Rc<Ctx>) {
    let d = ctx.draft.borrow().clone();
    let p = d.protocol.unwrap_or(Protocol::Smb);
    let (body, next) = frame(
        ctx,
        Step::Check,
        &format!("Checking {}", d.host),
        "I'm testing the connection before saving. Nothing is mounted yet.",
    );
    next.set_label("Save & connect");
    next.set_sensitive(false);

    let name = Entry::builder()
        .text(guide::suggest_name(p, &d.host))
        .hexpand(true)
        .build();
    let spin_row = GtkBox::new(Orientation::Horizontal, 8);
    let spin = Spinner::new();
    spin.start();
    spin_row.append(&spin);
    spin_row.append(&label("Testing…", &[]));
    let results = GtkBox::new(Orientation::Vertical, 4);
    body.append(&spin_row);
    body.append(&results);
    let advice = label("", &[]);
    body.append(&advice);
    body.append(&label("Name this place", &["heading"]));
    body.append(&name);
    let err = label("", &["nn-conflict"]);
    body.append(&err);

    let draft_for_probe = {
        let mut x = d.clone();
        x.name = name.text().to_string();
        x
    };
    let Some(place) = guide::build_place(&draft_for_probe) else { return };
    let password = d.password.clone();
    let (tx, rx) = std::sync::mpsc::channel::<Vec<ProbeStep>>();
    std::thread::spawn(move || {
        let _ = tx.send(probe::run(&place, &password));
    });
    glib::timeout_add_local(Duration::from_millis(150), {
        let (results, advice, next, spin_row) = (results.clone(), advice.clone(), next.clone(), spin_row.clone());
        move || match rx.try_recv() {
            Ok(steps) => {
                spin_row.set_visible(false);
                for s in &steps {
                    results.append(&label(
                        &format!("{} {} — {}", if s.ok { "✓" } else { "✗" }, s.label, s.detail),
                        &[if s.ok { "nn-badge-ok" } else { "nn-conflict" }],
                    ));
                }
                if let Some(bad) = steps.iter().find(|s| !s.ok) {
                    let diag = crate::diagnose::classify(&bad.detail);
                    advice.set_text(&format!("{}\n{}", diag.cause, diag.fix));
                    next.set_label("Save anyway");
                } else {
                    advice.set_text("All good.");
                }
                next.set_sensitive(true);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    next.connect_clicked({
        let ctx = ctx.clone();
        let (name, err) = (name.clone(), err.clone());
        move |btn| {
            let mut d = ctx.draft.borrow().clone();
            d.name = name.text().trim().to_string();
            let Some(place) = guide::build_place(&d) else { return };
            btn.set_sensitive(false);
            match dbus::upsert_place_sync(&place, &d.password) {
                Ok(id) => {
                    ctx.win.close();
                    (ctx.refresh)();
                    super::window::connect_after_save(&id);
                }
                Err(e) => {
                    btn.set_sensitive(true);
                    err.set_text(&format!("Couldn't save: {e}"));
                }
            }
        }
    });
}
