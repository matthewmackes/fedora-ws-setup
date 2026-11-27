//! System probes/actions shared with Settings pages (NM / BlueZ / wpctl).

use std::process::{Command, Stdio};

pub fn set_wifi(on: bool) -> Result<(), String> {
    // Turning Wi-Fi on clears airplane (nmcli radio all was off) (W58).
    if on {
        let _ = run("nmcli", &["radio", "all", "on"]);
    }
    run("nmcli", &["radio", "wifi", if on { "on" } else { "off" }])
}

pub fn set_bluetooth(on: bool) -> Result<(), String> {
    run(
        "bluetoothctl",
        &["power", if on { "on" } else { "off" }],
    )
}

pub fn set_airplane(on: bool) -> Result<(), String> {
    if on {
        let _ = set_wifi(false);
        let _ = set_bluetooth(false);
        run("nmcli", &["radio", "all", "off"])
    } else {
        run("nmcli", &["radio", "all", "on"])
    }
}

pub fn volume_summary() -> String {
    match Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().into(),
        _ => "WirePlumber / PipeWire not available".into(),
    }
}

pub fn volume_adjust(delta_pct: i32) -> Result<String, String> {
    let arg = if delta_pct >= 0 {
        format!("{delta_pct}%+")
    } else {
        format!("{}%-", delta_pct.unsigned_abs())
    };
    let status = Command::new("wpctl")
        .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &arg])
        .status()
        .map_err(|e| format!("wpctl: {e}"))?;
    if !status.success() {
        return Err(format!("wpctl set-volume failed ({status})"));
    }
    Ok(volume_summary())
}

pub fn volume_toggle_mute() -> Result<String, String> {
    let status = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .status()
        .map_err(|e| format!("wpctl: {e}"))?;
    if !status.success() {
        return Err(format!("wpctl set-mute failed ({status})"));
    }
    Ok(volume_summary())
}

pub fn wifi_summary() -> String {
    match Command::new("nmcli")
        .args(["-t", "-f", "WIFI", "radio"])
        .output()
    {
        Ok(o) if o.status.success() => format!("nmcli wifi: {}", String::from_utf8_lossy(&o.stdout).trim()),
        _ => "NetworkManager not available".into(),
    }
}

pub fn bt_summary() -> String {
    match Command::new("bluetoothctl").args(["show"]).output() {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.contains("Powered: yes") {
                "BlueZ: powered".into()
            } else {
                "BlueZ: off or no adapter".into()
            }
        }
        _ => "BlueZ not available".into(),
    }
}

pub fn wifi_networks() -> Vec<String> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "SSID,SIGNAL", "dev", "wifi", "list"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let mut nets = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.split(':');
        let ssid = parts.next().unwrap_or("").trim();
        if ssid.is_empty() || nets.iter().any(|s: &String| s.starts_with(ssid)) {
            continue;
        }
        let signal = parts.next().unwrap_or("");
        nets.push(format!("{ssid} ({signal}%)"));
        if nets.len() >= 12 {
            break;
        }
    }
    nets
}

pub fn wifi_connect(label: &str) -> Result<(), String> {
    let ssid = label.split(" (").next().unwrap_or(label).trim();
    if ssid.is_empty() {
        return Err("empty SSID".into());
    }
    run("nmcli", &["dev", "wifi", "connect", ssid])
}

pub fn bt_devices() -> Vec<String> {
    let Ok(out) = Command::new("bluetoothctl").arg("devices").output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| l.starts_with("Device "))
        .take(12)
        .collect()
}

pub fn bt_connect(line: &str) -> Result<(), String> {
    let mac = line.split_whitespace().nth(1).unwrap_or("");
    if mac.is_empty() {
        return Err("no device address".into());
    }
    run("bluetoothctl", &["connect", mac])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSink {
    pub id: u32,
    pub label: String,
    pub default: bool,
}

/// One `wpctl status` sink line, including the tree drawing characters.
pub fn parse_sink_line(line: &str) -> Option<AudioSink> {
    let default = line.contains('*');
    let rest = line.trim_start_matches(|c: char| !c.is_ascii_digit());
    let (num, after) = rest.split_once('.')?;
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let id = num.parse().ok()?;
    let label = after.trim();
    if label.is_empty() {
        return None;
    }
    Some(AudioSink {
        id,
        label: label.to_string(),
        default,
    })
}

pub fn set_default_sink(id: u32) -> Result<(), String> {
    let status = Command::new("wpctl")
        .args(["set-default", &id.to_string()])
        .status()
        .map_err(|e| format!("wpctl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wpctl set-default failed ({status})"))
    }
}

pub fn node_volume(id: u32, delta_pct: i32) -> Result<(), String> {
    let step = if delta_pct >= 0 {
        format!("{delta_pct}%+")
    } else {
        format!("{}%-", delta_pct.abs())
    };
    let status = Command::new("wpctl")
        .args(["set-volume", &id.to_string(), &step])
        .status()
        .map_err(|e| format!("wpctl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wpctl set-volume failed ({status})"))
    }
}

pub fn node_mute(id: u32) -> Result<(), String> {
    let status = Command::new("wpctl")
        .args(["set-mute", &id.to_string(), "toggle"])
        .status()
        .map_err(|e| format!("wpctl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wpctl set-mute failed ({status})"))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioStatus {
    pub sinks: Vec<AudioSink>,
    pub sources: Vec<AudioSink>,
    pub streams: Vec<AudioSink>,
}

pub fn audio_status() -> AudioStatus {
    let Ok(out) = Command::new("wpctl").arg("status").output() else {
        return AudioStatus::default();
    };
    if !out.status.success() {
        return AudioStatus::default();
    }
    parse_audio_status(&String::from_utf8_lossy(&out.stdout))
}

/// Audio sinks, sources, and streams from one `wpctl status` transcript.
/// The Video section is ignored, so a camera source is not treated as a microphone.
pub fn parse_audio_status(text: &str) -> AudioStatus {
    let mut status = AudioStatus::default();
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
        if video || section.is_empty() {
            continue;
        }
        let Some(node) = parse_sink_line(line) else {
            continue;
        };
        let bucket = match section {
            "sinks" => &mut status.sinks,
            "sources" => &mut status.sources,
            "streams" => &mut status.streams,
            _ => continue,
        };
        if bucket.len() < 8 {
            bucket.push(node);
        }
    }
    status
}

