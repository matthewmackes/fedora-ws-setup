use iced::widget::{column, row, text, text_input};
use iced::Element;

use crate::widgets::{
    accent_btn, child_nav_rows, choice, empty_state, hero, link_btn, page_frame, quiet_btn,
    section_label, settings_row, status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let a = &app.cfg.active().accessibility;
    page_frame(
        p,
        "Accessibility",
        Some("Text size, visual effects, contrast, keyboard"),
        column![
            iced::widget::text(
                "Super+U opens Accessibility. Magnifier · Super+Plus / Win+Esc to exit."
            )
            .size(13),
            section_label(p, "Vision"),
            child_nav_rows(p, "accessibility"),
            section_label(p, "Magnifier"),
            toggle_row(
                p,
                "zoom-in",
                "Compositor-native magnifier",
                Some("Prefer MDE output-scale zoom over magnus/kmag (A070)"),
                a.magnifier_native,
                Message::ToggleMagnifierNative,
            ),
            settings_row(
                p,
                "zoom-fit-best",
                "Zoom factor",
                Some(&format!("×{:.2}", a.magnifier_factor)),
                row![
                    quiet_btn(p, "1.5×", Message::SetMagnifierFactor(1.5)),
                    quiet_btn(p, "2×", Message::SetMagnifierFactor(2.0)),
                    quiet_btn(p, "3×", Message::SetMagnifierFactor(3.0)),
                    quiet_btn(p, "4×", Message::SetMagnifierFactor(4.0)),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "view-fullscreen",
                "Mode",
                Some("Full-screen zoom or lens (follow pointer)"),
                row![
                    choice(
                        p,
                        "Full",
                        a.magnifier_mode.eq_ignore_ascii_case("full"),
                        Message::SetMagnifierMode("full".into()),
                    ),
                    choice(
                        p,
                        "Lens",
                        a.magnifier_mode.eq_ignore_ascii_case("lens"),
                        Message::SetMagnifierMode("lens".into()),
                    ),
                ]
                .spacing(8)
                .into(),
            ),
            toggle_row(
                p,
                "input-keyboard",
                "Follow keyboard focus",
                Some("Keep the magnified region near the focused control"),
                a.magnifier_follow_focus,
                Message::ToggleMagnifierFollowFocus,
            ),
            section_label(p, "Touch & tablet"),
            toggle_row(
                p,
                "input-keyboard",
                "Auto on-screen keyboard",
                Some("Show OSK when a text field is focused on touch (A095)"),
                a.auto_osk,
                Message::ToggleAutoOsk,
            ),
            settings_row(
                p,
                "tablet",
                "Tablet mode",
                Some("Optional tablet posture layout"),
                {
                    let mode = a.tablet_mode.to_lowercase();
                    row![
                        choice(
                            p,
                            "Off",
                            mode == "off" || mode.is_empty(),
                            Message::SetTabletMode("off".into()),
                        ),
                        choice(
                            p,
                            "Auto",
                            mode == "auto",
                            Message::SetTabletMode("auto".into()),
                        ),
                        choice(
                            p,
                            "On",
                            mode == "on",
                            Message::SetTabletMode("on".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            section_label(p, "Coming with system tools"),
            empty_state(
                p,
                "Narrator · Captions",
                "Screen reader and live captions use distribution tools until an MDE backend lands.",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn textsize(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let persisted = app.cfg.active().accessibility.text_scale;
    let current = system::gsettings_get("org.gnome.desktop.interface", "text-scaling-factor");
    let sample_size = current.parse::<f32>().unwrap_or(persisted as f32) * 18.0;
    page_frame(
        p,
        "Text size",
        Some("System-wide text scale for shell, Settings, and apps"),
        column![
            hero(
                p,
                column![
                    text("Text size preview").size(sample_size),
                    text(format!(
                        "Current · gsettings {current} · saved {persisted:.2}"
                    ))
                    .size(12),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "format-text-larger",
                "Text size",
                Some("Factor 0.85–2.25. Persisted in config and applied via gsettings."),
                iced::widget::row![
                    text_input("1.00", &app.text_scale_edit)
                        .on_input(Message::SetTextScaleEdit)
                        .width(90),
                    accent_btn(p, "Apply", Message::ApplyTextScale),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "format-text-larger",
                "Slider",
                Some("System-wide scale 85%–225% (A072)"),
                {
                    let pct = (persisted * 100.0).round().clamp(85.0, 225.0) as u8;
                    row![
                        iced::widget::slider(85..=225, pct, |v| {
                            Message::ApplyTextScaleValue(format!("{:.2}", f32::from(v) / 100.0))
                        })
                        .width(220),
                        text(format!("{pct}%")).size(13),
                    ]
                    .spacing(12)
                    .into()
                },
            ),
            iced::widget::row![
                quiet_btn(p, "100%", Message::ApplyTextScaleValue("1.00".into())),
                quiet_btn(p, "125%", Message::ApplyTextScaleValue("1.25".into())),
                quiet_btn(p, "150%", Message::ApplyTextScaleValue("1.50".into())),
                quiet_btn(p, "175%", Message::ApplyTextScaleValue("1.75".into())),
                quiet_btn(p, "200%", Message::ApplyTextScaleValue("2.00".into())),
            ]
            .spacing(8),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn visual(app: &Control) -> Element<'_, Message> {
    use iced::widget::{row, slider, text};
    let p = &app.palette;
    let cfg = app.cfg.active();
    let opacity = cfg.theme.opacity.clamp(1, 100);
    let mut body = column![
        toggle_row(
            p,
            "preferences-desktop-effects",
            "Transparency effects",
            Some("Same setting as Personalization → Colours"),
            cfg.theme.transparency,
            Message::ToggleTransparency,
        ),
    ]
    .spacing(4);
    if cfg.theme.transparency {
        body = body.push(settings_row(
            p,
            "weather-clear",
            "Opacity",
            Some(&format!("{opacity}% · higher is more solid")),
            row![
                slider(1..=100, opacity, Message::SetOpacity).width(220),
                text(format!("{opacity}%")).size(13),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into(),
        ));
    }
    body = body
        .push(toggle_row(
            p,
            "applications-graphics",
            "Animation effects",
            Some("Window and UI motion throughout MDE and apps that honour it"),
            cfg.theme.animations,
            Message::ToggleAnimations,
        ))
        .push(toggle_row(
            p,
            "view-continuous",
            "Always show scrollbars",
            Some("Turns off overlay scrollbars where the toolkit supports it"),
            cfg.theme.always_show_scrollbars,
            Message::ToggleAlwaysShowScrollbars,
        ))
        .push(status_row(
            p,
            "preferences-system-notifications",
            "Dismiss notifications after",
            &format!("{} seconds", cfg.notifications.toast_seconds),
        ))
        .push(crate::widgets::link_btn(
            p,
            "Colours",
            Message::Navigate("colors".into()),
        ));
    page_frame(
        p,
        "Visual effects",
        Some("Transparency, animation, scrollbars"),
        body.into(),
    )
}

pub fn pointer(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let size = system::gsettings_get("org.gnome.desktop.interface", "cursor-size");
    let t = &app.cfg.active().tools;
    page_frame(
        p,
        "Mouse pointer",
        Some("Mouse pointer colour, size"),
        column![
            status_row(p, "input-mouse", "Cursor size", &size),
            settings_row(
                p,
                "zoom-in",
                "Size",
                Some("Applied via gsettings"),
                iced::widget::row![
                    quiet_btn(p, "24", Message::SetCursorSize(24)),
                    quiet_btn(p, "32", Message::SetCursorSize(32)),
                    quiet_btn(p, "48", Message::SetCursorSize(48)),
                    quiet_btn(p, "64", Message::SetCursorSize(64)),
                    quiet_btn(p, "96", Message::SetCursorSize(96)),
                ]
                .spacing(8)
                .into(),
            ),
            section_label(p, "Find My Mouse"),
            toggle_row(
                p,
                "input-mouse",
                "Find My Mouse",
                Some("Press Ctrl twice to spotlight the pointer"),
                t.find_my_mouse,
                Message::ToggleFindMyMouse,
            ),
            toggle_row(
                p,
                "input-mouse",
                "Click highlighter",
                Some("Brief ring on click for presentations"),
                t.mouse_highlighter,
                Message::ToggleMouseHighlighter,
            ),
            link_btn(p, "Productivity tools", Message::Navigate("tools".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn contrast(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mode = app.cfg.active().theme.mode.to_lowercase();
    page_frame(
        p,
        "Contrast themes",
        Some("System-wide high-contrast packs for low vision"),
        column![
            iced::widget::text(
                "High-contrast packs retint shell chrome and push GTK/icon themes (A071)."
            )
            .size(13),
            settings_row(
                p,
                "preferences-desktop-accessibility",
                "None / Light",
                Some("Standard light palette"),
                choice(
                    p,
                    "Apply",
                    mode == "light" || mode.is_empty(),
                    Message::SetMode("light".into()),
                ),
            ),
            settings_row(
                p,
                "weather-clear-night",
                "Dark",
                Some("Standard dark palette"),
                choice(
                    p,
                    "Apply",
                    mode == "dark",
                    Message::SetMode("dark".into()),
                ),
            ),
            settings_row(
                p,
                "weather-clear-night",
                "High contrast black",
                Some("hc-black · solid black / white text + HighContrastInverse GTK"),
                choice(
                    p,
                    "Apply",
                    mode == "hc-black",
                    Message::SetMode("hc-black".into()),
                ),
            ),
            settings_row(
                p,
                "weather-clear",
                "High contrast white",
                Some("hc-white · solid white / black text + HighContrast GTK"),
                choice(
                    p,
                    "Apply",
                    mode == "hc-white",
                    Message::SetMode("hc-white".into()),
                ),
            ),
            quiet_btn(p, "Open Colours", Message::Navigate("colors".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn shortcuts(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let remaps = &app.cfg.active().tools.keyboard_remaps;
    let guide = app.cfg.active().tools.shortcut_guide;
    let col = column![
        section_label(p, "Shortcut Guide"),
        toggle_row(
            p,
            "input-keyboard",
            "Shortcut Guide overlay",
            Some("Super+/ (PowerToys-style cheat sheet)"),
            guide,
            Message::ToggleShortcutGuide,
        ),
        section_label(p, "Common shortcuts"),
        status_row(p, "preferences-system", "Settings", "Super+I"),
        status_row(p, "preferences-desktop-accessibility", "Accessibility", "Super+U"),
        status_row(p, "start-here", "Start", "Super"),
        status_row(p, "view-paged", "Task View", "Super+Tab"),
        status_row(p, "utilities-system-monitor", "Performance", "Ctrl+Shift+Esc"),
        status_row(p, "view-grid", "Widgets", "Super+W"),
        status_row(p, "system-lock-screen", "Lock", "Super+L"),
        status_row(p, "edit-copy", "Clipboard history", "Super+V"),
        status_row(p, "system-search", "Search", "Super+S"),
        status_row(p, "system-run", "Command Palette", "Super+Alt+Space"),
        status_row(p, "input-keyboard", "Shortcut Guide", "Super+/"),
        status_row(p, "go-top", "Always on top", "Win+Ctrl+T"),
        status_row(p, "color-select", "Color picker", "Super+Shift+C"),
        status_row(p, "zoom-in", "Magnifier", "Super+Plus · Win+Esc exits"),
        section_label(p, "Keyboard Manager"),
        status_row(
            p,
            "input-keyboard",
            "Remaps",
            &format!("{} configured", remaps.len()),
        ),
        link_btn(p, "Edit remaps in Productivity tools", Message::Navigate("tools".into())),
        iced::widget::text("Full bindings live in the compositor and shell config.").size(12),
    ]
    .spacing(4);
    page_frame(
        p,
        "Keyboard",
        Some("Keyboard shortcuts for this desktop"),
        col.into(),
    )
}
