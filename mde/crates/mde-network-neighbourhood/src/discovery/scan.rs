use crate::config::Settings;
use crate::place::{DiscoveredHost, Protocol};
use anyhow::Result;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Probe configured (or auto-detected) subnets for known service ports.
pub fn subnet_scan(settings: &Settings) -> Result<Vec<DiscoveredHost>> {
    subnet_scan_with_progress(settings, &mut |_| {})
}

pub fn subnet_scan_with_progress(
    settings: &Settings,
    progress: &mut crate::discovery::ProgressFn<'_>,
) -> Result<Vec<DiscoveredHost>> {
    let subnets = if settings.scan_subnets.is_empty() {
        auto_subnets()
    } else {
        settings.scan_subnets.clone()
    };
    let ports = if settings.scan_ports.is_empty() {
        vec![22, 111, 139, 445, 2049, 4040, 8096, 8920]
    } else {
        settings.scan_ports.clone()
    };

    if subnets.is_empty() {
        progress("  No local subnets detected for scanning.");
        return Ok(vec![]);
    }

    progress(&format!(
        "  Subnets: {}",
        subnets.join(", ")
    ));
    progress(&format!(
        "  Ports: {}",
        ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let mut out = Vec::new();
    for cidr in &subnets {
        let ips = expand_cidr_limited(cidr, 256);
        progress(&format!(
            "  Scanning {cidr} ({} address(es))…",
            ips.len()
        ));
        let mut found_here = 0usize;
        let mut checked = 0usize;
        let total = ips.len();
        for ip in ips {
            checked += 1;
            if checked == 1 || checked % 32 == 0 || checked == total {
                progress(&format!(
                    "  … {cidr}: checked {checked}/{total}, found {found_here}"
                ));
            }
            for &port in &ports {
                if !port_open(ip, port) {
                    continue;
                }
                if let Some(h) = classify(ip, port) {
                    progress(&format!(
                        "  Found {} on {}:{}",
                        h.protocol.group_label(),
                        h.host,
                        port
                    ));
                    found_here += 1;
                    out.push(h);
                }
            }
        }
        progress(&format!("  Finished {cidr} — {found_here} hit(s)."));
    }
    Ok(out)
}

fn classify(ip: Ipv4Addr, port: u16) -> Option<DiscoveredHost> {
    let host = ip.to_string();
    let (protocol, name, url) = match port {
        22 => (Protocol::Ssh, format!("ssh:{host}"), String::new()),
        139 | 445 => (Protocol::Smb, format!("smb:{host}"), String::new()),
        2049 | 111 => (Protocol::Nfs, format!("nfs:{host}"), String::new()),
        8096 => (
            Protocol::Video,
            format!("jellyfin:{host}"),
            format!("http://{host}:8096"),
        ),
        8920 => (
            Protocol::Video,
            format!("jellyfin:{host}"),
            format!("https://{host}:8920"),
        ),
        4040 => (
            Protocol::Audio,
            format!("airsonic:{host}"),
            format!("http://{host}:4040"),
        ),
        80 | 443 => return None,
        _ => return None,
    };
    Some(DiscoveredHost {
        name,
        host,
        protocol,
        port: Some(port),
        path: String::new(),
        url,
        reachable: true,
        source: "subnet-scan".into(),
        last_seen: 0,
    })
}

fn port_open(ip: Ipv4Addr, port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    // Keep probes snappy — full /24 × ports must finish in a couple of minutes.
    TcpStream::connect_timeout(&addr, Duration::from_millis(40)).is_ok()
}

/// Prefer compact /24s around local interface addresses and the default gateway.
/// Large routes like /16 are never scanned whole — that never finishes and misses hosts.
fn auto_subnets() -> Vec<String> {
    let mut nets = Vec::new();

    // 1) Interface addresses → always scan that /24.
    if let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", "up"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            // ... inet 172.20.146.49/16 brd ...
            let parts: Vec<_> = line.split_whitespace().collect();
            if let Some(i) = parts.iter().position(|p| *p == "inet") {
                if let Some(cidr) = parts.get(i + 1) {
                    if let Some(slash) = cidr.find('/') {
                        if let Ok(ip) = cidr[..slash].parse::<Ipv4Addr>() {
                            if !ip.is_loopback() {
                                nets.push(format!("{}/24", ipv4_slash24(ip)));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2) Default gateway /24 — NAS boxes often live near the router, not near DHCP clients.
    if let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "route", "show", "default"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<_> = line.split_whitespace().collect();
            if let Some(i) = parts.iter().position(|p| *p == "via") {
                if let Some(gw) = parts.get(i + 1) {
                    if let Ok(ip) = gw.parse::<Ipv4Addr>() {
                        nets.push(format!("{}/24", ipv4_slash24(ip)));
                    }
                }
            }
        }
    }

    // 3) Connected routes that are already /24../28.
    if let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "route", "show"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "dev" {
                let cidr = parts[0];
                if cidr.contains('/') && !cidr.starts_with("default") {
                    if let Some((_, pref)) = cidr.split_once('/') {
                        if let Ok(p) = pref.parse::<u32>() {
                            if (24..=30).contains(&p) {
                                nets.push(cidr.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    nets.sort();
    nets.dedup();
    nets
}

fn ipv4_slash24(ip: Ipv4Addr) -> Ipv4Addr {
    let u = u32::from(ip) & !0xff;
    Ipv4Addr::from(u)
}

/// Expand IPv4 CIDR but cap host count.
fn expand_cidr_limited(cidr: &str, max_hosts: usize) -> Vec<Ipv4Addr> {
    let Some((ip_s, prefix_s)) = cidr.split_once('/') else {
        return vec![];
    };
    let Ok(ip) = ip_s.parse::<Ipv4Addr>() else {
        return vec![];
    };
    let Ok(prefix) = prefix_s.parse::<u32>() else {
        return vec![];
    };
    if prefix > 30 || prefix < 16 {
        return vec![];
    }
    let ip_u = u32::from(ip);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let network = ip_u & mask;
    let size = 1u32 << (32 - prefix);

    // For large nets, only scan a window centered on the seed IP (or network+/24).
    let (start, end) = if size > max_hosts as u32 + 2 {
        let center = ip_u.saturating_sub(network).min(size.saturating_sub(1));
        let half = (max_hosts as u32) / 2;
        let s = center.saturating_sub(half).max(1);
        let e = (s + max_hosts as u32).min(size.saturating_sub(1));
        (s, e)
    } else {
        let start = if size > 2 { 1 } else { 0 };
        let end = if size > 2 { size - 1 } else { size };
        (start, end)
    };

    let mut hosts = Vec::new();
    for off in start..end {
        if hosts.len() >= max_hosts {
            break;
        }
        hosts.push(Ipv4Addr::from(network.wrapping_add(off)));
    }
    hosts
}
