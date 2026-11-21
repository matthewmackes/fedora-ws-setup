//! System actions via stock CLIs / D-Bus-facing tools (NM, BlueZ, WirePlumber).

use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct RadioState {
    pub wifi: bool,
    pub bluetooth: bool,
    pub airplane: bool,
}

pub fn probe_radios() -> RadioState {
    RadioState {
        wifi: nm_wifi_enabled().unwrap_or(true),
        bluetooth: bt_powered().unwrap_or(true),
        airplane: nm_airplane().unwrap_or(false),
    }
}

pub fn set_wifi(on: bool) -> Result<(), String> {
    // Turning Wi-Fi on clears airplane (nmcli radio all was off) (W58).
    if on {
        let _ = run_ok("nmcli", &["radio", "all", "on"]);
    }
    run_ok("nmcli", &["radio", "wifi", if on { "on" } else { "off" }])
}

pub fn set_bluetooth(on: bool) -> Result<(), String> {
    // bluetoothctl power on/off
    run_ok(
        "bluetoothctl",
        &["power", if on { "on" } else { "off" }],
    )
}

pub fn set_airplane(on: bool) -> Result<(), String> {
    if on {
        let _ = set_wifi(false);
        let _ = set_bluetooth(false);
        run_ok("nmcli", &["radio", "all", "off"])
    } else {
        run_ok("nmcli", &["radio", "all", "on"])?;
        let _ = set_wifi(true);
        Ok(())
    }
}

pub fn volume_adjust(delta_pct: i32) -> Result<f32, String> {
    let arg = if delta_pct >= 0 {
        format!("{delta_pct}%+")
    } else {
        format!("{}%-", delta_pct.unsigned_abs())
    };
    run_ok("wpctl", &["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &arg])?;
    volume_level()
}

pub fn volume_toggle_mute() -> Result<(f32, bool), String> {
    run_ok("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])?;
    let level = volume_level().unwrap_or(0.0);
    let muted = volume_muted().unwrap_or(false);
    Ok((level, muted))
}

pub fn volume_level() -> Result<f32, String> {
    let out = run_stdout("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    // "Volume: 0.72" or "Volume: 0.72 [MUTED]"
    let vol = out
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("parse volume: {out}"))?
        .parse::<f32>()
        .map_err(|e| e.to_string())?;
    Ok(vol.clamp(0.0, 1.0))
}

pub fn volume_muted() -> Result<bool, String> {
    let out = run_stdout("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    Ok(out.contains("MUTED"))
}

/// One `wpctl status` node (sink / source / stream). Matches mde-control Sound parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioNode {
    pub id: u32,
    pub label: String,
}

/// Application streams from `wpctl status` (W399). Skips nested channel leaves.
pub fn app_streams() -> Vec<AudioNode> {
    let Ok(out) = Command::new("wpctl").arg("status").output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_app_streams(&String::from_utf8_lossy(&out.stdout))
}

pub fn parse_app_streams(text: &str) -> Vec<AudioNode> {
    let mut streams = Vec::new();
    let mut section = "";
    let mut video = false;
    for line in text.lines() {
        if let Some(kind) = audio_header(line) {
            if kind == "video" || kind == "settings" {
                video = true;
                section = "";
            } else if !video {
                section = kind;
            }
            continue;
        }
        if video || section != "streams" {
            continue;
        }
        let Some(node) = parse_audio_node(line) else {
            continue;
        };
        let lower = node.label.to_ascii_lowercase();
        if lower.starts_with("output_") || lower.starts_with("input_") {
            continue;
        }
        streams.push(node);
        if streams.len() >= 12 {
            break;
        }
    }
    streams
}

fn parse_audio_node(line: &str) -> Option<AudioNode> {
    let rest = line.trim_start_matches(|c: char| !c.is_ascii_digit());
    let (num, after) = rest.split_once('.')?;
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let id = num.parse().ok()?;
    let label = after
        .split('[')
        .next()
        .unwrap_or(after)
        .trim()
        .to_string();
    if label.is_empty() {
        return None;
    }
    Some(AudioNode { id, label })
}

fn audio_header(line: &str) -> Option<&'static str> {
    if parse_audio_node(line).is_some() {
        return None;
    }
    let trimmed = line.trim();
    if trimmed.contains("Sinks:") {
        Some("sinks")
    } else if trimmed.contains("Sources:") {
        Some("sources")
    } else if trimmed.contains("Filters:") {
        Some("filters")
    } else if trimmed.contains("Streams:") {
        Some("streams")
    } else if trimmed == "Video" {
        Some("video")
    } else if trimmed.starts_with("Settings") {
        Some("settings")
    } else {
        None
    }
}

/// Per-node volume step via `wpctl set-volume` (W399).
pub fn node_volume(id: u32, delta_pct: i32) -> Result<(), String> {
    let step = if delta_pct >= 0 {
        format!("{delta_pct}%+")
    } else {
        format!("{}%-", delta_pct.unsigned_abs())
    };
    run_ok("wpctl", &["set-volume", &id.to_string(), &step])
}

/// Per-node mute toggle via `wpctl set-mute` (W399).
pub fn node_mute(id: u32) -> Result<(), String> {
    run_ok("wpctl", &["set-mute", &id.to_string(), "toggle"])
}

/// Default source mute via `pactl` (W170).
pub fn mic_muted() -> Result<bool, String> {
    let out = run_stdout("pactl", &["get-source-mute", "@DEFAULT_SOURCE@"])?;
    // "Mute: yes" / "Mute: no"
    Ok(out.to_ascii_lowercase().contains("yes"))
}

pub fn mic_toggle_mute() -> Result<bool, String> {
    run_ok("pactl", &["set-source-mute", "@DEFAULT_SOURCE@", "toggle"])?;
    mic_muted()
}

pub fn brightness_adjust(delta_pct: i32) -> Result<f32, String> {
    let arg = if delta_pct >= 0 {
        format!("{delta_pct}%+")
    } else {
        format!("{}%-", delta_pct.unsigned_abs())
    };
    run_ok("brightnessctl", &["-q", "set", &arg])?;
    brightness_level()
}

pub fn brightness_level() -> Result<f32, String> {
    let cur = run_stdout("brightnessctl", &["g"])?
        .trim()
        .parse::<f32>()
        .map_err(|e| e.to_string())?;
    let max = run_stdout("brightnessctl", &["m"])?
        .trim()
        .parse::<f32>()
        .map_err(|e| e.to_string())?;
    if max <= 0.0 {
        return Ok(0.0);
    }
    Ok((cur / max).clamp(0.0, 1.0))
}

/// Battery percent from `/sys/class/power_supply/*/capacity` when a Battery exists.
pub fn battery_capacity() -> Option<u8> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for ent in dir.flatten() {
        let path = ent.path();
        let typ = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if !typ.trim().eq_ignore_ascii_case("Battery") {
            continue;
        }
        let cap = std::fs::read_to_string(path.join("capacity")).ok()?;
        if let Ok(v) = cap.trim().parse::<u8>() {
            return Some(v.min(100));
        }
    }
    None
}

/// True when a Battery supply reports Charging / Full.
pub fn battery_charging() -> Option<bool> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for ent in dir.flatten() {
        let path = ent.path();
        let typ = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if !typ.trim().eq_ignore_ascii_case("Battery") {
            continue;
        }
        let status = std::fs::read_to_string(path.join("status")).ok()?;
        let s = status.trim();
        return Some(s.eq_ignore_ascii_case("Charging") || s.eq_ignore_ascii_case("Full"));
    }
    None
}

