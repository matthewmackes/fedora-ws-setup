pub mod cache;
mod avahi;
mod scan;
mod ws_discovery;

pub use cache::{DiscoveryCache, DISCOVERED_FILE};

use crate::config::Settings;
use crate::place::{DiscoveredHost, Protocol};
use anyhow::Result;

pub type ProgressFn<'a> = dyn FnMut(&str) + 'a;

/// Run all enabled discovery backends and merge results.
pub fn discover_all(settings: &Settings) -> Result<Vec<DiscoveredHost>> {
    discover_all_with_progress(settings, &mut |_| {})
}

/// Same as `discover_all`, emitting human-readable stage lines for the UI log.
/// `progress(status, hosts_so_far)` — hosts_so_far grows after each backend.
pub fn discover_all_with_progress(
    settings: &Settings,
    progress: &mut ProgressFn<'_>,
) -> Result<Vec<DiscoveredHost>> {
    discover_all_with_progress_hosts(settings, progress, &mut |_, _| {})
}

pub fn discover_all_with_progress_hosts(
    settings: &Settings,
    progress: &mut ProgressFn<'_>,
    on_hosts: &mut dyn FnMut(&str, &[DiscoveredHost]),
) -> Result<Vec<DiscoveredHost>> {
    let mut out: Vec<DiscoveredHost> = Vec::new();

    if settings.discovery_classic {
        progress("Listening for mDNS / Avahi services…");
        match avahi::browse() {
            Ok(mut v) => {
                progress(&format!("  Avahi: {} host(s)", v.len()));
                out.append(&mut v);
                on_hosts("avahi", &out);
            }
            Err(e) => progress(&format!("  Avahi: skipped ({e})")),
        }

        progress("Probing WS-Discovery…");
        match ws_discovery::probe() {
            Ok(mut v) => {
                progress(&format!("  WS-Discovery: {} host(s)", v.len()));
                out.append(&mut v);
                on_hosts("ws-discovery", &out);
            }
            Err(e) => progress(&format!("  WS-Discovery: skipped ({e})")),
        }

        progress("Browsing SMB workgroups (smbtree)…");
        match smb_browse_hint() {
            Ok(mut v) => {
                progress(&format!("  SMB browse: {} share(s)/host(s)", v.len()));
                out.append(&mut v);
                on_hosts("smb-browse", &out);
            }
            Err(e) => progress(&format!("  SMB browse: skipped ({e})")),
        }
    } else {
        progress("Classic discovery disabled in Settings.");
    }

    if settings.discovery_scan {
        progress("Starting subnet port scan…");
        match scan::subnet_scan_with_progress(settings, progress) {
            Ok(mut v) => {
                progress(&format!("  Subnet scan: {} reachable service(s)", v.len()));
                out.append(&mut v);
                on_hosts("subnet-scan", &out);
            }
            Err(e) => progress(&format!("  Subnet scan: failed ({e})")),
        }
    } else {
        progress("Subnet scan disabled in Settings.");
    }

    progress("Merging and deduplicating results…");
    dedupe(&mut out);
    out.sort_by(|a, b| {
        a.protocol
            .group_label()
            .cmp(b.protocol.group_label())
            .then(a.name.cmp(&b.name))
    });
    progress(&format!("Discovery finished — {} unique host(s).", out.len()));
    on_hosts("done", &out);
    Ok(out)
}

fn smb_browse_hint() -> Result<Vec<DiscoveredHost>> {
    let output = std::process::Command::new("smbtree")
        .args(["-N"])
        .output();
    let Ok(output) = output else {
        return Ok(vec![]);
    };
    if !output.status.success() {
        return Ok(vec![]);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut hosts = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("\\\\").or_else(|| t.strip_prefix("\\")) {
            let parts: Vec<_> = rest.split('\\').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                continue;
            }
            let host = parts[0].to_string();
            let path = parts.get(1).copied().unwrap_or("").to_string();
            hosts.push(DiscoveredHost {
                name: if path.is_empty() {
                    host.clone()
                } else {
                    format!("{host}/{path}")
                },
                host,
                protocol: Protocol::Smb,
                port: Some(445),
                path,
                url: String::new(),
                reachable: true,
                source: "smb-browse".into(),
                last_seen: 0,
            });
        }
    }
    Ok(hosts)
}

fn dedupe(list: &mut Vec<DiscoveredHost>) {
    let mut seen = std::collections::HashSet::new();
    list.retain(|h| {
        let key = format!(
            "{}:{}:{}:{}",
            h.protocol.as_str(),
            h.host,
            h.port.unwrap_or(0),
            h.path
        );
        seen.insert(key)
    });
}
