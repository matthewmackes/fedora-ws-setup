use crate::place::DiscoveredHost;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATE_DIR: &str = "/var/lib/network-neighbourhood";
pub const DISCOVERED_FILE: &str = "/var/lib/network-neighbourhood/discovered.json";

static SCANNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryCache {
    /// Unix epoch seconds when the last successful (or attempted) scan finished.
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub scanning: bool,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub hosts: Vec<DiscoveredHost>,
    /// Last progress line from the background scanner (best-effort).
    #[serde(default)]
    pub last_status: String,
}

impl DiscoveryCache {
    pub fn load() -> Self {
        let path = Path::new(DISCOVERED_FILE);
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        ensure_state_dir()?;
        let json = serde_json::to_string_pretty(self)?;
        let tmp = Path::new(DISCOVERED_FILE).with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)
                .with_context(|| format!("create {}", tmp.display()))?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, DISCOVERED_FILE)?;
        Ok(())
    }

    pub fn age_secs(&self) -> Option<u64> {
        if self.updated_at == 0 {
            return None;
        }
        let now = now_epoch();
        Some(now.saturating_sub(self.updated_at))
    }
}

pub fn ensure_state_dir() -> Result<()> {
    fs::create_dir_all(STATE_DIR)?;
    Ok(())
}

/// Clear a stale "scanning" flag left behind if the daemon was killed mid-scan.
pub fn reset_stale_scanning_flag() {
    SCANNING.store(false, Ordering::SeqCst);
    let mut cache = DiscoveryCache::load();
    if cache.scanning {
        cache.scanning = false;
        if cache.last_status.contains("…") || cache.updated_at == 0 {
            cache.last_status =
                "Previous scan interrupted — waiting for next background pass.".into();
        }
        let _ = cache.save();
    }
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn is_scanning() -> bool {
    SCANNING.load(Ordering::Relaxed)
}

/// Merge newly found hosts into the on-disk cache without wiping prior results.
fn persist_partial(status: &str, extra_hosts: &[DiscoveredHost]) {
    let mut c = DiscoveryCache::load();
    c.scanning = true;
    c.last_status = status.to_string();
    if !extra_hosts.is_empty() {
        c.hosts.extend(extra_hosts.iter().cloned());
        dedupe_hosts(&mut c.hosts);
    }
    let _ = c.save();
}

fn host_key(h: &DiscoveredHost) -> String {
    format!(
        "{}:{}:{}:{}",
        h.protocol.as_str(),
        h.host,
        h.port.unwrap_or(0),
        h.path
    )
}

fn dedupe_hosts(list: &mut Vec<DiscoveredHost>) {
    let mut seen = std::collections::HashSet::new();
    list.retain(|h| seen.insert(host_key(h)));
}

/// Drop hosts not observed for longer than `max_age_secs` (keeps the DB from growing forever).
fn prune_stale(hosts: &mut Vec<DiscoveredHost>, now: u64, max_age_secs: u64) {
    hosts.retain(|h| {
        let seen = if h.last_seen == 0 { now } else { h.last_seen };
        now.saturating_sub(seen) <= max_age_secs
    });
}

/// Run discovery and persist results. Safe to call from a blocking thread.
pub fn run_scan_and_store(settings: &crate::config::Settings) -> Result<DiscoveryCache> {
    if SCANNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        for _ in 0..600 {
            if !SCANNING.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let mut cache = DiscoveryCache::load();
        cache.scanning = SCANNING.load(Ordering::SeqCst);
        if cache.last_status.is_empty() {
            cache.last_status = "Joined in-progress background scan.".into();
        }
        return Ok(cache);
    }

    let prior_cache = DiscoveryCache::load();
    let prior_hosts = prior_cache.hosts.clone();
    let prior_updated = prior_cache.updated_at;
    let cache = DiscoveryCache {
        updated_at: 0,
        scanning: true,
        last_error: String::new(),
        hosts: prior_hosts.clone(),
        last_status: "Starting background discovery…".into(),
    };
    let _ = cache.save();

    let result = (|| {
        let last = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let last_p = last.clone();
        let mut progress = move |msg: &str| {
            if let Ok(mut g) = last_p.lock() {
                *g = msg.to_string();
            }
            persist_partial(msg, &[]);
        };
        let last_h = last.clone();
        let mut on_hosts = move |_stage: &str, hosts: &[DiscoveredHost]| {
            let status = last_h.lock().map(|g| g.clone()).unwrap_or_default();
            persist_partial(&status, hosts);
        };

        let hosts = crate::discovery::discover_all_with_progress_hosts(
            settings,
            &mut progress,
            &mut on_hosts,
        )?;

        let now = now_epoch();
        let mut fresh = hosts;
        for h in &mut fresh {
            h.last_seen = now;
        }

        // Keep prior hosts not replaced this pass; backfill last_seen for legacy entries
        // using the previous cache timestamp so they start aging immediately.
        let mut merged = prior_hosts;
        for h in &mut merged {
            if h.last_seen == 0 {
                h.last_seen = if prior_updated > 0 { prior_updated } else { now };
            }
        }
        merged.extend(fresh);
        // Prefer the freshest copy of each key.
        {
            let mut best: std::collections::HashMap<String, DiscoveredHost> =
                std::collections::HashMap::new();
            for h in merged.drain(..) {
                let key = host_key(&h);
                match best.get(&key) {
                    Some(prev) if prev.last_seen > h.last_seen => {}
                    _ => {
                        best.insert(key, h);
                    }
                }
            }
            merged.extend(best.into_values());
        }
        // Age out hosts missing for ~3 scan intervals (floor 1h, ceiling 7d).
        let max_age = settings
            .discovery_interval_sec
            .max(60)
            .saturating_mul(3)
            .clamp(3600, 7 * 24 * 3600);
        prune_stale(&mut merged, now, max_age);
        dedupe_hosts(&mut merged);
        let n = merged.len();

        let out = DiscoveryCache {
            updated_at: now,
            scanning: false,
            last_error: String::new(),
            hosts: merged,
            last_status: format!("Discovery finished — {n} host(s)."),
        };
        out.save()?;
        Ok::<_, anyhow::Error>(out)
    })();

    SCANNING.store(false, Ordering::SeqCst);

    match result {
        Ok(cache) => Ok(cache),
        Err(e) => {
            let mut cache = DiscoveryCache::load();
            cache.scanning = false;
            cache.updated_at = now_epoch();
            cache.last_error = e.to_string();
            cache.last_status = format!("Scan failed: {e}");
            let _ = cache.save();
            Err(e)
        }
    }
}
