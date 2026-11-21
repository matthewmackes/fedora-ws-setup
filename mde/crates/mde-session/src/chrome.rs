//! Shared chrome recovery helpers for the session supervisor and `mde-session-ctl`.
use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn runtime_dir() -> PathBuf {
    PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()))
}

pub fn wayland_marker() -> PathBuf {
    runtime_dir().join("mde-comp-wayland")
}

pub fn session_cmd_path() -> PathBuf {
    runtime_dir().join("mde-session.cmd")
}

pub fn wallpaper_pid_path() -> PathBuf {
    runtime_dir().join("mde-wallpaper.pid")
}

pub fn lock_flag_path() -> PathBuf {
    runtime_dir().join("mde-locked")
}

pub fn unlock_req_path() -> PathBuf {
    runtime_dir().join("mde-unlock.req")
}

pub fn read_wayland_display() -> Result<String> {
    let marker = wayland_marker();
    let name = fs::read_to_string(&marker)
        .with_context(|| format!("read {}", marker.display()))?
        .trim()
        .to_string();
    if name.is_empty() {
        bail!("WAYLAND_DISPLAY marker is empty ({})", marker.display());
    }
    let sock = runtime_dir().join(&name);
    if !sock.exists() {
        bail!(
            "Wayland socket missing: {} (compositor may be down)",
            sock.display()
        );
    }
    Ok(name)
}

pub fn session_log(msg: &str) {
    let Ok(home) = env::var("HOME") else {
        return;
    };
    let dir = PathBuf::from(home).join(".local/state/mde");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("session.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{msg}");
    }
}

/// Ask the lock client to unlock, clear soft-lock flag, stop `mde-lock`.
pub fn unlock_seat() -> Result<()> {
    let _ = fs::write(unlock_req_path(), b"1\n");
    let _ = fs::remove_file(lock_flag_path());
    // Give mde-lock time to observe unlock req before SIGTERM (PAM UI can lag).
    thread::sleep(Duration::from_millis(400));
    let _ = Command::new("pkill").args(["-TERM", "-x", "mde-lock"]).status();
    thread::sleep(Duration::from_millis(350));
    let _ = Command::new("pkill").args(["-KILL", "-x", "mde-lock"]).status();
    // Reap zombies / stubborn children once more.
    thread::sleep(Duration::from_millis(100));
    let _ = Command::new("pkill").args(["-KILL", "-x", "mde-lock"]).status();
    let _ = fs::remove_file(lock_flag_path());
    let _ = fs::remove_file(unlock_req_path());
    session_log("chrome: unlock_seat");
    Ok(())
}

fn kill_pid_file(path: &Path) {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
            thread::sleep(Duration::from_millis(100));
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
    let _ = fs::remove_file(path);
}

/// Restart swaybg for the configured wallpaper on the live Wayland display.
pub fn restart_wallpaper(path: &str) -> Result<()> {
    let path = path.trim();
    if path.is_empty() {
        bail!("theme.wallpaper is empty");
    }
    if !PathBuf::from(path).is_file() {
        bail!("wallpaper is not a file: {path}");
    }
    let display = read_wayland_display()?;
    kill_pid_file(&wallpaper_pid_path());
    // Also reap any stray/zombie-replaced swaybg instances on this seat.
    let _ = Command::new("pkill").args(["-TERM", "-x", "swaybg"]).status();
    thread::sleep(Duration::from_millis(150));

    // Spawn via setsid so the session supervisor never owns a swaybg Child
    // (avoids defunct zombies when the parent forgets to wait).
    let status = Command::new("setsid")
        .args(["-f", "swaybg", "-i", path, "-m", "fill"])
        .env("WAYLAND_DISPLAY", &display)
        .env("XDG_RUNTIME_DIR", runtime_dir())
        .status()
        .context("spawn setsid swaybg")?;
    if !status.success() {
        bail!("setsid swaybg failed: {status}");
    }
    // Resolve the live pid for the watchdog pid file.
    thread::sleep(Duration::from_millis(100));
    if let Ok(out) = Command::new("pgrep").args(["-n", "-x", "swaybg"]).output() {
        if let Ok(pid) = String::from_utf8_lossy(&out.stdout).trim().parse::<i32>() {
            if pid_is_live(pid) {
                let _ = fs::write(wallpaper_pid_path(), pid.to_string());
            }
        }
    }
    session_log(&format!("chrome: wallpaper restarted ({path}) on {display}"));
    Ok(())
}

fn pid_is_live(pid: i32) -> bool {
    let path = format!("/proc/{pid}/stat");
    let Ok(stat) = fs::read_to_string(path) else {
        return false;
    };
    // /proc/pid/stat: pid (comm) state ...
    let Some(close) = stat.rfind(')') else {
        return false;
    };
    let rest = stat[close + 1..].trim_start();
    let state = rest.chars().next().unwrap_or('Z');
    // Z = zombie, not a usable wallpaper process.
    state != 'Z' && state != 'X'
}

pub fn wallpaper_alive() -> bool {
    if let Ok(text) = fs::read_to_string(wallpaper_pid_path()) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            if pid_is_live(pid) {
                return true;
            }
        }
    }
    // Any live non-zombie swaybg counts.
    let Ok(out) = Command::new("pgrep").args(["-x", "swaybg"]).output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|s| s.parse::<i32>().ok())
        .any(pid_is_live)
}

pub fn restart_shell() -> Result<()> {
    let _ = Command::new("pkill").args(["-TERM", "-x", "mde-shell"]).status();
    session_log("chrome: shell restart requested");
    Ok(())
}

pub fn restart_comp() -> Result<()> {
    let _ = Command::new("pkill").args(["-TERM", "-x", "mde-comp"]).status();
    session_log("chrome: compositor restart requested");
    Ok(())
}

pub fn configured_wallpaper() -> String {
    mde_config::MdeConfig::load()
        .ok()
        .map(|c| c.active().theme.wallpaper.clone())
        .unwrap_or_default()
}

/// Unlock + wallpaper + shell. Does not bounce the compositor.
pub fn restart_chrome() -> Result<()> {
    unlock_seat()?;
    let wall = configured_wallpaper();
    if !wall.trim().is_empty() {
        if let Err(e) = restart_wallpaper(&wall) {
            session_log(&format!("chrome: wallpaper failed: {e}"));
        }
    }
    restart_shell()?;
    session_log("chrome: chrome restarted");
    Ok(())
}

/// Full seat recovery for black screens after a broken lock / dead wallpaper.
/// Unlocks, then restarts the compositor (session respawns wallpaper + shell).
pub fn recover_seat() -> Result<()> {
    unlock_seat()?;
    // Brief pause so unlock hits the old compositor before it dies.
    thread::sleep(Duration::from_millis(150));
    restart_comp()?;
    session_log("chrome: recover_seat (comp bounce)");
    Ok(())
}

/// Request the running session supervisor to handle a command (preferred when available).
pub fn write_session_cmd(cmd: &str) -> Result<()> {
    let path = session_cmd_path();
    fs::write(&path, format!("{}\n", cmd.trim()))
        .with_context(|| format!("write {}", path.display()))?;
    session_log(&format!("chrome: wrote session cmd `{cmd}`"));
    Ok(())
}

pub fn take_session_cmd() -> Option<String> {
    let path = session_cmd_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return None;
    };
    let _ = fs::remove_file(&path);
    let cmd = text.trim().to_string();
    if cmd.is_empty() {
        None
    } else {
        Some(cmd)
    }
}
