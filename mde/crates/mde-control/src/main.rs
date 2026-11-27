//! mde-control — Win11-pattern Settings over ~/.config/mde/config.toml.
mod apps;
mod bing;
mod community;
mod nav;
mod pages;
mod system;
mod kdeconnect;
mod widgets;

use iced::widget::{button, column, container, image, row, scrollable, text, text_input, Space};
use iced::widget::container::Style as ContainerStyle;
use iced::{Background, Border, Color, Element, Fill, Padding, Task, Theme};
use mde_config::MdeConfig;
use mde_theme::Palette;

use crate::widgets::nav_btn;

fn main() -> iced::Result {
    mde_config::supervisor::install_panic_hook();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--bing-wallpaper") {
        std::process::exit(bing::run_cli());
    }
    if args.first().map(|a| a.as_str()) == Some("--resize-image") {
        let path = args.get(1).map(|s| s.as_str()).unwrap_or("");
        let preset = args.get(2).map(|s| s.as_str()).unwrap_or("1024w");
        let code = match system::resize_image(path, preset) {
            Ok(p) => {
                eprintln!("{p}");
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
        std::process::exit(code);
    }

    let page = args.first().cloned().unwrap_or_default();
    let boot_cfg = mde_config::MdeConfig::load().unwrap_or_default();
    let ui = mde_theme::ui_font(&boot_cfg.active().theme).to_string();
    let mono = mde_theme::mono_font(&boot_cfg.active().theme).to_string();
    let mut app = iced::application("Settings", Control::update, Control::view)
        .theme(Control::theme)
        .default_font(iced::Font::with_name(Box::leak(ui.clone().into_boxed_str())))
        .window(iced::window::Settings {
            size: iced::Size::new(1100.0, 720.0),
            // Negotiate decorations; compositor forces SSD (MDE standard).
            decorations: true,
            ..Default::default()
        });
    for bytes in mde_theme::font_bytes_selected(&ui, &mono) {
        app = app.font(bytes);
    }
    app.run_with(move || Control::new(page))
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(String),
    GoBack,
    SearchChanged(String),
    EditorSearch(String),
    ToggleAdvanced(bool),
    Save,
    Reload,
    Export,
    Import,
    ProfileName(String),
    CreateProfile,
    SelectProfile(String),
    SetAccent(String),
    LookSample,
    /// Personalization → Fonts: UI chrome typeface.
    SetUiFont(String),
    /// Personalization → Fonts: monospace typeface.
    SetMonoFont(String),
    FontsQuery(String),
    SetMode(String),
    SetWallpaper(String),
    ApplyWallpaper,
    DownloadBingWallpaper,
    ToggleBingDaily(bool),
    ToggleDarkSchedule(bool),
    SetDarkFrom(String),
    SetDarkUntil(String),
    SetAlign(String),
    /// A001: one-click Home/first-run Classic left taskbar preset.
    ApplyClassicLeftPreset,
    /// A001: dismiss the first-run Classic left prompt without applying.
    DismissClassicLeftPrompt,
    /// A002: taskbar Search mode — hide | icon | box.
    SetTaskbarSearch(String),
    SetCombine(String),
    ToggleTaskbarLabels(bool),
    ToggleTaskbarBadges(bool),
    SetTaskbarSize(String),
    ToggleXwayland(bool),
    ToggleNightLight(bool),
    ToggleDnd(bool),
    TogglePriority(bool),
    SetQuietFrom(String),
    SetQuietTo(String),
    ToggleClip(bool),
    ToggleCrossDevice(bool),
    ToggleClipClearOnLock(bool),
    SetClipSyncDir(String),
    ToggleWifi(bool),
    ToggleBt(bool),
    ToggleAirplane(bool),
    ApplyDisplayProfile(String),
    RefreshOutputs,
    OutputX(String, String),
    OutputY(String, String),
    OutputScale(String, f64),
    SaveLayout,
    ToggleFractional(bool),
    ExportBuf(String),
    VolUp,
    VolDown,
    VolMute,
    AppsQuery(String),
    LaunchApp(String),
    PinTaskbar(String),
    PinStart(String),
    WifiConnect(String),
    BtConnect(String),
    SetSink(u32),
    SetSource(u32),
    NodeVol(u32, i32),
    NodeMute(u32),
    RefreshMmc,
    /// W458: launch standalone mde-mmc console.
    LaunchMmc,
    LaunchMmcComp,
    LaunchMmcPerf,
    /// Check/uncheck a Start folder name on Advanced (moves off/on Start).
    ToggleAdvancedGroup(String, bool),
    /// Restore an individual Advanced app back onto Start.
    RestoreAdvancedApp(String),
    SetIdleLockEdit(String),
    ApplyIdleLock,
    ToggleRecentlyAdded(bool),
    ToggleTipsOptIn(bool),
    ToggleVoiceSearch(bool),
    SetVoiceSttUrl(String),
    ToggleWidgetCard(String, bool),
    SetStartLayout(String),
    /// A014: default Start pin tile size — small | medium | large.
    SetStartPinDefaultSize(String),
    ToggleStartFolder(String, bool),
    ToggleTaskbarAutohide(bool),
    ToggleWidgetsBtn(bool),
    ToggleNtp(bool),
    ToggleClock24h(bool),
    /// W391: taskbar clock seconds (Personalization → Taskbar).
    ToggleClockSeconds(bool),
    /// W391: taskbar ISO date format.
    ToggleClockDateIso(bool),
    RefreshTime,
    ToggleGameMode(bool),
    /// A045: Thunar / GTK file chooser — show file extensions.
    ToggleShowExtensions(bool),
    /// A045: show hidden files in file manager.
    ToggleShowHiddenFiles(bool),
    /// A082: always show End task on taskbar jump lists.
    ToggleAlwaysEndTask(bool),
    OpenScreenshotDir,
    SetScreenshotDirEdit(String),
    ApplyScreenshotDir,
    PickScreenshotDir,
    SetTextScaleEdit(String),
    ApplyTextScale,
    ApplyTextScaleValue(String),
    ToggleAnimations(bool),
    ToggleTransparency(bool),
    SetOpacity(u8),
    ToggleDynamicTransparency(bool),
    SetBlurPreset(String),
    ToggleAccentOnStartTaskbar(bool),
    ToggleAccentOnTitlebars(bool),
    ToggleAlwaysShowScrollbars(bool),
    SetCursorSize(i32),
    CheckUpdates,
    RefreshUpdateHistory,
    UpdateInstallAll,
    UpdateRestartNow,
    UpdateAndShutDown,
    UpdateScheduleRestart,
    UpdateCancelOffline,
    SetPowerProfile(String),
    SetIdleDpmsEdit(String),
    ApplyIdleDpms,
    StartFocusSession(u64),
    EndFocusSession,
    /// A033 — quiet hours preset `from|until` or empty to clear both.
    ApplyQuietPreset(String),
    ToggleMouseNatural(bool),
    ToggleTouchpadTap(bool),
    ToggleTouchpadNatural(bool),
    HostnameEdit(String),
    ApplyHostname,
    CopyAboutSpecs,
    RefreshNearby,
    NearbyPair(String),
    NearbyUnpair(String),
    // A015–A042 productivity tools
    ToggleCommandPalette(bool),
    ToggleFancyZones(bool),
    SetActiveZoneLayout(String),
    ToggleSnapGroups(bool),
    ToggleAlwaysOnTopShortcut(bool),
    ToggleAlwaysOnTopIndicator(bool),
    ToggleColorPickerHistory(bool),
    ToggleFindMyMouse(bool),
    ToggleMouseHighlighter(bool),
    SetAwakeMinutesEdit(String),
    ApplyAwakeMinutes,
    ToggleAdvancedPaste(bool),
    SetJumpListMax(u32),
    SetWorkspaceNameEdit(String),
    SaveWorkspacePreset,
    LaunchWorkspace(String),
    DeleteWorkspace(String),
    SetRemapFromEdit(String),
    SetRemapToEdit(String),
    AddKeyRemap,
    RemoveKeyRemap(usize),
    TrayShowIcon(String),
    TrayHideIcon(String),
    SetAppNotifBanners(String, bool),
    SetAppNotifSound(String, bool),
    SetAppNotifPriority(String, bool),
    // A070–A098 community audit
    ToggleMagnifierNative(bool),
    SetMagnifierFactor(f64),
    SetMagnifierMode(String),
    ToggleMagnifierFollowFocus(bool),
    ToggleShortcutGuide(bool),
    ToggleImageResizer(bool),
    InstallImageResizerUca,
    SetStylePack(String),
    ToggleStylePacks(bool),
    EnsureSampleStylePack,
    SetTaskbarMultiMode(String),
    SetWorldClock(usize, String),
    ToggleStorageSense(bool),
    ToggleStorageSenseTemp(bool),
    ToggleStorageSenseTrash(bool),
    SetStorageSenseDays(u32),
    RunStorageSense,
    TogglePrivacyIndicators(bool),
    ToggleAutoOsk(bool),
    SetTabletMode(String),
    SetGesture(String, String),
    RefreshPrivacyDashboard,
    ResizeImagePick(String),
    // A045–A059 community audit (display/apps A060+ may share helpers)
    SetDarkScheduleMode(String),
    ToggleSpotlight(bool),
    ToggleSpotlightLock(bool),
    SetNightStrength(u8),
    SetNightLightStrength(u8),
    ToggleNightSchedule(bool),
    ToggleNightLightSchedule(bool),
    SetNightFrom(String),
    SetNightUntil(String),
    ToggleAltSnap(bool),
    SetSsdRadius(u8),
    SetSsdCornerRadius(u8),
    ToggleSsdMica(bool),
    IdentifyDisplays,
    /// A060/A062: revert display layout positions from saved profile.
    RevertLayout,
    /// A062: apply per-output refresh/HDR/scale tuning.
    ApplyOutputTuning,
    ProjectMode(String),
    NudgeOutput(String, i32, i32),
    OutputRefresh(String, f64),
    ToggleOutputHdr(String, bool),
    StartupQuery(String),
    ToggleStartup(String, bool),
    SetMimeDefault(String, String),
    UninstallApp(String),
    RepairApp(String),
    PickAccountPicture,
    SetDesktopName(usize, String),
    SetDesktopWallpaper(usize, String),
    ApplyDesktopWallpaper(usize),
    ToggleGameModeDnd(bool),
}

#[derive(Debug, Clone)]
pub struct AppNotifPrefRow {
    pub app: String,
    pub banners: bool,
    pub sound: bool,
    pub priority: bool,
}



pub struct Control {
    pub cfg: MdeConfig,
    pub palette: Palette,
    pub page: String,
    pub history: Vec<String>,
    pub search: String,
    pub editor_search: String,
    pub advanced: bool,
    pub status: String,
    pub new_profile: String,
    pub export_buf: String,
    pub apps_query: String,
    pub apps: Vec<apps::DesktopApp>,
    pub wifi_list: Vec<String>,
    pub bt_list: Vec<String>,
    pub sinks: Vec<system::AudioSink>,
    pub sources: Vec<system::AudioSink>,
    pub streams: Vec<system::AudioSink>,
    pub disks: String,
    pub services: String,
    pub outputs: String,
    pub layout_heads: Vec<system::LayoutHead>,
    pub idle_lock_edit: String,
    pub idle_dpms_edit: String,
    pub screenshot_dir_edit: String,
    pub text_scale_edit: String,
    pub update_summary: String,
    pub update_history: String,
    pub hostname_edit: String,
    pub nearby_devices: Vec<kdeconnect::Device>,
    pub nearby_hint: String,
    pub awake_minutes_edit: String,
    pub workspace_name_edit: String,
    pub remap_from_edit: String,
    pub remap_to_edit: String,
    pub tray_known: Vec<String>,
    pub notif_app_prefs: Vec<AppNotifPrefRow>,
    pub notif_app_focus: String,
    /// A063: startup apps search filter.
    pub startup_query: String,
    /// Personalization → Fonts search filter.
    pub fonts_query: String,
}

impl Control {
    fn new(page: String) -> (Self, Task<Message>) {
        let mut cfg = MdeConfig::load().unwrap_or_default();
        // A045: first-run / reload push FM defaults from typed theme.
        if !cfg.extras.get("theme.fm_defaults_applied").and_then(|v| v.as_bool()).unwrap_or(false) {
            community::apply_fm_defaults(&cfg);
            cfg.extras.insert(
                "theme.fm_defaults_applied".into(),
                toml::Value::Boolean(true),
            );
            let _ = cfg.save();
        }
        let layout_heads = system::layout_heads(&cfg);
        let audio = system::audio_status();
        let palette = Palette::from_theme(&cfg.active().theme);
        let (page, notif_app_focus) = {
            let raw = page;
            if let Some((base, app)) = raw.split_once(':') {
                if nav::resolve(base) == "notifapps" || base == "notifapps" {
                    (nav::resolve("notifapps").to_string(), app.to_string())
                } else {
                    (nav::resolve(&raw).to_string(), String::new())
                }
            } else {
                (nav::resolve(&raw).to_string(), String::new())
            }
        };
        let idle_lock_edit = cfg.active().session.idle_lock_seconds.to_string();
        let idle_dpms_edit = cfg.active().session.idle_dpms_seconds.to_string();
        let screenshot_dir_edit = {
            let custom = cfg.active().shell.screenshot_dir.trim();
            if custom.is_empty() {
                system::default_screenshot_dir()
            } else {
                custom.to_string()
            }
        };
        let text_scale = system::gsettings_get("org.gnome.desktop.interface", "text-scaling-factor");
        let awake_minutes_edit = cfg.active().tools.awake_minutes.to_string();
        (
            Self {
                cfg,
                palette,
                page,
                history: Vec::new(),
                search: String::new(),
                editor_search: String::new(),
                advanced: false,
                status: format!("Loaded {}", mde_theme::about_line()),
                new_profile: String::new(),
                export_buf: String::new(),
                apps_query: String::new(),
                apps: apps::list_apps(400),
                wifi_list: system::wifi_networks(),
                bt_list: system::bt_devices(),
                sinks: audio.sinks,
                sources: audio.sources,
                streams: audio.streams,
                disks: system::disks(),
                services: system::services(),
                outputs: system::outputs_summary(),
                layout_heads,
                idle_lock_edit,
                idle_dpms_edit,
                screenshot_dir_edit,
                text_scale_edit: if text_scale.is_empty() || text_scale.contains("failed") {
                    "1.00".into()
                } else {
                    text_scale
                },
                update_summary: "Press Check for updates.".into(),
                update_history: String::new(),
                hostname_edit: system::pretty_hostname(),
                nearby_devices: Vec::new(),
                nearby_hint: String::new(),
                awake_minutes_edit,
                workspace_name_edit: String::new(),
                remap_from_edit: String::new(),
                remap_to_edit: String::new(),
                tray_known: system::tray_known_ids(),
                notif_app_prefs: system::load_app_notif_pref_rows(),
                notif_app_focus,
                startup_query: String::new(),
                fonts_query: String::new(),
            },
            Task::none(),
        )
    }

    fn reload_audio(&mut self) {
        let audio = system::audio_status();
        self.sinks = audio.sinks;
        self.sources = audio.sources;
        self.streams = audio.streams;
    }

    fn refresh_for_page(&mut self) {
        let cat = nav::category_of(&self.page);
        match self.page.as_str() {
            "network" | "wifi" => self.wifi_list = system::wifi_networks(),
            "nearby" => {
                self.nearby_devices = if kdeconnect::installed() {
                    kdeconnect::list_devices()
                } else {
                    Vec::new()
                };
                self.nearby_hint = if kdeconnect::installed() {
                    format!("{} device(s)", self.nearby_devices.len())
                } else {
                    "Install kde-connect · docs/kde-connect.md".into()
                };
            }
            "bluetooth" | "devices" => self.bt_list = system::bt_devices(),
            "sound" | "mixer" | "microphone" => self.reload_audio(),
            "mmc" | "storage" | "advanced" => {
                self.disks = system::disks();
                self.services = system::services();
            }
            "display" | "nightlight" | "system" => {
                self.outputs = system::outputs_summary();
                self.layout_heads = system::layout_heads(&self.cfg);
            }
            "installed" | "apps" => self.apps = apps::list_apps(400),
            "updatehistory" => self.update_history = system::update_history(),
            _ => {
                if cat == "network" {
                    self.wifi_list = system::wifi_networks();
                }
            }
        }
    }

    fn go(&mut self, key: &str, push: bool) {
        let resolved = nav::resolve(key).to_string();
        if push && self.page != resolved {
            self.history.push(self.page.clone());
        }
        self.page = resolved;
        self.refresh_for_page();
    }

    fn theme(&self) -> Theme {
        mde_theme::iced_theme(&self.palette)
    }

    fn pin_id(&mut self, id: &str, start: bool) {
        let pins = if start {
            &mut self.cfg.active_mut().start.pins
        } else {
            &mut self.cfg.active_mut().shell.pins
        };
        if !pins.iter().any(|p| p == id) {
            pins.push(id.to_string());
        }
        self.status = match self.cfg.save() {
            Ok(()) => format!("Pinned {id}"),
            Err(e) => format!("Pin save failed: {e}"),
        };
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(key) => self.go(&key, true),
            Message::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.page = prev;
                    self.refresh_for_page();
                }
            }
            Message::SearchChanged(s) => {
                self.search = s;
                if !self.search.trim().is_empty() && self.page != "home" {
                    // Keep browsing; search results also appear when on Home.
                }
            }
            Message::EditorSearch(s) => self.editor_search = s,
            Message::ToggleAdvanced(v) => self.advanced = v,
            Message::Save => match self.cfg.save() {
                Ok(()) => {
                    self.palette = Palette::from_theme(&self.cfg.active().theme);
                    self.status = "Saved ~/.config/mde/config.toml".into();
                }
                Err(e) => self.status = format!("Save failed: {e}"),
            },
            Message::Reload => match MdeConfig::load() {
                Ok(c) => {
                    self.cfg = c;
                    self.palette = Palette::from_theme(&self.cfg.active().theme);
                    self.idle_lock_edit = self.cfg.active().session.idle_lock_seconds.to_string();
                    self.idle_dpms_edit = self.cfg.active().session.idle_dpms_seconds.to_string();
                    let custom = self.cfg.active().shell.screenshot_dir.trim();
                    self.screenshot_dir_edit = if custom.is_empty() {
                        system::default_screenshot_dir()
                    } else {
                        custom.to_string()
                    };
                    self.status = "Reloaded".into();
                }
                Err(e) => self.status = format!("Reload failed: {e}"),
            },
            Message::Export => match self.cfg.export_toml() {
                Ok(t) => {
                    self.export_buf = t;
                    self.status = "Exported to buffer".into();
                    self.go("config", true);
                    self.advanced = true;
                }
                Err(e) => self.status = format!("Export failed: {e}"),
            },
            Message::Import => match MdeConfig::import_toml(&self.export_buf) {
                Ok(c) => {
                    self.cfg = c;
                    self.status = "Imported TOML — press Save to persist".into();
                }
                Err(e) => self.status = format!("Import failed: {e}"),
            },
            Message::ExportBuf(s) => self.export_buf = s,
            Message::VolUp => match system::volume_adjust(5) {
                Ok(s) => self.status = s,
                Err(e) => self.status = e,
            },
            Message::VolDown => match system::volume_adjust(-5) {
                Ok(s) => self.status = s,
                Err(e) => self.status = e,
            },
            Message::VolMute => match system::volume_toggle_mute() {
                Ok(s) => self.status = s,
                Err(e) => self.status = e,
            },
            Message::AppsQuery(s) => self.apps_query = s,
            Message::LaunchApp(id) => match apps::launch(&id) {
                Ok(()) => self.status = format!("Launched {id}"),
                Err(e) => self.status = e,
            },
            Message::PinTaskbar(id) => self.pin_id(&id, false),
            Message::PinStart(id) => self.pin_id(&id, true),
            Message::WifiConnect(ssid) => match system::wifi_connect(&ssid) {
                Ok(()) => self.status = format!("Connecting {ssid}"),
                Err(e) => self.status = e,
            },
            Message::SetSink(id) => {
                self.status = match system::set_default_sink(id) {
                    Ok(()) => {
                        self.reload_audio();
                        format!("Default sink {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::SetSource(id) => {
                self.status = match system::set_default_source(id) {
                    Ok(()) => {
                        self.reload_audio();
                        format!("Default source {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::NodeVol(id, delta) => {
                self.status = match system::node_volume(id, delta) {
                    Ok(()) => {
                        self.reload_audio();
                        format!("Volume {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::NodeMute(id) => {
                self.status = match system::node_mute(id) {
                    Ok(()) => {
                        self.reload_audio();
                        format!("Mute toggled {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::BtConnect(line) => match system::bt_connect(&line) {
                Ok(()) => self.status = format!("Connecting {line}"),
                Err(e) => self.status = e,
            },
            Message::ProfileName(s) => self.new_profile = s,
            Message::CreateProfile => {
                let slug = self
                    .new_profile
                    .trim()
                    .to_lowercase()
                    .replace(' ', "-");
                if slug.is_empty() {
                    self.status = "Enter a profile name".into();
                } else if let Err(e) = self.cfg.create_profile(&slug, self.new_profile.trim()) {
                    self.status = format!("{e}");
                } else {
                    self.cfg.active_profile = slug;
                    self.status = "Profile created".into();
                    self.new_profile.clear();
                }
            }
            Message::SelectProfile(name) => {
                self.cfg.ensure_community_profiles();
                if self.cfg.profiles.contains_key(&name) {
                    self.cfg.active_profile = name;
                    let _ = self.cfg.save();
                    self.palette = Palette::from_theme(&self.cfg.active().theme);
                    self.status = format!("Profile · {}", self.cfg.active().display_name);
                } else {
                    self.status = format!("Unknown profile · {name}");
                }
            }
            Message::LookSample => {}
            Message::SetAccent(a) => {
                self.cfg.active_mut().theme.accent = a;
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                if Palette::parse_rgb(&self.cfg.active().theme.accent).is_some() {
                    match self.cfg.save() {
                        Ok(()) => self.status = "Accent saved".into(),
                        Err(e) => self.status = format!("Save failed: {e}"),
                    }
                }
            }
            Message::SetUiFont(family) => {
                self.cfg.active_mut().theme.ui_font = family.clone();
                match self.cfg.save() {
                    Ok(()) => {
                        let mono = mde_theme::mono_font(&self.cfg.active().theme).to_string();
                        self.status = system::apply_ui_fonts(&family, &mono);
                    }
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::SetMonoFont(family) => {
                self.cfg.active_mut().theme.mono_font = family.clone();
                match self.cfg.save() {
                    Ok(()) => {
                        let ui = mde_theme::ui_font(&self.cfg.active().theme).to_string();
                        self.status = system::apply_ui_fonts(&ui, &family);
                    }
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::FontsQuery(q) => {
                self.fonts_query = q;
            }
            Message::SetMode(mode) => {
                self.cfg.active_mut().theme.mode = mode.clone();
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                let gtk = system::apply_contrast_theme(&mode);
                self.status = format!("Colour mode {mode} · {gtk}");
            }
            Message::SetWallpaper(path) => self.cfg.active_mut().theme.wallpaper = path,
            Message::ApplyWallpaper => {
                let path = self.cfg.active().theme.wallpaper.clone();
                match self.cfg.save() {
                    Ok(()) => match system::set_wallpaper(&path) {
                        Ok(()) => {
                            self.status = if path.trim().is_empty() {
                                "Wallpaper cleared".into()
                            } else {
                                format!("Wallpaper {path}")
                            };
                        }
                        Err(e) => self.status = e,
                    },
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::DownloadBingWallpaper => {
                self.status = "Getting today's Bing picture…".into();
                match bing::refresh_and_apply(&mut self.cfg) {
                    Ok(path) => self.status = format!("Bing wallpaper {path}"),
                    Err(e) => self.status = e.to_string(),
                }
            }
            Message::ToggleBingDaily(v) => {
                self.cfg.active_mut().theme.bing_daily = v;
                match self.cfg.save() {
                    Ok(()) => match bing::set_daily_timer(v) {
                        Ok(()) => {
                            self.status = if v {
                                "Daily Bing wallpaper on".into()
                            } else {
                                "Daily Bing wallpaper off".into()
                            };
                            if v {
                                match bing::refresh_and_apply(&mut self.cfg) {
                                    Ok(path) => {
                                        self.status =
                                            format!("Daily Bing wallpaper on · {path}");
                                    }
                                    Err(e) => {
                                        self.status = format!(
                                            "Daily Bing wallpaper on · {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => self.status = e,
                    },
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::ToggleDarkSchedule(v) => {
                self.cfg.active_mut().theme.dark_schedule = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Dark schedule on".into()
                } else {
                    "Dark schedule off".into()
                };
            }
            Message::SetDarkFrom(s) => {
                self.cfg.active_mut().theme.dark_from = s;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
            }
            Message::SetDarkUntil(s) => {
                self.cfg.active_mut().theme.dark_until = s;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
            }
            Message::SetAlign(a) => {
                let left = a.eq_ignore_ascii_case("left");
                self.cfg.active_mut().shell.taskbar_align = a;
                self.cfg.active_mut().shell.taskbar_left = left;
                let _ = self.cfg.save();
                self.status = format!("Taskbar {}", self.cfg.active().shell.taskbar_align);
            }
            Message::ApplyClassicLeftPreset => {
                // A001: keep center as default; one-click community left preset.
                self.cfg.active_mut().shell.taskbar_align = "left".into();
                self.cfg.active_mut().shell.taskbar_left = true;
                self.cfg.extras.insert(
                    "shell.classic_left_prompt_dismissed".into(),
                    toml::Value::Boolean(true),
                );
                let _ = self.cfg.save();
                self.status = "Classic left taskbar applied".into();
            }
            Message::DismissClassicLeftPrompt => {
                self.cfg.extras.insert(
                    "shell.classic_left_prompt_dismissed".into(),
                    toml::Value::Boolean(true),
                );
                let _ = self.cfg.save();
                self.status = "Classic left prompt dismissed".into();
            }
            Message::SetTaskbarSearch(mode) => {
                let mode = match mode.to_lowercase().as_str() {
                    "icon" => "icon",
                    "box" => "box",
                    _ => "hide",
                };
                self.cfg.active_mut().shell.taskbar_search = mode.into();
                let _ = self.cfg.save();
                self.status = format!("Search mode · {mode}");
            }
            Message::SetCombine(v) => {
                let mode = match v.to_lowercase().as_str() {
                    "never" => "never",
                    "whenfull" | "when_full" | "full" => "whenfull",
                    _ => "always",
                };
                self.cfg.active_mut().shell.taskbar_combine = mode.into();
                let _ = self.cfg.save();
                self.status = format!(
                    "Combine taskbar · {}",
                    match mode {
                        "never" => "Never",
                        "whenfull" => "When full",
                        _ => "Always",
                    }
                );
            }
            Message::ToggleTaskbarLabels(v) => {
                self.cfg.active_mut().shell.taskbar_labels = v;
                self.cfg
                    .extras
                    .insert("taskbar.show_labels".into(), toml::Value::Boolean(v));
                let _ = self.cfg.save();
                self.status = if v {
                    "Show labels · On".into()
                } else {
                    "Show labels · Off".into()
                };
            }
            Message::ToggleTaskbarBadges(v) => {
                self.cfg.active_mut().shell.taskbar_badges = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Taskbar badges · On".into()
                } else {
                    "Taskbar badges · Off".into()
                };
            }
            Message::SetTaskbarSize(size) => {
                let size = if size.eq_ignore_ascii_case("compact") {
                    "compact"
                } else {
                    "comfortable"
                };
                self.cfg.active_mut().shell.taskbar_size = size.into();
                let _ = self.cfg.save();
                self.status = format!("Taskbar size · {size}");
            }
            Message::ToggleXwayland(v) => {
                self.cfg.active_mut().session.xwayland = v;
                let _ = self.cfg.save();
            }
            Message::ToggleNightLight(v) => {
                self.cfg.active_mut().session.night_light = v;
                let _ = self.cfg.save();
                let strength = self.cfg.active().session.night_light_strength;
                self.status = match community::set_night_light_strength(v, strength) {
                    Ok(()) => {
                        if v {
                            "Night light on".into()
                        } else {
                            "Night light off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleDnd(v) => {
                self.cfg.active_mut().notifications.dnd = v;
                let _ = self.cfg.save();
            }
            Message::TogglePriority(v) => {
                self.cfg.active_mut().notifications.priority = v;
                let _ = self.cfg.save();
            }
            Message::SetQuietFrom(s) => {
                self.cfg.active_mut().notifications.quiet_from = s;
                let _ = self.cfg.save();
            }
            Message::SetQuietTo(s) => {
                self.cfg.active_mut().notifications.quiet_to = s;
                let _ = self.cfg.save();
            }
            Message::ApplyQuietPreset(s) => {
                let n = &mut self.cfg.active_mut().notifications;
                if s.trim().is_empty() {
                    n.quiet_from.clear();
                    n.quiet_to.clear();
                    self.status = "Quiet hours cleared".into();
                } else if let Some((from, until)) = s.split_once('|') {
                    n.quiet_from = from.trim().into();
                    n.quiet_to = until.trim().into();
                    self.status = format!("Quiet hours · {from}–{until}");
                }
                let _ = self.cfg.save();
            }
            Message::ToggleClip(v) => {
                self.cfg.active_mut().clipboard.history = v;
                let _ = self.cfg.save();
            }
            Message::ToggleCrossDevice(v) => {
                self.cfg.active_mut().clipboard.cross_device = v;
                let _ = self.cfg.save();
            }
            Message::ToggleClipClearOnLock(v) => {
                self.cfg
                    .extras
                    .insert("clipboard.clear_on_lock".into(), toml::Value::Boolean(v));
                let _ = self.cfg.save();
                self.status = if v {
                    "Clear clipboard on lock · On".into()
                } else {
                    "Clear clipboard on lock · Off".into()
                };
            }
            Message::SetClipSyncDir(s) => {
                self.cfg.active_mut().clipboard.sync_dir = s;
                let _ = self.cfg.save();
            }
            Message::ToggleWifi(v) => match system::set_wifi(v) {
                Ok(()) => {
                    self.cfg.active_mut().network.wifi = v;
                    if v {
                        self.cfg.active_mut().network.airplane_mode = false;
                    }
                    self.status = format!("Wi-Fi {}", if v { "on" } else { "off" });
                }
                Err(e) => self.status = e,
            },
            Message::ToggleBt(v) => match system::set_bluetooth(v) {
                Ok(()) => {
                    self.cfg.active_mut().network.bluetooth = v;
                    if v {
                        self.cfg.active_mut().network.airplane_mode = false;
                    }
                    self.status = format!("Bluetooth {}", if v { "on" } else { "off" });
                }
                Err(e) => self.status = e,
            },
            Message::ToggleAirplane(v) => match system::set_airplane(v) {
                Ok(()) => {
                    self.cfg.active_mut().network.airplane_mode = v;
                    if v {
                        self.cfg.active_mut().network.wifi = false;
                        self.cfg.active_mut().network.bluetooth = false;
                    }
                    self.status = format!("Airplane {}", if v { "on" } else { "off" });
                }
                Err(e) => self.status = e,
            },
            Message::RefreshOutputs => {
                self.outputs = system::outputs_summary();
                self.layout_heads = system::layout_heads(&self.cfg);
            }
            Message::OutputX(name, value) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    head.x_text = value;
                }
            }
            Message::OutputY(name, value) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    head.y_text = value;
                }
            }
            Message::OutputScale(name, scale) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    head.scale = scale;
                }
            }
            Message::SaveLayout => pages::system_pages::save_layout(self),
            Message::ApplyDisplayProfile(name) => {
                match std::process::Command::new("mde-comp")
                    .args(["ctl", "apply-profile", &name])
                    .output()
                {
                    Ok(o) if o.status.success() => {
                        self.status = format!("Applied display profile “{name}”");
                    }
                    Ok(o) => {
                        self.status = format!(
                            "mde-comp ctl: {}",
                            String::from_utf8_lossy(&o.stderr).trim()
                        );
                        if self.status == "mde-comp ctl: " {
                            self.status = String::from_utf8_lossy(&o.stdout).trim().into();
                        }
                    }
                    Err(e) => self.status = format!("mde-comp ctl: {e}"),
                }
            }
            Message::ToggleFractional(v) => {
                self.cfg.active_mut().displays.allow_fractional_scale = v;
                let _ = self.cfg.save();
            }
            Message::RefreshMmc => {
                self.disks = system::disks();
                self.services = system::services();
                self.status = "Refreshed disks / services".into();
            }
            Message::ToggleAdvancedGroup(name, on) => {
                let folder_id = format!("mde-folder:{name}");
                let start = &mut self.cfg.active_mut().start;
                if on {
                    start.pins.retain(|p| p != &folder_id);
                    if !start
                        .advanced_groups
                        .iter()
                        .any(|g| g.eq_ignore_ascii_case(&name))
                    {
                        start.advanced_groups.push(name.clone());
                    }
                    self.status = format!("Advanced · folder · {name}");
                } else {
                    start
                        .advanced_groups
                        .retain(|g| !g.eq_ignore_ascii_case(&name));
                    if !start.pins.iter().any(|p| p == &folder_id) {
                        start.pins.push(folder_id);
                    }
                    self.status = format!("Restored Start folder · {name}");
                }
                match self.cfg.save() {
                    Ok(()) => {}
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::RestoreAdvancedApp(id) => {
                let start = &mut self.cfg.active_mut().start;
                start.advanced_apps.retain(|a| a != &id);
                if !start.pins.iter().any(|p| p == &id) {
                    start.pins.push(id.clone());
                }
                match self.cfg.save() {
                    Ok(()) => self.status = format!("Restored to Start · {id}"),
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::LaunchMmc => {
                let _ = std::process::Command::new("mde-mmc").spawn();
                self.status = "Opened Management Console (mde-mmc)".into();
            }
            Message::LaunchMmcComp => {
                let _ = std::process::Command::new("mde-mmc")
                    .arg("compmgmt.msc")
                    .spawn();
                self.status = "Opened Computer Management".into();
            }
            Message::LaunchMmcPerf => {
                let _ = std::process::Command::new("mde-mmc")
                    .arg("perfmon.msc")
                    .spawn();
                self.status = "Opened Performance Monitor".into();
            }
            Message::SetIdleLockEdit(s) => self.idle_lock_edit = s,
            Message::ApplyIdleLock => {
                match self.idle_lock_edit.trim().parse::<u64>() {
                    Ok(v) => {
                        self.cfg.active_mut().session.idle_lock_seconds = v;
                        let _ = self.cfg.save();
                        self.status = format!("Idle lock {v}s");
                    }
                    Err(_) => self.status = "Bad idle lock value".into(),
                }
            }
            Message::ToggleRecentlyAdded(v) => {
                self.cfg.active_mut().start.recently_added = v;
                let _ = self.cfg.save();
            }
            Message::ToggleTipsOptIn(v) => {
                self.cfg.active_mut().start.tips_opt_in = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Tips · opt-in On".into()
                } else {
                    "Tips · Off (no-promo default)".into()
                };
            }
            Message::ToggleVoiceSearch(v) => {
                self.cfg
                    .extras
                    .insert("start.voice_search".into(), toml::Value::Boolean(v));
                let _ = self.cfg.save();
                self.status = if v {
                    "Voice search on · set start.voice_stt_url".into()
                } else {
                    "Voice search off".into()
                };
            }
            Message::SetVoiceSttUrl(s) => {
                if s.trim().is_empty() {
                    self.cfg.extras.remove("start.voice_stt_url");
                } else {
                    self.cfg.extras.insert(
                        "start.voice_stt_url".into(),
                        toml::Value::String(s.trim().to_string()),
                    );
                }
                let _ = self.cfg.save();
                self.status = "Network STT endpoint saved".into();
            }
            Message::ToggleWidgetCard(id, v) => {
                let w = &mut self.cfg.active_mut().widgets;
                match id.as_str() {
                    "weather" => w.weather = v,
                    "clock" => w.clock = v,
                    "calendar" => w.calendar = v,
                    "system" => w.system = v,
                    "tip" => w.tip = v,
                    "photos" => w.photos = v,
                    "background" => w.background = v,
                    "news" => w.news = v,
                    "markets" => w.markets = v,
                    "traffic" => w.traffic = v,
                    "third_party" => w.third_party = v,
                    _ => {}
                }
                let _ = self.cfg.save();
                self.status = format!("Widgets · {id} {}", if v { "on" } else { "off" });
            }
            Message::SetStartLayout(layout) => {
                self.cfg.active_mut().start.layout = layout;
                let _ = self.cfg.save();
                self.status = format!("Start layout · {}", self.cfg.active().start.layout);
            }
            Message::SetStartPinDefaultSize(size) => {
                let size = match size.to_lowercase().as_str() {
                    "small" => "small",
                    "large" => "large",
                    _ => "medium",
                };
                self.cfg.extras.insert(
                    "start.pin_default_size".into(),
                    toml::Value::String(size.into()),
                );
                let _ = self.cfg.save();
                self.status = format!("Start pin size · {size}");
            }
            Message::ToggleStartFolder(id, on) => {
                let folders = &mut self.cfg.active_mut().start.folders;
                if on {
                    if !folders.iter().any(|f| f.eq_ignore_ascii_case(&id)) {
                        folders.push(id.clone());
                    }
                } else {
                    folders.retain(|f| !f.eq_ignore_ascii_case(&id));
                }
                let _ = self.cfg.save();
                self.status = if on {
                    format!("Start folder on · {id}")
                } else {
                    format!("Start folder off · {id}")
                };
            }
            Message::ToggleTaskbarAutohide(v) => {
                self.cfg.active_mut().shell.taskbar_autohide = v;
                let _ = self.cfg.save();
            }
            Message::ToggleWidgetsBtn(v) => {
                self.cfg.active_mut().shell.widgets_btn = v;
                let _ = self.cfg.save();
            }
            Message::ToggleNtp(v) => {
                self.status = match system::set_ntp(v) {
                    Ok(()) => {
                        if v {
                            "NTP on".into()
                        } else {
                            "NTP off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleClockSeconds(v) => {
                self.cfg.active_mut().shell.clock_seconds = v;
                match self.cfg.save() {
                    Ok(()) => {
                        self.status = if v {
                            "Clock seconds · On".into()
                        } else {
                            "Clock seconds · Off".into()
                        }
                    }
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::ToggleClockDateIso(v) => {
                self.cfg.active_mut().shell.clock_date_iso = v;
                match self.cfg.save() {
                    Ok(()) => {
                        self.status = if v {
                            "Clock date · ISO".into()
                        } else {
                            "Clock date · month and day".into()
                        }
                    }
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::ToggleClock24h(v) => {
                self.cfg
                    .extras
                    .insert("clock_24h".into(), toml::Value::Boolean(v));
                let _ = self.cfg.save();
                self.status = if v {
                    "24-hour clock on".into()
                } else {
                    "24-hour clock off".into()
                };
            }
            Message::RefreshTime => self.status = "Time status refreshed".into(),
            Message::ToggleGameMode(v) => {
                self.status = match community::set_game_mode_with_dnd(&mut self.cfg, v) {
                    Ok(s) => s,
                    Err(e) => e,
                };
            }
            Message::ToggleShowExtensions(v) => {
                self.cfg.active_mut().theme.show_file_extensions = v;
                let _ = self.cfg.save();
                self.status = match system::set_show_file_extensions(v) {
                    Ok(()) => {
                        if v {
                            "Show file extensions on".into()
                        } else {
                            "Show file extensions off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleShowHiddenFiles(v) => {
                self.cfg.active_mut().theme.show_hidden_files = v;
                let _ = self.cfg.save();
                self.status = match system::set_show_hidden_files(v) {
                    Ok(()) => {
                        if v {
                            "Show hidden files on".into()
                        } else {
                            "Show hidden files off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleAlwaysEndTask(v) => {
                self.cfg.active_mut().shell.end_task_always = v;
                self.status = match self.cfg.save() {
                    Ok(()) => {
                        if v {
                            "Always show End task".into()
                        } else {
                            "End task when Shift held".into()
                        }
                    }
                    Err(e) => e.to_string(),
                };
            }
            Message::OpenScreenshotDir => {
                let dir = system::screenshot_dir(&self.cfg);
                let _ = std::fs::create_dir_all(&dir);
                self.status = match system::open_path(&dir) {
                    Ok(()) => format!("Opened {dir}"),
                    Err(e) => e,
                };
            }
            Message::SetScreenshotDirEdit(s) => self.screenshot_dir_edit = s,
            Message::ApplyScreenshotDir => {
                let path = self.screenshot_dir_edit.trim().to_string();
                self.cfg.active_mut().shell.screenshot_dir = path.clone();
                match self.cfg.save() {
                    Ok(()) => {
                        let _ = std::fs::create_dir_all(system::expand_home(&path));
                        self.status = format!("Captures folder · {path}");
                    }
                    Err(e) => self.status = format!("Save failed: {e}"),
                }
            }
            Message::PickScreenshotDir => {
                let picked = system::pick_directory("Captures folder");
                if let Some(p) = picked {
                    self.screenshot_dir_edit = p.clone();
                    self.cfg.active_mut().shell.screenshot_dir = p.clone();
                    match self.cfg.save() {
                        Ok(()) => self.status = format!("Captures folder · {p}"),
                        Err(e) => self.status = format!("Save failed: {e}"),
                    }
                } else {
                    self.status = "Folder pick cancelled".into();
                }
            }
            Message::SetTextScaleEdit(s) => self.text_scale_edit = s,
            Message::ApplyTextScale => {
                let v = self.text_scale_edit.trim().to_string();
                self.status = match v.parse::<f64>() {
                    Ok(n) => match system::apply_text_scale(&mut self.cfg, n) {
                        Ok(s) => s,
                        Err(e) => e,
                    },
                    Err(_) => "Bad text scale".into(),
                };
            }
            Message::ApplyTextScaleValue(v) => {
                self.text_scale_edit = v.clone();
                self.status = match v.parse::<f64>() {
                    Ok(n) => match system::apply_text_scale(&mut self.cfg, n) {
                        Ok(s) => s,
                        Err(e) => e,
                    },
                    Err(_) => "Bad text scale".into(),
                };
            }
            Message::ToggleAnimations(v) => {
                self.cfg.active_mut().theme.animations = v;
                let _ = self.cfg.save();
                let _ = system::gsettings_set(
                    "org.gnome.desktop.interface",
                    "enable-animations",
                    if v { "true" } else { "false" },
                );
                self.status = if v {
                    "Animation effects · On".into()
                } else {
                    "Animation effects · Off".into()
                };
            }
            Message::ToggleTransparency(v) => {
                self.cfg.active_mut().theme.transparency = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    format!(
                        "Transparency effects · On · {}%",
                        self.cfg.active().theme.opacity.clamp(1, 100)
                    )
                } else {
                    "Transparency effects · Off · solid".into()
                };
            }
            Message::SetOpacity(v) => {
                let v = v.clamp(1, 100);
                self.cfg.active_mut().theme.opacity = v;
                self.cfg.active_mut().theme.transparency = true;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = format!("Opacity · {v}%");
            }
            Message::ToggleDynamicTransparency(v) => {
                self.cfg.active_mut().theme.dynamic_transparency = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Dynamic transparency · On".into()
                } else {
                    "Dynamic transparency · Off".into()
                };
            }
            Message::SetBlurPreset(preset) => {
                let preset = match preset.to_lowercase().as_str() {
                    "off" | "none" => "off",
                    "subtle" => "subtle",
                    "thick" | "strong" => "thick",
                    _ => "acrylic",
                };
                self.cfg.active_mut().theme.blur_preset = preset.into();
                self.cfg.active_mut().theme.transparency = true;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = format!("Blur preset · {preset}");
            }
            Message::ToggleAccentOnStartTaskbar(v) => {
                self.cfg.active_mut().theme.accent_on_start_taskbar = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Accent on Start and taskbar · On".into()
                } else {
                    "Accent on Start and taskbar · Off".into()
                };
            }
            Message::ToggleAccentOnTitlebars(v) => {
                self.cfg.active_mut().theme.accent_on_titlebars = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Accent on title bars · On".into()
                } else {
                    "Accent on title bars · Off".into()
                };
            }
            Message::ToggleAlwaysShowScrollbars(v) => {
                self.cfg.active_mut().theme.always_show_scrollbars = v;
                let _ = self.cfg.save();
                // GTK/libadwaita: overlay-scrolling false → always-visible bars.
                let _ = system::gsettings_set(
                    "org.gnome.desktop.interface",
                    "overlay-scrolling",
                    if v { "false" } else { "true" },
                );
                self.status = if v {
                    "Always show scrollbars · On".into()
                } else {
                    "Always show scrollbars · Off".into()
                };
            }
            Message::SetCursorSize(size) => {
                self.status = match system::gsettings_set(
                    "org.gnome.desktop.interface",
                    "cursor-size",
                    &size.to_string(),
                ) {
                    Ok(()) => format!("Cursor size {size}"),
                    Err(e) => e,
                };
            }
            Message::HostnameEdit(s) => self.hostname_edit = s,
            Message::ApplyHostname => {
                self.status = match system::set_hostname(&self.hostname_edit) {
                    Ok(()) => {
                        self.hostname_edit = system::pretty_hostname();
                        format!("Renamed · {}", self.hostname_edit)
                    }
                    Err(e) => e,
                };
            }

            Message::RefreshNearby => {
                if kdeconnect::installed() {
                    self.nearby_devices = kdeconnect::list_devices();
                    let _ = kdeconnect::refresh_discovery();
                    self.nearby_devices = kdeconnect::list_devices();
                    self.nearby_hint = format!("{} device(s)", self.nearby_devices.len());
                    self.status = format!("Nearby · {}", self.nearby_hint);
                } else {
                    self.nearby_devices.clear();
                    self.nearby_hint = "Install kde-connect (dnf) · firewall 1716–1764".into();
                    self.status = self.nearby_hint.clone();
                }
            }
            Message::NearbyPair(id) => {
                self.status = match kdeconnect::pair(&id) {
                    Ok(()) => {
                        self.nearby_devices = kdeconnect::list_devices();
                        format!("Pair requested · {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::NearbyUnpair(id) => {
                self.status = match kdeconnect::unpair(&id) {
                    Ok(()) => {
                        self.nearby_devices = kdeconnect::list_devices();
                        format!("Unpaired · {id}")
                    }
                    Err(e) => e,
                };
            }


            Message::ToggleCommandPalette(v) => {
                self.cfg.active_mut().tools.command_palette = v;
                let _ = self.cfg.save();
                self.status = if v { "Command Palette on" } else { "Command Palette off" }.into();
            }
            Message::ToggleFancyZones(v) => {
                self.cfg.active_mut().tools.fancy_zones = v;
                let _ = self.cfg.save();
                self.status = if v { "FancyZones on" } else { "FancyZones off" }.into();
            }
            Message::SetActiveZoneLayout(name) => {
                self.cfg.active_mut().tools.active_zone_layout = name.clone();
                let _ = self.cfg.save();
                self.status = format!("Active zone · {name}");
            }
            Message::ToggleSnapGroups(v) => {
                self.cfg.active_mut().tools.snap_groups = v;
                let _ = self.cfg.save();
                self.status = if v { "Snap Groups on" } else { "Snap Groups off" }.into();
            }
            Message::ToggleAlwaysOnTopShortcut(v) => {
                self.cfg.active_mut().tools.always_on_top_shortcut = v;
                let _ = self.cfg.save();
                self.status = "Always on top shortcut saved".into();
            }
            Message::ToggleAlwaysOnTopIndicator(v) => {
                self.cfg.active_mut().tools.always_on_top_indicator = v;
                let _ = self.cfg.save();
                self.status = "Always on top indicator saved".into();
            }
            Message::ToggleColorPickerHistory(v) => {
                self.cfg.active_mut().tools.color_picker_history = v;
                let _ = self.cfg.save();
                self.status = "Color picker history saved".into();
            }
            Message::ToggleFindMyMouse(v) => {
                self.cfg.active_mut().tools.find_my_mouse = v;
                let _ = self.cfg.save();
                self.status = "Find My Mouse saved".into();
            }
            Message::ToggleMouseHighlighter(v) => {
                self.cfg.active_mut().tools.mouse_highlighter = v;
                let _ = self.cfg.save();
                self.status = "Mouse highlighter saved".into();
            }
            Message::SetAwakeMinutesEdit(s) => self.awake_minutes_edit = s,
            Message::ApplyAwakeMinutes => {
                match self.awake_minutes_edit.trim().parse::<u32>() {
                    Ok(n) => {
                        self.cfg.active_mut().tools.awake_minutes = n;
                        let _ = self.cfg.save();
                        self.status = format!("Awake default · {n} min");
                    }
                    Err(_) => self.status = "Awake minutes must be a number".into(),
                }
            }
            Message::ToggleAdvancedPaste(v) => {
                self.cfg.active_mut().tools.advanced_paste = v;
                let _ = self.cfg.save();
                self.status = "Advanced Paste saved".into();
            }
            Message::SetJumpListMax(n) => {
                self.cfg.active_mut().tools.jump_list_max = n;
                let _ = self.cfg.save();
                self.status = format!("Jump list depth · {n}");
            }
            Message::SetWorkspaceNameEdit(s) => self.workspace_name_edit = s,
            Message::SaveWorkspacePreset => {
                let name = self.workspace_name_edit.trim().to_string();
                if name.is_empty() {
                    self.status = "Enter a workspace name".into();
                } else {
                    let apps: Vec<mde_config::WorkspaceApp> = self
                        .cfg
                        .active()
                        .shell
                        .pins
                        .iter()
                        .map(|id| mde_config::WorkspaceApp {
                            id: id.clone(),
                            zone: -1,
                        })
                        .collect();
                    let preset = mde_config::WorkspacePreset { name: name.clone(), apps };
                    let tools = &mut self.cfg.active_mut().tools;
                    tools.workspaces.retain(|w| w.name != name);
                    tools.workspaces.push(preset);
                    let _ = self.cfg.save();
                    self.workspace_name_edit.clear();
                    self.status = format!("Workspace saved · {name}");
                }
            }
            Message::LaunchWorkspace(name) => {
                self.status = system::launch_workspace(&self.cfg, &name);
            }
            Message::DeleteWorkspace(name) => {
                self.cfg
                    .active_mut()
                    .tools
                    .workspaces
                    .retain(|w| w.name != name);
                let _ = self.cfg.save();
                self.status = format!("Deleted workspace · {name}");
            }
            Message::SetRemapFromEdit(s) => self.remap_from_edit = s,
            Message::SetRemapToEdit(s) => self.remap_to_edit = s,
            Message::AddKeyRemap => {
                let from = self.remap_from_edit.trim().to_string();
                let to = self.remap_to_edit.trim().to_string();
                if from.is_empty() || to.is_empty() {
                    self.status = "Enter from and to keysyms".into();
                } else {
                    self.cfg
                        .active_mut()
                        .tools
                        .keyboard_remaps
                        .push(mde_config::KeyRemap { from, to });
                    let _ = self.cfg.save();
                    self.remap_from_edit.clear();
                    self.remap_to_edit.clear();
                    system::publish_key_remaps(&self.cfg);
                    self.status = "Remap added".into();
                }
            }
            Message::RemoveKeyRemap(i) => {
                let remaps = &mut self.cfg.active_mut().tools.keyboard_remaps;
                if i < remaps.len() {
                    remaps.remove(i);
                    let _ = self.cfg.save();
                    system::publish_key_remaps(&self.cfg);
                    self.status = "Remap removed".into();
                }
            }
            Message::TrayShowIcon(id) => {
                system::tray_set_hidden(&mut self.cfg, &id, false);
                self.tray_known = system::tray_known_ids();
                self.status = format!("Tray show · {id}");
            }
            Message::TrayHideIcon(id) => {
                system::tray_set_hidden(&mut self.cfg, &id, true);
                self.tray_known = system::tray_known_ids();
                self.status = format!("Tray hide · {id}");
            }
            Message::SetAppNotifBanners(app, v) => {
                system::set_app_notif_field(&app, "banners", v);
                self.notif_app_prefs = system::load_app_notif_pref_rows();
                self.status = format!("{app} banners · {v}");
            }
            Message::SetAppNotifSound(app, v) => {
                system::set_app_notif_field(&app, "sound", v);
                self.notif_app_prefs = system::load_app_notif_pref_rows();
                self.status = format!("{app} sound · {v}");
            }
            Message::SetAppNotifPriority(app, v) => {
                system::set_app_notif_field(&app, "priority", v);
                self.notif_app_prefs = system::load_app_notif_pref_rows();
                self.status = format!("{app} priority · {v}");
            }

            Message::CopyAboutSpecs => {
                self.status = match system::copy_about_specs() {
                    Ok(()) => "About text copied".into(),
                    Err(e) => e,
                };
            }
            Message::CheckUpdates => {
                self.status = "Checking for updates…".into();
                match system::update_check_result() {
                    Ok(summary) => {
                        self.update_summary = summary;
                        self.status = "Update check finished".into();
                    }
                    Err(err) => {
                        self.update_summary = "We couldn’t check for updates".into();
                        self.status = err;
                    }
                }
            }
            Message::RefreshUpdateHistory => {
                self.update_history = system::update_history();
                self.status = "Update history refreshed".into();
            }
            Message::UpdateInstallAll => {
                self.status = system::update_install_all();
                self.update_summary = system::update_check_summary();
            }
            Message::UpdateRestartNow => {
                self.status = system::power_reboot();
            }
            Message::UpdateAndShutDown => {
                self.status = system::trigger_offline_update("power-off");
            }
            Message::UpdateScheduleRestart => {
                self.status = system::schedule_restart_tonight();
            }
            Message::UpdateCancelOffline => {
                self.status = system::cancel_offline_update();
                self.update_summary = system::update_check_summary();
            }
            Message::SetPowerProfile(profile) => {
                self.status = match system::set_power_profile(&profile) {
                    Ok(()) => format!("Power mode {profile}"),
                    Err(e) => e,
                };
            }
            Message::SetIdleDpmsEdit(s) => self.idle_dpms_edit = s,
            Message::ApplyIdleDpms => match self.idle_dpms_edit.trim().parse::<u64>() {
                Ok(v) => {
                    self.cfg.active_mut().session.idle_dpms_seconds = v;
                    let _ = self.cfg.save();
                    self.status = format!("DPMS {v}s");
                }
                Err(_) => self.status = "Bad DPMS value".into(),
            },
            Message::StartFocusSession(minutes) => {
                match system::start_focus_session(minutes) {
                    Ok(()) => {
                        self.cfg.active_mut().notifications.dnd = true;
                        let _ = self.cfg.save();
                        self.status = format!("Focus session · {minutes} min");
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::EndFocusSession => {
                let _ = system::end_focus_session();
                self.cfg.active_mut().notifications.dnd = false;
                let _ = self.cfg.save();
                self.status = "Focus session ended".into();
            }
            Message::ToggleMouseNatural(v) => {
                self.status = match system::set_mouse_natural_scroll(v) {
                    Ok(()) => {
                        if v {
                            "Mouse natural scroll on".into()
                        } else {
                            "Mouse natural scroll off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleTouchpadTap(v) => {
                self.status = match system::set_touchpad_tap(v) {
                    Ok(()) => {
                        if v {
                            "Tap to click on".into()
                        } else {
                            "Tap to click off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleTouchpadNatural(v) => {
                self.status = match system::set_touchpad_natural_scroll(v) {
                    Ok(()) => {
                        if v {
                            "Touchpad natural scroll on".into()
                        } else {
                            "Touchpad natural scroll off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleMagnifierNative(v) => {
                self.cfg.active_mut().accessibility.magnifier_native = v;
                self.status = match self.cfg.save() {
                    Ok(()) => {
                        if v {
                            "Compositor-native magnifier".into()
                        } else {
                            "External magnifier tools preferred".into()
                        }
                    }
                    Err(e) => format!("Save failed: {e}"),
                };
            }
            Message::SetMagnifierFactor(f) => {
                self.cfg.active_mut().accessibility.magnifier_factor = f.clamp(1.25, 4.0);
                let _ = self.cfg.save();
                self.status = format!("Magnifier ×{:.2}", self.cfg.active().accessibility.magnifier_factor);
            }
            Message::SetMagnifierMode(mode) => {
                self.cfg.active_mut().accessibility.magnifier_mode = mode.clone();
                let _ = self.cfg.save();
                self.status = format!("Magnifier mode {mode}");
            }
            Message::ToggleMagnifierFollowFocus(v) => {
                self.cfg.active_mut().accessibility.magnifier_follow_focus = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Magnifier follows focus".into()
                } else {
                    "Magnifier follow-focus off".into()
                };
            }
            Message::ToggleShortcutGuide(v) => {
                self.cfg.active_mut().tools.shortcut_guide = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Shortcut Guide on (Super+/)".into()
                } else {
                    "Shortcut Guide off".into()
                };
            }
            Message::ToggleImageResizer(v) => {
                self.cfg.active_mut().tools.image_resizer = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Image Resizer on".into()
                } else {
                    "Image Resizer off".into()
                };
            }
            Message::InstallImageResizerUca => {
                let presets = self.cfg.active().tools.image_resizer_presets.clone();
                self.status = match system::install_image_resizer_uca(&presets) {
                    Ok(s) => s,
                    Err(e) => e,
                };
            }
            Message::SetStylePack(id) => {
                self.cfg.active_mut().theme.style_pack = id.clone();
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if id.is_empty() {
                    "Stock style".into()
                } else {
                    format!("Style pack {id}")
                };
            }
            Message::ToggleStylePacks(v) => {
                self.cfg.active_mut().theme.style_packs_enabled = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Style packs enabled".into()
                } else {
                    "Style packs disabled".into()
                };
            }
            Message::EnsureSampleStylePack => {
                self.status = match mde_config::style_packs::ensure_sample_pack() {
                    Ok(p) => format!("Sample pack · {}", p.display()),
                    Err(e) => e.to_string(),
                };
            }
            Message::SetTaskbarMultiMode(mode) => {
                self.cfg.active_mut().shell.taskbar_multi_mode = mode.clone();
                self.cfg.active_mut().shell.taskbar_all_displays =
                    !mode.eq_ignore_ascii_case("main_only");
                let _ = self.cfg.save();
                self.status = format!("Multi-monitor taskbar · {mode}");
            }
            Message::SetWorldClock(idx, tz) => {
                let clocks = &mut self.cfg.active_mut().shell.world_clocks;
                while clocks.len() <= idx {
                    clocks.push(String::new());
                }
                clocks[idx] = tz.trim().to_string();
                clocks.truncate(2);
                clocks.retain(|c| !c.is_empty());
                let _ = self.cfg.save();
                self.status = "World clocks saved".into();
            }
            Message::ToggleStorageSense(v) => {
                self.cfg.active_mut().storage.sense_enabled = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Storage Sense on".into()
                } else {
                    "Storage Sense off".into()
                };
            }
            Message::ToggleStorageSenseTemp(v) => {
                self.cfg.active_mut().storage.sense_temp = v;
                let _ = self.cfg.save();
            }
            Message::ToggleStorageSenseTrash(v) => {
                self.cfg.active_mut().storage.sense_trash = v;
                let _ = self.cfg.save();
            }
            Message::SetStorageSenseDays(d) => {
                self.cfg.active_mut().storage.sense_downloads_days = d;
                let _ = self.cfg.save();
                self.status = format!("Downloads cleanup · {d} days");
            }
            Message::RunStorageSense => {
                self.status = system::run_storage_sense(&self.cfg);
            }
            Message::TogglePrivacyIndicators(v) => {
                self.cfg.active_mut().accessibility.privacy_indicators = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Privacy indicators on".into()
                } else {
                    "Privacy indicators off".into()
                };
            }
            Message::ToggleAutoOsk(v) => {
                self.cfg.active_mut().accessibility.auto_osk = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Auto OSK on".into()
                } else {
                    "Auto OSK off".into()
                };
            }
            Message::SetTabletMode(mode) => {
                self.cfg.active_mut().accessibility.tablet_mode = mode.clone();
                let _ = self.cfg.save();
                self.status = format!("Tablet mode · {mode}");
            }
            Message::SetGesture(which, action) => {
                let g = &mut self.cfg.active_mut().gestures;
                match which.as_str() {
                    "three_up" => g.three_up = action.clone(),
                    "three_down" => g.three_down = action.clone(),
                    "three_left" => g.three_left = action.clone(),
                    "three_right" => g.three_right = action.clone(),
                    "four_up" => g.four_up = action.clone(),
                    "four_tap" => g.four_tap = action.clone(),
                    _ => {}
                }
                let _ = self.cfg.save();
                system::publish_gestures(&self.cfg);
                self.status = format!("Gesture {which} → {action}");
            }
            Message::RefreshPrivacyDashboard => {
                self.status = format!(
                    "Privacy · {} active",
                    system::privacy_in_use().len()
                );
            }
            Message::ResizeImagePick(preset) => {
                if let Some(path) = system::pick_file("Resize image") {
                    self.status = match system::resize_image(&path, &preset) {
                        Ok(out) => format!("Resized · {out}"),
                        Err(e) => e,
                    };
                } else {
                    self.status = "Image pick cancelled".into();
                }
            }
            Message::SetDarkScheduleMode(mode) => {
                let mode = if mode.eq_ignore_ascii_case("sunrise_sunset") {
                    "sunrise_sunset"
                } else {
                    "fixed"
                };
                self.cfg.active_mut().theme.dark_schedule_mode = mode.into();
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = format!("Dark schedule · {mode}");
            }
            Message::ToggleSpotlight(v) => {
                self.cfg.active_mut().theme.spotlight = v;
                if v {
                    self.cfg.active_mut().theme.bing_daily = true;
                }
                let _ = self.cfg.save();
                self.status = match bing::set_daily_timer(v || self.cfg.active().theme.bing_daily) {
                    Ok(()) => {
                        if v {
                            match bing::refresh_and_apply(&mut self.cfg) {
                                Ok(path) => {
                                    let p = std::path::PathBuf::from(&path);
                                    let _ = community::record_spotlight(&mut self.cfg, &p);
                                    format!("Spotlight on · {path}")
                                }
                                Err(e) => format!("Spotlight on · {e}"),
                            }
                        } else {
                            "Spotlight off".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleSpotlightLock(v) => {
                self.cfg.active_mut().theme.spotlight_lock = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Spotlight on lock screen".into()
                } else {
                    "Spotlight lock off".into()
                };
            }
            Message::SetNightStrength(v) | Message::SetNightLightStrength(v) => {
                self.cfg.active_mut().session.night_light_strength = v;
                let on = self.cfg.active().session.night_light;
                let _ = self.cfg.save();
                self.status = match community::set_night_light_strength(on, v) {
                    Ok(()) => format!("Night light strength {v}%"),
                    Err(e) => e,
                };
            }
            Message::ToggleNightSchedule(v) | Message::ToggleNightLightSchedule(v) => {
                self.cfg.active_mut().session.night_light_schedule = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Night light schedule on".into()
                } else {
                    "Night light schedule off".into()
                };
            }
            Message::SetNightFrom(s) => {
                self.cfg.active_mut().session.night_from = s;
                let _ = self.cfg.save();
            }
            Message::SetNightUntil(s) => {
                self.cfg.active_mut().session.night_until = s;
                let _ = self.cfg.save();
            }
            Message::ToggleAltSnap(v) => {
                self.cfg.active_mut().compositor.alt_snap = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Alt+drag move/resize on".into()
                } else {
                    "Alt+drag off".into()
                };
            }
            Message::SetSsdRadius(r) | Message::SetSsdCornerRadius(r) => {
                let r = r.min(24);
                self.cfg.active_mut().theme.ssd_corner_radius = r;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = format!("SSD corner radius {r}px");
            }
            Message::ToggleSsdMica(v) => {
                self.cfg.active_mut().theme.ssd_mica = v;
                let _ = self.cfg.save();
                self.palette = Palette::from_theme(&self.cfg.active().theme);
                self.status = if v {
                    "Mica title bars on".into()
                } else {
                    "Solid title bars".into()
                };
            }
            Message::IdentifyDisplays => {
                self.status = community::identify_displays(&self.layout_heads);
            }
            Message::RevertLayout => {
                self.layout_heads = system::layout_heads(&self.cfg);
                self.status = "Reverted positions from saved layout profile".into();
            }
            Message::ApplyOutputTuning => {
                let mut lines = Vec::new();
                for head in &self.layout_heads {
                    lines.push(community::apply_head_tuning(head));
                }
                self.status = lines.join(" · ");
            }
            Message::ProjectMode(mode) => {
                self.cfg.active_mut().displays.project_mode = mode.clone();
                let _ = self.cfg.save();
                self.status = match community::apply_project_mode(&self.layout_heads, &mode) {
                    Ok(s) => s,
                    Err(e) => e,
                };
            }
            Message::NudgeOutput(name, dx, dy) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    let x: i32 = head.x_text.parse().unwrap_or(0);
                    let y: i32 = head.y_text.parse().unwrap_or(0);
                    head.x_text = (x + dx).to_string();
                    head.y_text = (y + dy).to_string();
                    self.status = format!("{name} → {},{}", head.x_text, head.y_text);
                }
            }
            Message::OutputRefresh(name, hz) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    head.refresh_hz = hz;
                    self.status = format!("{name} · {hz} Hz");
                }
            }
            Message::ToggleOutputHdr(name, v) => {
                if let Some(head) = self.layout_heads.iter_mut().find(|h| h.name == name) {
                    head.hdr = v;
                    self.status = format!(
                        "{} · HDR {}",
                        name,
                        if v { "on" } else { "off" }
                    );
                }
            }
            Message::StartupQuery(q) => self.startup_query = q,
            Message::ToggleStartup(path, enabled) => {
                self.status = match community::set_startup_enabled(std::path::Path::new(&path), enabled)
                {
                    Ok(()) => {
                        if enabled {
                            "Startup enabled".into()
                        } else {
                            "Startup disabled".into()
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::SetMimeDefault(mime, desktop) => {
                self.status = match community::set_xdg_mime_default(&mime, &desktop) {
                    Ok(()) => format!("Default · {desktop}"),
                    Err(e) => e,
                };
            }
            Message::UninstallApp(id) => {
                self.status = community::uninstall_desktop_app(&id);
            }
            Message::RepairApp(id) => {
                self.status = community::reset_app_data(&id);
            }
            Message::PickAccountPicture => {
                self.status = community::pick_account_picture();
            }
            Message::SetDesktopName(idx, name) => {
                community::set_desktop_name(&mut self.cfg, idx, name);
                let _ = self.cfg.save();
                self.status = format!("Desktop {} renamed", idx + 1);
            }
            Message::SetDesktopWallpaper(idx, path) => {
                community::set_desktop_wallpaper(&mut self.cfg, idx, path);
                let _ = self.cfg.save();
                self.status = format!("Desktop {} wallpaper set", idx + 1);
            }
            Message::ApplyDesktopWallpaper(i) => {
                let active = self.cfg.active();
                let path = active
                    .desktops
                    .wallpaper_for(i)
                    .filter(|p| std::path::Path::new(p).is_file())
                    .unwrap_or_else(|| active.theme.wallpaper.clone());
                if path.trim().is_empty() {
                    self.status = "No wallpaper for this desktop".into();
                } else {
                    self.status = match system::set_wallpaper(&path) {
                        Ok(()) => format!("Desktop {} wallpaper applied", i + 1),
                        Err(e) => e,
                    };
                }
            }
            Message::ToggleGameModeDnd(v) => {
                self.cfg.active_mut().session.game_mode_dnd = v;
                let _ = self.cfg.save();
                self.status = if v {
                    "Game Mode will enable Do not disturb".into()
                } else {
                    "Game Mode leaves notifications alone".into()
                };
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = &self.palette;
        let category = nav::category_of(&self.page);
        let q = self.search.to_lowercase();
        let canvas = widgets::canvas_color(palette);
        let fg = widgets::fg_color(palette);
        let cap = widgets::caption_color(palette);

        let back_enabled = !self.history.is_empty();
        let back = button(text("<").size(16).color(if back_enabled { fg } else { cap }))
            .on_press_maybe(back_enabled.then_some(Message::GoBack))
            .style(button::text)
            .padding([8, 10]);

        let account_name = system::user_display_name();
        let account_face: Element<'_, Message> = if let Some(path) = system::face_path() {
            container(image(path).width(48).height(48))
                .style(|_theme| ContainerStyle {
                    border: Border {
                        radius: 24.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        } else {
            container(
                row![text(account_name.chars().next().unwrap_or('U').to_string()).size(20).color(fg)]
                    .width(48)
                    .height(48)
                    .align_y(iced::Alignment::Center),
            )
                .width(48)
                .height(48)
                .style(|_theme| ContainerStyle {
                    background: Some(Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.15))),
                    border: Border {
                        radius: 24.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        };
        let account = button(
            row![
                account_face,
                column![
                    text(account_name).size(13).color(fg),
                    text("Local account").size(11).color(cap),
                ]
                .spacing(2),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::Navigate("yourinfo".into()))
        .style(widgets::nav_quiet)
        .padding([10, 12])
        .width(Fill);

        let search = container(
            text_input("Find a setting", &self.search).on_input(Message::SearchChanged),
        )
        .padding([6, 10])
        .width(Fill)
        .style(move |_theme| ContainerStyle {
            background: Some(Background::Color(widgets::card_fill(palette))),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: widgets::card_border(palette),
            },
            ..Default::default()
        });

        let mut nav_col = column![back, account, search]
            .spacing(10)
            .padding(Padding {
                top: 12.0,
                right: 12.0,
                bottom: 12.0,
                left: 16.0,
            })
            .width(300);

        if !q.is_empty() {
            for hit in nav::search_hits(&self.search) {
                let parent = hit
                    .parent
                    .and_then(nav::get)
                    .map(|p| p.title)
                    .unwrap_or("");
                let caption = if parent.is_empty() {
                    hit.caption.to_string()
                } else {
                    format!("{parent} > {}", hit.title)
                };
                nav_col = nav_col.push(widgets::nav_card(
                    palette,
                    hit.icon,
                    hit.title,
                    caption,
                    Message::Navigate(hit.key.into()),
                ));
            }
        } else {
            for cat in nav::CATEGORIES {
                if cat.key == "advanced" {
                    // Hairline above Advanced (always last in left nav).
                    let rule = Color::from_rgba(
                        if palette.dark { 1.0 } else { 0.0 },
                        if palette.dark { 1.0 } else { 0.0 },
                        if palette.dark { 1.0 } else { 0.0 },
                        0.14,
                    );
                    nav_col = nav_col.push(
                        column![
                            Space::with_height(10),
                            container(Space::new(Fill, 1)).width(Fill).style(move |_| {
                                ContainerStyle {
                                    background: Some(Background::Color(rule)),
                                    ..Default::default()
                                }
                            }),
                            Space::with_height(10),
                        ]
                        .padding(Padding {
                            top: 0.0,
                            right: 10.0,
                            bottom: 0.0,
                            left: 10.0,
                        }),
                    );
                }
                nav_col = nav_col.push(nav_btn(
                    palette,
                    cat.icon,
                    cat.title,
                    cat.key.to_string(),
                    category,
                ));
            }
        }

        let body = pages::view(self);
        let content = column![
            scrollable(body).height(Fill).width(Fill),
            widgets::status_toast(palette, &self.status),
        ]
        .height(Fill);

        let nav_panel = container(scrollable(nav_col)).height(Fill).style(move |_theme| {
            ContainerStyle {
                background: Some(Background::Color(canvas)),
                border: Border {
                    radius: 0.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

        container(
            row![nav_panel, content].height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .style(move |_theme| ContainerStyle {
            background: Some(Background::Color(canvas)),
            ..Default::default()
        })
        .into()
    }
}