fn audio_header(line: &str) -> Option<&'static str> {
    if parse_sink_line(line).is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_line_reads_id_and_default() {
        let current = parse_sink_line(" │  *   58. Analog Stereo").expect("default");
        assert_eq!(current.id, 58);
        assert!(current.default);
        assert_eq!(current.label, "Analog Stereo");
        let other = parse_sink_line(" │      62. HDMI").expect("hdmi");
        assert_eq!(other.id, 62);
        assert!(!other.default);
        assert!(parse_sink_line("Sources:").is_none());
    }

    #[test]
    fn audio_status_keeps_mic_and_streams_out_of_video() {
        let text = r#"
 ├─ Sinks:
 │  *   49. Built-in Audio Analog Stereo        [vol: 0.45]
 ├─ Sources:
 │  *   44. Built-in Audio Analog Stereo        [vol: 1.00]
 ├─ Filters:
 └─ Streams:
      80. Video Player
           81. output_FL
Video
 ├─ Sources:
 │  *   55. Integrated Camera
"#;
        let audio = parse_audio_status(text);
        assert_eq!(audio.sinks.len(), 1);
        assert_eq!(audio.sinks[0].id, 49);
        assert!(audio.sinks[0].default);
        assert_eq!(audio.sources.len(), 1);
        assert_eq!(audio.sources[0].id, 44);
        assert_eq!(audio.streams.len(), 2);
        assert_eq!(audio.streams[0].label, "Video Player");
        assert_eq!(audio.streams[1].id, 81);
    }
}

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

pub fn outputs_summary() -> String {
    capture("mde-comp", &["outputs"])
}

#[derive(Clone, Debug)]
pub struct LayoutHead {
    pub name: String,
    pub x_text: String,
    pub y_text: String,
    pub scale: f64,
    pub detail: String,
    /// A062: mode string e.g. `1920x1080@60`.
    pub mode: String,
    pub refresh_hz: f64,
    pub hdr: bool,
}

/// Connected outputs. Saved profile `layout` supplies x/y/scale when present.
pub fn layout_heads(cfg: &mde_config::MdeConfig) -> Vec<LayoutHead> {
    let saved = cfg
        .active()
        .displays
        .profiles
        .iter()
        .find(|p| p.name == "layout");
    let mut heads = read_live_outputs();
    if heads.is_empty() {
        if let Some(profile) = saved {
            for out in &profile.outputs {
                heads.push(LayoutHead {
                    name: out.name.clone(),
                    x_text: out.x.to_string(),
                    y_text: out.y.to_string(),
                    scale: out.scale,
                    detail: if out.mode.is_empty() {
                        "saved layout".into()
                    } else {
                        out.mode.clone()
                    },
                    mode: out.mode.clone(),
                    refresh_hz: out.refresh_hz,
                    hdr: out.hdr,
                });
            }
        }
        return heads;
    }
    if let Some(profile) = saved {
        for head in &mut heads {
            if let Some(out) = profile.outputs.iter().find(|o| o.name == head.name) {
                head.x_text = out.x.to_string();
                head.y_text = out.y.to_string();
                head.scale = out.scale;
                if !out.mode.is_empty() {
                    head.mode = out.mode.clone();
                }
                if out.refresh_hz > 0.0 {
                    head.refresh_hz = out.refresh_hz;
                }
                head.hdr = out.hdr;
            }
        }
    }
    heads
}

pub fn write_layout(heads: &[LayoutHead]) -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let mut body = String::new();
    for head in heads {
        let x: i32 = head
            .x_text
            .trim()
            .parse()
            .map_err(|_| format!("bad x for {}", head.name))?;
        let y: i32 = head
            .y_text
            .trim()
            .parse()
            .map_err(|_| format!("bad y for {}", head.name))?;
        body.push_str(&format!("{} {x} {y} {} 1\n", head.name, head.scale));
    }
    std::fs::write(
        std::path::Path::new(&runtime).join("mde-output-layout.txt"),
        body,
    )
    .map_err(|e| format!("layout file: {e}"))
}

fn read_live_outputs() -> Vec<LayoutHead> {
    if let Some(heads) = read_output_snapshot() {
        if !heads.is_empty() {
            return heads;
        }
    }
    scan_drm()
}

fn read_output_snapshot() -> Option<Vec<LayoutHead>> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let text = std::fs::read_to_string(std::path::Path::new(&runtime).join("mde-outputs.txt")).ok()?;
    let mut heads = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let Some(x) = parts.next().and_then(|s| s.parse::<i32>().ok()) else { continue };
        let Some(y) = parts.next().and_then(|s| s.parse::<i32>().ok()) else { continue };
        let Some(w) = parts.next().and_then(|s| s.parse::<i32>().ok()) else { continue };
        let Some(h) = parts.next().and_then(|s| s.parse::<i32>().ok()) else { continue };
        let Some(scale) = parts.next().and_then(|s| s.parse::<f64>().ok()) else { continue };
        heads.push(LayoutHead {
            name: name.to_string(),
            x_text: x.to_string(),
            y_text: y.to_string(),
            scale,
            detail: format!("{w}×{h} at {x},{y} · scale {scale}"),
            mode: format!("{w}x{h}"),
            refresh_hz: 0.0,
            hdr: false,
        });
    }
    Some(heads)
}

fn scan_drm() -> Vec<LayoutHead> {
    let mut heads = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else {
        return heads;
    };
    for ent in rd.flatten() {
        let fname = ent.file_name();
        let fname = fname.to_string_lossy();
        if !fname.starts_with("card") {
            continue;
        }
        let Some((_, name)) = fname.split_once('-') else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let status = std::fs::read_to_string(ent.path().join("status")).unwrap_or_default();
        if !status.trim().eq_ignore_ascii_case("connected") {
            continue;
        }
        let modes = std::fs::read_to_string(ent.path().join("modes")).unwrap_or_default();
        let mode = modes.lines().next().unwrap_or("").trim();
        heads.push(LayoutHead {
            name: name.to_string(),
            x_text: "0".into(),
            y_text: "0".into(),
            scale: 1.0,
            detail: if mode.is_empty() {
                "connected".into()
            } else {
                format!("{mode} · connected")
            },
            mode: mode.to_string(),
            refresh_hz: 0.0,
            hdr: false,
        });
    }
    heads.sort_by(|a, b| a.name.cmp(&b.name));
    heads
}

pub fn disks() -> String {
    capture("lsblk", &["-o", "NAME,SIZE,TYPE,MOUNTPOINT"])
}

pub fn services() -> String {
    capture(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--state=running",
            "--no-pager",
            "--no-legend",
        ],
    )
}

pub fn set_default_source(id: u32) -> Result<(), String> {
    set_default_sink(id)
}

pub fn hostname() -> String {
    let static_name = capture("hostnamectl", &["hostname", "--static"])
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if !static_name.is_empty() && !static_name.contains("failed") {
        return static_name;
    }
    let name = capture("hostnamectl", &["hostname"])
        .lines()
        .next()
        .unwrap_or("localhost")
        .trim()
        .to_string();
    if name.is_empty() || name.contains("failed") {
        "localhost".into()
    } else {
        name
    }
}

pub fn pretty_hostname() -> String {
    let pretty = capture("hostnamectl", &["hostname", "--pretty"])
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if pretty.is_empty() || pretty.contains("failed") {
        hostname()
    } else {
        pretty
    }
}

/// Sanitize a pretty name into a static hostname (letters, digits, hyphens).
pub fn static_hostname_from(pretty: &str) -> String {
    let mut out = String::new();
    for ch in pretty.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch.is_whitespace() {
            if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
        }
    }
    let trimmed = out.trim_matches('-').chars().take(63).collect::<String>();
    if trimmed.is_empty() {
        "localhost".into()
    } else {
        trimmed
    }
}