/// True when `powerprofilesctl` is on PATH (power-profiles-daemon).
pub fn powerprofilesctl_available() -> bool {
    Command::new("powerprofilesctl")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Current power-profiles-daemon profile name, if available.
pub fn power_profile() -> Option<String> {
    let out = run_stdout("powerprofilesctl", &["get"]).ok()?;
    let name = out.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Set power-profiles-daemon profile (`power-saver`, `balanced`, …).
pub fn set_power_profile(profile: &str) -> Result<(), String> {
    run_ok("powerprofilesctl", &["set", profile])
}

/// Absolute brightness percent 0–100 via brightnessctl.
pub fn brightness_percent() -> Result<u32, String> {
    let level = brightness_level()?;
    Ok((level * 100.0).round().clamp(0.0, 100.0) as u32)
}

pub fn set_brightness_percent(pct: u32) -> Result<f32, String> {
    let pct = pct.min(100);
    run_ok("brightnessctl", &["-q", "set", &format!("{pct}%")])?;
    brightness_level()
}

/// Best-effort Wi-Fi powersave without sudo (W400).
/// Tries `iw`, then `nmcli connection modify … 802-11-wireless.powersave`.
pub fn set_wifi_powersave(on: bool) -> String {
    let mut notes = Vec::new();
    let ifaces = wifi_ifaces();
    for iface in &ifaces {
        let arg = if on { "on" } else { "off" };
        if run_ok("iw", &["dev", iface, "set", "power_save", arg]).is_ok() {
            notes.push(format!("iw {iface}"));
        }
    }
    let target = if on { "3" } else { "2" }; // NM: 3=enable, 2=disable
    for name in active_wifi_connections() {
        if run_ok(
            "nmcli",
            &[
                "connection",
                "modify",
                &name,
                "802-11-wireless.powersave",
                target,
            ],
        )
        .is_ok()
        {
            notes.push(format!("nmcli {name}"));
        }
    }
    if notes.is_empty() {
        if on {
            "Wi-Fi powersave · unavailable".into()
        } else {
            "Wi-Fi powersave · off unavailable".into()
        }
    } else {
        format!("Wi-Fi powersave · {}", notes.join(" · "))
    }
}

fn wifi_ifaces() -> Vec<String> {
    let Ok(out) = Command::new("iw").arg("dev").output() else {
        return Vec::new();
    };
    let mut ifaces = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Interface ") {
            let name = rest.split_whitespace().next().unwrap_or("").trim();
            if !name.is_empty() && !ifaces.iter().any(|i| i == name) {
                ifaces.push(name.to_string());
            }
        }
    }
    ifaces
}

