use iced::widget::{column, row, text, text_input, Space};
use iced::Element;
use mde_config::{DisplayProfile, OutputHead};

use crate::widgets::{
    accent_btn, child_nav_rows, choice, empty_state, hero, link_btn, page_frame, quiet_btn,
    section_label, settings_row, status_row, toggle_row,
};
use crate::community;
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "System",
        Some("Display, sound, notifications, power"),
        column![
            status_row(p, "computer", "Device", &system::hostname()),
            child_nav_rows(p, "system"),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn display(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let mut profiles = row![].spacing(8);
    if cfg.displays.profiles.is_empty() {
        profiles = profiles.push(text("No named profiles yet.").size(13));
    } else {
        for prof in &cfg.displays.profiles {
            profiles = profiles.push(quiet_btn(
                p,
                prof.name.clone(),
                Message::ApplyDisplayProfile(prof.name.clone()),
            ));
        }
    }

    let project = cfg.displays.project_mode.as_str();
    let mut heads = column![section_label(p, "Arrange displays")].spacing(4);
    if app.layout_heads.is_empty() {
        heads = heads.push(empty_state(
            p,
            "No displays reported",
            "Refresh after the compositor publishes outputs.",
            Some(("Refresh".into(), Message::RefreshOutputs)),
        ));
    } else {
        let mut canvas = row![].spacing(10);
        for (i, head) in app.layout_heads.iter().enumerate() {
            canvas = canvas.push(
                column![
                    text(format!("#{}", i + 1)).size(15),
                    text(&head.name).size(12),
                    text(format!("{},{}", head.x_text, head.y_text)).size(11),
                ]
                .spacing(2)
                .padding(8),
            );
        }
        heads = heads.push(hero(p, canvas.into()));
        let mut hero_col = column![].spacing(8);
        for head in &app.layout_heads {
            hero_col = hero_col.push(
                text(format!("{} · {} · scale {}", head.name, head.detail, head.scale)).size(14),
            );
            let nx = head.name.clone();
            let ny = head.name.clone();
            let n1 = head.name.clone();
            let n2 = head.name.clone();
            let n125 = head.name.clone();
            let n15 = head.name.clone();
            let n_hdr = head.name.clone();
            let nl = head.name.clone();
            let nr = head.name.clone();
            let nu = head.name.clone();
            let nd = head.name.clone();
            let n_hz = head.name.clone();
            let refresh = if head.refresh_hz > 0.0 {
                format!("{:.0}", head.refresh_hz)
            } else {
                String::new()
            };
            let mut line = row![
                text_input("x", &head.x_text)
                    .on_input(move |s| Message::OutputX(nx.clone(), s))
                    .width(70),
                text_input("y", &head.y_text)
                    .on_input(move |s| Message::OutputY(ny.clone(), s))
                    .width(70),
                quiet_btn(p, "←", Message::NudgeOutput(nl, -80, 0)),
                quiet_btn(p, "→", Message::NudgeOutput(nr, 80, 0)),
                quiet_btn(p, "↑", Message::NudgeOutput(nu, 0, -80)),
                quiet_btn(p, "↓", Message::NudgeOutput(nd, 0, 80)),
                quiet_btn(p, "1×", Message::OutputScale(n1, 1.0)),
                quiet_btn(p, "2×", Message::OutputScale(n2, 2.0)),
            ]
            .spacing(6);
            if cfg.displays.allow_fractional_scale {
                line = line.push(quiet_btn(p, "1.25×", Message::OutputScale(n125, 1.25)));
                line = line.push(quiet_btn(p, "1.5×", Message::OutputScale(n15, 1.5)));
            }
            hero_col = hero_col.push(line);
            hero_col = hero_col.push(
                row![
                    text("Refresh").size(12),
                    quiet_btn(p, "60", Message::OutputRefresh(n_hz.clone(), 60.0)),
                    quiet_btn(p, "120", Message::OutputRefresh(n_hz.clone(), 120.0)),
                    quiet_btn(p, "144", Message::OutputRefresh(n_hz, 144.0)),
                    Space::with_width(8),
                    quiet_btn(
                        p,
                        if head.hdr { "HDR on" } else { "HDR off" },
                        Message::ToggleOutputHdr(n_hdr, !head.hdr),
                    ),
                ]
                .spacing(6),
            );
            if !refresh.is_empty() {
                hero_col = hero_col.push(text(format!("Preferred {refresh} Hz")).size(11));
            }
        }
        hero_col = hero_col.push(
            row![
                accent_btn(p, "Save layout", Message::SaveLayout),
                quiet_btn(p, "Revert", Message::RevertLayout),
                quiet_btn(p, "Apply tuning", Message::ApplyOutputTuning),
                quiet_btn(p, "Refresh", Message::RefreshOutputs),
                quiet_btn(p, "Identify", Message::IdentifyDisplays),
            ]
            .spacing(8),
        );
        heads = heads.push(hero_col);
    }

    let modes = row![
        choice(
            p,
            "PC",
            project.starts_with("pc"),
            Message::ProjectMode("pc".into()),
        ),
        choice(
            p,
            "Duplicate",
            project == "duplicate",
            Message::ProjectMode("duplicate".into()),
        ),
        choice(
            p,
            "Extend",
            project == "extend",
            Message::ProjectMode("extend".into()),
        ),
        choice(
            p,
            "Second",
            project == "second",
            Message::ProjectMode("second".into()),
        ),
    ]
    .spacing(8);

    page_frame(
        p,
        "Display",
        Some("Monitors, brightness, night light, scale"),
        column![
            heads,
            section_label(p, "Project (Super+P)"),
            settings_row(
                p,
                "video-display",
                "Project mode",
                Some("Four Win11 modes · saved in config"),
                modes.into(),
            ),
            section_label(p, "Brightness & colour"),
            toggle_row(
                p,
                "weather-clear-night",
                "Night light",
                Some("Warmer colours at night"),
                cfg.session.night_light,
                Message::ToggleNightLight,
            ),
            settings_row(
                p,
                "go-next",
                "Night light settings",
                Some("Schedule and strength"),
                link_btn(p, "Open", Message::Navigate("nightlight".into())),
            ),
            section_label(p, "Scale & layout"),
            toggle_row(
                p,
                "zoom-fit-best",
                "Allow fractional scale",
                Some("Enable 125% and 150% scale buttons"),
                cfg.displays.allow_fractional_scale,
                Message::ToggleFractional,
            ),
            toggle_row(
                p,
                "application-x-executable",
                "XWayland",
                Some("Run X11 apps (off by default)"),
                cfg.session.xwayland,
                Message::ToggleXwayland,
            ),
            section_label(p, "Display profiles"),
            profiles,
            status_row(
                p,
                "video-display",
                "Main display",
                if cfg.displays.main_display.is_empty() {
                    "(auto)"
                } else {
                    &cfg.displays.main_display
                },
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn nightlight(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    page_frame(
        p,
        "Night light",
        Some("Warmer colours at night"),
        column![
            toggle_row(
                p,
                "weather-clear-night",
                "Night light",
                Some("Uses wlsunset or gammastep at 4000K"),
                cfg.session.night_light,
                Message::ToggleNightLight,
            ),
            toggle_row(
                p,
                "preferences-system-time",
                "Schedule",
                Some("Follow the dark colour schedule window"),
                cfg.theme.dark_schedule,
                Message::ToggleDarkSchedule,
            ),
            settings_row(
                p,
                "appointment-soon",
                "Hours",
                Some("From / until (local HH:MM)"),
                row![
                    text_input("20:00", &cfg.theme.dark_from).on_input(Message::SetDarkFrom).width(90),
                    text_input("07:00", &cfg.theme.dark_until).on_input(Message::SetDarkUntil).width(90),
                ]
                .spacing(8)
                .into(),
            ),
            link_btn(p, "Display", Message::Navigate("display".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn save_layout(app: &mut Control) {
    let mut outputs = Vec::new();
    for head in &app.layout_heads {
        let x = match head.x_text.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                app.status = format!("Bad x for {}", head.name);
                return;
            }
        };
        let y = match head.y_text.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                app.status = format!("Bad y for {}", head.name);
                return;
            }
        };
        outputs.push(OutputHead {
            name: head.name.clone(),
            enabled: true,
            mode: head.mode.clone(),
            scale: head.scale,
            transform: "normal".into(),
            x,
            y,
            refresh_hz: head.refresh_hz,
            hdr: head.hdr,
        });
    }
    {
        let displays = &mut app.cfg.active_mut().displays;
        if let Some(profile) = displays.profiles.iter_mut().find(|p| p.name == "layout") {
            profile.outputs = outputs;
        } else {
            displays.profiles.push(DisplayProfile {
                name: "layout".into(),
                outputs,
            });
        }
    }
    if let Err(e) = app.cfg.save() {
        app.status = format!("Save failed: {e}");
        return;
    }
    if let Err(e) = system::write_layout(&app.layout_heads) {
        app.status = e;
        return;
    }
    let _ = std::process::Command::new("mde-comp")
        .args(["ctl", "apply-profile", "layout"])
        .output();
    for head in &app.layout_heads {
        let _ = community::apply_head_tuning(head);
    }
    app.status = "Layout saved".into();
}

pub fn sound(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut col = column![
        hero(
            p,
            column![
                text(system::volume_summary()).size(16),
                row![
                    quiet_btn(p, "Vol −", Message::VolDown),
                    quiet_btn(p, "Mute", Message::VolMute),
                    quiet_btn(p, "Vol +", Message::VolUp),
                ]
                .spacing(8),
            ]
            .spacing(10)
            .into(),
        ),
        section_label(p, "Output"),
    ]
    .spacing(4);
    if app.sinks.is_empty() {
        col = col.push(empty_state(
            p,
            "No output devices",
            "WirePlumber / PipeWire did not report sinks.",
            None,
        ));
    }
    for sink in &app.sinks {
        col = col.push(audio_row(p, sink, Some(Message::SetSink(sink.id))));
    }
    col = col.push(section_label(p, "Input"));
    if app.sources.is_empty() {
        col = col.push(empty_state(
            p,
            "No microphones",
            "No input sources reported.",
            None,
        ));
    }
    for source in &app.sources {
        col = col.push(audio_row(p, source, Some(Message::SetSource(source.id))));
    }
    col = col.push(settings_row(
        p,
        "multimedia-volume-control",
        "Volume mixer",
        Some("App volume mix"),
        link_btn(p, "Open", Message::Navigate("mixer".into())),
    ));
    page_frame(p, "Sound", Some("Volume levels, output, input"), col.into())
}

pub fn mixer(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let mut col = column![section_label(p, "Apps")].spacing(4);
    if app.streams.is_empty() {
        col = col.push(empty_state(
            p,
            "No application streams",
            "Apps playing sound will appear here.",
            None,
        ));
    }
    for stream in &app.streams {
        col = col.push(audio_row(p, stream, None));
    }
    page_frame(p, "Volume mixer", Some("App volume mix"), col.into())
}

fn audio_row(
    palette: &mde_theme::Palette,
    node: &system::AudioSink,
    choose: Option<Message>,
) -> Element<'static, Message> {
    let id = node.id;
    let mut controls = row![
        quiet_btn(palette, "Vol −", Message::NodeVol(id, -5)),
        quiet_btn(palette, "Vol +", Message::NodeVol(id, 5)),
        quiet_btn(palette, "Mute", Message::NodeMute(id)),
    ]
    .spacing(6);
    if let Some(message) = choose {
        let mark = if node.default { "Default" } else { "Use" };
        controls = row![accent_btn(palette, mark, message), controls].spacing(8);
    }
    settings_row(
        palette,
        "audio-volume-high",
        node.label.clone(),
        Some(format!("Node {id}")),
        controls.into(),
    )
}

pub fn notifications(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    page_frame(
        p,
        "Notifications",
        Some("Alerts from apps and system"),
        column![
            toggle_row(
                p,
                "preferences-system-notifications",
                "Notifications",
                Some("Show banners from apps and the system"),
                !cfg.notifications.dnd,
                |v| Message::ToggleDnd(!v),
            ),
            toggle_row(
                p,
                "notifications-disabled",
                "Do not disturb",
                Some("Notifications go to the notification centre only"),
                cfg.notifications.dnd,
                Message::ToggleDnd,
            ),
            toggle_row(
                p,
                "emblem-important",
                "Priority only",
                Some("Focus assist — suppress unknown apps"),
                cfg.notifications.priority,
                Message::TogglePriority,
            ),
            status_row(
                p,
                "appointment-soon",
                "Banner duration",
                &format!("{} seconds", cfg.notifications.toast_seconds),
            ),
            section_label(p, "Quiet hours"),
            settings_row(
                p,
                "alarm",
                "From / until",
                Some("Empty falls back to the dark schedule"),
                row![
                    text_input("22:00", &cfg.notifications.quiet_from)
                        .on_input(Message::SetQuietFrom)
                        .width(90),
                    text_input("07:00", &cfg.notifications.quiet_to)
                        .on_input(Message::SetQuietTo)
                        .width(90),
                ]
                .spacing(8)
                .into(),
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn focus(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let remaining = system::focus_remaining_secs();
    let mut hero_col = column![].spacing(10);
    if let Some(secs) = remaining {
        let mins = secs / 60 + 1;
        hero_col = hero_col.push(text(format!("{mins} min")).size(32));
        hero_col = hero_col.push(text("Focus session in progress. Do not disturb is on.").size(13));
        hero_col = hero_col.push(accent_btn(p, "End session", Message::EndFocusSession));
    } else {
        hero_col = hero_col.push(text("Start a focus session").size(18));
        hero_col = hero_col.push(
            text("Do not disturb turns on for the duration you choose.").size(13),
        );
        hero_col = hero_col.push(
            row![
                quiet_btn(p, "15 min", Message::StartFocusSession(15)),
                quiet_btn(p, "30 min", Message::StartFocusSession(30)),
                quiet_btn(p, "45 min", Message::StartFocusSession(45)),
                accent_btn(p, "60 min", Message::StartFocusSession(60)),
            ]
            .spacing(8),
        );
    }
    page_frame(
        p,
        "Focus",
        Some("Reduce distractions during focus sessions"),
        column![
            hero(p, hero_col.into()),
            toggle_row(
                p,
                "notifications-disabled",
                "Do not disturb",
                None::<String>,
                cfg.notifications.dnd,
                Message::ToggleDnd,
            ),
            toggle_row(
                p,
                "emblem-important",
                "Priority only",
                None::<String>,
                cfg.notifications.priority,
                Message::TogglePriority,
            ),
            section_label(p, "Quiet hours"),
            settings_row(
                p,
                "alarm",
                "From / until",
                None::<String>,
                row![
                    text_input("from", &cfg.notifications.quiet_from)
                        .on_input(Message::SetQuietFrom)
                        .width(90),
                    text_input("until", &cfg.notifications.quiet_to)
                        .on_input(Message::SetQuietTo)
                        .width(90),
                ]
                .spacing(8)
                .into(),
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn power(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let profile = system::power_profile();
    page_frame(
        p,
        "Power & battery",
        Some("Sleep, battery, power mode"),
        column![
            hero(
                p,
                column![
                    text(system::battery_summary()).size(22),
                    text(format!("Power mode · {profile}")).size(13),
                ]
                .spacing(6)
                .into(),
            ),
            section_label(p, "Power mode"),
            settings_row(
                p,
                "battery",
                "Power mode",
                Some("Optimise for efficiency or performance"),
                row![
                    choice(
                        p,
                        "Efficiency",
                        profile.contains("power-saver"),
                        Message::SetPowerProfile("power-saver".into()),
                    ),
                    choice(
                        p,
                        "Balanced",
                        profile.contains("balanced"),
                        Message::SetPowerProfile("balanced".into()),
                    ),
                    choice(
                        p,
                        "Performance",
                        profile.contains("performance"),
                        Message::SetPowerProfile("performance".into()),
                    ),
                ]
                .spacing(8)
                .into(),
            ),
            section_label(p, "Screen and sleep"),
            settings_row(
                p,
                "system-lock-screen",
                "Lock the screen after",
                Some("Seconds of idle before soft lock"),
                row![
                    text_input("300", &app.idle_lock_edit)
                        .on_input(Message::SetIdleLockEdit)
                        .width(90),
                    accent_btn(p, "Apply", Message::ApplyIdleLock),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "preferences-desktop-screensaver",
                "Turn off my screen after",
                Some("DPMS idle seconds"),
                row![
                    text_input("330", &app.idle_dpms_edit)
                        .on_input(Message::SetIdleDpmsEdit)
                        .width(90),
                    accent_btn(p, "Apply", Message::ApplyIdleDpms),
                ]
                .spacing(8)
                .into(),
            ),
            status_row(
                p,
                "appointment-soon",
                "Current",
                &format!(
                    "lock {}s · dpms {}s",
                    cfg.session.idle_lock_seconds, cfg.session.idle_dpms_seconds
                ),
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn storage(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let s = &app.cfg.active().storage;
    let large = system::storage_large_files_hint();
    page_frame(
        p,
        "Storage",
        Some("Storage space, cleanup, temporary files"),
        column![
            hero(p, text(system::disk_root_usage()).size(16).into()),
            section_label(p, "Storage Sense"),
            toggle_row(
                p,
                "emblem-system",
                "Storage Sense",
                Some("Automated cleanup when disk is low"),
                s.sense_enabled,
                Message::ToggleStorageSense,
            ),
            toggle_row(
                p,
                "user-trash",
                "Temporary files",
                Some("Cache, thumbnails, temp"),
                s.sense_temp,
                Message::ToggleStorageSenseTemp,
            ),
            toggle_row(
                p,
                "edit-delete",
                "Recycle Bin",
                Some("Empty trash during cleanup"),
                s.sense_trash,
                Message::ToggleStorageSenseTrash,
            ),
            settings_row(
                p,
                "folder-download",
                "Old Downloads",
                Some("Remove downloads older than N days (0 = never)"),
                row![
                    quiet_btn(p, "Off", Message::SetStorageSenseDays(0)),
                    quiet_btn(p, "30d", Message::SetStorageSenseDays(30)),
                    quiet_btn(p, "60d", Message::SetStorageSenseDays(60)),
                    text(format!("{} days", s.sense_downloads_days)).size(13),
                ]
                .spacing(8)
                .into(),
            ),
            accent_btn(p, "Run cleanup now", Message::RunStorageSense),
            section_label(p, "Large files"),
            hero(p, text(large).size(12).into()),
            section_label(p, "Volumes"),
            hero(p, text(app.disks.clone()).size(12).into()),
            quiet_btn(p, "Refresh", Message::RefreshMmc),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn clipboard(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let sync_hint = if cfg.clipboard.sync_dir.trim().is_empty() {
        "~/.local/share/mde/clipboard-sync"
    } else {
        cfg.clipboard.sync_dir.as_str()
    };
    let clear_on_lock = app
        .cfg
        .extras
        .get("clipboard.clear_on_lock")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max = app
        .cfg
        .extras
        .get("clipboard.max_items")
        .and_then(|v| {
            v.as_integer()
                .map(|i| i as u32)
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(cfg.clipboard.max_items);
    page_frame(
        p,
        "Clipboard",
        Some("Cut and copy history"),
        column![
            toggle_row(
                p,
                "edit-copy",
                "Clipboard history",
                Some("Win+V history in the shell"),
                cfg.clipboard.history,
                Message::ToggleClip,
            ),
            status_row(p, "view-list", "Max items", &format!("{max}")),
            toggle_row(
                p,
                "folder-remote",
                "Cross-device sync",
                Some("Mirror a folder with Syncthing or Nextcloud"),
                cfg.clipboard.cross_device,
                Message::ToggleCrossDevice,
            ),
            toggle_row(
                p,
                "system-lock-screen",
                "Clear clipboard on lock",
                Some("Removes unpinned clips when locking"),
                clear_on_lock,
                Message::ToggleClipClearOnLock,
            ),
            settings_row(
                p,
                "folder",
                "Sync folder",
                Some(sync_hint),
                text_input(sync_hint, &cfg.clipboard.sync_dir)
                    .on_input(Message::SetClipSyncDir)
                    .width(280)
                    .into(),
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn about(_app: &Control) -> Element<'_, Message> {
    let p = &_app.palette;
    page_frame(
        p,
        "About",
        Some("Device specifications"),
        column![
            hero(
                p,
                column![
                    text(system::hostname()).size(22),
                    text(system::about_specs()).size(12),
                ]
                .spacing(8)
                .into(),
            ),
            status_row(p, "avatar-default", "Signed in as", &system::user_display_name()),
            settings_row(
                p,
                "system-software-update",
                "Windows Update",
                Some("Check for updates"),
                link_btn(p, "Open", Message::Navigate("update".into())),
            ),
            settings_row(
                p,
                "preferences-system",
                "System",
                Some("Display, sound, power"),
                link_btn(p, "Open", Message::Navigate("system".into())),
            ),
        ]
        .spacing(4)
        .into(),
    )
}