pub fn set_hostname(pretty: &str) -> Result<(), String> {
    let pretty = pretty.trim();
    if pretty.is_empty() {
        return Err("Name cannot be empty".into());
    }
    let static_name = static_hostname_from(pretty);
    run("hostnamectl", &["hostname", &static_name, "--static"])?;
    let _ = run("hostnamectl", &["hostname", pretty, "--pretty"]);
    Ok(())
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn dmi(field: &str) -> String {
    read_file(&format!("/sys/class/dmi/id/{field}"))
        .trim()
        .to_string()
}

fn os_release_field(key: &str) -> String {
    for line in read_file("/etc/os-release").lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k == key {
                return v.trim().trim_matches('"').to_string();
            }
        }
    }
    String::new()
}

pub fn base_os() -> String {
    let pretty = os_release_field("PRETTY_NAME");
    if !pretty.is_empty() {
        return pretty;
    }
    let name = os_release_field("NAME");
    let version = os_release_field("VERSION");
    match (name.is_empty(), version.is_empty()) {
        (false, false) => format!("{name} {version}"),
        (false, true) => name,
        _ => "Fedora".into(),
    }
}

pub fn device_spec_rows() -> Vec<(String, String)> {
    let cpu = read_file("/proc/cpuinfo")
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| capture("uname", &["-p"]));
    let mem_kb = read_file("/proc/meminfo")
        .lines()
        .find(|l| l.starts_with("MemTotal:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(0.0);
    let mem = if mem_kb > 0.0 {
        format!("{:.1} GB", mem_kb / 1_048_576.0)
    } else {
        "Unknown".into()
    };
    let gpus: Vec<String> = capture("lspci", &[])
        .lines()
        .filter(|l| {
            let u = l.to_uppercase();
            u.contains("VGA") || u.contains("3D") || u.contains("DISPLAY")
        })
        .map(|l| {
            l.splitn(2, ':')
                .nth(1)
                .unwrap_or(l)
                .trim()
                .to_string()
        })
        .collect();
    let graphics = if gpus.is_empty() {
        "Unknown".into()
    } else {
        gpus.join("; ")
    };
    let storage = match std::fs::metadata("/") {
        Ok(_) => {
            // Prefer `df -B1 /` for a human-facing system-disk figure.
            let df = capture("df", &["-h", "--output=size,used,avail,pcent,target", "/"]);
            let line = df.lines().nth(1).unwrap_or("").trim();
            if line.is_empty() {
                "Unknown".into()
            } else {
                format!("{line} (system disk)")
            }
        }
        Err(_) => "Unknown".into(),
    };
    let manufacturer = dmi("sys_vendor");
    let model = {
        let family = dmi("product_family");
        let name = dmi("product_name");
        [family, name]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let arch = capture("uname", &["-m"]).trim().to_string();
    let system_type = if arch.contains("64") || arch == "x86_64" || arch == "aarch64" {
        format!("64-bit operating system, {arch} processor")
    } else {
        arch
    };
    vec![
        ("Device name".into(), hostname()),
        (
            "Manufacturer".into(),
            if manufacturer.is_empty() {
                "Unknown".into()
            } else {
                manufacturer
            },
        ),
        (
            "Model".into(),
            if model.is_empty() {
                "Unknown".into()
            } else {
                model
            },
        ),
        ("Processor".into(), cpu),
        ("Installed RAM".into(), mem),
        ("Graphics".into(), graphics),
        ("Storage".into(), storage),
        ("System type".into(), system_type),
    ]
}

pub fn mde_spec_rows() -> Vec<(String, String)> {
    let installed = capture(
        "rpm",
        &["-q", "--qf", "%{INSTALLTIME:day}", "filesystem"],
    )
    .trim()
    .to_string();
    let kernel = capture("uname", &["-r"]).trim().to_string();
    vec![
        ("Edition".into(), mde_theme::PRODUCT.to_string()),
        ("Version".into(), mde_theme::VERSION_FULL.to_string()),
        ("Base OS".into(), base_os()),
        (
            "Installed on".into(),
            if installed.is_empty() || installed.contains("failed") {
                "Unknown".into()
            } else {
                installed
            },
        ),
        ("Kernel".into(), kernel),
        ("Desktop".into(), mde_theme::DESKTOP_STACK.to_string()),
        (
            "Typography".into(),
            mde_theme::font_branding_for(
                &mde_config::MdeConfig::load()
                    .unwrap_or_default()
                    .active()
                    .theme,
            ),
        ),
        (
            "Default accent".into(),
            mde_theme::ACCENT_DEFAULT.to_string(),
        ),
    ]
}

pub fn about_specs() -> String {
    let mut lines = vec![
        format!("Device name: {}", hostname()),
        format!("Base OS: {}", base_os()),
        format!("Kernel: {}", capture("uname", &["-mr"]).trim()),
        mde_theme::about_line(),
        mde_theme::SUBTITLE.to_string(),
        mde_theme::TAGLINE.to_string(),
        mde_theme::DESKTOP_STACK.to_string(),
    ];
    lines.push("Licenses:".into());
    for (comp, lic) in mde_theme::licenses_at_play() {
        lines.push(format!("  {comp} — {lic}"));
    }
    lines.join("\n")
}

pub fn copy_about_specs() -> Result<(), String> {
    let text = about_specs();
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("wl-copy: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("wl-copy write: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("wl-copy: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wl-copy failed ({status})"))
    }
}

pub fn user_display_name() -> String {
    if let Ok(name) = std::env::var("USER") {
        if let Ok(out) = Command::new("getent").args(["passwd", &name]).output() {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                if let Some(gecos) = line.split(':').nth(4) {
                    let real = gecos.split(',').next().unwrap_or("").trim();
                    if !real.is_empty() {
                        return real.to_string();
                    }
                }
            }
        }
        return name;
    }
    "User".into()
}

pub fn username() -> String {
    std::env::var("USER").unwrap_or_else(|_| "user".into())
}

pub fn face_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::PathBuf::from(home).join(".face");
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn timedate_status() -> String {
    capture("timedatectl", &["status"])
}

pub fn set_ntp(on: bool) -> Result<(), String> {
    run(
        "timedatectl",
        &["set-ntp", if on { "true" } else { "false" }],
    )
}

pub fn localectl_status() -> String {
    capture("localectl", &["status"])
}

pub fn ethernet_summary() -> String {
    match Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "dev", "status"])
        .output()
    {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let wired: Vec<&str> = text
                .lines()
                .filter(|l| l.contains(":ethernet:") || l.contains(":wire:"))
                .collect();
            if wired.is_empty() {
                "No ethernet devices".into()
            } else {
                wired.join("\n")
            }
        }
        _ => "NetworkManager not available".into(),
    }
}

pub fn vpn_list() -> Vec<String> {
    let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("vpn") || l.contains("wireguard"))
        .map(|l| l.to_string())
        .take(12)
        .collect()
}