fn active_wifi_connections() -> Vec<String> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
        .output()
    else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim();
        let ty = parts.next().unwrap_or("").trim();
        if !name.is_empty() && ty.eq_ignore_ascii_case("wifi") {
            names.push(name.to_string());
        }
    }
    names
}

/// Restart swaybg for a wallpaper path (A046 per-desktop).
pub fn set_wallpaper(path: &str) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let pid_path = std::path::Path::new(&runtime).join("mde-wallpaper.pid");
    if let Ok(text) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        let _ = std::fs::remove_file(&pid_path);
    }
    let path = path.trim();
    if path.is_empty() {
        return Ok(());
    }
    if !std::path::Path::new(path).is_file() {
        return Err(format!("wallpaper is not a file: {path}"));
    }
    match Command::new("swaybg")
        .args(["-i", path, "-m", "fill"])
        .spawn()
    {
        Ok(child) => {
            let _ = std::fs::write(&pid_path, child.id().to_string());
            Ok(())
        }
        Err(e) => Err(format!("swaybg: {e}")),
    }
}

/// Toggle night light via wlsunset or gammastep (same helpers as mde-control).
pub fn set_night_light(on: bool, strength: u8) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let pid_path = std::path::Path::new(&runtime).join("mde-night-light.pid");
    if let Ok(text) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        let _ = std::fs::remove_file(&pid_path);
    }
    if !on {
        return Ok(());
    }
    let kelvin = mde_config::night_light_kelvin(strength).to_string();
    let mut child = Command::new("wlsunset");
    child.args(["-t", "4000", "-T", &kelvin]);
    let spawned = child.spawn().or_else(|_| {
        let mut alt = Command::new("gammastep");
        alt.args(["-O", &kelvin]);
        alt.spawn()
    });
    match spawned {
        Ok(child) => {
            let _ = std::fs::write(&pid_path, child.id().to_string());
            Ok(())
        }
        Err(e) => Err(format!("night light needs wlsunset or gammastep ({e})")),
    }
}

fn color_filter_state_path() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::Path::new(&runtime).join("mde-color-filter.state")
}

fn color_filter_clear() {
    let path = color_filter_state_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        let line = text.lines().next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("pid ") {
            if let Ok(pid) = rest.parse::<i32>() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
        } else if line == "hyprshade" {
            let _ = Command::new("hyprshade").arg("off").status();
        } else if line == "wl-gammactl" {
            let _ = Command::new("pkill").args(["-x", "wl-gammactl"]).status();
            // Restore neutral gamma when possible.
            let _ = Command::new("wl-gammactl")
                .args(["-c", "1.0", "-g", "1.0", "-b", "1.0"])
                .spawn();
        }
    }
    let _ = std::fs::remove_file(&path);
}

pub fn color_filter_active() -> bool {
    color_filter_state_path().is_file()
}

