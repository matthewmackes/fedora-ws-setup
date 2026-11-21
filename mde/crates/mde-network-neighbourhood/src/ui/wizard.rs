use crate::dbus;
use crate::place::{
    CredentialScope, DiscoveredHost, MountPolicy, Place, Protocol,
};
use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Entry, Expander, Grid,
    Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

pub fn run_add_wizard(window: &ApplicationWindow, refresh: Rc<dyn Fn()>) {
    run_add_dialog(window, None, refresh);
}

pub fn run_add_from_discovered(
    window: &ApplicationWindow,
    host: &DiscoveredHost,
    refresh: Rc<dyn Fn()>,
) {
    let mut place = Place::new(host.name.clone(), host.protocol, host.host.clone());
    place.port = host.port;
    place.path = host.path.clone();
    place.url = host.url.clone();
    run_add_dialog(window, Some(place), refresh);
}

pub fn run_edit_credentials(
    window: &ApplicationWindow,
    place: &Place,
    refresh: Rc<dyn Fn()>,
) {
    let dlg = modal_window(window, "Edit credentials", 440, -1);
    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);

    content.append(&Label::builder()
        .label(&format!("{} — {}", place.name, place.host))
        .halign(Align::Start)
        .build());
    if !place.last_error.is_empty() {
        content.append(&Label::builder()
            .label(&place.last_error)
            .halign(Align::Start)
            .wrap(true)
            .css_classes(["nn-conflict"])
            .build());
    }

    let user = Entry::builder().text(&place.username).hexpand(true).build();
    let pass = Entry::builder().visibility(false).hexpand(true).build();
    pass.set_placeholder_text(Some("Enter password / token"));
    content.append(&labeled("User name", &user));
    content.append(&labeled("Password / token", &pass));

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    actions.set_halign(Align::End);
    let cancel = Button::with_label("Cancel");
    let save = Button::with_label("Save");
    let save_retry = Button::with_label("Save & Retry");
    save_retry.add_css_class("nn-accent");
    actions.append(&cancel);
    actions.append(&save);
    actions.append(&save_retry);
    content.append(&actions);
    dlg.set_child(Some(&content));

    let place = place.clone();
    cancel.connect_clicked({
        let dlg = dlg.clone();
        move |_| dlg.close()
    });
    let save_fn = {
        let place = place.clone();
        let user = user.clone();
        let pass = pass.clone();
        let refresh = refresh.clone();
        let dlg = dlg.clone();
        Rc::new(move |retry: bool| {
            let mut p = place.clone();
            p.username = user.text().to_string();
            p.last_error.clear();
            let password = pass.text().to_string();
            match dbus::upsert_place_sync(&p, &password) {
                Ok(id) => {
                    refresh();
                    if retry {
                        std::thread::spawn(move || {
                            let _ = dbus::mount_place_sync(&id);
                        });
                    }
                    dlg.close();
                }
                Err(e) => eprintln!("credential save failed: {e}"),
            }
        })
    };
    save.connect_clicked({
        let save_fn = save_fn.clone();
        move |_| save_fn(false)
    });
    save_retry.connect_clicked({
        let save_fn = save_fn.clone();
        move |_| save_fn(true)
    });
    dlg.present();
}

fn labeled(text: &str, w: &impl IsA<gtk4::Widget>) -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 4);
    let l = Label::new(Some(text));
    l.set_halign(Align::Start);
    b.append(&l);
    b.append(w);
    b
}

fn modal_window(parent: &ApplicationWindow, title: &str, width: i32, height: i32) -> Window {
    let mut b = Window::builder();
    b = b
        .title(title)
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(true);
    if width > 0 {
        b = b.default_width(width);
    }
    if height > 0 {
        b = b.default_height(height);
    }
    b.build()
}

fn alert(parent: &impl IsA<gtk4::Window>, title: &str, detail: &str) {
    let alert = gtk4::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(detail)
        .buttons(["OK"])
        .default_button(0)
        .cancel_button(0)
        .build();
    alert.show(Some(parent));
}

fn dropdown_ids(pairs: &[(&str, &str)], selected_id: &str) -> (DropDown, Vec<String>) {
    let labels: Vec<&str> = pairs.iter().map(|(_, l)| *l).collect();
    let ids: Vec<String> = pairs.iter().map(|(id, _)| (*id).to_string()).collect();
    let dd = DropDown::from_strings(&labels);
    if let Some(idx) = ids.iter().position(|id| id == selected_id) {
        dd.set_selected(idx as u32);
    } else {
        dd.set_selected(0);
    }
    (dd, ids)
}

