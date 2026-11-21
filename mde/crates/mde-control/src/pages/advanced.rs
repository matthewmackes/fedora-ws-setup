use iced::widget::{column, row, text, text_input, toggler};
use iced::Element;
use mde_config::{ConfigKey, MdeConfig};

use crate::widgets::{
    accent_btn, child_nav_rows, hero, key_value, page_frame, quiet_btn, section_label, settings_row,
    status_row,
};
use crate::{Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let start = &app.cfg.active().start;

    // In-use Start folders (on Start) plus groups already in Advanced (stay listed/checked).
    let mut group_names: Vec<String> = app.cfg.start_folders_in_use();
    for g in &start.advanced_groups {
        if !group_names.iter().any(|n| n.eq_ignore_ascii_case(g)) {
            group_names.push(g.clone());
        }
    }
    group_names.sort_by_key(|s| s.to_lowercase());

    let mut groups_col = column![
        iced::widget::text(
            "Checked folders leave Start and appear below as Advanced groups (flat app list)."
        )
        .size(12),
    ]
    .spacing(4);
    if group_names.is_empty() {
        groups_col = groups_col.push(
            iced::widget::text("No Start folders in use. Create a folder on Start, then check it here.")
                .size(12),
        );
    } else {
        for name in &group_names {
            let checked = start.is_advanced_group(name);
            groups_col = groups_col.push(settings_row(
                p,
                "folder",
                name.clone(),
                Some("Move Start folder to Advanced"),
                toggler(checked)
                    .on_toggle({
                        let name = name.clone();
                        move |on| Message::ToggleAdvancedGroup(name.clone(), on)
                    })
                    .into(),
            ));
        }
    }

    let mut apps_col = column![].spacing(6);
    for group in &start.advanced_groups {
        apps_col = apps_col.push(section_label(p, group.as_str()));
        let folder_apps = app.cfg.start_pin_folder_apps(group);
        if folder_apps.is_empty() {
            apps_col = apps_col.push(iced::widget::text("(empty folder)").size(12));
        } else {
            for id in folder_apps {
                let label = crate::apps::desktop_name_for(&id);
                apps_col = apps_col.push(quiet_btn(p, label, Message::LaunchApp(id)));
            }
        }
    }

    apps_col = apps_col.push(section_label(p, "Apps"));
    if start.advanced_apps.is_empty() {
        apps_col = apps_col.push(
            iced::widget::text("Right-click a Start pin → Move to Advanced.")
                .size(12),
        );
    } else {
        for id in &start.advanced_apps {
            let label = crate::apps::desktop_name_for(id);
            apps_col = apps_col.push(
                row![
                    quiet_btn(p, label, Message::LaunchApp(id.clone())),
                    quiet_btn(p, "Restore to Start", Message::RestoreAdvancedApp(id.clone())),
                ]
                .spacing(8),
            );
        }
    }

    page_frame(
        p,
        "Advanced",
        Some("System tools, Start folders, and apps moved off Start"),
        column![
            section_label(p, "System tools"),
            iced::widget::text(
                "Management Console and the Config Editor for options that go beyond everyday Settings."
            )
            .size(13),
            row![
                accent_btn(p, "Open Management Console", Message::LaunchMmc),
                quiet_btn(p, "Computer Management", Message::LaunchMmcComp),
                quiet_btn(p, "Config Editor", Message::Navigate("config".into())),
            ]
            .spacing(8),
            section_label(p, "For developers"),
            crate::widgets::toggle_row(
                p,
                "utilities-system-monitor",
                "Always show End task",
                Some("Taskbar jump lists always offer End task (A082)"),
                app.cfg.active().shell.end_task_always,
                Message::ToggleAlwaysEndTask,
            ),
            child_nav_rows(p, "advanced"),
            section_label(p, "Start folders in use"),
            groups_col,
            section_label(p, "Advanced apps"),
            apps_col,
        ]
        .spacing(8)
        .into(),
    )
}

/// W458 / Q7: Settings no longer hosts a read-only MMC page — launch the real console.
pub fn mmc(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Management Console",
        Some("Primary administrator interface"),
        column![
            iced::widget::text(
                "Computer Management and Linux admin snap-ins live in mde-mmc — a full console with tree, results, and actions."
            )
            .size(13),
            accent_btn(p, "Open Management Console", Message::LaunchMmc),
            quiet_btn(p, "Open Performance Monitor", Message::LaunchMmcPerf),
            quiet_btn(p, "Open Computer Management", Message::LaunchMmcComp),
            quiet_btn(p, "Open Config Editor", Message::Navigate("config".into())),
            status_row(
                p,
                "utilities-system-monitor",
                "Ctrl+Shift+Esc",
                "Opens MMC Performance (not a third-party task manager)",
            ),
            status_row(
                p,
                "preferences-system",
                "Console",
                "mde-mmc (writable snap-ins; not a Settings page)",
            ),
        ]
        .spacing(8)
        .into(),
    )
}

pub fn editor(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let q = app.editor_search.to_lowercase();
    let keys: Vec<ConfigKey> = MdeConfig::catalog()
        .into_iter()
        .filter(|k| {
            (app.advanced || !k.advanced)
                && (q.is_empty()
                    || k.path.to_lowercase().contains(&q)
                    || k.label.to_lowercase().contains(&q)
                    || k.category.to_lowercase().contains(&q))
        })
        .collect();

    let mut list = column![
        iced::widget::text("Single source of truth for MDE. Changes Save to ~/.config/mde/config.toml.")
            .size(13),
        settings_row(
            p,
            "system-search",
            "Search settings",
            None::<String>,
            text_input("Search settings", &app.editor_search)
                .on_input(Message::EditorSearch)
                .width(260)
                .into(),
        ),
        settings_row(
            p,
            "preferences-system",
            "Advanced keys",
            Some("Show advanced catalog entries"),
            toggler(app.advanced)
                .on_toggle(Message::ToggleAdvanced)
                .into(),
        ),
        settings_row(
            p,
            "document-new",
            "New profile",
            None::<String>,
            row![
                text_input("New profile", &app.new_profile)
                    .on_input(Message::ProfileName)
                    .width(160),
                accent_btn(p, "Create", Message::CreateProfile),
            ]
            .spacing(8)
            .into(),
        ),
        row![
            accent_btn(p, "Save", Message::Save),
            quiet_btn(p, "Reload", Message::Reload),
            quiet_btn(p, "Export", Message::Export),
            quiet_btn(p, "Import", Message::Import),
        ]
        .spacing(8),
        status_row(p, "user-bookmarks", "Active profile", &app.cfg.active_profile),
    ]
    .spacing(4);

    let mut profiles = row![].spacing(6);
    for name in app.cfg.profiles.keys() {
        profiles = profiles.push(quiet_btn(
            p,
            name.clone(),
            Message::SelectProfile(name.clone()),
        ));
    }
    list = list.push(profiles);

    for k in keys {
        let value = key_value(&app.cfg, &k.path);
        list = list.push(settings_row(
            p,
            "document-properties",
            format!("{} · {}", k.category, k.label),
            Some(k.path.clone()),
            text(value).size(13).into(),
        ));
    }

    list = list.push(section_label(p, "Export / import buffer"));
    list = list.push(
        text_input("Paste TOML here, or Export to fill", &app.export_buf)
            .on_input(Message::ExportBuf),
    );
    if app.advanced && !app.export_buf.is_empty() {
        list = list.push(hero(p, text(app.export_buf.clone()).size(11).into()));
    }

    page_frame(p, "Config Editor", Some("Full TOML store"), list.into())
}