/// W423: stronger color / night-style filter (hyprshade → wl-gammactl → wlsunset/gammastep).
pub fn toggle_color_filter() -> Result<String, String> {
    if color_filter_active() {
        color_filter_clear();
        return Ok("Color filters · off".into());
    }
    let path = color_filter_state_path();
    // hyprshade (Hyprland) — invert or grayscale shader if present.
    for shader in ["grayscale", "invert", "blue-light-filter"] {
        if Command::new("hyprshade")
            .args(["on", shader])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            let _ = std::fs::write(&path, "hyprshade\n");
            return Ok(format!("Color filters · hyprshade · {shader}"));
        }
    }
    // wl-gammactl: strong warm/dim contrast (contrast / gamma / brightness).
    if let Ok(child) = Command::new("wl-gammactl")
        .args(["-c", "1.15", "-g", "0.85", "-b", "0.92"])
        .spawn()
    {
        let _ = std::fs::write(&path, format!("wl-gammactl\npid {}\n", child.id()));
        return Ok("Color filters · wl-gammactl".into());
    }
    // Constant strong warm preset (stronger than night light 4000K).
    let mut child = Command::new("wlsunset");
    child.args(["-t", "2500", "-T", "2500"]);
    let spawned = child.spawn().or_else(|_| {
        let mut alt = Command::new("gammastep");
        alt.args(["-O", "2500"]);
        alt.spawn()
    });
    match spawned {
        Ok(child) => {
            let _ = std::fs::write(&path, format!("pid {}\n", child.id()));
            Ok("Color filters · 2500K".into())
        }
        Err(e) => Err(format!(
            "Color filters · needs hyprshade, wl-gammactl, wlsunset, or gammastep ({e})"
        )),
    }
}

/// Aggregate RX/TX byte counters across non-loopback interfaces.
pub fn net_counters() -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return (0, 0);
    };
    let mut rx = 0u64;
    let mut tx = 0u64;
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "lo" {
            continue;
        }
        let base = entry.path().join("statistics");
        if let Ok(text) = std::fs::read_to_string(base.join("rx_bytes")) {
            rx = rx.saturating_add(text.trim().parse().unwrap_or(0));
        }
        if let Ok(text) = std::fs::read_to_string(base.join("tx_bytes")) {
            tx = tx.saturating_add(text.trim().parse().unwrap_or(0));
        }
    }
    (rx, tx)
}

/// Rate between successive polls (bytes/sec). First call seeds and returns zeros.
pub fn net_speed_rates() -> (u64, u64) {
    use std::sync::Mutex;
    use std::time::Instant;
    struct Sample {
        at: Instant,
        rx: u64,
        tx: u64,
    }
    static LAST: Mutex<Option<Sample>> = Mutex::new(None);
    let (rx, tx) = net_counters();
    let now = Instant::now();
    let Ok(mut guard) = LAST.lock() else {
        return (0, 0);
    };
    let rates = if let Some(prev) = guard.as_ref() {
        let secs = now.duration_since(prev.at).as_secs_f64().max(0.05);
        let down = ((rx.saturating_sub(prev.rx) as f64) / secs).round() as u64;
        let up = ((tx.saturating_sub(prev.tx) as f64) / secs).round() as u64;
        (down, up)
    } else {
        (0, 0)
    };
    *guard = Some(Sample { at: now, rx, tx });
    rates
}

/// Human-readable ↓↑ line for the network glyph (W441).
pub fn net_speed_status() -> String {
    let (down, up) = net_speed_rates();
    format!("↓ {} · ↑ {}", format_rate(down), format_rate(up))
}

fn format_rate(bps: u64) -> String {
    if bps >= 1_000_000 {
        format!("{:.1} MB/s", bps as f64 / 1_000_000.0)
    } else if bps >= 1_000 {
        format!("{:.0} KB/s", bps as f64 / 1_000.0)
    } else {
        format!("{bps} B/s")
    }
}

/// Top Wi-Fi SSIDs from `nmcli` for the quick-connect flyout (W28).
#[derive(Debug, Clone)]
pub struct WifiAp {
    pub ssid: String,
    pub signal: u8,
    pub connected: bool,
}

pub fn wifi_list(limit: usize) -> Vec<WifiAp> {
    let _ = Command::new("nmcli")
        .args(["dev", "wifi", "rescan"])
        .status();
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "IN-USE,SSID,SIGNAL", "dev", "wifi", "list"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let mut nets = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(3, ':');
        let in_use = parts.next().unwrap_or("").trim();
        let ssid = parts.next().unwrap_or("").trim();
        let signal = parts
            .next()
            .unwrap_or("0")
            .trim()
            .parse::<u8>()
            .unwrap_or(0);
        if ssid.is_empty() || nets.iter().any(|a: &WifiAp| a.ssid == ssid) {
            continue;
        }
        nets.push(WifiAp {
            ssid: ssid.into(),
            signal,
            connected: in_use == "*",
        });
        if nets.len() >= limit {
            break;
        }
    }
    nets
}

/// True when NetworkManager already has a connection named like `ssid` (W147).
pub fn wifi_known_connection(ssid: &str) -> bool {
    let ssid = ssid.trim();
    if ssid.is_empty() {
        return false;
    }
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|line| line.trim() == ssid)
}

