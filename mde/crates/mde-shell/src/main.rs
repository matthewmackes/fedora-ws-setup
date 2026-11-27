//! mde-shell — one process for taskbar, Start, flyouts, tray, notifications.
//! The bar is a bottom wlr-layer-shell panel. Flyouts, the soft lock, and the OSD
//! use a second layer surface above it.
pub(crate) mod agenda;
pub(crate) mod apps;
pub(crate) mod recent;
pub(crate) mod calendar;
pub(crate) mod clipboard;
pub(crate) mod widgets;
pub(crate) mod files;
pub(crate) mod icons;
pub(crate) mod idle;
pub(crate) mod keyboard;
pub(crate) mod notifications;
pub(crate) mod notify_bus;
pub(crate) mod osd;
pub(crate) mod sni;
pub(crate) mod system;
pub(crate) mod tray;
mod tools;
mod core;
mod taskbar;
mod start;
mod quick;
mod notify_view;
mod calendar_view;
mod widgets_view;
mod alttab;
mod taskview;
mod snap_view;
mod menus;
mod tray_view;
mod osd_view;
mod screencast;
mod screenshot;
mod snip_view;
mod connect_view;
mod shortcut_guide;
mod emoji_view;
mod kdeconnect;
mod dictation;
pub(crate) use osd_view::view_osd;

pub(crate) use clipboard::ClipboardHistory;
pub(crate) use iced::widget::container::Style as ContainerStyle;
pub(crate) use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, stack, text, text_input, Space,
};
pub(crate) use iced::{Background, Border, Color, Element, Fill, Subscription, Task, Theme};
pub(crate) use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
pub(crate) use iced_layershell::settings::{LayerShellSettings, Settings};
pub(crate) use iced_layershell::to_layer_message;
pub(crate) use iced_layershell::MultiApplication;
pub(crate) use idle::{IdleLock, LockPhase};
pub(crate) use mde_config::MdeConfig;
pub(crate) use mde_theme::Palette;
pub(crate) use notifications::NotificationCentre;
pub(crate) use notify_bus::NotifyHub;
pub(crate) use osd::OsdState;
pub(crate) use std::cell::Cell;
pub(crate) use std::collections::{HashMap, HashSet};
pub(crate) use std::time::{Duration, Instant, SystemTime};
pub(crate) use tray::TrayHost;

pub(crate) fn shell_started() -> Instant {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *START.get_or_init(Instant::now)
}


/// Pointer X tracking for context-menu anchoring (updated on CursorMoved without a Message).
mod pointer_x {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    static X: AtomicU32 = AtomicU32::new(0);
    static MAX: AtomicU32 = AtomicU32::new(0);
    static SEEN: AtomicBool = AtomicBool::new(false);

    pub fn note(x: f32) {
        if !x.is_finite() || x < 0.0 {
            return;
        }
        X.store(x.to_bits(), Ordering::Relaxed);
        if !SEEN.swap(true, Ordering::Relaxed) {
            MAX.store(x.to_bits(), Ordering::Relaxed);
            return;
        }
        let prev = f32::from_bits(MAX.load(Ordering::Relaxed));
        if x > prev {
            MAX.store(x.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn last() -> Option<f32> {
        if !SEEN.load(Ordering::Relaxed) {
            return None;
        }
        let x = f32::from_bits(X.load(Ordering::Relaxed));
        x.is_finite().then_some(x)
    }

    pub fn max_seen() -> Option<f32> {
        if !SEEN.load(Ordering::Relaxed) {
            return None;
        }
        let x = f32::from_bits(MAX.load(Ordering::Relaxed));
        x.is_finite().then_some(x)
    }
}

/// Idle reset for high-frequency pointer events without rebuilding chrome every tick.
fn throttled_user_activity() -> Option<Message> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_MS: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let prev = LAST_MS.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < 750 {
        return None;
    }
    LAST_MS.store(now, Ordering::Relaxed);
    Some(Message::UserActivity)
}

pub(crate) fn lock_flag_path() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(runtime).join("mde-locked")
}

pub(crate) fn write_lock_flag() {
    let _ = std::fs::write(lock_flag_path(), b"1\n");
}

pub(crate) fn clear_lock_flag() {
    let _ = std::fs::remove_file(lock_flag_path());
}

pub(crate) fn mde_lock_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "mde-lock"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Prefer `~/.local/bin/mde-lock` over stale `/usr/bin` copies.
pub(crate) fn mde_lock_bin() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let local = std::path::PathBuf::from(home).join(".local/bin/mde-lock");
        if local.is_file() {
            return local;
        }
    }
    std::path::PathBuf::from("mde-lock")
}

fn main() -> Result<(), iced_layershell::Error> {
    mde_config::supervisor::install_panic_hook();
    let _started = shell_started();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let (autohide, bar_h) = MdeConfig::load()
        .ok()
        .map(|c| {
            let shell = &c.active().shell;
            let h = if shell.taskbar_size.eq_ignore_ascii_case("compact") {
                Shell::BAR_H_COMPACT
            } else {
                Shell::BAR_H_COMFORTABLE
            };
            (shell.taskbar_autohide, h)
        })
        .unwrap_or((false, Shell::BAR_H_COMFORTABLE));
    Shell::run(Settings {
        layer_settings: LayerShellSettings {
            // Auto-hide starts as a thin edge hit target with no exclusive zone.
            // Position is bottom-only forever (A006).
            size: Some((0, if autohide { Shell::BAR_HIT } else { bar_h })),
            exclusive_zone: if autohide { 0 } else { bar_h as i32 },
            anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
            // A009: never steal keyboard focus from fullscreen / focused clients.
            keyboard_interactivity: KeyboardInteractivity::None,
            ..Default::default()
        },
        fonts: {
            let cfg = mde_config::MdeConfig::load().unwrap_or_default();
            let ui = mde_theme::ui_font(&cfg.active().theme);
            let mono = mde_theme::mono_font(&cfg.active().theme);
            mde_theme::font_bytes_selected(ui, mono)
                .into_iter()
                .map(std::borrow::Cow::from)
                .collect()
        },
        default_font: {
            let cfg = mde_config::MdeConfig::load().unwrap_or_default();
            let ui = mde_theme::ui_font(&cfg.active().theme).to_string();
            iced::Font::with_name(Box::leak(ui.into_boxed_str()))
        },
        ..Default::default()
    })
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub(crate) enum Message {
    ToggleStart,
    /// Right-click the taskbar background or clock.
    OpenTaskbarMenu,
    /// Right-click Start → Win+X style quick link menu.
    ToggleQuickLink,
    /// Taskbar menu → system monitor.
    OpenTaskManager,
    /// Taskbar menu → Shortcut Guide overlay.
    OpenShortcutGuide,
    ToggleQuick,
    ToggleNotifications,
    ToggleClipboard,
    /// Open Settings; optional page id (`network`, `sound`, …) passed to `mde-control`.
    OpenControl(String),
    /// Search pill: always open Start and focus the search box (W70).
    OpenStartSearch,
    /// Taskbar Task View button (Super+Tab equivalent).
    ToggleTaskView,
    /// Combined taskbar app button: launch / focus / cycle windows.
    ActivateTaskbarApp {
        pin: Option<String>,
        indices: Vec<usize>,
    },
    /// Auto-hide: pointer entered the taskbar surface.
    BarHoverEnter,
    /// Auto-hide: pointer left the taskbar surface.
    BarHoverExit,
    /// Pin drag-reorder: pointer entered a pinned icon.
    PinBarEnter(String),
    /// Start pin drag-reorder (W452).
    StartPinEnter(String),
    StartPinExit,
    /// Pin drag-reorder: pointer left a pinned icon.
    PinBarExit,
    /// Global left button down (pin drag / click-activate).
    PinGlobalDown,
    /// Global left button up (finish pin drag or activate).
    PinGlobalUp,
    /// Inactive QS stubs (Nearby Share, etc.).
    QsStub(&'static str),
    /// W449 Connect / Wireless display flyout.
    ToggleConnect,
    ConnectSelectOutput(String),
    ConnectToggleAudio,
    StartShare,
    StopShare,
    ShareStarted(Result<String, String>),
    ProjectMode(connect_view::ProjectMode),
    DemoNotify,
    DismissNotif(u32),
    NotifAction(u32, String),
    ClearNotifs,
    /// Restore notifications after Clear all (W57).
    UndoClearNotifs,
    Power(PowerAction),
    SetWifi(bool),
    SetBt(bool),
    SetAirplane(bool),
    VolUp,
    VolDown,
    VolMute,
    BrightUp,
    BrightDown,
    PollBus,
    RefreshRadios,
    LaunchPin(String),
    ToggleTray,
    ToggleKeyboard,
    ToggleShift,
    ToggleSymbols,
    TypeKey(String),
    ActivateTray(String),
    TrayMenu(String),
    TraySecondary(String),
    TrayMenuClick(String, i32),
    PasteClip(u64),
    Unlock,
    UserActivity,
    Dismiss,
    /// W515: snip overlay mode → capture.
    SnipCapture(mde_protocol::ScreenshotMode),
    /// W515: dismiss snip overlay.
    SnipCancel,
    /// W515: async capture finished.
    ScreenshotCaptured(screenshot::CaptureOutcome),
    /// W515: click thumbnail toast → open editor.
    ScreenshotToastOpen,
    StartQuery(String),
    OpenFile(String),
    ClearClip,
    PinClip(u64),
    FocusWindow(usize),
    ToggleDnd,
    ShowDesktop,
    /// Enter confirms Alt+Tab toast / closes Task View (does not dismiss Start).
    ConfirmSwitch,
    CalPrev,
    CalNext,
    CalSelect(u32),
    ToggleWidgets,
    RefreshWeather,
    RefreshNews,
    RefreshMarkets,
    RefreshTraffic,
    OpenNewsLink(String),
    /// Start → All apps list (W11).
    ToggleAllApps,
    AllAppsBack,
    /// Snap layout picker selection (W14).
    SnapLayout(String),
    /// Task View: switch compositor virtual desktop.
    SwitchDesktop(usize),
    /// Task View: create a new virtual desktop.
    NewDesktop,
    /// Task View: close/merge current desktop.
    CloseDesktop,
    /// Move window (global index) to desktop N.
    MoveWindowToDesktop { window: usize, desktop: usize },
    /// Taskbar +N overflow list of pins beyond the first 8.
    TogglePinOverflow,
    /// Taskbar +N overflow for running windows beyond the first 4 (W56).
    ToggleWinOverflow,
    /// Unpin an id from shell.pins (taskbar context).
    UnpinTaskbar(String),
    /// Unpin an id from start.pins (W383).
    UnpinStart(String),
    /// Move Start pin/folder into Settings › Advanced (hides from Start grid).
    MoveToAdvanced(String),
    /// W453: wrap pin in a Start folder.
    CreateStartFolder(String),
    DissolveStartFolder(String),
    ToggleStartFolderExpand(String),
    /// Pin an id to start.pins (W384/W385).
    PinToStart(String),
    /// Taskbar pin jump list (Unpin + recent files).
    OpenJumpList(String),
    JumpListOpenFile(String),
    /// Dismiss a recent jump-list path (W381).
    JumpListUnpinRecent(String),
    /// Close every window index in the list (W381/W382).
    EndTasks(Vec<usize>),
    /// Start / All apps / Recommended context menu (W383–W385).
    OpenAppCtx { kind: AppCtxKind, id: String },
    /// Empty desktop context menu (W388).
    OpenDesktopMenu,
    /// SSD title-bar system menu (W389).
    OpenSysMenu,
    /// Desktop View · Large/Medium/Small icons (W417).
    DesktopIconSize(&'static str),
    /// Desktop Sort · Name / Date modified (W418).
    DesktopSortBy(&'static str),
    /// Desktop New · Folder (W419).
    DesktopNewFolder,
    /// Desktop New · Shortcut (W419).
    DesktopNewShortcut,
    /// Desktop Refresh — reload recents / status.
    DesktopMenuRefresh,
    /// Taskbar menu · Lock / Unlock the taskbar (W420).
    ToggleTaskbarLock,
    /// App / Recommended · Uninstall confirm prompt (W421).
    UninstallPrompt,
    ConfirmUninstall,
    CancelUninstall,
    /// Reorder start.pins (↑ = -1, ↓ = +1).
    ReorderStartPin { id: String, delta: i32 },
    /// Compact volume slider flyout (W27).
    ToggleVolumeFlyout,
    /// Network quick-connect flyout (W28).
    ToggleNetworkFlyout,
    RefreshWifi,
    ConnectWifi(String),
    /// QS VPN flyout (W397).
    ToggleVpnFlyout,
    RefreshVpn,
    VpnConnect(String),
    VpnDisconnect(String),
    /// QS Mobile hotspot flyout (W396).
    ToggleHotspotFlyout,
    RefreshHotspot,
    HotspotStart,
    HotspotStop,
    DismissToast,
    WindowAction(String),
    OpenBackground,
    ToggleNightLight,
    /// Snap assist: focus window then snap opposite half (W24).
    SnapAssistPick(usize),
    /// Confirm Restart / Shut down after Start prompt (W46).
    ConfirmPower,
    CancelPowerConfirm,
    /// Start File Explorer thin launcher (W50).
    OpenFileExplorer,
    /// Start folder pins: Documents / Pictures / Downloads (W60).
    OpenUserFolder(String),
    /// QS dark/light theme toggle (W53).
    ToggleThemeDark,
    /// QS media transport (W49).
    MediaPlayPause,
    MediaPrevious,
    MediaNext,
    MediaVolUp,
    MediaVolDown,
    /// Start safe-mode affordance (W54).
    RequestSafeMode,
    /// Taskbar icon hover → status text (W77).
    HoverStatus(String),
    /// Running-window hover → truncated status + peek stub (W117).
    HoverPeek(String),
    /// Start → copy config.toml path (W83).
    ExportConfig,
    /// QS Accessibility / magnifier stub (W78).
    AccessibilityStub,
    /// Dismiss a Start Recommended item (W85).
    DismissRecommended(String),
    /// Focus assist priority-only mode (W86).
    TogglePriority,
    /// QS battery detailed flyout (W88).
    ToggleBatteryFlyout,
    /// Clipboard clear Yes/No confirm (W89).
    ConfirmClearClip,
    CancelClearClip,
    /// Toggle taskbar date format (W93).
    ToggleClockDateFormat,
    /// Taskbar menu: show or hide clock seconds.
    ToggleClockSeconds,
    /// Taskbar menu: ISO date vs month and day.
    ToggleClockIso,
    /// Exit chrome cleanly so the session respawns it (W94).
    RestartShell,
    /// Mute/unmute an app in Notification Center (W105).
    MuteNotifApp(String),
    UnmuteNotifApp(String),
    /// Reorder shell.pins on taskbar overflow (↑ = -1, ↓ = +1) (W106).
    ReorderShellPin { id: String, delta: i32 },
    /// QS Auto brightness stub (W107).
    ToggleAutoBright,
    /// QS/Start Project — opens Connect flyout (W449; was stub W110).
    #[allow(dead_code)]
    ProjectStub,
    /// Open session.log from Start About (W114).
    OpenSessionLog,
    /// Start search → open DuckDuckGo for the query (W95).
    SearchWeb,
    /// Taskbar middle-click: focus then close window (W97).
    CloseWindow(usize),
    /// Enter / leave Quick Settings tile edit mode (W98).
    EditQsTiles,
    /// Finish editing QS tiles.
    QsEditDone,
    /// Add a catalog tile while editing QS.
    QsAddTile(String),
    /// Remove a pinned QS tile while editing.
    QsRemoveTile(String),
    /// Clipboard history filter (W103).
    ClipQuery(String),
    /// Start power “Update & restart” stub (W104).
    UpdateAndRestart,
    /// QS App volume mixer — per-app PipeWire streams (W399).
    OpenAppMixer,
    /// Per-app stream Vol ± (W399).
    AppStreamVol(u32, i32),
    /// Per-app stream mute toggle (W399).
    AppStreamMute(u32),
    /// Clipboard paste as plain text (W123).
    PasteClipPlain(u64),
    /// Start Pinned section collapse toggle (W124).
    TogglePinnedCollapse,
    /// Start footer power button.
    TogglePowerMenu,
    /// Start footer account button.
    ToggleAccountMenu,
    /// Start Recommended “More” — extra lists, not the home grid.
    ToggleStartMore,
    /// Notification snooze 15m / 1h (W125).
    SnoozeNotif { id: u32, secs: u64 },
    /// Taskbar auto-hide toggle stub (W126).
    ToggleAutoHide,
    /// Taskbar menu: show/hide Widgets button (W387).
    ToggleWidgetsBtnFromMenu,
    /// Alt+Space → move focused window to desktop N (W131).
    WinMenuMoveDesktop(usize),
    /// Start Recommended clear all via dismiss.json (W133).
    ClearRecommended,
    /// Safely remove USB tip stub (W134).
    SafeRemoveUsb,
    /// Toast body expand/collapse (W136).
    ToggleToastExpand,
    /// Super+R Run dialog input / submit (W141).
    RunQuery(String),
    RunSubmit,
    /// Start All apps A–Z jump letter (W144).
    AllAppsJump(char),
    /// QS/NC notification sounds toggle (W145).
    ToggleNotifSounds,
    /// QS taskbar left/center alignment (W146).
    ToggleTaskbarLeft,
    /// About crash reports tip (W154).
    OpenCrashReports,
    /// NC group header → elevate app to priority-apps.txt (W156).
    ElevateNotifPriority(String),
    /// Super+X quick link row (W161).
    QuickLink(&'static str),
    /// Clipboard cloud sync pause (W163).
    ToggleClipSyncPause,
    /// Start accent swatch (W164).
    SetThemeAccent(String),
    /// NC group header Clear — only that app_name (W165).
    ClearNotifGroup(String),
    /// QS Hidden icons → tray overflow manage UI (W398 / W266).
    OpenHiddenIcons,
    /// Persist hide/show for an SNI tray id (W398).
    ToggleTrayHidden(String),
    /// QS / battery flyout battery saver toggle (W400).
    ToggleBatterySaver,
    /// A089: set power-profiles-daemon profile from battery flyout.
    SetPowerProfile(String),
    /// QS Focus timer — 25m session + DND (W404).
    ToggleFocusTimer,
    /// Taskbar mic mute toggle (W170).
    ToggleMicMute,
    /// Start Account “Sign in options” stub (W173).
    AccountSignInStub,
    /// W462: Switch user (same seat limits as lock W428).
    SwitchUser,
    /// About Feedback Hub stub (W174).
    FeedbackHubStub,
    /// NC group → app notification settings deep link stub (W185).
    NotifAppSettings(String),
    /// About → copy short diagnostics via wl-copy (W194).
    CopyDiagnostics,
    /// Clipboard “Paste as file” stub (W183).
    PasteClipAsFileStub,
    // A015–A042 community tools
    PaletteSelect(String),
    ToggleAwake,
    SetAwakeMinutes(u32),
    OpenBtFlyout,
    BtConnect(String),
    BtDisconnect(String),
    BtStartPair,
    BtPairDevice(String),
    WifiPasswordInput(String),
    WifiJoinWithPassword,
    WifiAskPassword(String),
    QsMoveTile(String, i32),
    JumpListPinForever(String),
    JumpListClearRecent,
    RestoreSnapGroup(String),
    SaveCurrentSnapGroup,
    ApplyZone(usize),
    OpenColorPickerHistory,
    CopyColorSample(String),
    FindMyMousePulse,
    ToggleMouseHighlighterLive,
    AdvancedPaste(u64, String),
    OpenSnapGroups,

    /// W195: Start search emoji tip → emoji panel.
    OpenEmojiPanel,
    InsertEmoji(String),
    EmojiOverflow,
    /// A068: emoji panel search box.
    EmojiQuery(String),
    /// W196: dismiss a toast by notification id (stack-aware).
    DismissToastId(u32),
    /// W197: Combine taskbar buttons Always/Never.
    ToggleTaskbarCombine,
    /// W203: ↑/↓ reorder among pinned clipboard items.
    ReorderPinnedClip { id: u64, delta: i32 },
    /// W204 / W264: Start “Add folder” → pin Documents path.
    NewStartFolderStub,
    /// W255: Start search type chips (Apps / Documents / Web).
    SetSearchType(String),
    /// W257: QS taskbar “Show labels” stub.
    ToggleTaskbarLabels,
    /// W205: adjust banner toast duration (±1s).
    ToastSecsDelta(i32),
    /// W206: End task for a running window index.
    EndTask(usize),
    /// W206 / W382: right-click running window jump list.
    OpenWinJump(usize),
    /// W208: Alt+Tab includes Desktop stub toggle.
    ToggleAlttabDesktop,
    /// W211: undo last snap via mde-window-action restore.
    UndoSnap,
    /// W213: Start power Update & shut down stub.
    UpdateAndShutDown,
    /// W214: About Reset this PC tip.
    ResetThisPcTip,
    /// W216: number keys 1/2 invoke toast actions while banner visible.
    ToastActionKey(usize),
    /// W217: show-desktop strip hover → delayed peek status.
    PeekDesktopEnter,
    PeekDesktopExit,
    /// W224: Start pin Small/Large resize stub (legacy).
    StartPinResizeStub,
    /// W410: set Start pin Small/Large metrics.
    SetStartPinSize { id: String, large: bool },
    /// W225: NC group header chevron expand/collapse.
    ToggleNotifGroup(String),
    /// W231: winmenu Center → mde-snap.req center.
    CenterWindow,
    /// W233: Start Tips → rotate tip status text.
    RotateTip,
    /// W234: About Support bundle → path tip + wl-copy.
    SupportBundleTip,
    /// W245: Notification Clear all Yes/No confirm.
    ConfirmClearNotifs,
    CancelClearNotifs,
    /// W246: Pin an All apps entry to shell.pins / taskbar.
    PinToTaskbar(String),
    /// W251: winmenu nudge focused window by 20px.
    NudgeWindow(&'static str),
    /// W253: Start Account change picture stub.
    AccountPictureStub,
    /// W254: About Clean temporary files tip.
    CleanTempTip,
    /// W236: dismiss entire toast stack.
    DismissAllToasts,
    /// W238: open classic volume mixer (pavucontrol/pwvucontrol).
    OpenVolumeMixer,
    /// W239: cycle Widgets photos card to next ~/Pictures image.
    CyclePhotos,
    /// W243: save image clip as PNG under ~/Pictures/Clipboard.
    SaveClipImage(u64),
    /// W244: Recommended → open parent folder / desktop location.
    OpenRecommendedLocation(String),
    /// W263: Clipboard share to Nearby Share stub.
    ClipShareNearbyStub,

    /// W395 Nearby Share flyout.
    ToggleNearbyFlyout,
    RefreshNearby,
    NearbyPair(String),
    NearbyUnpair(String),
    NearbyAcceptPair(String),
    NearbyPing(String),
    NearbyRing(String),
    NearbyShareClipboard(String),
    NearbyShareFile(String),
    NearbyShareText(String),
    NearbyMedia(String),
    NearbyLock(String),
    NearbySendKeys(String),
    NearbyPresenter(String),
    NearbyPickShare(String),
    /// W437: clipboard history → pick device then send.
    ClipShareNearbyPick,
    ClipShareNearbyTo(String),
    /// W266: SNI tray overflow chevron → list hidden names stub.
    TrayOverflowStub,
    /// W270: Network glyph hover → speed stub status.
    NetSpeedHover,
    /// W273: Start search mic → voice search stub.
    VoiceSearchStub,
    /// W274: About / QS energy recommendations tip.
    EnergyRecTip,
    /// W276: quiet hours preset `HH:MM-HH:MM` or empty to clear.
    QuietHoursPreset(String),
    /// W280: soft-lock / Widgets-style re-read of weather.cache.
    RefreshLockWeather,
    /// W283: clipboard history max items ± (extras clipboard.max_items).
    ClipMaxItemsDelta(i32),
    /// W284: Start pin Live tile stub.
    StartLiveTileStub,
}

/// Which Start-surface context menu is open (W383–W385).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppCtxKind {
    StartPin,
    AllApps,
    Recommended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppCtx {
    pub kind: AppCtxKind,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PowerAction {
    Lock,
    Suspend,
    Reboot,
    PowerOff,
    SignOut,
    Hibernate,
}

pub(crate) struct Shell {
    cfg: MdeConfig,
    palette: Palette,
    start_open: bool,
    /// W461: dedicated Search surface from taskbar.
    search_open: bool,
    quick_open: bool,
    /// Quick Settings “Edit” mode — add/remove tiles (W98).
    qs_edit: bool,
    notifications_open: bool,
    clipboard_open: bool,
    status: String,
    notifs: NotificationCentre,
    clipboard: ClipboardHistory,
    osd: Option<OsdState>,
    idle: IdleLock,
    wifi_on: bool,
    bt_on: bool,
    airplane: bool,
    hub: NotifyHub,
    tray: TrayHost,
    tray_open: bool,
    keyboard_open: bool,
    keyboard_shift: bool,
    keyboard_symbols: bool,
    config_mtime: Option<SystemTime>,
    idle_accum_ms: u32,
    start_query: String,
    file_hits: Vec<String>,
    apps: Vec<apps::AppEntry>,
    sni: sni::SniWatch,
    menu_board: sni::MenuBoard,
    tray_menu: Option<sni::TrayMenu>,
    overlay_id: Option<iced::window::Id>,
    overlay_frame: Option<FlyoutFrame>,
    pending_start_focus: bool,
    /// Brief Start search border flash after search-pill focus (W186).
    search_highlight_until: Option<Instant>,
    osd_until: Option<Instant>,
    clock: String,
    windows: Vec<WindowInfo>,
    idle_inhibited: bool,
    tab_accum_ms: u32,
    vol_level: f32,
    vol_muted: bool,
    /// Default source mute (pactl) for taskbar mic glyph (W170).
    mic_muted: bool,
    bright_level: f32,
    /// Child `mde-lock` process when ext-session-lock is engaged.
    lock_child: Option<std::process::Child>,
    /// Non-zero exits of `mde-lock` during this lock. A crash does not unlock.
    lock_deaths: u32,
    /// Task View overlay (Super+Tab).
    taskview_open: bool,
    /// Last Alt+Tab selection index into active-desktop windows.
    alttab_index: usize,
    /// Active virtual desktop (from compositor JSON).
    desktop_active: usize,
    /// Number of virtual desktops.
    desktop_count: usize,
    /// Brief Alt+Tab toast until this instant.
    alttab_until: Option<Instant>,
    /// Notification Center month calendar (Win11).
    calendar: calendar::MonthGrid,
    cal_selected: Option<u32>,
    /// Widgets Board (Win11-like left panel).
    widgets_open: bool,
    weather: widgets::WeatherCard,
    news_card: widgets::NewsCard,
    markets_card: widgets::MarketsCard,
    traffic_card: widgets::TrafficCard,

    system_card: widgets::SystemCard,
    /// Start menu “All apps” browse mode (W11).
    start_all_apps: bool,
    /// Super+Z snap layout picker (W14).
    snapflyout_open: bool,
    /// +N taskbar pin overflow flyout (W18).
    pin_overflow_open: bool,
    /// +N taskbar running-window overflow (W56).
    win_overflow_open: bool,
    /// Right-click pin jump list desktop id (W35 / W381).
    jump_list_pin: Option<String>,
    /// Right-click running window jump / End task (W206 / W382).
    jump_list_win: Option<usize>,
    /// Start / All apps / Recommended context menu (W383–W385).
    app_ctx: Option<AppCtx>,
    /// Empty desktop context menu (W388).
    desktop_menu_open: bool,
    /// SSD title-bar system menu (W389).
    sysmenu_open: bool,
    /// Half already filled ("left"|"right"); overlay lists windows for the other half (W24).
    snap_assist_side: Option<String>,
    /// Auto-dismiss snap assist after ~8s without click (W202).
    snap_assist_until: Option<Instant>,
    /// Compact volume flyout (W27).
    vol_flyout_open: bool,
    /// Network SSID flyout (W28).
    net_flyout_open: bool,
    wifi_aps: Vec<system::WifiAp>,
    /// QS VPN flyout (W397).
    vpn_flyout_open: bool,
    vpn_list: Vec<system::VpnConn>,
    /// QS Mobile hotspot flyout (W396).
    hotspot_flyout_open: bool,
    /// A023 Keep awake inhibit.
    awake_state: tools::AwakeState,
    /// A037 QS Bluetooth flyout.
    bt_flyout_open: bool,
    bt_devices: Vec<system::BtDevice>,
    bt_pairing: bool,
    /// A038 Wi-Fi password join.
    wifi_password_ssid: String,
    wifi_password: String,
    /// A019 color history flyout.
    color_history_open: bool,
    color_history: Vec<tools::ColorSample>,
    /// A015 palette hits cache.
    palette_hits: Vec<tools::PaletteHit>,
    /// A017 snap groups UI.
    snap_groups_open: bool,
    snap_groups: Vec<tools::SnapGroup>,
    /// A024 find-my-mouse overlay pulse remaining ticks.
    find_mouse_ticks: u32,
    mouse_highlighter_live: bool,

    /// W395 Nearby Share (KDE Connect).
    nearby_flyout_open: bool,
    nearby: kdeconnect::Status,
    /// W437 pending clip text to send after device pick.
    nearby_pending_clip: Option<String>,
    /// W424 voice typing session.
    dictation: dictation::Session,
    hotspot: system::HotspotInfo,
    /// QS per-app volume mixer flyout (W399).
    app_mixer_open: bool,
    app_streams: Vec<system::AudioNode>,
    /// W404 Focus timer end Instant + previous DND to restore.
    focus_timer_until: Option<Instant>,
    focus_timer_prev_dnd: Option<bool>,
    /// Alt+Space window actions menu (W32).
    winmenu_open: bool,
    /// Brief notification toast above the bar (W25).
    toast: Option<notifications::Notification>,
    toast_until: Option<Instant>,
    /// Toast shows full body when true (W136).
    toast_expanded: bool,
    /// Second stacked toast under the primary (W196, ≤2).
    toast_stack: Option<notifications::Notification>,
    toast_stack_until: Option<Instant>,
    /// Super+R Run dialog (W141).
    run_open: bool,
    run_query: String,
    /// Super+X quick link menu (W161).
    quicklink_open: bool,
    /// Right-click menu on the taskbar background.
    taskbar_menu_open: bool,
    /// Horizontal anchor (px) for the taskbar context menu — click X when opened.
    taskbar_menu_anchor_x: Option<f32>,
    /// All apps A–Z jump filter (W144); None = show all.
    all_apps_jump: Option<char>,
    /// Cleared notifications held for Undo toast (W57).
    cleared_stash: Option<Vec<notifications::Notification>>,
    clear_undo_until: Option<Instant>,
    /// Start Recommended cache (W26).
    recent: Vec<recent::RecentItem>,
    /// Taskbar tray battery glyph (None = no battery).
    battery_pct: Option<u8>,
    battery_charging: bool,
    /// Pending Restart/PowerOff confirmation in Start (W46).
    power_confirm: Option<PowerAction>,
    /// `playerctl` present — QS media row (W49).
    media_available: bool,
    media_title: String,
    media_playing: bool,
    /// Quiet-hours auto-DND already applied for the current window (W67).
    quiet_hours_applied: bool,
    /// Battery detail flyout (W88).
    bat_flyout_open: bool,
    /// A092: mic/camera/location in use (refreshed on poll).
    privacy_uses: Vec<(String, String)>,
    /// Clipboard Clear all confirmation row (W89).
    clip_clear_confirm: bool,
    /// Notification Clear all confirmation row (W245).
    notif_clear_confirm: bool,
    /// App context Uninstall Yes/No (W421).
    uninstall_confirm: bool,
    /// Clipboard flyout search filter (W103).
    clip_query: String,
    /// Start Pinned grid collapsed (session-only, W124).
    pins_collapsed: bool,
    /// Taskbar peek title for running-window hover (W117).
    peek_title: Option<String>,
    /// W215: last ≤3 Start calculator results (session).
    calc_history: Vec<String>,
    /// W217: show-desktop peek arms after ~400ms hover.
    peek_desktop_until: Option<Instant>,
    /// W225: NC app groups collapsed (session).
    nc_collapsed: HashSet<String>,
    /// W230: brief critical tint on volume glyph after mute.
    vol_mute_flash_until: Option<Instant>,
    /// W265: brief taskbar accent flash on critical toast.
    critical_led_until: Option<Instant>,
    /// W233: Start Tips rotation index.
    tip_idx: usize,
    /// W235: last ≤3 OpenControl page ids this session.
    recent_settings: Vec<String>,
    /// W239: current Widgets photos path (cycle target).
    photos_path: Option<String>,
    /// W255: Start search type filter (`""` | apps | documents | web).
    search_type: String,
    /// Start power button menu (Sleep / Shut down / Restart).
    power_menu_open: bool,
    /// Typed compositor IPC client (`mde.sock`).
    ipc: std::sync::Arc<mde_protocol::ShellClient>,
    /// Start account button menu.
    account_menu_open: bool,
    /// Recommended “More” reveals the extra Start lists.
    start_more: bool,
    /// W453: expanded Start folder pin id.
    start_folder_open: Option<String>,
    /// Main taskbar layer-shell window id (for SizeChange auto-hide).
    bar_id: Cell<Option<iced::window::Id>>,
    /// Pointer is over the taskbar (auto-hide reveal).
    bar_hover: bool,
    /// A009: defer auto-hide collapse after pointer leave (Win11-like grace).
    bar_hide_after: Option<Instant>,
    /// Pin currently under the pointer (drag-reorder).
    hover_pin: Option<String>,
    /// Left button held — pin drag / click-activate tracking.
    pin_down: bool,
    pin_drag_src: Option<String>,
    pin_drag_moved: bool,
    start_pin_drag_src: Option<String>,
    start_pin_drag_moved: bool,
    /// Start pin under pointer (drag only while `pin_down`).
    start_pin_hover: Option<String>,
    /// True after pointer leaves the press-down pin — avoids click/micro-slip reorders.
    start_pin_drag_armed: bool,
    /// Per-app cycle index for multi-window taskbar buttons.
    app_cycle: HashMap<String, usize>,
    /// W449 Connect / Wireless display.
    connect_open: bool,
    /// W426 first-class emoji picker flyout.
    emoji_open: bool,
    /// A073: PowerToys-style Shortcut Guide overlay.
    shortcut_guide_open: bool,
    /// A068: emoji search query.
    emoji_query: String,
    /// W515 Win11 snip overlay.
    snip_open: bool,
    /// W515 preselected mode when overlay opens.
    snip_preselect: Option<mde_protocol::ScreenshotMode>,
    /// W515 awaiting focused-window geometry from compositor.
    pending_window_capture: bool,
    /// W515 post-capture thumbnail toast.
    screenshot_toast: Option<snip_view::ScreenshotToast>,
    share_active: bool,
    share_include_audio: bool,
    share_output: Option<String>,
    outputs: Vec<mde_protocol::OutputInfo>,
}

/// One combined taskbar button (pinned and/or running).
#[derive(Debug, Clone)]
pub(crate) struct TaskbarAppEntry {
    pub pin: Option<String>,
    pub label: String,
    pub icon: String,
    pub indices: Vec<usize>,
    pub running: bool,
    pub progress: bool,
    /// A004: unread count from Notify `desktop-entry` / app_name protocol.
    pub unread: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowInfo {
    /// Stable compositor id when known (protocol / new JSON).
    id: Option<u64>,
    title: String,
    app_id: String,
    /// True when the client is fullscreen (A009 — do not steal focus).
    fullscreen: bool,
    /// 0-based virtual desktop index from compositor.
    workspace: usize,
}

impl MultiApplication for Shell {
    type Message = Message;
    type Flags = ();
    type Theme = Theme;
    type Executor = iced::executor::Default;

    fn new(_flags: ()) -> (Self, Task<Message>) {
        let cfg = MdeConfig::load().unwrap_or_default();
        let palette = Palette::from_theme(&cfg.active().theme);
        let p = cfg.active().clone();
        let mut notifs = NotificationCentre::default();
        notifs.dnd = p.notifications.dnd;
        notifs.priority = p.notifications.priority;
        notifs.priority_apps = notifications::load_priority_apps(&p.shell.pins);
        notifs.muted_apps = notifications::load_muted_apps();
        notifs.load_persisted();
        let hub = NotifyHub::default();
        hub.set_dnd(p.notifications.dnd);
        hub.set_priority(notifs.priority, notifs.priority_apps.clone());
        hub.set_muted_apps(notifs.muted_apps.clone());
        let radios = system::probe_radios();
        let hub_bg = hub.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            if let Ok(rt) = rt {
                rt.block_on(notify_bus::serve(hub_bg));
            }
        });
        let sni = sni::SniWatch::default();
        let sni_bg = sni.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            if let Ok(rt) = rt {
                rt.block_on(sni::serve(sni_bg));
            }
        });
        let mut tray = TrayHost::default();
        tray.upsert(tray::TrayItem {
            id: "mde-shell".into(),
            title: "MDE".into(),
            status: "Active".into(),
            icon: String::new(),
            icon_path: String::new(),
            bus: String::new(),
            path: String::new(),
        });
        let config_mtime = MdeConfig::config_path()
            .ok()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        let mut shell = Self {
                status: format!(
                    "MDE Shell {} · layout={} · pins={}",
                    mde_theme::VERSION_FULL,
                    p.compositor.layout,
                    p.shell.pins.len()
                ),
                clipboard: ClipboardHistory::with_sync(
                    p.clipboard.history,
                    clip_max_from_cfg(&cfg),
                    p.clipboard.cross_device,
                    if p.clipboard.sync_dir.trim().is_empty() {
                        None
                    } else {
                        Some(std::path::PathBuf::from(p.clipboard.sync_dir.trim()))
                    },
                ),
                idle: IdleLock::new(if std::env::var_os("MDE_SAFE").is_some() {
                    86_400
                } else {
                    p.session.idle_lock_seconds
                }),
                wifi_on: radios.wifi,
                bt_on: radios.bluetooth,
                airplane: radios.airplane,
                cfg,
                palette,
                start_open: false,
                search_open: false,
                quick_open: false,
                qs_edit: false,
                notifications_open: false,
                clipboard_open: false,
                notifs,
                osd: None,
                hub,
                tray,
                tray_open: false,
                keyboard_open: false,
                keyboard_shift: false,
                keyboard_symbols: false,
                config_mtime,
                idle_accum_ms: 0,
                start_query: String::new(),
                file_hits: Vec::new(),
                apps: apps::list_apps(400),
                sni,
                menu_board: sni::MenuBoard::default(),
                tray_menu: None,
                overlay_id: None,
                overlay_frame: None,
                pending_start_focus: false,
                search_highlight_until: None,
                osd_until: None,
                clock: clock_now_cfg(p.shell.clock_date_iso, p.shell.clock_seconds),
                windows: Vec::new(),
                idle_inhibited: false,
                tab_accum_ms: 0,
                vol_level: system::volume_level().unwrap_or(0.5),
                vol_muted: system::volume_muted().unwrap_or(false),
                mic_muted: system::mic_muted().unwrap_or(false),
                bright_level: system::brightness_level().unwrap_or(0.7),
                lock_child: None,
                lock_deaths: 0,
                taskview_open: false,
                alttab_index: 0,
                desktop_active: 0,
                desktop_count: 2,
                alttab_until: None,
                calendar: calendar::MonthGrid::current(),
                cal_selected: None,
                widgets_open: false,
                news_card: widgets::NewsCard::default(),
                markets_card: widgets::MarketsCard::default(),
                traffic_card: widgets::TrafficCard::default(),
                weather: widgets::WeatherCard::default(),
                system_card: {
                    let mut s = widgets::SystemCard::default();
                    s.refresh();
                    s
                },
                start_all_apps: false,
                snapflyout_open: false,
                pin_overflow_open: false,
                win_overflow_open: false,
                jump_list_pin: None,
                jump_list_win: None,
                app_ctx: None,
                desktop_menu_open: false,
                sysmenu_open: false,
                snap_assist_side: None,
                snap_assist_until: None,
                vol_flyout_open: false,
                bat_flyout_open: false,
                privacy_uses: Vec::new(),
                net_flyout_open: false,
                wifi_aps: Vec::new(),
                vpn_flyout_open: false,
                vpn_list: Vec::new(),
                hotspot_flyout_open: false,
                awake_state: tools::load_awake(),
                bt_flyout_open: false,
                bt_devices: Vec::new(),
                bt_pairing: false,
                wifi_password_ssid: String::new(),
                wifi_password: String::new(),
                color_history_open: false,
                color_history: tools::load_color_history(12),
                palette_hits: Vec::new(),
                snap_groups_open: false,
                snap_groups: tools::load_snap_groups(),
                find_mouse_ticks: 0,
                mouse_highlighter_live: false,

                nearby_flyout_open: false,
                nearby: kdeconnect::Status::default(),
                nearby_pending_clip: None,
                dictation: dictation::Session::default(),
                hotspot: system::HotspotInfo::default(),
                app_mixer_open: false,
                app_streams: Vec::new(),
                focus_timer_until: None,
                focus_timer_prev_dnd: None,
                winmenu_open: false,
                toast: None,
                toast_until: None,
                toast_expanded: false,
                toast_stack: None,
                toast_stack_until: None,
                run_open: false,
                run_query: String::new(),
                quicklink_open: false,
                taskbar_menu_open: false,
                taskbar_menu_anchor_x: None,
                all_apps_jump: None,
                cleared_stash: None,
                clear_undo_until: None,
                clip_clear_confirm: false,
                uninstall_confirm: false,
                notif_clear_confirm: false,
                clip_query: String::new(),
                recent: recent::load(8),
                battery_pct: system::battery_capacity(),
                battery_charging: system::battery_charging().unwrap_or(false),
                power_confirm: None,
                media_available: system::playerctl_available(),
                media_title: String::new(),
                media_playing: false,
                quiet_hours_applied: false,
                pins_collapsed: false,
                peek_title: None,
                calc_history: Vec::new(),
                peek_desktop_until: None,
                nc_collapsed: HashSet::new(),
                vol_mute_flash_until: None,
                critical_led_until: None,
                tip_idx: 0,
                recent_settings: Vec::new(),
                photos_path: widgets::random_picture(),
                search_type: String::new(),
                power_menu_open: false,
                ipc: mde_protocol::ShellClient::connect(),
                account_menu_open: false,
                start_more: false,
                start_folder_open: None,
                bar_id: Cell::new(None),
                bar_hover: false,
                bar_hide_after: None,
                hover_pin: None,
                pin_down: false,
                pin_drag_src: None,
                pin_drag_moved: false,
                start_pin_drag_src: None,
                start_pin_drag_moved: false,
                start_pin_hover: None,
                start_pin_drag_armed: false,
                app_cycle: HashMap::new(),
                connect_open: false,
                emoji_open: false,
                shortcut_guide_open: false,
                emoji_query: String::new(),
                snip_open: false,
                snip_preselect: None,
                pending_window_capture: false,
                screenshot_toast: None,
                share_active: false,
                share_include_audio: false,
                share_output: None,
                outputs: load_outputs_snapshot(),
        };
        tracing::info!(
            elapsed_ms = shell_started().elapsed().as_millis() as u64,
            "mde-budget shell_ready"
        );
        if lock_flag_path().is_file() {
            // Re-assert lock after chrome restart (keeps seat locked across shell crashes).
            shell.engage_lock("shell-restart");
        } else if mde_lock_running() {
            // Orphan mde-lock with no flag → black locked seat. Release it.
            write_unlock_req();
            let _ = std::process::Command::new("pkill")
                .args(["-TERM", "-x", "mde-lock"])
                .status();
            shell.status = "Cleared orphan mde-lock".into();
        }
        (shell, Task::none())
    }

    fn namespace(&self) -> String {
        "mde-shell".into()
    }

    fn theme(&self) -> Theme {
        mde_theme::iced_theme(&self.palette)
    }

    fn style(&self, theme: &Self::Theme) -> iced_layershell::Appearance {
        // The window itself stays clear. The bar and each flyout paint their own glass,
        // so menu corners can be rounded instead of a full-width slab.
        iced_layershell::Appearance {
            background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            text_color: theme.palette().text,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(400)).map(|_| Message::PollBus),
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                    match &key {
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                            Some(Message::Dismiss)
                        }
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                            Some(Message::ConfirmSwitch)
                        }
                        // W216: 1/2 → first/second toast action while banner visible.
                        iced::keyboard::Key::Character(c) if c.as_str() == "1" => {
                            Some(Message::ToastActionKey(0))
                        }
                        iced::keyboard::Key::Character(c) if c.as_str() == "2" => {
                            Some(Message::ToastActionKey(1))
                        }
                        _ => Some(Message::UserActivity),
                    }
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Some(Message::PinGlobalDown),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::PinGlobalUp),
                // Cursor/wheel/finger-move used to emit UserActivity on every event,
                // forcing a full Shell::view rebuild while scrolling menus (and
                // re-resolving every icon). Throttle idle pulses; scrollable still
                // updates its own offset without an application message.
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    pointer_x::note(position.x);
                    throttled_user_activity()
                }
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. })
                | iced::Event::Touch(iced::touch::Event::FingerMoved { .. }) => {
                    throttled_user_activity()
                }
                iced::Event::Mouse(_) | iced::Event::Touch(_) => Some(Message::UserActivity),
                _ => None,
            }),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        self.update_inner(message)
    }

    fn view(&self, id: iced::window::Id) -> Element<'_, Message> {
        if self.overlay_id == Some(id) {
            self.view_overlay()
        } else {
            self.bar_id.set(Some(id));
            self.view_bar()
        }
    }

    fn remove_id(&mut self, id: iced::window::Id) {
        if self.overlay_id == Some(id) {
            self.overlay_id = None;
            self.overlay_frame = None;
        }
        if self.bar_id.get() == Some(id) {
            self.bar_id.set(None);
        }
    }
}

