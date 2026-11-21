//! Mutually exclusive flyout flags + typed `ShellCmd` reduce.

use mde_protocol::ShellCmd;

/// Open/closed chrome surfaces that share exclusivity rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlyoutFlags {
    pub start_open: bool,
    pub quick_open: bool,
    pub notifications_open: bool,
    pub clipboard_open: bool,
    pub tray_open: bool,
    pub keyboard_open: bool,
    pub taskview_open: bool,
    pub widgets_open: bool,
    pub snapflyout_open: bool,
    pub winmenu_open: bool,
    pub run_open: bool,
    pub quicklink_open: bool,
    pub vol_flyout_open: bool,
    pub net_flyout_open: bool,
    pub bat_flyout_open: bool,
    /// W399 QS per-app volume mixer.
    pub app_mixer_open: bool,
    /// W397 QS VPN flyout.
    pub vpn_flyout_open: bool,
    /// W396 QS Mobile hotspot flyout.
    pub hotspot_flyout_open: bool,
    /// W395 QS Nearby Share (KDE Connect).
    pub nearby_flyout_open: bool,
    pub pin_overflow_open: bool,
    pub win_overflow_open: bool,
    pub taskbar_menu_open: bool,
    pub power_menu_open: bool,
    /// W449 Connect / Wireless display flyout.
    pub connect_open: bool,
    /// W426 emoji picker flyout.
    pub emoji_open: bool,
    /// W515 Win11 snip overlay.
    pub snip_open: bool,
}

impl FlyoutFlags {
    pub fn close_all(&mut self) {
        *self = Self::default();
    }

}