pub fn wifi_connect(ssid: &str) -> Result<(), String> {
    let ssid = ssid.trim();
    if ssid.is_empty() {
        return Err("empty SSID".into());
    }
    // W147: unknown SSID → password tip instead of interactive nmcli prompt.
    if !wifi_known_connection(ssid) {
        return Err("Enter password in NetworkManager / nmtui".into());
    }
    run_ok("nmcli", &["dev", "wifi", "connect", ssid])
}

/// VPN / WireGuard profile for the QS VPN flyout (W397).
#[derive(Debug, Clone)]
pub struct VpnConn {
    pub name: String,
    pub kind: String,
    pub active: bool,
}

/// List NetworkManager VPN and WireGuard connections (W397).
pub fn vpn_list() -> Vec<VpnConn> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let active = active_connection_names();
    let mut list = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim();
        let kind = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let is_vpn = kind == "vpn"
            || kind == "wireguard"
            || kind.contains("vpn")
            || kind.contains("wireguard");
        if !is_vpn {
            continue;
        }
        list.push(VpnConn {
            name: name.into(),
            kind: if kind.contains("wireguard") {
                "WireGuard".into()
            } else {
                "VPN".into()
            },
            active: active.iter().any(|a| a == name),
        });
        if list.len() >= 16 {
            break;
        }
    }
    list
}

pub fn vpn_connect(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("empty VPN name".into());
    }
    run_ok("nmcli", &["connection", "up", "id", name])
}

pub fn vpn_disconnect(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("empty VPN name".into());
    }
    run_ok("nmcli", &["connection", "down", "id", name])
}

/// Mobile hotspot status for the QS flyout (W396).
#[derive(Debug, Clone, Default)]
pub struct HotspotInfo {
    pub active: bool,
    pub ssid: String,
    pub connection: String,
    pub device: String,
    /// Alias for `device` (A039 call sites).
    pub iface: String,
    pub password: String,
}

const HOTSPOT_SSID: &str = "MDE-Hotspot";

/// Detect an active or known Wi-Fi AP / shared hotspot connection (W396).
pub fn hotspot_status() -> HotspotInfo {
    let mut info = HotspotInfo {
        ssid: HOTSPOT_SSID.into(),
        ..HotspotInfo::default()
    };
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
    else {
        return info;
    };
    if !out.status.success() {
        return info;
    }
    let active = active_connection_names();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim();
        let kind = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let wireless = kind.contains("wireless") || kind == "wifi";
        if !wireless && !name.eq_ignore_ascii_case("Hotspot") {
            continue;
        }
        let mode = run_stdout(
            "nmcli",
            &["-g", "802-11-wireless.mode", "connection", "show", name],
        )
        .unwrap_or_default();
        let mode = mode.trim();
        let named_hotspot = name.eq_ignore_ascii_case("Hotspot")
            || name.eq_ignore_ascii_case(HOTSPOT_SSID)
            || name.to_ascii_lowercase().contains("hotspot");
        if mode != "ap" && !named_hotspot {
            continue;
        }
        let is_active = active.iter().any(|a| a == name);
        let ssid = run_stdout(
            "nmcli",
            &["-g", "802-11-wireless.ssid", "connection", "show", name],
        )
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if named_hotspot {
                HOTSPOT_SSID.into()
            } else {
                name.into()
            }
        });
        let device = run_stdout(
            "nmcli",
            &["-g", "connection.interface-name", "connection", "show", name],
        )
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
        let password = run_stdout(
            "nmcli",
            &[
                "-s",
                "-g",
                "802-11-wireless-security.psk",
                "connection",
                "show",
                name,
            ],
        )
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
        info = HotspotInfo {
            active: is_active,
            ssid,
            connection: name.into(),
            iface: device.clone(),
            device,
            password,
        };
        if is_active {
            break;
        }
    }
    if info.device.is_empty() {
        if let Some(dev) = wifi_device_for_hotspot() {
            info.iface = dev.clone();
            info.device = dev;
        }
    }
    info
}

/// Start a Wi-Fi hotspot via NetworkManager (W396).
pub fn hotspot_start() -> Result<HotspotInfo, String> {
    let current = hotspot_status();
    if current.active {
        return Ok(current);
    }
    let ifname = wifi_device_for_hotspot().ok_or_else(|| {
        "No Wi-Fi device for hotspot (nmcli device status)".to_string()
    })?;
    let password = if current.password.len() >= 8 {
        current.password.clone()
    } else {
        generate_hotspot_password()
    };
    let ssid = if current.ssid.is_empty() {
        HOTSPOT_SSID.to_string()
    } else {
        current.ssid.clone()
    };
    // Prefer bringing an existing Hotspot profile up; else create via wifi hotspot.
    if !current.connection.is_empty() {
        match run_ok("nmcli", &["connection", "up", "id", &current.connection]) {
            Ok(()) => {
                let mut info = hotspot_status();
                if info.password.is_empty() {
                    info.password = password;
                }
                return Ok(info);
            }
            Err(_) => { /* fall through to create */ }
        }
    }
    run_ok(
        "nmcli",
        &[
            "device",
            "wifi",
            "hotspot",
            "ifname",
            &ifname,
            "ssid",
            &ssid,
            "password",
            &password,
        ],
    )?;
    let mut info = hotspot_status();
    if info.password.is_empty() {
        info.password = password;
    }
    if info.ssid.is_empty() {
        info.ssid = ssid;
    }
    if info.device.is_empty() {
        info.device = ifname;
    }
    info.active = true;
    Ok(info)
}