pub fn firewall_status() -> String {
    let fw = capture("firewall-cmd", &["--state"]);
    if fw.contains("running") || fw.contains("not running") {
        let zone = capture("firewall-cmd", &["--get-default-zone"]);
        format!("firewalld: {}\nDefault zone: {}", fw.trim(), zone.trim())
    } else {
        let ufw = capture("ufw", &["status"]);
        if ufw.contains("Status:") {
            ufw
        } else {
            "No firewalld/ufw status available".into()
        }
    }
}

pub fn selinux_status() -> String {
    let s = capture("getenforce", &[]);
    if s.contains("failed") || s.contains(":") {
        "SELinux tools not available".into()
    } else {
        format!("SELinux: {}", s.trim())
    }
}

pub fn gamemode_status() -> String {
    if which("gamemoded") {
        let enabled = capture("systemctl", &["--user", "is-enabled", "gamemoded"]);
        let status = capture("gamemoded", &["-s"]);
        format!(
            "Enabled: {}\n{}",
            enabled.trim(),
            status.trim().to_string()
        )
    } else {
        "Game Mode needs the gamemode package.".into()
    }
}

pub fn set_gamemode(on: bool) -> Result<(), String> {
    if on {
        let _ = run("systemctl", &["--user", "unmask", "gamemoded"]);
        run("systemctl", &["--user", "start", "gamemoded"])
    } else {
        let _ = run("systemctl", &["--user", "stop", "gamemoded"]);
        run("systemctl", &["--user", "mask", "gamemoded"])
    }
}

pub fn gsettings_get(schema: &str, key: &str) -> String {
    capture("gsettings", &["get", schema, key])
        .trim()
        .trim_matches('\'')
        .to_string()
}

pub fn gsettings_set(schema: &str, key: &str, value: &str) -> Result<(), String> {
    run("gsettings", &["set", schema, key, value])
}

fn xfconf_get_bool(channel: &str, prop: &str) -> bool {
    let out = capture("xfconf-query", &["-c", channel, "-p", prop]);
    out.trim().eq_ignore_ascii_case("true")
}

fn xfconf_set_bool(channel: &str, prop: &str, value: bool) -> Result<(), String> {
    let v = if value { "true" } else { "false" };
    // Create property if missing.
    let _ = run(
        "xfconf-query",
        &["-c", channel, "-p", prop, "-n", "-t", "bool", "-s", v],
    );
    run(
        "xfconf-query",
        &["-c", channel, "-p", prop, "-s", v],
    )
}

/// A045: Thunar hidden files + GTK file-chooser hidden.
pub fn show_hidden_files() -> bool {
    if which("xfconf-query") {
        return xfconf_get_bool("thunar", "/misc-show-hidden-files");
    }
    gsettings_get("org.gtk.Settings.FileChooser", "show-hidden") == "true"
}

pub fn set_show_hidden_files(on: bool) -> Result<(), String> {
    let mut ok = false;
    if which("xfconf-query") {
        xfconf_set_bool("thunar", "/misc-show-hidden-files", on)?;
        ok = true;
    }
    let _ = gsettings_set(
        "org.gtk.Settings.FileChooser",
        "show-hidden",
        if on { "true" } else { "false" },
    );
    if ok || which("gsettings") {
        Ok(())
    } else {
        Err("No Thunar xfconf or gsettings available".into())
    }
}

/// A045: prefer showing extensions (GTK file chooser; Thunar shows them by default).
pub fn show_file_extensions() -> bool {
    // Invert of "hide extensions" style prefs when present.
    let gtk = gsettings_get("org.gtk.gtk4.Settings.FileChooser", "show-type-column");
    if gtk == "true" || gtk == "false" {
        return gtk == "true";
    }
    // Stored in MDE extras when FM has no native knob — default true for community.
    true
}

pub fn set_show_file_extensions(on: bool) -> Result<(), String> {
    let _ = gsettings_set(
        "org.gtk.Settings.FileChooser",
        "show-type-column",
        if on { "true" } else { "false" },
    );
    // Thunar: ensure display name includes extension (default behavior).
    Ok(())
}

pub fn update_check_summary() -> String {
    update_check_result().unwrap_or_else(|e| e)
}

pub fn update_check_result() -> Result<String, String> {
    let pk = capture("pkcon", &["get-updates", "--plain"]);
    if pk.contains("failed") || pk.contains("pkcon:") {
        let dnf = capture("dnf", &["check-update", "-q"]);
        if dnf.trim().is_empty() || dnf.contains("failed") {
            Err("Could not query updates (pkcon/dnf).".into())
        } else if dnf.contains("Error") {
            Err(dnf)
        } else {
            Ok(format!("dnf check-update:\n{dnf}"))
        }
    } else {
        let lines: Vec<&str> = pk.lines().take(20).collect();
        let body = lines.join("\n");
        if body.trim().is_empty() {
            Ok("You're up to date".into())
        } else {
            Ok(body)
        }
    }
}

pub fn update_history() -> String {
    capture(
        "dnf",
        &[
            "history",
            "list",
            "--reverse",
            "-n",
            "12",
        ],
    )
}

