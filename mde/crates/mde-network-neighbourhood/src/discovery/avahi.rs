use crate::place::{DiscoveredHost, Protocol};
use anyhow::Result;
use std::process::Command;

/// Browse common service types via `avahi-browse`.
pub fn browse() -> Result<Vec<DiscoveredHost>> {
    let types = [
        ("_smb._tcp", Protocol::Smb, 445),
        ("_ssh._tcp", Protocol::Ssh, 22),
        ("_sftp-ssh._tcp", Protocol::Ssh, 22),
        ("_nfs._tcp", Protocol::Nfs, 2049),
        ("_webdav._tcp", Protocol::WebDav, 443),
        ("_webdavs._tcp", Protocol::WebDav, 443),
        ("_http._tcp", Protocol::WebDav, 80), // filtered later for jellyfin/airsonic paths
    ];

    let mut out = Vec::new();
    for (svc, proto, default_port) in types {
        let output = Command::new("avahi-browse")
            .args(["-trkp", svc])
            .output();
        let Ok(output) = output else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            // parse: =;iface;proto;name;type;domain;host;addr;port;txt
            if !line.starts_with('=') {
                continue;
            }
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() < 9 {
                continue;
            }
            let name = parts[3].to_string();
            let host = parts[6].trim_end_matches('.').to_string();
            let port: u16 = parts[8].parse().unwrap_or(default_port);
            let txt = parts.get(9).copied().unwrap_or("");

            let mut protocol = proto;
            let mut url = String::new();
            let mut path = String::new();

            // Detect Jellyfin / Airsonic from HTTP TXT or name.
            if svc == "_http._tcp" {
                let lower = format!("{name} {txt}").to_ascii_lowercase();
                if lower.contains("jellyfin") {
                    protocol = Protocol::Video;
                    url = format!("http://{host}:{port}");
                } else if lower.contains("airsonic")
                    || lower.contains("navidrome")
                    || lower.contains("subsonic")
                {
                    protocol = Protocol::Audio;
                    url = format!("http://{host}:{port}");
                } else {
                    continue;
                }
            }

            if protocol == Protocol::Smb {
                path.clear();
            }

            out.push(DiscoveredHost {
                name,
                host,
                protocol,
                port: Some(port),
                path,
                url,
                reachable: true,
                source: format!("avahi:{svc}"),
                last_seen: 0,
            });
        }
    }
    Ok(out)
}