/// Stop the active hotspot / shared AP connection (W396).
pub fn hotspot_stop() -> Result<(), String> {
    let info = hotspot_status();
    if !info.active && info.connection.is_empty() {
        return Err("No hotspot connection".into());
    }
    let name = if info.connection.is_empty() {
        "Hotspot".to_string()
    } else {
        info.connection
    };
    // Down first; leave the profile so Start can reuse SSID/password.
    run_ok("nmcli", &["connection", "down", "id", &name]).or_else(|_| {
        run_ok("nmcli", &["connection", "down", &name])
    })?;
    Ok(())
}

fn wifi_device_for_hotspot() -> Option<String> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device", "status"])
        .output()
    else {
        return None;
    };
    if !out.status.success() {
        return None;
    }
    let mut connected = None;
    let mut disconnected = None;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(3, ':');
        let device = parts.next().unwrap_or("").trim();
        let kind = parts.next().unwrap_or("").trim();
        let state = parts.next().unwrap_or("").trim();
        if device.is_empty() || kind != "wifi" {
            continue;
        }
        if state.starts_with("disconnected") {
            if disconnected.is_none() {
                disconnected = Some(device.to_string());
            }
        } else if connected.is_none() {
            connected = Some(device.to_string());
        }
    }
    // Prefer a free radio so the client uplink can stay up when a second NIC exists.
    disconnected.or(connected)
}

fn generate_hotspot_password() -> String {
    use std::io::Read;
    let mut buf = [0u8; 12];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((t >> (i * 8)) as u8).wrapping_add(i as u8);
        }
    }
    const CHARS: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    buf.iter()
        .map(|b| CHARS[(*b as usize) % CHARS.len()] as char)
        .collect()
}

fn active_connection_names() -> Vec<String> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show", "--active"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Best-effort hibernate via systemctl (W153).
pub fn hibernate() -> String {
    match Command::new("systemctl").arg("hibernate").status() {
        Ok(s) if s.success() => "Hibernate".into(),
        _ => "Hibernate · not available".into(),
    }
}

fn nm_wifi_enabled() -> Result<bool, String> {
    let out = run_stdout("nmcli", &["-t", "-f", "WIFI", "radio"])?;
    Ok(out.trim().eq_ignore_ascii_case("enabled"))
}

fn nm_airplane() -> Result<bool, String> {
    // Approximate: both wifi and wwan off
    let out = run_stdout("nmcli", &["-t", "radio"])?;
    Ok(out.lines().all(|l| l.contains(":disabled") || l.contains(":absent")))
}

fn bt_powered() -> Result<bool, String> {
    let out = run_stdout("bluetoothctl", &["show"])?;
    Ok(out.lines().any(|l| l.contains("Powered: yes")))
}