fn selected_id(dd: &DropDown, ids: &[String]) -> String {
    let idx = dd.selected() as usize;
    ids.get(idx)
        .cloned()
        .unwrap_or_else(|| ids.first().cloned().unwrap_or_default())
}

fn run_add_dialog(window: &ApplicationWindow, initial: Option<Place>, refresh: Rc<dyn Fn()>) {
    let settings = dbus::get_settings_sync().unwrap_or_default();

    let dlg = modal_window(window, "Add network place", 780, 560);
    let content = GtkBox::new(Orientation::Vertical, 10);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);

    content.append(&Label::builder()
        .label("Add a network place")
        .halign(Align::Start)
        .css_classes(["nn-hero-title"])
        .build());
    content.append(&Label::builder()
        .label("Paste a URI on the left, or fill the wizard on the right. Both stay in sync — Finish is blocked if they conflict.")
        .halign(Align::Start)
        .wrap(true)
        .css_classes(["nn-hero-sub"])
        .build());

    let split = GtkBox::new(Orientation::Horizontal, 12);
    split.set_hexpand(true);
    split.set_vexpand(true);

    // —— Left: URI ——
    let left = GtkBox::new(Orientation::Vertical, 8);
    left.add_css_class("nn-split-pane");
    left.set_hexpand(true);
    left.append(&Label::builder().label("Network URI").halign(Align::Start).build());
    let uri_entry = Entry::builder()
        .placeholder_text("smb://server/share · sftp://user@host/path · nfs://host/export · http://host:8096")
        .hexpand(true)
        .build();
    left.append(&uri_entry);
    left.append(&Label::builder()
        .label("Examples:\nsmb://nas/media\n\\\\fileserver\\docs\nsftp://me@gw:/data\nnfs://storage/export")
        .halign(Align::Start)
        .css_classes(["nn-example"])
        .build());

    // —— Right: wizard ——
    let right = GtkBox::new(Orientation::Vertical, 8);
    right.add_css_class("nn-split-pane");
    right.set_hexpand(true);

    let grid = Grid::new();
    grid.set_column_spacing(8);
    grid.set_row_spacing(8);
    grid.set_hexpand(true);

    let name = Entry::new();
    let host = Entry::new();
    let path = Entry::new();
    let user = Entry::new();
    let pass = Entry::builder().visibility(false).build();
    let url = Entry::new();

    let proto_pairs: Vec<(&str, &str)> = Protocol::all()
        .iter()
        .map(|p| (p.as_str(), p.group_label()))
        .collect();
    let (proto, proto_ids) = dropdown_ids(&proto_pairs, Protocol::Smb.as_str());

    fn add_row(grid: &Grid, r: i32, label: &str, w: &impl IsA<gtk4::Widget>) {
        let l = Label::new(Some(label));
        l.set_halign(Align::Start);
        grid.attach(&l, 0, r, 1, 1);
        grid.attach(w, 1, r, 1, 1);
    }
    add_row(&grid, 0, "Protocol", &proto);
    add_row(&grid, 1, "Name", &name);
    add_row(&grid, 2, "Host", &host);
    add_row(&grid, 3, "Share / path", &path);
    add_row(&grid, 4, "URL", &url);
    add_row(&grid, 5, "User name", &user);
    add_row(&grid, 6, "Password / token", &pass);
    right.append(&grid);

    let browse_btn = Button::with_label("Browse shares…");
    browse_btn.set_halign(Align::Start);
    right.append(&browse_btn);

    let shares_scroll = ScrolledWindow::builder()
        .min_content_height(100)
        .hexpand(true)
        .build();
    let shares_list = ListBox::new();
    shares_list.set_selection_mode(gtk4::SelectionMode::Single);
    shares_scroll.set_child(Some(&shares_list));
    shares_scroll.set_visible(false);
    right.append(&shares_scroll);

    // Connect once — do not re-attach on every Browse click.
    shares_list.connect_row_activated({
        let path = path.clone();
        move |_, row| {
            if let Some(lab) = row.child().and_downcast::<Label>() {
                path.set_text(&lab.label());
            }
        }
    });

    // Credential scope
    let scope_machine = CheckButton::with_label(CredentialScope::Machine.label());
    let scope_user = CheckButton::with_label(CredentialScope::User.label());
    scope_user.set_group(Some(&scope_machine));
    match settings.default_credential_scope {
        CredentialScope::User => scope_user.set_active(true),
        CredentialScope::Machine => scope_machine.set_active(true),
    }
    right.append(&Label::builder().label("Credential storage").halign(Align::Start).build());
    right.append(&scope_machine);
    right.append(&scope_user);

    // Advanced
    let advanced = Expander::builder().label("Advanced").build();
    let adv_box = GtkBox::new(Orientation::Vertical, 8);
    let policy_pairs: Vec<(&str, &str)> = MountPolicy::all()
        .iter()
        .map(|p| (p.as_str(), p.label()))
        .collect();
    let (policy, policy_ids) = dropdown_ids(&policy_pairs, MountPolicy::MountNow.as_str());

    let letters: Vec<String> = std::iter::once("(none)".to_string())
        .chain(('A'..='Z').map(|c| format!("{c}:")))
        .collect();
    let letter_refs: Vec<&str> = letters.iter().map(|s| s.as_str()).collect();
    let drive = DropDown::from_strings(&letter_refs);
    drive.set_selected(0);

    adv_box.append(&labeled("Mount policy", &policy));
    adv_box.append(&labeled("Drive letter", &drive));
    advanced.set_child(Some(&adv_box));
    right.append(&advanced);

    split.append(&left);
    split.append(&right);
    content.append(&split);

    let conflict = Label::builder()
        .label("")
        .halign(Align::Start)
        .wrap(true)
        .css_classes(["nn-conflict"])
        .build();
    content.append(&conflict);

    let footer = GtkBox::new(Orientation::Horizontal, 8);
    footer.set_halign(Align::End);
    let cancel = Button::with_label("Cancel");
    let finish = Button::with_label("Finish");
    finish.add_css_class("nn-accent");
    footer.append(&cancel);
    footer.append(&finish);
    content.append(&footer);
    dlg.set_child(Some(&content));

    if let Some(p) = &initial {
        name.set_text(&p.name);
        host.set_text(&p.host);
        path.set_text(&p.path);
        url.set_text(&p.url);
        user.set_text(&p.username);
        if let Some(idx) = Protocol::all().iter().position(|x| *x == p.protocol) {
            proto.set_selected(idx as u32);
        }
        if !p.url.is_empty() {
            uri_entry.set_text(&p.url);
        } else if p.protocol == Protocol::Smb && !p.path.is_empty() {
            uri_entry.set_text(&format!("smb://{}/{}", p.host, p.path));
        }
    }

    // Conflict detection state
    let dirty_uri = Rc::new(RefCell::new(false));
    let dirty_wiz = Rc::new(RefCell::new(false));
    let proto_ids = Rc::new(proto_ids);
    let policy_ids = Rc::new(policy_ids);

    let check_conflict = {
        let uri_entry = uri_entry.clone();
        let proto = proto.clone();
        let proto_ids = proto_ids.clone();
        let host = host.clone();
        let path = path.clone();
        let name = name.clone();
        let url = url.clone();
        let user = user.clone();
        let conflict = conflict.clone();
        let dirty_uri = dirty_uri.clone();
        let dirty_wiz = dirty_wiz.clone();
        Rc::new(move || -> bool {
            if !*dirty_uri.borrow() || !*dirty_wiz.borrow() {
                conflict.set_text("");
                return false;
            }
            let u = uri_entry.text().to_string();
            if u.trim().is_empty() {
                conflict.set_text("");
                return false;
            }
            let Some(parsed) = dbus::parse_network_uri(&u) else {
                conflict.set_text("URI side cannot be parsed — fix the URI or clear it.");
                return true;
            };
            let id = selected_id(&proto, &proto_ids);
            let protocol = Protocol::parse(&id).unwrap_or(Protocol::Smb);
            let mut mismatches = Vec::new();
            if parsed.protocol != protocol {
                mismatches.push(format!(
                    "protocol (URI {} vs wizard {})",
                    parsed.protocol.group_label(),
                    protocol.group_label()
                ));
            }
            if !host.text().is_empty()
                && !parsed.host.is_empty()
                && host.text().as_str() != parsed.host
            {
                mismatches.push("host".into());
            }
            if !path.text().is_empty()
                && !parsed.path.is_empty()
                && path.text().as_str() != parsed.path
            {
                mismatches.push("path/share".into());
            }
            if !url.text().is_empty()
                && !parsed.url.is_empty()
                && url.text().as_str() != parsed.url
            {
                mismatches.push("URL".into());
            }
            if !user.text().is_empty()
                && !parsed.username.is_empty()
                && user.text().as_str() != parsed.username
            {
                mismatches.push("user name".into());
            }
            let _ = &name;
            if mismatches.is_empty() {
                conflict.set_text("");
                false
            } else {
                conflict.set_text(&format!(
                    "Conflict — clear one side or make them match: {}",
                    mismatches.join(", ")
                ));
                true
            }
        })
    };

    uri_entry.connect_changed({
        let dirty_uri = dirty_uri.clone();
        let dirty_wiz = dirty_wiz.clone();
        let check_conflict = check_conflict.clone();
        let proto = proto.clone();
        let name = name.clone();
        let host = host.clone();
        let path = path.clone();
        let url = url.clone();
        let user = user.clone();
        move |e| {
            *dirty_uri.borrow_mut() = !e.text().is_empty();
            if !*dirty_wiz.borrow() {
                if let Some(p) = dbus::parse_network_uri(&e.text()) {
                    name.set_text(&p.name);
                    host.set_text(&p.host);
                    path.set_text(&p.path);
                    url.set_text(&p.url);
                    user.set_text(&p.username);
                    if let Some(idx) = Protocol::all().iter().position(|x| *x == p.protocol) {
                        proto.set_selected(idx as u32);
                    }
                    *dirty_uri.borrow_mut() = true;
                    *dirty_wiz.borrow_mut() = false;
                }
            }
            check_conflict();
        }
    });

    let mark_wiz = {
        let dirty_wiz = dirty_wiz.clone();
        let check_conflict = check_conflict.clone();
        Rc::new(move || {
            *dirty_wiz.borrow_mut() = true;
            check_conflict();
        })
    };
    for e in [&name, &host, &path, &url, &user] {
        let mark_wiz = mark_wiz.clone();
        e.connect_changed(move |_| mark_wiz());
    }
    proto.connect_selected_notify({
        let mark_wiz = mark_wiz.clone();
        move |_| mark_wiz()
    });

    browse_btn.connect_clicked({
        let host = host.clone();
        let user = user.clone();
        let pass = pass.clone();
        let shares_list = shares_list.clone();
        let shares_scroll = shares_scroll.clone();
        let dlg = dlg.clone();
        move |_| {
            let h = host.text().to_string();
            if h.is_empty() {
                return;
            }
            match dbus::list_smb_shares_sync(&h, &user.text(), &pass.text()) {
                Ok(shares) => {
                    while let Some(c) = shares_list.first_child() {
                        shares_list.remove(&c);
                    }
                    for s in shares {
                        let row = ListBoxRow::new();
                        let lab = Label::new(Some(&s));
                        lab.set_halign(Align::Start);
                        lab.set_margin_start(8);
                        lab.set_margin_end(8);
                        lab.set_margin_top(6);
                        lab.set_margin_bottom(6);
                        row.set_child(Some(&lab));
                        shares_list.append(&row);
                    }
                    shares_scroll.set_visible(true);
                }
                Err(e) => alert(&dlg, "Could not list shares", &e.to_string()),
            }
        }
    });

    cancel.connect_clicked({
        let dlg = dlg.clone();
        move |_| dlg.close()
    });

    finish.connect_clicked({
        let dlg = dlg.clone();
        let check_conflict = check_conflict.clone();
        let dirty_uri = dirty_uri.clone();
        let dirty_wiz = dirty_wiz.clone();
        let uri_entry = uri_entry.clone();
        let proto = proto.clone();
        let proto_ids = proto_ids.clone();
        let policy = policy.clone();
        let policy_ids = policy_ids.clone();
        let name = name.clone();
        let host = host.clone();
        let path = path.clone();
        let url = url.clone();
        let user = user.clone();
        let pass = pass.clone();
        let scope_user = scope_user.clone();
        let drive = drive.clone();
        let letters = letters.clone();
        let conflict = conflict.clone();
        let refresh = refresh.clone();
        move |_| {
            if check_conflict() {
                return;
            }

            let uri_text = uri_entry.text().to_string();
            let use_uri = *dirty_uri.borrow() && !*dirty_wiz.borrow();
            let use_wiz = *dirty_wiz.borrow() && (!*dirty_uri.borrow() || uri_text.trim().is_empty());

            let mut place = if use_uri || (!use_wiz && !uri_text.trim().is_empty() && !*dirty_wiz.borrow())
            {
                match dbus::parse_network_uri(&uri_text) {
                    Some(p) => p,
                    None => {
                        conflict.set_text("Cannot parse URI.");
                        return;
                    }
                }
            } else if !uri_text.trim().is_empty() && !*dirty_wiz.borrow() {
                match dbus::parse_network_uri(&uri_text) {
                    Some(p) => p,
                    None => {
                        conflict.set_text("Cannot parse URI.");
                        return;
                    }
                }
            } else {
                let id = selected_id(&proto, &proto_ids);
                let protocol = Protocol::parse(&id).unwrap_or(Protocol::Smb);
                let mut p = Place::new(name.text().as_str(), protocol, host.text().as_str());
                p.path = path.text().to_string();
                p.url = url.text().to_string();
                p
            };

            if *dirty_wiz.borrow() && !name.text().is_empty() {
                place.name = name.text().to_string();
            }
            if *dirty_wiz.borrow() {
                if !host.text().is_empty() {
                    place.host = host.text().to_string();
                }
                if !path.text().is_empty() {
                    place.path = path.text().to_string();
                }
                if !url.text().is_empty() {
                    place.url = url.text().to_string();
                }
            }
            if !user.text().is_empty() {
                place.username = user.text().to_string();
            }

            place.credential_scope = if scope_user.is_active() {
                CredentialScope::User
            } else {
                CredentialScope::Machine
            };
            let mp_id = selected_id(&policy, &policy_ids);
            if let Some(mp) = MountPolicy::parse(&mp_id) {
                place.mount_policy = mp;
                place.sync_automount_flag();
            }
            let idx = drive.selected() as usize;
            if idx > 0 && idx < letters.len() {
                place.drive_letter = Some(letters[idx].trim_end_matches(':').to_string());
            }

            if place.host.is_empty() && place.url.is_empty() {
                conflict.set_text("Host or URL is required.");
                return;
            }

            let password = pass.text().to_string();
            match dbus::upsert_place_sync(&place, &password) {
                Ok(id) => {
                    let mp = place.mount_point();
                    let policy = place.mount_policy;
                    dlg.close();
                    refresh();
                    if matches!(
                        policy,
                        MountPolicy::MountNow | MountPolicy::Automount
                    ) {
                        std::thread::spawn(move || {
                            let _ = dbus::call_sync("BeginAdminSession", "");
                            match dbus::mount_place_sync(&id) {
                                Ok(_) => {
                                    if policy == MountPolicy::MountNow {
                                        let _ = std::process::Command::new("xdg-open")
                                            .arg(&mp)
                                            .stdin(std::process::Stdio::null())
                                            .stdout(std::process::Stdio::null())
                                            .stderr(std::process::Stdio::null())
                                            .spawn();
                                    }
                                }
                                Err(e) => eprintln!("mount: {e}"),
                            }
                        });
                    }
                }
                Err(e) => {
                    conflict.set_text(&format!("Save failed: {e}"));
                }
            }
        }
    });

    dlg.present();
}

