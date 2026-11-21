use iced::widget::{column, row, text, text_input};
use iced::Element;

use crate::widgets::{
    accent_btn, accent_swatches, appearance_card, child_nav_rows, choice, link_btn, page_frame,
    quiet_btn, section_label, settings_row, status_row, toggle_row,
};
use crate::{Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Personalization",
        Some("Background, colours, themes, Start, taskbar"),
        column![appearance_card(p), child_nav_rows(p, "personalization")]
            .spacing(4)
            .into(),
    )
}

pub fn background(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let current = if cfg.theme.wallpaper.trim().is_empty() {
        "(none set)".to_string()
    } else {
        cfg.theme.wallpaper.clone()
    };
    let caption = {
        let title = cfg.theme.bing_title.trim();
        let credit = cfg.theme.bing_copyright.trim();
        if !title.is_empty() && !credit.is_empty() {
            Some(format!("{title} — {credit}"))
        } else if !title.is_empty() {
            Some(title.to_string())
        } else if !credit.is_empty() {
            Some(credit.to_string())
        } else {
            None
        }
    };
    let mut body = column![
        appearance_card(p),
        status_row(p, "preferences-desktop-wallpaper", "Current", &current),
    ]
    .spacing(4);
    if let Some(cap) = caption {
        body = body.push(status_row(p, "text-x-generic", "Bing", &cap));
    }
    body = body
        .push(status_row(p, "view-fullscreen", "Fit", "Fill"))
        .push(settings_row(
            p,
            "folder-pictures",
            "Picture path",
            Some("Absolute path to an image"),
            text_input("/path/to/image.png", &cfg.theme.wallpaper)
                .on_input(Message::SetWallpaper)
                .width(320)
                .into(),
        ))
        .push(accent_btn(p, "Apply wallpaper", Message::ApplyWallpaper))
        .push(accent_btn(
            p,
            "Download Bing wallpaper of the day",
            Message::DownloadBingWallpaper,
        ))
        .push(toggle_row(
            p,
            "emblem-downloads",
            "Daily Bing wallpaper",
            Some("User timer updates ~/Pictures/Bing each day (en-US UHD)"),
            cfg.theme.bing_daily,
            Message::ToggleBingDaily,
        ))
        .push(toggle_row(
            p,
            "image-x-generic",
            "Spotlight wallpaper pack",
            Some("Bing daily → desktop, lock, and ~/Pictures/Spotlight history"),
            cfg.theme.spotlight,
            Message::ToggleSpotlight,
        ))
        .push(toggle_row(
            p,
            "system-lock-screen",
            "Spotlight on lock screen",
            Some("Copies the Spotlight image for the lock face"),
            cfg.theme.spotlight_lock,
            Message::ToggleSpotlightLock,
        ));
    let hist = crate::community::spotlight_history();
    if !hist.is_empty() {
        let cap = hist
            .first()
            .map(|p| format!("Latest · {}", p.display()))
            .unwrap_or_default();
        body = body.push(status_row(p, "folder-pictures", "Spotlight history", &cap));
    }
    page_frame(
        p,
        "Background",
        Some("Background image, colour, Bing wallpaper"),
        body.into(),
    )
}

