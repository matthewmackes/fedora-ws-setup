//! W447: KDE Connect helper — D-Bus (`busctl`/`gdbus`) first, `kdeconnect-cli` fallback.
//! Used by Nearby Share (W395), pairing (W448), Clipboard → Nearby (W437).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BUS: &str = "org.kde.kdeconnect";
const DAEMON_PATH: &str = "/modules/kdeconnect";
const DAEMON_IFACE: &str = "org.kde.kdeconnect.daemon";

#[derive(Debug, Clone, Default)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub paired: bool,
    pub reachable: bool,
    pub type_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub installed: bool,
    pub daemon_running: bool,
    pub hint: String,
    pub devices: Vec<Device>,
    pub fetched_at: Option<Instant>,
}

impl Status {
    pub fn empty_hint() -> Self {
        Self {
            installed: which("kdeconnectd") || which("kdeconnect-cli"),
            daemon_running: false,
            hint: if which("kdeconnectd") || which("kdeconnect-cli") {
                "Starting kdeconnectd…".into()
            } else {
                "Install kde-connect (dnf) · see docs/kde-connect.md · open ports 1716–1764".into()
            },
            devices: Vec::new(),
            fetched_at: None,
        }
    }
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

fn run_out(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Start `kdeconnectd` on demand (leave running). Returns true if a daemon appears reachable.
pub fn ensure_daemon() -> bool {
    if daemon_alive() {
        return true;
    }
    if !which("kdeconnectd") && !which("kdeconnect-cli") {
        return false;
    }
    // Prefer systemd user unit when present.
    let _ = Command::new("systemctl")
        .args(["--user", "start", "kdeconnectd.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if daemon_alive() {
        return true;
    }
    if which("kdeconnectd") {
        let _ = Command::new("kdeconnectd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(150));
            if daemon_alive() {
                return true;
            }
        }
    }
    daemon_alive()
}

fn daemon_alive() -> bool {
    if busctl_call(&[
        "--user",
        "call",
        BUS,
        DAEMON_PATH,
        DAEMON_IFACE,
        "deviceIds",
    ])
    .is_some()
    {
        return true;
    }
    // Alternate bus name used on some builds.
    if busctl_call(&[
        "--user",
        "call",
        "org.kde.kdeconnect.daemon",
        DAEMON_PATH,
        DAEMON_IFACE,
        "deviceIds",
    ])
    .is_some()
    {
        return true;
    }
    which("kdeconnect-cli")
        && run_out("kdeconnect-cli", &["--list-devices"]).is_some()
}

fn bus_name() -> &'static str {
    // Prefer canonical name; fall back if only .daemon is claimed.
    if busctl_call(&["--user", "get-property", BUS, DAEMON_PATH, DAEMON_IFACE, "isRegistered"]).is_some()
        || busctl_call(&["--user", "call", BUS, DAEMON_PATH, DAEMON_IFACE, "deviceIds"]).is_some()
    {
        BUS
    } else {
        "org.kde.kdeconnect.daemon"
    }
}

fn busctl_call(args: &[&str]) -> Option<String> {
    run_out("busctl", args)
}

fn device_prop(bus: &str, id: &str, prop: &str) -> Option<String> {
    let path = format!("{DAEMON_PATH}/devices/{id}");
    let out = busctl_call(&[
        "--user",
        "get-property",
        bus,
        &path,
        "org.kde.kdeconnect.device",
        prop,
    ])?;
    parse_busctl_variant(&out)
}

fn parse_busctl_variant(s: &str) -> Option<String> {
    // Examples: `s "Pixel"` · `b true` · `as 2 "a" "b"`
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("s ") {
        return Some(unquote(rest.trim()));
    }
    if let Some(rest) = t.strip_prefix("b ") {
        return Some(rest.trim().to_string());
    }
    if let Some(rest) = t.strip_prefix("as ") {
        // as N "id1" "id2" …
        return Some(rest.to_string());
    }
    // Fall through: strip type letter
    let parts: Vec<&str> = t.splitn(2, char::is_whitespace).collect();
    if parts.len() == 2 {
        return Some(unquote(parts[1].trim()));
    }
    Some(unquote(t))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        inner.to_string()
    } else {
        s.to_string()
    }
}

fn parse_as_string_list(busctl_out: &str) -> Vec<String> {
    // `as 2 "id1" "id2"` or nested in call reply `as 2 "id1" "id2"`
    let mut ids = Vec::new();
    let mut rest = busctl_out;
    if let Some(idx) = rest.find("as ") {
        rest = &rest[idx + 3..];
    }
    // skip count
    let mut chars = rest.chars().peekable();
    while chars.peek().is_some_and(|c| c.is_whitespace() || c.is_ascii_digit()) {
        chars.next();
    }
    let rest: String = chars.collect();
    let mut in_q = false;
    let mut cur = String::new();
    for c in rest.chars() {
        match c {
            '"' if !in_q => {
                in_q = true;
                cur.clear();
            }
            '"' if in_q => {
                in_q = false;
                if !cur.is_empty() {
                    ids.push(cur.clone());
                }
                cur.clear();
            }
            c if in_q => cur.push(c),
            _ => {}
        }
    }
    ids
}

/// Refresh device list (starts daemon when possible).
pub fn refresh() -> Status {
    let installed = which("kdeconnectd") || which("kdeconnect-cli");
    if !installed {
        return Status {
            installed: false,
            daemon_running: false,
            hint: "Install package kde-connect · firewall 1716–1764 · docs/kde-connect.md".into(),
            devices: Vec::new(),
            fetched_at: Some(Instant::now()),
        };
    }
    let running = ensure_daemon();
    if !running {
        return Status {
            installed: true,
            daemon_running: false,
            hint: "kdeconnectd not running · check firewall / session".into(),
            devices: Vec::new(),
            fetched_at: Some(Instant::now()),
        };
    }

    if let Some(devices) = list_devices_dbus() {
        return Status {
            installed: true,
            daemon_running: true,
            hint: if devices.is_empty() {
                "No devices · open KDE Connect on your phone · Pair below".into()
            } else {
                format!("{} device(s)", devices.len())
            },
            devices,
            fetched_at: Some(Instant::now()),
        };
    }

    let devices = list_devices_cli();
    Status {
        installed: true,
        daemon_running: true,
        hint: if devices.is_empty() {
            "No devices (cli) · pair from phone or Settings → Nearby".into()
        } else {
            format!("{} device(s) · cli", devices.len())
        },
        devices,
        fetched_at: Some(Instant::now()),
    }
}

fn list_devices_dbus() -> Option<Vec<Device>> {
    let bus = bus_name();
    let raw = busctl_call(&["--user", "call", bus, DAEMON_PATH, DAEMON_IFACE, "deviceIds"])?;
    let ids = parse_as_string_list(&raw);
    if ids.is_empty() && !raw.contains("as 0") {
        // Some versions use devices(bool onlyReachable, bool onlyPaired)
        let raw2 = busctl_call(&[
            "--user",
            "call",
            bus,
            DAEMON_PATH,
            DAEMON_IFACE,
            "devices",
            "bb",
            "false",
            "false",
        ])?;
        let ids2 = parse_as_string_list(&raw2);
        return Some(ids2.into_iter().filter_map(|id| device_from_id(bus, &id)).collect());
    }
    Some(
        ids.into_iter()
            .filter_map(|id| device_from_id(bus, &id))
            .collect(),
    )
}

fn device_from_id(bus: &str, id: &str) -> Option<Device> {
    let name = device_prop(bus, id, "name").unwrap_or_else(|| id.to_string());
    let paired = device_prop(bus, id, "isTrusted")
        .or_else(|| device_prop(bus, id, "isPaired"))
        .map(|v| v == "true")
        .unwrap_or(false);
    let reachable = device_prop(bus, id, "isReachable")
        .map(|v| v == "true")
        .unwrap_or(false);
    let type_name = device_prop(bus, id, "type").unwrap_or_default();
    Some(Device {
        id: id.to_string(),
        name,
        paired,
        reachable,
        type_name,
    })
}

fn list_devices_cli() -> Vec<Device> {
    let Some(text) = run_out("kdeconnect-cli", &["-l"]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        // "- Pixel: id123 (paired and reachable)"
        let line = line.trim().trim_start_matches('-').trim();
        if line.is_empty() {
            continue;
        }
        let (name, rest) = match line.split_once(':') {
            Some(p) => p,
            None => continue,
        };
        let rest = rest.trim();
        let id = rest
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let lower = rest.to_lowercase();
        out.push(Device {
            id,
            name: name.trim().to_string(),
            paired: lower.contains("paired"),
            reachable: lower.contains("reachable"),
            type_name: String::new(),
        });
    }
    out
}

fn device_method(id: &str, plugin_path: &str, iface: &str, method: &str, args: &[&str]) -> Result<(), String> {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}{plugin_path}");
    let mut cmd_args = vec![
        "--user".into(),
        "call".into(),
        bus.to_string(),
        path,
        iface.to_string(),
        method.to_string(),
    ];
    for a in args {
        cmd_args.push((*a).to_string());
    }
    let str_refs: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();
    if busctl_call(&str_refs).is_some() {
        return Ok(());
    }
    Err(format!("D-Bus {method} failed"))
}

