use crate::config::Settings;
use crate::dbus;
use crate::place::{CredentialScope, MountPolicy};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DropDown, Entry, Grid, Label, Orientation,
    ScrolledWindow, SpinButton,
};
use std::collections::HashMap;
use std::rc::Rc;

pub fn build_settings_page(on_saved: Rc<dyn Fn()>) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 0);
    let scroll = ScrolledWindow::builder().vexpand(true).hexpand(true).build();
    let page = GtkBox::new(Orientation::Vertical, 12);
    page.set_margin_start(24);
    page.set_margin_end(24);
    page.set_margin_top(20);
    page.set_margin_bottom(24);

    page.append(&Label::builder()
        .label("Settings")
        .halign(Align::Start)
        .css_classes(["nn-hero-title"])
        .build());

    let settings = dbus::get_settings_sync().unwrap_or_default();

    // Discovery
    page.append(&section("Discovery"));
    let disc_classic = CheckButton::with_label("Classic discovery (Avahi, WS-Discovery, SMB browse)");
    disc_classic.set_active(settings.discovery_classic);
    let disc_scan = CheckButton::with_label("Subnet port scan");
    disc_scan.set_active(settings.discovery_scan);
    let disc_bg = CheckButton::with_label("Background scanner (daemon keeps the neighbourhood database fresh)");
    disc_bg.set_active(settings.discovery_background);
    page.append(&disc_classic);
    page.append(&disc_scan);
    page.append(&disc_bg);

    let interval = SpinButton::with_range(60.0, 3600.0, 30.0);
    interval.set_value(settings.discovery_interval_sec.max(60) as f64);
    page.append(&labeled("Background scan interval (seconds)", &interval));

    let subnets = Entry::builder()
        .text(&settings.scan_subnets.join(", "))
        .placeholder_text("CIDRs, comma-separated (empty = auto)")
        .hexpand(true)
        .build();
    page.append(&labeled("Scan subnets", &subnets));

    let ports = Entry::builder()
        .text(
            &settings
                .scan_ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )
        .placeholder_text("22, 445, 2049, …")
        .hexpand(true)
        .build();
    page.append(&labeled("Scan ports", &ports));

    // Defaults
    page.append(&section("Defaults"));
    let policy_pairs: Vec<(&str, &str)> = MountPolicy::all()
        .iter()
        .map(|p| (p.as_str(), p.label()))
        .collect();
    let (policy, policy_ids) = dropdown_ids(&policy_pairs, settings.default_mount_policy.as_str());
    page.append(&labeled("Default mount policy", &policy));

    let scope_pairs = [
        ("machine", CredentialScope::Machine.label()),
        ("user", CredentialScope::User.label()),
    ];
    let (scope, scope_ids) =
        dropdown_ids(&scope_pairs, settings.default_credential_scope.as_str());
    page.append(&labeled("Default credential scope", &scope));

    // Mount tuning
    page.append(&section("Mount tuning"));
    let timeout = SpinButton::with_range(5.0, 300.0, 1.0);
    timeout.set_value(settings.mount_timeout_sec as f64);
    page.append(&labeled("Mount timeout (seconds)", &timeout));

    let idle = SpinButton::with_range(30.0, 86400.0, 30.0);
    idle.set_value(settings.automount_idle_sec as f64);
    page.append(&labeled("Automount idle (seconds)", &idle));

    let admin_sess = SpinButton::with_range(60.0, 3600.0, 30.0);
    admin_sess.set_value(settings.admin_session_sec as f64);
    page.append(&labeled("Admin session length (seconds)", &admin_sess));

    let presets = Entry::builder()
        .text(&presets_to_text(&settings.mount_option_presets))
        .placeholder_text("smb=nounix,noserverino; nfs=vers=4")
        .hexpand(true)
        .build();
    page.append(&labeled("Mount option presets", &presets));

    // UI
    page.append(&section("Appearance"));
    let density_pairs = [
        ("comfortable", "Comfortable"),
        ("compact", "Compact"),
        ("dense", "Dense"),
    ];
    let (density, density_ids) = dropdown_ids(&density_pairs, &settings.ui_density);
    page.append(&labeled("Density", &density));

    // Shortcuts
    page.append(&section("Keyboard shortcuts"));
    let shortcut_keys = [
        ("refresh", "Refresh"),
        ("add", "Add place"),
        ("disconnect", "Disconnect"),
        ("open", "Open in Files"),
        ("properties", "Properties"),
        ("discover", "Browse network"),
        ("settings", "Settings"),
    ];
    let shortcut_entries: Vec<(String, Entry)> = shortcut_keys
        .iter()
        .map(|(id, label)| {
            let e = Entry::builder()
                .text(settings.shortcuts.get(*id).cloned().unwrap_or_default())
                .hexpand(true)
                .build();
            page.append(&labeled(label, &e));
            ((*id).to_string(), e)
        })
        .collect();

    let save = Button::with_label("Save settings");
    save.add_css_class("nn-accent");
    save.set_halign(Align::Start);
    page.append(&save);

    save.connect_clicked({
        let on_saved = on_saved.clone();
        let policy_ids = policy_ids.clone();
        let scope_ids = scope_ids.clone();
        let density_ids = density_ids.clone();
        move |_| {
            let mut s = Settings::default();
            s.discovery_classic = disc_classic.is_active();
            s.discovery_scan = disc_scan.is_active();
            s.discovery_background = disc_bg.is_active();
            s.discovery_interval_sec = interval.value() as u64;
            s.scan_subnets = subnets
                .text()
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            s.scan_ports = ports
                .text()
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            let policy_id = selected_id(&policy, &policy_ids);
            s.default_mount_policy = MountPolicy::parse(&policy_id).unwrap_or_default();
            let scope_id = selected_id(&scope, &scope_ids);
            s.default_credential_scope = CredentialScope::parse(&scope_id).unwrap_or_default();
            s.mount_timeout_sec = timeout.value() as u64;
            s.automount_idle_sec = idle.value() as u64;
            s.admin_session_sec = admin_sess.value() as u64;
            s.mount_option_presets = text_to_presets(&presets.text());
            s.ui_density = selected_id(&density, &density_ids);
            if s.ui_density.is_empty() {
                s.ui_density = "comfortable".into();
            }
            let mut map = HashMap::new();
            for (id, e) in &shortcut_entries {
                map.insert(id.clone(), e.text().to_string());
            }
            s.shortcuts = map;
            if let Err(e) = dbus::set_settings_sync(&s) {
                eprintln!("settings save failed: {e}");
            }
            on_saved();
        }
    });

    scroll.set_child(Some(&page));
    outer.append(&scroll);
    outer
}

fn dropdown_ids(pairs: &[(&str, &str)], selected: &str) -> (DropDown, Vec<String>) {
    let labels: Vec<&str> = pairs.iter().map(|(_, l)| *l).collect();
    let ids: Vec<String> = pairs.iter().map(|(id, _)| (*id).to_string()).collect();
    let dd = DropDown::from_strings(&labels);
    if let Some(idx) = ids.iter().position(|id| id == selected) {
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

fn section(title: &str) -> Label {
    Label::builder()
        .label(title)
        .halign(Align::Start)
        .margin_top(12)
        .css_classes(["nn-nav-title"])
        .build()
}

fn labeled(text: &str, w: &impl IsA<gtk4::Widget>) -> Grid {
    let g = Grid::new();
    g.set_column_spacing(12);
    g.set_row_spacing(4);
    let l = Label::new(Some(text));
    l.set_halign(Align::Start);
    l.set_width_chars(28);
    g.attach(&l, 0, 0, 1, 1);
    g.attach(w, 1, 0, 1, 1);
    g
}

fn presets_to_text(map: &HashMap<String, String>) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn text_to_presets(s: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for part in s.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}
