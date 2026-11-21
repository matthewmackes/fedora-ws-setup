use std::path::PathBuf;

use crate::messages::{ShellCmd, SnapLayout, WinAction, WorkspaceOp};

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("mde.sock")
}

pub fn shell_cmd_path() -> PathBuf {
    runtime_dir().join("mde-shell.cmd")
}

pub fn focus_req_path() -> PathBuf {
    runtime_dir().join("mde-focus.req")
}

pub fn snap_req_path() -> PathBuf {
    runtime_dir().join("mde-snap.req")
}

pub fn window_action_req_path() -> PathBuf {
    runtime_dir().join("mde-window-action.req")
}

pub fn workspace_req_path() -> PathBuf {
    runtime_dir().join("mde-workspace.req")
}

pub fn show_desktop_req_path() -> PathBuf {
    runtime_dir().join("mde-show-desktop.req")
}

pub fn windows_json_path() -> PathBuf {
    runtime_dir().join("mde-windows.json")
}

pub fn outputs_json_path() -> PathBuf {
    runtime_dir().join("mde-outputs.json")
}

/// Legacy focus payload: list index or `id:<WindowId>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyFocus {
    Index(usize),
    Id(u64),
}

impl LegacyFocus {
    pub fn parse(text: &str) -> Option<Self> {
        let t = text.trim();
        if let Some(rest) = t.strip_prefix("id:") {
            return rest.trim().parse().ok().map(Self::Id);
        }
        t.parse().ok().map(Self::Index)
    }

    pub fn as_legacy(&self) -> String {
        match self {
            Self::Index(i) => i.to_string(),
            Self::Id(id) => format!("id:{id}"),
        }
    }
}

pub fn write_shell_cmd_legacy(cmd: &ShellCmd) {
    let _ = std::fs::write(shell_cmd_path(), cmd.as_legacy());
}

pub fn write_focus_legacy(focus: &LegacyFocus) {
    let _ = std::fs::write(focus_req_path(), focus.as_legacy());
}

pub fn write_snap_legacy(layout: &SnapLayout) {
    let _ = std::fs::write(snap_req_path(), layout.as_legacy());
}

pub fn write_window_action_legacy(action: &WinAction) {
    let _ = std::fs::write(window_action_req_path(), action.as_legacy());
}

pub fn write_workspace_legacy(op: &WorkspaceOp) {
    let _ = std::fs::write(workspace_req_path(), op.as_legacy());
}

pub fn write_show_desktop_legacy() {
    let _ = std::fs::write(show_desktop_req_path(), b"1\n");
}

/// When true (default), compositor and shell still write/read legacy files.
pub fn legacy_enabled() -> bool {
    match std::env::var("MDE_IPC_LEGACY") {
        Ok(v) => v != "0" && !v.eq_ignore_ascii_case("false"),
        Err(_) => true,
    }
}
