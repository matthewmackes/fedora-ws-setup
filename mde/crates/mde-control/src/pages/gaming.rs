use iced::widget::{column, row, text_input};
use iced::Element;

use crate::widgets::{
    accent_btn, child_nav_rows, empty_state, page_frame, quiet_btn, section_label, settings_row,
    status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Gaming",
        Some("Game Mode, captures"),
        child_nav_rows(p, "gaming"),
    )
}

pub fn gamemode(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let status = system::gamemode_status();
    let missing = status.contains("needs the gamemode");
    let on = !status.to_lowercase().contains("masked") && !missing;
    if missing {
        return page_frame(
            p,
            "Game Mode",
            Some("Optimise your PC for play"),
            empty_state(
                p,
                "Game Mode needs the gamemode package",
                "Install gamemode, then return here to enable it.",
                None,
            ),
        );
    }
    page_frame(
        p,
        "Game Mode",
        Some("Optimise your PC for play"),
        column![
            toggle_row(
                p,
                "applications-games",
                "Game Mode",
                Some("Give games priority for the processor and graphics"),
                on,
                Message::ToggleGameMode,
            ),
            status_row(p, "dialog-information", "Status", &status),
            toggle_row(
                p,
                "preferences-system-notifications",
                "Pause notifications in Game Mode",
                Some("Also toggles Do not disturb while Game Mode is on (A066)"),
                app.cfg.active().session.game_mode_dnd,
                Message::ToggleGameModeDnd,
            ),
            iced::widget::text(
                "Steam: set launch options to gamemoderun %command%. Other games: start with gamemoderun."
            )
            .size(12),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn captures(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let dir = system::screenshot_dir(&app.cfg);
    page_frame(
        p,
        "Captures",
        Some("Save location, screenshot shortcuts"),
        column![
            settings_row(
                p,
                "folder-pictures",
                "Captures location",
                Some(dir.as_str()),
                row![
                    quiet_btn(p, "Change", Message::PickScreenshotDir),
                    accent_btn(p, "Open folder", Message::OpenScreenshotDir),
                ]
                .spacing(8)
                .into(),
            ),
            {
                let edit: Element<'_, Message> = row![
                    text_input("~/Pictures/Screenshots", &app.screenshot_dir_edit)
                        .on_input(Message::SetScreenshotDirEdit)
                        .padding(8)
                        .width(iced::Length::Fill),
                    quiet_btn(p, "Apply", Message::ApplyScreenshotDir),
                ]
                .spacing(8)
                .into();
                edit
            },
            section_label(p, "Shortcuts"),
            status_row(p, "camera-photo", "Open snipping UI", "Print / Shift+Print / Super+Shift+S"),
            status_row(p, "camera-photo", "Snip UI · Window mode", "Super+Print"),
            status_row(p, "camera-photo", "Edit last shot (toast click)", "swappy"),
            status_row(
                p,
                "media-record",
                "Screen recording",
                "Out of scope · screenshots and snip overlay only",
            ),
            iced::widget::text(
                "Captures save as PNG under the folder above. Toast click opens the last shot in swappy."
            )
            .size(12),
        ]
        .spacing(4)
        .into(),
    )
}
