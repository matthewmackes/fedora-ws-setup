use iced::widget::{column, row, text, Space};
use iced::Element;

use crate::widgets::{
    accent_btn, child_nav_rows, empty_state, hero, page_frame, quiet_btn, section_label,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let summary = app.update_summary.clone();
    let lower = summary.to_lowercase();
    let restart = system::offline_update_pending();
    let busy = system::updates_busy();
    let up_to_date = !restart
        && !busy
        && (lower.contains("no packages")
            || lower.contains("you're up to date")
            || summary.trim().is_empty()
            || summary.contains("Press Check"));
    let available = !restart && !busy && !up_to_date && !summary.contains("couldn’t")
        && !summary.contains("Could not");

    let title = if busy {
        "Working on updates"
    } else if restart {
        "Restart required"
    } else if up_to_date && summary.contains("Press Check") {
        "Windows Update"
    } else if up_to_date {
        "You're up to date"
    } else if available {
        "Updates available"
    } else {
        "Windows Update"
    };

    let caption = if busy {
        system::updates_busy_status()
    } else if restart {
        "Updates will be installed when your PC restarts. Use Update and restart or Update and shut down from Start › Power."
            .into()
    } else if summary.contains("Press Check") {
        "Check for Fedora (dnf/pkcon) and Flatpak updates.".into()
    } else {
        "Updates install through PackageKit (pkcon) / dnf and Flatpak. Restart if kernel or offline updates require it.".into()
    };

    let mut actions = row![].spacing(8);
    if restart {
        actions = actions
            .push(accent_btn(p, "Restart now", Message::UpdateRestartNow))
            .push(quiet_btn(p, "Update and shut down", Message::UpdateAndShutDown))
            .push(quiet_btn(p, "Schedule restart", Message::UpdateScheduleRestart))
            .push(quiet_btn(p, "Cancel update", Message::UpdateCancelOffline));
    } else if available {
        actions = actions
            .push(accent_btn(p, "Download & install all", Message::UpdateInstallAll))
            .push(quiet_btn(p, "Check for updates", Message::CheckUpdates));
    } else {
        actions = actions.push(accent_btn(p, "Check for updates", Message::CheckUpdates));
    }

    page_frame(
        p,
        "Windows Update",
        Some("Check for updates, update history"),
        column![
            hero(
                p,
                column![
                    text(title).size(22),
                    text(caption).size(13),
                    actions,
                ]
                .spacing(10)
                .into(),
            ),
            hero(p, text(summary).size(12).into()),
            child_nav_rows(p, "update"),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn history(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let history = if app.update_history.is_empty() {
        system::update_history()
    } else {
        app.update_history.clone()
    };
    let flat = system::flatpak_update_history();
    page_frame(
        p,
        "Update history",
        Some("Updates, installs and removals"),
        column![
            section_label(p, "Quality updates / package changes"),
            if history.contains("failed") || history.trim().is_empty() {
                empty_state(
                    p,
                    "No history",
                    "dnf history was empty or unavailable.",
                    Some(("Refresh".into(), Message::RefreshUpdateHistory)),
                )
            } else {
                hero(p, text(history).size(12).into())
            },
            section_label(p, "Flatpak / app updates"),
            if flat.trim().is_empty() {
                text("No Flatpak history yet.").size(12)
            } else {
                text(flat).size(12)
            },
            Space::with_height(8),
            quiet_btn(p, "Refresh", Message::RefreshUpdateHistory),
        ]
        .spacing(4)
        .into(),
    )
}