impl Shell {

    const BAR_H_COMFORTABLE: u32 = 48;
    const BAR_H_COMPACT: u32 = 36;
    const BAR_HIT: u32 = 2;

    /// A008: Comfortable (48) or Compact (36) taskbar height.
    fn bar_height(&self) -> u32 {
        if self
            .cfg
            .active()
            .shell
            .taskbar_size
            .eq_ignore_ascii_case("compact")
        {
            Self::BAR_H_COMPACT
        } else {
            Self::BAR_H_COMFORTABLE
        }
    }

    fn close_flyouts(&mut self) {
        self.start_open = false;
        self.search_open = false;
        self.quick_open = false;
        self.qs_edit = false;
        self.notifications_open = false;
        self.clipboard_open = false;
        self.tray_open = false;
        self.keyboard_open = false;
        self.taskview_open = false;
        self.alttab_until = None;
        self.widgets_open = false;
        self.start_all_apps = false;
        self.start_folder_open = None;
        self.start_pin_hover = None;
        self.start_pin_drag_src = None;
        self.start_pin_drag_moved = false;
        self.start_pin_drag_armed = false;
        self.snapflyout_open = false;
        self.pin_overflow_open = false;
        self.win_overflow_open = false;
        self.jump_list_pin = None;
        self.jump_list_win = None;
        self.app_ctx = None;
        self.desktop_menu_open = false;
        self.sysmenu_open = false;
        self.snap_assist_side = None;
        self.snap_assist_until = None;
        self.vol_flyout_open = false;
        self.bat_flyout_open = false;
        self.net_flyout_open = false;
        self.vpn_flyout_open = false;
        self.hotspot_flyout_open = false;
        self.bt_flyout_open = false;
        self.color_history_open = false;
        self.snap_groups_open = false;
        self.wifi_password_ssid.clear();
        self.wifi_password.clear();

        self.nearby_flyout_open = false;
        self.app_mixer_open = false;
        self.winmenu_open = false;
        self.power_confirm = None;
        self.clip_clear_confirm = false;
        self.notif_clear_confirm = false;
        self.uninstall_confirm = false;
        self.clip_query.clear();
        self.peek_title = None;
        self.peek_desktop_until = None;
        self.run_open = false;
        self.run_query.clear();
        self.quicklink_open = false;
        self.taskbar_menu_open = false;
        self.taskbar_menu_anchor_x = None;
        self.all_apps_jump = None;
        self.power_menu_open = false;
        self.account_menu_open = false;
        self.start_more = false;
        self.connect_open = false;
        self.emoji_open = false;
        self.shortcut_guide_open = false;
        self.emoji_query.clear();
        self.snip_open = false;
        self.snip_preselect = None;
        self.pending_window_capture = false;
    }

    /// Banner duration from extras `notifications.toast_secs` (default 5) (W205).
    fn toast_secs(&self) -> u64 {
        let from_extras = self
            .cfg
            .extras
            .get("notifications.toast_secs")
            .and_then(|v| {
                v.as_integer()
                    .map(|i| i as u64)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });
        from_extras
            .unwrap_or_else(|| self.cfg.active().notifications.toast_seconds)
            .clamp(1, 30)
    }

    fn set_toast_secs(&mut self, secs: u64) {
        let secs = secs.clamp(1, 30);
        self.cfg.extras.insert(
            "notifications.toast_secs".into(),
            toml::Value::Integer(secs as i64),
        );
        self.cfg.active_mut().notifications.toast_seconds = secs;
        let _ = self.cfg.save();
    }

    /// W283: clipboard.max_items from extras (default 25) or profile.
    fn clip_max_items(&self) -> usize {
        clip_max_from_cfg(&self.cfg)
    }

    fn set_clip_max_items(&mut self, max: u32) {
        let max = max.clamp(5, 100);
        self.cfg.extras.insert(
            "clipboard.max_items".into(),
            toml::Value::Integer(max as i64),
        );
        self.cfg.active_mut().clipboard.max_items = max;
        self.clipboard.set_max_items(max as usize);
        let _ = self.cfg.save();
    }

