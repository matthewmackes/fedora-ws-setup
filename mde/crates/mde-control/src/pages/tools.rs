//! PowerToys-like / community tools Settings (A015–A026, A030, A032, A040–A042).

use iced::widget::{column, row, text, text_input, Space};
use iced::Element;

use crate::widgets::{
    accent_btn, choice, page_frame, quiet_btn, section_label, settings_row, status_row, toggle_row,
};
use crate::{Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let t = &app.cfg.active().tools;
    page_frame(
        p,
        "Productivity tools",
        Some("Command Palette, FancyZones, Awake, and more"),
        column![
            section_label(p, "Launchers"),
            toggle_row(
                p,
                "system-search",
                "Command Palette",
                Some("Super+Alt+Space · apps, files, calc, shell"),
                t.command_palette,
                Message::ToggleCommandPalette,
            ),
            status_row(
                p,
                "input-keyboard",
                "Plugins",
                &t.palette_plugins.join(", "),
            ),
            section_label(p, "Windowing"),
            toggle_row(
                p,
                "view-grid",
                "FancyZones",
                Some("Custom snap layouts in the snap flyout"),
                t.fancy_zones,
                Message::ToggleFancyZones,
            ),
            settings_row(
                p,
                "view-grid",
                "Active layout",
                Some("Used when snapping to a custom zone"),
                {
                    let mut r = row![].spacing(8);
                    for layout in &t.zone_layouts {
                        let selected = t.active_zone_layout == layout.name;
                        let label = layout.name.clone();
                        r = r.push(choice(
                            p,
                            &label,
                            selected,
                            Message::SetActiveZoneLayout(layout.name.clone()),
                        ));
                    }
                    r.into()
                },
            ),
            toggle_row(
                p,
                "object-group",
                "Snap Groups",
                Some("Remember and restore snapped window groups"),
                t.snap_groups,
                Message::ToggleSnapGroups,
            ),
            toggle_row(
                p,
                "go-top",
                "Always on top shortcut",
                Some("Win+Ctrl+T toggles the focused window"),
                t.always_on_top_shortcut,
                Message::ToggleAlwaysOnTopShortcut,
            ),
            toggle_row(
                p,
                "emblem-favorite",
                "Always on top title-bar pin",
                Some("Show a pin glyph on always-on-top windows"),
                t.always_on_top_indicator,
                Message::ToggleAlwaysOnTopIndicator,
            ),
            section_label(p, "Utilities"),
            toggle_row(
                p,
                "input-keyboard",
                "Shortcut Guide",
                Some("Super+/ overlay · PowerToys-style (A073)"),
                t.shortcut_guide,
                Message::ToggleShortcutGuide,
            ),
            toggle_row(
                p,
                "image-x-generic",
                "Image Resizer",
                Some("Resize presets + Thunar context action (A076)"),
                t.image_resizer,
                Message::ToggleImageResizer,
            ),
            settings_row(
                p,
                "image-x-generic",
                "Resize an image",
                Some("Pick a file and apply a preset"),
                {
                    let mut r = row![].spacing(8);
                    for preset in &t.image_resizer_presets {
                        r = r.push(quiet_btn(
                            p,
                            preset.as_str(),
                            Message::ResizeImagePick(preset.clone()),
                        ));
                    }
                    r.into()
                },
            ),
            quiet_btn(
                p,
                "Install Thunar context action",
                Message::InstallImageResizerUca,
            ),
            toggle_row(
                p,
                "color-select",
                "Color picker history",
                Some("Super+Shift+C keeps recent hex/rgb/hsl samples"),
                t.color_picker_history,
                Message::ToggleColorPickerHistory,
            ),
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
                "Mouse click highlighter",
                Some("Show a brief ring on click (presentations)"),
                t.mouse_highlighter,
                Message::ToggleMouseHighlighter,
            ),
            settings_row(
                p,
                "appointment-soon",
                "Awake default",
                Some("Minutes for Keep awake (0 = until turned off)"),
                row![
                    text_input("0", &app.awake_minutes_edit)
                        .on_input(Message::SetAwakeMinutesEdit)
                        .width(90),
                    accent_btn(p, "Apply", Message::ApplyAwakeMinutes),
                ]
                .spacing(8)
                .into(),
            ),
            section_label(p, "Clipboard & jump lists"),
            toggle_row(
                p,
                "edit-paste",
                "Advanced Paste transforms",
                Some("Markdown / HTML / JSON / case transforms on Win+V"),
                t.advanced_paste,
                Message::ToggleAdvancedPaste,
            ),
            settings_row(
                p,
                "document-open-recent",
                "Jump list depth",
                Some("Recent files shown on taskbar jump lists"),
                row![
                    quiet_btn(p, "3", Message::SetJumpListMax(3)),
                    quiet_btn(p, "5", Message::SetJumpListMax(5)),
                    quiet_btn(p, "8", Message::SetJumpListMax(8)),
                    quiet_btn(p, "12", Message::SetJumpListMax(12)),
                ]
                .spacing(8)
                .into(),
            ),
            status_row(
                p,
                "document-open-recent",
                "Current depth",
                &t.jump_list_max.to_string(),
            ),
            section_label(p, "Workspaces"),
            workspace_section(app),
            section_label(p, "Keyboard Manager"),
            keyboard_mgr_section(app),
            iced::widget::text(
                "Remaps apply on the next compositor reload. Use keysym names like Caps_Lock → Escape.",
            )
            .size(12),
        ]
        .spacing(4)
        .into(),
    )
}