pub fn run_disconnect_chooser(
    window: &ApplicationWindow,
    place: &Place,
    refresh: Rc<dyn Fn()>,
) {
    let dlg = modal_window(window, "Disconnect", 400, -1);
    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.append(&Label::builder()
        .label(&format!("Disconnect “{}”?", place.name))
        .halign(Align::Start)
        .build());

    let eject = Button::with_label("Eject for now");
    let disable = Button::with_label("Remove automount");
    let delete = Button::with_label("Delete place");
    delete.add_css_class("destructive-action");
    let cancel = Button::with_label("Cancel");

    let id = place.id.clone();
    let refresh2 = refresh.clone();
    eject.connect_clicked({
        let dlg = dlg.clone();
        let id = id.clone();
        move |_| {
            let _ = dbus::call_sync("UnmountPlace", &id);
            refresh2();
            dlg.close();
        }
    });
    let refresh3 = refresh.clone();
    disable.connect_clicked({
        let dlg = dlg.clone();
        let id = id.clone();
        move |_| {
            let _ = dbus::call_sync("DisableAutomount", &id);
            refresh3();
            dlg.close();
        }
    });
    let refresh4 = refresh;
    delete.connect_clicked({
        let dlg = dlg.clone();
        let id = id.clone();
        move |_| {
            let _ = dbus::call_sync("RemovePlace", &id);
            refresh4();
            dlg.close();
        }
    });
    cancel.connect_clicked({
        let dlg = dlg.clone();
        move |_| dlg.close()
    });

    content.append(&eject);
    content.append(&disable);
    content.append(&delete);
    content.append(&cancel);
    dlg.set_child(Some(&content));
    dlg.present();
}