/// Side effects the iced shell must perform after a ShellCmd reduce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlyoutEffect {
    Status(String),
    EngageLock(&'static str),
    Unlock,
    RefreshLevels,
    RefreshWindows,
    RefreshWeather,
    ClipboardOpenPrep,
    ActivatePin(u8),
    CycleAltTab,
    CycleAltTabRev,
    CycleAltTabQuiet,
    WriteWindowAction(&'static str),
    ShowDesktop,
    LaunchControl(&'static str),
    OpenExplorer,
    VolUp,
    VolDown,
    Mute,
    BrightUp,
    BrightDown,
    /// Open Win11 snip overlay (optionally pre-select a mode).
    OpenSnip {
        preselect: Option<mde_protocol::ScreenshotMode>,
    },
    ColorPick,
    /// A024 Find My Mouse pulse.
    FindMyMouse,
    Emoji,
    InputLang,
    Diag,
    /// Clear run query when opening Run.
    ClearRunQuery,
    /// Focus Start search box after open.
    PendingStartFocus,
    /// Second Super+Z while snap flyout open → undo snap.
    SnapUndo,
    /// W424 toggle voice typing session.
    DictationToggle,
    /// Empty desktop context menu (W388).
    OpenDesktopMenu,
    /// SSD title-bar system menu (W389).
    OpenSysMenu,
    /// W422 / W403: start or stop magnifier backend.
    Magnifier(bool),
    /// W423: toggle color filter pipeline.
    ColorFilter,
    /// A081: launch MMC Performance Monitor.
    OpenPerfmon,
    /// A073: open Shortcut Guide overlay.
    ShortcutGuide,
}

/// Apply a compositor `ShellCmd` to flyout flags; return effects for the shell.
pub fn apply_shell_cmd(flags: &mut FlyoutFlags, cmd: ShellCmd) -> Vec<FlyoutEffect> {
    let mut effects = Vec::new();
    match cmd {
        ShellCmd::Lock => {
            effects.push(FlyoutEffect::EngageLock("Super+L"));
        }
        ShellCmd::Unlock => {
            flags.close_all();
            effects.push(FlyoutEffect::Unlock);
        }
        ShellCmd::ToggleStart => {
            let open = !flags.start_open;
            flags.close_all();
            flags.start_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Start open".into()
            } else {
                "Start closed".into()
            }));
        }
        ShellCmd::StartSearch => {
            flags.close_all();
            flags.start_open = true;
            effects.push(FlyoutEffect::PendingStartFocus);
            effects.push(FlyoutEffect::Status("Start search".into()));
        }
        ShellCmd::ToggleQuick => {
            let open = !flags.quick_open;
            flags.close_all();
            flags.quick_open = open;
            if open {
                effects.push(FlyoutEffect::RefreshLevels);
            }
            effects.push(FlyoutEffect::Status(if open {
                "Quick Settings".into()
            } else {
                "Quick Settings closed".into()
            }));
        }
        ShellCmd::ToggleNotifications => {
            let open = !flags.notifications_open;
            flags.close_all();
            flags.notifications_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Notification centre".into()
            } else {
                "Notifications closed".into()
            }));
        }
        ShellCmd::ToggleClipboard => {
            let open = !flags.clipboard_open;
            flags.close_all();
            flags.clipboard_open = open;
            if open {
                effects.push(FlyoutEffect::ClipboardOpenPrep);
            }
            effects.push(FlyoutEffect::Status("Clipboard history".into()));
        }
        ShellCmd::ToggleWidgets => {
            let open = !flags.widgets_open;
            flags.close_all();
            flags.widgets_open = open;
            if open {
                effects.push(FlyoutEffect::RefreshWeather);
            }
            effects.push(FlyoutEffect::Status(if open {
                "Widgets Board".into()
            } else {
                "Widgets closed".into()
            }));
        }
        ShellCmd::ToggleTaskView => {
            let open = !flags.taskview_open;
            flags.close_all();
            flags.taskview_open = open;
            if open {
                effects.push(FlyoutEffect::RefreshWindows);
                effects.push(FlyoutEffect::Status("Task View".into()));
            } else {
                effects.push(FlyoutEffect::Status("Task View closed".into()));
            }
        }
        ShellCmd::ToggleSnapFlyout => {
            if flags.snapflyout_open {
                flags.close_all();
                effects.push(FlyoutEffect::SnapUndo);
                effects.push(FlyoutEffect::Status("Snap undo".into()));
            } else {
                flags.close_all();
                flags.snapflyout_open = true;
                effects.push(FlyoutEffect::Status("Snap layouts".into()));
            }
        }
        ShellCmd::ToggleWinMenu => {
            let open = !flags.winmenu_open;
            flags.close_all();
            flags.winmenu_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Window actions".into()
            } else {
                "Window menu closed".into()
            }));
        }
        ShellCmd::ToggleRun => {
            let open = !flags.run_open;
            flags.close_all();
            flags.run_open = open;
            if open {
                effects.push(FlyoutEffect::ClearRunQuery);
            }
            effects.push(FlyoutEffect::Status(if open {
                "Command Palette".into()
            } else {
                "Palette closed".into()
            }));
        }
        ShellCmd::ToggleQuickLink => {
            let open = !flags.quicklink_open;
            flags.close_all();
            flags.quicklink_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Quick link".into()
            } else {
                "Quick link closed".into()
            }));
        }
        ShellCmd::ToggleDesktopMenu => {
            flags.close_all();
            effects.push(FlyoutEffect::OpenDesktopMenu);
        }
        ShellCmd::ToggleSysMenu => {
            flags.close_all();
            effects.push(FlyoutEffect::OpenSysMenu);
        }
        ShellCmd::ShowDesktop => {
            flags.close_all();
            effects.push(FlyoutEffect::ShowDesktop);
            effects.push(FlyoutEffect::Status("Show desktop".into()));
        }
        ShellCmd::OpenCalendar => {
            flags.close_all();
            flags.notifications_open = true;
            effects.push(FlyoutEffect::Status(
                "Calendar · Notification centre".into(),
            ));
        }
        ShellCmd::OpenEaseOfAccess => {
            flags.close_all();
            effects.push(FlyoutEffect::LaunchControl("accessibility"));
            effects.push(FlyoutEffect::Status(
                "Ease of Access · Accessibility".into(),
            ));
        }
        ShellCmd::OpenSettings { .. } => {
            flags.close_all();
            effects.push(FlyoutEffect::LaunchControl(""));
        }
        ShellCmd::OpenExplorer => {
            effects.push(FlyoutEffect::OpenExplorer);
        }
        ShellCmd::VolUp => effects.push(FlyoutEffect::VolUp),
        ShellCmd::VolDown => effects.push(FlyoutEffect::VolDown),
        ShellCmd::Mute => effects.push(FlyoutEffect::Mute),
        ShellCmd::BrightUp => effects.push(FlyoutEffect::BrightUp),
        ShellCmd::BrightDown => effects.push(FlyoutEffect::BrightDown),
        ShellCmd::ActivatePin { index } => {
            effects.push(FlyoutEffect::ActivatePin(index));
        }
        ShellCmd::AltTab => {
            flags.taskview_open = false;
            flags.start_open = false;
            flags.quick_open = false;
            flags.notifications_open = false;
            flags.clipboard_open = false;
            flags.tray_open = false;
            flags.keyboard_open = false;
            effects.push(FlyoutEffect::CycleAltTab);
        }
        ShellCmd::AltTabRev => {
            flags.taskview_open = false;
            flags.start_open = false;
            flags.quick_open = false;
            flags.notifications_open = false;
            flags.clipboard_open = false;
            flags.tray_open = false;
            flags.keyboard_open = false;
            effects.push(FlyoutEffect::CycleAltTabRev);
        }
        ShellCmd::AltTabQuiet => {
            flags.taskview_open = false;
            effects.push(FlyoutEffect::CycleAltTabQuiet);
        }
        ShellCmd::MagnifierOn => {
            effects.push(FlyoutEffect::Magnifier(true));
        }
        ShellCmd::MagnifierOff => {
            effects.push(FlyoutEffect::Magnifier(false));
        }
        ShellCmd::ColorPick => effects.push(FlyoutEffect::ColorPick),
        ShellCmd::ColorFilter => {
            effects.push(FlyoutEffect::ColorFilter);
        }
        ShellCmd::Screenshot { mode, geom: _ } => {
            // W515: all screenshot cmds open the snip overlay (Win11 GUI).
            flags.close_all();
            flags.snip_open = true;
            let preselect = match mode {
                mde_protocol::ScreenshotMode::Window => {
                    Some(mde_protocol::ScreenshotMode::Window)
                }
                mde_protocol::ScreenshotMode::Screen => {
                    Some(mde_protocol::ScreenshotMode::Screen)
                }
                mde_protocol::ScreenshotMode::Area | mde_protocol::ScreenshotMode::Snip => {
                    Some(mde_protocol::ScreenshotMode::Area)
                }
            };
            effects.push(FlyoutEffect::OpenSnip { preselect });
        }
        ShellCmd::Emoji => {
            let open = !flags.emoji_open;
            flags.close_all();
            flags.emoji_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Emoji".into()
            } else {
                "Emoji closed".into()
            }));
        }
        ShellCmd::Dictation => {
            effects.push(FlyoutEffect::DictationToggle);
        }
        ShellCmd::Connect | ShellCmd::Project => {
            let open = !flags.connect_open;
            flags.close_all();
            flags.connect_open = open;
            effects.push(FlyoutEffect::Status(if open {
                "Wireless display".into()
            } else {
                "Wireless display closed".into()
            }));
        }
        ShellCmd::InputLang => effects.push(FlyoutEffect::InputLang),
        ShellCmd::Diag => effects.push(FlyoutEffect::Diag),
        ShellCmd::GpuReset => {
            effects.push(FlyoutEffect::Status(
                "GPU reset · not supported · restart session".into(),
            ));
        }
        ShellCmd::MinimizeOthersTip => {
            effects.push(FlyoutEffect::Status("Minimize others · Super+Home".into()));
        }
        ShellCmd::Unminimize => {
            effects.push(FlyoutEffect::WriteWindowAction("unminimize"));
            effects.push(FlyoutEffect::Status("Restored minimized".into()));
        }
        ShellCmd::OpenPerfmon => {
            effects.push(FlyoutEffect::OpenPerfmon);
        }
        ShellCmd::ShortcutGuide => {
            effects.push(FlyoutEffect::ShortcutGuide);
        }
        ShellCmd::Raw(s) => {
            match s.as_str() {
                // A024: Find My Mouse spotlight.
                "findmouse" => {
                    effects.push(FlyoutEffect::FindMyMouse);
                }
                // Preserve unknown verbs as status for debugging.
                other if !other.is_empty() => {
                    effects.push(FlyoutEffect::Status(format!("Shell cmd · {other}")));
                }
                _ => {}
            }
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_start_is_exclusive() {
        let mut flags = FlyoutFlags {
            quick_open: true,
            ..FlyoutFlags::default()
        };
        let effects = apply_shell_cmd(&mut flags, ShellCmd::ToggleStart);
        assert!(flags.start_open);
        assert!(!flags.quick_open);
        assert!(effects.iter().any(|e| matches!(e, FlyoutEffect::Status(_))));
    }

    #[test]
    fn second_snapflyout_undoes() {
        let mut flags = FlyoutFlags {
            snapflyout_open: true,
            ..FlyoutFlags::default()
        };
        let effects = apply_shell_cmd(&mut flags, ShellCmd::ToggleSnapFlyout);
        assert!(!flags.snapflyout_open);
        assert!(effects.contains(&FlyoutEffect::SnapUndo));
    }

    #[test]
    fn pin_activates() {
        let mut flags = FlyoutFlags::default();
        let effects = apply_shell_cmd(&mut flags, ShellCmd::ActivatePin { index: 3 });
        assert_eq!(effects, vec![FlyoutEffect::ActivatePin(3)]);
    }

    #[test]
    fn connect_and_project_open_flyout() {
        let mut flags = FlyoutFlags {
            quick_open: true,
            ..FlyoutFlags::default()
        };
        let _ = apply_shell_cmd(&mut flags, ShellCmd::Connect);
        assert!(flags.connect_open);
        assert!(!flags.quick_open);
        let _ = apply_shell_cmd(&mut flags, ShellCmd::Project);
        assert!(!flags.connect_open);
        let _ = apply_shell_cmd(&mut flags, ShellCmd::Project);
        assert!(flags.connect_open);
    }

    #[test]
    fn all_legacy_verbs_parse_and_apply() {
        for raw in [
            "lock",
            "unlock",
            "start",
            "quick",
            "notifications",
            "clipboard",
            "widgets",
            "taskview",
            "snapflyout",
            "winmenu",
            "run",
            "quicklink",
            "showdesktop",
            "date",
            "ease",
            "settings",
            "explorer",
            "volup",
            "mute",
            "alttab",
            "pin 2",
            "minothers-tip",
        ] {
            let mut flags = FlyoutFlags::default();
            let _ = apply_shell_cmd(&mut flags, ShellCmd::parse(raw));
        }
    }
}