pub fn flatpak_update_history() -> String {
    let out = capture("flatpak", &["history", "--columns=time,change,application"]);
    if out.contains("failed") || out.trim().is_empty() {
        String::new()
    } else {
        out.lines()
            .filter(|l| {
                let t = l.to_lowercase();
                t.contains("update") || t.contains("upgrade") || t.contains("install")
            })
            .take(12)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn offline_update_pending() -> bool {
    std::path::Path::new("/system-update").exists()
}

fn updates_state_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let state = std::env::var("XDG_STATE_HOME").unwrap_or_else(|_| format!("{home}/.local/state"));
    std::path::PathBuf::from(state).join("ws-settings/updates.json")
}

pub fn updates_busy() -> bool {
    std::fs::read_to_string(updates_state_path())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("busy").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

pub fn updates_busy_status() -> String {
    std::fs::read_to_string(updates_state_path())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| {
            v.get("busy_status")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Working on updates…".into())
}

pub fn trigger_offline_update(mode: &str) -> String {
    let status = Command::new("mde-privileged")
        .args(["offline-trigger", mode])
        .status();
    match status {
        Ok(s) if s.success() => {
            let _ = Command::new("mde-privileged")
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

pub fn power_reboot() -> String {
    let _ = Command::new("mde-privileged")
        .args(["power", "reboot"])
        .spawn();
    "Restarting…".into()
}

pub fn cancel_offline_update() -> String {
    let out = capture("pkcon", &["offline-cancel"]);
    if out.contains("failed") {
        format!("Couldn’t cancel update: {out}")
    } else {
        "Update cancelled".into()
    }
}

pub fn schedule_restart_tonight() -> String {
    // Today/tomorrow simple slot: 3:00 AM local (Power & battery idle tip).
    let out = Command::new("systemd-run")
        .args([
            "--user",
            "--unit=mde-update-scheduled-reboot",
            "--on-calendar=03:00",
            "mde-privileged",
            "power",
            "reboot",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            "Restart scheduled for 3:00 AM (Power & battery quiet hours stand-in)".into()
        }
        Ok(o) => format!(
            "Couldn’t schedule restart: {}",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => format!("Couldn’t schedule restart: {e}"),
    }
}

pub fn update_install_all() -> String {
    let dl = Command::new("pkcon")
        .args(["update", "--only-download", "-y", "--plain"])
        .status();
    match dl {
        Ok(s) if s.success() => {
            let _ = Command::new("pkcon").args(["offline-trigger"]).status();
            let _ = Command::new("flatpak")
                .args(["update", "-y", "--noninteractive"])
                .status();
            if offline_update_pending() {
                "Download finished · Restart required".into()
            } else {
                let inst = Command::new("pkcon")
                    .args(["update", "-y", "--plain"])
                    .status();
                if inst.map(|s| s.success()).unwrap_or(false) {
                    "Updates installed".into()
                } else {
                    "Some updates couldn’t be installed".into()
                }
            }
        }
        Ok(_) => "Couldn’t download updates".into(),
        Err(e) => format!("Download failed: {e}"),
    }
}

pub fn default_screenshot_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        let pictures = Command::new("xdg-user-dir")
            .arg("PICTURES")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{home}/Pictures"));
        format!("{pictures}/Screenshots")
    } else {
        "~/Pictures/Screenshots".into()
    }
}

pub fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    path.to_string()
}

/// Resolves configured captures folder (`shell.screenshot_dir`) or the default.
pub fn screenshot_dir(cfg: &mde_config::MdeConfig) -> String {
    let custom = cfg.active().shell.screenshot_dir.trim();
    if custom.is_empty() {
        default_screenshot_dir()
    } else {
        expand_home(custom)
    }
}

pub fn pick_file(title: &str) -> Option<String> {
    // zenity / kdialog file picker.
    if which("zenity") {
        let out = Command::new("zenity")
            .args(["--file-selection", "--title", title])
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    if which("kdialog") {
        let out = Command::new("kdialog")
            .args(["--getopenfilename", ".", title])
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

pub fn pick_directory(title: &str) -> Option<String> {
    let out = Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            &format!("--title={title}"),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn camera_devices() -> String {
    capture("ls", &["/dev/video0", "/dev/video1"])
}

pub fn open_path(path: &str) -> Result<(), String> {
    run("xdg-open", &[path])
}

pub fn list_users() -> String {
    capture(
        "getent",
        &["passwd"],
    )
    .lines()
    .filter_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let uid: u32 = parts.nth(1)?.parse().ok()?;
        if (1000..65534).contains(&uid) {
            Some(format!("{name} (uid {uid})"))
        } else {
            None
        }
    })
    .take(16)
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn startup_apps() -> Vec<String> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(std::path::PathBuf::from(home).join(".config/autostart"));
    }
    dirs.push(std::path::PathBuf::from("/etc/xdg/autostart"));
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    out.push(name.to_string());
                }
            }
            if out.len() >= 24 {
                return out;
            }
        }
    }
    out
}

pub fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn capture_journal() -> String {
    capture(
        "journalctl",
        &[
            "--user",
            "-n",
            "40",
            "--no-pager",
            "-o",
            "short-iso",
        ],
    )
}

pub fn battery_summary() -> String {
    let out = capture(
        "upower",
        &["-i", "/org/freedesktop/UPower/devices/DisplayDevice"],
    );
    if out.contains("failed") || out.contains("upower:") {
        return "No battery reported (desktop / upower unavailable).".into();
    }
    let mut pct = String::new();
    let mut state = String::new();
    for line in out.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("percentage:") {
            pct = rest.trim().to_string();
        }
        if let Some(rest) = t.strip_prefix("state:") {
            state = rest.trim().to_string();
        }
    }
    if pct.is_empty() && state.is_empty() {
        "Battery status unavailable.".into()
    } else {
        format!("{} · {}", if pct.is_empty() { "—" } else { &pct }, if state.is_empty() { "—" } else { &state })
    }
}

pub fn power_profile() -> String {
    let active = capture("powerprofilesctl", &["get"]).trim().to_string();
    if active.contains("failed") || active.contains("powerprofilesctl:") {
        "power-profiles-daemon not available".into()
    } else {
        active
    }
}

pub fn set_power_profile(profile: &str) -> Result<(), String> {
    run("powerprofilesctl", &["set", profile])
}

pub fn disk_root_usage() -> String {
    match Command::new("df")
        .args(["-h", "--output=size,used,avail,pcent,target", "/"])
        .output()
    {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .nth(1)
                .map(|l| {
                    let parts: Vec<_> = l.split_whitespace().collect();
                    if parts.len() >= 4 {
                        format!(
                            "Local Disk (/) · {} used of {} ({} free, {})",
                            parts[1],
                            parts[0],
                            parts[2],
                            parts[3]
                        )
                    } else {
                        l.trim().to_string()
                    }
                })
                .unwrap_or_else(|| "df: no data".into())
        }
        _ => "df unavailable".into(),
    }
}

pub fn printers() -> Vec<String> {
    let out = capture("lpstat", &["-a"]);
    if out.contains("failed") || out.contains("lpstat:") {
        return Vec::new();
    }
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .take(16)
        .collect()
}

pub fn mouse_natural_scroll() -> bool {
    let v = gsettings_get("org.gnome.desktop.peripherals.mouse", "natural-scroll");
    v == "true"
}

pub fn set_mouse_natural_scroll(on: bool) -> Result<(), String> {
    gsettings_set(
        "org.gnome.desktop.peripherals.mouse",
        "natural-scroll",
        if on { "true" } else { "false" },
    )
}

pub fn touchpad_tap() -> bool {
    let v = gsettings_get("org.gnome.desktop.peripherals.touchpad", "tap-to-click");
    v == "true"
}

pub fn set_touchpad_tap(on: bool) -> Result<(), String> {
    gsettings_set(
        "org.gnome.desktop.peripherals.touchpad",
        "tap-to-click",
        if on { "true" } else { "false" },
    )
}

pub fn touchpad_natural_scroll() -> bool {
    let v = gsettings_get("org.gnome.desktop.peripherals.touchpad", "natural-scroll");
    v == "true"
}

pub fn set_touchpad_natural_scroll(on: bool) -> Result<(), String> {
    gsettings_set(
        "org.gnome.desktop.peripherals.touchpad",
        "natural-scroll",
        if on { "true" } else { "false" },
    )
}

pub fn default_web_browser() -> String {
    let out = capture("xdg-settings", &["get", "default-web-browser"]);
    let t = out.trim();
    if t.is_empty() || t.contains("failed") || t.contains("xdg-settings:") {
        "Not set".into()
    } else {
        t.to_string()
    }
}

pub fn focus_until_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_STATE_HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".local/state"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    base.join("mde").join("focus-until")
}

pub fn focus_remaining_secs() -> Option<u64> {
    let path = focus_until_path();
    let text = std::fs::read_to_string(path).ok()?;
    let until: f64 = text.trim().parse().ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    if until > now {
        Some((until - now) as u64)
    } else {
        None
    }
}

