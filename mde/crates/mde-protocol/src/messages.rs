use serde::{Deserialize, Serialize};

/// Stable window handle (not a list index).
pub type WindowId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapLayout {
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    LeftTwoThirds,
    RightTwoThirds,
    LeftThird,
    CenterThird,
    RightThird,
    Maximize,
    Restore,
    TopHalf,
    BottomHalf,
    Center,
}

impl SnapLayout {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim() {
            "left" => Self::Left,
            "right" => Self::Right,
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-right" => Self::BottomRight,
            "left-2/3" | "left-two-thirds" => Self::LeftTwoThirds,
            "right-2/3" | "right-two-thirds" => Self::RightTwoThirds,
            "left-1/3" | "left-third" => Self::LeftThird,
            "center-1/3" | "center-third" => Self::CenterThird,
            "right-1/3" | "right-third" => Self::RightThird,
            "maximize" | "up" => Self::Maximize,
            "restore" | "down" => Self::Restore,
            "top" | "top-half" => Self::TopHalf,
            "bottom" | "bottom-half" => Self::BottomHalf,
            "center" => Self::Center,
            _ => return None,
        })
    }

    pub fn as_legacy(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
            Self::LeftTwoThirds => "left-2/3",
            Self::RightTwoThirds => "right-2/3",
            Self::LeftThird => "left-1/3",
            Self::CenterThird => "center-1/3",
            Self::RightThird => "right-1/3",
            Self::Maximize => "maximize",
            Self::Restore => "restore",
            Self::TopHalf => "top-half",
            Self::BottomHalf => "bottom-half",
            Self::Center => "center",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlwaysOnTopMode {
    Toggle,
    On,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WinAction {
    Minimize,
    Maximize,
    Close,
    MinimizeOthers,
    Restore,
    Unminimize,
    Nudge { dx: i32, dy: i32 },
    /// W412: keep window above others.
    AlwaysOnTop(AlwaysOnTopMode),
    /// W413: cycle focused window opacity.
    Opacity,
    /// W414: cascade mapped windows on the active workspace.
    Cascade,
    /// W415: start pointer move grab on focused window.
    Move,
    /// W416: start pointer resize grab (bottom-right) on focused window.
    Size,
}

impl WinAction {
    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        Some(match t {
            "minimize" => Self::Minimize,
            "maximize" => Self::Maximize,
            "close" => Self::Close,
            "minimize-others" => Self::MinimizeOthers,
            "restore" | "undo-snap" => Self::Restore,
            "unminimize" => Self::Unminimize,
            "nudge left" | "nudge-left" => Self::Nudge { dx: -20, dy: 0 },
            "nudge right" | "nudge-right" => Self::Nudge { dx: 20, dy: 0 },
            "nudge up" | "nudge-up" => Self::Nudge { dx: 0, dy: -20 },
            "nudge down" | "nudge-down" => Self::Nudge { dx: 0, dy: 20 },
            "always-on-top" | "always-on-top toggle" => {
                Self::AlwaysOnTop(AlwaysOnTopMode::Toggle)
            }
            "always-on-top on" => Self::AlwaysOnTop(AlwaysOnTopMode::On),
            "always-on-top off" => Self::AlwaysOnTop(AlwaysOnTopMode::Off),
            "opacity" => Self::Opacity,
            "cascade" => Self::Cascade,
            "move" => Self::Move,
            "size" => Self::Size,
            _ => return None,
        })
    }

    pub fn as_legacy(&self) -> String {
        match self {
            Self::Minimize => "minimize".into(),
            Self::Maximize => "maximize".into(),
            Self::Close => "close".into(),
            Self::MinimizeOthers => "minimize-others".into(),
            Self::Restore => "restore".into(),
            Self::Unminimize => "unminimize".into(),
            Self::Nudge { dx, dy } => {
                if *dx < 0 {
                    "nudge-left".into()
                } else if *dx > 0 {
                    "nudge-right".into()
                } else if *dy < 0 {
                    "nudge-up".into()
                } else {
                    "nudge-down".into()
                }
            }
            Self::AlwaysOnTop(AlwaysOnTopMode::Toggle) => "always-on-top".into(),
            Self::AlwaysOnTop(AlwaysOnTopMode::On) => "always-on-top on".into(),
            Self::AlwaysOnTop(AlwaysOnTopMode::Off) => "always-on-top off".into(),
            Self::Opacity => "opacity".into(),
            Self::Cascade => "cascade".into(),
            Self::Move => "move".into(),
            Self::Size => "size".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceOp {
    Switch(usize),
    New,
    Close,
    MoveFocused(usize),
}

impl WorkspaceOp {
    pub fn parse(s: &str) -> Option<Self> {
        let cmd = s.trim();
        if let Some(rest) = cmd.strip_prefix("switch ") {
            return rest.trim().parse().ok().map(Self::Switch);
        }
        if cmd == "new" {
            return Some(Self::New);
        }
        if cmd == "close" {
            return Some(Self::Close);
        }
        if let Some(rest) = cmd.strip_prefix("move ") {
            return rest.trim().parse().ok().map(Self::MoveFocused);
        }
        None
    }

    pub fn as_legacy(&self) -> String {
        match self {
            Self::Switch(n) => format!("switch {n}"),
            Self::New => "new".into(),
            Self::Close => "close".into(),
            Self::MoveFocused(n) => format!("move {n}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// Screenshot capture mode (W501).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotMode {
    /// Full outputs via grim.
    Screen,
    /// Interactive region (slurp); opens swappy when available.
    Area,
    /// Focused window (geometry from compositor).
    Window,
    /// Win+Shift+S snip — region + swappy (default bare `screenshot`).
    #[default]
    Snip,
}

/// One-shot compositor → shell chrome commands (was `mde-shell.cmd`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShellCmd {
    Lock,
    /// Clear soft-lock + ask `mde-lock` to release ext-session-lock.
    Unlock,
    ToggleStart,
    StartSearch,
    ToggleQuick,
    ToggleNotifications,
    ToggleClipboard,
    ToggleWidgets,
    ToggleTaskView,
    ToggleSnapFlyout,
    ToggleWinMenu,
    ToggleRun,
    ToggleQuickLink,
    /// Empty desktop context menu (W388).
    ToggleDesktopMenu,
    /// SSD title-bar system menu (W389).
    ToggleSysMenu,
    ShowDesktop,
    OpenCalendar,
    OpenEaseOfAccess,
    OpenSettings { page: Option<String> },
    OpenExplorer,
    VolUp,
    VolDown,
    Mute,
    BrightUp,
    BrightDown,
    ActivatePin { index: u8 },
    AltTab,
    AltTabRev,
    AltTabQuiet,
    MagnifierOn,
    MagnifierOff,
    ColorPick,
    ColorFilter,
    /// Screenshot capture (W500–W514). `geom` is grim `x,y WxH` for window mode.
    Screenshot {
        mode: ScreenshotMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom: Option<String>,
    },
    Emoji,
    Dictation,
    Connect,
    Project,
    InputLang,
    Diag,
    GpuReset,
    MinimizeOthersTip,
    Unminimize,
    /// A081: open MMC Performance Monitor (Ctrl+Shift+Esc).
    OpenPerfmon,
    /// A073: Shortcut Guide overlay (Super+/).
    ShortcutGuide,
    /// Unknown legacy string — preserve so nothing is dropped during migration.
    Raw(String),
}

impl ShellCmd {
    pub fn parse(s: &str) -> Self {
        let t = s.trim();
        match t {
            "lock" => Self::Lock,
            "unlock" => Self::Unlock,
            "start" => Self::ToggleStart,
            "startsearch" => Self::StartSearch,
            "quick" => Self::ToggleQuick,
            "notifications" => Self::ToggleNotifications,
            "clipboard" => Self::ToggleClipboard,
            "widgets" => Self::ToggleWidgets,
            "taskview" => Self::ToggleTaskView,
            "snapflyout" => Self::ToggleSnapFlyout,
            "winmenu" => Self::ToggleWinMenu,
            "run" => Self::ToggleRun,
            "quicklink" => Self::ToggleQuickLink,
            "desktopmenu" => Self::ToggleDesktopMenu,
            "sysmenu" => Self::ToggleSysMenu,
            "showdesktop" => Self::ShowDesktop,
            "date" => Self::OpenCalendar,
            "ease" => Self::OpenEaseOfAccess,
            "settings" => Self::OpenSettings { page: None },
            "explorer" => Self::OpenExplorer,
            "volup" => Self::VolUp,
            "voldown" => Self::VolDown,
            "mute" => Self::Mute,
            "brightup" => Self::BrightUp,
            "brightdown" => Self::BrightDown,
            "alttab" => Self::AltTab,
            "alttab-rev" => Self::AltTabRev,
            "alttab-quiet" => Self::AltTabQuiet,
            "magnifier" => Self::MagnifierOn,
            "magnifier-off" => Self::MagnifierOff,
            "shortcutguide" | "shortcut-guide" => Self::ShortcutGuide,
            "colorpick" => Self::ColorPick,
            "colorfilter" => Self::ColorFilter,
            "screenshot" | "screenshot:snip" => Self::Screenshot {
                mode: ScreenshotMode::Snip,
                geom: None,
            },
            "screenshot:screen" => Self::Screenshot {
                mode: ScreenshotMode::Screen,
                geom: None,
            },
            "screenshot:area" => Self::Screenshot {
                mode: ScreenshotMode::Area,
                geom: None,
            },
            "screenshot:window" => Self::Screenshot {
                mode: ScreenshotMode::Window,
                geom: None,
            },
            other if other.starts_with("screenshot:window:") => {
                let g = other["screenshot:window:".len()..].trim();
                Self::Screenshot {
                    mode: ScreenshotMode::Window,
                    geom: if g.is_empty() {
                        None
                    } else {
                        Some(g.to_string())
                    },
                }
            },
            "emoji" => Self::Emoji,
            "dictation" => Self::Dictation,
            "connect" => Self::Connect,
            "project" => Self::Project,
            "inputlang" => Self::InputLang,
            "diag" => Self::Diag,
            "gpu-reset" => Self::GpuReset,
            "minothers-tip" => Self::MinimizeOthersTip,
            "minothers" => Self::MinimizeOthersTip,
            "unminimize" => Self::Unminimize,
            "perfmon" | "taskmgr" => Self::OpenPerfmon,
            other => {
                if let Some(rest) = other
                    .strip_prefix("pin:")
                    .or_else(|| other.strip_prefix("pin "))
                {
                    if let Ok(idx) = rest.trim().parse::<u8>() {
                        return Self::ActivatePin { index: idx };
                    }
                }
                Self::Raw(other.to_string())
            }
        }
    }

    pub fn as_legacy(&self) -> String {
        match self {
            Self::Lock => "lock".into(),
            Self::Unlock => "unlock".into(),
            Self::ToggleStart => "start".into(),
            Self::StartSearch => "startsearch".into(),
            Self::ToggleQuick => "quick".into(),
            Self::ToggleNotifications => "notifications".into(),
            Self::ToggleClipboard => "clipboard".into(),
            Self::ToggleWidgets => "widgets".into(),
            Self::ToggleTaskView => "taskview".into(),
            Self::ToggleSnapFlyout => "snapflyout".into(),
            Self::ToggleWinMenu => "winmenu".into(),
            Self::ToggleRun => "run".into(),
            Self::ToggleQuickLink => "quicklink".into(),
            Self::ToggleDesktopMenu => "desktopmenu".into(),
            Self::ToggleSysMenu => "sysmenu".into(),
            Self::ShowDesktop => "showdesktop".into(),
            Self::OpenCalendar => "date".into(),
            Self::OpenEaseOfAccess => "ease".into(),
            Self::OpenSettings { .. } => "settings".into(),
            Self::OpenExplorer => "explorer".into(),
            Self::VolUp => "volup".into(),
            Self::VolDown => "voldown".into(),
            Self::Mute => "mute".into(),
            Self::BrightUp => "brightup".into(),
            Self::BrightDown => "brightdown".into(),
            Self::ActivatePin { index } => format!("pin {index}"),
            Self::AltTab => "alttab".into(),
            Self::AltTabRev => "alttab-rev".into(),
            Self::AltTabQuiet => "alttab-quiet".into(),
            Self::MagnifierOn => "magnifier".into(),
            Self::MagnifierOff => "magnifier-off".into(),
            Self::ColorPick => "colorpick".into(),
            Self::ColorFilter => "colorfilter".into(),
            Self::Screenshot { mode, geom } => match mode {
                ScreenshotMode::Screen => "screenshot:screen".into(),
                ScreenshotMode::Area => "screenshot:area".into(),
                ScreenshotMode::Snip => "screenshot".into(),
                ScreenshotMode::Window => match geom {
                    Some(g) if !g.is_empty() => format!("screenshot:window:{g}"),
                    _ => "screenshot:window".into(),
                },
            },
            Self::Emoji => "emoji".into(),
            Self::Dictation => "dictation".into(),
            Self::Connect => "connect".into(),
            Self::Project => "project".into(),
            Self::InputLang => "inputlang".into(),
            Self::Diag => "diag".into(),
            Self::GpuReset => "gpu-reset".into(),
            Self::MinimizeOthersTip => "minothers-tip".into(),
            Self::Unminimize => "unminimize".into(),
            Self::OpenPerfmon => "perfmon".into(),
            Self::ShortcutGuide => "shortcutguide".into(),
            Self::Raw(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub fullscreen: bool,
    pub workspace: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowsSnapshot {
    pub active: usize,
    pub count: usize,
    pub windows: Vec<WindowInfo>,
}

/// Live output head for Connect / Display (W449).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputInfo {
    pub name: String,
    pub make: String,
    pub model: String,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputsSnapshot {
    pub outputs: Vec<OutputInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Locked,
    Unlocked,
}

/// Shell → compositor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", content = "p", rename_all = "snake_case")]
pub enum ClientToComp {
    FocusWindow { id: WindowId },
    /// Legacy list-index focus (migration only).
    FocusIndex { index: usize },
    Snap { layout: SnapLayout },
    WindowAction { action: WinAction },
    Workspace { op: WorkspaceOp },
    ShowDesktop,
    /// W515: ask for focused toplevel geometry as grim `x,y WxH`.
    FocusedWindowGeom,
}

/// Compositor → shell.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", content = "p", rename_all = "snake_case")]
pub enum CompToShell {
    ShellCmd(ShellCmd),
    WindowsSnapshot(WindowsSnapshot),
    OutputsSnapshot(OutputsSnapshot),
    WorkspaceState { active: usize, count: usize },
    SnapAssist { side: Side },
    LockState(LockState),
    /// Reply to [`ClientToComp::FocusedWindowGeom`] (W515).
    FocusedWindowGeom(Option<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Payload {
    C2S(ClientToComp),
    S2C(CompToShell),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    pub v: u32,
    pub id: u64,
    pub msg: Payload,
}

impl Envelope {
    pub fn c2s(id: u64, msg: ClientToComp) -> Self {
        Self {
            v: crate::VERSION,
            id,
            msg: Payload::C2S(msg),
        }
    }

    pub fn s2c(id: u64, msg: CompToShell) -> Self {
        Self {
            v: crate::VERSION,
            id,
            msg: Payload::S2C(msg),
        }
    }

    pub fn encode_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }

    pub fn decode_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_cmd_roundtrip_legacy() {
        for raw in [
            "lock",
            "unlock",
            "start",
            "pin 3",
            "alttab-rev",
            "showdesktop",
            "minothers-tip",
            "screenshot",
            "screenshot:screen",
            "screenshot:area",
            "screenshot:window",
            "screenshot:window:10,20 300x200",
        ] {
            let cmd = ShellCmd::parse(raw);
            assert_eq!(ShellCmd::parse(&cmd.as_legacy()), cmd);
        }
    }

    #[test]
    fn envelope_json_roundtrip() {
        let env = Envelope::s2c(
            1,
            CompToShell::ShellCmd(ShellCmd::ActivatePin { index: 2 }),
        );
        let line = env.encode_line().unwrap();
        let back = Envelope::decode_line(&line).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn snap_and_workspace_parse() {
        assert_eq!(SnapLayout::parse("left-2/3"), Some(SnapLayout::LeftTwoThirds));
        assert_eq!(
            WorkspaceOp::parse("switch 2"),
            Some(WorkspaceOp::Switch(2))
        );
    }

    #[test]
    fn outputs_snapshot_serde() {
        let snap = OutputsSnapshot {
            outputs: vec![OutputInfo {
                name: "eDP-1".into(),
                make: "AUO".into(),
                model: "Panel".into(),
                width: 1920,
                height: 1080,
                scale: 1.25,
                x: 0,
                y: 0,
            }],
        };
        let env = Envelope::s2c(2, CompToShell::OutputsSnapshot(snap.clone()));
        let line = env.encode_line().unwrap();
        let back = Envelope::decode_line(&line).unwrap();
        assert_eq!(back, env);
        let Payload::S2C(CompToShell::OutputsSnapshot(s)) = back.msg else {
            panic!("expected OutputsSnapshot");
        };
        assert_eq!(s.outputs[0].name, "eDP-1");
        assert_eq!(s.outputs[0].scale, 1.25);
    }
}
