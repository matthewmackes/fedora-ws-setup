use iced::widget::{column, text, text_input};
use iced::Element;

use crate::widgets::{
    child_nav_rows, hero, link_btn, page_frame, quiet_btn, section_label, settings_row,
    status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Time & language",
        Some("Date and time, language and region, typing"),
        child_nav_rows(p, "time"),
    )
}

pub fn datetime(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let clock_24h = app
        .cfg
        .extras
        .get("clock_24h")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = system::timedate_status();
    let ntp = status.to_lowercase().contains("system clock synchronized: yes")
        || status.to_lowercase().contains("ntp service: active");
    let now = mde_theme::now().format(if clock_24h { "%H:%M" } else { "%-I:%M %p" });
    let day = mde_theme::now().format("%A, %B %-d, %Y");
    page_frame(
        p,
        "Date & time",
        Some("Time zones, automatic clock settings"),
        column![
            hero(
                p,
                column![
                    text(now.to_string()).size(36),
                    text(day.to_string()).size(14),
                ]
                .spacing(4)
                .into(),
            ),
            toggle_row(
                p,
                "preferences-system-time",
                "Set time automatically",
                Some("NTP via timedatectl"),
                ntp,
                Message::ToggleNtp,
            ),
            toggle_row(
                p,
                "clock",
                "24-hour clock",
                Some("On the taskbar and in Settings"),
                clock_24h,
                Message::ToggleClock24h,
            ),
            toggle_row(
                p,
                "preferences-system-time",
                "Show seconds on taskbar",
                Some("Also under Personalization → Taskbar"),
                app.cfg.active().shell.clock_seconds,
                Message::ToggleClockSeconds,
            ),
            toggle_row(
                p,
                "x-office-calendar",
                "ISO date on taskbar",
                Some("YYYY-MM-DD"),
                app.cfg.active().shell.clock_date_iso,
                Message::ToggleClockDateIso,
            ),
            quiet_btn(p, "Refresh", Message::RefreshTime),
            section_label(p, "World clocks"),
            iced::widget::text(
                "Up to two IANA zones in Notification Centre (A098)."
            )
            .size(12),
            settings_row(
                p,
                "preferences-system-time",
                "Clock 1",
                Some("e.g. America/New_York"),
                text_input(
                    "America/New_York",
                    app.cfg
                        .active()
                        .shell
                        .world_clocks
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or(""),
                )
                .on_input(|s| Message::SetWorldClock(0, s))
                .width(220)
                .into(),
            ),
            settings_row(
                p,
                "preferences-system-time",
                "Clock 2",
                Some("Optional second zone"),
                text_input(
                    "Europe/London",
                    app.cfg
                        .active()
                        .shell
                        .world_clocks
                        .get(1)
                        .map(|s| s.as_str())
                        .unwrap_or(""),
                )
                .on_input(|s| Message::SetWorldClock(1, s))
                .width(220)
                .into(),
            ),
            section_label(p, "Status"),
            hero(p, text(status).size(11).into()),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn language(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Language & region",
        Some("Language, region, keyboard layouts"),
        column![
            section_label(p, "Locale"),
            hero(p, text(system::localectl_status()).size(12).into()),
            empty_hint(p),
        ]
        .spacing(4)
        .into(),
    )
}

fn empty_hint(p: &mde_theme::Palette) -> Element<'static, Message> {
    crate::widgets::empty_state(
        p,
        "Change language",
        "Use localectl or GNOME Region & Language for system locale and keyboard layouts.",
        None,
    )
}

pub fn typing(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let animations = app.cfg.active().theme.animations;
    page_frame(
        p,
        "Typing",
        Some("Key repeat, Caps Lock"),
        column![
            status_row(
                p,
                "applications-graphics",
                "UI animations",
                if animations { "On" } else { "Off" },
            ),
            settings_row(
                p,
                "input-keyboard",
                "Keyboard shortcuts",
                Some("Accessibility keyboard page"),
                link_btn(p, "Open", Message::Navigate("shortcuts".into())),
            ),
        ]
        .spacing(4)
        .into(),
    )
}
