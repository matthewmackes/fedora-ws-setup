use iced::widget::{column, row};
use iced::Element;

use crate::widgets::{
    child_nav_rows, choice, page_frame, section_label, settings_row, status_row, toggle_row,
};
use crate::{system, Control, Message};

fn gesture_choices(p: &mde_theme::Palette, current: &str, which: &str) -> Element<'static, Message> {
    let current = current.to_lowercase();
    let pick = |label: &str, id: &str| {
        choice(
            p,
            label,
            current == id,
            Message::SetGesture(which.into(), id.into()),
        )
    };
    row![
        pick("None", "none"),
        pick("Task view", "taskview"),
        pick("Desktop", "desktop"),
        pick("Desktops ←", "desktop_prev"),
        pick("Desktops →", "desktop_next"),
        pick("Start", "start"),
        pick("Overview", "overview"),
    ]
    .spacing(6)
    .into()
}

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Bluetooth & devices",
        Some("Printers, mouse, touchpad, nearby sharing"),
        child_nav_rows(p, "devices"),
    )
}

pub fn bluetooth(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Bluetooth",
        Some("Add and manage Bluetooth devices"),
        column![
            toggle_row(
                p,
                "bluetooth",
                "Bluetooth",
                None::<String>,
                app.cfg.active().network.bluetooth,
                Message::ToggleBt,
            ),
            status_row(
                p,
                "network-wireless",
                "Adapter",
                if app.cfg.active().network.bluetooth {
                    "On"
                } else {
                    "Off"
                },
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn printers(_app: &Control) -> Element<'_, Message> {
    let p = &_app.palette;
    page_frame(
        p,
        "Printers & scanners",
        Some("Add printers and scanners"),
        column![status_row(
            p,
            "printer",
            "Printers",
            "Add via system print settings",
        )]
        .spacing(4)
        .into(),
    )
}

pub fn mouse(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Mouse",
        Some("Buttons, pointer speed"),
        column![
            toggle_row(
                p,
                "input-mouse",
                "Natural scrolling",
                Some("Content moves with your finger"),
                system::mouse_natural_scroll(),
                Message::ToggleMouseNatural,
            ),
            settings_row(
                p,
                "preferences-desktop-accessibility",
                "Mouse pointer",
                Some("Size and style"),
                crate::widgets::link_btn(p, "Open", Message::Navigate("pointer".into())),
            ),
        ]
        .spacing(4)
        .into(),
    )
}

/// A094: Win11-like touchpad gesture map (saved to config; compositor hooks optional).
pub fn touchpad(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let g = &app.cfg.active().gestures;
    page_frame(
        p,
        "Touchpad",
        Some("Taps, scrolling, gestures"),
        column![
            toggle_row(
                p,
                "input-touchpad",
                "Tap to click",
                None::<String>,
                system::touchpad_tap(),
                Message::ToggleTouchpadTap,
            ),
            toggle_row(
                p,
                "input-touchpad",
                "Natural scrolling",
                None::<String>,
                system::touchpad_natural_scroll(),
                Message::ToggleTouchpadNatural,
            ),
            section_label(p, "Three-finger gestures"),
            settings_row(
                p,
                "go-up",
                "Swipe up",
                Some("Default · Task View"),
                gesture_choices(p, &g.three_up, "three_up"),
            ),
            settings_row(
                p,
                "go-down",
                "Swipe down",
                Some("Show desktop"),
                gesture_choices(p, &g.three_down, "three_down"),
            ),
            settings_row(
                p,
                "go-previous",
                "Swipe left",
                Some("Previous virtual desktop"),
                gesture_choices(p, &g.three_left, "three_left"),
            ),
            settings_row(
                p,
                "go-next",
                "Swipe right",
                Some("Next virtual desktop"),
                gesture_choices(p, &g.three_right, "three_right"),
            ),
            section_label(p, "Four-finger gestures"),
            settings_row(
                p,
                "go-up",
                "Swipe up",
                Some("Overview / widgets"),
                gesture_choices(p, &g.four_up, "four_up"),
            ),
            settings_row(
                p,
                "input-touchpad",
                "Tap",
                Some("Open Start"),
                gesture_choices(p, &g.four_tap, "four_tap"),
            ),
            status_row(
                p,
                "dialog-information",
                "Backend",
                "Gestures stored in config.toml · libinput 4-finger when hardware allows",
            ),
        ]
        .spacing(4)
        .into(),
    )
}