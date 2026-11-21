//! Settings helper for Nearby / KDE Connect pairing (W448). Mirrors mde-shell wrapper
//! via `kdeconnect-cli` + `busctl` (no new crates).

use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub paired: bool,
    pub reachable: bool,
}

pub fn ensure_daemon() {
    let _ = Command::new("systemctl")
        .args(["--user", "start", "kdeconnectd.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !daemon_up() {
        let _ = Command::new("kdeconnectd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

fn daemon_up() -> bool {
    Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.kdeconnect",
            "/modules/kdeconnect",
            "org.kde.kdeconnect.daemon",
            "deviceIds",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn installed() -> bool {
    which("kdeconnectd") || which("kdeconnect-cli")
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn list_devices() -> Vec<Device> {
    ensure_daemon();
    let Ok(out) = Command::new("kdeconnect-cli").args(["-l"]).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut devices = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('-').trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let rest = rest.trim();
        let id = rest.split_whitespace().next().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let lower = rest.to_lowercase();
        devices.push(Device {
            id,
            name: name.trim().to_string(),
            paired: lower.contains("paired"),
            reachable: lower.contains("reachable"),
        });
    }
    devices
}

pub fn pair(id: &str) -> Result<(), String> {
    ensure_daemon();
    run_cli(&["-d", id, "--pair"])
}

pub fn unpair(id: &str) -> Result<(), String> {
    ensure_daemon();
    run_cli(&["-d", id, "--unpair"])
}

pub fn refresh_discovery() -> Result<(), String> {
    ensure_daemon();
    run_cli(&["--refresh"])
}

fn run_cli(args: &[&str]) -> Result<(), String> {
    if !which("kdeconnect-cli") {
        return Err("kdeconnect-cli missing — install kde-connect".into());
    }
    let st = Command::new("kdeconnect-cli")
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("kdeconnect-cli failed ({st})"))
    }
}

const DAEMON_PATH: &str = "/modules/kdeconnect";

fn busctl_call(args: &[&str]) -> Option<String> {
    let out = Command::new("busctl").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn bus_name() -> &'static str {
  const BUS: &str = "org.kde.kdeconnect";
    if busctl_call(&["--user", "call", BUS, DAEMON_PATH, "org.kde.kdeconnect.daemon", "deviceIds"])
        .is_some()
    {
        BUS
    } else {
        "org.kde.kdeconnect.daemon"
    }
}

/// Phone battery percent when the battery plugin reports it (A078).
pub fn device_battery(id: &str) -> Option<(u8, bool)> {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/battery");
    let out = busctl_call(&[
        "--user",
        "get-property",
        bus,
        &path,
        "org.kde.kdeconnect.device.battery",
        "charge",
    ])?;
    let pct = out
        .split_whitespace()
        .filter_map(|t| t.parse::<i32>().ok())
        .next()
        .unwrap_or(-1);
    if pct < 0 {
        return None;
    }
    let charging = busctl_call(&[
        "--user",
        "get-property",
        bus,
        &path,
        "org.kde.kdeconnect.device.battery",
        "isCharging",
    ])
    .map(|s| s.contains("true"))
    .unwrap_or(false);
    Some((pct.clamp(0, 100) as u8, charging))
}

/// Whether the notifications plugin is present on the device dbus path (A078).
pub fn notifications_plugin_active(id: &str) -> bool {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/notifications");
    busctl_call(&["--user", "introspect", bus, &path]).is_some()
}
