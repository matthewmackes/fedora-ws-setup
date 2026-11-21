use iced::widget::column;
use iced::Element;

use crate::widgets::{
    accent_btn, child_nav_rows, empty_state, hero, page_frame, section_label, settings_row,
    status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    page_frame(
        p,
        "Network & internet",
        Some("Wi-Fi, airplane mode, ethernet, VPN"),
        column![
            hero(
                p,
                column![
                    iced::widget::text(system::wifi_summary()).size(16),
                    iced::widget::text(system::ethernet_summary()).size(12),
                ]
                .spacing(6)
                .into(),
            ),
            toggle_row(
                p,
                "network-wireless",
                "Wi-Fi",
                None::<String>,
                cfg.network.wifi,
                Message::ToggleWifi,
            ),
            toggle_row(
                p,
                "airplane-mode",
                "Airplane mode",
                Some("Turns off Wi-Fi and Bluetooth"),
                cfg.network.airplane_mode,
                Message::ToggleAirplane,
            ),
            child_nav_rows(p, "network"),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn wifi(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let mut col = column![
        toggle_row(
            p,
            "network-wireless",
            "Wi-Fi",
            Some(system::wifi_summary()),
            cfg.network.wifi,
            Message::ToggleWifi,
        ),
        toggle_row(
            p,
            "airplane-mode",
            "Airplane mode",
            Some("Wi-Fi and Bluetooth stay off while airplane mode is on"),
            cfg.network.airplane_mode,
            Message::ToggleAirplane,
        ),
        section_label(p, "Nearby networks"),
    ]
    .spacing(4);
    if cfg.network.airplane_mode {
        col = col.push(empty_state(
            p,
            "Airplane mode is on",
            "Turn off airplane mode to see Wi-Fi networks.",
            Some(("Turn off".into(), Message::ToggleAirplane(false))),
        ));
    } else if app.wifi_list.is_empty() {
        col = col.push(empty_state(
            p,
            "No networks found",
            "nmcli returned no scan results.",
            None,
        ));
    }
    for net in &app.wifi_list {
        col = col.push(settings_row(
            p,
            "network-wireless",
            net.clone(),
            Some("Select to connect"),
            accent_btn(p, "Connect", Message::WifiConnect(net.clone())),
        ));
    }
    page_frame(p, "Wi-Fi", Some("Connect, manage known networks"), col.into())
}

pub fn ethernet(_app: &Control) -> Element<'_, Message> {
    let p = &_app.palette;
    page_frame(
        p,
        "Ethernet",
        Some("Wired connections"),
        column![
            status_row(p, "network-wired", "Status", &system::ethernet_summary()),
            empty_state(
                p,
                "IP settings",
                "Use nm-connection-editor for DHCP or manual addresses.",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn vpn(_app: &Control) -> Element<'_, Message> {
    let p = &_app.palette;
    let list = system::vpn_list();
    let mut col = column![section_label(p, "VPN connections")].spacing(4);
    if list.is_empty() {
        col = col.push(empty_state(
            p,
            "No VPN profiles",
            "Add a VPN or WireGuard connection in NetworkManager.",
            None,
        ));
    }
    for item in list {
        col = col.push(status_row(p, "network-vpn", &item, "Available"));
    }
    page_frame(p, "VPN", Some("VPN connections"), col.into())
}

pub fn proxy(_app: &Control) -> Element<'_, Message> {
    let p = &_app.palette;
    page_frame(
        p,
        "Proxy",
        Some("Proxy server settings"),
        column![
            status_row(p, "preferences-system-network", "Mode", "Off (system default)"),
            empty_state(
                p,
                "Manual proxy",
                "Set http_proxy / https_proxy in the session environment, or configure NetworkManager.",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

/// W448: full pairing UI for KDE Connect / Nearby Share.
pub fn nearby(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut col = column![
        section_label(p, "Phone Link features"),
        status_row(
            p,
            "mail-message",
            "SMS & notifications",
            "Pair a device · enable SMS + Notification sync in KDE Connect app",
        ),
        status_row(
            p,
            "battery",
            "Phone battery",
            "QS Nearby flyout · refresh after pairing",
        ),
        section_label(p, "KDE Connect"),
        status_row(
            p,
            "network-wireless-connected-symbolic",
            "Status",
            if app.nearby_hint.is_empty() {
                "Refresh to scan"
            } else {
                &app.nearby_hint
            },
        ),
        settings_row(
            p,
            "view-refresh",
            "Refresh devices",
            Some("Starts kdeconnectd on demand · leaves it running"),
            accent_btn(p, "Refresh", Message::RefreshNearby),
        ),
    ]
    .spacing(4);

    if !crate::kdeconnect::installed() {
        col = col.push(empty_state(
            p,
            "kde-connect not installed",
            "sudo dnf install kde-connect · open firewall ports 1716–1764 · see crates/docs/kde-connect.md",
            None,
        ));
    } else if app.nearby_devices.is_empty() {
        col = col.push(empty_state(
            p,
            "No devices",
            "Open KDE Connect on your phone on the same LAN, then Refresh. Unpaired devices can Pair here.",
            Some(("Refresh".into(), Message::RefreshNearby)),
        ));
    }

    for d in &app.nearby_devices {
        let mut state = match (d.paired, d.reachable) {
            (true, true) => "Paired · reachable".to_string(),
            (true, false) => "Paired · offline".to_string(),
            (false, true) => "Unpaired · visible — Pair to connect".to_string(),
            (false, false) => "Unpaired · offline".to_string(),
        };
        // A078: surface battery + plugin hints for Phone-Link parity.
        if d.paired {
            if let Some((pct, charging)) = crate::kdeconnect::device_battery(&d.id) {
                state.push_str(&format!(
                    " · battery {pct}%{}",
                    if charging { " charging" } else { "" }
                ));
            }
            if crate::kdeconnect::notifications_plugin_active(&d.id) {
                state.push_str(" · notifications");
            }
        }
        let action = if d.paired {
            accent_btn(p, "Unpair", Message::NearbyUnpair(d.id.clone()))
        } else {
            accent_btn(p, "Pair", Message::NearbyPair(d.id.clone()))
        };
        col = col.push(settings_row(
            p,
            "phone",
            format!("{} ({})", d.name, d.id),
            Some(state),
            action,
        ));
    }

    col = col
        .push(section_label(p, "Phone Link–class features"))
        .push(status_row(
            p,
            "phone",
            "Clipboard share",
            "QS Nearby · send clipboard to paired device",
        ))
        .push(status_row(
            p,
            "mail-send",
            "Share files / text / URLs",
            "QS Nearby flyout · shareUrl / shareText",
        ))
        .push(status_row(
            p,
            "battery",
            "Battery",
            "Shown per paired device when the battery plugin is on",
        ))
        .push(status_row(
            p,
            "preferences-system-notifications",
            "Notification mirror",
            "Enable the Notifications plugin in KDE Connect on the phone",
        ))
        .push(status_row(
            p,
            "mail-message-new",
            "SMS",
            "Requires kdeconnect SMS plugin · conversations via D-Bus",
        ))
        .push(iced::widget::text(
            "First-run: sudo dnf install kde-connect · open firewall 1716–1764 · see crates/docs/kde-connect.md",
        )
        .size(12));

    page_frame(
        p,
        "Nearby Share",
        Some("Pair phones and tablets via KDE Connect · Phone Link parity"),
        col.into(),
    )
}
