//! Typed IPC between `mde-comp` / `mde-anvil` and `mde-shell`.
//!
//! Transport: newline-delimited JSON on `$XDG_RUNTIME_DIR/mde.sock`.
//! Legacy `$XDG_RUNTIME_DIR/mde-*.req` / `mde-shell.cmd` files remain as a
//! dual-write / fallback shim until clients fully migrate.

mod bus;
mod legacy;
mod messages;

pub use bus::{CompBus, ShellClient};
pub use legacy::{
    focus_req_path, legacy_enabled, shell_cmd_path, show_desktop_req_path, snap_req_path,
    outputs_json_path, socket_path, window_action_req_path, windows_json_path, workspace_req_path,
    write_focus_legacy,
    write_shell_cmd_legacy, write_show_desktop_legacy, write_snap_legacy,
    write_window_action_legacy, write_workspace_legacy, LegacyFocus,
};
pub use messages::*;

/// Protocol schema version carried on every envelope.
pub const VERSION: u32 = 1;
