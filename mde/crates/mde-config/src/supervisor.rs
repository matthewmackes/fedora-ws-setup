//! Crash and lock decisions shared by `mde-session` and `mde-shell`.
//!
//! These functions do not spawn processes. Callers log the returned action and
//! apply it. Thresholds match the fidelity program: compositor restarts at most
//! 3 times, the lock client is respawned at most 10 times, and a lock crash
//! never unlocks the seat.

/// How many times `mde-comp` may be started again after it exits.
pub const COMP_RESTART_MAX: u32 = 3;
/// Gap between compositor restarts.
pub const COMP_RESTART_GAP_SECS: u64 = 2;
/// How many times a crashed `mde-lock` may be spawned again.
pub const LOCK_RESPAWN_MAX: u32 = 10;
/// Gap between lock respawns.
pub const LOCK_RESPAWN_GAP_SECS: u64 = 1;
/// Shell respawn burst before the session cools down. Matches the existing loop.
pub const SHELL_RESPAWN_MAX: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompAction {
    /// Start the compositor again. `attempt` is 1-based.
    Restart { attempt: u32 },
    /// Give up and return to the greeter.
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockAction {
    /// Clean exit (status 0): the user unlocked.
    Unlock,
    /// Non-zero exit and the cap is not exhausted. `attempt` is 1-based.
    Respawn { attempt: u32 },
    /// Cap exhausted, or the seat must stay locked with no new client.
    StayLocked,
}

/// `prior_restarts` is how many restarts have already been performed.
pub fn comp_after_exit(prior_restarts: u32) -> CompAction {
    if prior_restarts >= COMP_RESTART_MAX {
        CompAction::Stop
    } else {
        CompAction::Restart {
            attempt: prior_restarts + 1,
        }
    }
}

/// `clean` is true only when the lock client exits 0.
/// `prior_crashes` counts previous non-zero exits in this lock session.
pub fn lock_after_exit(clean: bool, prior_crashes: u32) -> LockAction {
    if clean {
        return LockAction::Unlock;
    }
    if prior_crashes >= LOCK_RESPAWN_MAX {
        LockAction::StayLocked
    } else {
        LockAction::Respawn {
            attempt: prior_crashes + 1,
        }
    }
}

pub fn shell_respawn_line(restart_index: u32) -> String {
    format!("mde-shell respawned ({restart_index}/{SHELL_RESPAWN_MAX})")
}

pub fn comp_restart_line(attempt: u32, status: &str) -> String {
    format!("mde-comp exited: {status}; restart {attempt}/{COMP_RESTART_MAX}")
}

pub fn lock_respawn_line(attempt: u32) -> String {
    format!("mde-lock crashed; respawn {attempt}/{LOCK_RESPAWN_MAX}; seat stays locked")
}

/// Append a line to `~/.local/state/mde/session.log`.
pub fn session_log(msg: &str) {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let dir = std::path::PathBuf::from(home).join(".local/state/mde");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("session.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{msg}");
    }
}

/// Log the panic location, then abort. Do not unwind across a Wayland loop.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let line = format!("panic: {loc}: {info}");
        session_log(&line);
        eprintln!("{line}");
        std::process::abort();
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comp_restarts_three_times_then_stops() {
        assert_eq!(comp_after_exit(0), CompAction::Restart { attempt: 1 });
        assert_eq!(comp_after_exit(2), CompAction::Restart { attempt: 3 });
        assert_eq!(comp_after_exit(3), CompAction::Stop);
    }

    #[test]
    fn lock_crash_never_unlocks() {
        assert_eq!(lock_after_exit(false, 0), LockAction::Respawn { attempt: 1 });
        assert_eq!(
            lock_after_exit(false, 9),
            LockAction::Respawn { attempt: 10 }
        );
        assert_eq!(lock_after_exit(false, 10), LockAction::StayLocked);
        assert_eq!(lock_after_exit(true, 4), LockAction::Unlock);
    }

    #[test]
    fn shell_respawn_line_names_the_restart() {
        let line = shell_respawn_line(1);
        assert!(line.contains("respawned"));
        assert!(line.contains("1/8"));
    }
}