pub fn colors(app: &Control) -> Element<'_, Message> {
    use iced::widget::slider;
    let p = &app.palette;
    let cfg = app.cfg.active();
    let dark_mode = cfg.theme.mode.eq_ignore_ascii_case("dark");
    let schedule = if app.palette.dark && !dark_mode {
        "Dark colours are on because the schedule window is active."
    } else if app.palette.dark {
        "Dark colours."
    } else {
        "Light colours."
    };
    let opacity = cfg.theme.opacity.clamp(1, 100);
    let mut effects = column![
        section_label(p, "Transparency effects"),
        toggle_row(
            p,
            "preferences-desktop-effects",
            "Transparency effects",
            Some("Mica-like chrome on the taskbar, Start, flyouts, and surfaces"),
            cfg.theme.transparency,
            Message::ToggleTransparency,
        ),
    ]
    .spacing(4);
    if cfg.theme.transparency {
        effects = effects
            .push(settings_row(
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
            ))
            .push(toggle_row(
                p,
                "preferences-desktop-wallpaper",
                "Dynamic transparency",
                Some("Clearer over wallpaper · more solid over windows"),
                cfg.theme.dynamic_transparency,
                Message::ToggleDynamicTransparency,
            ))
            .push({
                let blur = cfg.theme.blur_preset.to_lowercase();
                settings_row(
                    p,
                    "preferences-desktop-effect",
                    "Acrylic blur preset",
                    Some("Density presets (subtle → thick)"),
                    row![
                        choice(
                            p,
                            "Off",
                            blur == "off" || blur == "none",
                            Message::SetBlurPreset("off".into()),
                        ),
                        choice(
                            p,
                            "Subtle",
                            blur == "subtle",
                            Message::SetBlurPreset("subtle".into()),
                        ),
                        choice(
                            p,
                            "Acrylic",
                            blur == "acrylic" || blur.is_empty(),
                            Message::SetBlurPreset("acrylic".into()),
                        ),
                        choice(
                            p,
                            "Thick",
                            blur == "thick" || blur == "strong",
                            Message::SetBlurPreset("thick".into()),
                        ),
                    ]
                    .spacing(8)
                    .into(),
                )
            });
    }
    effects = effects
        .push(toggle_row(
            p,
            "color-select",
            "Show accent colour on Start and taskbar",
            Some("Selected Start and taskbar controls use the accent"),
            cfg.theme.accent_on_start_taskbar,
            Message::ToggleAccentOnStartTaskbar,
        ))
        .push(toggle_row(
            p,
            "window-new",
            "Show accent colour on title bars",
            Some("Server-side title bars pick up the accent wash"),
            cfg.theme.accent_on_titlebars,
            Message::ToggleAccentOnTitlebars,
        ));
    page_frame(
        p,
        "Colours",
        Some("Accent colour, transparency, colour theme"),
        column![
            appearance_card(p),
            status_row(p, "preferences-color", "Status", schedule),
            effects,
            section_label(p, "Accent"),
            accent_swatches(p, &cfg.theme.accent),
            settings_row(
                p,
                "color-select",
                "Custom accent",
                Some("Full #rrggbb is saved immediately"),
                text_input("#0078d4", &cfg.theme.accent)
                    .on_input(Message::SetAccent)
                    .width(140)
                    .into(),
            ),
            section_label(p, "Colour mode"),
            settings_row(
                p,
                "weather-clear",
                "Choose your mode",
                Some(&format!("Saved mode: {}", cfg.theme.mode)),
                row![
                    choice(p, "Light", !dark_mode, Message::SetMode("light".into())),
                    choice(p, "Dark", dark_mode, Message::SetMode("dark".into())),
                ]
                .spacing(8)
                .into(),
            ),
            toggle_row(
                p,
                "appointment-soon",
                "Dark schedule",
                Some("While mode is light, dark colours during these hours"),
                cfg.theme.dark_schedule,
                Message::ToggleDarkSchedule,
            ),
            settings_row(
                p,
                "weather-few-clouds",
                "Schedule mode",
                Some("Fixed hours or approximate sunrise/sunset"),
                row![
                    choice(
                        p,
                        "Hours",
                        !cfg.theme.dark_schedule_mode.eq_ignore_ascii_case("sunrise_sunset"),
                        Message::SetDarkScheduleMode("fixed".into()),
                    ),
                    choice(
                        p,
                        "Sunrise/sunset",
                        cfg.theme.dark_schedule_mode.eq_ignore_ascii_case("sunrise_sunset"),
                        Message::SetDarkScheduleMode("sunrise_sunset".into()),
                    ),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "preferences-system-time",
                "From / until",
                None::<String>,
                row![
                    text_input("20:00", &cfg.theme.dark_from)
                        .on_input(Message::SetDarkFrom)
                        .width(90),
                    text_input("07:00", &cfg.theme.dark_until)
                        .on_input(Message::SetDarkUntil)
                        .width(90),
                ]
                .spacing(8)
                .into(),
            ),
            section_label(p, "Title bars (SSD)"),
            settings_row(
                p,
                "view-grid",
                "Corner radius",
                Some("Server-side title bar top corners (px)"),
                row![
                    slider(0..=16, cfg.theme.ssd_corner_radius, Message::SetSsdRadius).width(180),
                    text(format!("{} px", cfg.theme.ssd_corner_radius)).size(13),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center)
                .into(),
            ),
            toggle_row(
                p,
                "preferences-desktop-effects",
                "Mica title bars",
                Some("Translucent SSD when transparency effects are on"),
                cfg.theme.ssd_mica,
                Message::ToggleSsdMica,
            ),
            section_label(p, "Style packs (A087)"),
            toggle_row(
                p,
                "applications-graphics",
                "Enable style packs",
                Some("Load TOML packs from ~/.local/share/mde/styles/"),
                cfg.theme.style_packs_enabled,
                Message::ToggleStylePacks,
            ),
            status_row(
                p,
                "folder",
                "Packs folder",
                &mde_config::style_packs::styles_root().display().to_string(),
            ),
            {
                let packs = mde_config::list_packs();
                let active = cfg.theme.style_pack.clone();
                let mut pack_row = row![choice(
                    p,
                    "Stock",
                    active.is_empty(),
                    Message::SetStylePack(String::new()),
                ),]
                .spacing(6);
                for pack in packs {
                    pack_row = pack_row.push(choice(
                        p,
                        pack.name.as_str(),
                        active == pack.id,
                        Message::SetStylePack(pack.id.clone()),
                    ));
                }
                settings_row(
                    p,
                    "preferences-desktop-theme",
                    "Active pack",
                    Some("Safe token overlays — accent, opacity, blur"),
                    pack_row.into(),
                )
            },
            quiet_btn(p, "Install sample pack", Message::EnsureSampleStylePack),
            link_btn(p, "Visual effects", Message::Navigate("visual".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn themes(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let packs = mde_config::list_packs();
    let mut pack_row = row![
        choice(
            p,
            "Stock",
            cfg.theme.style_pack.is_empty(),
            Message::SetStylePack(String::new()),
        )
    ]
    .spacing(8);
    for pack in &packs {
        let id = pack.id.clone();
        let label = if pack.name.is_empty() {
            pack.id.clone()
        } else {
            pack.name.clone()
        };
        pack_row = pack_row.push(choice(
            p,
            &label,
            cfg.theme.style_pack == id,
            Message::SetStylePack(id),
        ));
    }
    page_frame(
        p,
        "Themes",
        Some("Install, create, manage · style packs (A087)"),
        column![
            appearance_card(p),
            settings_row(
                p,
                "weather-clear",
                "Light",
                Some("Light chrome and surfaces"),
                accent_btn(p, "Apply", Message::SetMode("light".into())),
            ),
            settings_row(
                p,
                "weather-clear-night",
                "Dark",
                Some("Dark chrome and surfaces"),
                accent_btn(p, "Apply", Message::SetMode("dark".into())),
            ),
            settings_row(
                p,
                "preferences-desktop-accessibility",
                "High contrast black",
                Some("System-wide contrast pack"),
                accent_btn(p, "Apply", Message::SetMode("hc-black".into())),
            ),
            settings_row(
                p,
                "preferences-desktop-accessibility",
                "High contrast white",
                Some("System-wide contrast pack"),
                accent_btn(p, "Apply", Message::SetMode("hc-white".into())),
            ),
            status_row(p, "preferences-desktop-theme", "Active mode", &cfg.theme.mode),
            section_label(p, "Style packs"),
            toggle_row(
                p,
                "applications-graphics",
                "Enable style packs",
                Some("~/.local/share/mde/styles/*.toml"),
                cfg.theme.style_packs_enabled,
                Message::ToggleStylePacks,
            ),
            settings_row(
                p,
                "preferences-desktop-theme",
                "Active pack",
                Some("Official Windhawk-like overlays for chrome tokens"),
                pack_row.into(),
            ),
            quiet_btn(p, "Create sample pack", Message::EnsureSampleStylePack),
            link_btn(p, "Colours", Message::Navigate("colors".into())),
            link_btn(p, "Background", Message::Navigate("background".into())),
            link_btn(p, "Contrast themes", Message::Navigate("contrast".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn lockscreen(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    page_frame(
        p,
        "Lock screen",
        Some("Lock screen images and behaviour"),
        column![
            status_row(
                p,
                "system-lock-screen",
                "Idle lock",
                &format!("{} seconds", cfg.session.idle_lock_seconds),
            ),
            status_row(
                p,
                "system-users",
                "Switch user",
                "Single-user lock only · multi-seat switching out of scope",
            ),
            status_row(
                p,
                "preferences-desktop-wallpaper",
                "Background",
                if cfg.theme.wallpaper.trim().is_empty() {
                    "Desktop wallpaper"
                } else {
                    &cfg.theme.wallpaper
                },
            ),
            link_btn(p, "Background", Message::Navigate("background".into())),
            link_btn(p, "Power & battery", Message::Navigate("power".into())),
        ]
        .spacing(4)
        .into(),
    )
}

/// A043 — quiet default card set + per-card toggles + third-party drop-in API note.
pub fn widgets(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let w = &app.cfg.active().widgets;
    page_frame(
        p,
        "Widgets",
        Some("Board cards · quiet by default · Win+W"),
        column![
            iced::widget::text(
                "Quiet defaults hide News, Markets, and Traffic. Drop JSON cards in ~/.local/share/mde/widgets/ for third-party widgets."
            )
            .size(12),
            section_label(p, "Cards"),
            toggle_row(p, "weather-clear", "Weather", None::<String>, w.weather, |v| {
                Message::ToggleWidgetCard("weather".into(), v)
            }),
            toggle_row(p, "preferences-system-time", "Clock", None::<String>, w.clock, |v| {
                Message::ToggleWidgetCard("clock".into(), v)
            }),
            toggle_row(p, "x-office-calendar", "Calendar", None::<String>, w.calendar, |v| {
                Message::ToggleWidgetCard("calendar".into(), v)
            }),
            toggle_row(p, "utilities-system-monitor", "System", None::<String>, w.system, |v| {
                Message::ToggleWidgetCard("system".into(), v)
            }),
            toggle_row(p, "help-hint", "Tip", None::<String>, w.tip, |v| {
                Message::ToggleWidgetCard("tip".into(), v)
            }),
            toggle_row(p, "folder-pictures", "Photos", None::<String>, w.photos, |v| {
                Message::ToggleWidgetCard("photos".into(), v)
            }),
            toggle_row(
                p,
                "preferences-desktop-wallpaper",
                "Background",
                None::<String>,
                w.background,
                |v| Message::ToggleWidgetCard("background".into(), v),
            ),
            toggle_row(p, "application-rss+xml", "News", Some("Off in quiet default"), w.news, |v| {
                Message::ToggleWidgetCard("news".into(), v)
            }),
            toggle_row(p, "office-chart-line", "Markets", Some("Off in quiet default"), w.markets, |v| {
                Message::ToggleWidgetCard("markets".into(), v)
            }),
            toggle_row(p, "car", "Traffic", Some("Off in quiet default"), w.traffic, |v| {
                Message::ToggleWidgetCard("traffic".into(), v)
            }),
            toggle_row(
                p,
                "application-x-addon",
                "Third-party cards",
                Some("~/.local/share/mde/widgets/*.json"),
                w.third_party,
                |v| Message::ToggleWidgetCard("third_party".into(), v),
            ),
            status_row(
                p,
                "view-grid",
                "Taskbar button",
                "Off by default · toggle under Taskbar",
            ),
            link_btn(p, "Taskbar", Message::Navigate("taskbar".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn fonts(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Fonts",
        Some("Install, manage"),
        column![
            status_row(p, "preferences-desktop-font", "UI font", mde_theme::FONT),
            status_row(p, "preferences-desktop-font", "Mono", mde_theme::FONT_MONO),
            iced::widget::text("The quick brown fox jumps over the lazy dog.").size(16),
            iced::widget::text("Install fonts under ~/.local/share/fonts or /usr/share/fonts.")
                .size(12),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn window_layout(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let decorations = if cfg.compositor.decorations.eq_ignore_ascii_case("server")
        || cfg.compositor.decorations.trim().is_empty()
    {
        "MDE standard · server-side title bars"
    } else {
        cfg.compositor.decorations.as_str()
    };
    page_frame(
        p,
        "Window Layout",
        Some("Arrange windows, title bars, gaps, desktops"),
        column![
            section_label(p, "Title bars"),
            status_row(p, "window-new", "Title bars", decorations),
            iced::widget::text(
                "All normal windows use the MDE server-side decoration (SSD). Client title bars are not used."
            )
            .size(13),
            toggle_row(
                p,
                "color-select",
                "Show accent colour on title bars",
                Some("Same setting as Personalization → Colours"),
                cfg.theme.accent_on_titlebars,
                Message::ToggleAccentOnTitlebars,
            ),
            toggle_row(
                p,
                "input-mouse",
                "AltSnap · Alt+drag move",
                Some("Alt+left-drag move · Alt+right-drag resize from anywhere"),
                cfg.compositor.alt_snap,
                Message::ToggleAltSnap,
            ),
            section_label(p, "Virtual desktops"),
            {
                let count = 4usize;
                let mut rows = column![].spacing(4);
                for i in 0..count {
                    let name = cfg.desktops.names.get(i).cloned().unwrap_or_default();
                    let wall = cfg.desktops.wallpapers.get(i).cloned().unwrap_or_default();
                    rows = rows
                        .push(settings_row(
                            p,
                            "view-paged",
                            &format!("Desktop {}", i + 1),
                            Some("Name + optional wallpaper path"),
                            column![
                                text_input("Work", &name)
                                    .on_input(move |s| Message::SetDesktopName(i, s))
                                    .width(220),
                                text_input("/path/wall.jpg", &wall)
                                    .on_input(move |s| Message::SetDesktopWallpaper(i, s))
                                    .width(320),
                            ]
                            .spacing(6)
                            .into(),
                        ));
                }
                rows
            },
            section_label(p, "Layout"),
            status_row(p, "view-grid", "Compositor layout", &cfg.compositor.layout),
            status_row(
                p,
                "view-paged",
                "Task View",
                "Title and icon tiles · live window thumbnails out of scope",
            ),
            iced::widget::text("Snap layouts and multitasking are handled by mde-shell / mde-anvil.")
                .size(13),
            section_label(p, "Always on top"),
            toggle_row(
                p,
                "go-top",
                "Win+Ctrl+T shortcut",
                Some("Toggle always on top for the focused window"),
                cfg.tools.always_on_top_shortcut,
                Message::ToggleAlwaysOnTopShortcut,
            ),
            toggle_row(
                p,
                "emblem-favorite",
                "Title-bar pin indicator",
                Some("Show a pin glyph when always on top"),
                cfg.tools.always_on_top_indicator,
                Message::ToggleAlwaysOnTopIndicator,
            ),
            link_btn(p, "FancyZones & Snap Groups", Message::Navigate("tools".into())),
            link_btn(p, "Colours", Message::Navigate("colors".into())),
            link_btn(p, "Config Editor", Message::Navigate("config".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn start(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let layout = cfg.start.layout.to_lowercase().replace(' ', "_").replace('-', "_");
    let classic = layout == "classic" || layout == "vertical";
    let more_pins = layout == "more_pins" || layout == "morepins" || layout.is_empty();
    let pin_preview = if classic {
        "Live preview · Classic vertical · All apps left · pins right"
    } else if more_pins {
        "Live preview · More pins · up to 36 pinned tiles · taller Start"
    } else {
        "Live preview · Default · up to 18 pinned tiles"
    };
    let folder_toggle = |id: &'static str, label: &str| {
        let on = cfg
            .start
            .folders
            .iter()
            .any(|f| f.eq_ignore_ascii_case(id));
        let id = id.to_string();
        toggle_row(
            p,
            "folder",
            label,
            None::<String>,
            on,
            move |v| Message::ToggleStartFolder(id.clone(), v),
        )
    };
    page_frame(
        p,
        "Start",
        Some("Pinned apps, layout, and folders beside power"),
        column![
            settings_row(
                p,
                "start-here",
                "Layout",
                Some("Community primary: More pins · Classic = StartAllBack vertical"),
                row![
                    choice(
                        p,
                        "More pins",
                        more_pins && !classic,
                        Message::SetStartLayout("more_pins".into()),
                    ),
                    choice(
                        p,
                        "Default",
                        !more_pins && !classic,
                        Message::SetStartLayout("default".into()),
                    ),
                    choice(
                        p,
                        "Classic",
                        classic,
                        Message::SetStartLayout("classic".into()),
                    ),
                ]
                .spacing(8)
                .into(),
            ),
            status_row(p, "view-grid-symbolic", "Density", pin_preview),
            settings_row(
                p,
                "view-fullscreen",
                "Default pin size",
                Some("Small · Medium · Large (per-pin Large via right-click still works)"),
                {
                    let size = app
                        .cfg
                        .extras
                        .get("start.pin_default_size")
                        .and_then(|v| v.as_str())
                        .unwrap_or("medium")
                        .to_lowercase();
                    row![
                        choice(
                            p,
                            "Small",
                            size == "small",
                            Message::SetStartPinDefaultSize("small".into()),
                        ),
                        choice(
                            p,
                            "Medium",
                            size == "medium" || size.is_empty(),
                            Message::SetStartPinDefaultSize("medium".into()),
                        ),
                        choice(
                            p,
                            "Large",
                            size == "large",
                            Message::SetStartPinDefaultSize("large".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            status_row(
                p,
                "folder",
                "Pin folders",
                &format!(
                    "{} folders in extras · create via Start drag / right-click",
                    app.cfg
                        .extras
                        .get("start.pin_folders")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0)
                ),
            ),
            status_row(
                p,
                "security-high",
                "No promo policy",
                "Start never shows Recommended, Store ads, or account marketing",
            ),
            toggle_row(
                p,
                "help-about",
                "Show Tips",
                Some("Opt-in only · off by default (community quiet Start)"),
                cfg.start.tips_opt_in,
                Message::ToggleTipsOptIn,
            ),
            toggle_row(
                p,
                "view-list",
                "Show recently added apps",
                None::<String>,
                cfg.start.recently_added,
                Message::ToggleRecentlyAdded,
            ),
            toggle_row(
                p,
                "audio-input-microphone",
                "Voice search (network STT)",
                Some("Super+H dictation · set extras start.voice_stt_url"),
                app.cfg.extras.get("start.voice_search").and_then(|v| v.as_bool()).unwrap_or(false),
                Message::ToggleVoiceSearch,
            ),
            settings_row(
                p,
                "network-transmit-receive",
                "Network STT endpoint",
                Some("HTTP multipart upload for Super+H / voice search"),
                {
                    let url = app
                        .cfg
                        .extras
                        .get("start.voice_stt_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    text_input("https://…/stt", url)
                        .on_input(Message::SetVoiceSttUrl)
                        .width(280)
                        .into()
                },
            ),
            section_label(p, "Folders"),
            status_row(
                p,
                "folder",
                "Folders next to power",
                "Win11 checklist · toggles appear beside the power button on Start",
            ),
            folder_toggle("settings", "Settings"),
            folder_toggle("documents", "Documents"),
            folder_toggle("downloads", "Downloads"),
            folder_toggle("music", "Music"),
            folder_toggle("pictures", "Pictures"),
            folder_toggle("videos", "Videos"),
            folder_toggle("home", "Personal folder"),
            folder_toggle("desktop", "Desktop"),
            folder_toggle("network", "Network"),
            folder_toggle("explorer", "File Explorer"),
            status_row(
                p,
                "system-file-manager",
                "File manager",
                "Thunar is MDE Explorer · classic menus / Peek wait on W8 track",
            ),
            status_row(
                p,
                "view-paged",
                "Bulk rename",
                "Use Thunar’s built-in Bulk Rename (Edit → Bulk Rename)",
            ),
            status_row(
                p,
                "system-search",
                "Start search",
                "Local only · content search: Desktop, Documents, Downloads",
            ),
            status_row(
                p,
                "view-pinned",
                "Pinned apps",
                &format!("{} pins", cfg.start.pins.len()),
            ),
            link_btn(p, "Installed apps", Message::Navigate("installed".into())),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn taskbar(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let cfg = app.cfg.active();
    let align = cfg.shell.taskbar_align.to_lowercase();
    page_frame(
        p,
        "Taskbar",
        Some("Taskbar behaviours, system tray icons"),
        column![
            section_label(p, "Taskbar behaviours"),
            settings_row(
                p,
                "preferences-desktop",
                "Taskbar alignment",
                None::<String>,
                row![
                    choice(p, "Left", align == "left", Message::SetAlign("left".into())),
                    choice(
                        p,
                        "Center",
                        align == "center",
                        Message::SetAlign("center".into()),
                    ),
                ]
                .spacing(8)
                .into(),
            ),
            settings_row(
                p,
                "view-list",
                "Combine taskbar buttons",
                Some("Always · When full · Never (ExplorerPatcher-style)"),
                {
                    let combine = cfg.shell.taskbar_combine.to_lowercase();
                    row![
                        choice(
                            p,
                            "Always",
                            combine == "always" || combine.is_empty(),
                            Message::SetCombine("always".into()),
                        ),
                        choice(
                            p,
                            "When full",
                            combine == "whenfull" || combine == "when_full" || combine == "full",
                            Message::SetCombine("whenfull".into()),
                        ),
                        choice(
                            p,
                            "Never",
                            combine == "never",
                            Message::SetCombine("never".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            toggle_row(
                p,
                "format-text-symbolic",
                "Show labels",
                Some("App names beside icons"),
                cfg.shell.taskbar_labels,
                Message::ToggleTaskbarLabels,
            ),
            toggle_row(
                p,
                "mark-location",
                "Show badges",
                Some("Unread counts from app notifications"),
                cfg.shell.taskbar_badges,
                Message::ToggleTaskbarBadges,
            ),
            settings_row(
                p,
                "zoom-fit-best",
                "Taskbar size",
                Some("Compact (36px) or Comfortable (48px)"),
                {
                    let size = cfg.shell.taskbar_size.to_lowercase();
                    row![
                        choice(
                            p,
                            "Compact",
                            size == "compact",
                            Message::SetTaskbarSize("compact".into()),
                        ),
                        choice(
                            p,
                            "Comfortable",
                            size != "compact",
                            Message::SetTaskbarSize("comfortable".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            status_row(
                p,
                "go-bottom",
                "Taskbar position",
                "Bottom only (community top-edge is out of scope)",
            ),
            status_row(
                p,
                "image-x-generic",
                "Thumbnail peek",
                "Title-only hover · live thumbnails intentionally deferred",
            ),
            status_row(
                p,
                "user-desktop",
                "Show desktop",
                "Click the far-right strip (or Super+D) · no peek-on-hover",
            ),
            toggle_row(
                p,
                "view-conceal",
                "Automatically hide the taskbar",
                None::<String>,
                cfg.shell.taskbar_autohide,
                Message::ToggleTaskbarAutohide,
            ),
            toggle_row(
                p,
                "process-stop",
                "Always show End task",
                Some("Win11-style · when off, End task still appears for running windows"),
                cfg.shell.end_task_always,
                Message::ToggleAlwaysEndTask,
            ),
            toggle_row(
                p,
                "view-grid",
                "Show Widgets button",
                Some("Off by default · Win+W still opens Widgets"),
                cfg.shell.widgets_btn,
                Message::ToggleWidgetsBtn,
            ),
            section_label(p, "Multiple displays"),
            settings_row(
                p,
                "video-display",
                "Show my taskbar on",
                Some("Win11-like multi-monitor modes (A096)"),
                {
                    let mode = cfg.shell.taskbar_multi_mode.to_lowercase();
                    row![
                        choice(
                            p,
                            "All taskbars",
                            mode == "all" || mode.is_empty(),
                            Message::SetTaskbarMultiMode("all".into()),
                        ),
                        choice(
                            p,
                            "Main + buttons where open",
                            mode == "main_buttons",
                            Message::SetTaskbarMultiMode("main_buttons".into()),
                        ),
                        choice(
                            p,
                            "Main only",
                            mode == "main_only",
                            Message::SetTaskbarMultiMode("main_only".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            section_label(p, "For developers"),
            toggle_row(
                p,
                "utilities-system-monitor",
                "Always show End task",
                Some("Jump lists always offer End task (A082)"),
                cfg.shell.end_task_always,
                Message::ToggleAlwaysEndTask,
            ),
            link_btn(p, "Widgets board", Message::Navigate("widgets".into())),
            link_btn(
                p,
                "Other system tray icons",
                Message::Navigate("trayicons".into()),
            ),
            link_btn(
                p,
                "Productivity tools",
                Message::Navigate("tools".into()),
            ),
            settings_row(
                p,
                "system-search",
                "Search mode",
                Some("Hide · icon · or search box on the taskbar"),
                {
                    let mode = cfg.shell.taskbar_search.to_lowercase();
                    row![
                        choice(
                            p,
                            "Hide",
                            mode == "hide" || mode.is_empty(),
                            Message::SetTaskbarSearch("hide".into()),
                        ),
                        choice(
                            p,
                            "Icon",
                            mode == "icon",
                            Message::SetTaskbarSearch("icon".into()),
                        ),
                        choice(
                            p,
                            "Search box",
                            mode == "box" || mode == "search" || mode == "bar",
                            Message::SetTaskbarSearch("box".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
            section_label(p, "Taskbar clock"),
            toggle_row(
                p,
                "preferences-system-time",
                "Show seconds on clock",
                Some("Also available under Time & language"),
                cfg.shell.clock_seconds,
                Message::ToggleClockSeconds,
            ),
            toggle_row(
                p,
                "x-office-calendar",
                "ISO date on clock",
                Some("YYYY-MM-DD instead of weekday M/D"),
                cfg.shell.clock_date_iso,
                Message::ToggleClockDateIso,
            ),
            section_label(p, "Multiple displays"),
            settings_row(
                p,
                "video-display",
                "Taskbar on all displays",
                Some("all · main taskbar + pinned buttons only · main display only"),
                {
                    let mode = cfg.shell.taskbar_multi_mode.to_lowercase();
                    row![
                        choice(
                            p,
                            "All taskbars",
                            mode == "all" || mode.is_empty(),
                            Message::SetTaskbarMultiMode("all".into()),
                        ),
                        choice(
                            p,
                            "Main + pins",
                            mode == "main_buttons",
                            Message::SetTaskbarMultiMode("main_buttons".into()),
                        ),
                        choice(
                            p,
                            "Main only",
                            mode == "main_only",
                            Message::SetTaskbarMultiMode("main_only".into()),
                        ),
                    ]
                    .spacing(8)
                    .into()
                },
            ),
        ]
        .spacing(4)
        .into(),
    )
}
