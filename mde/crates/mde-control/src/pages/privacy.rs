use iced::widget::column;
use iced::Element;

use crate::widgets::{
    child_nav_rows, empty_state, hero, link_btn, page_frame, quiet_btn, section_label, settings_row,
    status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let fw = system::firewall_status();
    let fw_on = fw.to_lowercase().contains("running");
    let privacy = system::privacy_in_use();
    let indicators = app.cfg.active().accessibility.privacy_indicators;
    let mut body = column![
        section_label(p, "Security"),
        settings_row(
            p,
            "security-high",
            "Firewall & network protection",
            Some(if fw_on {
                "On"
            } else {
                "Off — your PC may be exposed"
            }),
            link_btn(p, "Open", Message::Navigate("firewall".into())),
        ),
        status_row(p, "security-medium", "SELinux", &system::selinux_status()),
        status_row(
            p,
            "system-software-update",
            "Windows Update",
            "Check for updates",
        ),
        link_btn(p, "Windows Update", Message::Navigate("update".into())),
        section_label(p, "Privacy dashboard"),
        toggle_row(
            p,
            "security-high",
            "Taskbar privacy indicators",
            Some("Show when camera, microphone, or location is in use (A092)"),
            indicators,
            Message::TogglePrivacyIndicators,
        ),
        quiet_btn(p, "Refresh activity", Message::RefreshPrivacyDashboard),
    ]
    .spacing(4);
    if privacy.is_empty() {
        body = body.push(status_row(
            p,
            "emblem-ok",
            "In use now",
            "No camera / mic / location activity detected",
        ));
    } else {
        for (kind, detail) in &privacy {
            body = body.push(status_row(
                p,
                match kind.as_str() {
                    "camera" => "camera-web",
                    "microphone" => "audio-input-microphone",
                    "location" => "find-location",
                    _ => "dialog-information",
                },
                kind.as_str(),
                detail.as_str(),
            ));
        }
    }
    body = body
        .push(section_label(p, "App permissions"))
        .push(child_nav_rows(p, "privacy"));
    page_frame(
        p,
        "Privacy & security",
        Some("Windows Security, camera, microphone, location"),
        body.into(),
    )
}

pub fn security(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut body = column![section_label(p, "Health cards")].spacing(4);
    for (title, detail, ok) in system::security_health_cards() {
        let caption = if ok {
            format!("OK · {detail}")
        } else {
            format!("Attention · {detail}")
        };
        body = body.push(settings_row(
            p,
            if ok {
                "security-high"
            } else {
                "security-medium"
            },
            title,
            Some(caption),
            link_btn(p, "Open", Message::Navigate("home".into())),
        ));
    }
    body = body
        .push(section_label(p, "Firewall"))
        .push(hero(
            p,
            iced::widget::text(system::firewall_status()).size(12).into(),
        ))
        .push(section_label(p, "SELinux"))
        .push(status_row(
            p,
            "security-medium",
            "Status",
            &system::selinux_status(),
        ))
        .push(empty_state(
            p,
            "Encryption & Secure Boot",
            "Check cryptsetup and firmware settings for device encryption and Secure Boot.",
            None,
        ))
        .push(link_btn(
            p,
            "Firewall details",
            Message::Navigate("firewall".into()),
        ));
    page_frame(
        p,
        "Windows Security",
        Some("Firewall, SELinux, encryption, Secure Boot"),
        body.into(),
    )
}

pub fn camera(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Camera",
        Some("Cameras, apps that use the camera"),
        column![
            status_row(p, "camera-web", "Devices", &system::camera_devices()),
            empty_state(
                p,
                "App access",
                "Grant camera access per-app via Flatpak permissions or PipeWire.",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn microphone(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut col = column![
        status_row(p, "audio-input-microphone", "Volume", &system::volume_summary()),
        section_label(p, "Input devices"),
    ]
    .spacing(4);
    if app.sources.is_empty() {
        col = col.push(empty_state(
            p,
            "No microphones",
            "No input sources reported.",
            None,
        ));
    }
    for source in &app.sources {
        col = col.push(status_row(
            p,
            "audio-input-microphone",
            &source.label,
            if source.default { "Default" } else { "" },
        ));
    }
    page_frame(p, "Microphone", Some("Microphone access"), col.into())
}

pub fn location(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let present = system::which("geoclue") || system::which("geoclue2");
    page_frame(
        p,
        "Location",
        Some("Location services"),
        column![
            status_row(
                p,
                "find-location",
                "GeoClue",
                if present {
                    "Installed"
                } else {
                    "Not detected"
                },
            ),
            empty_state(
                p,
                "Location access",
                "Apps request location through the portal when GeoClue is running.",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn activity(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let uses = system::privacy_in_use();
    let mut col = column![
        section_label(p, "Privacy dashboard"),
        settings_row(
            p,
            "view-refresh",
            "Refresh sensors",
            Some("Camera, microphone, and location (best-effort)"),
            crate::widgets::accent_btn(p, "Refresh", Message::RefreshPrivacyDashboard),
        ),
        toggle_row(
            p,
            "camera-web",
            "Taskbar privacy indicators",
            Some("Shows when mic/camera/location are in use"),
            app.cfg.active().accessibility.privacy_indicators,
            Message::TogglePrivacyIndicators,
        ),
    ]
    .spacing(4);
    if uses.is_empty() {
        col = col.push(empty_state(
            p,
            "No active sensors",
            "Nothing is using the camera, microphone, or location right now.",
            None,
        ));
    } else {
        col = col.push(section_label(p, "In use now"));
        for (kind, detail) in uses {
            col = col.push(status_row(
                p,
                match kind.as_str() {
                    "Camera" => "camera-web",
                    "Microphone" => "audio-input-microphone",
                    _ => "find-location",
                },
                &kind,
                &detail,
            ));
        }
    }
    col = col.push(section_label(p, "Activity history"));
    col = col.push(empty_state(
        p,
        "Recent files",
        "Tracked by the shell and xdg recent documents. Clear from Start → Recommended or Files.",
        None,
    ));
    page_frame(
        p,
        "Activity history",
        Some("Privacy indicators and recent activity"),
        col.into(),
    )
}
