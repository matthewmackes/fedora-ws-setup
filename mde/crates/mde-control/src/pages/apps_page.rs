use iced::widget::{column, image, row, text, text_input};
use iced::{Element, Fill};

use crate::community;
use crate::widgets::{
    accent_btn, child_nav_rows, empty_state, page_frame, quiet_btn, section_label, settings_row,
    status_row, toggle_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Apps",
        Some("Installed apps, default apps, startup"),
        child_nav_rows(p, "apps"),
    )
}

pub fn installed(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let q = app.apps_query.to_lowercase();
    let mut list = column![
        settings_row(
            p,
            "system-search",
            "Search",
            None::<String>,
            text_input("Search installed apps", &app.apps_query)
                .on_input(Message::AppsQuery)
                .width(280)
                .into(),
        ),
        status_row(
            p,
            "view-grid",
            "Desktop entries",
            &format!("{}", app.apps.len()),
        ),
    ]
    .spacing(4);
    let mut shown = 0usize;
    for app_entry in &app.apps {
        if shown >= 80 {
            list = list.push(text("… refine search to see more").size(12));
            break;
        }
        if !q.is_empty()
            && !app_entry.name.to_lowercase().contains(&q)
            && !app_entry.id.to_lowercase().contains(&q)
        {
            continue;
        }
        shown += 1;
        let face: Element<'_, Message> =
            if let Some(path) = mde_theme::icons::for_ui(&app_entry.icon, 32, p.dark) {
                image(path).width(32).height(32).into()
            } else {
                text(app_entry.name.chars().next().unwrap_or('•').to_string())
                    .size(18)
                    .into()
            };
        let id = app_entry.id.clone();
        let id_u = id.clone();
        let id_r = id.clone();
        list = list.push(
            crate::widgets::card(
                p,
                row![
                    face,
                    column![
                        text(app_entry.name.clone()).size(14),
                        text(app_entry.id.clone()).size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    quiet_btn(p, "Open", Message::LaunchApp(app_entry.id.clone())),
                    quiet_btn(p, "Uninstall", Message::UninstallApp(id_u)),
                    quiet_btn(p, "Reset data", Message::RepairApp(id_r)),
                    quiet_btn(p, "Pin bar", Message::PinTaskbar(app_entry.id.clone())),
                    quiet_btn(p, "Pin Start", Message::PinStart(app_entry.id.clone())),
                ]
                .spacing(8)
                .padding([10, 0])
                .align_y(iced::Alignment::Center)
                .into(),
            ),
        );
    }
    if shown == 0 {
        list = list.push(empty_state(
            p,
            "No matching apps",
            "Try another name.",
            None,
        ));
    }
    page_frame(
        p,
        "Installed apps",
        Some("Uninstall and manage apps on your PC"),
        list.into(),
    )
}

pub fn defaults(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let pick_id = app.apps_query.trim();
    let mut col = column![
        settings_row(
            p,
            "system-search",
            "Choose default app",
            Some("Search installed apps, then Set default on a row"),
            text_input("App id e.g. firefox.desktop", &app.apps_query)
                .on_input(Message::AppsQuery)
                .width(280)
                .into(),
        ),
        settings_row(
            p,
            "web-browser",
            "Web browser",
            Some(&community::xdg_mime_default("x-scheme-handler/http")),
            quiet_btn(p, "Set", Message::SetMimeDefault(
                "x-scheme-handler/http".into(),
                pick_id.to_string(),
            )),
        ),
    ]
    .spacing(4);
    for slot in community::default_app_slots() {
        let mime = slot.mime.to_string();
        col = col.push(settings_row(
            p,
            "text-x-generic",
            slot.label,
            Some(&slot.current),
            quiet_btn(
                p,
                "Set",
                Message::SetMimeDefault(mime.clone(), pick_id.to_string()),
            ),
        ));
    }
    col = col.push(toggle_row(
        p,
        "text-x-generic",
        "Show file extensions",
        Some("Thunar / GTK file chooser"),
        system::show_file_extensions(),
        |v| Message::ToggleShowExtensions(v),
    ));
    col = col.push(toggle_row(
        p,
        "view-reveal",
        "Show hidden files",
        None::<String>,
        system::show_hidden_files(),
        |v| Message::ToggleShowHiddenFiles(v),
    ));
    page_frame(
        p,
        "Default apps",
        Some("Defaults for file and link types"),
        col.into(),
    )
}

pub fn startup(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let q = app.startup_query.to_lowercase();
    let entries = community::list_startup_entries();
    let mut col = column![
        settings_row(
            p,
            "system-search",
            "Search",
            None::<String>,
            text_input("Filter startup apps", &app.startup_query)
                .on_input(Message::StartupQuery)
                .width(280)
                .into(),
        ),
        text("Toggle writes Hidden=/X-GNOME-Autostart-enabled in ~/.config/autostart.").size(12),
    ]
    .spacing(4);
    if entries.is_empty() {
        col = col.push(empty_state(
            p,
            "No startup apps",
            "Nothing is set to start when you sign in.",
            None,
        ));
    }
    for entry in entries {
        if !q.is_empty()
            && !entry.name.to_lowercase().contains(&q)
            && !entry.id.to_lowercase().contains(&q)
        {
            continue;
        }
        let path = entry.path.display().to_string();
        col = col.push(toggle_row(
            p,
            "system-run",
            &format!("{} · impact {}", entry.name, entry.impact),
            Some(&entry.id),
            entry.enabled,
            move |v| Message::ToggleStartup(path.clone(), v),
        ));
    }
    page_frame(
        p,
        "Startup",
        Some("Apps that start when you sign in"),
        col.into(),
    )
}