/// True when `playerctl` is on PATH (W49).
pub fn playerctl_available() -> bool {
    Command::new("playerctl")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[derive(Debug, Clone, Default)]
pub struct MediaStatus {
    pub title: String,
    pub album: String,
    pub playing: bool,
    /// 0.0–1.0 position fraction when known.
    pub progress: f32,
    pub art_url: String,
}

/// Best-effort Now Playing via playerctl (W49 / A055).
pub fn media_status() -> MediaStatus {
    let title = run_stdout(
        "playerctl",
        &["metadata", "--format", "{{artist}} — {{title}}"],
    )
    .ok()
    .map(|s| s.trim().chars().take(48).collect::<String>())
    .filter(|s| !s.is_empty() && s != " — ")
    .unwrap_or_else(|| "Not playing".into());
    let album = run_stdout("playerctl", &["metadata", "xesam:album"])
        .ok()
        .map(|s| s.trim().chars().take(40).collect::<String>())
        .unwrap_or_default();
    let art_url = run_stdout("playerctl", &["metadata", "mpris:artUrl"])
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let playing = run_stdout("playerctl", &["status"])
        .map(|s| s.trim().eq_ignore_ascii_case("Playing"))
        .unwrap_or(false);
    let pos = run_stdout("playerctl", &["position"])
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    let len = run_stdout("playerctl", &["metadata", "mpris:length"])
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    // mpris:length is microseconds.
    let progress = if len > 0.0 {
        ((pos * 1_000_000.0) / len).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    MediaStatus {
        title,
        album,
        playing,
        progress,
        art_url,
    }
}

pub fn media_play_pause() -> Result<(), String> {
    run_ok("playerctl", &["play-pause"])
}

pub fn media_previous() -> Result<(), String> {
    run_ok("playerctl", &["previous"])
}

pub fn media_next() -> Result<(), String> {
    run_ok("playerctl", &["next"])
}

fn run_ok(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .status()
        .map_err(|e| format!("{bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} {:?} failed ({status})", args))
    }
}

/// Push the platform Mackes icon theme + color-scheme for GTK apps.
pub fn apply_mackes_icon_theme(dark_ui: bool) {
    let icons = mde_theme::icons::theme_name_for_ui(dark_ui);
    let scheme = if dark_ui {
        "prefer-dark"
    } else {
        "prefer-light"
    };
    let _ = run_ok(
        "gsettings",
        &["set", "org.gnome.desktop.interface", "icon-theme", icons],
    );
    let _ = run_ok(
        "gsettings",
        &["set", "org.gnome.desktop.interface", "color-scheme", scheme],
    );
}

fn run_stdout(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("{bin}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{bin} {:?} failed", args));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_default() {
        let _ = RadioState::default();
    }

    #[test]
    fn app_streams_skip_channel_leaves() {
        let text = r#"
Audio
 ├─ Sinks:
 │  *   49. Built-in Audio Analog Stereo        [vol: 0.45]
 └─ Streams:
      80. Firefox
           81. output_FL
      90. Music Player
Video
 └─ Streams:
      99. Camera
"#;
        let streams = parse_app_streams(text);
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].id, 80);
        assert_eq!(streams[0].label, "Firefox");
        assert_eq!(streams[1].id, 90);
    }
}

/// A037: Bluetooth device for QS flyout.
#[derive(Debug, Clone)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

pub fn bt_device_list() -> Vec<BtDevice> {
    bt_parse_devices(&["devices", "Paired"])
}

pub fn bt_device_list_discovered() -> Vec<BtDevice> {
    let mut paired = bt_device_list();
    let discovered = bt_parse_devices(&["devices"]);
    for d in discovered {
        if !paired.iter().any(|p| p.mac == d.mac) {
            paired.push(d);
        }
    }
    paired
}

fn bt_parse_devices(args: &[&str]) -> Vec<BtDevice> {
    let Ok(out) = Command::new("bluetoothctl").args(args).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let connected = bt_connected_macs();
    let mut list = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        // Device AA:BB:CC:DD:EE:FF Name
        let Some(rest) = line.strip_prefix("Device ") else {
            continue;
        };
        let mut parts = rest.splitn(2, ' ');
        let mac = parts.next().unwrap_or("").trim().to_string();
        let name = parts.next().unwrap_or(mac.as_str()).trim().to_string();
        if mac.is_empty() {
            continue;
        }
        list.push(BtDevice {
            connected: connected.iter().any(|c| c.eq_ignore_ascii_case(&mac)),
            paired: true,
            mac,
            name,
        });
        if list.len() >= 16 {
            break;
        }
    }
    list
}

fn bt_connected_macs() -> Vec<String> {
    let Ok(out) = Command::new("bluetoothctl").args(["devices", "Connected"]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("Device "))
        .filter_map(|rest| rest.split_whitespace().next().map(|s| s.to_string()))
        .collect()
}

pub fn bt_connect_mac(mac: &str) -> Result<(), String> {
    run_ok("bluetoothctl", &["connect", mac.trim()])
}

pub fn bt_disconnect_mac(mac: &str) -> Result<(), String> {
    run_ok("bluetoothctl", &["disconnect", mac.trim()])
}

pub fn bt_scan_on() -> Result<(), String> {
    let _ = Command::new("bluetoothctl").args(["scan", "on"]).spawn();
    Ok(())
}

pub fn bt_pair_mac(mac: &str) -> Result<(), String> {
    run_ok("bluetoothctl", &["pair", mac.trim()])?;
    let _ = run_ok("bluetoothctl", &["trust", mac.trim()]);
    run_ok("bluetoothctl", &["connect", mac.trim()])
}

