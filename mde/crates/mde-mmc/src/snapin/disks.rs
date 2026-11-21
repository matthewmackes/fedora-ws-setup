//! Disk Management — lsblk inventory + UDisks/helper mutations.

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{
    ActionItem, Gauge, Property, ResultRow, ResultsView, SummaryChip, Tone, TreeNode,
};

pub struct Disks;

fn lsblk_rows() -> Result<Vec<ResultRow>, String> {
    let out = Command::new("lsblk")
        .args([
            "-J",
            "-o",
            "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,UUID,MODEL",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    fn walk(node: &serde_json::Value, rows: &mut Vec<ResultRow>, depth: usize) {
        let name = node.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let size = node.get("size").and_then(|x| x.as_str()).unwrap_or("");
        let typ = node.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let fstype = node.get("fstype").and_then(|x| x.as_str()).unwrap_or("");
        let mp = node
            .get("mountpoint")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let model = node.get("model").and_then(|x| x.as_str()).unwrap_or("");
        let pad = "  ".repeat(depth);
        rows.push(ResultRow {
            id: format!("/dev/{name}"),
            icon: if typ == "disk" {
                "drive-harddisk".into()
            } else {
                "drive-removable-media".into()
            },
            cells: vec![
                format!("{pad}{name}"),
                size.into(),
                typ.into(),
                fstype.into(),
                mp.into(),
                model.into(),
            ],
        });
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for c in children {
                walk(c, rows, depth + 1);
            }
        }
    }
    if let Some(arr) = v.get("blockdevices").and_then(|a| a.as_array()) {
        for d in arr {
            walk(d, &mut rows, 0);
        }
    }
    Ok(rows)
}

impl SnapIn for Disks {
    fn key(&self) -> &'static str {
        "diskmgmt"
    }
    fn name(&self) -> &'static str {
        "Disk Management"
    }
    fn icon(&self) -> &'static str {
        "drive-harddisk"
    }
    fn description(&self) -> &'static str {
        "Manage disks, partitions and volumes"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("diskmgmt", "Disk Management", "drive-harddisk", "diskmgmt")
            .with_desc(self.description())
            .with_children(vec![
                TreeNode::new("diskmgmt-volumes", "Volumes", "drive-harddisk", "diskmgmt"),
                TreeNode::new("diskmgmt-lvm", "LVM", "drive-harddisk", "diskmgmt"),
                TreeNode::new("diskmgmt-raid", "RAID / mdadm", "drive-harddisk", "diskmgmt"),
                TreeNode::new("diskmgmt-luks", "LUKS", "security-high", "diskmgmt"),
                TreeNode::new("diskmgmt-remote", "iSCSI / NFS", "network-server", "diskmgmt"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "diskmgmt-lvm" => lvm_view(),
            "diskmgmt-raid" => ResultsView {
                detail: read_file("/proc/mdstat"),
                status: "RAID".into(),
                properties: vec![Property::new(
                    "Source",
                    "/proc/mdstat — mdadm arrays on this host",
                )],
                ..Default::default()
            },
            "diskmgmt-luks" => {
                let rows = crypto_rows();
                ResultsView {
                    columns: vec![
                        "Name".into(),
                        "Type".into(),
                        "FS".into(),
                        "Mount".into(),
                    ],
                    column_weights: vec![3, 2, 2, 3],
                    rows,
                    status: "LUKS / crypto".into(),
                    ..Default::default()
                }
            }
            "diskmgmt-remote" => remote_view(),
            _ => match lsblk_rows() {
                Ok(rows) => {
                    let disks = rows.iter().filter(|r| r.cells.get(2).map(|s| s.as_str()) == Some("disk")).count();
                    let parts = rows.iter().filter(|r| r.cells.get(2).map(|s| s.as_str()) == Some("part")).count();
                    let (used_pct, used_disp) = df_root();
                    ResultsView {
                        summaries: vec![
                            SummaryChip::new("Disks", disks.to_string()),
                            SummaryChip::new("Partitions", parts.to_string()),
                            SummaryChip::new("Root FS", used_disp.clone()).tone(
                                if used_pct >= 90.0 {
                                    Tone::Bad
                                } else if used_pct >= 80.0 {
                                    Tone::Warn
                                } else {
                                    Tone::Good
                                },
                            ),
                        ],
                        gauges: if used_pct > 0.0 {
                            vec![Gauge::new("Root filesystem", used_pct, used_disp)]
                        } else {
                            Vec::new()
                        },
                        columns: vec![
                            "Name".into(),
                            "Size".into(),
                            "Type".into(),
                            "FS".into(),
                            "Mount".into(),
                            "Model".into(),
                        ],
                        column_weights: vec![3, 2, 2, 2, 3, 3],
                        rows,
                        detail: String::new(),
                        status: "Disks".into(),
                        ..Default::default()
                    }
                }
                Err(e) => ResultsView {
                    detail: e.clone(),
                    status: e,
                    ..Default::default()
                },
            },
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if let Some(dev) = selection {
            if dev.starts_with("/dev/") {
                a.push(ActionItem::new("trim", "Optimize Drives (TRIM)"));
                a.push(
                    ActionItem::new("format_warn", "Format…")
                        .destructive()
                        .disabled(),
                );
            }
        }
        a.push(ActionItem::new("rescan", "Rescan Disks"));
        a
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        selection: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" | "rescan" => Ok("Disk list refreshed".into()),
            "trim" => {
                let mp = selection.ok_or("Select a mounted volume")?;
                // Prefer mountpoint; if /dev path, refuse without mount.
                if mp.starts_with("/dev/") {
                    return Err("Select a row with a mount point, or pass mount path".into());
                }
                if confirm_token != Some(mp) {
                    return Err(format!("Type the mount path “{mp}” to confirm TRIM"));
                }
                let msg = helper::run(&["trim", mp])?;
                helper::audit(
                    &std::env::var("USER").unwrap_or_else(|_| "?".into()),
                    "trim",
                    mp,
                    "diskmgmt",
                );
                Ok(msg)
            }
            _ => Err(format!("Unknown or disabled action {action_id}")),
        }
    }
}