pub fn start_focus_session(minutes: u64) -> Result<(), String> {
    let until = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs_f64()
        + (minutes as f64) * 60.0;
    let path = focus_until_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, format!("{until}")).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn end_focus_session() -> Result<(), String> {
    let path = focus_until_path();
    let _ = std::fs::write(&path, "0");
    Ok(())
}

fn capture(bin: &str, args: &[&str]) -> String {
    match Command::new(bin).args(args).output() {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut lines: Vec<&str> = text.lines().take(16).collect();
            if text.lines().count() > 16 {
                lines.push("…");
            }
            lines.join("\n")
        }
        Ok(_) => format!("{bin} failed"),
        Err(e) => format!("{bin}: {e}"),
    }
}

fn run(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .status()
        .map_err(|e| format!("{bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} failed ({status})"))
    }
}

/// A040: known tray ids from shell state file (if present).
pub fn tray_known_ids() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(home).join(".local/state/mde/tray-known.txt");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .take(64)
        .collect()
}

pub fn tray_set_hidden(cfg: &mut mde_config::MdeConfig, id: &str, hide: bool) {
    let mut hidden: Vec<String> = cfg
        .extras
        .get("tray.hidden")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if hide {
        if !hidden.iter().any(|h| h == id) {
            hidden.push(id.into());
        }
    } else {
        hidden.retain(|h| h != id);
    }
    let arr = hidden
        .into_iter()
        .map(toml::Value::String)
        .collect::<Vec<_>>();
    cfg.extras
        .insert("tray.hidden".into(), toml::Value::Array(arr));
    let _ = cfg.save();
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppNotifPrefsFile {
    #[serde(default)]
    apps: Vec<AppNotifPrefStored>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppNotifPrefStored {
    app: String,
    #[serde(default = "default_true_bool")]
    banners: bool,
    #[serde(default = "default_true_bool")]
    sound: bool,
    #[serde(default)]
    priority: bool,
}

fn default_true_bool() -> bool {
    true
}

fn app_notif_prefs_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".config/mde/app-notif-prefs.json")
}

pub fn load_app_notif_pref_rows() -> Vec<crate::AppNotifPrefRow> {
    let Ok(text) = std::fs::read_to_string(app_notif_prefs_path()) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_str::<AppNotifPrefsFile>(&text) else {
        return Vec::new();
    };
    file.apps
        .into_iter()
        .map(|a| crate::AppNotifPrefRow {
            app: a.app,
            banners: a.banners,
            sound: a.sound,
            priority: a.priority,
        })
        .collect()
}

pub fn set_app_notif_field(app: &str, field: &str, value: bool) {
    let app = app.trim().to_lowercase();
    if app.is_empty() {
        return;
    }
    let path = app_notif_prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<AppNotifPrefsFile>(&t).ok())
        .unwrap_or(AppNotifPrefsFile { apps: Vec::new() });
    if let Some(existing) = file.apps.iter_mut().find(|a| a.app == app) {
        match field {
            "banners" => existing.banners = value,
            "sound" => existing.sound = value,
            "priority" => existing.priority = value,
            _ => {}
        }
    } else {
        let mut row = AppNotifPrefStored {
            app: app.clone(),
            banners: true,
            sound: true,
            priority: false,
        };
        match field {
            "banners" => row.banners = value,
            "sound" => row.sound = value,
            "priority" => row.priority = value,
            _ => {}
        }
        file.apps.push(row);
    }
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
}

pub fn launch_workspace(cfg: &mde_config::MdeConfig, name: &str) -> String {
    let Some(ws) = cfg
        .active()
        .tools
        .workspaces
        .iter()
        .find(|w| w.name == name)
    else {
        return format!("Workspace not found · {name}");
    };
    let mut launched = 0usize;
    for app in &ws.apps {
        let id = app.id.trim();
        if id.is_empty() {
            continue;
        }
        let ok = if id.ends_with(".desktop") {
            Command::new("gtk-launch").arg(id).spawn().is_ok()
        } else {
            Command::new("sh")
                .args(["-c", id])
                .spawn()
                .is_ok()
        };
        if ok {
            launched += 1;
        }
    }
    format!("Workspace · {name} · launched {launched}")
}

pub fn publish_key_remaps(cfg: &mde_config::MdeConfig) {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(runtime).join("mde-key-remaps.txt");
    let mut body = String::new();
    for r in &cfg.active().tools.keyboard_remaps {
        body.push_str(&format!("{}={}\n", r.from.trim(), r.to.trim()));
    }
    let _ = std::fs::write(path, body);
}

/// A094: publish touchpad gesture map for compositor / shell consumers.
pub fn publish_gestures(cfg: &mde_config::MdeConfig) {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(runtime).join("mde-gestures.toml");
    let g = &cfg.active().gestures;
    let body = format!(
        "three_up = \"{}\"\nthree_down = \"{}\"\nthree_left = \"{}\"\nthree_right = \"{}\"\nfour_up = \"{}\"\nfour_tap = \"{}\"\n",
        g.three_up, g.three_down, g.three_left, g.three_right, g.four_up, g.four_tap
    );
    let _ = std::fs::write(path, body);
}

/// A071: push high-contrast / normal GTK + icon themes so apps follow shell packs.
///
/// Normal light/dark always use the platform Mackes set (dark glyphs on light UI,
/// light glyphs on dark UI). High-contrast packs keep the a11y themes.
pub fn apply_contrast_theme(mode: &str) -> String {
    let mode = mode.to_ascii_lowercase();
    let (gtk, icons, cursor) = match mode.as_str() {
        "hc-black" => ("HighContrastInverse", "HighContrastInverse", "Adwaita"),
        "hc-white" => ("HighContrast", "HighContrast", "Adwaita"),
        "dark" => (
            "Adwaita-dark",
            mde_theme::icons::theme_name_for_ui(true),
            "Adwaita",
        ),
        _ => (
            "Adwaita",
            mde_theme::icons::theme_name_for_ui(false),
            "Adwaita",
        ),
    };
    let mut notes = Vec::new();
    let scheme = match mode.as_str() {
        "dark" | "hc-black" => "prefer-dark",
        _ => "prefer-light",
    };
    for (schema, key, val) in [
        ("org.gnome.desktop.interface", "gtk-theme", gtk),
        ("org.gnome.desktop.interface", "icon-theme", icons),
        ("org.gnome.desktop.interface", "cursor-theme", cursor),
        ("org.gnome.desktop.interface", "color-scheme", scheme),
        (
            "org.gnome.desktop.a11y.interface",
            "high-contrast",
            if mode.starts_with("hc-") {
                "true"
            } else {
                "false"
            },
        ),
    ] {
        match gsettings_set(schema, key, val) {
            Ok(()) => notes.push(format!("{key}={val}")),
            Err(e) => notes.push(e),
        }
    }
    // Also tip qt when available.
    let _ = std::env::set_var("GTK_THEME", gtk);
    format!("Contrast · {}", notes.join(" · "))
}