pub fn wifi_connect_password(ssid: &str, password: &str) -> Result<(), String> {
    let ssid = ssid.trim();
    let password = password.trim();
    if ssid.is_empty() {
        return Err("empty SSID".into());
    }
    if password.is_empty() {
        return Err("password required".into());
    }
    run_ok(
        "nmcli",
        &["dev", "wifi", "connect", ssid, "password", password],
    )
}

/// A039: backend availability probes for QS tiles.
pub fn hotspot_available() -> bool {
    which("nmcli")
}

pub fn vpn_available() -> bool {
    !vpn_list().is_empty() || which("nmcli")
}

pub fn cast_available() -> bool {
    which("xdg-desktop-portal")
        || std::path::Path::new("/usr/libexec/xdg-desktop-portal").exists()
        || which("ashpd")
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A089: best-effort time remaining from upower or sysfs.
pub fn battery_time_remaining() -> Option<String> {
    if let Ok(o) = Command::new("upower")
        .args(["-i", "/org/freedesktop/UPower/devices/battery_BAT0"])
        .output()
    {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with("time to empty") {
                let rest = l.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
                if !rest.is_empty() && !rest.eq_ignore_ascii_case("unknown") {
                    return Some(rest.to_string());
                }
            }
            if l.starts_with("time to full") {
                let rest = l.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
                if !rest.is_empty() {
                    return Some(format!("Full in {rest}"));
                }
            }
        }
    }
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for ent in dir.flatten() {
        let path = ent.path();
        let typ = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if !typ.trim().eq_ignore_ascii_case("Battery") {
            continue;
        }
        let status = std::fs::read_to_string(path.join("status")).unwrap_or_default();
        let key = if status.trim().eq_ignore_ascii_case("Charging") {
            "charge_full_design"
        } else {
            "energy_now"
        };
        let _ = key;
        if let Ok(rem) = std::fs::read_to_string(path.join("energy_full")) {
            let _ = rem;
        }
        if let (Ok(now), Ok(rate)) = (
            std::fs::read_to_string(path.join("energy_now")),
            std::fs::read_to_string(path.join("power_now")),
        ) {
            let now = now.trim().parse::<f64>().ok()?;
            let rate = rate.trim().parse::<f64>().ok()?;
            if rate > 0.0 && now > 0.0 {
                let mins = (now / rate * 60.0).round() as u64;
                let h = mins / 60;
                let m = mins % 60;
                return Some(if h > 0 {
                    format!("{h} hr {m} min remaining")
                } else {
                    format!("{m} min remaining")
                });
            }
        }
    }
    None
}

/// A092: mic/camera/location in use (best-effort).
pub fn privacy_in_use() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(o) = Command::new("pw-cli").args(["ls", "Node"]).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            let l = line.to_lowercase();
            if l.contains("audio") && (l.contains("capture") || l.contains("input")) {
                out.push(("Microphone".into(), "PipeWire capture active".into()));
                break;
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir("/dev") {
        for ent in rd.flatten() {
            let n = ent.file_name().to_string_lossy().into_owned();
            if n.starts_with("video") {
                if let Ok(o) = Command::new("fuser").arg(ent.path()).output() {
                    let text = String::from_utf8_lossy(&o.stdout);
                    if !text.trim().is_empty() {
                        out.push(("Camera".into(), format!("/dev/{n} in use")));
                        break;
                    }
                }
            }
        }
    }
    if which("geoclue") || which("geoclue2") {
        if let Ok(o) = Command::new("busctl")
            .args(["--user", "tree", "org.freedesktop.GeoClue2"])
            .output()
        {
            if o.status.success() {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.contains("Client") {
                    out.push(("Location".into(), "GeoClue client active".into()));
                }
            }
        }
    }
    out
}

/// A095: tablet posture heuristic (chassis type / env).
pub fn tablet_posture_active(tablet_mode: &str) -> bool {
    match tablet_mode.trim().to_lowercase().as_str() {
        "on" => true,
        "off" => false,
        _ => {
            if std::env::var("MDE_TABLET").map(|v| v == "1").unwrap_or(false) {
                return true;
            }
            if let Ok(t) = std::fs::read_to_string("/sys/class/dmi/id/chassis_type") {
                if let Ok(n) = t.trim().parse::<u8>() {
                    return matches!(n, 30 | 31 | 32 | 33);
                }
            }
            let inputs = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
            inputs.contains("Tablet") || (inputs.contains("Touchscreen") && !inputs.contains("AT Translated"))
        }
    }
}

/// A098: format IANA TZ via `date` (no extra crates).
pub fn world_clock_time(tz: &str) -> Option<String> {
    let tz = tz.trim();
    if tz.is_empty() {
        return None;
    }
    let out = Command::new("date")
        .env("TZ", tz)
        .args(["+%H:%M"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}