fn lvm_view() -> ResultsView {
    let mut rows = Vec::new();
    for line in cmd_text("pvs", &["--noheadings", "-o", "pv_name,vg_name,pv_size,pv_free"]).lines()
    {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 4 {
            continue;
        }
        rows.push(ResultRow {
            id: format!("pv:{}", p[0]),
            icon: "drive-harddisk".into(),
            cells: vec![
                "PV".into(),
                p[0].into(),
                p[1].into(),
                p[2].into(),
                p[3].into(),
            ],
        });
    }
    for line in cmd_text("vgs", &["--noheadings", "-o", "vg_name,vg_size,vg_free"]).lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 3 {
            continue;
        }
        rows.push(ResultRow {
            id: format!("vg:{}", p[0]),
            icon: "folder".into(),
            cells: vec![
                "VG".into(),
                p[0].into(),
                "—".into(),
                p[1].into(),
                p[2].into(),
            ],
        });
    }
    for line in cmd_text("lvs", &["--noheadings", "-o", "lv_name,vg_name,lv_size"]).lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 3 {
            continue;
        }
        rows.push(ResultRow {
            id: format!("lv:{}:{}", p[1], p[0]),
            icon: "drive-removable-media".into(),
            cells: vec![
                "LV".into(),
                p[0].into(),
                p[1].into(),
                p[2].into(),
                "—".into(),
            ],
        });
    }
    let n = rows.len();
    ResultsView {
        columns: vec![
            "Kind".into(),
            "Name".into(),
            "Group".into(),
            "Size".into(),
            "Free".into(),
        ],
        column_weights: vec![1, 3, 2, 2, 2],
        rows,
        summaries: vec![SummaryChip::new("LVM objects", n.to_string())],
        status: "LVM".into(),
        detail: if n == 0 {
            "No LVM physical volumes, volume groups, or logical volumes reported.".into()
        } else {
            String::new()
        },
        ..Default::default()
    }
}

fn crypto_rows() -> Vec<ResultRow> {
    let text = cmd_text("lsblk", &["-J", "-o", "NAME,TYPE,FSTYPE,MOUNTPOINT"]);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    fn walk(node: &serde_json::Value, rows: &mut Vec<ResultRow>) {
        let name = node.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let typ = node.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let fstype = node.get("fstype").and_then(|x| x.as_str()).unwrap_or("");
        let mp = node
            .get("mountpoint")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if fstype.contains("crypto") || typ == "crypt" || fstype == "crypto_LUKS" {
            rows.push(ResultRow {
                id: format!("/dev/{name}"),
                icon: "security-high".into(),
                cells: vec![name.into(), typ.into(), fstype.into(), mp.into()],
            });
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for c in children {
                walk(c, rows);
            }
        }
    }
    if let Some(arr) = v.get("blockdevices").and_then(|a| a.as_array()) {
        for d in arr {
            walk(d, &mut rows);
        }
    }
    rows
}

fn remote_view() -> ResultsView {
    let mounts = cmd_text(
        "findmnt",
        &["-t", "nfs,nfs4,cifs", "-o", "TARGET,SOURCE,FSTYPE", "-n"],
    );
    let mut rows: Vec<ResultRow> = mounts
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            let p: Vec<&str> = line.split_whitespace().collect();
            ResultRow {
                id: format!("mnt:{i}"),
                icon: "network-server".into(),
                cells: vec![
                    p.first().unwrap_or(&"").to_string(),
                    p.get(1).unwrap_or(&"").to_string(),
                    p.get(2).unwrap_or(&"").to_string(),
                ],
            }
        })
        .collect();
    let iscsi = cmd_text("iscsiadm", &["-m", "session"]);
    if !iscsi.is_empty() && !iscsi.contains("No active") && !iscsi.contains("iscsiadm:") {
        for (i, line) in iscsi.lines().enumerate() {
            rows.push(ResultRow {
                id: format!("iscsi:{i}"),
                icon: "network-wired".into(),
                cells: vec![line.to_string(), "iSCSI".into(), "session".into()],
            });
        }
    }
    ResultsView {
        columns: vec!["Target / Path".into(), "Source".into(), "Type".into()],
        column_weights: vec![3, 3, 1],
        rows,
        status: "Remote storage".into(),
        detail: if mounts.is_empty() && (iscsi.is_empty() || iscsi.contains("No active")) {
            "No NFS/CIFS mounts or iSCSI sessions detected.".into()
        } else {
            String::new()
        },
        ..Default::default()
    }
}

fn df_root() -> (f32, String) {
    let text = cmd_text("df", &["-h", "--output=pcent,used,size,target", "/"]);
    for line in text.lines().skip(1) {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 4 {
            continue;
        }
        let pct = p[0].trim_end_matches('%').parse::<f32>().unwrap_or(0.0);
        return (pct, format!("{} used of {} ({})", p[1], p[2], p[0]));
    }
    (0.0, "—".into())
}

fn cmd_text(bin: &str, args: &[&str]) -> String {
    match Command::new(bin).args(args).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().into(),
        Ok(o) => String::from_utf8_lossy(&o.stderr).trim().into(),
        Err(e) => format!("{bin}: {e}"),
    }
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| format!("{path}: {e}"))
}