/// Apply UI + monospace fonts to fontconfig, GTK, GSettings, and Qt.
pub fn apply_ui_fonts(ui: &str, mono: &str) -> String {
    let ui = ui.trim();
    let mono = mono.trim();
    let ui = if ui.is_empty() { mde_theme::FONT } else { ui };
    let mono = if mono.is_empty() {
        mde_theme::FONT_MONO
    } else {
        mono
    };
    let body = "10.5";
    let title = "9";
    let look_font = format!("{ui} {body}");
    let look_title = format!("{ui} {title}");
    let mono_gs = format!("{mono} {body}");
    let mut notes = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        let fc_dir = std::path::PathBuf::from(&home).join(".config/fontconfig/conf.d");
        let _ = std::fs::create_dir_all(&fc_dir);
        let fc_path = fc_dir.join("50-mde-ui-font.conf");
        let xml = format!(
            r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <alias binding="same">
    <family>sans-serif</family>
    <prefer><family>{ui}</family></prefer>
  </alias>
  <alias binding="same">
    <family>monospace</family>
    <prefer><family>{mono}</family></prefer>
  </alias>
  <alias binding="same">
    <family>Segoe UI</family>
    <prefer><family>{ui}</family></prefer>
  </alias>
  <alias binding="same">
    <family>Segoe UI Variable</family>
    <prefer><family>{ui}</family></prefer>
  </alias>
</fontconfig>
"#
        );
        match std::fs::write(&fc_path, xml) {
            Ok(()) => notes.push("fontconfig".into()),
            Err(e) => notes.push(format!("fontconfig: {e}")),
        }

        for gtk_ver in ["gtk-3.0", "gtk-4.0"] {
            let dir = std::path::PathBuf::from(&home).join(".config").join(gtk_ver);
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("settings.ini");
            let mut body_ini = if path.is_file() {
                std::fs::read_to_string(&path).unwrap_or_default()
            } else {
                "[Settings]\n".into()
            };
            if body_ini.contains("gtk-font-name=") {
                let re = regex_lite_replace_font_line(&body_ini, "gtk-font-name", &look_font);
                body_ini = re;
            } else if body_ini.contains("[Settings]") {
                body_ini = body_ini.replacen(
                    "[Settings]",
                    &format!("[Settings]\ngtk-font-name={look_font}"),
                    1,
                );
            } else {
                body_ini.push_str(&format!("\n[Settings]\ngtk-font-name={look_font}\n"));
            }
            let _ = std::fs::write(&path, body_ini);
        }
        notes.push("gtk.ini".into());

        // Qt5/Qt6 font via qt5ct / qt6ct if present; else kdeglobals General font.
        let kde = std::path::PathBuf::from(&home).join(".config/kdeglobals");
        if kde.is_file() || std::fs::write(&kde, "[General]\n").is_ok() {
            let mut text = std::fs::read_to_string(&kde).unwrap_or_else(|_| "[General]\n".into());
            if !text.contains("[General]") {
                text.push_str("\n[General]\n");
            }
            // Qt font string: "Family,size,-1,5,400,0,0,0,0,0"
            let qt_font = format!("\"{ui},{body},-1,5,400,0,0,0,0,0\"");
            let qt_fixed = format!("\"{mono},{body},-1,5,400,0,0,0,0,0\"");
            text = upsert_ini_key(&text, "General", "font", &qt_font);
            text = upsert_ini_key(&text, "General", "fixed", &qt_fixed);
            let _ = std::fs::write(&kde, text);
            notes.push("kdeglobals".into());
        }

        // xfconf xsettings when available
        let _ = run(
            "xfconf-query",
            &[
                "-c",
                "xsettings",
                "-p",
                "/Gtk/FontName",
                "-s",
                &look_font,
            ],
        );
        let _ = run(
            "xfconf-query",
            &[
                "-c",
                "xfwm4",
                "-p",
                "/general/title_font",
                "-s",
                &look_title,
            ],
        );
    }

    for (key, val) in [
        ("font-name", look_font.as_str()),
        ("monospace-font-name", mono_gs.as_str()),
    ] {
        match gsettings_set("org.gnome.desktop.interface", key, val) {
            Ok(()) => notes.push(format!("gsettings {key}")),
            Err(e) => notes.push(e),
        }
    }

    let _ = run("fc-cache", &["-f"]);
    notes.push("fc-cache".into());
    format!("Fonts · {ui} / {mono} · {}", notes.join(" · "))
}

fn regex_lite_replace_font_line(text: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    for line in text.lines() {
        if line.trim_start().starts_with(&format!("{key}=")) {
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn upsert_ini_key(text: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut in_section = false;
    let mut replaced = false;
    let mut section_end = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            if in_section && section_end.is_none() {
                section_end = Some(i);
            }
            in_section = t.eq_ignore_ascii_case(&header);
            continue;
        }
        if in_section && t.starts_with(&format!("{key}=")) {
            lines[i] = format!("{key}={value}");
            replaced = true;
            break;
        }
    }
    if !replaced {
        if let Some(idx) = lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&header)) {
            lines.insert(idx + 1, format!("{key}={value}"));
        } else {
            lines.push(header);
            lines.push(format!("{key}={value}"));
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let _ = section_end;
    out
}

/// A072: persist + apply text scale to gsettings and MDE config.
pub fn apply_text_scale(cfg: &mut mde_config::MdeConfig, scale: f64) -> Result<String, String> {
    let scale = scale.clamp(0.85, 2.25);
    cfg.active_mut().accessibility.text_scale = scale;
    cfg.save().map_err(|e| e.to_string())?;
    let v = format!("{scale:.2}");
    gsettings_set(
        "org.gnome.desktop.interface",
        "text-scaling-factor",
        &v,
    )?;
    // Hint iced shells via runtime file.
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let _ = std::fs::write(
        std::path::Path::new(&runtime).join("mde-text-scale"),
        format!("{scale}"),
    );
    Ok(format!("Text scale {v}"))
}

/// A090: hint for large files under the home folder.
pub fn storage_large_files_hint() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut found: Vec<(u64, String)> = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(&home)];
    let mut seen = 0u32;
    while let Some(dir) = stack.pop() {
        if seen > 800 {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            seen += 1;
            let p = ent.path();
            let Ok(meta) = ent.metadata() else {
                continue;
            };
            if meta.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "node_modules" {
                    continue;
                }
                stack.push(p);
            } else if meta.len() > 100 * 1024 * 1024 {
                found.push((meta.len(), p.display().to_string()));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.truncate(5);
    if found.is_empty() {
        "No files over 100 MB found in a quick scan.".into()
    } else {
        found
            .into_iter()
            .map(|(b, p)| format!("{:.1} GB · {p}", b as f64 / 1_073_741_824.0))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A090: run Storage Sense cleanup once.
pub fn run_storage_sense(cfg: &mde_config::MdeConfig) -> String {
    let s = &cfg.active().storage;
    let mut cleaned = Vec::new();
    if s.sense_temp {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        for rel in [".cache/thumbnails", ".cache/mesa_shader_cache"] {
            let path = home.join(rel);
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
                let _ = std::fs::create_dir_all(&path);
                cleaned.push(rel);
            }
        }
        let tmp = std::env::temp_dir();
        if let Ok(rd) = std::fs::read_dir(&tmp) {
            for ent in rd.flatten().take(40) {
                let p = ent.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("mde-") || name.ends_with(".tmp") {
                    let _ = if p.is_dir() {
                        std::fs::remove_dir_all(&p)
                    } else {
                        std::fs::remove_file(&p)
                    };
                }
            }
        }
        cleaned.push("temp");
    }
    if s.sense_trash {
        let trash = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".local/share/Trash/files");
        if trash.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&trash) {
                for ent in rd.flatten() {
                    let p = ent.path();
                    let _ = if p.is_dir() {
                        std::fs::remove_dir_all(&p)
                    } else {
                        std::fs::remove_file(&p)
                    };
                }
            }
            cleaned.push("trash");
        }
    }
    if s.sense_downloads_days > 0 {
        let dl = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Downloads");
        if dl.is_dir() {
            let cutoff = std::time::SystemTime::now()
                - std::time::Duration::from_secs(u64::from(s.sense_downloads_days) * 86400);
            if let Ok(rd) = std::fs::read_dir(&dl) {
                let mut n = 0u32;
                for ent in rd.flatten() {
                    let p = ent.path();
                    if let Ok(meta) = p.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified < cutoff {
                                let _ = if p.is_dir() {
                                    std::fs::remove_dir_all(&p)
                                } else {
                                    std::fs::remove_file(&p)
                                };
                                n += 1;
                            }
                        }
                    }
                }
                if n > 0 {
                    cleaned.push("downloads");
                }
            }
        }
    }
    if cleaned.is_empty() {
        "Storage Sense · nothing to clean".into()
    } else {
        format!("Storage Sense · cleaned {}", cleaned.join(", "))
    }
}

