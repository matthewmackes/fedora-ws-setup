//! Performance Monitor — live counters (collector sets scaffold).

use std::fs;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::SnapIn;
use crate::model::{
    ActionItem, Gauge, ResultRow, ResultsView, SummaryChip, Tone, TreeNode,
};

pub struct Perfmon;

static LAST_CPU: Mutex<Option<(u64, u64)>> = Mutex::new(None);
static LAST_NET: Mutex<Option<(u64, u64, u64)>> = Mutex::new(None);
static LAST_DISK: Mutex<Option<(u64, u64)>> = Mutex::new(None);

fn cpu_pct() -> f32 {
    let text = fs::read_to_string("/proc/stat").unwrap_or_default();
    let line = text.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace().skip(1);
    let mut vals = [0u64; 8];
    for v in vals.iter_mut() {
        *v = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    }
    let idle = vals[3] + vals[4];
    let total: u64 = vals.iter().sum();
    let mut slot = LAST_CPU.lock().unwrap();
    let pct = if let Some((pi, pt)) = *slot {
        let di = idle.saturating_sub(pi) as f32;
        let dt = total.saturating_sub(pt) as f32;
        if dt > 0.0 {
            (1.0 - di / dt) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };
    *slot = Some((idle, total));
    pct
}

fn mem_info() -> (f32, String) {
    let text = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0f32;
    let mut avail = 0f32;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = v
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
        }
        if let Some(v) = line.strip_prefix("MemAvailable:") {
            avail = v
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
        }
    }
    let pct = if total > 0.0 {
        (1.0 - avail / total) * 100.0
    } else {
        0.0
    };
    let used_gib = (total - avail) / 1_048_576.0;
    let total_gib = total / 1_048_576.0;
    (
        pct,
        format!("{used_gib:.1} / {total_gib:.1} GiB ({pct:.0}%)"),
    )
}

fn net_mbs() -> f32 {
    let text = fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in text.lines().skip(2) {
        let mut sp = line.split_whitespace();
        let Some(name) = sp.next() else { continue };
        if name.starts_with("lo") {
            continue;
        }
        let r: u64 = sp.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        for _ in 0..7 {
            sp.next();
        }
        let t: u64 = sp.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        rx += r;
        tx += t;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut slot = LAST_NET.lock().unwrap();
    let mbs = if let Some((prx, ptx, pts)) = *slot {
        let dt = now.saturating_sub(pts).max(1) as f32;
        let bytes = (rx.saturating_sub(prx) + tx.saturating_sub(ptx)) as f32;
        bytes / dt / 1_000_000.0
    } else {
        0.0
    };
    *slot = Some((rx, tx, now));
    mbs
}

fn disk_mbs() -> f32 {
    let text = fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut sectors = 0u64;
    for line in text.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 14 {
            continue;
        }
        let name = p[2];
        if name.starts_with("loop") || name.starts_with("ram") {
            continue;
        }
        let r: u64 = p[5].parse().unwrap_or(0);
        let w: u64 = p[9].parse().unwrap_or(0);
        sectors += r + w;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut slot = LAST_DISK.lock().unwrap();
    let mbs = if let Some((ps, pts)) = *slot {
        let dt = now.saturating_sub(pts).max(1) as f32;
        let bytes = sectors.saturating_sub(ps) as f32 * 512.0;
        bytes / dt / 1_000_000.0
    } else {
        0.0
    };
    *slot = Some((sectors, now));
    mbs
}

/// Map throughput to a soft 0–100 scale for the bar (100 ≈ 100 MB/s).
fn throughput_bar(mbs: f32) -> f32 {
    (mbs / 100.0 * 100.0).clamp(0.0, 100.0)
}

fn loadavg_line() -> String {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| {
            let parts: Vec<&str> = s.split_whitespace().take(3).collect();
            if parts.len() == 3 {
                format!("{} {} {}", parts[0], parts[1], parts[2])
            } else {
                s.trim().to_string()
            }
        })
        .unwrap_or_else(|| "—".into())
}

fn uptime_line() -> String {
    let Ok(text) = fs::read_to_string("/proc/uptime") else {
        return "uptime unavailable".into();
    };
    let secs: f64 = text
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let total = secs as u64;
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let mins = (total % 3_600) / 60;
    if days > 0 {
        format!("Uptime {days}d {hours}h {mins}m")
    } else {
        format!("Uptime {hours}h {mins}m")
    }
}