pub fn pair(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(id, "", "org.kde.kdeconnect.device", "requestPair", &[]).is_ok()
        || device_method(id, "", "org.kde.kdeconnect.device", "pair", &[]).is_ok()
    {
        return Ok(());
    }
    cli_device(id, &["--pair"])
}

pub fn unpair(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(id, "", "org.kde.kdeconnect.device", "unpair", &[]).is_ok() {
        return Ok(());
    }
    cli_device(id, &["--unpair"])
}

pub fn accept_pairing(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(id, "", "org.kde.kdeconnect.device", "acceptPairing", &[]).is_ok() {
        return Ok(());
    }
    // No stable CLI accept; pair request is enough tip.
    Err("Accept pairing on the other device, or retry Pair".into())
}

pub fn ping(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(
        id,
        "/ping",
        "org.kde.kdeconnect.device.ping",
        "sendPing",
        &[],
    )
    .is_ok()
    {
        return Ok(());
    }
    cli_device(id, &["--ping"])
}

pub fn ring(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(
        id,
        "/findmyphone",
        "org.kde.kdeconnect.device.findmyphone",
        "ring",
        &[],
    )
    .is_ok()
    {
        return Ok(());
    }
    cli_device(id, &["--ring"])
}

pub fn share_text(id: &str, text: &str) -> Result<(), String> {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/share");
    let args = [
        "--user",
        "call",
        bus,
        &path,
        "org.kde.kdeconnect.device.share",
        "shareText",
        "s",
        text,
    ];
    if busctl_call(&args).is_some() {
        return Ok(());
    }
    let _ = Command::new("kdeconnect-cli")
        .args(["-d", id, "--share-text", text])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn share_url_or_file(id: &str, path_or_url: &str) -> Result<(), String> {
    ensure_daemon();
    let url = if path_or_url.starts_with("file:")
        || path_or_url.starts_with("http:")
        || path_or_url.starts_with("https:")
    {
        path_or_url.to_string()
    } else {
        format!("file://{path_or_url}")
    };
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/share");
    let args = [
        "--user",
        "call",
        bus,
        &path,
        "org.kde.kdeconnect.device.share",
        "shareUrl",
        "s",
        &url,
    ];
    if busctl_call(&args).is_some() {
        return Ok(());
    }
    cli_device(id, &["--share", path_or_url])
}

pub fn send_clipboard(id: &str) -> Result<(), String> {
    ensure_daemon();
    // Prefer explicit CLI which reads system clipboard.
    if which("kdeconnect-cli") {
        return cli_device(id, &["--send-clipboard"]);
    }
    // Fallback: share wl-paste text.
    let text = run_out("wl-paste", &["--no-newline"]).unwrap_or_default();
    if text.is_empty() {
        return Err("Clipboard empty".into());
    }
    share_text(id, &text)
}

/// A078: phone battery percent when the battery plugin reports it.
pub fn device_battery(id: &str) -> Option<(u8, bool)> {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/battery");
    // chargePercent (i) + isCharging (b) via properties.
    let out = busctl_call(&[
        "--user",
        "get-property",
        bus,
        &path,
        "org.kde.kdeconnect.device.battery",
        "charge",
    ])?;
    // busctl prints `i 87` or similar.
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

/// A078: best-effort SMS app launch / conversation tip via telephony plugin.
pub fn open_sms(id: &str) -> Result<(), String> {
    ensure_daemon();
    if device_method(
        id,
        "/sms",
        "org.kde.kdeconnect.device.sms",
        "requestAllConversations",
        &[],
    )
    .is_ok()
    {
        return Ok(());
    }
    Err("SMS plugin unavailable on this device".into())
}

/// A078: mirror phone notifications into the desktop bus (plugin enable tip).
pub fn notifications_plugin_active(id: &str) -> bool {
    ensure_daemon();
    let bus = bus_name();
    let path = format!("{DAEMON_PATH}/devices/{id}/notifications");
    busctl_call(&[
        "--user",
        "introspect",
        bus,
        &path,
    ])
    .is_some()
}

pub fn media_play_pause(id: &str) -> Result<(), String> {
    device_method(
        id,
        "/mprisremote",
        "org.kde.kdeconnect.device.mprisremote",
        "sendAction",
        &["s", "PlayPause"],
    )
    .or_else(|_| {
        // Older iface
        device_method(
            id,
            "/mprisremote",
            "org.kde.kdeconnect.device.mprisremote",
            "action",
            &["s", "PlayPause"],
        )
    })
}

pub fn presenter_pointer(id: &str) -> Result<(), String> {
    // Best-effort: start pointer lock / next slide tip via presentation plugin.
    device_method(
        id,
        "/presenter",
        "org.kde.kdeconnect.device.presenter",
        "pointer",
        &[],
    )
    .or_else(|_| Ok(())) // soft — plugin may be absent
    .map(|_| ())
}

pub fn lock_device(id: &str) -> Result<(), String> {
    if device_method(
        id,
        "/lockdevice",
        "org.kde.kdeconnect.device.lockdevice",
        "setLocked",
        &["b", "true"],
    )
    .is_ok()
    {
        return Ok(());
    }
    cli_device(id, &["--lock"])
}

pub fn send_keys(id: &str, keys: &str) -> Result<(), String> {
    // Remote input / keyboard plugin.
    if device_method(
        id,
        "/mousepad",
        "org.kde.kdeconnect.device.mousepad",
        "sendKeyPress",
        &["s", keys, "b", "false", "b", "false", "b", "false", "b", "false"],
    )
    .is_ok()
    {
        return Ok(());
    }
    cli_device(id, &["--send-keys", keys])
}

fn cli_device(id: &str, extra: &[&str]) -> Result<(), String> {
    if !which("kdeconnect-cli") {
        return Err("kdeconnect-cli missing".into());
    }
    let mut args = vec!["-d", id];
    args.extend_from_slice(extra);
    let status = Command::new("kdeconnect-cli")
        .args(&args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kdeconnect-cli failed ({status})"))
    }
}

/// Default Open / Save / Dismiss actions for inbound share toasts (W395).
/// KDE Connect posts via org.freedesktop.Notifications (hosted by mde-shell);
/// when synthesizing a toast, attach these actions.
pub fn inbound_toast_actions(_path: &str) -> Vec<(String, String)> {
    vec![
        ("open".into(), "Open".into()),
        ("save".into(), "Save".into()),
        ("dismiss".into(), "Dismiss".into()),
    ]
}