fn workspace_section(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let t = &app.cfg.active().tools;
    let mut col = column![
        settings_row(
            p,
            "folder",
            "New workspace name",
            Some("Saves current taskbar pins as a launch preset"),
            row![
                text_input("Dev setup", &app.workspace_name_edit)
                    .on_input(Message::SetWorkspaceNameEdit)
                    .width(180),
                accent_btn(p, "Save", Message::SaveWorkspacePreset),
            ]
            .spacing(8)
            .into(),
        ),
    ]
    .spacing(4);
    if t.workspaces.is_empty() {
        col = col.push(text("No workspaces yet.").size(13));
    } else {
        for ws in &t.workspaces {
            let name = ws.name.clone();
            let name2 = ws.name.clone();
            col = col.push(settings_row(
                p,
                "folder",
                ws.name.clone(),
                Some(format!("{} apps", ws.apps.len())),
                row![
                    accent_btn(p, "Launch", Message::LaunchWorkspace(name)),
                    quiet_btn(p, "Delete", Message::DeleteWorkspace(name2)),
                ]
                .spacing(8)
                .into(),
            ));
        }
    }
    col.into()
}

fn keyboard_mgr_section(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let remaps = &app.cfg.active().tools.keyboard_remaps;
    let mut col = column![
        settings_row(
            p,
            "input-keyboard",
            "Add remap",
            Some("from → to keysym"),
            row![
                text_input("Caps_Lock", &app.remap_from_edit)
                    .on_input(Message::SetRemapFromEdit)
                    .width(120),
                text("→").size(14),
                text_input("Escape", &app.remap_to_edit)
                    .on_input(Message::SetRemapToEdit)
                    .width(120),
                accent_btn(p, "Add", Message::AddKeyRemap),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
        ),
    ]
    .spacing(4);
    if remaps.is_empty() {
        col = col.push(text("No remaps configured.").size(13));
    } else {
        for (i, remap) in remaps.iter().enumerate() {
            col = col.push(settings_row(
                p,
                "input-keyboard",
                format!("{} → {}", remap.from, remap.to),
                None::<String>,
                quiet_btn(p, "Remove", Message::RemoveKeyRemap(i)),
            ));
        }
    }
    col.into()
}

pub fn tray_icons(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let hidden = app
        .cfg
        .extras
        .get("tray.hidden")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut col = column![
        iced::widget::text(
            "Choose which StatusNotifier icons stay hidden. The taskbar shows up to four; the rest go in overflow (›)."
        )
        .size(13),
        section_label(p, "Hidden icons"),
    ]
    .spacing(4);
    if hidden.is_empty() {
        col = col.push(text("No icons are hidden.").size(13));
    } else {
        for id in &hidden {
            col = col.push(settings_row(
                p,
                "view-more-symbolic",
                id.clone(),
                Some("Currently hidden from the taskbar"),
                quiet_btn(p, "Show", Message::TrayShowIcon(id.clone())),
            ));
        }
    }
    col = col.push(Space::with_height(8));
    col = col.push(section_label(p, "Known icons"));
    if app.tray_known.is_empty() {
        col = col.push(text("Open the tray once in the shell to populate this list.").size(13));
    } else {
        for id in &app.tray_known {
            let is_hidden = hidden.iter().any(|h| h == id);
            col = col.push(settings_row(
                p,
                "view-more-symbolic",
                id.clone(),
                Some(if is_hidden { "Hidden" } else { "Visible / overflow" }),
                if is_hidden {
                    quiet_btn(p, "Show", Message::TrayShowIcon(id.clone()))
                } else {
                    quiet_btn(p, "Hide", Message::TrayHideIcon(id.clone()))
                },
            ));
        }
    }
    page_frame(
        p,
        "Other system tray icons",
        Some("Show or hide icons in the taskbar corner"),
        col.into(),
    )
}

pub fn notif_apps(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut col = column![
        iced::widget::text(
            "Per-app banners, sounds, and priority. Deep link from Notification Centre selects an app below."
        )
        .size(13),
    ]
    .spacing(4);
    if !app.notif_app_focus.is_empty() {
        col = col.push(status_row(
            p,
            "preferences-system-notifications",
            "Focused app",
            &app.notif_app_focus,
        ));
    }
    if app.notif_app_prefs.is_empty() && app.notif_app_focus.is_empty() {
        col = col.push(text("No per-app prefs yet — mute or open Settings from a notification.").size(13));
    }
    let mut apps = app.notif_app_prefs.clone();
    if !app.notif_app_focus.is_empty()
        && !apps
            .iter()
            .any(|a| a.app.eq_ignore_ascii_case(&app.notif_app_focus))
    {
        apps.insert(
            0,
            crate::AppNotifPrefRow {
                app: app.notif_app_focus.clone(),
                banners: true,
                sound: true,
                priority: false,
            },
        );
    }
    for pref in apps {
        let app_id = pref.app.clone();
        let a1 = pref.app.clone();
        let a2 = pref.app.clone();
        let a3 = pref.app.clone();
        col = col.push(section_label(p, &pref.app));
        col = col.push(toggle_row(
            p,
            "preferences-system-notifications",
            "Show banners",
            None::<String>,
            pref.banners,
            move |v| Message::SetAppNotifBanners(a1.clone(), v),
        ));
        col = col.push(toggle_row(
            p,
            "audio-volume-high",
            "Play a sound",
            None::<String>,
            pref.sound,
            move |v| Message::SetAppNotifSound(a2.clone(), v),
        ));
        col = col.push(toggle_row(
            p,
            "emblem-important",
            "Priority",
            Some("Allowed when Focus priority-only is on"),
            pref.priority,
            move |v| Message::SetAppNotifPriority(a3.clone(), v),
        ));
        let _ = app_id;
    }
    page_frame(
        p,
        "App notifications",
        Some("Banners, sounds, and priority per app"),
        col.into(),
    )
}