/// A092: best-effort list of apps holding camera/mic (PipeWire /lsof).
pub fn privacy_in_use() -> Vec<(String, String)> {
    let mut out = Vec::new();
    // Mic: check default source peak via wpctl; list clients via pw-dump when present.
    if let Ok(o) = Command::new("pw-cli").args(["ls", "Node"]).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            let l = line.to_lowercase();
            if l.contains("audio") && (l.contains("capture") || l.contains("input")) {
                out.push(("microphone".into(), "PipeWire capture node".into()));
                break;
            }
        }
    }
    // Camera: V4L2 open handles.
    if let Ok(rd) = std::fs::read_dir("/dev") {
        for ent in rd.flatten() {
            let name = ent.file_name();
            let n = name.to_string_lossy();
            if n.starts_with("video") {
                if let Ok(o) = Command::new("fuser").arg(ent.path()).output() {
                    let text = String::from_utf8_lossy(&o.stdout);
                    if !text.trim().is_empty() {
                        out.push(("camera".into(), format!("/dev/{n} in use")));
                    }
                }
            }
        }
    }
    // GeoClue clients (best-effort).
    if which("geoclue") || which("geoclue2") {
        if let Ok(o) = Command::new("busctl")
            .args([
                "--user",
                "tree",
                "org.freedesktop.GeoClue2",
            ])
            .output()
        {
            if o.status.success() {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.contains("Client") {
                    out.push(("location".into(), "GeoClue client active".into()));
                }
            }
        }
    }
    out
}

/// A093: compact security health for Home cards.
pub fn security_health_cards() -> Vec<(String, String, bool)> {
    let fw = firewall_status();
    let fw_ok = fw.to_lowercase().contains("running");
    let selinux = selinux_status();
    let selinux_ok = !selinux.to_lowercase().contains("disabled");
    let updates = update_check_summary();
    let updates_ok = updates.to_lowercase().contains("up to date")
        || updates.to_lowercase().contains("no packages")
        || updates.contains("Press Check");
    let disk = disk_root_usage();
    let disk_ok = {
        let d = disk.to_lowercase();
        !(d.contains("100%") || d.contains("99%") || d.contains("full"))
    };
    vec![
        (
            "Firewall".into(),
            if fw_ok {
                "firewalld running".into()
            } else {
                "Check firewall status".into()
            },
            fw_ok,
        ),
        ("SELinux".into(), selinux, selinux_ok),
        (
            "Updates".into(),
            if updates_ok {
                "Up to date / checked".into()
            } else {
                "Updates may be available".into()
            },
            updates_ok,
        ),
        ("Storage".into(), disk, disk_ok),
    ]
}

/// A076: resize image with ImageMagick `convert` or `magick`.
pub fn resize_image(path: &str, preset: &str) -> Result<String, String> {
    let src = std::path::Path::new(path);
    if !src.is_file() {
        return Err("Not a file".into());
    }
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png");
    let parent = src.parent().unwrap_or_else(|| std::path::Path::new("."));
    let dest = parent.join(format!("{stem}_{preset}.{ext}"));
    let geom: String = match preset {
        "50%" | "Small" => "50%".into(),
        "1024w" => "1024x>".into(),
        "1920w" => "1920x>".into(),
        other if other.ends_with('%') => other.to_string(),
        other if other.ends_with('w') => {
            let w = other.trim_end_matches('w');
            format!("{w}x>")
        }
        other => other.to_string(),
    };
    let bin = if which("magick") {
        "magick"
    } else if which("convert") {
        "convert"
    } else {
        return Err("Install ImageMagick (magick/convert)".into());
    };
    let status = if bin == "magick" {
        Command::new("magick")
            .args([path, "-resize", &geom, dest.to_str().unwrap_or("out.png")])
            .status()
    } else {
        Command::new("convert")
            .args([path, "-resize", &geom, dest.to_str().unwrap_or("out.png")])
            .status()
    }
    .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(dest.display().to_string())
    } else {
        Err(format!("resize failed ({status})"))
    }
}

/// Install a Thunar custom action for Image Resizer (A076).
pub fn install_image_resizer_uca(presets: &[String]) -> Result<String, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "no config dir".to_string())?
        .join("Thunar");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("uca.xml");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("mde-image-resizer") {
        return Ok("Thunar Image Resizer action already installed".into());
    }
    let preset = presets.first().cloned().unwrap_or_else(|| "1024w".into());
    let action = format!(
        r#"
<action>
        <icon>image-resize-symbolic</icon>
        <name>Resize with MDE Image Resizer</name>
        <unique-id>mde-image-resizer-1</unique-id>
        <command>sh -c 'mde-control tools --resize-image "%f" "{preset}" || convert "%f" -resize 1024x&gt; "%f_resized.png"'</command>
        <description>PowerToys-like Image Resizer (A076)</description>
        <patterns>*.jpg;*.jpeg;*.png;*.webp;*.bmp</patterns>
        <image-files/>
</action>
"#
    );
    let body = if existing.contains("</actions>") {
        existing.replacen("</actions>", &format!("{action}</actions>"), 1)
    } else {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>{action}</actions>\n")
    };
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(format!("Installed Thunar action · {}", path.display()))
}