    /// W276: apply quiet hours HH:MM–HH:MM preset (empty clears).
    fn set_quiet_hours_preset(&mut self, preset: &str) {
        let n = &mut self.cfg.active_mut().notifications;
        let preset = preset.trim();
        if preset.is_empty() || preset.eq_ignore_ascii_case("off") {
            n.quiet_from.clear();
            n.quiet_to.clear();
            self.quiet_hours_applied = false;
            let _ = self.cfg.save();
            self.status = "Quiet hours · Off".into();
            return;
        }
        let (from, to) = match preset.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => {
                self.status = "Quiet hours · bad preset".into();
                return;
            }
        };
        if from.is_empty() || to.is_empty() {
            self.status = "Quiet hours · bad preset".into();
            return;
        }
        n.quiet_from = from.into();
        n.quiet_to = to.into();
        self.quiet_hours_applied = false;
        let _ = self.cfg.save();
        self.status = format!("Quiet hours · {from}–{to}");
        self.apply_quiet_hours();
    }

    /// Toast card left/start align; extras `notifications.toast_left` (W256).
    fn toast_left(&self) -> bool {
        self.cfg
            .extras
            .get("notifications.toast_left")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Match taskbar pin cluster: left when `taskbar_left` or align is `"left"`.
    fn taskbar_left_align(&self) -> bool {
        taskbar_is_left_aligned(
            self.cfg.active().shell.taskbar_left,
            &self.cfg.active().shell.taskbar_align,
        )
    }

    /// A005: labels beside taskbar icons (`shell.taskbar_labels`, legacy extras fallback).
    fn taskbar_labels(&self) -> bool {
        if self.cfg.active().shell.taskbar_labels {
            return true;
        }
        self.cfg
            .extras
            .get("taskbar.show_labels")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// W417: `desktop.icon_size` — large / medium / small (default medium).
    fn desktop_icon_size_key(&self) -> &str {
        self.cfg
            .extras
            .get("desktop.icon_size")
            .and_then(|v| v.as_str())
            .filter(|s| matches!(*s, "large" | "medium" | "small"))
            .unwrap_or("medium")
    }

    /// Pixel size for Start Desktop entry faces (W417).
    fn desktop_icon_px(&self) -> u16 {
        match self.desktop_icon_size_key() {
            "large" => 32,
            "small" => 16,
            _ => 22,
        }
    }

    /// W418: `desktop.sort` — name (default) or date.
    fn desktop_sort_key(&self) -> &str {
        self.cfg
            .extras
            .get("desktop.sort")
            .and_then(|v| v.as_str())
            .filter(|s| matches!(*s, "name" | "date"))
            .unwrap_or("name")
    }

    /// W420: `shell.taskbar_locked` extras — blocks pin drag / reorder.
    fn taskbar_locked(&self) -> bool {
        self.cfg
            .extras
            .get("shell.taskbar_locked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// W255: empty filter shows all; otherwise only the named section.
    fn search_shows(&self, kind: &str) -> bool {
        self.search_type.is_empty() || self.search_type == kind
    }

    /// W255: Apps / Documents / Web chips when Start query is non-empty.
    fn pin_documents_folder(&mut self) -> String {
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => return "HOME unset".into(),
        };
        let docs = std::process::Command::new("xdg-user-dir")
            .arg("DOCUMENTS")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p)
                    }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| format!("{home}/Documents"));
        let pins = &mut self.cfg.active_mut().start.pins;
        if pins.iter().any(|p| p == &docs) {
            return format!("Already pinned · {docs}");
        }
        pins.push(docs.clone());
        let _ = self.cfg.save();
        format!("Pinned folder · {docs}")
    }

    /// Show a toast; if one is already visible, stack a second under it (W196, ≤2).
    fn push_toast(&mut self, n: notifications::Notification, sticky: bool) {
        // W265: flash taskbar accent briefly on critical urgency.
        if n.urgency >= 2 {
            self.critical_led_until = Some(Instant::now() + Duration::from_millis(900));
        }
        let until = if sticky {
            None
        } else {
            Some(Instant::now() + Duration::from_secs(self.toast_secs()))
        };
        if self.toast.as_ref().is_some_and(|t| t.id == n.id) {
            self.toast = Some(n);
            self.toast_until = until;
            return;
        }
        if self.toast_stack.as_ref().is_some_and(|t| t.id == n.id) {
            self.toast_stack = Some(n);
            self.toast_stack_until = until;
            return;
        }
        if self.toast.is_some() {
            self.toast_stack = Some(n);
            self.toast_stack_until = until;
        } else {
            self.toast = Some(n);
            self.toast_until = until;
            self.toast_expanded = false;
        }
    }

    fn dismiss_toast_id(&mut self, id: u32) {
        if self.toast.as_ref().is_some_and(|t| t.id == id) {
            self.toast = self.toast_stack.take();
            self.toast_until = self.toast_stack_until.take();
            self.toast_expanded = false;
        } else if self.toast_stack.as_ref().is_some_and(|t| t.id == id) {
            self.toast_stack = None;
            self.toast_stack_until = None;
        }
    }

    /// Soft lock + spawn `mde-lock` when available (ext-session-lock). Soft iced
    /// overlay remains the fallback if the lock client is missing.
    fn engage_lock(&mut self, reason: &str) {
        self.close_flyouts();
        // W449: stop screen share on lock.
        if self.share_active || screencast::is_active() {
            screencast::stop();
            self.share_active = false;
        }
        self.idle.lock_now();
        write_lock_flag();
        // W223: extras clipboard.clear_on_lock → drop unpinned clips on lock.
        if self.clipboard_clear_on_lock() {
            self.clipboard.clear_unpinned();
        }
        if self
            .lock_child
            .as_mut()
            .and_then(|c| c.try_wait().ok().flatten())
            .is_none()
            && self.lock_child.is_some()
        {
            self.status = format!("Session locked ({reason})");
            return;
        }
        self.lock_child = None;
        self.lock_deaths = 0;
        // Reap any stray mde-lock left by a prior shell (avoids black double-lock).
        if mde_lock_running() {
            let _ = std::process::Command::new("pkill")
                .args(["-TERM", "-x", "mde-lock"])
                .status();
            std::thread::sleep(Duration::from_millis(150));
            let _ = std::process::Command::new("pkill")
                .args(["-KILL", "-x", "mde-lock"])
                .status();
        }
        // W121: prefer mde-lock; fall back to loginctl lock-session; soft overlay last.
        match std::process::Command::new(mde_lock_bin()).spawn() {
            Ok(child) => {
                self.lock_child = Some(child);
                self.status = format!("Session locked via mde-lock ({reason})");
            }
            Err(_) => {
                let login_ok = std::process::Command::new("loginctl")
                    .arg("lock-session")
                    .spawn()
                    .is_ok();
                self.status = if login_ok {
                    format!("Session locked via loginctl ({reason})")
                } else {
                    format!(
                        "Soft locked ({reason} — mde-lock/loginctl unavailable; iced overlay fallback)"
                    )
                };
            }
        }
    }

    fn poll_lock_child(&mut self) {
        let Some(child) = self.lock_child.as_mut() else {
            return;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                self.lock_child = None;
                if self.idle.phase != LockPhase::Locked {
                    return;
                }
                use mde_config::supervisor::{lock_after_exit, lock_respawn_line, session_log, LockAction};
                match lock_after_exit(status.success(), self.lock_deaths) {
                    LockAction::Unlock => {
                        self.idle.unlock();
                        self.idle_accum_ms = 0;
                        self.lock_deaths = 0;
                        clear_lock_flag();
                        self.status = "Unlocked".into();
                    }
                    LockAction::Respawn { attempt } => {
                        self.lock_deaths = attempt;
                        session_log(&lock_respawn_line(attempt));
                        match std::process::Command::new(mde_lock_bin()).spawn() {
                            Ok(child) => {
                                self.lock_child = Some(child);
                                self.status = format!(
                                    "Session stays locked; mde-lock respawned ({attempt})"
                                );
                            }
                            Err(err) => {
                                self.status =
                                    format!("Session stays locked; mde-lock respawn failed: {err}");
                            }
                        }
                    }
                    LockAction::StayLocked => {
                        session_log("mde-lock respawn cap reached; seat stays locked");
                        self.status = "Session stays locked".into();
                    }
                }
            }
            Ok(None) => {}
            Err(_) => {
                self.lock_child = None;
                if self.idle.phase == LockPhase::Locked {
                    self.status = "Session stays locked".into();
                }
            }
        }
    }

    fn protocol_lock_active(&self) -> bool {
        self.lock_child.is_some()
    }

    fn flyout_open(&self) -> bool {
        self.start_open
            || self.quick_open
            || self.notifications_open
            || self.clipboard_open
            || self.tray_open
            || self.keyboard_open
            || self.taskview_open
            || self.alttab_until.is_some()
            || self.widgets_open
            || self.snapflyout_open
            || self.pin_overflow_open
            || self.win_overflow_open
            || self.jump_list_pin.is_some()
            || self.jump_list_win.is_some()
            || self.app_ctx.is_some()
            || self.desktop_menu_open
            || self.sysmenu_open
            || self.snap_assist_side.is_some()
            || self.vol_flyout_open
            || self.bat_flyout_open
            || self.net_flyout_open
            || self.vpn_flyout_open
            || self.hotspot_flyout_open
            || self.nearby_flyout_open
            || self.app_mixer_open
            || self.winmenu_open
            || self.run_open
            || self.quicklink_open
            || self.taskbar_menu_open
            || self.toast.is_some()
            || self.clear_undo_until.is_some()
            || self.connect_open
            || self.emoji_open
            || self.snip_open
            || self.screenshot_toast.is_some()
            || self.share_active
    }

    fn sync_overlay(&mut self) -> Task<Message> {
        let centered = !self.taskbar_left_align();
        let major = self.start_open
            || self.quick_open
            || self.notifications_open
            || self.clipboard_open
            || self.tray_open
            || self.keyboard_open
            || self.taskview_open
            || self.widgets_open
            || self.snapflyout_open
            || self.snap_assist_side.is_some()
            || self.pin_overflow_open
            || self.win_overflow_open
            || self.jump_list_pin.is_some()
            || self.jump_list_win.is_some()
            || self.app_ctx.is_some()
            || self.desktop_menu_open
            || self.sysmenu_open
            || self.vol_flyout_open
            || self.bat_flyout_open
            || self.net_flyout_open
            || self.vpn_flyout_open
            || self.hotspot_flyout_open
            || self.nearby_flyout_open
            || self.app_mixer_open
            || self.winmenu_open
            || self.run_open
            || self.quicklink_open
            || self.taskbar_menu_open
            || self.connect_open
            || self.emoji_open
            || self.snip_open
            || self.alttab_until.is_some();
        let frame = overlay_frame(FlyoutInput {
            locked: self.idle.phase == LockPhase::Locked,
            taskview: self.taskview_open,
            start: self.start_open,
            search: self.search_open,
            quick: self.quick_open,
            side: self.notifications_open
                || self.clipboard_open
                || self.tray_open
                || self.pin_overflow_open
                || self.win_overflow_open
                || self.net_flyout_open
                || self.vpn_flyout_open
                || self.hotspot_flyout_open
                || self.nearby_flyout_open
                || self.app_mixer_open,
            jumplist: self.jump_list_pin.is_some() || self.jump_list_win.is_some(),
            app_ctx: self.app_ctx.is_some(),
            desktop_menu: self.desktop_menu_open,
            taskbar_menu: self.taskbar_menu_open,
            widgets: self.widgets_open,
            keyboard: self.keyboard_open,
            alttab: self.alttab_until.is_some(),
            snapflyout: self.snapflyout_open || self.snap_assist_side.is_some(),
            volume: self.vol_flyout_open
                || self.bat_flyout_open
                || self.vpn_flyout_open
                || self.hotspot_flyout_open
                || self.nearby_flyout_open
                || self.app_mixer_open
                || self.emoji_open,
            winmenu: self.winmenu_open,
            sysmenu: self.sysmenu_open,
            run: self.run_open,
            quicklink: self.quicklink_open,
            connect: self.connect_open,
            share_pill: self.share_active && !major,
            // Full-zone Snip place for overlay and thumbnail toast (no mica card wrap).
            snip: self.snip_open || self.screenshot_toast.is_some(),
            toast: (self.toast.is_some() && !self.notifications_open)
                || self.clear_undo_until.is_some(),
            osd: self.osd.is_some(),
            centered,
        });
        let bar = self.sync_bar_geometry();
        let Some(frame) = frame else {
            self.overlay_frame = None;
            if let Some(id) = self.overlay_id.take() {
                return Task::batch([Task::done(Message::RemoveWindow(id)), bar]);
            }
            return bar;
        };
        if let Some(id) = self.overlay_id {
            if self.overlay_frame == Some(frame) {
                return bar;
            }
            self.overlay_frame = Some(frame);
            return Task::batch([
                Task::done(Message::AnchorSizeChange {
                    id,
                    anchor: frame.anchor,
                    size: (frame.width, frame.height),
                }),
                Task::done(Message::MarginChange {
                    id,
                    margin: frame.margin,
                }),
                bar,
            ]);
        }
        self.overlay_frame = Some(frame);
        let (id, message) = Message::layershell_open(NewLayerShellSettings {
            size: Some((frame.width, frame.height)),
            layer: Layer::Top,
            anchor: frame.anchor,
            exclusive_zone: Some(0),
            margin: Some(frame.margin),
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            use_last_output: false,
            events_transparent: false,
        });
        self.overlay_id = Some(id);
        Task::batch([Task::done(message), bar])
    }

    fn show_osd(&mut self, osd: OsdState) {
        match osd.kind {
            osd::OsdKind::Volume => {
                self.vol_level = osd.level;
                self.vol_muted = osd.muted;
            }
            osd::OsdKind::Brightness => self.bright_level = osd.level,
            osd::OsdKind::Mic => {
                self.mic_muted = osd.muted;
            }
            osd::OsdKind::Dictation => {}
        }
        self.status = osd.label();
        self.osd = Some(osd.clone());
        // W424: keep listening pill until second Super+H; other OSDs auto-dismiss.
        self.osd_until = if matches!(osd.kind, osd::OsdKind::Dictation) {
            None
        } else {
            Some(Instant::now() + Duration::from_millis(1600))
        };
    }

    fn refresh_levels(&mut self) {
        if let Ok(level) = system::volume_level() {
            self.vol_level = level;
        }
        if let Ok(muted) = system::volume_muted() {
            self.vol_muted = muted;
        }
        if let Ok(muted) = system::mic_muted() {
            self.mic_muted = muted;
        }
        if let Ok(level) = system::brightness_level() {
            self.bright_level = level;
        }
    }

    fn apply_windows_snapshot(&mut self, snap: mde_protocol::WindowsSnapshot) {
        self.desktop_active = snap.active;
        self.desktop_count = snap.count.max(1);
        self.idle_inhibited = snap.windows.iter().any(|w| w.fullscreen);
        self.windows = snap
            .windows
            .into_iter()
            .filter(|w| !w.title.is_empty())
            .map(|w| WindowInfo {
                id: Some(w.id),
                title: w.title,
                app_id: w.app_id,
                workspace: w.workspace,
                fullscreen: w.fullscreen,
            })
            .collect();
    }

    fn refresh_outputs(&mut self) {
        self.outputs = load_outputs_snapshot();
        if self.share_output.is_none() {
            self.share_output = self.outputs.first().map(|o| o.name.clone());
        }
    }

    fn refresh_windows(&mut self) {
        let path = mde_protocol::windows_json_path();
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        if let Ok(snap) = serde_json::from_str::<mde_protocol::WindowsSnapshot>(&text) {
            self.apply_windows_snapshot(snap);
            return;
        }
        // Legacy JSON without `id` fields.
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return;
        };
        if let Some(active) = v.get("active").and_then(|a| a.as_u64()) {
            self.desktop_active = active as usize;
        }
        if let Some(count) = v.get("count").and_then(|c| c.as_u64()) {
            self.desktop_count = (count as usize).max(1);
        }
        let windows = v.get("windows").and_then(|w| w.as_array());
        self.idle_inhibited = windows
            .map(|arr| {
                arr.iter()
                    .any(|w| w.get("fullscreen").and_then(|f| f.as_bool()).unwrap_or(false))
            })
            .unwrap_or(false);
        self.windows = windows
            .map(|arr| {
                arr.iter()
                    .filter_map(|w| {
                        let title = w.get("title")?.as_str()?.to_string();
                        if title.is_empty() {
                            return None;
                        }
                        let app_id = w
                            .get("app_id")
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();
                        let workspace = w
                            .get("workspace")
                            .and_then(|ws| ws.as_u64())
                            .unwrap_or(0) as usize;
                        let id = w.get("id").and_then(|i| i.as_u64());
                        let fullscreen = w
                            .get("fullscreen")
                            .and_then(|f| f.as_bool())
                            .unwrap_or(false);
                        Some(WindowInfo {
                            id,
                            title,
                            app_id,
                            workspace,
                            fullscreen,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    fn request_focus_index(&self, index: usize) {
        let legacy = mde_protocol::legacy_enabled();
        if let Some(id) = self.windows.get(index).and_then(|w| w.id) {
            self.ipc.focus_window(id, legacy);
        } else {
            self.ipc.focus_index(index, legacy);
        }
    }

    fn active_window_indices(&self) -> Vec<usize> {
        self.windows
            .iter()
            .enumerate()
            .filter(|(_, w)| w.workspace == self.desktop_active)
            .map(|(i, _)| i)
            .collect()
    }

    /// A046: per-desktop wallpaper from `desktops.wallpapers` when set.
    fn apply_desktop_wallpaper(&self, index: usize) {
        let active = self.cfg.active();
        let path = active
            .desktops
            .wallpaper_for(index)
            .filter(|p| std::path::Path::new(p).is_file())
            .unwrap_or_else(|| active.theme.wallpaper.clone());
        if path.trim().is_empty() {
            return;
        }
        let _ = system::set_wallpaper(&path);
    }

    fn write_workspace_req(&self, cmd: &str) {
        let Some(op) = mde_protocol::WorkspaceOp::parse(cmd) else {
            return;
        };
        self.ipc
            .workspace(op, mde_protocol::legacy_enabled());
    }

    fn write_snap_req(&self, kind: &str) {
        let Some(layout) = mde_protocol::SnapLayout::parse(kind) else {
            return;
        };
        self.ipc.snap(layout, mde_protocol::legacy_enabled());
    }

    /// Cycle focus to the next window on the active desktop.
    fn cycle_alttab(&mut self) {
        self.cycle_alttab_inner(true, false);
    }

    /// Alt+Shift+Tab reverse cycle with overlay (W248).
    fn cycle_alttab_rev(&mut self) {
        self.cycle_alttab_inner(true, true);
    }

    /// Alt+Esc quiet cycle — focus next without overlay (W108).
    fn cycle_alttab_quiet(&mut self) {
        self.cycle_alttab_inner(false, false);
    }

    fn cycle_alttab_inner(&mut self, show_overlay: bool, reverse: bool) {
        self.refresh_windows();
        let indices = self.active_window_indices();
        let desktop_slot = self.alttab_shows_desktop();
        let count = indices.len() + usize::from(desktop_slot);
        if count == 0 {
            self.status = if !show_overlay {
                "Alt+Esc · no windows".into()
            } else if reverse {
                "Alt+Shift+Tab · no windows".into()
            } else {
                "Alt+Tab · no windows".into()
            };
            self.alttab_until = None;
            return;
        }
        // `alttab_index` indexes into the active-desktop subset (+ optional Desktop stub).
        if reverse {
            self.alttab_index = if self.alttab_index == 0 {
                count - 1
            } else {
                self.alttab_index - 1
            };
        } else {
            self.alttab_index = (self.alttab_index + 1) % count;
        }
        let label = if !show_overlay {
            "Alt+Esc"
        } else if reverse {
            "Alt+Shift+Tab"
        } else {
            "Alt+Tab"
        };
        if desktop_slot && self.alttab_index == indices.len() {
            if show_overlay {
                self.alttab_until = Some(Instant::now() + Duration::from_millis(1000));
                self.status = format!("{label} · Desktop");
            } else {
                self.alttab_until = None;
                self.ipc
                    .show_desktop(mde_protocol::legacy_enabled());
                self.status = format!("{label} · Desktop");
            }
            return;
        }
        let global = indices[self.alttab_index];
        let title = self.windows[global].title.clone();
        self.request_focus_index(global);
        if show_overlay {
            self.alttab_until = Some(Instant::now() + Duration::from_millis(1000));
            self.status = format!("{label} · {title}");
        } else {
            self.alttab_until = None;
            self.status = format!("{label} · {title}");
        }
    }

    fn auto_bright_enabled(&self) -> bool {
        self.cfg
            .extras
            .get("qs.auto_brightness")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// W401: schedule-backed brightness (day ~70%, evening ~45%, night ~30%; dimmer on battery).
    fn apply_dynamic_brightness(&mut self, force: bool) -> Option<String> {
        if !self.auto_bright_enabled() && !force {
            return None;
        }
        let now = mde_theme::now();
        let minutes = (chrono::Timelike::num_seconds_from_midnight(&now) / 60) as u16;
        let mut target = if (7 * 60..18 * 60).contains(&minutes) {
            70u32
        } else if (18 * 60..21 * 60).contains(&minutes) {
            45
        } else {
            30
        };
        if self.battery_pct.is_some() && !self.battery_charging {
            target = target.saturating_sub(10).max(15);
        }
        let current = (self.bright_level * 100.0).round() as i32;
        if !force && (current - target as i32).unsigned_abs() < 4 {
            return None;
        }
        match system::set_brightness_percent(target) {
            Ok(level) => {
                self.bright_level = level;
                Some(format!("Dynamic brightness · {target}%"))
            }
            Err(e) => Some(e),
        }
    }

    fn start_focus_timer(&mut self, dur: Duration) {
        self.focus_timer_prev_dnd = Some(self.notifs.dnd);
        self.focus_timer_until = Some(Instant::now() + dur);
        if !self.notifs.dnd {
            self.notifs.dnd = true;
            self.hub.set_dnd(true);
            self.cfg.active_mut().notifications.dnd = true;
            let _ = self.cfg.save();
        }
    }

    fn end_focus_timer(&mut self) {
        self.focus_timer_until = None;
        if let Some(prev) = self.focus_timer_prev_dnd.take() {
            self.notifs.dnd = prev;
            self.hub.set_dnd(prev);
            self.cfg.active_mut().notifications.dnd = prev;
            let _ = self.cfg.save();
        }
    }

    fn focus_timer_active(&self) -> bool {
        self.focus_timer_until.is_some_and(|u| Instant::now() < u)
    }


    fn start_folder_apps(&self, folder_id: &str) -> Vec<String> {
        let name = folder_id.strip_prefix("mde-folder:").unwrap_or(folder_id);
        self.cfg.start_pin_folder_apps(name)
    }

    /// Hide from Start and record under Settings › Advanced (`start.advanced_*`).
    fn move_start_pin_to_advanced(&mut self, id: &str) -> String {
        if id.starts_with("mde-folder:") {
            let name = mde_config::StartConfig::folder_name(id).to_string();
            let start = &mut self.cfg.active_mut().start;
            start.pins.retain(|p| p != id);
            if !start
                .advanced_groups
                .iter()
                .any(|g| g.eq_ignore_ascii_case(&name))
            {
                start.advanced_groups.push(name.clone());
            }
            let _ = self.cfg.save();
            return format!("Advanced · folder · {name}");
        }
        // Remove from any Start folder membership.
        for folder_name in self.cfg.start_pin_folder_names() {
            let mut apps = self.cfg.start_pin_folder_apps(&folder_name);
            let before = apps.len();
            apps.retain(|a| a != id);
            if apps.len() != before {
                self.start_folder_set_apps(&format!("mde-folder:{folder_name}"), apps);
            }
        }
        let start = &mut self.cfg.active_mut().start;
        start.pins.retain(|p| p != id);
        if !start.advanced_apps.iter().any(|a| a == id) {
            start.advanced_apps.push(id.to_string());
        }
        let _ = self.cfg.save();
        format!("Advanced · {id}")
    }

    fn start_folder_set_apps(&mut self, folder_id: &str, apps: Vec<String>) {
        let name = folder_id
            .strip_prefix("mde-folder:")
            .unwrap_or(folder_id)
            .to_string();
        let mut arr = self
            .cfg
            .extras
            .get("start.pin_folders")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let mut found = false;
        for item in &mut arr {
            let n = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if n.eq_ignore_ascii_case(&name) {
                let mut map = item.as_table().cloned().unwrap_or_default();
                map.insert(
                    "apps".into(),
                    toml::Value::Array(
                        apps.iter().cloned().map(toml::Value::String).collect(),
                    ),
                );
                *item = toml::Value::Table(map);
                found = true;
                break;
            }
        }
        if !found {
            let mut map = toml::map::Map::new();
            map.insert("name".into(), toml::Value::String(name));
            map.insert(
                "apps".into(),
                toml::Value::Array(apps.into_iter().map(toml::Value::String).collect()),
            );
            arr.push(toml::Value::Table(map));
        }
        self.cfg
            .extras
            .insert("start.pin_folders".into(), toml::Value::Array(arr));
        let _ = self.cfg.save();
    }

    /// W450: pin grid capacity from `start.layout` (default vs more_pins).

    fn news_feed_url(&self) -> String {
        if let Some(u) = self
            .cfg
            .extras
            .get("widgets.news.feed")
            .and_then(|v| v.as_str())
        {
            if !u.is_empty() {
                return u.to_string();
            }
        }
        widgets::DEFAULT_NEWS_FEEDS[0].1.to_string()
    }

    fn markets_symbols(&self) -> Vec<String> {
        if let Some(arr) = self.cfg.extras.get("widgets.markets.symbols").and_then(|v| v.as_array()) {
            let v: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            if !v.is_empty() {
                return v;
            }
        }
        Vec::new()
    }

    fn traffic_endpoints(&self) -> (String, String) {
        let home = self
            .cfg
            .extras
            .get("widgets.traffic.home")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let work = self
            .cfg
            .extras
            .get("widgets.traffic.work")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (home, work)
    }

    fn refresh_widget_feeds_if_needed(&mut self) {
        if self.news_card.needs_refresh() {
            let url = self.news_feed_url();
            self.news_card.refresh(&url);
        }
        if self.markets_card.needs_refresh() {
            let syms = self.markets_symbols();
            self.markets_card.refresh(&syms);
        }
        if self.traffic_card.needs_refresh() {
            let (h, w) = self.traffic_endpoints();
            self.traffic_card.refresh(&h, &w);
        }
    }

    fn start_pin_cap(&self) -> usize {
        let layout = self.cfg.active().start.layout.to_lowercase().replace(' ', "_").replace('-', "_");
        match layout.as_str() {
            "more_pins" | "morepins" => 36,
            _ => 18,
        }
    }

    /// W450: taller Start card when More pins is selected.
    fn start_card_height(&self) -> u16 {
        let layout = self.cfg.active().start.layout.to_lowercase().replace(' ', "_").replace('-', "_");
        match layout.as_str() {
            "more_pins" | "morepins" => START_CARD_H.saturating_add(120),
            _ => START_CARD_H,
        }
    }

    fn reorder_start_pin_before(&mut self, src: &str, target: &str) {
        if src == target {
            return;
        }
        let pins = &mut self.cfg.active_mut().start.pins;
        let Some(si) = pins.iter().position(|p| p == src) else {
            return;
        };
        let src_val = pins.remove(si);
        let ti = pins.iter().position(|p| p == target).unwrap_or(pins.len());
        pins.insert(ti, src_val);
        let _ = self.cfg.save();
    }

    /// W410 / A014: Large when listed in `start.pin_large`, or default size is large.
    fn start_pin_is_large(&self, id: &str) -> bool {
        if let Some(arr) = self
            .cfg
            .extras
            .get("start.pin_large")
            .and_then(|v| v.as_array())
        {
            if arr
                .iter()
                .filter_map(|v| v.as_str())
                .any(|s| s == id)
            {
                return true;
            }
        }
        self.cfg
            .extras
            .get("start.pin_default_size")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("large"))
    }

    fn set_start_pin_large(&mut self, id: &str, large: bool) {
        let mut ids: Vec<String> = self
            .cfg
            .extras
            .get("start.pin_large")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if large {
            if !ids.iter().any(|s| s == id) {
                ids.push(id.to_string());
            }
        } else {
            ids.retain(|s| s != id);
        }
        let arr = ids
            .into_iter()
            .map(toml::Value::String)
            .collect::<Vec<_>>();
        self.cfg
            .extras
            .insert("start.pin_large".into(), toml::Value::Array(arr));
        let _ = self.cfg.save();
    }

    /// W400: battery saver on via extras or power-profiles-daemon `power-saver`.
    fn battery_saver_on(&self) -> bool {
        if self
            .cfg
            .extras
            .get("qs.battery_saver")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return true;
        }
        system::power_profile()
            .map(|p| p.eq_ignore_ascii_case("power-saver"))
            .unwrap_or(false)
    }

    fn apply_battery_saver(&mut self, on: bool) -> Result<String, String> {
        self.cfg
            .extras
            .insert("qs.battery_saver".into(), toml::Value::Boolean(on));
        let _ = self.cfg.save();
        let mut notes = Vec::new();
        if system::powerprofilesctl_available() {
            let profile = if on { "power-saver" } else { "balanced" };
            match system::set_power_profile(profile) {
                Ok(()) => notes.push(format!("profile {profile}")),
                Err(e) => notes.push(e),
            }
        } else if on {
            // Software fallback: dim display + Wi-Fi powersave (no sudo install).
            let prior = system::brightness_percent().unwrap_or(70);
            self.cfg.extras.insert(
                "qs.battery_saver_bright".into(),
                toml::Value::Integer(prior as i64),
            );
            let _ = self.cfg.save();
            match system::set_brightness_percent(prior.min(40).max(15)) {
                Ok(level) => {
                    self.bright_level = level;
                    notes.push(format!("brightness {:.0}%", level * 100.0));
                }
                Err(e) => notes.push(e),
            }
            notes.push(system::set_wifi_powersave(true));
        } else {
            let restore = self
                .cfg
                .extras
                .get("qs.battery_saver_bright")
                .and_then(|v| v.as_integer())
                .map(|i| i.clamp(10, 100) as u32)
                .unwrap_or(70);
            match system::set_brightness_percent(restore) {
                Ok(level) => {
                    self.bright_level = level;
                    notes.push(format!("brightness {:.0}%", level * 100.0));
                }
                Err(e) => notes.push(e),
            }
            notes.push(system::set_wifi_powersave(false));
        }
        Ok(if on {
            format!("Battery saver On · {}", notes.join(" · "))
        } else {
            format!("Battery saver Off · {}", notes.join(" · "))
        })
    }

    /// W398: SNI ids the user hid from the taskbar tray strip.
    fn tray_hidden_ids(&self) -> Vec<String> {
        let Some(val) = self.cfg.extras.get("tray.hidden") else {
            return Vec::new();
        };
        let Some(arr) = val.as_array() else {
            return Vec::new();
        };
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn persist_tray_hidden(&mut self, ids: &[String]) {
        let arr = ids
            .iter()
            .map(|s| toml::Value::String(s.clone()))
            .collect::<Vec<_>>();
        self.cfg
            .extras
            .insert("tray.hidden".into(), toml::Value::Array(arr));
        let _ = self.cfg.save();
    }

    /// Pinned Quick Settings tile ids from extras `qs.tiles`, else the compact default.
    fn qs_tiles(&self) -> Vec<String> {
        let Some(val) = self.cfg.extras.get("qs.tiles") else {
            return QS_DEFAULT_TILES.iter().map(|s| (*s).to_string()).collect();
        };
        let Some(arr) = val.as_array() else {
            return QS_DEFAULT_TILES.iter().map(|s| (*s).to_string()).collect();
        };
        let mut out = Vec::new();
        for item in arr {
            let Some(id) = item.as_str() else {
                continue;
            };
            if qs_catalog_label(id).is_none() {
                continue;
            }
            if out.iter().any(|t| t == id) {
                continue;
            }
            out.push(id.to_string());
            if out.len() >= QS_TILES_MAX {
                break;
            }
        }
        if out.is_empty() {
            out = QS_DEFAULT_TILES.iter().map(|s| (*s).to_string()).collect();
        }
        // A039: hide Hotspot / VPN / Cast when backend is missing.
        out.retain(|id| match id.as_str() {
            "hotspot" => system::hotspot_available(),
            "vpn" => system::vpn_available(),
            "cast" | "project" => system::cast_available(),
            _ => true,
        });
        if out.is_empty() {
            QS_DEFAULT_TILES.iter().map(|s| (*s).to_string()).collect()
        } else {
            out
        }
    }

    fn persist_qs_tiles(&mut self, tiles: &[String]) {
        let arr = tiles
            .iter()
            .map(|t| toml::Value::String(t.clone()))
            .collect::<Vec<_>>();
        self.cfg
            .extras
            .insert("qs.tiles".into(), toml::Value::Array(arr));
        let _ = self.cfg.save();
    }

    /// Notification sounds on by default; extras `notifications.sounds` (W145).
    fn notif_sounds_enabled(&self) -> bool {
        self.cfg
            .extras
            .get("notifications.sounds")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    /// Alt+Tab includes Desktop stub; extras `alttab.show_desktop` (W208).
    fn alttab_shows_desktop(&self) -> bool {
        self.cfg
            .extras
            .get("alttab.show_desktop")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Clear unpinned clipboard history when locking; extras `clipboard.clear_on_lock` (W223).
    fn clipboard_clear_on_lock(&self) -> bool {
        self.cfg
            .extras
            .get("clipboard.clear_on_lock")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// W215: remember last ≤3 calculator results for this session.
    fn push_calc_history(&mut self, expr: &str, value: f64) {
        let line = format!("{} {}", expr.trim(), format_math_result(value));
        if self.calc_history.first().is_some_and(|h| h == &line) {
            return;
        }
        self.calc_history.insert(0, line);
        self.calc_history.truncate(3);
    }

    /// W235: remember last ≤3 Settings pages opened this session.
    fn push_recent_settings(&mut self, page: &str) {
        let page = page.trim();
        if page.is_empty() {
            return;
        }
        self.recent_settings.retain(|p| p != page);
        self.recent_settings.insert(0, page.to_string());
        self.recent_settings.truncate(3);
    }

    /// Quiet-hours schedule window active right now (W210).
    fn quiet_hours_active_now(&self) -> bool {
        let n = &self.cfg.active().notifications;
        let (from_s, until_s) = if !n.quiet_from.trim().is_empty() && !n.quiet_to.trim().is_empty() {
            (n.quiet_from.as_str(), n.quiet_to.as_str())
        } else {
            let t = &self.cfg.active().theme;
            (t.dark_from.as_str(), t.dark_until.as_str())
        };
        let Some(from) = parse_hhmm_minutes(from_s) else {
            return false;
        };
        let Some(until) = parse_hhmm_minutes(until_s) else {
            return false;
        };
        let now = mde_theme::now();
        let minutes = (chrono::Timelike::num_seconds_from_midnight(&now) / 60) as u16;
        mde_theme::minutes_in_window(minutes, from, until)
    }

    fn maybe_play_notif_sound(&self) {
        if !self.notif_sounds_enabled() {
            return;
        }
        // Best-effort; skipped entirely when toggle is off.
        if std::process::Command::new("canberra-gtk-play")
            .args(["-i", "message-new-instant"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
        let _ = std::process::Command::new("paplay")
            .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    fn keyboard_pad(&self) -> Element<'_, Message> {
        let rows = if self.keyboard_symbols {
            keyboard::symbol_rows()
        } else {
            keyboard::letter_rows(self.keyboard_shift)
        };
        let mut col = column![
            row![
                text("Touch keyboard").size(15),
                Space::with_width(Fill),
                quiet(text(if self.keyboard_shift { "Shift" } else { "shift" }).size(12))
                    .on_press(Message::ToggleShift)
                    .style(if self.keyboard_shift {
                        accent_pill
                    } else {
                        glass_button
                    }),
                quiet(text(if self.keyboard_symbols { "ABC" } else { "&123" }).size(12))
                    .on_press(Message::ToggleSymbols),
                quiet(text("✕").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
            hairline(&self.palette),
        ]
        .spacing(8);
        for row_keys in rows {
            let mut line = row![].spacing(3);
            for key in row_keys {
                let token = (*key).to_string();
                let wide = token.len() > 1;
                line = line.push(
                    quiet(text(token.clone()).size(13))
                        .on_press(Message::TypeKey(token))
                        .padding(if wide { [10, 14] } else { [10, 12] }),
                );
            }
            col = col.push(container(line).center_x(Fill));
        }
        col = col.push(
            row![
                quiet(text("Space").size(13))
                    .on_press(Message::TypeKey(" ".into()))
                    .padding([10, 48]),
                quiet(text("⌫").size(13))
                    .on_press(Message::TypeKey("key:BackSpace".into()))
                    .padding([10, 16]),
                quiet(text("Enter").size(13))
                    .on_press(Message::TypeKey("key:Return".into()))
                    .style(accent_pill)
                    .padding([10, 18]),
            ]
            .spacing(6),
        );
        container(col)
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn invoke_tray(&mut self, id: &str, primary: bool) {
        let Some(item) = self.tray.items.iter().find(|item| item.id == id) else {
            return;
        };
        if item.bus.is_empty() || item.path.is_empty() {
            self.status = format!("{} has no tray service", item.title);
            return;
        }
        let bus = item.bus.clone();
        let path = item.path.clone();
        let title = item.title.clone();
        if primary {
            sni::request_activate(bus, path);
            self.status = format!("Activated {title}");
        } else {
            let generation = self.menu_board.begin();
            self.tray_menu = Some(sni::TrayMenu {
                item_id: id.to_string(),
                bus: bus.clone(),
                menu_path: String::new(),
                entries: Vec::new(),
                note: "Loading menu…".into(),
            });
            sni::load_menu(
                self.menu_board.clone(),
                generation,
                id.to_string(),
                bus,
                path,
            );
            self.status = format!("Menu for {title}");
        }
    }

    fn sync_sni(&mut self) {
        let entries = self.sni.snapshot();
        self.tray.items.retain(|i| !i.id.starts_with("sni:"));
        for entry in entries {
            let short = entry.bus.rsplit('.').next().unwrap_or(&entry.bus);
            let title = if entry.title.is_empty() {
                short.to_string()
            } else {
                entry.title
            };
            let status = if entry.status.is_empty() {
                "Active".into()
            } else {
                entry.status
            };
            let icon_path =
                icons::find_png(&entry.icon_name, self.palette.dark).unwrap_or(entry.icon_file);
            self.tray.upsert(tray::TrayItem {
                id: format!("sni:{}{}", entry.bus, entry.path),
                title,
                status,
                icon: entry.icon_name,
                icon_path,
                bus: entry.bus,
                path: entry.path,
            });
        }
    }

    /// Super+1..0: focus / cycle a running window for the pin, else launch (W31).
    fn activate_taskbar_pin(&mut self, idx: usize) {
        let Some(pin) = self.cfg.active().shell.pins.get(idx).cloned() else {
            self.status = format!("No pin at Super+{}", idx + 1);
            return;
        };
        self.refresh_windows();
        let indices = self.window_indices_for_pin(&pin);
        self.activate_taskbar_app(Some(pin), indices);
    }

    fn bar_visually_hidden(&self) -> bool {
        if !self.cfg.active().shell.taskbar_autohide {
            return false;
        }
        if self.bar_hover || self.flyout_open() {
            return false;
        }
        // A009: stay revealed until the leave grace elapses.
        if let Some(until) = self.bar_hide_after {
            if Instant::now() < until {
                return false;
            }
        }
        true
    }

    fn sync_bar_geometry(&self) -> Task<Message> {
        let Some(id) = self.bar_id.get() else {
            return Task::none();
        };
        let h = if self.bar_visually_hidden() {
            Self::BAR_HIT
        } else {
            self.bar_height()
        };
        Task::done(Message::SizeChange {
            id,
            size: (0, h),
        })
    }

    fn pin_matches_window(pin: &str, win: &WindowInfo) -> bool {
        let needle = pin.trim_end_matches(".desktop").to_lowercase();
        if needle.is_empty() {
            return false;
        }
        let id = win.app_id.to_lowercase();
        if !id.is_empty()
            && (id == needle || id.contains(&needle) || needle.contains(&id))
        {
            return true;
        }
        let label = apps::resolve_desktop(pin)
            .map(|p| apps::desktop_name(&p))
            .unwrap_or_else(|| pin.to_string())
            .to_lowercase();
        if label.is_empty() {
            return false;
        }
        let title = win.title.to_lowercase();
        title.contains(&label) || label.contains(&title.chars().take(8).collect::<String>())
    }

    fn window_indices_for_pin(&self, pin: &str) -> Vec<usize> {
        self.windows
            .iter()
            .enumerate()
            .filter(|(_, w)| w.workspace == self.desktop_active && Self::pin_matches_window(pin, w))
            .map(|(i, _)| i)
            .collect()
    }

    /// A004: match Notify desktop-entry / app_name to a pin or app_id.
    fn notif_matches_app(n: &notifications::Notification, pin_or_id: &str) -> bool {
        let needle = pin_or_id.trim_end_matches(".desktop").to_lowercase();
        if needle.is_empty() {
            return false;
        }
        if let Some(de) = n.desktop_entry.as_deref() {
            let de = de.trim_end_matches(".desktop").to_lowercase();
            if de == needle || de.contains(&needle) || needle.contains(&de) {
                return true;
            }
        }
        let app = n.app_name.to_lowercase();
        !app.is_empty() && (app == needle || app.contains(&needle) || needle.contains(&app))
    }

    fn badge_for_app(&self, pin_or_id: &str) -> (u32, bool) {
        if !self.cfg.active().shell.taskbar_badges {
            return (0, false);
        }
        let mut unread = 0u32;
        let mut progress = false;
        for n in &self.notifs.items {
            if Self::notif_matches_app(n, pin_or_id) {
                unread = unread.saturating_add(1);
                if n.progress.is_some_and(|p| p > 0.0 && p < 1.0) {
                    progress = true;
                }
            }
        }
        (unread, progress)
    }

    fn clear_badges_for_app(&mut self, pin_or_id: &str) {
        if !self.cfg.active().shell.taskbar_badges {
            return;
        }
        let before = self.notifs.items.len();
        self.notifs
            .items
            .retain(|n| !Self::notif_matches_app(n, pin_or_id));
        if self.notifs.items.len() != before {
            self.notifs.persist();
        }
    }

    fn taskbar_app_entries(&self) -> Vec<TaskbarAppEntry> {
        let multi = self
            .cfg
            .active()
            .shell
            .taskbar_multi_mode
            .to_lowercase();
        let hide_unpinned = multi == "main_buttons" || multi == "main_only";
        let combine = self.cfg.active().shell.taskbar_combine.to_lowercase();
        let when_full = combine == "whenfull" || combine == "when_full" || combine == "full";
        // When-full starts uncombined (like Never); overflows into Always grouping below.
        let never = combine == "never" || when_full;
        let max_icons = 12usize;
        let mut claimed = HashSet::new();
        let mut out = Vec::new();

        let push_pin_combined = |out: &mut Vec<TaskbarAppEntry>,
                                 pin: &str,
                                 indices: Vec<usize>,
                                 shell: &Shell| {
            let label = apps::resolve_desktop(pin)
                .map(|p| apps::desktop_name(&p))
                .unwrap_or_else(|| pin.to_string());
            let icon = apps::desktop_icon(pin).unwrap_or_default();
            let (unread, progress) = shell.badge_for_app(pin);
            out.push(TaskbarAppEntry {
                pin: Some(pin.to_string()),
                label,
                icon,
                running: !indices.is_empty(),
                progress,
                unread,
                indices,
            });
        };

        for pin in &self.cfg.active().shell.pins {
            let indices = self.window_indices_for_pin(pin);
            for &i in &indices {
                claimed.insert(i);
            }
            if never {
                // One button per window; keep a single idle pin if none running.
                if indices.is_empty() {
                    push_pin_combined(&mut out, pin, Vec::new(), self);
                } else {
                    for &i in &indices {
                        let label = self
                            .windows
                            .get(i)
                            .map(|w| {
                                if w.title.trim().is_empty() {
                                    apps::resolve_desktop(pin)
                                        .map(|p| apps::desktop_name(&p))
                                        .unwrap_or_else(|| pin.clone())
                                } else {
                                    w.title.clone()
                                }
                            })
                            .unwrap_or_else(|| pin.clone());
                        let icon = apps::desktop_icon(pin).unwrap_or_default();
                        let (unread, progress) = self.badge_for_app(pin);
                        out.push(TaskbarAppEntry {
                            pin: Some(pin.clone()),
                            label,
                            icon,
                            running: true,
                            progress,
                            unread,
                            indices: vec![i],
                        });
                    }
                }
            } else {
                push_pin_combined(&mut out, pin, indices, self);
            }
        }

        // Unpinned running apps on the active desktop (A096 main_buttons hides these).
        if !hide_unpinned {
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, win) in self.windows.iter().enumerate() {
            if win.workspace != self.desktop_active || claimed.contains(&i) {
                continue;
            }
            if never {
                let icon = if win.app_id.is_empty() {
                    "application-x-executable".into()
                } else {
                    win.app_id.clone()
                };
                let label = if win.title.trim().is_empty() {
                    icon.clone()
                } else {
                    win.title.clone()
                };
                let key = if win.app_id.is_empty() {
                    format!("title:{}", win.title.to_lowercase())
                } else {
                    win.app_id.clone()
                };
                let (unread, progress) = self.badge_for_app(&key);
                out.push(TaskbarAppEntry {
                    pin: None,
                    label,
                    icon,
                    running: true,
                    progress,
                    unread,
                    indices: vec![i],
                });
                continue;
            }
            let key = if win.app_id.is_empty() {
                format!("title:{}", win.title.to_lowercase())
            } else {
                win.app_id.to_lowercase()
            };
            if let Some((_, idxs)) = groups.iter_mut().find(|(k, _)| k == &key) {
                idxs.push(i);
            } else {
                groups.push((key, vec![i]));
            }
        }
        for (_key, indices) in groups {
            let Some(first) = indices.first().and_then(|&i| self.windows.get(i)) else {
                continue;
            };
            let icon = if first.app_id.is_empty() {
                "application-x-executable".into()
            } else {
                first.app_id.clone()
            };
            let label = if first.title.trim().is_empty() {
                icon.clone()
            } else {
                first
                    .title
                    .split_whitespace()
                    .next()
                    .unwrap_or(icon.as_str())
                    .to_string()
            };
            let badge_key = if first.app_id.is_empty() {
                label.clone()
            } else {
                first.app_id.clone()
            };
            let (unread, progress) = self.badge_for_app(&badge_key);
            out.push(TaskbarAppEntry {
                pin: None,
                label,
                icon,
                running: true,
                progress,
                unread,
                indices,
            });
        }
        }

        // A005 when-full: prefer uncombined windows until the bar would overflow, then
        // fall back to Always-style grouping (rebuild from pins + app_id groups).
        if when_full && out.len() > max_icons {
            // Rebuild combined (Always) view — same as never=false path above.
            let mut claimed2 = HashSet::new();
            let mut combined = Vec::new();
            for pin in &self.cfg.active().shell.pins {
                let indices = self.window_indices_for_pin(pin);
                for &i in &indices {
                    claimed2.insert(i);
                }
                push_pin_combined(&mut combined, pin, indices, self);
            }
            if !hide_unpinned {
            let mut groups2: Vec<(String, Vec<usize>)> = Vec::new();
            for (i, win) in self.windows.iter().enumerate() {
                if win.workspace != self.desktop_active || claimed2.contains(&i) {
                    continue;
                }
                let key = if win.app_id.is_empty() {
                    format!("title:{}", win.title.to_lowercase())
                } else {
                    win.app_id.to_lowercase()
                };
                if let Some((_, idxs)) = groups2.iter_mut().find(|(k, _)| k == &key) {
                    idxs.push(i);
                } else {
                    groups2.push((key, vec![i]));
                }
            }
            for (_key, indices) in groups2 {
                let Some(first) = indices.first().and_then(|&i| self.windows.get(i)) else {
                    continue;
                };
                let icon = if first.app_id.is_empty() {
                    "application-x-executable".into()
                } else {
                    first.app_id.clone()
                };
                let label = if first.title.trim().is_empty() {
                    icon.clone()
                } else {
                    first
                        .title
                        .split_whitespace()
                        .next()
                        .unwrap_or(icon.as_str())
                        .to_string()
                };
                let badge_key = if first.app_id.is_empty() {
                    label.clone()
                } else {
                    first.app_id.clone()
                };
                let (unread, progress) = self.badge_for_app(&badge_key);
                combined.push(TaskbarAppEntry {
                    pin: None,
                    label,
                    icon,
                    running: true,
                    progress,
                    unread,
                    indices,
                });
            }
            }
            return combined;
        }
        let _ = (when_full, max_icons);
        out
    }

    fn reorder_shell_pin_before(&mut self, src: &str, target: &str) {
        if src == target {
            return;
        }
        let pins = &mut self.cfg.active_mut().shell.pins;
        let Some(from) = pins.iter().position(|p| p == src) else {
            return;
        };
        let Some(mut to) = pins.iter().position(|p| p == target) else {
            return;
        };
        let item = pins.remove(from);
        if from < to {
            to -= 1;
        }
        pins.insert(to, item);
        let _ = self.cfg.save();
        self.status = format!("Moved pin");
    }

    fn focus_window_index(&mut self, index: usize) {
        self.request_focus_index(index);
        if let Some(win) = self.windows.get(index) {
            self.status = format!("Focus {}", win.title);
        }
    }

    fn activate_taskbar_app(&mut self, pin: Option<String>, indices: Vec<usize>) {
        // A004: activating an app clears its notification badges.
        if let Some(ref id) = pin {
            self.clear_badges_for_app(id);
        } else if let Some(&i) = indices.first() {
            if let Some(win) = self.windows.get(i) {
                let key = if win.app_id.is_empty() {
                    win.title.clone()
                } else {
                    win.app_id.clone()
                };
                self.clear_badges_for_app(&key);
            }
        }
        if indices.is_empty() {
            let Some(id) = pin else {
                return;
            };
            let expanded = expand_user_path(&id);
            if std::path::Path::new(&expanded).exists() {
                match files::open_path(&expanded) {
                    Ok(()) => {
                        self.status = format!("Opened {expanded}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            } else {
                match apps::launch_desktop(&id) {
                    Ok(()) => {
                        recent::record_launch(&id);
                        self.recent = recent::load(8);
                        self.status = format!("Launched {id}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            }
            return;
        }
        if indices.len() == 1 {
            self.focus_window_index(indices[0]);
            return;
        }
        let key = pin
            .clone()
            .unwrap_or_else(|| format!("win:{}", indices[0]));
        let next = self.app_cycle.get(&key).copied().unwrap_or(0) % indices.len();
        let idx = indices[next];
        self.app_cycle.insert(key, (next + 1) % indices.len());
        self.focus_window_index(idx);
    }

    fn write_window_action(&self, action: &str) {
        let Some(action) = mde_protocol::WinAction::parse(action) else {
            return;
        };
        self.ipc
            .window_action(action, mde_protocol::legacy_enabled());
    }

    fn launch_control(&mut self, page: &str) {
        let page = settings_page_id(page.trim());
        let mut cmd = std::process::Command::new("mde-control");
        if !page.is_empty() {
            cmd.arg(page);
            // Hint file for older control builds / deep-link consumers (W66).
            let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
            let _ = std::fs::write(
                std::path::Path::new(&runtime).join("mde-control.page"),
                format!("{page}\n"),
            );
            // W235: track recent Settings pages for Start.
            self.push_recent_settings(page);
        }
        let _ = cmd.spawn();
        self.status = if page.is_empty() {
            "Launching Settings".into()
        } else {
            let label = settings_page_label(page);
            format!("Open Settings · {label}")
        };
    }

    /// Auto-enable DND once while quiet hours are active (W67).
    fn apply_quiet_hours(&mut self) {
        let n = &self.cfg.active().notifications;
        let (from_s, until_s) = if !n.quiet_from.trim().is_empty() && !n.quiet_to.trim().is_empty() {
            (n.quiet_from.as_str(), n.quiet_to.as_str())
        } else {
            let t = &self.cfg.active().theme;
            (t.dark_from.as_str(), t.dark_until.as_str())
        };
        let Some(from) = parse_hhmm_minutes(from_s) else {
            return;
        };
        let Some(until) = parse_hhmm_minutes(until_s) else {
            return;
        };
        let now = mde_theme::now();
        let minutes = (chrono::Timelike::num_seconds_from_midnight(&now) / 60) as u16;
        let in_quiet = mde_theme::minutes_in_window(minutes, from, until);
        if !in_quiet {
            self.quiet_hours_applied = false;
            return;
        }
        if self.quiet_hours_applied {
            return;
        }
        self.quiet_hours_applied = true;
        if self.notifs.dnd {
            return;
        }
        self.notifs.dnd = true;
        self.hub.set_dnd(true);
        self.cfg.active_mut().notifications.dnd = true;
        let _ = self.cfg.save();
        self.status = "Quiet hours · Focus assist on".into();
    }

    fn flyout_flags(&self) -> core::FlyoutFlags {
        core::FlyoutFlags {
            start_open: self.start_open,
            quick_open: self.quick_open,
            notifications_open: self.notifications_open,
            clipboard_open: self.clipboard_open,
            tray_open: self.tray_open,
            keyboard_open: self.keyboard_open,
            taskview_open: self.taskview_open,
            widgets_open: self.widgets_open,
            snapflyout_open: self.snapflyout_open,
            winmenu_open: self.winmenu_open,
            run_open: self.run_open,
            quicklink_open: self.quicklink_open,
            vol_flyout_open: self.vol_flyout_open,
            net_flyout_open: self.net_flyout_open,
            bat_flyout_open: self.bat_flyout_open,
            app_mixer_open: self.app_mixer_open,
            vpn_flyout_open: self.vpn_flyout_open,
            hotspot_flyout_open: self.hotspot_flyout_open,
            nearby_flyout_open: self.nearby_flyout_open,
            pin_overflow_open: self.pin_overflow_open,
            win_overflow_open: self.win_overflow_open,
            taskbar_menu_open: self.taskbar_menu_open,
            power_menu_open: self.power_menu_open,
            connect_open: self.connect_open,
            emoji_open: self.emoji_open,
            snip_open: self.snip_open,
        }
    }

    fn apply_flyout_flags(&mut self, f: core::FlyoutFlags) {
        self.start_open = f.start_open;
        self.quick_open = f.quick_open;
        self.notifications_open = f.notifications_open;
        self.clipboard_open = f.clipboard_open;
        self.tray_open = f.tray_open;
        self.keyboard_open = f.keyboard_open;
        self.taskview_open = f.taskview_open;
        self.widgets_open = f.widgets_open;
        self.snapflyout_open = f.snapflyout_open;
        self.winmenu_open = f.winmenu_open;
        self.run_open = f.run_open;
        self.quicklink_open = f.quicklink_open;
        self.vol_flyout_open = f.vol_flyout_open;
        self.net_flyout_open = f.net_flyout_open;
        self.bat_flyout_open = f.bat_flyout_open;
        self.app_mixer_open = f.app_mixer_open;
        self.vpn_flyout_open = f.vpn_flyout_open;
        self.hotspot_flyout_open = f.hotspot_flyout_open;
        self.nearby_flyout_open = f.nearby_flyout_open;
        self.pin_overflow_open = f.pin_overflow_open;
        self.win_overflow_open = f.win_overflow_open;
        self.taskbar_menu_open = f.taskbar_menu_open;
        self.power_menu_open = f.power_menu_open;
        self.connect_open = f.connect_open;
        self.emoji_open = f.emoji_open;
        self.snip_open = f.snip_open;
    }

    fn apply_shell_cmd(&mut self, cmd: mde_protocol::ShellCmd) {
        let mut flags = self.flyout_flags();
        let effects = core::apply_shell_cmd(&mut flags, cmd);
        // Context menus live outside FlyoutFlags — drop them whenever chrome exclusivity resets.
        self.jump_list_pin = None;
        self.jump_list_win = None;
        self.app_ctx = None;
        self.desktop_menu_open = false;
        self.sysmenu_open = false;
        self.apply_flyout_flags(flags);
        for effect in effects {
            match effect {
                core::FlyoutEffect::Status(s) => self.status = s,
                core::FlyoutEffect::EngageLock(reason) => self.engage_lock(reason),
                core::FlyoutEffect::Unlock => {
                    write_unlock_req();
                    self.idle.unlock();
                    self.idle_accum_ms = 0;
                    clear_lock_flag();
                    // Drop our child handle; mde-lock exits after unlock req / session kill.
                    self.lock_child = None;
                    self.lock_deaths = 0;
                    self.status = "Unlocked".into();
                }
                core::FlyoutEffect::RefreshLevels => self.refresh_levels(),
                core::FlyoutEffect::RefreshWindows => self.refresh_windows(),
                core::FlyoutEffect::RefreshWeather => {
                    if self.weather.needs_refresh() {
                        self.weather.refresh();
                    }
                }
                core::FlyoutEffect::ClipboardOpenPrep => {
                    self.clipboard.drop_expired();
                    if let Ok(out) = std::process::Command::new("wl-paste").arg("-n").output() {
                        if out.status.success() {
                            let text = String::from_utf8_lossy(&out.stdout);
                            if !text.is_empty() {
                                self.clipboard.push(&text);
                            }
                        }
                    }
                    self.status = format!(
                        "Clipboard history ({} items)",
                        self.clipboard.items.len()
                    );
                }
                core::FlyoutEffect::ActivatePin(idx) => {
                    self.activate_taskbar_pin(idx as usize);
                }
                core::FlyoutEffect::CycleAltTab => self.cycle_alttab(),
                core::FlyoutEffect::CycleAltTabRev => self.cycle_alttab_rev(),
                core::FlyoutEffect::CycleAltTabQuiet => self.cycle_alttab_quiet(),
                core::FlyoutEffect::WriteWindowAction(a) => self.write_window_action(a),
                core::FlyoutEffect::ShowDesktop => {
                    self.ipc
                        .show_desktop(mde_protocol::legacy_enabled());
                }
                core::FlyoutEffect::OpenDesktopMenu => {
                    self.close_flyouts();
                    self.desktop_menu_open = true;
                    self.status = "Desktop menu".into();
                }
                core::FlyoutEffect::OpenSysMenu => {
                    self.close_flyouts();
                    self.sysmenu_open = true;
                    self.status = "System menu".into();
                }
                core::FlyoutEffect::Magnifier(on) => {
                    self.status = spawn_magnifier(on);
                }
                core::FlyoutEffect::ShortcutGuide => {
                    if self.cfg.active().tools.shortcut_guide {
                        self.close_flyouts();
                        self.shortcut_guide_open = !self.shortcut_guide_open;
                        self.status = if self.shortcut_guide_open {
                            "Shortcut Guide".into()
                        } else {
                            "Shortcut Guide closed".into()
                        };
                    } else {
                        self.status = "Shortcut Guide disabled in Settings".into();
                    }
                }
                core::FlyoutEffect::ColorFilter => {
                    match system::toggle_color_filter() {
                        Ok(s) => self.status = s,
                        Err(e) => self.status = e,
                    }
                }
                core::FlyoutEffect::LaunchControl(page) => self.launch_control(page),
                core::FlyoutEffect::OpenExplorer => match files::open_file_explorer() {
                    Ok(path) => {
                        self.close_flyouts();
                        self.status = format!("File Explorer · {path}");
                    }
                    Err(e) => self.status = e,
                },
                core::FlyoutEffect::OpenPerfmon => {
                    self.close_flyouts();
                    let _ = std::process::Command::new("mde-mmc")
                        .arg("perfmon.msc")
                        .spawn();
                    self.status = "Opened Performance Monitor".into();
                }
                core::FlyoutEffect::VolUp => {
                    if let Ok(level) = system::volume_adjust(5) {
                        self.show_osd(OsdState::volume(level, false));
                    }
                }
                core::FlyoutEffect::VolDown => {
                    if let Ok(level) = system::volume_adjust(-5) {
                        self.show_osd(OsdState::volume(level, false));
                    }
                }
                core::FlyoutEffect::Mute => {
                    if let Ok((level, muted)) = system::volume_toggle_mute() {
                        self.vol_muted = muted;
                        if muted {
                            self.vol_mute_flash_until =
                                Some(Instant::now() + Duration::from_millis(900));
                        } else {
                            self.vol_mute_flash_until = None;
                        }
                        self.show_osd(OsdState::volume(level, muted));
                    }
                }
                core::FlyoutEffect::BrightUp => {
                    if let Ok(level) = system::brightness_adjust(5) {
                        self.show_osd(OsdState {
                            kind: osd::OsdKind::Brightness,
                            level,
                            muted: false,
                        });
                    }
                }
                core::FlyoutEffect::BrightDown => {
                    if let Ok(level) = system::brightness_adjust(-5) {
                        self.show_osd(OsdState {
                            kind: osd::OsdKind::Brightness,
                            level,
                            muted: false,
                        });
                    }
                }
                core::FlyoutEffect::OpenSnip { preselect } => {
                    self.snip_preselect = preselect;
                    self.snip_open = true;
                    self.status = "Screen snipping".into();
                }
                core::FlyoutEffect::ColorPick => {
                    self.status = spawn_color_picker();
                    if self.cfg.active().tools.color_picker_history {
                        if let Some(hex) = extract_hex_from_status(&self.status) {
                            tools::push_color_history(&hex);
                            self.color_history = tools::load_color_history(12);
                        }
                        self.color_history_open = true;
                    }
                }
                core::FlyoutEffect::FindMyMouse => {
                    if self.cfg.active().tools.find_my_mouse {
                        self.find_mouse_ticks = 40;
                        self.status = "Find My Mouse".into();
                    }
                }
                core::FlyoutEffect::DictationToggle => {
                    if self.dictation.active {
                        // Stop + commit
                        let url = self
                            .cfg
                            .extras
                            .get("start.voice_stt_url")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());
                        match dictation::stop_and_transcribe(url.as_deref()) {
                            Ok(()) => {
                                self.dictation.mark_pending();
                                self.osd = None;
                                self.osd_until = None;
                                self.status = "Dictation · transcribing…".into();
                            }
                            Err(e) => {
                                self.dictation.clear();
                                self.osd = None;
                                self.status = e;
                            }
                        }
                    } else {
                        match dictation::start_listening() {
                            Ok(()) => {
                                self.dictation.toggle_start();
                                self.show_osd(OsdState::dictation());
                                // Override label via status; OSD shows Microphone
                                self.status = "Dictation · listening (Super+H to stop)".into();
                                if !dictation::has_offline_model()
                                    && self
                                        .cfg
                                        .extras
                                        .get("start.voice_stt_url")
                                        .and_then(|v| v.as_str())
                                        .filter(|s| !s.is_empty())
                                        .is_none()
                                {
                                    self.status = "Dictation · listening · no model yet (docs/voice-typing.md)".into();
                                }
                            }
                            Err(e) => self.status = e,
                        }
                    }
                }
                core::FlyoutEffect::Emoji => {
                    // flags already toggled emoji_open via apply_shell_cmd.
                    if self.emoji_open {
                        self.status = "Emoji".into();
                    } else {
                        self.status = "Emoji closed".into();
                    }
                }
                core::FlyoutEffect::InputLang => self.status = input_language_status(),
                core::FlyoutEffect::Diag => {
                    let text = session_diagnostics_line();
                    let copied = std::process::Command::new("wl-copy")
                        .arg(&text)
                        .spawn()
                        .is_ok();
                    self.status = if copied {
                        "Diagnostics copied".into()
                    } else {
                        format!("Diagnostics · {text}")
                    };
                }
                core::FlyoutEffect::ClearRunQuery => {
                    self.run_query.clear();
                    self.palette_hits = build_palette_hits(self);
                }
                core::FlyoutEffect::PendingStartFocus => {
                    self.start_all_apps = false;
                    self.pending_start_focus = true;
                }
                core::FlyoutEffect::SnapUndo => self.write_window_action("restore"),
            }
        }
    }

    fn drain_protocol(&mut self) {
        for msg in self.ipc.try_recv() {
            match msg {
                mde_protocol::CompToShell::ShellCmd(cmd) => self.apply_shell_cmd(cmd),
                mde_protocol::CompToShell::WindowsSnapshot(snap) => {
                    self.apply_windows_snapshot(snap);
                }
                mde_protocol::CompToShell::OutputsSnapshot(snap) => {
                    self.outputs = snap.outputs;
                    if self.share_output.is_none() {
                        self.share_output = self.outputs.first().map(|o| o.name.clone());
                    }
                }
                mde_protocol::CompToShell::WorkspaceState { active, count } => {
                    self.desktop_active = active;
                    self.desktop_count = count.max(1);
                }
                mde_protocol::CompToShell::SnapAssist { side } => {
                    self.snap_assist_side = Some(match side {
                        mde_protocol::Side::Left => "left".into(),
                        mde_protocol::Side::Right => "right".into(),
                    });
                    self.snap_assist_until = Some(Instant::now() + Duration::from_secs(8));
                }
                mde_protocol::CompToShell::LockState(_) => {}
                mde_protocol::CompToShell::FocusedWindowGeom(geom) => {
                    if self.pending_window_capture {
                        self.pending_window_capture = false;
                        let g = geom.clone();
                        // Capture runs after overlay is already closed.
                        let outcome = screenshot::capture(
                            mde_protocol::ScreenshotMode::Window,
                            g.as_deref(),
                        );
                        self.apply_capture_outcome(outcome);
                    }
                }
            }
        }
    }

    fn apply_capture_outcome(&mut self, outcome: screenshot::CaptureOutcome) {
        self.status = outcome.status_line();
        if let screenshot::CaptureOutcome::Saved { path, .. } = outcome {
            self.screenshot_toast = Some(snip_view::ScreenshotToast::new(path));
        }
    }

    fn begin_snip_capture(&mut self, mode: mde_protocol::ScreenshotMode) -> Task<Message> {
        self.snip_open = false;
        self.snip_preselect = None;
        match mode {
            mde_protocol::ScreenshotMode::Window => {
                self.pending_window_capture = true;
                self.ipc
                    .send(mde_protocol::ClientToComp::FocusedWindowGeom);
                self.status = "Screenshot · capturing window…".into();
                Task::none()
            }
            other => {
                // Brief pause so the dimmer/toolbar leave the frame before grim.
                Task::perform(
                    async move {
                        tokio::time::sleep(Duration::from_millis(80)).await;
                        tokio::task::spawn_blocking(move || screenshot::capture(other, None))
                            .await
                            .unwrap_or_else(|e| {
                                screenshot::CaptureOutcome::Failed(format!("join: {e}"))
                            })
                    },
                    Message::ScreenshotCaptured,
                )
            }
        }
    }

    fn take_shell_cmd(&mut self) {
        self.drain_protocol();
        // Legacy file fallback (dual-read during migration).
        let path = mde_protocol::shell_cmd_path();
        let Ok(cmd) = std::fs::read_to_string(&path) else {
            return;
        };
        let _ = std::fs::remove_file(&path);
        self.apply_shell_cmd(mde_protocol::ShellCmd::parse(&cmd));
    }

    fn maybe_reload_config(&mut self) {
        let Ok(path) = MdeConfig::config_path() else {
            return;
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            return;
        };
        let Ok(mtime) = meta.modified() else {
            return;
        };
        if self.config_mtime == Some(mtime) {
            return;
        }
        self.config_mtime = Some(mtime);
        if let Ok(cfg) = MdeConfig::load() {
            let next = Palette::from_theme(&cfg.active().theme);
            if next.dark != self.palette.dark {
                system::apply_mackes_icon_theme(next.dark);
            }
            self.palette = next;
            self.notifs.dnd = cfg.active().notifications.dnd;
            self.notifs.priority = cfg.active().notifications.priority;
            self.notifs.priority_apps =
                notifications::load_priority_apps(&cfg.active().shell.pins);
            self.notifs.muted_apps = notifications::load_muted_apps();
            write_key_remaps(&self.cfg);
            self.hub.set_dnd(cfg.active().notifications.dnd);
            self.hub
                .set_priority(self.notifs.priority, self.notifs.priority_apps.clone());
            self.hub.set_muted_apps(self.notifs.muted_apps.clone());
            let idle_secs = if std::env::var_os("MDE_SAFE").is_some() {
                86_400
            } else {
                cfg.active().session.idle_lock_seconds
            };
            self.idle.set_lock_after_secs(idle_secs);
            self.cfg = cfg;
            self.status = "Config reloaded".into();
        }
    }

    fn persist_network(&mut self) {
        let n = self.cfg.active_mut();
        n.network.wifi = self.wifi_on;
        n.network.bluetooth = self.bt_on;
        n.network.airplane_mode = self.airplane;
        let _ = self.cfg.save();
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenTaskbarMenu => {
                self.idle.activity();
                let open = !self.taskbar_menu_open;
                let anchor = pointer_x::last();
                self.close_flyouts();
                self.taskbar_menu_open = open;
                self.taskbar_menu_anchor_x = if open { anchor } else { None };
                self.status = if open {
                    "Taskbar menu".into()
                } else {
                    "Taskbar menu closed".into()
                };
            }
            Message::ToggleQuickLink => {
                self.idle.activity();
                let open = !self.quicklink_open;
                self.close_flyouts();
                self.quicklink_open = open;
                self.status = if open {
                    "Quick link".into()
                } else {
                    "Quick link closed".into()
                };
            }
            Message::OpenTaskManager => {
                self.idle.activity();
                self.close_flyouts();
                self.status = spawn_task_manager();
            }
            Message::ToggleStart => {
                self.idle.activity();
                let open = !self.start_open;
                self.close_flyouts();
                self.start_open = open;
                self.start_all_apps = false;
                self.status = if open {
                    "Start open".into()
                } else {
                    "Start closed".into()
                };
                if open {
                    let a11y = &self.cfg.active().accessibility;
                    if a11y.auto_osk
                        || system::tablet_posture_active(&a11y.tablet_mode)
                    {
                        self.keyboard_open = true;
                    }
                    self.pending_start_focus = true;
                    return self.sync_overlay();
                }
            }
            Message::OpenShortcutGuide => {
                if !self.cfg.active().tools.shortcut_guide {
                    self.status = "Shortcut Guide disabled in Settings".into();
                } else {
                    self.close_flyouts();
                    self.shortcut_guide_open = true;
                    self.status = "Shortcut Guide".into();
                }
            }
            Message::OpenStartSearch => {
                // W461: dedicated Search surface (not full Start home).
                self.idle.activity();
                self.close_flyouts();
                self.search_open = true;
                self.start_query.clear();
                self.pending_start_focus = true;
                let n = files::refresh_file_index(4000);
                self.status = if n > 0 {
                    format!("Search · indexed {n} files")
                } else {
                    "Search".into()
                };
                return self.sync_overlay();
            }
            Message::QsStub(label) => {
                self.idle.activity();
                self.status = format!("{label} · Not available yet");
            }
            Message::ToggleConnect => {
                self.idle.activity();
                let open = !self.connect_open;
                self.close_flyouts();
                self.connect_open = open;
                if open {
                    self.refresh_outputs();
                }
                self.status = if open {
                    "Wireless display".into()
                } else {
                    "Wireless display closed".into()
                };
            }
            Message::ConnectSelectOutput(name) => {
                self.idle.activity();
                self.share_output = Some(name.clone());
                self.status = format!("Share · {name}");
            }
            Message::ConnectToggleAudio => {
                self.idle.activity();
                self.share_include_audio = !self.share_include_audio;
                self.status = if self.share_include_audio {
                    "Include system audio · On".into()
                } else {
                    "Include system audio · Off".into()
                };
            }
            Message::StartShare => {
                self.idle.activity();
                if self.share_active {
                    self.status = "Already sharing".into();
                } else {
                    let audio = self.share_include_audio;
                    self.status = "Starting share…".into();
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || screencast::start_blocking(audio))
                                .await
                                .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        Message::ShareStarted,
                    );
                }
            }
            Message::StopShare => {
                self.idle.activity();
                screencast::stop();
                self.share_active = false;
                self.status = "Sharing stopped".into();
            }
            Message::ShareStarted(result) => {
                match result {
                    Ok(label) => {
                        self.share_active = true;
                        self.status = format!("Sharing · {label}");
                    }
                    Err(e) => {
                        self.share_active = false;
                        self.status = format!("Share failed · {e}");
                    }
                }
            }
            Message::ProjectMode(mode) => {
                self.idle.activity();
                if self.outputs.is_empty() {
                    self.outputs = load_outputs_snapshot();
                }
                match connect_view::apply_project(&self.outputs, mode) {
                    Ok(s) => self.status = s,
                    Err(e) => self.status = e,
                }
            }
            Message::ToggleAllApps => {
                self.idle.activity();
                self.start_all_apps = true;
                self.all_apps_jump = None;
                self.status = "All apps".into();
            }
            Message::AllAppsBack => {
                self.idle.activity();
                self.start_all_apps = false;
                self.all_apps_jump = None;
                self.status = "Start".into();
            }
            Message::SnapLayout(kind) => {
                self.idle.activity();
                self.write_snap_req(&kind);
                self.snapflyout_open = false;
                if kind == "left" || kind == "right" {
                    self.snap_assist_side = Some(kind.clone());
                    self.snap_assist_until = Some(Instant::now() + Duration::from_secs(8));
                    self.refresh_windows();
                } else {
                    self.snap_assist_side = None;
                    self.snap_assist_until = None;
                }
                self.status = format!("Snap {kind}");
            }
            Message::SnapAssistPick(index) => {
                self.idle.activity();
                let opposite = match self.snap_assist_side.as_deref() {
                    Some("left") => "right",
                    Some("right") => "left",
                    _ => {
                        self.snap_assist_side = None;
                        self.snap_assist_until = None;
                        self.status = "Snap assist closed".into();
                        return self.sync_overlay();
                    }
                };
                self.request_focus_index(index);
                self.write_snap_req(opposite);
                self.snap_assist_side = None;
                self.snap_assist_until = None;
                let title = self
                    .windows
                    .get(index)
                    .map(|w| w.title.as_str())
                    .unwrap_or("window");
                self.status = format!("Snap assist · {title} → {opposite}");
            }
            Message::TogglePinOverflow => {
                self.idle.activity();
                let open = !self.pin_overflow_open;
                self.close_flyouts();
                self.pin_overflow_open = open;
                self.status = if open {
                    "Pin overflow".into()
                } else {
                    "Pin overflow closed".into()
                };
            }
            Message::ToggleWinOverflow => {
                self.idle.activity();
                let open = !self.win_overflow_open;
                self.close_flyouts();
                self.win_overflow_open = open;
                if open {
                    self.refresh_windows();
                }
                self.status = if open {
                    "Window overflow".into()
                } else {
                    "Window overflow closed".into()
                };
            }
            Message::UnpinTaskbar(id) => {
                self.idle.activity();
                self.jump_list_pin = None;
                self.app_ctx = None;
                let pins = &mut self.cfg.active_mut().shell.pins;
                let before = pins.len();
                pins.retain(|p| p != &id);
                if pins.len() != before {
                    let _ = self.cfg.save();
                    self.status = format!("Unpinned {id}");
                } else {
                    self.status = format!("{id} was not pinned");
                }
            }
            Message::CreateStartFolder(pin) => {
                self.idle.activity();
                if pin.starts_with("mde-folder:") {
                    self.status = "Already a folder".into();
                    return Task::none();
                }
                let base = apps::resolve_desktop(&pin)
                    .map(|p| apps::desktop_name(&p))
                    .unwrap_or_else(|| pin.clone());
                let mut name = format!("{base} folder");
                let mut n = 2u32;
                while self
                    .cfg
                    .active()
                    .start
                    .pins
                    .iter()
                    .any(|p| p == &format!("mde-folder:{name}"))
                {
                    name = format!("{base} folder {n}");
                    n += 1;
                }
                let folder_id = format!("mde-folder:{name}");
                let pins = &mut self.cfg.active_mut().start.pins;
                if let Some(i) = pins.iter().position(|p| p == &pin) {
                    pins[i] = folder_id.clone();
                } else {
                    pins.push(folder_id.clone());
                }
                let _ = self.cfg.save();
                self.start_folder_set_apps(&folder_id, vec![pin]);
                self.status = format!("Folder · {name}");
                self.app_ctx = None;
            }
            Message::DissolveStartFolder(folder_id) => {
                self.idle.activity();
                let apps = self.start_folder_apps(&folder_id);
                let pins = &mut self.cfg.active_mut().start.pins;
                if let Some(i) = pins.iter().position(|p| p == &folder_id) {
                    pins.remove(i);
                    for (k, app) in apps.into_iter().enumerate() {
                        pins.insert(i + k, app);
                    }
                }
                // drop extras entry
                if let Some(toml::Value::Array(arr)) = self.cfg.extras.get_mut("start.pin_folders") {
                    let name = folder_id.strip_prefix("mde-folder:").unwrap_or(&folder_id);
                    arr.retain(|item| {
                        item.get("name")
                            .and_then(|v| v.as_str())
                            .map(|n| !n.eq_ignore_ascii_case(name))
                            .unwrap_or(true)
                    });
                }
                let _ = self.cfg.save();
                self.start_folder_open = None;
                self.app_ctx = None;
                self.status = "Folder dissolved".into();
            }
            Message::ToggleStartFolderExpand(id) => {
                self.idle.activity();
                if self.start_folder_open.as_ref() == Some(&id) {
                    self.start_folder_open = None;
                } else {
                    self.start_folder_open = Some(id);
                }
            }
            Message::UnpinStart(id) => {
                self.idle.activity();
                self.app_ctx = None;
                let pins = &mut self.cfg.active_mut().start.pins;
                let before = pins.len();
                pins.retain(|p| p != &id);
                if pins.len() != before {
                    let _ = self.cfg.save();
                    self.status = format!("Unpinned from Start · {id}");
                } else {
                    self.status = format!("{id} was not on Start");
                }
            }
            Message::MoveToAdvanced(id) => {
                self.idle.activity();
                self.app_ctx = None;
                self.status = self.move_start_pin_to_advanced(&id);
            }
            Message::PinToStart(id) => {
                self.idle.activity();
                self.app_ctx = None;
                let pins = &mut self.cfg.active_mut().start.pins;
                if pins.iter().any(|p| p == &id) {
                    self.status = format!("Already on Start · {id}");
                } else {
                    pins.push(id.clone());
                    let _ = self.cfg.save();
                    self.status = format!("Pinned to Start · {id}");
                }
            }
            Message::OpenJumpList(id) => {
                self.idle.activity();
                let open = self.jump_list_pin.as_ref() != Some(&id);
                self.close_flyouts();
                if open {
                    self.jump_list_pin = Some(id.clone());
                    self.status = format!("Jump list · {id}");
                } else {
                    self.status = "Jump list closed".into();
                }
            }
            Message::JumpListUnpinRecent(path) => {
                self.idle.activity();
                recent::dismiss(&path);
                if let Some(pin) = self.jump_list_pin.clone() {
                    tools::jump_unpin_forever(&pin, &path);
                }
                self.recent = recent::load(8);
                self.status = format!("Unpinned recent · {path}");
            }
            Message::OpenAppCtx { kind, id } => {
                self.idle.activity();
                let open = self
                    .app_ctx
                    .as_ref()
                    .is_none_or(|c| c.kind != kind || c.id != id);
                self.close_flyouts();
                if open {
                    self.app_ctx = Some(AppCtx { kind, id: id.clone() });
                    self.status = format!("App menu · {id}");
                } else {
                    self.status = "App menu closed".into();
                }
            }
            Message::OpenDesktopMenu => {
                self.idle.activity();
                let open = !self.desktop_menu_open;
                self.close_flyouts();
                self.desktop_menu_open = open;
                self.status = if open {
                    "Desktop menu".into()
                } else {
                    "Desktop menu closed".into()
                };
            }
            Message::OpenSysMenu => {
                self.idle.activity();
                let open = !self.sysmenu_open;
                self.close_flyouts();
                self.sysmenu_open = open;
                self.status = if open {
                    "System menu".into()
                } else {
                    "System menu closed".into()
                };
            }
            Message::DesktopIconSize(size) => {
                self.idle.activity();
                self.desktop_menu_open = false;
                self.cfg
                    .extras
                    .insert("desktop.icon_size".into(), toml::Value::String(size.into()));
                let _ = self.cfg.save();
                self.status = format!("Desktop icons · {size}");
            }
            Message::DesktopSortBy(sort) => {
                self.idle.activity();
                self.desktop_menu_open = false;
                self.cfg
                    .extras
                    .insert("desktop.sort".into(), toml::Value::String(sort.into()));
                let _ = self.cfg.save();
                self.status = format!("Desktop sort · {sort}");
            }
            Message::DesktopNewFolder => {
                self.idle.activity();
                self.desktop_menu_open = false;
                match files::desktop_new_folder() {
                    Ok(path) => self.status = format!("New folder · {path}"),
                    Err(e) => self.status = e,
                }
            }
            Message::DesktopNewShortcut => {
                self.idle.activity();
                self.desktop_menu_open = false;
                match files::desktop_new_shortcut() {
                    Ok(path) => self.status = format!("New shortcut · {path}"),
                    Err(e) => self.status = e,
                }
            }
            Message::ToggleTaskbarLock => {
                self.idle.activity();
                let on = !self.taskbar_locked();
                self.cfg
                    .extras
                    .insert("shell.taskbar_locked".into(), toml::Value::Boolean(on));
                let _ = self.cfg.save();
                self.taskbar_menu_open = false;
                self.status = if on {
                    "Taskbar locked".into()
                } else {
                    "Taskbar unlocked".into()
                };
            }
            Message::UninstallPrompt => {
                self.idle.activity();
                self.uninstall_confirm = true;
                self.status = "Uninstall · confirm?".into();
            }
            Message::CancelUninstall => {
                self.uninstall_confirm = false;
                self.status = "Uninstall cancelled".into();
            }
            Message::ConfirmUninstall => {
                self.idle.activity();
                self.uninstall_confirm = false;
                let id = self
                    .app_ctx
                    .as_ref()
                    .map(|c| c.id.clone())
                    .unwrap_or_default();
                self.app_ctx = None;
                self.status = uninstall_app(&id);
            }
            Message::DesktopMenuRefresh => {
                self.idle.activity();
                self.desktop_menu_open = false;
                self.recent = recent::load(8);
                self.refresh_windows();
                self.status = "Desktop · refreshed".into();
            }
            Message::JumpListOpenFile(path) => {
                self.idle.activity();
                self.jump_list_pin = None;
                let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                self.status = format!("Open {path}");
                self.close_flyouts();
            }
            Message::ReorderStartPin { id, delta } => {
                self.idle.activity();
                let pins = &mut self.cfg.active_mut().start.pins;
                if let Some(idx) = pins.iter().position(|p| p == &id) {
                    let dest = (idx as i32 + delta).clamp(0, pins.len().saturating_sub(1) as i32) as usize;
                    if dest != idx {
                        let item = pins.remove(idx);
                        pins.insert(dest, item);
                        let _ = self.cfg.save();
                        self.status = format!("Moved pin {}", if delta < 0 { "up" } else { "down" });
                    }
                }
            }
            Message::ReorderShellPin { id, delta } => {
                self.idle.activity();
                let pins = &mut self.cfg.active_mut().shell.pins;
                if let Some(idx) = pins.iter().position(|p| p == &id) {
                    let dest = (idx as i32 + delta).clamp(0, pins.len().saturating_sub(1) as i32) as usize;
                    if dest != idx {
                        let item = pins.remove(idx);
                        pins.insert(dest, item);
                        let _ = self.cfg.save();
                        self.status = format!(
                            "Taskbar pin {}",
                            if delta < 0 { "moved up" } else { "moved down" }
                        );
                    }
                }
            }
            Message::MuteNotifApp(app) => {
                self.idle.activity();
                notifications::mute_app(&app, &mut self.notifs.muted_apps);
                self.hub.set_muted_apps(self.notifs.muted_apps.clone());
                // Drop existing toasts/list entries from this app.
                let mute = app.clone();
                self.notifs
                    .items
                    .retain(|n| !notifications::is_muted_app(&n.app_name, &self.notifs.muted_apps));
                self.notifs.persist();
                if self
                    .toast
                    .as_ref()
                    .is_some_and(|t| notifications::is_muted_app(&t.app_name, &self.notifs.muted_apps))
                {
                    self.toast = self.toast_stack.take();
                    self.toast_until = self.toast_stack_until.take();
                    self.toast_expanded = false;
                }
                if self.toast_stack.as_ref().is_some_and(|t| {
                    notifications::is_muted_app(&t.app_name, &self.notifs.muted_apps)
                }) {
                    self.toast_stack = None;
                    self.toast_stack_until = None;
                }
                self.status = format!("Muted {mute}");
            }
            Message::UnmuteNotifApp(app) => {
                self.idle.activity();
                notifications::unmute_app(&app, &mut self.notifs.muted_apps);
                self.hub.set_muted_apps(self.notifs.muted_apps.clone());
                self.status = format!("Unmuted {app}");
            }
            Message::ToggleAutoBright => {
                self.idle.activity();
                let on = !self.auto_bright_enabled();
                self.cfg
                    .extras
                    .insert("qs.auto_brightness".into(), toml::Value::Boolean(on));
                let _ = self.cfg.save();
                if on {
                    if let Some(msg) = self.apply_dynamic_brightness(true) {
                        self.status = msg;
                    } else {
                        self.status = "Dynamic brightness · On".into();
                    }
                } else {
                    self.status = "Dynamic brightness · Off".into();
                }
            }
            Message::ProjectStub => {
                // Legacy alias → Connect flyout (W449).
                self.idle.activity();
                let open = !self.connect_open;
                self.close_flyouts();
                self.connect_open = open;
                if open && self.outputs.is_empty() {
                    self.outputs = load_outputs_snapshot();
                }
                self.status = "Wireless display".into();
            }
            Message::OpenSessionLog => {
                self.idle.activity();
                match open_session_log() {
                    Ok(path) => {
                        self.status = format!("Session log · {path}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::SearchWeb => {
                // A084: web search removed — local results only.
                self.idle.activity();
                self.status = "Search is local only (no web)".into();
            }
            Message::CloseWindow(index) => {
                self.idle.activity();
                self.request_focus_index(index);
                self.write_window_action("close");
                let title = self
                    .windows
                    .get(index)
                    .map(|w| w.title.clone())
                    .unwrap_or_else(|| "window".into());
                self.status = format!("Close {title}");
            }
            Message::EditQsTiles => {
                self.idle.activity();
                if self.quick_open {
                    self.qs_edit = true;
                    self.status = "Edit quick settings".into();
                }
            }
            Message::QsEditDone => {
                self.idle.activity();
                self.qs_edit = false;
                self.status = "Quick Settings".into();
            }
            Message::QsAddTile(id) => {
                self.idle.activity();
                let mut tiles = self.qs_tiles();
                if tiles.len() >= QS_TILES_MAX {
                    self.status = format!("At most {QS_TILES_MAX} quick settings");
                } else if tiles.iter().any(|t| t == &id) {
                    self.status = "Already pinned".into();
                } else if qs_catalog_label(&id).is_none() {
                    self.status = "Unknown tile".into();
                } else {
                    tiles.push(id.clone());
                    self.persist_qs_tiles(&tiles);
                    self.status = format!("Added {}", qs_catalog_label(&id).unwrap_or("tile"));
                }
            }
            Message::QsRemoveTile(id) => {
                self.idle.activity();
                let mut tiles = self.qs_tiles();
                if tiles.len() <= 1 {
                    self.status = "Keep at least one tile".into();
                } else if let Some(pos) = tiles.iter().position(|t| t == &id) {
                    tiles.remove(pos);
                    self.persist_qs_tiles(&tiles);
                    self.status = format!("Removed {}", qs_catalog_label(&id).unwrap_or("tile"));
                }
            }
            Message::ClipQuery(s) => {
                self.idle.activity();
                self.clip_query = s;
            }
            Message::UpdateAndRestart => {
                self.idle.activity();
                self.status = trigger_offline_update("reboot");
                self.close_flyouts();
            }
            Message::OpenAppMixer => {
                self.idle.activity();
                let open = !self.app_mixer_open;
                self.close_flyouts();
                self.app_mixer_open = open;
                if open {
                    self.app_streams = system::app_streams();
                }
                self.status = if open {
                    format!("App mixer ({} streams)", self.app_streams.len())
                } else {
                    "App mixer closed".into()
                };
            }
            Message::AppStreamVol(id, delta) => {
                self.idle.activity();
                match system::node_volume(id, delta) {
                    Ok(()) => {
                        self.app_streams = system::app_streams();
                        self.status = format!("App vol {id} {delta:+}%");
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::AppStreamMute(id) => {
                self.idle.activity();
                match system::node_mute(id) {
                    Ok(()) => {
                        self.app_streams = system::app_streams();
                        self.status = format!("App mute {id}");
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::OpenVolumeMixer => {
                // W238: classic mixer — pavucontrol / pwvucontrol if present.
                self.idle.activity();
                self.status = open_volume_mixer();
            }
            Message::CyclePhotos => {
                // W239: cycle Widgets photos card through ~/Pictures.
                self.idle.activity();
                match widgets::next_picture(self.photos_path.as_deref()) {
                    Some(path) => {
                        let name = std::path::Path::new(&path)
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.clone());
                        self.photos_path = Some(path);
                        self.status = format!("Photos · {name}");
                    }
                    None => self.status = "Photos · no images in ~/Pictures".into(),
                }
            }
            Message::SaveClipImage(id) => {
                self.idle.activity();
                match self.clipboard.save_image_to_pictures(id) {
                    Ok(path) => self.status = format!("Saved · {path}"),
                    Err(e) => self.status = e,
                }
            }
            Message::OpenRecommendedLocation(id) => {
                self.idle.activity();
                self.status = open_recommended_location(&id);
                if !self.status.contains("failed") {
                    self.close_flyouts();
                }
            }
            Message::DismissAllToasts => {
                // W236: clear primary + stacked toasts.
                self.idle.activity();
                self.toast = None;
                self.toast_until = None;
                self.toast_stack = None;
                self.toast_stack_until = None;
                self.toast_expanded = false;
                self.status = "Toasts dismissed".into();
            }
            Message::PasteClipPlain(id) => match self.clipboard.paste_plain(id) {
                Ok(()) => {
                    self.idle.activity();
                    self.status = format!("Pasted plain text #{id}");
                }
                Err(e) => self.status = e,
            },
            Message::TogglePinnedCollapse => {
                self.idle.activity();
                self.pins_collapsed = !self.pins_collapsed;
                self.status = if self.pins_collapsed {
                    "Pinned collapsed".into()
                } else {
                    "Pinned expanded".into()
                };
            }
            Message::TogglePowerMenu => {
                self.idle.activity();
                self.power_menu_open = !self.power_menu_open;
                if self.power_menu_open {
                    self.account_menu_open = false;
                } else {
                    self.power_confirm = None;
                }
                self.status = if self.power_menu_open {
                    "Power".into()
                } else {
                    "Start".into()
                };
            }
            Message::ToggleAccountMenu => {
                self.idle.activity();
                self.account_menu_open = !self.account_menu_open;
                if self.account_menu_open {
                    self.power_menu_open = false;
                    self.power_confirm = None;
                }
                self.status = if self.account_menu_open {
                    "Account".into()
                } else {
                    "Start".into()
                };
            }
            Message::ToggleStartMore => {
                self.idle.activity();
                self.start_more = !self.start_more;
                self.status = if self.start_more {
                    "Start more".into()
                } else {
                    "Start".into()
                };
            }
            Message::SnoozeNotif { id, secs } => {
                self.idle.activity();
                if self.notifs.snooze(id, secs) {
                    let mins = secs / 60;
                    self.status = format!("Snoozed {mins}m");
                } else {
                    self.status = "Snooze failed".into();
                }
            }
            Message::ToggleAutoHide => {
                self.idle.activity();
                let on = !self.cfg.active().shell.taskbar_autohide;
                self.cfg.active_mut().shell.taskbar_autohide = on;
                let _ = self.cfg.save();
                // Exclusive zone is fixed at layer create — respawn for a correct reserve.
                self.status = if on {
                    "Auto-hide · On · restarting shell…".into()
                } else {
                    "Auto-hide · Off · restarting shell…".into()
                };
                let _ = self.sync_overlay();
                std::process::exit(0);
            }
            Message::ToggleWidgetsBtnFromMenu => {
                self.idle.activity();
                let on = !self.cfg.active().shell.widgets_btn;
                self.cfg.active_mut().shell.widgets_btn = on;
                let _ = self.cfg.save();
                self.taskbar_menu_open = false;
                self.status = if on {
                    "Widgets button · On".into()
                } else {
                    "Widgets button · Off".into()
                };
            }
            Message::ToggleTaskView => {
                self.idle.activity();
                let open = !self.taskview_open;
                self.close_flyouts();
                self.taskview_open = open;
                if open {
                    self.refresh_windows();
                    self.status = "Task View".into();
                } else {
                    self.status = "Task View closed".into();
                }
            }
            Message::ActivateTaskbarApp { pin, indices } => {
                self.idle.activity();
                // Skip click-activate when this press was a pin drag.
                if self.pin_drag_moved {
                    self.pin_drag_moved = false;
                } else {
                    self.activate_taskbar_app(pin, indices);
                }
            }
            Message::BarHoverEnter => {
                self.bar_hover = true;
                self.bar_hide_after = None;
                return self.sync_bar_geometry();
            }
            Message::BarHoverExit => {
                self.bar_hover = false;
                // A009: Win11-like delay before collapsing (flyouts keep it up via flyout_open).
                self.bar_hide_after = Some(Instant::now() + Duration::from_millis(450));
                return self.sync_bar_geometry();
            }
            Message::StartPinEnter(id) => {
                // Hover alone must never reorder or create folders — that left icons
                // stuck and turned selected pins into `mde-folder:` tiles.
                self.start_pin_hover = Some(id.clone());
                if self.taskbar_locked() || !self.pin_down {
                    return Task::none();
                }
                if self.start_pin_drag_src.is_none() {
                    self.start_pin_drag_src = Some(id);
                    return Task::none();
                }
                // Ignore enter events on the press-down tile until the pointer has
                // actually left it (click / layout jitter must not reshuffle pins).
                if !self.start_pin_drag_armed {
                    return Task::none();
                }
                if self.start_pin_drag_src.as_ref() != Some(&id) {
                    let src = self.start_pin_drag_src.clone().unwrap_or_default();
                    // W453: drop onto a folder pin → nest; else reorder.
                    // Folder creation is via context menu (CreateStartFolder), not hover-drop.
                    if id.starts_with("mde-folder:") && !src.starts_with("mde-folder:") {
                        let mut apps = self.start_folder_apps(&id);
                        if !apps.iter().any(|a| a == &src) {
                            apps.push(src.clone());
                            self.start_folder_set_apps(&id, apps);
                        }
                        let pins = &mut self.cfg.active_mut().start.pins;
                        pins.retain(|p| p != &src);
                        let _ = self.cfg.save();
                        self.start_pin_drag_moved = true;
                        self.start_pin_drag_src = Some(id.clone());
                        self.start_pin_drag_armed = false; // require leave again before next drop
                        self.status = "Added to folder".into();
                    } else {
                        self.reorder_start_pin_before(&src, &id);
                        self.start_pin_drag_moved = true;
                        self.start_pin_drag_src = Some(id);
                        self.start_pin_drag_armed = false;
                        self.status = "Start pin moved".into();
                    }
                }
            }
            Message::StartPinExit => {
                if self.pin_down && self.start_pin_drag_src.is_some() {
                    self.start_pin_drag_armed = true;
                }
            }
Message::PinBarEnter(id) => {
                let label = apps::resolve_desktop(&id)
                    .map(|p| apps::desktop_name(&p))
                    .unwrap_or_else(|| id.clone());
                self.hover_pin = Some(id.clone());
                if !self.pin_down {
                    self.status = label;
                }
                if self.pin_down {
                    if self.pin_drag_src.is_none() {
                        self.pin_drag_src = Some(id);
                    } else if self.pin_drag_src.as_ref() != Some(&id) {
                        let src = self.pin_drag_src.clone().unwrap_or_default();
                        self.reorder_shell_pin_before(&src, &id);
                        self.pin_drag_moved = true;
                        self.pin_drag_src = Some(id);
                    }
                }
            }
            Message::PinBarExit => {
                // Keep hover_pin until another pin enter / global up clears it.
            }
            Message::PinGlobalDown => {
                if self.taskbar_locked() {
                    self.pin_down = false;
                    self.pin_drag_src = None;
                    self.start_pin_drag_src = None;
                    self.status = "Taskbar locked".into();
                } else {
                    self.pin_down = true;
                    self.pin_drag_moved = false;
                    self.pin_drag_src = self.hover_pin.clone();
                    self.start_pin_drag_moved = false;
                    self.start_pin_drag_armed = false;
                    self.start_pin_drag_src = self.start_pin_hover.clone();
                }
            }
            Message::PinGlobalUp => {
                // Click activate comes from the icon button. If the pointer dragged
                // across pins, keep pin_drag_moved / start_pin_drag_moved so activate is swallowed.
                self.pin_down = false;
                self.pin_drag_src = None;
                self.start_pin_drag_src = None;
                self.start_pin_hover = None;
                self.start_pin_drag_armed = false;
            }
            Message::WinMenuMoveDesktop(desk) => {
                self.idle.activity();
                // Focused window is already compositor focus; write move N (W131).
                self.write_workspace_req(&format!("move {desk}"));
                self.winmenu_open = false;
                self.status = format!("Move → Desktop {}", desk + 1);
            }
            Message::ClearRecommended => {
                self.idle.activity();
                let ids: Vec<String> = self.recent.iter().map(|r| r.id.clone()).collect();
                let n = recent::dismiss_all(&ids);
                self.recent = recent::load(8);
                self.status = format!("Recommended cleared · {n}");
            }
            Message::SafeRemoveUsb => {
                self.idle.activity();
                self.status = "Safely remove · use Files / eject".into();
            }
            Message::ToggleToastExpand => {
                self.idle.activity();
                self.toast_expanded = !self.toast_expanded;
                if self.toast_until.is_some() {
                    self.toast_until =
                        Some(Instant::now() + Duration::from_secs(self.toast_secs()));
                }
            }
            Message::RunQuery(s) => {
                self.idle.activity();
                self.run_query = s;
                self.palette_hits = build_palette_hits(self);
            }
            Message::RunSubmit => {
                self.idle.activity();
                let cmd = self.run_query.trim().to_string();
                if cmd.is_empty() {
                    self.status = "Run · enter a command".into();
                } else {
                    self.status = launch_run_command(&cmd);
                    self.run_open = false;
                    self.run_query.clear();
                }
            }
            Message::AllAppsJump(ch) => {
                self.idle.activity();
                if self.all_apps_jump == Some(ch) {
                    self.all_apps_jump = None;
                } else {
                    self.all_apps_jump = Some(ch);
                }
            }
            Message::ToggleNotifSounds => {
                self.idle.activity();
                let on = !self.notif_sounds_enabled();
                self.cfg
                    .extras
                    .insert("notifications.sounds".into(), toml::Value::Boolean(on));
                let _ = self.cfg.save();
                self.status = if on {
                    "Notification sounds · On".into()
                } else {
                    "Notification sounds · Off".into()
                };
            }
            Message::ToggleTaskbarLeft => {
                self.idle.activity();
                let left = !self.cfg.active().shell.taskbar_left;
                self.cfg.active_mut().shell.taskbar_left = left;
                self.cfg.active_mut().shell.taskbar_align =
                    if left { "left".into() } else { "center".into() };
                let _ = self.cfg.save();
                self.status = if left {
                    "Taskbar · Left".into()
                } else {
                    "Taskbar · Center".into()
                };
            }
            Message::ToggleTaskbarCombine => {
                // W197: Always ↔ Never; Never raises running-icon cap before overflow.
                self.idle.activity();
                let never = !self
                    .cfg
                    .active()
                    .shell
                    .taskbar_combine
                    .eq_ignore_ascii_case("never");
                self.cfg.active_mut().shell.taskbar_combine =
                    if never { "never".into() } else { "always".into() };
                let _ = self.cfg.save();
                self.status = if never {
                    "Combine taskbar · Never".into()
                } else {
                    "Combine taskbar · Always".into()
                };
            }
            Message::OpenEmojiPanel => {
                self.idle.activity();
                let open = !self.emoji_open;
                self.close_flyouts();
                self.emoji_open = open;
                self.status = if open { "Emoji".into() } else { "Emoji closed".into() };
            }
            Message::InsertEmoji(emoji) => {
                self.idle.activity();
                self.status = emoji_view::insert_emoji(&emoji);
            }
            Message::EmojiQuery(q) => {
                self.emoji_query = q;
            }
            Message::EmojiOverflow => {
                self.idle.activity();
                self.status = emoji_view::spawn_emoji_overflow();
            }
            Message::NewStartFolderStub => {
                // W264: pin Documents folder path (chooser hard → path string).
                self.idle.activity();
                self.status = self.pin_documents_folder();
            }
            Message::SetSearchType(kind) => {
                // W255: Apps / Documents / Web chips filter Start search sections.
                self.idle.activity();
                let next = kind.to_ascii_lowercase();
                self.search_type = if self.search_type == next {
                    String::new()
                } else {
                    next
                };
                self.status = match self.search_type.as_str() {
                    "apps" => "Search · Apps".into(),
                    "documents" => "Search · Documents".into(),
                    _ => "Search · All".into(),
                };
            }
            Message::ToggleTaskbarLabels => {
                // A005: real labels beside icons.
                self.idle.activity();
                let on = !self.taskbar_labels();
                self.cfg.active_mut().shell.taskbar_labels = on;
                self.cfg
                    .extras
                    .insert("taskbar.show_labels".into(), toml::Value::Boolean(on));
                let _ = self.cfg.save();
                self.status = if on {
                    "Show labels · On".into()
                } else {
                    "Show labels · Off".into()
                };
            }

            Message::ToggleNearbyFlyout => {
                self.idle.activity();
                let open = !self.nearby_flyout_open;
                self.close_flyouts();
                self.nearby_flyout_open = open;
                if open {
                    self.nearby = kdeconnect::refresh();
                }
                self.status = if open {
                    format!("Nearby Share · {}", self.nearby.hint)
                } else {
                    "Nearby closed".into()
                };
            }
            Message::RefreshNearby => {
                self.idle.activity();
                self.nearby = kdeconnect::refresh();
                self.status = format!("Nearby · {}", self.nearby.hint);
            }
            Message::NearbyPair(id) => {
                self.idle.activity();
                self.status = match kdeconnect::pair(&id) {
                    Ok(()) => {
                        self.nearby = kdeconnect::refresh();
                        format!("Pair requested · {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::NearbyUnpair(id) => {
                self.idle.activity();
                self.status = match kdeconnect::unpair(&id) {
                    Ok(()) => {
                        self.nearby = kdeconnect::refresh();
                        format!("Unpaired · {id}")
                    }
                    Err(e) => e,
                };
            }
            Message::NearbyAcceptPair(id) => {
                self.idle.activity();
                self.status = match kdeconnect::accept_pairing(&id) {
                    Ok(()) => {
                        self.nearby = kdeconnect::refresh();
                        "Pairing accepted".into()
                    }
                    Err(e) => e,
                };
            }
            Message::NearbyPing(id) => {
                self.idle.activity();
                self.status = match kdeconnect::ping(&id) {
                    Ok(()) => "Ping sent".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyRing(id) => {
                self.idle.activity();
                self.status = match kdeconnect::ring(&id) {
                    Ok(()) => "Ringing device…".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyShareClipboard(id) => {
                self.idle.activity();
                self.status = match kdeconnect::send_clipboard(&id) {
                    Ok(()) => "Clipboard sent".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyShareFile(id) => {
                self.idle.activity();
                // zenity/kdialog file picker
                let path = std::process::Command::new("zenity")
                    .args(["--file-selection", "--title=Share file"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    });
                self.status = match path {
                    Some(p) if !p.is_empty() => match kdeconnect::share_url_or_file(&id, &p) {
                        Ok(()) => format!("Shared · {p}"),
                        Err(e) => e,
                    },
                    _ => "Share cancelled".into(),
                };
            }
            Message::NearbyShareText(id) => {
                self.idle.activity();
                let text = std::process::Command::new("wl-paste")
                    .args(["--no-newline"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default();
                self.status = if text.is_empty() {
                    "Clipboard empty".into()
                } else {
                    match kdeconnect::share_text(&id, &text) {
                        Ok(()) => "Text shared".into(),
                        Err(e) => e,
                    }
                };
            }
            Message::NearbyMedia(id) => {
                self.idle.activity();
                self.status = match kdeconnect::media_play_pause(&id) {
                    Ok(()) => "Media · play/pause".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyLock(id) => {
                self.idle.activity();
                self.status = match kdeconnect::lock_device(&id) {
                    Ok(()) => "Lock sent".into(),
                    Err(e) => e,
                };
            }
            Message::NearbySendKeys(id) => {
                self.idle.activity();
                self.status = match kdeconnect::send_keys(&id, "Return") {
                    Ok(()) => "Remote key · Return".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyPresenter(id) => {
                self.idle.activity();
                self.status = match kdeconnect::presenter_pointer(&id) {
                    Ok(()) => "Presenter · pointer".into(),
                    Err(e) => e,
                };
            }
            Message::NearbyPickShare(id) => {
                self.idle.activity();
                if let Some(text) = self.nearby_pending_clip.take() {
                    self.status = match kdeconnect::share_text(&id, &text) {
                        Ok(()) => "Clip shared to Nearby".into(),
                        Err(e) => e,
                    };
                    self.nearby_flyout_open = false;
                } else {
                    self.status = match kdeconnect::send_clipboard(&id) {
                        Ok(()) => "Clipboard sent".into(),
                        Err(e) => e,
                    };
                }
            }
            Message::ClipShareNearbyPick => {
                self.idle.activity();
                // Capture selected/top clip text
                let text = self
                    .clipboard
                    .items
                    .first()
                    .map(|e| e.full.clone())
                    .unwrap_or_default();
                if text.is_empty() {
                    self.status = "Clipboard empty".into();
                } else {
                    self.nearby_pending_clip = Some(text);
                    self.close_flyouts();
                    self.nearby_flyout_open = true;
                    self.nearby = kdeconnect::refresh();
                    self.status = "Nearby · pick a device to send clip".into();
                }
            }
            Message::ClipShareNearbyTo(id) => {
                self.idle.activity();
                let text = self.nearby_pending_clip.take().unwrap_or_else(|| {
                    self.clipboard
                        .items
                        .first()
                        .map(|e| e.full.clone())
                        .unwrap_or_default()
                });
                self.status = if text.is_empty() {
                    "Clipboard empty".into()
                } else {
                    match kdeconnect::share_text(&id, &text) {
                        Ok(()) => format!("Shared clip → {id}"),
                        Err(e) => e,
                    }
                };
            }

            Message::ClipShareNearbyStub => {
                // W263 → W437: open Nearby device picker with pending clip.
                return self.update_inner(Message::ClipShareNearbyPick);
            }
            Message::TrayOverflowStub => {
                // W266 / W386: open tray flyout so hidden icons expose Menu (right-click path).
                self.idle.activity();
                self.close_flyouts();
                self.tray_open = true;
                let hidden: Vec<String> = self
                    .tray
                    .items
                    .iter()
                    .filter(|i| i.id.starts_with("sni:"))
                    .skip(4)
                    .map(|i| {
                        if i.title.is_empty() {
                            "•".into()
                        } else {
                            i.title.clone()
                        }
                    })
                    .collect();
                self.status = if hidden.is_empty() {
                    "System tray".into()
                } else {
                    format!("Hidden · {}", hidden.join(" · "))
                };
            }
            Message::NetSpeedHover => {
                // W441: real RX/TX rates from /sys/class/net.
                self.status = system::net_speed_status();
            }
            Message::VoiceSearchStub => {
                // W459: network STT with Settings opt-in (extras start.voice_search).
                self.idle.activity();
                let enabled = self
                    .cfg
                    .extras
                    .get("start.voice_search")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !enabled {
                    self.status =
                        "Voice search · enable in Settings › Start (network STT opt-in)".into();
                    return Task::none();
                }
                let url = self
                    .cfg
                    .extras
                    .get("start.voice_stt_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if url.is_empty() {
                    self.status =
                        "Voice search · set extras start.voice_stt_url to your STT endpoint".into();
                    return Task::none();
                }
                self.status = spawn_network_voice_search(&url);
            }
            Message::QuietHoursPreset(preset) => {
                self.idle.activity();
                self.set_quiet_hours_preset(&preset);
            }
            Message::RefreshLockWeather => {
                self.idle.activity();
                let line = widgets::read_weather_cache_line();
                self.status = if line.is_empty() {
                    "Weather · no cache".into()
                } else {
                    format!("Weather · {line}")
                };
            }
            Message::ClipMaxItemsDelta(delta) => {
                self.idle.activity();
                let cur = self.clip_max_items() as i64 + delta as i64;
                let next = cur.clamp(5, 100) as u32;
                self.set_clip_max_items(next);
                self.status = format!("Clipboard · max {next}");
            }
            Message::StartLiveTileStub => {
                // W460: live tiles declined — no-op if anything still emits this.
                self.idle.activity();
                self.status = "Live tiles · removed".into();
            }
            Message::EnergyRecTip => {
                // W274: energy recommendations tip.
                self.idle.activity();
                self.status = "Energy recommendations · tip".into();
            }
            Message::ReorderPinnedClip { id, delta } => {
                self.idle.activity();
                self.status = if self.clipboard.reorder_pinned(id, delta) {
                    if delta < 0 {
                        "Pinned clip moved up".into()
                    } else {
                        "Pinned clip moved down".into()
                    }
                } else {
                    "Pinned clip unchanged".into()
                };
            }
            Message::DismissToastId(id) => {
                if self.toast.as_ref().is_some_and(|t| t.id == id) {
                    self.toast = self.toast_stack.take();
                    self.toast_until = self.toast_stack_until.take();
                    self.toast_expanded = false;
                    self.status = "Toast dismissed".into();
                } else if self.toast_stack.as_ref().is_some_and(|t| t.id == id) {
                    self.toast_stack = None;
                    self.toast_stack_until = None;
                    self.status = "Toast dismissed".into();
                }
            }
            Message::ToastSecsDelta(delta) => {
                self.idle.activity();
                let cur = self.toast_secs() as i64 + delta as i64;
                let next = cur.clamp(1, 30) as u64;
                self.set_toast_secs(next);
                self.status = format!("Banner · {next}s");
            }
            Message::EndTask(index) => {
                self.idle.activity();
                self.request_focus_index(index);
                self.write_window_action("close");
                let title = self
                    .windows
                    .get(index)
                    .map(|w| w.title.clone())
                    .unwrap_or_else(|| "window".into());
                self.jump_list_pin = None;
                self.jump_list_win = None;
                self.status = format!("End task · {title}");
            }
            Message::EndTasks(indices) => {
                self.idle.activity();
                let n = indices.len();
                for &index in indices.iter().rev() {
                    self.request_focus_index(index);
                    self.write_window_action("close");
                }
                self.jump_list_pin = None;
                self.jump_list_win = None;
                self.status = format!("Close all · {n} windows");
            }
            Message::OpenWinJump(index) => {
                self.idle.activity();
                let open = self.jump_list_win != Some(index);
                self.close_flyouts();
                if open {
                    self.jump_list_win = Some(index);
                    self.status = "End task".into();
                } else {
                    self.status = "Jump list closed".into();
                }
            }
            Message::ToggleAlttabDesktop => {
                self.idle.activity();
                let on = !self.alttab_shows_desktop();
                self.cfg
                    .extras
                    .insert("alttab.show_desktop".into(), toml::Value::Boolean(on));
                let _ = self.cfg.save();
                self.status = if on {
                    "Alt+Tab shows desktop · On".into()
                } else {
                    "Alt+Tab shows desktop · Off".into()
                };
            }
            Message::UndoSnap => {
                self.idle.activity();
                self.write_window_action("restore");
                self.winmenu_open = false;
                self.snapflyout_open = false;
                self.status = "Snap undo".into();
            }
            Message::UpdateAndShutDown => {
                self.idle.activity();
                self.status = trigger_offline_update("power-off");
                self.close_flyouts();
            }
            Message::ResetThisPcTip => {
                self.idle.activity();
                self.status = "Use Fedora install media · not in MDE".into();
            }
            Message::ToastActionKey(idx) => {
                // W216: while toast visible, 1/2 → first/second action.
                if let Some(n) = self.toast.clone() {
                    if let Some((key, _)) = n.actions.get(idx) {
                        let key = key.clone();
                        let id = n.id;
                        return self.update_inner(Message::NotifAction(id, key));
                    }
                }
            }
            Message::PeekDesktopEnter => {
                // Hover peek disabled (Win11 taskbar parity pass).
            }
            Message::PeekDesktopExit => {
                self.peek_desktop_until = None;
            }
            Message::StartPinResizeStub => {
                // Legacy one-shot; prefer SetStartPinSize from Small/Large buttons.
                self.idle.activity();
                self.status = "Pin size · use Small / Large".into();
            }
            Message::SetStartPinSize { id, large } => {
                self.idle.activity();
                self.set_start_pin_large(&id, large);
                self.status = if large {
                    format!("Pin large · {id}")
                } else {
                    format!("Pin small · {id}")
                };
            }
            Message::ToggleNotifGroup(app) => {
                self.idle.activity();
                if self.nc_collapsed.contains(&app) {
                    self.nc_collapsed.remove(&app);
                    self.status = format!("{app} · expanded");
                } else {
                    self.nc_collapsed.insert(app.clone());
                    self.status = format!("{app} · collapsed");
                }
            }
            Message::CenterWindow => {
                self.idle.activity();
                self.write_snap_req("center");
                self.winmenu_open = false;
                self.status = "Window centered".into();
            }
            Message::RotateTip => {
                self.idle.activity();
                self.tip_idx = self.tip_idx.wrapping_add(1);
                self.status = widgets::tip_at(self.tip_idx).to_string();
            }
            Message::SupportBundleTip => {
                self.idle.activity();
                let path = support_bundle_path();
                let copied = std::process::Command::new("wl-copy")
                    .arg(&path)
                    .spawn()
                    .is_ok();
                self.status = if copied {
                    format!("Support bundle · {path} · copied")
                } else {
                    format!("Support bundle · {path}")
                };
            }
            Message::OpenCrashReports => {
                self.idle.activity();
                match open_crash_reports() {
                    Ok(path) => {
                        self.status = format!("Crash reports · {path}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::ClearNotifGroup(app) => {
                self.idle.activity();
                let removed = self.notifs.clear_group(&app);
                for id in &removed {
                    self.hub.queue_closed(*id, 2);
                }
                self.status = if removed.is_empty() {
                    format!("No notifications for {app}")
                } else {
                    format!("Cleared {} · {app}", removed.len())
                };
            }
            Message::OpenHiddenIcons => {
                // W398: open tray overflow; list overflow SNI + manage hide list.
                self.idle.activity();
                self.close_flyouts();
                self.tray_open = true;
                {
                    let ids: Vec<String> = self.tray.items.iter().map(|i| i.id.clone()).collect();
                    persist_tray_known(&ids);
                }
                let hidden = self.tray_hidden_ids();
                let overflow: Vec<String> = self
                    .tray
                    .items
                    .iter()
                    .filter(|i| i.id.starts_with("sni:"))
                    .skip(4)
                    .map(|i| {
                        if i.title.is_empty() {
                            i.id.clone()
                        } else {
                            i.title.clone()
                        }
                    })
                    .collect();
                self.status = if !hidden.is_empty() {
                    format!("Hidden icons · {}", hidden.join(" · "))
                } else if overflow.is_empty() {
                    "System tray · no overflow".into()
                } else {
                    format!("Tray overflow · {}", overflow.join(" · "))
                };
            }
            Message::ToggleTrayHidden(id) => {
                self.idle.activity();
                let mut hidden = self.tray_hidden_ids();
                if let Some(pos) = hidden.iter().position(|h| h == &id) {
                    hidden.remove(pos);
                    self.status = format!("Shown · {id}");
                } else {
                    hidden.push(id.clone());
                    self.status = format!("Hidden · {id}");
                }
                self.persist_tray_hidden(&hidden);
            }
            Message::ToggleBatterySaver => {
                self.idle.activity();
                let on = !self.battery_saver_on();
                match self.apply_battery_saver(on) {
                    Ok(msg) => self.status = msg,
                    Err(e) => self.status = e,
                }
            }
            Message::SetPowerProfile(profile) => {
                self.idle.activity();
                self.status = match system::set_power_profile(&profile) {
                    Ok(()) => {
                        let saver = profile.eq_ignore_ascii_case("power-saver");
                        self.cfg
                            .extras
                            .insert("qs.battery_saver".into(), toml::Value::Boolean(saver));
                        let _ = self.cfg.save();
                        format!("Power mode · {profile}")
                    }
                    Err(e) => e,
                };
            }
            Message::ToggleFocusTimer => {
                self.idle.activity();
                if self.focus_timer_until.is_some() {
                    self.end_focus_timer();
                    self.status = "Focus timer · ended".into();
                } else {
                    self.start_focus_timer(Duration::from_secs(25 * 60));
                    self.status = "Focus timer · 25m · DND on".into();
                }
            }
            Message::ToggleMicMute => {
                self.idle.activity();
                match system::mic_toggle_mute() {
                    Ok(muted) => {
                        self.mic_muted = muted;
                        self.show_osd(OsdState::mic(muted));
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::SwitchUser => {
                self.idle.activity();
                self.close_flyouts();
                // A057: single-user lock — sign out to greeter (not multi-seat switch).
                self.status = format!(
                    "Sign out to switch user · {}",
                    sign_out_best_effort()
                );
            }
            Message::AccountSignInStub => {
                // W408: open Accounts → Sign-in options in Settings.
                self.idle.activity();
                self.close_flyouts();
                self.launch_control("signin");
                self.status = "Settings · Sign-in options".into();
            }
            Message::FeedbackHubStub => {
                // A099: Feedback Hub entry removed.
                self.idle.activity();
                self.status = "Feedback Hub removed · use Settings › About".into();
                self.close_flyouts();
            }
            Message::NotifAppSettings(app) => {
                self.idle.activity();
                // A032: deep link to per-app prefs page with app preselect.
                let target = if app.trim().is_empty() {
                    "notifapps".to_string()
                } else {
                    format!("notifapps:{}", app.trim())
                };
                self.launch_control(&target);
                self.close_flyouts();
                self.status = if app.trim().is_empty() {
                    "Settings · App notifications".into()
                } else {
                    format!("Settings · App notifications · {app}")
                };
            }

            Message::PaletteSelect(action) => {
                self.idle.activity();
                self.status = run_palette_action(self, &action);
                if !action.starts_with("calc:") {
                    self.close_flyouts();
                }
            }
            Message::ToggleAwake => {
                self.idle.activity();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let active = tools::awake_is_active(&self.awake_state, now);
                if active {
                    self.awake_state = tools::AwakeState { active: false, until_unix: 0 };
                    self.idle_inhibited = false;
                    self.status = "Awake off".into();
                } else {
                    let mins = self.cfg.active().tools.awake_minutes;
                    let until = if mins == 0 { 0 } else { now + mins as u64 * 60 };
                    self.awake_state = tools::AwakeState { active: true, until_unix: until };
                    self.idle_inhibited = true;
                    self.status = if mins == 0 {
                        "Awake · until turned off".into()
                    } else {
                        format!("Awake · {mins} min")
                    };
                }
                tools::save_awake(&self.awake_state);
            }
            Message::SetAwakeMinutes(m) => {
                self.idle.activity();
                self.cfg.active_mut().tools.awake_minutes = m;
                let _ = self.cfg.save();
                self.status = format!("Awake default · {m} min");
            }
            Message::OpenBtFlyout => {
                self.idle.activity();
                let open = !self.bt_flyout_open;
                self.close_flyouts();
                self.bt_flyout_open = open;
                if open {
                    self.bt_devices = system::bt_device_list();
                    self.status = format!("Bluetooth · {} devices", self.bt_devices.len());
                }
            }
            Message::BtConnect(mac) => {
                self.idle.activity();
                self.status = match system::bt_connect_mac(&mac) {
                    Ok(()) => {
                        self.bt_devices = system::bt_device_list();
                        format!("BT connected · {mac}")
                    }
                    Err(e) => e,
                };
            }
            Message::BtDisconnect(mac) => {
                self.idle.activity();
                self.status = match system::bt_disconnect_mac(&mac) {
                    Ok(()) => {
                        self.bt_devices = system::bt_device_list();
                        format!("BT disconnected · {mac}")
                    }
                    Err(e) => e,
                };
            }
            Message::BtStartPair => {
                self.idle.activity();
                self.bt_pairing = true;
                self.status = match system::bt_scan_on() {
                    Ok(()) => {
                        self.bt_devices = system::bt_device_list_discovered();
                        "BT pairing · scanning…".into()
                    }
                    Err(e) => e,
                };
            }
            Message::BtPairDevice(mac) => {
                self.idle.activity();
                self.status = match system::bt_pair_mac(&mac) {
                    Ok(()) => {
                        self.bt_pairing = false;
                        self.bt_devices = system::bt_device_list();
                        format!("BT paired · {mac}")
                    }
                    Err(e) => e,
                };
            }
            Message::WifiAskPassword(ssid) => {
                self.idle.activity();
                self.wifi_password_ssid = ssid;
                self.wifi_password.clear();
                self.status = "Enter Wi-Fi password".into();
            }
            Message::WifiPasswordInput(s) => {
                self.idle.activity();
                self.wifi_password = s;
            }
            Message::WifiJoinWithPassword => {
                self.idle.activity();
                let ssid = self.wifi_password_ssid.clone();
                let pw = self.wifi_password.clone();
                self.status = match system::wifi_connect_password(&ssid, &pw) {
                    Ok(()) => {
                        self.wifi_password_ssid.clear();
                        self.wifi_password.clear();
                        self.wifi_aps = system::wifi_list(12);
                        format!("Joined · {ssid}")
                    }
                    Err(e) => e,
                };
            }
            Message::QsMoveTile(id, dir) => {
                self.idle.activity();
                let mut tiles = self.qs_tiles();
                if let Some(pos) = tiles.iter().position(|t| t == &id) {
                    let new_pos = if dir < 0 {
                        pos.saturating_sub(1)
                    } else {
                        (pos + 1).min(tiles.len().saturating_sub(1))
                    };
                    if new_pos != pos {
                        tiles.swap(pos, new_pos);
                        self.persist_qs_tiles(&tiles);
                        self.status = format!("Moved {}", qs_catalog_label(&id).unwrap_or("tile"));
                    }
                }
            }
            Message::JumpListPinForever(path) => {
                self.idle.activity();
                let pin = self.jump_list_pin.clone().unwrap_or_default();
                tools::jump_pin_forever(&pin, &path);
                self.status = format!("Pinned forever · {path}");
            }
            Message::JumpListClearRecent => {
                self.idle.activity();
                let pin = self.jump_list_pin.clone().unwrap_or_default();
                tools::jump_clear_recent_for_app(&pin);
                self.status = "Jump list cleared".into();
            }
            Message::SaveCurrentSnapGroup => {
                self.idle.activity();
                let members: Vec<tools::SnapGroupMember> = self
                    .windows
                    .iter()
                    .filter(|w| w.workspace == self.desktop_active)
                    .take(8)
                    .map(|w| tools::SnapGroupMember {
                        app_id: w.app_id.clone(),
                        title: w.title.clone(),
                        x: 0,
                        y: 0,
                        w: 0,
                        h: 0,
                    })
                    .collect();
                if members.len() < 2 {
                    self.status = "Snap Group · need 2+ windows".into();
                } else {
                    let id = format!("g{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
                    let label = members
                        .iter()
                        .map(|m| {
                            if m.title.is_empty() {
                                m.app_id.clone()
                            } else {
                                m.title.chars().take(18).collect()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" + ");
                    let group = tools::SnapGroup {
                        id,
                        label,
                        members,
                        saved_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                    };
                    tools::save_snap_group(group);
                    self.snap_groups = tools::load_snap_groups();
                    self.status = "Snap Group saved".into();
                }
            }
            Message::RestoreSnapGroup(id) => {
                self.idle.activity();
                if let Some(g) = self.snap_groups.iter().find(|g| g.id == id) {
                    match tools::write_restore_group(g) {
                        Ok(()) => self.status = format!("Restoring · {}", g.label),
                        Err(e) => self.status = e,
                    }
                }
            }
            Message::OpenSnapGroups => {
                self.idle.activity();
                let open = !self.snap_groups_open;
                self.close_flyouts();
                self.snap_groups_open = open;
                if open {
                    self.snap_groups = tools::load_snap_groups();
                }
            }
            Message::ApplyZone(idx) => {
                self.idle.activity();
                let tools_cfg = &self.cfg.active().tools;
                let layout = tools_cfg
                    .zone_layouts
                    .iter()
                    .find(|l| l.name == tools_cfg.active_zone_layout)
                    .or_else(|| tools_cfg.zone_layouts.first());
                if let Some(layout) = layout {
                    if let Some(z) = layout.zones.get(idx) {
                        match tools::write_zone_snap(idx, z.x, z.y, z.w, z.h) {
                            Ok(()) => self.status = format!("Zone {} · {}", idx + 1, layout.name),
                            Err(e) => self.status = e,
                        }
                    }
                }
                self.close_flyouts();
            }
            Message::OpenColorPickerHistory => {
                self.idle.activity();
                let open = !self.color_history_open;
                self.close_flyouts();
                self.color_history_open = open;
                if open {
                    self.color_history = tools::load_color_history(12);
                }
            }
            Message::CopyColorSample(value) => {
                self.idle.activity();
                let _ = std::process::Command::new("wl-copy").arg(&value).spawn();
                self.status = format!("Copied · {value}");
            }
            Message::FindMyMousePulse => {
                self.idle.activity();
                if self.cfg.active().tools.find_my_mouse {
                    self.find_mouse_ticks = 40;
                    self.status = "Find My Mouse".into();
                }
            }
            Message::ToggleMouseHighlighterLive => {
                self.idle.activity();
                self.mouse_highlighter_live = !self.mouse_highlighter_live;
                self.cfg.active_mut().tools.mouse_highlighter = self.mouse_highlighter_live;
                let _ = self.cfg.save();
                self.status = if self.mouse_highlighter_live {
                    "Highlighter on"
                } else {
                    "Highlighter off"
                }
                .into();
            }
            Message::AdvancedPaste(id, kind) => {
                self.idle.activity();
                self.status = match self.clipboard.items.iter().find(|e| e.id == id) {
                    Some(entry) => match tools::transform_clip(&kind, &entry.full) {
                        Ok(out) => {
                            let _ = std::process::Command::new("wl-copy").arg(&out).status();
                            format!("Advanced Paste · {kind}")
                        }
                        Err(e) => e,
                    },
                    None => "clip missing".into(),
                };
            }

            Message::CopyDiagnostics => {
                self.idle.activity();
                let text = session_diagnostics_line();
                let copied = std::process::Command::new("wl-copy")
                    .arg(&text)
                    .spawn()
                    .is_ok();
                self.status = if copied {
                    "Diagnostics copied".into()
                } else {
                    format!("Diagnostics · {text}")
                };
            }
            Message::PasteClipAsFileStub => {
                self.idle.activity();
                // Prefer selected/first clip id from history when stub message has no id.
                let id = self.clipboard.items.first().map(|e| e.id).unwrap_or(0);
                match self.clipboard.paste_as_file(id) {
                    Ok(s) => self.status = s,
                    Err(e) => self.status = e,
                }
            }
            Message::ElevateNotifPriority(app) => {
                self.idle.activity();
                notifications::elevate_priority_app(&app, &mut self.notifs.priority_apps);
                if notifications::is_muted_app(&app, &self.notifs.muted_apps) {
                    notifications::unmute_app(&app, &mut self.notifs.muted_apps);
                    self.hub.set_muted_apps(self.notifs.muted_apps.clone());
                }
                self.hub
                    .set_priority(self.notifs.priority, self.notifs.priority_apps.clone());
                self.status = format!("Priority · {app}");
            }
            Message::QuickLink(kind) => {
                self.idle.activity();
                match kind {
                    "terminal" => {
                        self.close_flyouts();
                        self.status = spawn_terminal();
                    }
                    "settings" => {
                        self.close_flyouts();
                        self.launch_control("");
                    }
                    "explorer" => match files::open_file_explorer() {
                        Ok(path) => {
                            self.close_flyouts();
                            self.status = format!("File Explorer · {path}");
                        }
                        Err(e) => self.status = e,
                    },
                    "run" => {
                        self.close_flyouts();
                        self.run_open = true;
                        self.run_query.clear();
                        self.status = "Run".into();
                    }
                    "desktop" => match files::open_user_folder("desktop") {
                        Ok(path) => {
                            self.close_flyouts();
                            self.status = format!("Desktop · {path}");
                        }
                        Err(e) => self.status = e,
                    },
                    "taskmgr" => {
                        self.close_flyouts();
                        self.status = spawn_task_manager();
                    }
                    "signout" => {
                        self.close_flyouts();
                        return self.update_inner(Message::Power(PowerAction::SignOut));
                    }
                    "sleep" => {
                        self.close_flyouts();
                        return self.update_inner(Message::Power(PowerAction::Suspend));
                    }
                    "shutdown" => {
                        self.close_flyouts();
                        return self.update_inner(Message::Power(PowerAction::PowerOff));
                    }
                    "restart" => {
                        self.close_flyouts();
                        return self.update_inner(Message::Power(PowerAction::Reboot));
                    }
                    "update-restart" => {
                        self.close_flyouts();
                        return self.update_inner(Message::UpdateAndRestart);
                    }
                    "update-shutdown" => {
                        self.close_flyouts();
                        return self.update_inner(Message::UpdateAndShutDown);
                    }
                    "disks" => {
                        self.close_flyouts();
                        match std::process::Command::new("mde-mmc").arg("diskmgmt.msc").spawn() {
                            Ok(_) => self.status = "Disk Management".into(),
                            Err(e) => {
                                self.status = spawn_admin_tool(
                                    "Disks",
                                    &["gnome-disks", "plasma-diskmanager", "gsmartcontrol"],
                                );
                                if self.status.contains("none found") {
                                    self.status = format!("Disks · mde-mmc: {e}");
                                }
                            }
                        }
                    }
                    "services" => {
                        self.close_flyouts();
                        match std::process::Command::new("mde-mmc").arg("services.msc").spawn() {
                            Ok(_) => self.status = "Services".into(),
                            Err(e) => {
                                self.status = spawn_admin_tool(
                                    "Services",
                                    &["gnome-system-monitor", "systemd-manager-qt", "stacer"],
                                );
                                if self.status.contains("none found") {
                                    self.status = format!("Services · mde-mmc: {e}");
                                }
                            }
                        }
                    }
                    "compmgmt" | "eventvwr" | "devmgmt" => {
                        self.close_flyouts();
                        let arg = match kind {
                            "eventvwr" => "eventvwr.msc",
                            "devmgmt" => "devmgmt.msc",
                            _ => "compmgmt.msc",
                        };
                        match std::process::Command::new("mde-mmc").arg(arg).spawn() {
                            Ok(_) => self.status = format!("MMC · {arg}"),
                            Err(e) => self.status = format!("mde-mmc: {e}"),
                        }
                    }
                    "netcon" => {
                        self.close_flyouts();
                        self.launch_control("network");
                        self.status = "Network connections".into();
                    }
                    _ => self.status = format!("Quick link · {kind}"),
                }
            }
            Message::ToggleClipSyncPause => {
                self.idle.activity();
                let next = !self.clipboard.sync_paused;
                self.status = self.clipboard.set_sync_paused(next);
            }
            Message::SetThemeAccent(hex) => {
                self.idle.activity();
                self.cfg.active_mut().theme.accent = hex.clone();
                match self.cfg.save() {
                    Ok(()) => {
                        self.palette = Palette::from_theme(&self.cfg.active().theme);
                        self.status = format!("Accent {hex}");
                    }
                    Err(e) => self.status = format!("Accent save failed: {e}"),
                }
            }
            Message::SwitchDesktop(n) => {
                self.idle.activity();
                self.write_workspace_req(&format!("switch {n}"));
                self.desktop_active = n;
                self.apply_desktop_wallpaper(n);
                let label = self.cfg.active().desktops.name_for(n);
                self.status = format!("{label}");
            }
            Message::NewDesktop => {
                self.idle.activity();
                self.write_workspace_req("new");
                self.status = "New desktop".into();
            }
            Message::CloseDesktop => {
                self.idle.activity();
                self.write_workspace_req("close");
                self.status = "Close desktop".into();
            }
            Message::MoveWindowToDesktop { window, desktop } => {
                self.idle.activity();
                // Focus the window first so compositor `move N` targets it, then move.
                self.request_focus_index(window);
                self.write_workspace_req(&format!("move {desktop}"));
                self.status = format!("Move → Desktop {}", desktop + 1);
            }
            Message::ToggleQuick => {
                self.idle.activity();
                let open = !self.quick_open;
                self.close_flyouts();
                self.quick_open = open;
                if open {
                    self.refresh_levels();
                    if self.media_available {
                        let m = system::media_status();
                        self.media_title = m.title;
                        self.media_playing = m.playing;
                    }
                    return Task::batch([
                        Task::done(Message::RefreshRadios),
                        self.sync_overlay(),
                    ]);
                }
                self.status = "Quick Settings closed".into();
            }
            Message::ToggleNotifications => {
                self.idle.activity();
                let open = !self.notifications_open;
                self.close_flyouts();
                self.notifications_open = open;
                self.status = format!(
                    "Notification centre ({} items, DND={})",
                    self.notifs.items.len(),
                    self.notifs.dnd
                );
            }
            Message::ToggleClipboard => {
                self.idle.activity();
                let open = !self.clipboard_open;
                self.close_flyouts();
                self.clipboard_open = open;
                if open {
                    // W143: drop unpinned clips older than 24h on open.
                    self.clipboard.drop_expired();
                    if let Ok(out) = std::process::Command::new("wl-paste")
                        .arg("-n")
                        .output()
                    {
                        if out.status.success() {
                            let text = String::from_utf8_lossy(&out.stdout);
                            if !text.is_empty() {
                                self.clipboard.push(&text);
                            }
                        }
                    }
                }
                self.status = format!("Clipboard history ({} items)", self.clipboard.items.len());
            }
            Message::OpenControl(page) => {
                self.idle.activity();
                self.launch_control(&page);
                self.close_flyouts();
            }
            Message::DemoNotify => {
                if let Some(id) = self.notifs.push_with_actions(
                    "mde-shell",
                    "Welcome to MDE",
                    "org.freedesktop.Notifications host is in-process",
                    1,
                    vec![("default".into(), "OK".into())],
                ) {
                    if let Some(n) = self.notifs.items.iter().find(|n| n.id == id).cloned() {
                        self.push_toast(n, false);
                        self.maybe_play_notif_sound();
                    }
                }
                self.status = "Demo notification posted".into();
            }
            Message::DismissNotif(id) => {
                self.notifs.dismiss(id);
                self.hub.queue_closed(id, 2);
            }
            Message::NotifAction(id, key) => {
                // W395: Open/Save on inbound share toasts — best-effort path from body.
                let body = self
                    .notifs
                    .items
                    .iter()
                    .find(|n| n.id == id)
                    .map(|n| n.body.clone())
                    .unwrap_or_default();
                if key == "open" || key == "save" {
                    let path = body
                        .lines()
                        .map(str::trim)
                        .find(|l| l.starts_with('/') || l.starts_with("file://"))
                        .unwrap_or(body.trim());
                    let path = path.strip_prefix("file://").unwrap_or(path);
                    if !path.is_empty() && std::path::Path::new(path).exists() {
                        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                        self.status = format!("Opened · {path}");
                    } else if key == "save" {
                        // Reveal Downloads if no explicit path.
                        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                        let dl = format!("{home}/Downloads");
                        let _ = std::process::Command::new("xdg-open").arg(&dl).spawn();
                        self.status = "Save · opened Downloads".into();
                    } else {
                        self.status = format!("Notification action {key}");
                    }
                } else {
                    self.status = format!("Notification action {key}");
                }
                self.hub.queue_invoke(id, key.clone());
                self.notifs.dismiss(id);
                self.hub.queue_closed(id, 2);
                self.dismiss_toast_id(id);
            }
            Message::ClearNotifs => {
                // W245: ask Yes/No once before clearing (like clipboard W89).
                self.notif_clear_confirm = true;
                self.status = "Clear notification history?".into();
            }
            Message::ConfirmClearNotifs => {
                self.notif_clear_confirm = false;
                // Soft undo: stash cleared list for 5s (W57).
                if !self.notifs.items.is_empty() {
                    self.cleared_stash = Some(self.notifs.items.clone());
                    self.clear_undo_until = Some(Instant::now() + Duration::from_secs(5));
                }
                let ids: Vec<u32> = self.notifs.items.iter().map(|n| n.id).collect();
                self.notifs.clear();
                for id in ids {
                    self.hub.queue_closed(id, 2);
                }
                self.status = "Notifications cleared · Undo available".into();
            }
            Message::CancelClearNotifs => {
                self.notif_clear_confirm = false;
                self.status = "Clear cancelled".into();
            }
            Message::UndoClearNotifs => {
                if let Some(items) = self.cleared_stash.take() {
                    self.notifs.items = items;
                    self.notifs.persist();
                    self.status = format!("Restored {} notifications", self.notifs.items.len());
                }
                self.clear_undo_until = None;
            }
            Message::PinToTaskbar(id) => {
                self.idle.activity();
                let pins = &mut self.cfg.active_mut().shell.pins;
                if pins.iter().any(|p| p == &id) {
                    self.status = format!("Already pinned · {id}");
                } else {
                    pins.push(id.clone());
                    let _ = self.cfg.save();
                    self.status = format!("Pinned {id}");
                }
            }
            Message::NudgeWindow(dir) => {
                self.idle.activity();
                self.winmenu_open = false;
                // W251: compositor applies nudge via mde-window-action.req.
                let action = match dir {
                    "left" => "nudge left",
                    "right" => "nudge right",
                    "up" => "nudge up",
                    "down" => "nudge down",
                    _ => {
                        self.status = "Nudge · Super+Alt+←/→".into();
                        return Task::none();
                    }
                };
                self.write_window_action(action);
                self.status = format!("Nudge {dir} · 20px");
            }
            Message::AccountPictureStub => {
                // W407: pick image → ~/.face (+ avatar.png) for greeter/lock/Start.
                self.idle.activity();
                self.status = set_account_picture();
                self.close_flyouts();
            }
            Message::CleanTempTip => {
                self.idle.activity();
                self.status = "Use Fedora utilities · tip".into();
            }
            Message::Power(action) => match action {
                PowerAction::Lock => {
                    self.engage_lock("Start → Lock");
                }
                PowerAction::Suspend | PowerAction::Reboot | PowerAction::PowerOff => {
                    // W46 / W193: confirm Sleep / Restart / Shut down in Start.
                    self.power_confirm = Some(action);
                    self.power_menu_open = true;
                    self.account_menu_open = false;
                    self.status = match action {
                        PowerAction::Suspend => "Confirm sleep?".into(),
                        PowerAction::Reboot => "Confirm restart?".into(),
                        _ => "Confirm shut down?".into(),
                    };
                }
                PowerAction::Hibernate => {
                    self.status = system::hibernate();
                    self.close_flyouts();
                }
                PowerAction::SignOut => {
                    self.status = sign_out_best_effort();
                    self.close_flyouts();
                }
            },
            Message::ConfirmPower => {
                if let Some(action) = self.power_confirm.take() {
                    match action {
                        PowerAction::Suspend => {
                            let _ = std::process::Command::new("mde-privileged")
                                .args(["power", "suspend"])
                                .spawn();
                            self.status = "Suspend".into();
                        }
                        PowerAction::Reboot => {
                            let _ = std::process::Command::new("mde-privileged")
                                .args(["power", "reboot"])
                                .spawn();
                            self.status = "Reboot".into();
                        }
                        PowerAction::PowerOff => {
                            let _ = std::process::Command::new("mde-privileged")
                                .args(["power", "poweroff"])
                                .spawn();
                            self.status = "Power off".into();
                        }
                        _ => {}
                    }
                    self.close_flyouts();
                }
            }
            Message::CancelPowerConfirm => {
                self.power_confirm = None;
                self.status = "Cancelled".into();
            }
            Message::OpenFileExplorer => {
                self.idle.activity();
                match files::open_file_explorer() {
                    Ok(path) => {
                        self.status = format!("File Explorer · {path}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::OpenUserFolder(kind) => {
                self.idle.activity();
                match files::open_user_folder(&kind) {
                    Ok(path) => {
                        self.status = format!("Opened {path}");
                        self.close_flyouts();
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::ToggleThemeDark => {
                self.idle.activity();
                let want_dark = !self.palette.dark;
                {
                    let theme = &mut self.cfg.active_mut().theme;
                    if want_dark {
                        theme.mode = "dark".into();
                    } else {
                        theme.mode = "light".into();
                        // Schedule would keep dark overnight — clear override for light.
                        theme.dark_schedule = false;
                    }
                }
                match self.cfg.save() {
                    Ok(()) => {
                        self.palette = Palette::from_theme(&self.cfg.active().theme);
                        system::apply_mackes_icon_theme(self.palette.dark);
                        self.status = format!("Theme {}", if self.palette.dark { "dark" } else { "light" });
                    }
                    Err(e) => self.status = format!("Theme save failed: {e}"),
                }
            }
            Message::MediaPlayPause => {
                self.idle.activity();
                match system::media_play_pause() {
                    Ok(()) => {
                        let m = system::media_status();
                        self.media_title = m.title;
                        self.media_playing = m.playing;
                        self.status = if self.media_playing {
                            "Playing".into()
                        } else {
                            "Paused".into()
                        };
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::MediaPrevious => {
                self.idle.activity();
                self.status = match system::media_previous() {
                    Ok(()) => {
                        let m = system::media_status();
                        self.media_title = m.title;
                        self.media_playing = m.playing;
                        "Previous track".into()
                    }
                    Err(e) => e,
                };
            }
            Message::MediaNext => {
                self.idle.activity();
                self.status = match system::media_next() {
                    Ok(()) => {
                        let m = system::media_status();
                        self.media_title = m.title;
                        self.media_playing = m.playing;
                        "Next track".into()
                    }
                    Err(e) => e,
                };
            }
            Message::MediaVolUp => {
                self.idle.activity();
                self.status = match system::volume_adjust(5) {
                    Ok(v) => format!("Volume {:.0}%", v * 100.0),
                    Err(e) => e,
                };
            }
            Message::MediaVolDown => {
                self.idle.activity();
                self.status = match system::volume_adjust(-5) {
                    Ok(v) => format!("Volume {:.0}%", v * 100.0),
                    Err(e) => e,
                };
            }
            Message::RequestSafeMode => {
                self.idle.activity();
                let home = std::env::var("HOME").unwrap_or_default();
                let dir = std::path::PathBuf::from(&home).join(".local/state/mde");
                let _ = std::fs::create_dir_all(&dir);
                let req = dir.join("safe.req");
                let _ = std::fs::write(&req, b"1\n");
                self.status =
                    "Safe mode: set MDE_SAFE=1 (or relaunch session). Wrote ~/.local/state/mde/safe.req for next login."
                        .into();
            }
            Message::HoverStatus(title) => {
                if !title.is_empty() {
                    self.peek_title = None;
                    self.status = title;
                }
            }
            Message::HoverPeek(_title) => {
                // Hover peek / thumbnails disabled — no status flash.
                self.peek_title = None;
            }
            Message::ExportConfig => {
                self.idle.activity();
                match MdeConfig::config_path() {
                    Ok(path) => {
                        let shown = path.display().to_string();
                        let copied = std::process::Command::new("wl-copy")
                            .arg(&shown)
                            .spawn()
                            .is_ok();
                        if !copied {
                            if let Some(parent) = path.parent() {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(parent)
                                    .spawn();
                            }
                        }
                        self.status = if copied {
                            format!("Config path copied · {shown}")
                        } else {
                            format!("Config · {shown}")
                        };
                    }
                    Err(e) => self.status = format!("Config path unavailable: {e}"),
                }
            }
            Message::AccessibilityStub => {
                // W403: launch working magnifier when possible; Settings remains a fallback.
                self.idle.activity();
                self.close_flyouts();
                self.status = spawn_magnifier(true);
            }
            Message::SetWifi(on) => {
                match system::set_wifi(on) {
                    Ok(()) => {
                        self.wifi_on = on;
                        // Turning Wi-Fi on clears airplane visually + in state (W58).
                        if on {
                            self.airplane = false;
                        }
                        self.persist_network();
                        self.status = format!("Wi-Fi {}", on_off(on));
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::SetBt(on) => {
                match system::set_bluetooth(on) {
                    Ok(()) => {
                        self.bt_on = on;
                        if on {
                            self.airplane = false;
                        }
                        self.persist_network();
                        self.status = format!("Bluetooth {}", on_off(on));
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::SetAirplane(on) => {
                match system::set_airplane(on) {
                    Ok(()) => {
                        self.airplane = on;
                        if on {
                            self.wifi_on = false;
                            self.bt_on = false;
                        } else {
                            // Leaving airplane: re-probe so tiles match radios.
                            let r = system::probe_radios();
                            self.wifi_on = r.wifi;
                            self.bt_on = r.bluetooth;
                        }
                        self.persist_network();
                        self.status = format!("Airplane {}", on_off(on));
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::VolUp => match system::volume_adjust(5) {
                Ok(level) => {
                    // Keep glyph sync even when flyout/QS suppress a second OSD (W61).
                    self.vol_level = level;
                    self.vol_muted = false;
                    if self.vol_flyout_open || self.quick_open {
                        self.status = format!("Volume {}%", (level * 100.0).round() as i32);
                    } else {
                        self.show_osd(OsdState::volume(level, false));
                    }
                }
                Err(e) => self.status = e,
            },
            Message::VolDown => match system::volume_adjust(-5) {
                Ok(level) => {
                    self.vol_level = level;
                    self.vol_muted = false;
                    if self.vol_flyout_open || self.quick_open {
                        self.status = format!("Volume {}%", (level * 100.0).round() as i32);
                    } else {
                        self.show_osd(OsdState::volume(level, false));
                    }
                }
                Err(e) => self.status = e,
            },
            Message::VolMute => match system::volume_toggle_mute() {
                Ok((level, muted)) => {
                    self.vol_level = level;
                    self.vol_muted = muted;
                    if muted {
                        // W230: brief critical flash on mute.
                        self.vol_mute_flash_until =
                            Some(Instant::now() + Duration::from_millis(900));
                    } else {
                        self.vol_mute_flash_until = None;
                    }
                    if self.vol_flyout_open || self.quick_open {
                        self.status = if muted {
                            "Volume muted".into()
                        } else {
                            format!("Volume {}%", (level * 100.0).round() as i32)
                        };
                    } else {
                        self.show_osd(OsdState::volume(level, muted));
                    }
                }
                Err(e) => self.status = e,
            },
            Message::BrightUp => match system::brightness_adjust(5) {
                Ok(level) => self.show_osd(OsdState {
                    kind: osd::OsdKind::Brightness,
                    level,
                    muted: false,
                }),
                Err(e) => self.status = e,
            },
            Message::BrightDown => match system::brightness_adjust(-5) {
                Ok(level) => self.show_osd(OsdState {
                    kind: osd::OsdKind::Brightness,
                    level,
                    muted: false,
                }),
                Err(e) => self.status = e,
            },
            Message::PollBus => {
                if let Some(menu) = self.menu_board.snapshot() {
                    self.tray_menu = Some(menu);
                }
                self.take_shell_cmd();
                self.apply_quiet_hours();
                if self.pending_start_focus && self.overlay_id.is_some() && (self.start_open || self.search_open) {
                    self.pending_start_focus = false;
                    // W186: flash Start search field border once.
                    self.search_highlight_until =
                        Some(Instant::now() + Duration::from_millis(700));
                    return Task::batch([
                        self.sync_overlay(),
                        iced::widget::text_input::focus(start_search_id()),
                    ]);
                }
                if self
                    .search_highlight_until
                    .is_some_and(|u| Instant::now() >= u)
                {
                    self.search_highlight_until = None;
                }
                if self
                    .vol_mute_flash_until
                    .is_some_and(|u| Instant::now() >= u)
                {
                    self.vol_mute_flash_until = None;
                }
                if self
                    .critical_led_until
                    .is_some_and(|u| Instant::now() >= u)
                {
                    self.critical_led_until = None;
                }
                // W424: ingest dictation result when pending.
                if self.dictation.pending {
                    if let Some(line) = dictation::read_result_if_ready() {
                        self.dictation.clear();
                        self.osd = None;
                        if line.is_empty()
                            || line.starts_with("STT HTTP")
                            || line.starts_with("no ")
                            || line.starts_with("No STT")
                        {
                            self.status = if line.is_empty() {
                                "Dictation · no text".into()
                            } else {
                                format!("Dictation · {line}")
                            };
                        } else {
                            self.status = dictation::inject_text(&line);
                        }
                    }
                }
                // W459: ingest network STT result file when present.
                {
                    let runtime =
                        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                    let path = std::path::Path::new(&runtime).join("mde-voice.txt");
                    if path.is_file() {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            let line = text.lines().next().unwrap_or("").trim().to_string();
                            let _ = std::fs::remove_file(&path);
                            if !line.is_empty()
                                && !line.starts_with("STT HTTP")
                                && !line.starts_with("no ")
                            {
                                self.start_query = line;
                                self.status = "Voice search · done".into();
                                if self.search_open || self.start_open {
                                    self.file_hits = files::search_files(&self.start_query, 8);
                for hit in files::search_file_index(&self.start_query, 8) {
                    if !self.file_hits.iter().any(|h| h == &hit) {
                        self.file_hits.push(hit);
                    }
                }
                self.file_hits.truncate(12);
                                }
                            } else if !line.is_empty() {
                                self.status = format!("Voice search · {line}");
                            }
                        }
                    }
                }
                let shell = &self.cfg.active().shell;
                let clock = clock_now_cfg(shell.clock_date_iso, shell.clock_seconds);
                let clock_changed = clock != self.clock;
                if clock_changed {
                    self.clock = clock;
                }
                self.tab_accum_ms = self.tab_accum_ms.saturating_add(400);
                if self.tab_accum_ms >= 2000 {
                    self.tab_accum_ms = 0;
                    // Window list prefers protocol push; file refresh is the fallback.
                    self.refresh_windows();
                    self.sync_sni();
                    self.refresh_levels();
                    self.battery_pct = system::battery_capacity();
                    self.battery_charging = system::battery_charging().unwrap_or(false);
                    if self.cfg.active().accessibility.privacy_indicators {
                        self.privacy_uses = system::privacy_in_use();
                    } else {
                        self.privacy_uses.clear();
                    }
                }
                for id in self.notifs.expire(Duration::from_secs(45)) {
                    self.hub.queue_closed(id, 1);
                }
                // W125: re-push snoozed notifications whose Instant has elapsed.
                let woke = self.notifs.wake_snoozed();
                if woke > 0 {
                    self.status = format!("Snooze ended · {woke}");
                }
                for id in self.hub.take_ui_closes() {
                    self.notifs.dismiss(id);
                }
                if self.find_mouse_ticks > 0 {
                    self.find_mouse_ticks = self.find_mouse_ticks.saturating_sub(1);
                }
                // A023: expire timed awake
                {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if self.awake_state.active && !tools::awake_is_active(&self.awake_state, now) {
                        self.awake_state.active = false;
                        tools::save_awake(&self.awake_state);
                    }
                    if tools::awake_is_active(&self.awake_state, now) {
                        self.idle_inhibited = true;
                    }
                }
                if self.osd_until.is_some_and(|u| Instant::now() >= u) {
                    self.osd = None;
                    self.osd_until = None;
                }
                if self.alttab_until.is_some_and(|u| Instant::now() >= u) {
                    self.alttab_until = None;
                }
                if self.toast_until.is_some_and(|u| Instant::now() >= u) {
                    // Promote stacked toast when primary expires (W196).
                    self.toast = self.toast_stack.take();
                    self.toast_until = self.toast_stack_until.take();
                    self.toast_expanded = false;
                }
                if self
                    .screenshot_toast
                    .as_ref()
                    .is_some_and(|t| t.expired())
                {
                    self.screenshot_toast = None;
                }
                if self.toast_stack_until.is_some_and(|u| Instant::now() >= u) {
                    self.toast_stack = None;
                    self.toast_stack_until = None;
                }
                if self
                    .snap_assist_until
                    .is_some_and(|u| Instant::now() >= u)
                    && self.snap_assist_side.is_some()
                {
                    // W202: dismiss snap assist after ~8s without click.
                    self.snap_assist_side = None;
                    self.snap_assist_until = None;
                    self.status = "Snap assist timed out".into();
                }
                // W217: ~400ms hover on show-desktop strip → Peek desktop status.
                if self
                    .peek_desktop_until
                    .is_some_and(|u| Instant::now() >= u)
                {
                    self.peek_desktop_until = None;
                    self.status = "Peek desktop".into();
                }
                if self.clear_undo_until.is_some_and(|u| Instant::now() >= u) {
                    self.clear_undo_until = None;
                    self.cleared_stash = None;
                }
                let mut notif_dirty = false;
                for n in self.hub.drain() {
                    notif_dirty = true;
                    let clone = n.clone();
                    if let Some(slot) = self.notifs.items.iter_mut().find(|item| item.id == n.id) {
                        *slot = n;
                    } else {
                        self.notifs.items.insert(0, n);
                    }
                    // Win11 toast popup above the bar (~5s), unless Notification Center is open.
                    // W176: urgency >= 2 stays until click/close (no toast_until expiry).
                    // W196: ≤2 stacked toasts when another arrives while one is visible.
                    if !self.notifications_open {
                        let sticky = clone.urgency >= 2;
                        self.push_toast(clone, sticky);
                        self.maybe_play_notif_sound();
                    }
                }
                if self.notifs.items.len() > 50 {
                    self.notifs.items.truncate(50);
                    notif_dirty = true;
                }
                if notif_dirty {
                    self.notifs.persist();
                }
                self.maybe_reload_config();
                if clock_changed {
                    let next = Palette::from_theme(&self.cfg.active().theme);
                    if next.dark != self.palette.dark {
                        self.palette = next;
                        system::apply_mackes_icon_theme(self.palette.dark);
                    }
                    if self.cfg.active().session.night_light_schedule {
                        let on = mde_config::night_light_schedule_active(&self.cfg.active().session);
                        let strength = self.cfg.active().session.night_light_strength;
                        let _ = system::set_night_light(on, strength);
                    }
                    // W401: re-evaluate schedule-backed brightness on clock change.
                    if let Some(msg) = self.apply_dynamic_brightness(false) {
                        self.status = msg;
                    }
                }
                // W404: end focus timer when Instant elapses.
                if self
                    .focus_timer_until
                    .is_some_and(|u| Instant::now() >= u)
                {
                    self.end_focus_timer();
                    self.status = "Focus timer · done".into();
                }
                self.clipboard.poll_primary(); // includes W143 expiry drop
                let mut bar_sync = Task::none();
                // A009: apply deferred auto-hide after leave grace.
                if self
                    .bar_hide_after
                    .is_some_and(|u| Instant::now() >= u)
                    && !self.bar_hover
                    && !self.flyout_open()
                {
                    self.bar_hide_after = None;
                    bar_sync = self.sync_bar_geometry();
                }
                if self.quick_open && self.media_available {
                    let m = system::media_status();
                    self.media_title = m.title;
                    self.media_playing = m.playing;
                }
                self.idle_accum_ms = self.idle_accum_ms.saturating_add(400);
                if self.idle_accum_ms >= 1000 {
                    let secs = (self.idle_accum_ms / 1000) as u64;
                    self.idle_accum_ms %= 1000;
                    if self.idle_inhibited {
                        self.idle.activity();
                    }
                    let was_locked = self.idle.phase == LockPhase::Locked;
                    let phase = self.idle.tick_idle(secs);
                    if phase == LockPhase::Locked && !was_locked {
                        self.engage_lock("idle");
                    } else if phase == LockPhase::IdleWarning {
                        self.status = "Idle warning — soft lock soon".into();
                    }
                }
                self.poll_lock_child();
                {
                    let runtime =
                        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                    let path = std::path::Path::new(&runtime).join("mde-snap-assist.req");
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        let _ = std::fs::remove_file(&path);
                        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
                        if let Some(first) = lines.next() {
                            if first == "left" || first == "right" {
                                // Super+Left/Right from compositor → open Snap assist overlay (W63).
                                self.close_flyouts();
                                self.snap_assist_side = Some(first.to_string());
                                self.snap_assist_until =
                                    Some(Instant::now() + Duration::from_secs(8));
                                self.refresh_windows();
                                self.status = format!("Snap assist · fill opposite of {first}");
                            } else if let Some(taken) = first.strip_prefix("quadrant ") {
                                // Quarter snap → remaining corner hint (W81).
                                let remaining = lines.next().unwrap_or("");
                                self.status = if remaining.is_empty() {
                                    format!("Snap · {taken}")
                                } else {
                                    format!("Snap · {taken} · remaining {remaining}")
                                };
                            } else if let Some(taken) = first.strip_prefix("thirds ") {
                                // W122: remaining third(s) after left/right 1/3 snap.
                                let remaining = lines.next().unwrap_or("");
                                self.status = if remaining.is_empty() {
                                    format!("Snap · {taken}")
                                } else {
                                    format!("Snap · {taken} · remaining {remaining}")
                                };
                            }
                        }
                    }
                }
                // W168 / W188: compositor writes mde-workspace.notify (anim / new).
                {
                    let runtime =
                        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                    let path = std::path::Path::new(&runtime).join("mde-workspace.notify");
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        let _ = std::fs::remove_file(&path);
                        let t = text.trim();
                        if let Some(rest) = t.strip_prefix("anim ") {
                            if let Ok(n) = rest.trim().parse::<usize>() {
                                self.desktop_active = n;
                                self.status = format!("Desktop {}", n + 1);
                            }
                        } else if let Some(rest) = t.strip_prefix("new ") {
                            if let Ok(n) = rest.trim().parse::<usize>() {
                                self.desktop_active = n;
                                self.status = format!("Desktop {} · new", n + 1);
                            }
                        }
                    }
                }
                return bar_sync;
            }
            Message::PasteClip(id) => match self.clipboard.paste_back(id) {
                Ok(()) => {
                    self.idle.activity();
                    self.status = format!("Pasted clipboard #{id} via wl-copy");
                }
                Err(e) => self.status = e,
            },
            Message::Unlock => {
                // Ask mde-lock to unlock ext-session-lock cleanly; keep soft overlay unlock.
                write_unlock_req();
                self.idle.unlock();
                self.idle_accum_ms = 0;
                self.status = "Unlocked".into();
                // Child exit is observed by poll_lock_child (do not kill — that skips protocol unlock).
            }
            Message::UserActivity => {
                if self.idle.phase != LockPhase::Locked {
                    self.idle.activity();
                    self.idle_accum_ms = 0;
                }
            }
            Message::Dismiss => {
                if self.idle.phase != LockPhase::Locked {
                    if self.snip_open {
                        self.snip_open = false;
                        self.snip_preselect = None;
                        self.status = "Snip cancelled".into();
                    } else if self.screenshot_toast.is_some() {
                        self.screenshot_toast = None;
                        self.status = "Screenshot toast dismissed".into();
                    } else if self.clear_undo_until.is_some() {
                        self.clear_undo_until = None;
                        self.cleared_stash = None;
                        self.status = "Undo dismissed".into();
                    } else if self.toast.is_some() {
                        self.toast = self.toast_stack.take();
                        self.toast_until = self.toast_stack_until.take();
                        self.toast_expanded = false;
                        self.status = "Toast dismissed".into();
                    } else if self.flyout_open() {
                        self.close_flyouts();
                        self.status = "Closed".into();
                    }
                }
            }
            Message::SnipCancel => {
                self.snip_open = false;
                self.snip_preselect = None;
                self.status = "Snip cancelled".into();
            }
            Message::SnipCapture(mode) => {
                return self.begin_snip_capture(mode);
            }
            Message::ScreenshotCaptured(outcome) => {
                self.apply_capture_outcome(outcome);
            }
            Message::ScreenshotToastOpen => {
                if let Some(toast) = self.screenshot_toast.take() {
                    screenshot::open_swappy(&toast.path);
                    self.status = format!("Screenshot · edit · {}", toast.path.display());
                }
            }
            Message::StartQuery(s) => {
                self.file_hits = files::search_files(&s, 8);
                for hit in files::search_file_index(&s, 8) {
                    if !self.file_hits.iter().any(|h| h == &hit) {
                        self.file_hits.push(hit);
                    }
                }
                self.file_hits.truncate(12);
                // W215: after a math result, keep last 3 in session Calculator list.
                if let Some(v) = try_eval_math(s.trim()) {
                    self.push_calc_history(s.trim(), v);
                }
                if s.trim().is_empty() {
                    self.search_type.clear();
                }
                self.start_query = s;
            }
            Message::OpenFile(path) => match files::open_path(&path) {
                Ok(()) => {
                    self.status = format!("Opened {path}");
                    self.close_flyouts();
                }
                Err(e) => self.status = e,
            },
            Message::ClearClip => {
                // W89: ask Yes/No once before clearing.
                self.clip_clear_confirm = true;
                self.status = "Clear clipboard history?".into();
            }
            Message::ConfirmClearClip => {
                self.clip_clear_confirm = false;
                self.clipboard.clear_unpinned();
                self.status = "Cleared unpinned clipboard history".into();
            }
            Message::CancelClearClip => {
                self.clip_clear_confirm = false;
                self.status = "Clear cancelled".into();
            }
            Message::PinClip(id) => {
                self.status = match self.clipboard.toggle_pin(id) {
                    Some(true) => "Pinned clip".into(),
                    Some(false) => "Unpinned clip".into(),
                    None => "Pin limit reached (10) or missing".into(),
                };
            }
            Message::FocusWindow(index) => {
                self.request_focus_index(index);
                let title = self
                    .windows
                    .get(index)
                    .map(|w| w.title.clone())
                    .unwrap_or_else(|| "window".into());
                self.alttab_index = index;
                self.taskview_open = false;
                self.alttab_until = None;
                self.win_overflow_open = false;
                self.status = format!("Focus {title}");
            }
            Message::ConfirmSwitch => {
                // Soft lock / Start ignore Enter here; Unlock and search keep their own paths.
                if self.idle.phase == LockPhase::Locked {
                    return self.sync_overlay();
                }
                if self.alttab_until.is_some() {
                    // W208: Desktop stub confirms show-desktop.
                    let indices = self.active_window_indices();
                    if self.alttab_shows_desktop() && self.alttab_index == indices.len() {
                        self.ipc
                            .show_desktop(mde_protocol::legacy_enabled());
                        self.status = "Alt+Tab · Desktop".into();
                    } else {
                        // Focus already applied on each alttab cycle.
                        self.status = "Alt+Tab confirmed".into();
                    }
                    self.alttab_until = None;
                } else if self.taskview_open {
                    if !self.windows.is_empty() {
                        let index = self.alttab_index.min(self.windows.len() - 1);
                        self.request_focus_index(index);
                        self.status = format!("Focus {}", self.windows[index].title);
                    }
                    self.taskview_open = false;
                }
            }
            Message::RefreshRadios => {
                let r = system::probe_radios();
                self.wifi_on = r.wifi;
                self.bt_on = r.bluetooth;
                self.airplane = r.airplane;
                self.status = "Quick Settings open".into();
            }
            Message::LaunchPin(id) => {
                if self.start_pin_drag_moved {
                    self.start_pin_drag_moved = false;
                    return Task::none();
                }
                // W264: folder path pins open via xdg-open.
                let expanded = expand_user_path(&id);
                if std::path::Path::new(&expanded).exists() {
                    match files::open_path(&expanded) {
                        Ok(()) => {
                            self.status = format!("Opened {expanded}");
                            self.close_flyouts();
                        }
                        Err(e) => self.status = e,
                    }
                } else {
                    match apps::launch_desktop(&id) {
                        Ok(()) => {
                            recent::record_launch(&id);
                            self.recent = recent::load(8);
                            self.status = format!("Launched {id}");
                            self.close_flyouts();
                        }
                        Err(e) => self.status = e,
                    }
                }
            }
            Message::ToggleVolumeFlyout => {
                self.idle.activity();
                let open = !self.vol_flyout_open;
                self.close_flyouts();
                self.vol_flyout_open = open;
                if open {
                    self.refresh_levels();
                }
                self.status = if open {
                    "Volume".into()
                } else {
                    "Volume closed".into()
                };
            }
            Message::ToggleNetworkFlyout => {
                self.idle.activity();
                let open = !self.net_flyout_open;
                self.close_flyouts();
                self.net_flyout_open = open;
                if open {
                    self.wifi_aps = system::wifi_list(8);
                }
                self.status = if open {
                    format!("Network ({} SSIDs)", self.wifi_aps.len())
                } else {
                    "Network closed".into()
                };
            }
            Message::RefreshWifi => {
                self.wifi_aps = system::wifi_list(8);
                self.status = format!("Wi-Fi scan ({} )", self.wifi_aps.len());
            }
            Message::ConnectWifi(ssid) => {
                self.idle.activity();
                match system::wifi_connect(&ssid) {
                    Ok(()) => {
                        self.status = format!("Connecting to {ssid}");
                        self.wifi_aps = system::wifi_list(8);
                        self.wifi_password_ssid.clear();
                        self.wifi_password.clear();
                    }
                    Err(e) if e.contains("password") || e.contains("nmtui") || e.contains("NetworkManager") => {
                        self.wifi_password_ssid = ssid;
                        self.wifi_password.clear();
                        self.status = "Enter Wi-Fi password".into();
                    }
                    Err(e) => self.status = e,
                }
            },
            Message::ToggleVpnFlyout => {
                self.idle.activity();
                let open = !self.vpn_flyout_open;
                self.close_flyouts();
                self.vpn_flyout_open = open;
                if open {
                    self.vpn_list = system::vpn_list();
                }
                self.status = if open {
                    format!("VPN ({} profiles)", self.vpn_list.len())
                } else {
                    "VPN closed".into()
                };
            }
            Message::RefreshVpn => {
                self.vpn_list = system::vpn_list();
                self.status = format!("VPN scan ({} )", self.vpn_list.len());
            }
            Message::VpnConnect(name) => match system::vpn_connect(&name) {
                Ok(()) => {
                    self.status = format!("VPN connecting · {name}");
                    self.vpn_list = system::vpn_list();
                }
                Err(e) => self.status = e,
            },
            Message::VpnDisconnect(name) => match system::vpn_disconnect(&name) {
                Ok(()) => {
                    self.status = format!("VPN disconnected · {name}");
                    self.vpn_list = system::vpn_list();
                }
                Err(e) => self.status = e,
            },
            Message::ToggleHotspotFlyout => {
                self.idle.activity();
                let open = !self.hotspot_flyout_open;
                self.close_flyouts();
                self.hotspot_flyout_open = open;
                if open {
                    self.hotspot = system::hotspot_status();
                }
                self.status = if open {
                    if self.hotspot.active {
                        format!("Hotspot · {}", self.hotspot.ssid)
                    } else {
                        "Mobile hotspot".into()
                    }
                } else {
                    "Hotspot closed".into()
                };
            }
            Message::RefreshHotspot => {
                self.hotspot = system::hotspot_status();
                self.status = if self.hotspot.active {
                    format!("Hotspot · {}", self.hotspot.ssid)
                } else {
                    "Hotspot off".into()
                };
            }
            Message::HotspotStart => match system::hotspot_start() {
                Ok(info) => {
                    self.hotspot = info;
                    self.status = format!("Hotspot on · {}", self.hotspot.ssid);
                }
                Err(e) => self.status = e,
            },
            Message::HotspotStop => match system::hotspot_stop() {
                Ok(()) => {
                    self.hotspot = system::hotspot_status();
                    self.hotspot.active = false;
                    self.status = "Hotspot stopped".into();
                }
                Err(e) => self.status = e,
            },
            Message::DismissToast => {
                self.toast = self.toast_stack.take();
                self.toast_until = self.toast_stack_until.take();
                self.toast_expanded = false;
            }
            Message::WindowAction(action) => {
                self.idle.activity();
                self.write_window_action(&action);
                self.winmenu_open = false;
                self.sysmenu_open = false;
                self.snapflyout_open = false;
                self.status = format!("Window {action}");
            }
            Message::OpenBackground => {
                self.idle.activity();
                // "Change wallpaper…" → Personalization → Background.
                self.launch_control("background");
                self.close_flyouts();
                self.status = "Settings · Background".into();
            },
            Message::ToggleTray => {
                let open = !self.tray_open;
                self.close_flyouts();
                self.tray_open = open;
                if open {
                    let ids: Vec<String> = self.tray.items.iter().map(|i| i.id.clone()).collect();
                    persist_tray_known(&ids);
                }
                self.status = format!("Tray ({} items)", self.tray.items.len());
            }
            Message::ActivateTray(id) => self.invoke_tray(&id, true),
            Message::TrayMenu(id) => {
                self.close_flyouts();
                self.tray_open = true;
                                self.invoke_tray(&id, false);
            }
            Message::TraySecondary(id) => {
                if let Some(item) = self.tray.items.iter().find(|item| item.id == id) {
                    if !item.bus.is_empty() && !item.path.is_empty() {
                        sni::request_secondary_activate(item.bus.clone(), item.path.clone());
                        self.status = format!("Secondary activate {}", item.title);
                    }
                }
            }
            Message::TrayMenuClick(item_id, menu_id) => {
                if let Some(menu) = self.tray_menu.clone() {
                    if menu.item_id == item_id {
                        sni::click_menu(menu.bus, menu.menu_path, menu_id);
                        self.status = format!("Menu item {menu_id}");
                    }
                }
            }
            Message::ToggleKeyboard => {
                let open = !self.keyboard_open;
                self.close_flyouts();
                self.keyboard_open = open;
                self.status = if open {
                    "On-screen keyboard".into()
                } else {
                    "Keyboard hidden".into()
                };
            }
            Message::ToggleShift => self.keyboard_shift = !self.keyboard_shift,
            Message::ToggleSymbols => self.keyboard_symbols = !self.keyboard_symbols,
            Message::ToggleDnd => {
                self.idle.activity();
                self.notifs.dnd = !self.notifs.dnd;
                if self.notifs.dnd {
                    self.notifs.priority = false;
                    self.cfg.active_mut().notifications.priority = false;
                }
                self.hub.set_dnd(self.notifs.dnd);
                self.hub
                    .set_priority(self.notifs.priority, self.notifs.priority_apps.clone());
                self.cfg.active_mut().notifications.dnd = self.notifs.dnd;
                let _ = self.cfg.save();
                self.status = format!("Do not disturb {}", on_off(self.notifs.dnd));
            }
            Message::TogglePriority => {
                self.idle.activity();
                self.notifs.priority = !self.notifs.priority;
                if self.notifs.priority {
                    // Priority filters unknown apps; clear full DND so allowlist works.
                    self.notifs.dnd = false;
                    self.cfg.active_mut().notifications.dnd = false;
                }
                self.notifs.priority_apps =
                    notifications::load_priority_apps(&self.cfg.active().shell.pins);
                self.hub.set_dnd(self.notifs.dnd);
                self.hub
                    .set_priority(self.notifs.priority, self.notifs.priority_apps.clone());
                self.cfg.active_mut().notifications.priority = self.notifs.priority;
                let _ = self.cfg.save();
                self.status = format!("Focus assist priority {}", on_off(self.notifs.priority));
            }
            Message::DismissRecommended(id) => {
                recent::dismiss(&id);
                self.recent = recent::load(8);
                self.status = format!("Hidden from recommended · {id}");
            }
            Message::ToggleBatteryFlyout => {
                self.idle.activity();
                let open = !self.bat_flyout_open;
                self.close_flyouts();
                self.bat_flyout_open = open;
                if open {
                    self.battery_pct = system::battery_capacity();
                    self.battery_charging = system::battery_charging().unwrap_or(false);
                }
                self.status = if open {
                    "Battery".into()
                } else {
                    "Battery closed".into()
                };
            }
            Message::ToggleClockSeconds => {
                self.idle.activity();
                let on = !self.cfg.active().shell.clock_seconds;
                self.cfg.active_mut().shell.clock_seconds = on;
                let _ = self.cfg.save();
                let iso = self.cfg.active().shell.clock_date_iso;
                self.clock = clock_now_cfg(iso, on);
                self.status = if on {
                    "Clock seconds on".into()
                } else {
                    "Clock seconds off".into()
                };
            }
            Message::ToggleClockIso => {
                self.idle.activity();
                let on = !self.cfg.active().shell.clock_date_iso;
                self.cfg.active_mut().shell.clock_date_iso = on;
                let _ = self.cfg.save();
                let secs = self.cfg.active().shell.clock_seconds;
                self.clock = clock_now_cfg(on, secs);
                self.status = if on {
                    "Clock date · ISO".into()
                } else {
                    "Clock date · month and day".into()
                };
            }
            Message::ToggleClockDateFormat => {
                // W93 ISO + W237 seconds: right-click cycles date/seconds modes.
                self.idle.activity();
                let iso = self.cfg.active().shell.clock_date_iso;
                let secs = self.cfg.active().shell.clock_seconds;
                let (next_iso, next_secs) = match (iso, secs) {
                    (false, false) => (true, false),
                    (true, false) => (false, true),
                    (false, true) => (true, true),
                    (true, true) => (false, false),
                };
                self.cfg.active_mut().shell.clock_date_iso = next_iso;
                self.cfg.active_mut().shell.clock_seconds = next_secs;
                let _ = self.cfg.save();
                self.clock = clock_now_cfg(next_iso, next_secs);
                self.status = match (next_iso, next_secs) {
                    (true, false) => "Date format · YYYY-MM-DD".into(),
                    (false, true) => "Clock · seconds on".into(),
                    (true, true) => "Clock · ISO + seconds".into(),
                    (false, false) => "Date format · weekday M/D".into(),
                };
            }
            Message::RestartShell => {
                self.status = "Restarting shell…".into();
                // Session supervisor respawns chrome after a clean exit (W94).
                std::process::exit(0);
            }
            Message::ToggleNightLight => {
                self.idle.activity();
                let on = !self.cfg.active().session.night_light;
                let strength = self.cfg.active().session.night_light_strength;
                match system::set_night_light(on, strength) {
                    Ok(()) => {
                        self.cfg.active_mut().session.night_light = on;
                        let _ = self.cfg.save();
                        self.status = format!("Night light {}", on_off(on));
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::ShowDesktop => {
                self.idle.activity();
                self.close_flyouts();
                self.ipc
                    .show_desktop(mde_protocol::legacy_enabled());
                self.status = "Show desktop".into();
            }
            Message::CalPrev => {
                self.calendar = self.calendar.prev();
                self.cal_selected = None;
            }
            Message::CalNext => {
                self.calendar = self.calendar.next();
                self.cal_selected = None;
            }
            Message::CalSelect(day) => {
                self.cal_selected = Some(day);
                self.status = format!(
                    "{} {} {}",
                    self.calendar.title.split_whitespace().next().unwrap_or("Day"),
                    day,
                    self.calendar.year
                );
            }
            Message::ToggleWidgets => {
                let open = !self.widgets_open;
                self.close_flyouts();
                self.widgets_open = open;
                if open {
                    self.refresh_widget_feeds_if_needed();
                }
                if open {
                    if self.weather.needs_refresh() {
                        self.weather.refresh();
                    }
                    self.system_card.refresh();
                }
                self.status = if open {
                    "Widgets Board".into()
                } else {
                    "Widgets closed".into()
                };
            }

            Message::RefreshNews => {
                self.idle.activity();
                let url = self.news_feed_url();
                self.news_card.refresh(&url);
                self.status = if self.news_card.error.is_empty() {
                    format!("News · {} items", self.news_card.headlines.len())
                } else {
                    format!("News · {}", self.news_card.error)
                };
            }
            Message::RefreshMarkets => {
                self.idle.activity();
                let syms = self.markets_symbols();
                self.markets_card.refresh(&syms);
                self.status = if self.markets_card.error.is_empty() {
                    format!("Markets · {} quotes", self.markets_card.rows.len())
                } else {
                    format!("Markets · {}", self.markets_card.error)
                };
            }
            Message::RefreshTraffic => {
                self.idle.activity();
                let (home, work) = self.traffic_endpoints();
                self.traffic_card.refresh(&home, &work);
                self.status = if self.traffic_card.error.is_empty() {
                    format!("Traffic · {}", self.traffic_card.eta)
                } else {
                    format!("Traffic · {}", self.traffic_card.error)
                };
            }
            Message::OpenNewsLink(url) => {
                self.idle.activity();
                if !url.is_empty() {
                    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                    self.status = format!("Open · {url}");
                }
            }

            Message::RefreshWeather => {
                self.weather.refresh();
                self.system_card.refresh();
                self.status = format!(
                    "Weather · {} · {}",
                    self.weather.place, self.weather.temp_c
                );
            }
            Message::TypeKey(token) => match keyboard::type_token(&token) {
                Ok(()) => self.status = format!("Typed {token}"),
                Err(e) => self.status = e,
            },
            // iced_layershell-generated control messages (SizeChange, etc.)
            _ => return Task::none(),
        }
        self.sync_overlay()
    }

    fn view_overlay(&self) -> Element<'_, Message> {
        let mut stack = column![].spacing(0).width(Fill).height(Fill);

        // Soft iced overlay is the fallback when mde-lock (ext-session-lock) is not running.
        if self.idle.phase == LockPhase::Locked && !self.protocol_lock_active() {
            let user = std::env::var("USER").unwrap_or_else(|_| "User".into());
            // W240: ~/.face or ~/.local/share/mde/avatar.png → iced image; else initial.
            let avatar: Element<'_, Message> = match lock_avatar_path() {
                Some(path) => image(path).width(56).height(56).into(),
                None => {
                    let initial = user
                        .chars()
                        .next()
                        .unwrap_or('U')
                        .to_uppercase()
                        .to_string();
                    container(text(initial).size(28))
                        .width(56)
                        .height(56)
                        .center_x(Fill)
                        .center_y(Fill)
                        .into()
                }
            };
            stack = stack.push(
                container(
                    column![
                        text(clock_time()).size(56),
                        text(clock_long_date()).size(16),
                        text(widgets::read_weather_cache_line()).size(13),
                        // W280: Refresh weather → re-read weather.cache / status.
                        quiet(text("Refresh weather").size(12))
                            .on_press(Message::RefreshLockWeather)
                            .padding([4, 8]),
                        Space::with_height(28),
                        avatar,
                        text(user).size(15),
                        Space::with_height(10),
                        // A055: full media card on soft lock.
                        {
                            let media = if self.media_available {
                                system::media_status()
                            } else {
                                system::MediaStatus::default()
                            };
                            let prog = (media.progress * 100.0).round() as u32;
                            let transport = row![
                                quiet(text("⏮").size(16))
                                    .on_press(Message::MediaPrevious)
                                    .padding([8, 12]),
                                quiet(text(if media.playing { "⏸" } else { "▶" }).size(18))
                                    .on_press(Message::MediaPlayPause)
                                    .style(accent_pill)
                                    .padding([8, 16]),
                                quiet(text("⏭").size(16))
                                    .on_press(Message::MediaNext)
                                    .padding([8, 12]),
                                quiet(text("Vol −").size(12))
                                    .on_press(Message::MediaVolDown)
                                    .padding([6, 10]),
                                quiet(text("Vol +").size(12))
                                    .on_press(Message::MediaVolUp)
                                    .padding([6, 10]),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center);
                            let card = column![
                                text("Now playing").size(11),
                                text(if self.media_available {
                                    media.title.clone()
                                } else {
                                    "No media player".into()
                                })
                                .size(14),
                                text(if media.album.is_empty() {
                                    " ".to_string()
                                } else {
                                    media.album.clone()
                                })
                                .size(11),
                                text(format!("Progress · {prog}%")).size(11),
                                transport,
                            ]
                            .spacing(4)
                            .align_x(iced::Alignment::Center)
                            .padding([12, 16]);
                            flyout_inset(card, &self.palette)
                        },
                        Space::with_height(12),
                        quiet(text("Unlock").size(14))
                            .on_press(Message::Unlock)
                            .style(accent_pill)
                            .padding([12, 36]),
                        text("Soft lock · mde-lock (ext-session-lock) unavailable")
                            .size(11),
                        // W260: lock slideshow tip.
                        text("Slideshow · use Settings")
                            .size(11)
                            .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
                    ]
                    .spacing(6)
                    .align_x(iced::Alignment::Center),
                )
                .padding(32)
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill),
            );
        } else if self.taskview_open {
            stack = stack.push(self.view_taskview());
        } else if self.alttab_until.is_some() {
            stack = stack.push(self.view_alttab());
        } else if self.snap_assist_side.is_some() {
            stack = stack.push(self.view_snap_assist());
        } else if self.snapflyout_open {
            stack = stack.push(self.view_snapflyout());
        } else if self.pin_overflow_open {
            stack = stack.push(self.view_pin_overflow());
        } else if self.win_overflow_open {
            stack = stack.push(self.view_win_overflow());
        } else if self.jump_list_pin.is_some() {
            stack = stack.push(self.view_jump_list());
        } else if self.jump_list_win.is_some() {
            stack = stack.push(self.view_win_jump());
        } else if self.app_ctx.is_some() {
            stack = stack.push(self.view_app_ctx());
        } else if self.desktop_menu_open {
            stack = stack.push(self.view_desktop_menu());
        } else if self.widgets_open {
            stack = stack.push(self.view_widgets());
        } else if self.search_open {
            stack = stack.push(self.view_search_surface());
        } else if self.start_open {
            stack = stack.push(self.view_start());
        } else if self.sysmenu_open {
            stack = stack.push(self.view_sysmenu());
        } else if self.winmenu_open {
            stack = stack.push(self.view_winmenu());
        } else if self.quicklink_open {
            stack = stack.push(self.view_quicklink());
        } else if self.taskbar_menu_open {
            stack = stack.push(self.view_taskbar_menu());
        } else if self.run_open {
            stack = stack.push(self.view_run());
        } else if self.connect_open {
            stack = stack.push(self.view_connect());
        } else if self.vol_flyout_open {
            stack = stack.push(self.view_volume_flyout());
        } else if self.bat_flyout_open {
            stack = stack.push(self.view_battery_flyout());
        } else if self.net_flyout_open {
            stack = stack.push(self.view_network_flyout());
        } else if self.bt_flyout_open {
            stack = stack.push(self.view_bt_flyout());
        } else if self.color_history_open {
            stack = stack.push(self.view_color_history());
        } else if self.snap_groups_open {
            stack = stack.push(self.view_snap_groups());
        } else if self.find_mouse_ticks > 0 {
            stack = stack.push(self.view_find_mouse());
        } else if self.vpn_flyout_open {
            stack = stack.push(self.view_vpn_flyout());
        } else if self.hotspot_flyout_open {
            stack = stack.push(self.view_hotspot_flyout());
        } else if self.nearby_flyout_open {
            stack = stack.push(self.view_nearby_flyout());
        } else if self.app_mixer_open {
            stack = stack.push(self.view_app_mixer());
        } else if self.emoji_open {
            stack = stack.push(self.view_emoji());
        } else if self.shortcut_guide_open {
            stack = stack.push(self.view_shortcut_guide());
        } else if self.snip_open {
            stack = stack.push(self.view_snip_overlay());
        } else if self.screenshot_toast.is_some() {
            stack = stack.push(self.view_screenshot_toast());
        } else if self.share_active {
            stack = stack.push(self.view_share_pill());
        } else if self.tray_open {
            stack = stack.push(self.view_tray());
        } else if self.keyboard_open {
            stack = stack.push(self.keyboard_pad());
        } else if self.quick_open {
            stack = stack.push(self.view_quick());
        } else if self.notifications_open {
            stack = stack.push(self.view_notifications());
        } else if self.clipboard_open {
            stack = stack.push(self.view_clipboard());
        } else if self.clear_undo_until.is_some() {
            stack = stack.push(self.view_clear_undo_toast());
        } else if self.toast.is_some() {
            stack = stack.push(self.view_toast_stack());
        } else if let Some(osd) = &self.osd {
            stack = stack.push(view_osd(osd, &self.palette));
        }

        let place = self
            .overlay_frame
            .map(|frame| frame.place)
            .unwrap_or(FlyoutPlace::Start);
        flyout_stage(
            stack.into(),
            &self.palette,
            place,
            self.toast_left(),
            self.taskbar_left_align(),
            self.taskbar_menu_anchor_x,
            self.start_card_height(),
        )
    }

}

/// Overlay surface geometry. Dismissable menus fill the zone above the taskbar so
/// clicks outside the card close them. The OSD stays a small centered pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlyoutFrame {
    width: u32,
    height: u32,
    anchor: Anchor,
    margin: (i32, i32, i32, i32),
    place: FlyoutPlace,
    dismiss: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlyoutPlace {
    Start,
    /// Dedicated taskbar Search (W461).
    Search,
    /// Quick Settings (Win11 Control Center) — compact, no scroll.
    Quick,
    Side,
    Widgets,
    Keyboard,
    Lock,
    Osd,
    AltTab,
    TaskView,
    SnapFlyout,
    Volume,
    WinMenu,
    /// SSD title-bar system menu (W389) — compact, not Alt+Space winmenu.
    SysMenu,
    Run,
    QuickLink,
    /// Right-click menu for the taskbar background and clock.
    TaskbarMenu,
    /// Right-click menu for a pinned app or running window.
    JumpList,
    /// Start / All apps / Recommended app context menu (W383–W385).
    AppCtx,
    /// Empty-desktop context menu (W388).
    DesktopMenu,
    Toast,
    /// W449 Connect / Wireless display (~400×420).
    Connect,
    /// Persistent sharing pill while ScreenCast is active.
    SharePill,
    /// W515 Win11 snip overlay (full-zone dimmer + toolbar).
    Snip,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlyoutInput {
    locked: bool,
    taskview: bool,
    start: bool,
    /// Dedicated Search (W461).
    search: bool,
    /// Quick Settings Control Center (separate from tall NC Side card).
    quick: bool,
    side: bool,
    widgets: bool,
    keyboard: bool,
    alttab: bool,
    snapflyout: bool,
    volume: bool,
    winmenu: bool,
    sysmenu: bool,
    run: bool,
    /// Super+X / Start right-click quick link menu (W161 / W390).
    quicklink: bool,
    /// Right-click taskbar background / clock.
    taskbar_menu: bool,
    /// Right-click pin or running window.
    jumplist: bool,
    app_ctx: bool,
    desktop_menu: bool,
    toast: bool,
    osd: bool,
    centered: bool,
    /// W449 Connect flyout.
    connect: bool,
    /// W449 sharing pill (non-expiring).
    share_pill: bool,
    /// W515 snip overlay.
    snip: bool,
}

pub(crate) fn overlay_frame(input: FlyoutInput) -> Option<FlyoutFrame> {
    const GAP: i32 = 10;
    // Full zone above the bar: Neutral exclusive zone + stretch anchors.
    let full = FlyoutFrame {
        width: 0,
        height: 0,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right | Anchor::Bottom,
        margin: (0, 0, 0, 0),
        place: FlyoutPlace::Start,
        dismiss: true,
    };
    if input.locked {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Lock,
            ..full
        });
    }
    if input.taskview {
        return Some(FlyoutFrame {
            place: FlyoutPlace::TaskView,
            ..full
        });
    }
    if input.snip {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Snip,
            ..full
        });
    }
    if input.search {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Search,
            ..full
        });
    }
    if input.start {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Start,
            ..full
        });
    }
    if input.widgets {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Widgets,
            ..full
        });
    }
    if input.jumplist {
        return Some(FlyoutFrame {
            place: FlyoutPlace::JumpList,
            ..full
        });
    }
    if input.app_ctx {
        return Some(FlyoutFrame {
            place: FlyoutPlace::AppCtx,
            ..full
        });
    }
    if input.desktop_menu {
        return Some(FlyoutFrame {
            place: FlyoutPlace::DesktopMenu,
            ..full
        });
    }
    if input.taskbar_menu {
        return Some(FlyoutFrame {
            place: FlyoutPlace::TaskbarMenu,
            ..full
        });
    }
    if input.quick {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Quick,
            ..full
        });
    }
    if input.connect {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Connect,
            ..full
        });
    }
    if input.side {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Side,
            ..full
        });
    }
    if input.keyboard {
        return Some(FlyoutFrame {
            place: FlyoutPlace::Keyboard,
            ..full
        });
    }
    if input.snapflyout {
        return Some(FlyoutFrame {
            place: FlyoutPlace::SnapFlyout,
            ..full
        });
    }
    // Context menus use the full dismiss zone; card size/placement is in flyout_stage.
    // (Compact Bottom|Left surfaces with GAP+56 margins floated menus too high.)
    if input.sysmenu {
        return Some(FlyoutFrame {
            place: FlyoutPlace::SysMenu,
            ..full
        });
    }
    if input.winmenu {
        return Some(FlyoutFrame {
            place: FlyoutPlace::WinMenu,
            ..full
        });
    }
    if input.quicklink {
        // W161 / W390: Super+X / Start right-click quick link (expanded).
        return Some(FlyoutFrame {
            place: FlyoutPlace::QuickLink,
            ..full
        });
    }
    if input.run {
        // W141: compact Super+R Run dialog centered above the bar.
        return Some(FlyoutFrame {
            width: 420,
            height: 120,
            anchor: Anchor::Bottom,
            margin: (0, 0, GAP + 56, 0),
            place: FlyoutPlace::Run,
            dismiss: true,
        });
    }
    if input.volume {
        return Some(FlyoutFrame {
            width: 360,
            height: 120,
            anchor: Anchor::Bottom | Anchor::Right,
            margin: (0, 16, GAP + 56, 0),
            place: FlyoutPlace::Volume,
            dismiss: true,
        });
    }
    if input.share_pill {
        return Some(FlyoutFrame {
            width: 420,
            height: 56,
            anchor: Anchor::Bottom,
            margin: (0, 0, GAP + 56, 0),
            place: FlyoutPlace::SharePill,
            dismiss: false,
        });
    }
    if input.alttab {
        return Some(FlyoutFrame {
            width: 720,
            height: 160,
            anchor: Anchor::Bottom,
            margin: (0, 0, GAP + 56, 0),
            place: FlyoutPlace::AltTab,
            dismiss: false,
        });
    }
    if input.toast {
        // W75: full zone so the card can sit bottom-right above the taskbar.
        return Some(FlyoutFrame {
            place: FlyoutPlace::Toast,
            dismiss: false,
            ..full
        });
    }
    if input.osd {
        return Some(FlyoutFrame {
            width: 340,
            height: 72,
            anchor: Anchor::Bottom,
            margin: (0, 0, GAP, 0),
            place: FlyoutPlace::Osd,
            dismiss: false,
        });
    }
    // Start card left/center is applied in flyout_stage from taskbar alignment.
    let _ = input.centered;
    None
}

/// Quick Settings / Notification Center card. Three toggles fit across the padding.
pub(crate) const SIDE_CARD_W: u16 = 388;
/// Tall enough for pinned NC chrome + month calendar without scrolling controls.
/// Capped for HiDPI logical heights (e.g. 1920×1080 @1.25 → 864px tall).
pub(crate) const SIDE_CARD_H: u16 = 720;
/// Compact Quick Settings card — 3×3 tiles + sliders + media + footer, no scroll.
pub(crate) const QS_CARD_H: u16 = 520;
/// W449 Connect / Wireless display card.
pub(crate) const CONNECT_CARD_W: u16 = 400;
pub(crate) const CONNECT_CARD_H: u16 = 420;
pub(crate) const SIDE_PAD_X: u16 = 16;
pub(crate) const QS_GAP: u16 = 8;
pub(crate) const QS_TILE_W: u16 = (SIDE_CARD_W - SIDE_PAD_X * 2 - QS_GAP * 2) / 3;
pub(crate) const QS_TILE_H: u16 = 56;
/// Max pinned QS tiles (3×3) so the compact card never scrolls.
pub(crate) const QS_TILES_MAX: usize = 9;

/// Default Win11-compact Control Center tile ids (order matters).
pub(crate) const QS_DEFAULT_TILES: &[&str] = &[
    "wifi",
    "bluetooth",
    "airplane",
    "dnd",
    "night_light",
    "dark",
    "accessibility",
    "keyboard",
    "project",
];

/// Catalog of pinable QS tiles: (id, icon, label).
pub(crate) const QS_CATALOG: &[(&str, &str, &str)] = &[
    ("wifi", "network-wireless", "Wi-Fi"),
    ("bluetooth", "bluetooth", "Bluetooth"),
    ("airplane", "airplane-mode", "Airplane"),
    ("dnd", "notification-disabled", "DND"),
    ("night_light", "night-light", "Eye comfort"),
    ("dark", "weather-clear-night", "Dark mode"),
    ("accessibility", "preferences-desktop-accessibility", "Accessibility"),
    ("keyboard", "input-keyboard", "Keyboard"),
    ("project", "preferences-desktop-display", "Project"),
    ("priority", "emblem-important", "Priority"),
    ("auto_brightness", "weather-clear", "Dynamic brightness"),
    ("cast", "video-display", "Wireless display"),
    ("nearby", "network-wireless-connected-symbolic", "Nearby Share"),
    ("autohide", "view-paged-symbolic", "Auto-hide"),
    ("eject", "media-eject", "Eject tip"),
    ("hotspot", "network-wireless", "Mobile hotspot"),
    ("notif_sounds", "audio-volume-high", "Notif sounds"),
    ("taskbar_left", "object-align-left", "Taskbar left"),
    ("hidden_icons", "view-more-symbolic", "Hidden icons"),
    ("combine", "view-grid-symbolic", "Combine"),
    ("vpn", "network-vpn", "VPN"),
    ("show_labels", "format-text-symbolic", "Show labels"),
    ("alttab", "user-desktop", "Alt+Tab"),
    ("timer", "appointment-soon", "Focus timer"),
    ("awake", "alarm", "Keep awake"),
];

pub(crate) fn qs_catalog_label(id: &str) -> Option<&'static str> {
    QS_CATALOG
        .iter()
        .find(|(tid, _, _)| *tid == id)
        .map(|(_, _, label)| *label)
}

pub(crate) fn qs_catalog_icon(id: &str) -> Option<&'static str> {
    QS_CATALOG
        .iter()
        .find(|(tid, _, _)| *tid == id)
        .map(|(_, icon, _)| *icon)
}
/// Compact 2-column Widgets board (left dock) — fits 1080p without scroll.
pub(crate) const WIDGETS_CARD_W: u16 = 440;
pub(crate) const WIDGETS_CARD_H: u16 = 700;

/// Start card. Six pin tiles fit across the padding.
pub(crate) const START_CARD_W: u16 = 692;
pub(crate) const START_CARD_H: u16 = 760;
pub(crate) const START_PAD_X: u16 = 20;
pub(crate) const START_PIN_W: u16 = 96;
pub(crate) const START_PIN_GAP: u16 = 8;

pub(crate) fn flyout_card<'a>(
    content: Element<'a, Message>,
    palette: &Palette,
    width: u16,
    height: u16,
) -> Element<'a, Message> {
    let fill = mde_theme::flyout_fill(palette);
    let edge = mde_theme::flyout_edge(palette);
    let radius = mde_theme::FLYOUT_RADIUS;
    let shadow_a = if palette.dark { 0.45 } else { 0.28 };
    container(content)
        .width(width)
        .height(height)
        .style(move |_| ContainerStyle {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: 1.0,
                radius: radius.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, shadow_a),
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .into()
}

/// True when taskbar pins/Start sit on the left (W146).
pub(crate) fn taskbar_is_left_aligned(taskbar_left: bool, taskbar_align: &str) -> bool {
    taskbar_left || taskbar_align.eq_ignore_ascii_case("left")
}

/// Card above the taskbar, with a clear click-catcher filling the rest of the overlay.
pub(crate) fn flyout_stage<'a>(
    content: Element<'a, Message>,
    palette: &Palette,
    place: FlyoutPlace,
    toast_left: bool,
    start_left: bool,
    taskbar_menu_x: Option<f32>,
    start_card_h: u16,
) -> Element<'a, Message> {
    // Context-menu cards must fit their items (no forced scroll). Full-zone overlays
    // sit the card just above the taskbar (12px), matching Start — not GAP+56+64.
    let (width, height) = match place {
        FlyoutPlace::Start | FlyoutPlace::Search => (START_CARD_W, start_card_h),
        FlyoutPlace::Quick => (SIDE_CARD_W, QS_CARD_H),
        FlyoutPlace::Side => (SIDE_CARD_W, SIDE_CARD_H),
        FlyoutPlace::Widgets => (WIDGETS_CARD_W, WIDGETS_CARD_H),
        FlyoutPlace::Keyboard => (820, 340),
        FlyoutPlace::Lock => (460, 360),
        FlyoutPlace::Osd => (340, 72),
        FlyoutPlace::AltTab => (720, 160),
        FlyoutPlace::TaskView => (1100, 720),
        FlyoutPlace::SnapFlyout => (380, 420),
        FlyoutPlace::Volume => (360, 120),
        FlyoutPlace::WinMenu => (300, 560),
        FlyoutPlace::SysMenu => (240, 300),
        // W390 expanded Win+X list — tall enough that items are not scrolled.
        FlyoutPlace::QuickLink => (300, 700),
        FlyoutPlace::TaskbarMenu => (280, 320),
        FlyoutPlace::JumpList => (300, 400),
        FlyoutPlace::AppCtx => (280, 320),
        FlyoutPlace::DesktopMenu => (280, 520),
        FlyoutPlace::Run => (420, 120),
        // Room for ≤2 stacked toasts (W47/W75/W136/W196).
        FlyoutPlace::Toast => (380, 360),
        FlyoutPlace::Connect => (CONNECT_CARD_W, CONNECT_CARD_H),
        FlyoutPlace::SharePill => (420, 56),
        // Full-zone content (dimmer + toolbar / toast) — no mica card wrap.
        FlyoutPlace::Snip => (0, 0),
    };
    if place == FlyoutPlace::Snip {
        return content;
    }
    let card = flyout_card(content, palette, width, height);
    let bottom_dock = |card: Element<'a, Message>, align: DockAlign| -> Element<'a, Message> {
        let row = match align {
            DockAlign::Left => row![card, Space::with_width(Fill)].padding([0, 16]),
            DockAlign::Center => {
                row![Space::with_width(Fill), card, Space::with_width(Fill)].padding([0, 16])
            }
            DockAlign::Right => row![Space::with_width(Fill), card].padding([0, 16]),
        };
        column![Space::with_height(Fill), row, Space::with_height(12)]
            .width(Fill)
            .height(Fill)
            .into()
    };
    let placed: Element<'a, Message> = match place {
        // Left-aligned taskbar: Start / Win+X / jump lists dock above the left cluster.
        FlyoutPlace::Start
        | FlyoutPlace::Search
        | FlyoutPlace::QuickLink
        | FlyoutPlace::JumpList
        | FlyoutPlace::AppCtx
        | FlyoutPlace::WinMenu
        | FlyoutPlace::SysMenu
            if start_left =>
        {
            bottom_dock(card, DockAlign::Left)
        }
        // Centered taskbar (or desktop menu): card sits centered above the bar.
        FlyoutPlace::Start
        | FlyoutPlace::Search
        | FlyoutPlace::TaskView
        | FlyoutPlace::JumpList
        | FlyoutPlace::AppCtx
        | FlyoutPlace::DesktopMenu
        | FlyoutPlace::WinMenu
        | FlyoutPlace::SysMenu
        | FlyoutPlace::QuickLink => bottom_dock(card, DockAlign::Center),
        // Taskbar / clock menu: anchor near click X (fallback bottom-right).
        FlyoutPlace::TaskbarMenu => {
            if let Some(x) = taskbar_menu_x {
                let card_w = 280.0_f32;
                let width_est = pointer_x::max_seen()
                    .unwrap_or(x + card_w)
                    .max(x + 48.0)
                    .max(card_w + 32.0);
                let left = (x - card_w / 2.0).clamp(16.0, (width_est - card_w - 16.0).max(16.0));
                column![
                    Space::with_height(Fill),
                    row![
                        Space::with_width(left as u16),
                        card,
                        Space::with_width(Fill),
                    ],
                    Space::with_height(12),
                ]
                .width(Fill)
                .height(Fill)
                .into()
            } else {
                bottom_dock(card, DockAlign::Right)
            }
        }
        // Win11 Widgets Board docks to the left.
        FlyoutPlace::Widgets => bottom_dock(card, DockAlign::Left),
        // W256: toast can align start/left via extras notifications.toast_left.
        FlyoutPlace::Toast if toast_left => bottom_dock(card, DockAlign::Left),
        // Quick Settings / Notification Center / Connect / toast: bottom-right (W75/W449).
        FlyoutPlace::Quick | FlyoutPlace::Side | FlyoutPlace::Toast | FlyoutPlace::Connect => {
            bottom_dock(card, DockAlign::Right)
        }
        FlyoutPlace::Keyboard
        | FlyoutPlace::Osd
        | FlyoutPlace::AltTab
        | FlyoutPlace::SnapFlyout
        | FlyoutPlace::Volume
        | FlyoutPlace::SharePill
        | FlyoutPlace::Run => {
            column![
                Space::with_height(Fill),
                row![Space::with_width(Fill), card, Space::with_width(Fill)],
                Space::with_height(match place {
                    FlyoutPlace::Osd => 72,
                    FlyoutPlace::AltTab => 64,
                    FlyoutPlace::SnapFlyout => 80,
                    FlyoutPlace::Volume => 64,
                    FlyoutPlace::SharePill => 64,
                    FlyoutPlace::Run => 72,
                    _ => 12,
                }),
            ]
            .width(Fill)
            .height(Fill)
            .into()
        }
        FlyoutPlace::Lock => container(card).center(Fill).into(),
        // Unreachable: Snip returns before mica card wrap above.
        FlyoutPlace::Snip => card,
    };
    if place == FlyoutPlace::Osd
        || place == FlyoutPlace::AltTab
        || place == FlyoutPlace::Toast
        || place == FlyoutPlace::SharePill
    {
        return placed;
    }
    stack![
        mouse_area(Space::with_width(Fill).height(Fill)).on_press(Message::Dismiss),
        placed,
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Quiet glass buttons: no solid fill until hover, rounded like Windows 11.
pub(crate) fn quiet<'a>(content: impl Into<Element<'a, Message>>) -> iced::widget::Button<'a, Message> {
    button(content).style(glass_button).padding([6, 8])
}

/// Colored accent swatch for Start (W164).
pub(crate) fn accent_swatch_btn(hex: &'static str, current: &str) -> Element<'static, Message> {
    let selected = current.eq_ignore_ascii_case(hex);
    let rgb = Palette::parse_rgb(hex).unwrap_or((0.0, 0.47, 0.83));
    let color = Color::from_rgb(rgb.0, rgb.1, rgb.2);
    button(Space::with_width(1).height(1))
        .width(22)
        .height(22)
        .padding(0)
        .on_press(Message::SetThemeAccent(hex.to_string()))
        .style(move |_theme: &Theme, status| {
            let border_w = if selected { 2.0 } else { 1.0 };
            let alpha = match status {
                button::Status::Hovered | button::Status::Pressed => 1.0,
                _ => 0.95,
            };
            button::Style {
                background: Some(Background::Color(Color::from_rgba(
                    color.r, color.g, color.b, alpha,
                ))),
                border: Border {
                    color: if selected {
                        Color::WHITE
                    } else {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.25)
                    },
                    width: border_w,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

pub(crate) fn chrome_btn(
    icon: &str,
    fallback: &str,
    px: u32,
    dark_ui: bool,
) -> iced::widget::Button<'static, Message> {
    quiet(app_face(icon, fallback.to_string(), px, false, dark_ui))
}

pub(crate) fn app_face(
    icon: &str,
    label: String,
    px: u32,
    with_label: bool,
    dark_ui: bool,
) -> Element<'static, Message> {
    // Light glyphs on dark mica; dark glyphs on light mica (Win11 + Classic Start).
    let glyph = if icon.is_empty() {
        None
    } else {
        mde_theme::icons::for_ui(icon, px, dark_ui)
    };
    let mark = label.chars().next().unwrap_or('•').to_string();
    match (glyph, with_label) {
        (Some(path), true) => {
            row![image(path).width(px as u16).height(px as u16), text(label).size(13)]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
        }
        (Some(path), false) => image(path).width(px as u16).height(px as u16).into(),
        (None, true) => text(label).size(13).into(),
        (None, false) => text(mark).size(13).into(),
    }
}

/// W175: format calculator result as “= N”.
pub(crate) fn format_math_result(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("= {}", v as i64)
    } else if v.is_finite() {
        let s = format!("= {:.10}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        "= ?".into()
    }
}

/// W175: tiny safe arithmetic parser (digits + +-*/ and parentheses). No eval crate.
pub(crate) fn try_eval_math(input: &str) -> Option<f64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_op = s.chars().any(|c| matches!(c, '+' | '-' | '*' | '/'));
    if !has_digit || !has_op {
        return None;
    }
    if !s.chars().all(|c| {
        c.is_ascii_digit() || matches!(c, '+' | '-' | '*' | '/' | '.' | '(' | ')' | ' ')
    }) {
        return None;
    }
    let mut p = MathParser { src: s.as_bytes(), i: 0 };
    let v = p.parse_expr()?;
    p.skip_ws();
    if p.i != p.src.len() {
        return None;
    }
    if !v.is_finite() {
        return None;
    }
    Some(v)
}

pub(crate) struct MathParser<'a> {
    src: &'a [u8],
    i: usize,
}

impl<'a> MathParser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.src.len() && self.src[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn parse_expr(&mut self) -> Option<f64> {
        let mut v = self.parse_term()?;
        loop {
            self.skip_ws();
            let Some(&op) = self.src.get(self.i) else { break };
            if op == b'+' || op == b'-' {
                self.i += 1;
                let rhs = self.parse_term()?;
                v = if op == b'+' { v + rhs } else { v - rhs };
            } else {
                break;
            }
        }
        Some(v)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut v = self.parse_factor()?;
        loop {
            self.skip_ws();
            let Some(&op) = self.src.get(self.i) else { break };
            if op == b'*' || op == b'/' {
                self.i += 1;
                let rhs = self.parse_factor()?;
                if op == b'*' {
                    v *= rhs;
                } else if rhs == 0.0 {
                    return None;
                } else {
                    v /= rhs;
                }
            } else {
                break;
            }
        }
        Some(v)
    }

    fn parse_factor(&mut self) -> Option<f64> {
        self.skip_ws();
        let c = *self.src.get(self.i)?;
        if c == b'+' {
            self.i += 1;
            return self.parse_factor();
        }
        if c == b'-' {
            self.i += 1;
            return Some(-self.parse_factor()?);
        }
        if c == b'(' {
            self.i += 1;
            let v = self.parse_expr()?;
            self.skip_ws();
            if self.src.get(self.i).copied() != Some(b')') {
                return None;
            }
            self.i += 1;
            return Some(v);
        }
        self.parse_number()
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.i;
        while self.i < self.src.len() && self.src[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i < self.src.len() && self.src[self.i] == b'.' {
            self.i += 1;
            while self.i < self.src.len() && self.src[self.i].is_ascii_digit() {
                self.i += 1;
            }
        }
        if self.i == start {
            return None;
        }
        std::str::from_utf8(&self.src[start..self.i])
            .ok()?
            .parse::<f64>()
            .ok()
    }
}

pub(crate) fn bar_icon_btn(
    icon: &str,
    label: String,
    running: bool,
    progress: bool,
    unread: u32,
    win_count: usize,
    show_label: bool,
    message: Message,
    dark_ui: bool,
) -> Element<'static, Message> {
    let tip = label.clone();
    let face = app_face(
        icon,
        tip,
        if show_label { 20 } else { 24 },
        show_label,
        dark_ui,
    );
    let accent = Color::from_rgb(0.0, 0.47, 0.83);
    // A004: unread count from Notify protocol; else W177 multi-window count.
    let badge: Element<'static, Message> = if unread > 0 {
        let n = if unread > 9 {
            "9+".into()
        } else {
            unread.to_string()
        };
        container(text(n).size(9))
            .padding([1, 4])
            .style(move |_| ContainerStyle {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else if win_count > 1 {
        let n = if win_count > 9 {
            "9+".into()
        } else {
            win_count.to_string()
        };
        container(text(n).size(9))
            .padding([1, 4])
            .style(move |_| ContainerStyle {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::with_height(0).into()
    };
    // A004/A083: thicker underline when Notify progress hint is present.
    let (h, w) = if progress {
        (5u16, 28u16)
    } else if running {
        (3u16, 20u16)
    } else {
        (3u16, 0u16)
    };
    let indicator = container(Space::with_height(h)).width(w).style(move |_| {
        ContainerStyle {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: 1.5.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });
    quiet(
        column![face, badge, indicator]
            .spacing(2)
            .align_x(iced::Alignment::Center),
    )
    .on_press(message)
    .padding(if show_label { [4, 10] } else { [6, 8] })
    .into()
}

pub(crate) fn start_pin_tile(
    icon: &str,
    label: String,
    message: Message,
    large: bool,
    dark_ui: bool,
) -> Element<'static, Message> {
    let short: String = label.chars().take(if large { 18 } else { 12 }).collect();
    let icon_px = if large { 48 } else { 32 };
    let width = if large {
        START_PIN_W * 2 + START_PIN_GAP
    } else {
        START_PIN_W
    };
    quiet(
        column![
            app_face(icon, label, icon_px, false, dark_ui),
            text(short).size(if large { 12 } else { 11 }),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .width(Fill),
    )
    .on_press(message)
    .width(width)
    .padding(if large { [12, 8] } else { [8, 4] })
    .clip(true)
    .into()
}

/// Horizontal quick toggle. `width` is the outer button width, padding included.
fn qs_button(
    icon: &str,
    label: &str,
    message: Message,
    width: u16,
    dark_ui: bool,
    style: fn(&Theme, button::Status) -> button::Style,
) -> Element<'static, Message> {
    let face = row![
        app_face(icon, label.to_string(), 16, false, dark_ui),
        text(label.to_string()).size(12),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .width(Fill);
    button(face)
        .on_press(message)
        .width(width)
        .height(QS_TILE_H)
        .padding([6, 12])
        .clip(true)
        .style(style)
        .into()
}

pub(crate) fn qs_tile(
    icon: &str,
    label: &'static str,
    active: bool,
    message: Message,
    width: u16,
    dark_ui: bool,
) -> Element<'static, Message> {
    let style = if active { qs_tile_on } else { qs_tile_off };
    qs_button(icon, label, message, width, dark_ui, style)
}

/// W267: Dark mode tile — quiet “Scheduled” when theme.dark_schedule is on.
pub(crate) fn qs_dark_tile(
    icon: &str,
    active: bool,
    scheduled: bool,
    message: Message,
    width: u16,
    dark_ui: bool,
) -> Element<'static, Message> {
    let label = if scheduled { "Scheduled" } else { "Dark mode" };
    let style = if active { qs_tile_on } else { qs_tile_off };
    qs_button(icon, label, message, width, dark_ui, style)
}

pub(crate) fn qs_tile_on(theme: &Theme, status: button::Status) -> button::Style {
    let mut s = accent_pill(theme, status);
    s.border.radius = mde_theme::TILE_RADIUS.into();
    s
}

pub(crate) fn qs_tile_off(theme: &Theme, status: button::Status) -> button::Style {
    let text = theme.palette().text;
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(text.r, text.g, text.b, 0.14)
        }
        _ => Color::from_rgba(text.r, text.g, text.b, 0.08),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text,
        border: Border {
            radius: mde_theme::TILE_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Dimmed QS tile when airplane mode forces wifi/bt off (W227).
pub(crate) fn qs_tile_dim(
    icon: &str,
    label: &'static str,
    message: Message,
    width: u16,
    dark_ui: bool,
) -> Element<'static, Message> {
    qs_button(icon, label, message, width, dark_ui, qs_tile_dim_style)
}

pub(crate) fn qs_tile_dim_style(theme: &Theme, status: button::Status) -> button::Style {
    let text = theme.palette().text;
    let muted = Color::from_rgba(text.r, text.g, text.b, 0.38);
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(text.r, text.g, text.b, 0.06)
        }
        _ => Color::from_rgba(text.r, text.g, text.b, 0.03),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: muted,
        border: Border {
            radius: mde_theme::TILE_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn mute_flash_style(theme: &Theme, status: button::Status) -> button::Style {
    let danger = theme.palette().danger;
    let alpha = match status {
        button::Status::Hovered | button::Status::Pressed => 1.0,
        _ => 0.88,
    };
    button::Style {
        background: Some(Background::Color(Color { a: alpha, ..danger })),
        text_color: Color::WHITE,
        border: Border {
            radius: mde_theme::CONTROL_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Persistent muted volume glyph tint after flash (W230).
pub(crate) fn mute_persistent_style(theme: &Theme, status: button::Status) -> button::Style {
    let danger = theme.palette().danger;
    let text = theme.palette().text;
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(
                danger.r, danger.g, danger.b, 0.28,
            )))
        }
        _ => Some(Background::Color(Color::from_rgba(
            danger.r, danger.g, danger.b, 0.16,
        ))),
    };
    button::Style {
        background,
        text_color: text,
        border: Border {
            radius: mde_theme::CONTROL_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Emphasize the matched query substring for Start search rows (W55).
pub(crate) fn highlight_match(label: &str, query: &str) -> String {
    let q = query.trim();
    if q.is_empty() {
        return label.to_string();
    }
    let lower = label.to_lowercase();
    let q_lower = q.to_lowercase();
    if let Some(pos) = lower.find(&q_lower) {
        let end = pos + q_lower.len();
        // Keep original casing; mark the hit with « » for a light visual cue.
        format!(
            "{}«{}»{}",
            &label[..pos],
            &label[pos..end],
            &label[end..]
        )
    } else {
        label.to_string()
    }
}

pub(crate) fn section_label(label: &'static str) -> Element<'static, Message> {
    text(label).size(13).into()
}

pub(crate) fn hairline(palette: &Palette) -> Element<'static, Message> {
    let edge = mde_theme::flyout_edge(palette);
    container(Space::with_height(1))
        .width(Fill)
        .style(move |_| ContainerStyle {
            background: Some(Background::Color(edge)),
            ..Default::default()
        })
        .into()
}

pub(crate) fn show_desktop_strip() -> Element<'static, Message> {
    // Win11 far-right peek: 2px vertical hit target.
    let edge = Color::from_rgba(1.0, 1.0, 1.0, 0.55);
    quiet(
        container(Space::with_width(2).height(40)).style(move |_| ContainerStyle {
            background: Some(Background::Color(edge)),
            border: Border {
                radius: 0.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .on_press(Message::ShowDesktop)
    .padding([4, 2])
    .into()
}

pub(crate) fn flyout_inset<'a>(
    content: impl Into<Element<'a, Message>>,
    palette: &Palette,
) -> Element<'a, Message> {
    let fill = if palette.dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.06)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.04)
    };
    let edge = mde_theme::flyout_edge(palette);
    // Inset cards share flyout mica radius family (8px controls inside 12px shells).
    let radius = mde_theme::CONTROL_RADIUS;
    container(content)
        .padding(12)
        .width(Fill)
        .style(move |_| ContainerStyle {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: 1.0,
                radius: radius.into(),
            },
            ..Default::default()
        })
        .into()
}

pub(crate) fn level_track(level: f32, palette: &Palette) -> Element<'static, Message> {
    let accent = mde_theme::accent(palette);
    let track = if palette.dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.14)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.12)
    };
    let fill_w = ((level.clamp(0.0, 1.0)) * 320.0) as u16;
    // Win11 QS meters: thick track (W37).
    let h = 10u16;
    container(
        container(Space::with_width(fill_w.max(6)).height(h)).style(move |_| ContainerStyle {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: 5.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .width(Fill)
    .height(h)
    .style(move |_| ContainerStyle {
        background: Some(Background::Color(track)),
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Thin progress bar for toast progress hints (W116 / W438).
pub(crate) fn thin_progress(level: f32, palette: &Palette) -> Element<'static, Message> {
    let accent = mde_theme::accent(palette);
    let track = if palette.dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.12)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.10)
    };
    let fill_w = ((level.clamp(0.0, 1.0)) * 280.0) as u16;
    let h = 3u16;
    container(
        container(Space::with_width(fill_w.max(4)).height(h)).style(move |_| ContainerStyle {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .width(Fill)
    .height(h)
    .style(move |_| ContainerStyle {
        background: Some(Background::Color(track)),
        border: Border {
            radius: 2.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}


pub(crate) fn glass_button(theme: &Theme, status: button::Status) -> button::Style {
    let text = theme.palette().text;
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(text.r, text.g, text.b, 0.10)))
        }
        _ => None,
    };
    button::Style {
        background,
        text_color: text,
        border: Border {
            radius: mde_theme::CONTROL_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn accent_pill(theme: &Theme, status: button::Status) -> button::Style {
    let accent = theme.palette().primary;
    let alpha = match status {
        button::Status::Hovered | button::Status::Pressed => 1.0,
        _ => 0.92,
    };
    button::Style {
        background: Some(Background::Color(Color { a: alpha, ..accent })),
        text_color: Color::WHITE,
        border: Border {
            radius: mde_theme::CONTROL_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn start_search_id() -> iced::widget::text_input::Id {
    iced::widget::text_input::Id::new("mde-start-search")
}


pub(crate) fn unlock_req_path() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::Path::new(&runtime).join("mde-unlock.req")
}

pub(crate) fn write_unlock_req() {
    let path = unlock_req_path();
    let _ = std::fs::write(&path, b"unlock\n");
}

/// W283: extras `clipboard.max_items` (default 25) else profile clipboard.max_items.
pub(crate) fn clip_max_from_cfg(cfg: &MdeConfig) -> usize {
    let from_extras = cfg.extras.get("clipboard.max_items").and_then(|v| {
        v.as_integer()
            .map(|i| i as u32)
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    });
    from_extras
        .unwrap_or_else(|| cfg.active().clipboard.max_items)
        .clamp(5, 100) as usize
}

/// Best-effort Sign out via loginctl (session first, then user).
pub(crate) fn sign_out_best_effort() -> String {
    if let Ok(session) = std::env::var("XDG_SESSION_ID") {
        if !session.is_empty() {
            return match std::process::Command::new("loginctl")
                .args(["terminate-session", &session])
                .spawn()
            {
                Ok(_) => format!("Signing out (session {session})"),
                Err(e) => format!("Sign out failed: {e}"),
            };
        }
    }
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() {
        return "Sign out: no USER / XDG_SESSION_ID".into();
    }
    match std::process::Command::new("loginctl")
        .args(["terminate-user", &user])
        .spawn()
    {
        Ok(_) => format!("Signing out (user {user})"),
        Err(e) => format!("Sign out failed: {e}"),
    }
}

pub(crate) fn battery_icon_name(pct: u8, charging: bool) -> &'static str {
    if charging {
        return "battery-good-charging";
    }
    match pct {
        0..=15 => "battery-caution",
        16..=40 => "battery-low",
        41..=70 => "battery-good",
        _ => "battery-full",
    }
}

pub(crate) fn clock_now_cfg(iso: bool, seconds: bool) -> String {
    format!("{}  {}", clock_date_cfg(iso), clock_time_cfg(seconds))
}

pub(crate) fn clock_time() -> String {
    clock_time_cfg(false)
}

/// Taskbar clock; optional seconds (W237).
pub(crate) fn clock_time_cfg(seconds: bool) -> String {
    if seconds {
        mde_theme::now().format("%-I:%M:%S %p").to_string()
    } else {
        mde_theme::now().format("%-I:%M %p").to_string()
    }
}

/// Soft-lock / lock avatar path (W240).
pub(crate) fn lock_avatar_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    for rel in [".face", ".local/share/mde/avatar.png"] {
        let p = std::path::PathBuf::from(&home).join(rel);
        if p.is_file() {
            return Some(p.display().to_string());
        }
    }
    None
}

/// QS “Open volume mixer” → pavucontrol / pwvucontrol (W238).
pub(crate) fn open_volume_mixer() -> String {
    for bin in ["pavucontrol", "pwvucontrol"] {
        if std::process::Command::new(bin).spawn().is_ok() {
            return format!("Volume mixer · {bin}");
        }
    }
    "Volume mixer · install pavucontrol or pwvucontrol".into()
}

/// Recommended “Open location” → xdg-open parent / desktop dir (W244).
pub(crate) fn open_recommended_location(id: &str) -> String {
    let path = std::path::Path::new(id);
    if path.is_file() || id.contains('/') {
        if let Some(parent) = path.parent() {
            if std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .is_ok()
            {
                return format!("Open location · {}", parent.display());
            }
        }
        return "Open location · failed".into();
    }
    match apps::resolve_desktop(id) {
        Some(desk) => {
            if let Some(parent) = desk.parent() {
                if std::process::Command::new("xdg-open")
                    .arg(parent)
                    .spawn()
                    .is_ok()
                {
                    return format!("Open location · {}", parent.display());
                }
            }
            "Open location · failed".into()
        }
        None => "Open location · not found".into(),
    }
}

pub(crate) fn clock_date_cfg(iso: bool) -> String {
    // W93: toggle between weekday M/D and ISO date.
    if iso {
        mde_theme::now().format("%Y-%m-%d").to_string()
    } else {
        mde_theme::now().format("%a %-m/%-d").to_string()
    }
}

pub(crate) fn clock_long_date() -> String {
    mde_theme::now().format("%A, %B %-d").to_string()
}

/// A098: format a world-clock line via `TZ=<id> date` (IANA or UTC±N).
pub(crate) fn world_clock_label(tz: &str) -> String {
    let tz = tz.trim();
    if tz.is_empty() {
        return String::new();
    }
    let out = std::process::Command::new("date")
        .env("TZ", tz)
        .arg("+%H:%M %Z")
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let short = tz.rsplit('/').next().unwrap_or(tz);
            format!("{short} · {t}")
        }
        _ => format!("{tz} · —"),
    }
}

/// Super+R Run: gtk-launch / xdg-open / sh -c (W141).
pub(crate) fn launch_run_command(cmd: &str) -> String {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return "Run · empty".into();
    }
    // W459: MMC consoles — `services.msc`, `compmgmt.msc`, bare `mmc`, …
    {
        let lower = cmd.to_ascii_lowercase();
        let base = lower.split_whitespace().next().unwrap_or("");
        let msc = matches!(
            base,
            "mmc"
                | "compmgmt.msc"
                | "devmgmt.msc"
                | "diskmgmt.msc"
                | "lusrmgr.msc"
                | "services.msc"
                | "eventvwr.msc"
                | "taskschd.msc"
                | "perfmon.msc"
                | "network.msc"
                | "firewall.msc"
                | "packages.msc"
        ) || base.ends_with(".msc");
        if msc {
            let arg = if base == "mmc" { None } else { Some(base) };
            let mut c = std::process::Command::new("mde-mmc");
            if let Some(a) = arg {
                c.arg(a);
            }
            return match c.spawn() {
                Ok(_) => format!("Run · mde-mmc {base}"),
                Err(e) => format!("Run · mde-mmc failed: {e}"),
            };
        }
    }
    // Desktop id without path → gtk-launch.
    if !cmd.contains('/')
        && !cmd.contains(' ')
        && (cmd.ends_with(".desktop") || !cmd.contains('.'))
    {
        let id = cmd.trim_end_matches(".desktop");
        if std::process::Command::new("gtk-launch").arg(id).spawn().is_ok() {
            return format!("Run · gtk-launch {id}");
        }
    }
    // Existing path or URL-ish → xdg-open.
    let looks_uri = cmd.contains("://") || cmd.starts_with('/');
    if looks_uri
        && std::process::Command::new("xdg-open")
            .arg(cmd)
            .spawn()
            .is_ok()
    {
        return format!("Run · xdg-open {cmd}");
    }
    match std::process::Command::new("sh").args(["-c", cmd]).spawn() {
        Ok(_) => format!("Run · {cmd}"),
        Err(e) => format!("Run failed: {e}"),
    }
}

/// Launch first available terminal (W161 quick link).
/// A081: Ctrl+Shift+Esc / Quick Link Task Manager → MMC Performance.
pub(crate) fn spawn_task_manager() -> String {
    match std::process::Command::new("mde-mmc")
        .arg("perfmon.msc")
        .spawn()
    {
        Ok(_) => "Performance · mde-mmc perfmon.msc".into(),
        Err(e) => format!("Performance · mde-mmc: {e}"),
    }
}

pub(crate) fn spawn_admin_tool(label: &str, bins: &[&str]) -> String {
    for bin in bins {
        if std::process::Command::new(bin).spawn().is_ok() {
            return format!("{label} · {bin}");
        }
    }
    format!("{label} · none found")
}

pub(crate) fn spawn_terminal() -> String {
    for bin in ["weston-terminal", "kitty", "foot", "alacritty", "gnome-terminal"] {
        if std::process::Command::new(bin).spawn().is_ok() {
            return format!("Terminal · {bin}");
        }
    }
    "Terminal · none found".into()
}

/// W407: pick an image (zenity) and write `~/.face` + `~/.config/mde/avatar.png`.
pub(crate) fn set_account_picture() -> String {
    let Ok(home) = std::env::var("HOME") else {
        return "HOME unset".into();
    };
    let picked = std::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--title=Choose account picture",
            "--file-filter=Images | *.png *.jpg *.jpeg *.webp *.svg",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(src) = picked else {
        return "Change picture · cancelled".into();
    };
    let face = std::path::PathBuf::from(&home).join(".face");
    let avatar = std::path::PathBuf::from(&home)
        .join(".config")
        .join("mde")
        .join("avatar.png");
    if let Some(parent) = avatar.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::copy(&src, &face) {
        Ok(_) => {
            let _ = std::fs::copy(&src, &avatar);
            format!("Account picture · {}", face.display())
        }
        Err(e) => format!("Account picture · {e}"),
    }
}

/// W409: Feedback Hub — copy recent journal + open mailto.
pub(crate) fn open_feedback_hub() -> String {
    let journal = std::process::Command::new("journalctl")
        .args(["--user", "-n", "40", "--no-pager"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let body = format!("MDE feedback\n\n--- journal (user, last 40) ---\n{journal}\n");
    let _ = std::process::Command::new("wl-copy").arg(&body).status();
    let mailto = "mailto:?subject=MDE%20feedback";
    if std::process::Command::new("xdg-open").arg(mailto).spawn().is_ok() {
        return "Feedback · journal copied · mail client".into();
    }
    let _ = std::process::Command::new("xdg-open")
        .arg("https://github.com/")
        .spawn();
    "Feedback · journal copied to clipboard".into()
}

/// W403 / W422 / A070: compositor-native scale magnifier preferred; external tools optional.
pub(crate) fn spawn_magnifier(on: bool) -> String {
    let cfg = mde_config::MdeConfig::load().unwrap_or_default();
    let a = &cfg.active().accessibility;
    let factor = a.magnifier_factor.clamp(1.25, 4.0);
    let native = a.magnifier_native;
    // Publish intent for compositor follow-focus / lens consumers.
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let _ = std::fs::write(
        std::path::Path::new(&runtime).join("mde-magnifier.json"),
        format!(
            "{{\"on\":{},\"factor\":{},\"mode\":\"{}\",\"follow_focus\":{}}}",
            on, factor, a.magnifier_mode, a.magnifier_follow_focus
        ),
    );
    if on {
        if native {
            match set_output_scale_nudge(factor) {
                Ok(s) => {
                    return format!(
                        "Magnifier · compositor ×{factor:.2} ({}) · {s}",
                        a.magnifier_mode
                    );
                }
                Err(e) => {
                    // Fall through to external tools.
                    tracing::warn!(%e, "native magnifier scale failed");
                }
            }
        }
        for bin in ["magnus", "kmag", "gnome-magnifier", "xzoom"] {
            if std::process::Command::new(bin).spawn().is_ok() {
                return format!("Magnifier · {bin}");
            }
        }
        match set_output_scale_nudge(factor) {
            Ok(s) => format!("Magnifier · scale {s}"),
            Err(e) => {
                let _ = std::process::Command::new("mde-control")
                    .arg("accessibility")
                    .spawn();
                format!("Magnifier · {e} · opened Settings")
            }
        }
    } else {
        for bin in ["magnus", "kmag", "gnome-magnifier", "xzoom"] {
            let _ = std::process::Command::new("pkill").arg("-x").arg(bin).status();
        }
        match set_output_scale_nudge(1.0) {
            Ok(s) => format!("Magnifier off · scale {s}"),
            Err(_) => "Magnifier off".into(),
        }
    }
}

fn set_output_scale_nudge(scale: f64) -> Result<String, String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::Path::new(&runtime).join("mde-output-layout.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out = String::new();
    let mut touched = false;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // name x y w h [scale]
        if parts.len() >= 5 {
            let name = parts[0];
            let x = parts[1];
            let y = parts[2];
            let w = parts[3];
            let h = parts[4];
            out.push_str(&format!("{name} {x} {y} {w} {h} {scale}\n"));
            touched = true;
        } else if !line.trim().is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !touched {
        // Seed from live outputs snapshot when layout file is empty.
        let snap = std::path::Path::new(&runtime).join("mde-outputs.json");
        let body = std::fs::read_to_string(snap).map_err(|e| e.to_string())?;
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let Some(arr) = v.get("outputs").and_then(|o| o.as_array()) else {
            return Err("no outputs".into());
        };
        for o in arr {
            let name = o.get("name").and_then(|n| n.as_str()).unwrap_or("eDP-1");
            let x = o.get("x").and_then(|n| n.as_i64()).unwrap_or(0);
            let y = o.get("y").and_then(|n| n.as_i64()).unwrap_or(0);
            let w = o.get("width").and_then(|n| n.as_i64()).unwrap_or(1920);
            let h = o.get("height").and_then(|n| n.as_i64()).unwrap_or(1080);
            out.push_str(&format!("{name} {x} {y} {w} {h} {scale}\n"));
            touched = true;
        }
    }
    if !touched {
        return Err("no layout heads".into());
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(format!("{scale}"))
}


/// W421: uninstall via Flatpak or packagekit/dnf (best-effort; may need polkit).
pub(crate) fn uninstall_app(id: &str) -> String {
    let (kind, label) = apps::detect_uninstall(id);
    let status = apps::run_uninstall(&kind);
    if label.is_empty() {
        status
    } else if status.contains("Uninstalled") {
        format!("{status} ({label})")
    } else {
        status
    }
}

/// Super+. emoji picker stub (W90).
pub(crate) fn spawn_emoji_picker() -> String {
    for bin in ["smile", "gnome-characters"] {
        if std::process::Command::new(bin).spawn().is_ok() {
            return format!("Emoji panel · {bin}");
        }
    }
    "Emoji panel · use system picker".into()
}

/// W195: true when Start query looks like an emoji shortcode or a single emoji.
pub(crate) fn query_wants_emoji(query: &str) -> bool {
    const SHORTCODES: &[&str] = &[
        "smile",
        "smiley",
        "grin",
        "laugh",
        "wink",
        "heart",
        "thumbsup",
        "thumbsdown",
        "fire",
        "star",
        "ok",
        "wave",
        "clap",
        "tada",
        "rocket",
        "eyes",
        "thinking",
        "cry",
        "sob",
        "joy",
    ];
    let raw = query.trim();
    if raw.is_empty() {
        return false;
    }
    // `:smile:` style shortcodes (anywhere in the query).
    let lower = raw.to_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if let Some(end) = bytes[i + 1..].iter().position(|&b| b == b':') {
                let name = &lower[i + 1..i + 1 + end];
                if SHORTCODES.iter().any(|s| *s == name)
                    || (name.len() >= 2
                        && name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_'))
                {
                    return true;
                }
                i += 1 + end;
                continue;
            }
        }
        i += 1;
    }
    // Single emoji / symbol grapheme (non-ASCII letter/digit).
    let mut chars = raw.chars();
    if let Some(c) = chars.next() {
        if chars.next().is_none() && !c.is_ascii() && !c.is_alphanumeric() {
            return true;
        }
        // Also treat a lone non-ASCII letter-like emoji presentation char.
        if raw.chars().count() == 1 && !c.is_ascii() {
            return true;
        }
    }
    false
}

/// Super+Shift+C color picker stub (W152).
pub(crate) fn spawn_color_picker() -> String {
    for bin in ["hyprpicker", "wl-color-picker"] {
        if which_bin(bin) {
            match std::process::Command::new(bin).spawn() {
                Ok(_) => return format!("Color picker · {bin}"),
                Err(e) => return format!("Color picker · {bin} failed: {e}"),
            }
        }
    }
    // grim + slurp pixel sample → clipboard (W427 fallback).
    if which_bin("grim") && which_bin("slurp") {
        let script = concat!(
            "set -e\n",
            "g=$(slurp -p) || exit 2\n",
            "f=$(mktemp --suffix=.ppm)\n",
            "grim -g \"$g\" -t ppm \"$f\"\n",
            "rgb=$(python3 - \"$f\" <<'PY'\n",
            "import sys\n",
            "p=sys.argv[1]\n",
            "with open(p,'rb') as f:\n",
            "    assert f.readline().startswith(b'P6')\n",
            "    while True:\n",
            "        line=f.readline()\n",
            "        if not line.startswith(b'#'):\n",
            "            # width height line consumed above when non-comment; re-read dims if needed\n",
            "            dims=line\n",
            "            break\n",
            "    _=f.readline()  # maxval\n",
            "    r,g,b=f.read(3)\n",
            "print(f'#{r:02X}{g:02X}{b:02X}')\n",
            "PY\n",
            ")\n",
            "rm -f \"$f\"\n",
            "if command -v wl-copy >/dev/null 2>&1; then\n",
            "  printf '%s' \"$rgb\" | wl-copy\n",
            "fi\n",
            "printf '%s' \"$rgb\"\n",
        );
        match std::process::Command::new("bash")
            .args(["-lc", script])
            .output()
        {
            Ok(o) if o.status.success() => {
                let hex = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if hex.is_empty() {
                    return "Color picker · sampled (empty result)".into();
                }
                return format!("Color picker · {hex} (copied)");
            }
            Ok(o) if o.status.code() == Some(2) => "Color picker · cancelled".into(),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if err.is_empty() {
                    "Color picker · grim/slurp failed".into()
                } else {
                    format!("Color picker · grim/slurp failed: {err}")
                }
            }
            Err(e) => format!("Color picker · spawn failed: {e}"),
        }
    } else {
        "Color picker · failed — install hyprpicker or wl-color-picker, or grim+slurp".into()
    }
}


pub(crate) fn which_bin(name: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        if dir.join(name).is_file() {
            return true;
        }
    }
    false
}

/// Open ~/.local/state/mde/ for crash / session artifacts (W154).
pub(crate) fn open_crash_reports() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let dir = std::path::PathBuf::from(&home).join(".local/state/mde");
    let shown = dir.display().to_string();
    if dir.is_dir() {
        files::open_path(&shown)?;
        Ok(shown)
    } else {
        Err(format!("Crash reports · {shown} (after first session)"))
    }
}


/// W459: best-effort network STT — record ~3s wav, POST to configured URL, fill Start query via status parse.
pub(crate) fn spawn_network_voice_search(url: &str) -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let wav = format!("{runtime}/mde-voice.wav");
    let out = format!("{runtime}/mde-voice.txt");
    let script = format!(
        r#"set -e
rm -f '{out}'
if command -v arecord >/dev/null 2>&1; then
  arecord -q -d 3 -f cd '{wav}' || true
elif command -v parec >/dev/null 2>&1; then
  timeout 3 parec --file-format=wav '{wav}' || true
else
  echo 'no recorder (arecord/parec)' > '{out}'
  exit 0
fi
if [ ! -s '{wav}' ]; then
  echo 'no audio captured' > '{out}'
  exit 0
fi
code=$(curl -sS -o '{out}' -w '%{{http_code}}' -F "file=@{wav}" '{url}' || echo 000)
if [ "$code" != "200" ] && [ "$code" != "201" ]; then
  echo "STT HTTP $code" > '{out}'
fi
"#
    );
    match std::process::Command::new("bash").args(["-lc", &script]).spawn() {
        Ok(_) => "Voice search · listening (network STT)…".into(),
        Err(e) => format!("Voice search · failed: {e}"),
    }
}

/// Minimal query encoding for DuckDuckGo (W95) — no extra crate.
pub(crate) fn urlencoding_query(q: &str) -> String {
    let mut out = String::with_capacity(q.len() * 3);
    for b in q.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// PackageKit offline update prepared (`/system-update` symlink).
pub(crate) fn offline_update_pending() -> bool {
    std::path::Path::new("/system-update").exists()
}

/// Settings › Windows Update marks busy in `updates.json` while working.
pub(crate) fn updates_busy() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let state = std::env::var("XDG_STATE_HOME")
        .unwrap_or_else(|_| format!("{home}/.local/state"));
    let path = format!("{state}/ws-settings/updates.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("busy").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

/// Trigger PackageKit offline install, then reboot into it (power-off or reboot after).
pub(crate) fn trigger_offline_update(mode: &str) -> String {
    if updates_busy() {
        return "Updates in progress…".into();
    }
    if !offline_update_pending() {
        return "No offline update prepared · open Windows Update".into();
    }
    let trigger = std::process::Command::new("mde-privileged")
        .args(["offline-trigger", mode])
        .status();
    match trigger {
        Ok(s) if s.success() => {
            let _ = std::process::Command::new("mde-privileged")
                .args(["power", "reboot"])
                .spawn();
            if mode == "power-off" {
                "Update and shut down…".into()
            } else {
                "Update and restart…".into()
            }
        }
        Ok(_) => "Couldn’t prepare offline update".into(),
        Err(e) => format!("Offline update failed: {e}"),
    }
}

/// Open ~/.local/state/mde/session.log (W114).
pub(crate) fn open_session_log() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let path = std::path::PathBuf::from(&home).join(".local/state/mde/session.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !path.exists() {
        let _ = std::fs::write(&path, "# MDE session log\n");
    }
    let shown = path.display().to_string();
    files::open_path(&shown)?;
    Ok(shown)
}

/// Short diagnostics line for About → Copy diagnostics (W194).
pub(crate) fn session_diagnostics_line() -> String {
    let ver = env!("CARGO_PKG_VERSION");
    let config = MdeConfig::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.config/mde/config.toml".into());
    let state = support_bundle_path();
    format!(
        "MDE v{ver} · {} · rustc N/A · config={config} · state={state}",
        mde_theme::PRODUCT
    )
}

/// Expand `~/…` to `$HOME/…` for folder path pins (W264).
pub(crate) fn expand_user_path(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    } else if id == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
    }
    id.to_string()
}

/// Super+Space layout tip from `setxkbmap -query` (W261).
pub(crate) fn input_language_status() -> String {
    let out = std::process::Command::new("setxkbmap")
        .arg("-query")
        .output();
    let Ok(out) = out else {
        return "Layout · tip: Settings · Keyboard".into();
    };
    if !out.status.success() {
        return "Layout · tip: Settings · Keyboard".into();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut layout = String::new();
    let mut variant = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("layout:") {
            layout = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("variant:") {
            variant = rest.trim().to_string();
        }
    }
    if layout.is_empty() {
        return "Layout · tip: Settings · Keyboard".into();
    }
    if variant.is_empty() {
        format!("Layout · {layout} · tip: Settings · Keyboard")
    } else {
        format!("Layout · {layout}({variant}) · tip: Settings · Keyboard")
    }
}

/// Session state dir tip for About → Support bundle (W234).
pub(crate) fn support_bundle_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    format!("{home}/.local/state/mde")
}

pub(crate) fn settings_page_id(key: &str) -> &str {
    match key {
        "settings" => "",
        "wifi" | "wlan" => "wifi",
        "network" | "network-internet" => "network",
        "nearby" | "kdeconnect" | "kde-connect" => "nearby",
        "night" | "night-light" | "blue-light" => "nightlight",
        "display" | "screen" | "displays" => "display",
        "audio" => "sound",
        "ease" | "easeofaccess" => "accessibility",
        "colours" | "color" | "dark-mode" => "colors",
        "wallpaper" => "background",
        "compmgmt" => "mmc",
        "editor" | "regedit" => "config",
        "windowsupdate" => "update",
        "time-language" => "time",
        other => other,
    }
}

pub(crate) fn qs_stub_tile(
    icon: &str,
    label: &'static str,
    width: u16,
    dark_ui: bool,
) -> Element<'static, Message> {
    qs_button(icon, label, Message::QsStub(label), width, dark_ui, qs_tile_off)
}

pub(crate) fn relative_age(created: Instant) -> String {
    let secs = created.elapsed().as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

pub(crate) fn settings_page_label(page: &str) -> String {
    match page.trim().to_ascii_lowercase().as_str() {
        "home" | "" => "Settings".into(),
        "network" | "wifi" => "Network & internet".into(),
        "nearby" => "Nearby Share".into(),
        "devices" => "Bluetooth & devices".into(),
        "bluetooth" => "Bluetooth".into(),
        "sound" | "mixer" => "Sound".into(),
        "display" | "nightlight" | "night" | "night-light" => "Display".into(),
        "personalization" | "colors" | "background" | "taskbar" => "Personalization".into(),
        "apps" | "installed" | "startup" => "Apps".into(),
        "system" | "about" | "clipboard" | "storage" | "power" => "System".into(),
        "notifications" | "focus" => "Notifications".into(),
        "accounts" | "yourinfo" | "signin" => "Accounts".into(),
        "time" | "datetime" | "language" => "Time & language".into(),
        "gaming" | "gamemode" | "captures" => "Gaming".into(),
        "accessibility" | "ease" | "textsize" | "contrast" => "Accessibility".into(),
        "privacy" | "security" | "camera" | "microphone" => "Privacy & security".into(),
        "update" | "updatehistory" => "Windows Update".into(),
        "advanced" | "mmc" | "config" => "Advanced".into(),
        other => other.to_string(),
    }
}

pub(crate) fn parse_hhmm_minutes(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

pub(crate) fn on_off(v: bool) -> &'static str {
    if v {
        "on"
    } else {
        "off"
    }
}

fn load_outputs_snapshot() -> Vec<mde_protocol::OutputInfo> {
    let path = mde_protocol::outputs_json_path();
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<mde_protocol::OutputsSnapshot>(&text)
        .map(|s| s.outputs)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        overlay_frame, taskbar_is_left_aligned, Anchor, FlyoutInput, FlyoutPlace, CONNECT_CARD_H,
        CONNECT_CARD_W, QS_CARD_H, QS_GAP, QS_TILE_H, QS_TILE_W, SIDE_CARD_H, SIDE_CARD_W,
        SIDE_PAD_X, START_CARD_W, START_PAD_X, START_PIN_GAP, START_PIN_W, WIDGETS_CARD_H,
        WIDGETS_CARD_W,
    };

    #[test]
    fn start_menu_fills_zone_for_click_away() {
        let frame = overlay_frame(FlyoutInput {
            start: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.width, 0);
        assert_eq!(frame.height, 0);
        assert!(frame.dismiss);
        assert_eq!(frame.place, FlyoutPlace::Start);
        assert!(frame.anchor.contains(Anchor::Left));
        assert!(frame.anchor.contains(Anchor::Right));
    }

    #[test]
    fn taskbar_left_align_follows_bool_or_align_string() {
        assert!(taskbar_is_left_aligned(true, "center"));
        assert!(taskbar_is_left_aligned(false, "left"));
        assert!(taskbar_is_left_aligned(false, "Left"));
        assert!(!taskbar_is_left_aligned(false, "center"));
    }

    #[test]
    fn side_menus_also_fill_for_click_away() {
        let frame = overlay_frame(FlyoutInput {
            side: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::Side);
        assert!(frame.dismiss);
        assert!(frame.anchor.contains(Anchor::Top));
    }

    #[test]
    fn quick_settings_fills_zone_with_compact_place() {
        let frame = overlay_frame(FlyoutInput {
            quick: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::Quick);
        assert!(frame.dismiss);
        assert_eq!(frame.width, 0);
    }

    #[test]
    fn connect_flyout_fills_zone() {
        let frame = overlay_frame(FlyoutInput {
            connect: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::Connect);
        assert!(frame.dismiss);
        assert_eq!(CONNECT_CARD_W, 400);
        assert_eq!(CONNECT_CARD_H, 420);
    }

    #[test]
    fn share_pill_is_compact_non_dismiss() {
        let frame = overlay_frame(FlyoutInput {
            share_pill: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::SharePill);
        assert!(!frame.dismiss);
        assert_eq!(frame.width, 420);
        assert_eq!(frame.height, 56);
    }

    #[test]
    fn osd_stays_a_compact_pill() {
        let frame = overlay_frame(FlyoutInput {
            osd: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.width, 340);
        assert!(!frame.dismiss);
        assert_eq!(frame.place, FlyoutPlace::Osd);
    }

    #[test]
    fn closed_flyout_has_no_surface() {
        assert!(overlay_frame(FlyoutInput::default()).is_none());
    }

    #[test]
    fn task_view_fills_zone() {
        let frame = overlay_frame(FlyoutInput {
            taskview: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::TaskView);
        assert!(frame.dismiss);
        assert_eq!(frame.width, 0);
    }

    #[test]
    fn alttab_toast_is_compact() {
        let frame = overlay_frame(FlyoutInput {
            alttab: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::AltTab);
        assert!(!frame.dismiss);
        assert_eq!(frame.width, 720);
    }

    #[test]
    fn quick_settings_tiles_fit_the_card() {
        let inner = SIDE_CARD_W - SIDE_PAD_X * 2;
        let row = QS_TILE_W * 3 + QS_GAP * 2;
        assert!(row <= inner, "qs row {row}px exceeds card inner {inner}px");
        assert!(QS_TILE_W >= 100, "qs tile {QS_TILE_W}px is too narrow to read");
    }

    #[test]
    fn quick_settings_card_fits_without_scroll() {
        // Compact budget: pad + 3 tile rows + gaps + volume + brightness + media + footer.
        const PAD: u16 = 16 * 2;
        const SECTION_GAP: u16 = 12;
        const VOLUME_H: u16 = 88;
        const BRIGHT_H: u16 = 56;
        const MEDIA_H: u16 = 56;
        const FOOTER_H: u16 = 40;
        let tiles = QS_TILE_H * 3 + QS_GAP * 2;
        // Four section gaps: tiles→vol, vol→bright, bright→media, media→footer.
        let budget =
            PAD + tiles + SECTION_GAP * 4 + VOLUME_H + BRIGHT_H + MEDIA_H + FOOTER_H;
        assert!(
            budget <= QS_CARD_H,
            "qs content budget {budget}px exceeds card {QS_CARD_H}px"
        );
        assert!(
            QS_CARD_H + 48 + 24 <= 864,
            "qs card {QS_CARD_H}px overflows a 864px logical desktop with taskbar"
        );
        assert!(QS_TILE_H >= 56, "qs tile height {QS_TILE_H}px is too short");
    }

    #[test]
    fn notification_centre_card_tall_enough_for_chrome_and_calendar() {
        assert!(
            SIDE_CARD_H >= 700,
            "NC side card {SIDE_CARD_H}px is too short for chrome + calendar"
        );
        // Fits 1080p @1.25 (864 logical) above a 48px taskbar with margin.
        assert!(
            SIDE_CARD_H + 48 + 24 <= 864,
            "side card {SIDE_CARD_H}px overflows a 864px logical desktop with taskbar"
        );
    }

    #[test]
    fn widgets_board_fits_left_dock_without_scroll() {
        assert!(WIDGETS_CARD_W >= 400);
        assert!(
            WIDGETS_CARD_H + 48 + 24 <= 1080,
            "widgets card {WIDGETS_CARD_H}px overflows a 1080p desktop with taskbar"
        );
    }

    #[test]
    fn start_pin_grid_fits_the_card() {
        let inner = START_CARD_W - START_PAD_X * 2;
        let row = START_PIN_W * 6 + START_PIN_GAP * 5;
        assert!(
            row <= inner,
            "start pin row {row}px exceeds card inner {inner}px"
        );
    }

    #[test]
    fn notif_toast_fills_zone_bottom_right() {
        // W75: toast uses the full overlay zone; stage places the card bottom-right.
        let frame = overlay_frame(FlyoutInput {
            toast: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::Toast);
        assert!(!frame.dismiss);
        assert_eq!(frame.width, 0);
        assert!(frame.anchor.contains(Anchor::Bottom));
        assert!(frame.anchor.contains(Anchor::Right));
    }

    #[test]
    fn quicklink_fills_zone_like_start() {
        // W390: compact Bottom|Left + GAP+56 floated the menu too high and forced scroll.
        let frame = overlay_frame(FlyoutInput {
            quicklink: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::QuickLink);
        assert!(frame.dismiss);
        assert_eq!(frame.width, 0);
        assert_eq!(frame.height, 0);
        assert!(frame.anchor.contains(Anchor::Top));
        assert!(frame.anchor.contains(Anchor::Bottom));
    }

    #[test]
    fn sysmenu_is_not_winmenu_frame() {
        let frame = overlay_frame(FlyoutInput {
            sysmenu: true,
            ..FlyoutInput::default()
        })
        .unwrap();
        assert_eq!(frame.place, FlyoutPlace::SysMenu);
        assert_eq!(frame.width, 0);
    }

    #[test]
    fn desktop_and_app_ctx_have_own_places() {
        assert_eq!(
            overlay_frame(FlyoutInput {
                desktop_menu: true,
                ..FlyoutInput::default()
            })
            .unwrap()
            .place,
            FlyoutPlace::DesktopMenu
        );
        assert_eq!(
            overlay_frame(FlyoutInput {
                app_ctx: true,
                ..FlyoutInput::default()
            })
            .unwrap()
            .place,
            FlyoutPlace::AppCtx
        );
    }
}


fn extract_hex_from_status(status: &str) -> Option<String> {
    for token in status.split_whitespace() {
        let t = token.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '#');
        let t = t.trim_start_matches('#');
        if t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("#{t}"));
        }
    }
    // Also search embedded #RRGGBB
    if let Some(i) = status.find('#') {
        let slice = &status[i..];
        let hex: String = slice.chars().take(7).collect();
        if hex.len() == 7 && hex[1..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(hex.to_ascii_uppercase());
        }
    }
    None
}

fn build_palette_hits(shell: &Shell) -> Vec<tools::PaletteHit> {
    let plugins = &shell.cfg.active().tools.palette_plugins;
    let apps: Vec<(String, String)> = shell
        .apps
        .iter()
        .take(400)
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    let files: Vec<(String, String)> = shell
        .recent
        .iter()
        .filter(|r| r.is_file)
        .map(|r| (r.id.clone(), r.label.clone()))
        .collect();
    tools::palette_search(&shell.run_query, plugins, &apps, &files, 12)
}

fn run_palette_action(_shell: &mut Shell, action: &str) -> String {
    if let Some(rest) = action.strip_prefix("app:") {
        return match apps::launch_desktop(rest) {
            Ok(()) => format!("Launch · {rest}"),
            Err(e) => e,
        };
    }
    if let Some(rest) = action.strip_prefix("file:") {
        let _ = std::process::Command::new("xdg-open").arg(rest).spawn();
        return format!("Open · {rest}");
    }
    if let Some(rest) = action.strip_prefix("calc:") {
        let _ = std::process::Command::new("wl-copy").arg(rest).spawn();
        return format!("Copied · {rest}");
    }
    if let Some(rest) = action.strip_prefix("shell:") {
        return launch_run_command(rest);
    }
    launch_run_command(action)
}

fn persist_tray_known(ids: &[String]) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(home).join(".local/state/mde/tray-known.txt");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = ids.join("\n");
    let _ = std::fs::write(path, body);
}

fn write_key_remaps(cfg: &mde_config::MdeConfig) {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(runtime).join("mde-key-remaps.txt");
    let mut body = String::new();
    for r in &cfg.active().tools.keyboard_remaps {
        body.push_str(&format!("{}={}
", r.from.trim(), r.to.trim()));
    }
    let _ = std::fs::write(path, body);
}