impl SnapIn for Perfmon {
    fn key(&self) -> &'static str {
        "perfmon"
    }
    fn name(&self) -> &'static str {
        "Performance Monitor"
    }
    fn icon(&self) -> &'static str {
        "utilities-system-monitor"
    }
    fn description(&self) -> &'static str {
        "See how this computer is using processor, memory, disks and network"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new(
            "perfmon",
            "Performance Monitor",
            "utilities-system-monitor",
            "perfmon",
        )
        .with_children(vec![
            TreeNode::new(
                "perfmon-live",
                "Live counters",
                "utilities-system-monitor",
                "perfmon",
            ),
            TreeNode::new(
                "perfmon-collectors",
                "Data Collector Sets",
                "folder",
                "perfmon",
            ),
            TreeNode::new("perfmon-alerts", "Alerts", "dialog-warning", "perfmon"),
        ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "perfmon-collectors" => ResultsView {
                detail: "Data collector sets: configure sampling to ~/.local/state/mde/perfmon/ (W474).\nCreate collectors from Action pane in a follow-up sheet.".into(),
                status: "Collectors".into(),
                ..Default::default()
            },
            "perfmon-alerts" => ResultsView {
                detail: "Alerts: threshold rules not yet configured.".into(),
                status: "Alerts".into(),
                ..Default::default()
            },
            _ => {
                let cpu = cpu_pct();
                let (mem, mem_disp) = mem_info();
                let disk = disk_mbs();
                let net = net_mbs();
                ResultsView {
                    summaries: vec![
                        SummaryChip::new("CPU", format!("{cpu:.0}%")).tone(
                            if cpu >= 90.0 {
                                Tone::Bad
                            } else if cpu >= 75.0 {
                                Tone::Warn
                            } else {
                                Tone::Good
                            },
                        ),
                        SummaryChip::new("Memory", format!("{mem:.0}%")).tone(
                            if mem >= 90.0 {
                                Tone::Bad
                            } else if mem >= 75.0 {
                                Tone::Warn
                            } else {
                                Tone::Good
                            },
                        ),
                        SummaryChip::new("Disk", format!("{disk:.1} MB/s")).tone(Tone::Accent),
                        SummaryChip::new("Network", format!("{net:.1} MB/s")).tone(Tone::Accent),
                    ],
                    gauges: vec![
                        Gauge::new("Processor", cpu, format!("{cpu:.1} %")),
                        Gauge::new("Memory", mem, mem_disp),
                        Gauge::new("Disk I/O", throughput_bar(disk), format!("{disk:.2} MB/s"))
                            .tone(Tone::Accent),
                        Gauge::new("Network", throughput_bar(net), format!("{net:.2} MB/s"))
                            .tone(Tone::Accent),
                    ],
                    columns: vec!["Counter".into(), "Instance".into(), "Value".into()],
                    column_weights: vec![3, 2, 2],
                    rows: vec![
                        ResultRow {
                            id: "cpu".into(),
                            icon: "cpu".into(),
                            cells: vec![
                                "Processor".into(),
                                "_Total".into(),
                                format!("{cpu:.1} %"),
                            ],
                        },
                        ResultRow {
                            id: "mem".into(),
                            icon: "media-memory".into(),
                            cells: vec![
                                "Memory".into(),
                                "Committed".into(),
                                format!("{mem:.1} %"),
                            ],
                        },
                        ResultRow {
                            id: "disk".into(),
                            icon: "drive-harddisk".into(),
                            cells: vec![
                                "Physical Disk".into(),
                                "_Total".into(),
                                format!("{disk:.2} MB/s"),
                            ],
                        },
                        ResultRow {
                            id: "net".into(),
                            icon: "network-wired".into(),
                            cells: vec![
                                "Network Interface".into(),
                                "_Total".into(),
                                format!("{net:.2} MB/s"),
                            ],
                        },
                    ],
                    detail: format!(
                        "Live sample · Ctrl+Shift+Esc opens this view.\n\
                         Press Refresh (or F5) for the next sample. First sample after open may read 0% until a delta is available.\n\
                         Load: {}",
                        loadavg_line()
                    ),
                    status: format!("CPU {cpu:.0}% · Mem {mem:.0}% · Disk {disk:.1} · Net {net:.1}"),
                    ..Default::default()
                }
            }
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Sample now"),
            ActionItem::new("clear", "Clear display"),
            ActionItem::new("uptime", "Show uptime"),
        ]
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        _selection: Option<&str>,
        _confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" | "clear" => Ok("Sampled".into()),
            "uptime" => Ok(uptime_line()),
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}
