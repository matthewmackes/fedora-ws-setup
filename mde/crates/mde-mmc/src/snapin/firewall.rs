//! Firewall snap-in (firewalld).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{
    ActionItem, Property, ResultRow, ResultsView, SummaryChip, Tone, TreeNode,
};

pub struct Firewall;

impl SnapIn for Firewall {
    fn key(&self) -> &'static str {
        "firewall"
    }
    fn name(&self) -> &'static str {
        "Firewall"
    }
    fn icon(&self) -> &'static str {
        "preferences-system-network"
    }
    fn description(&self) -> &'static str {
        "firewalld zones and rules"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("firewall", "Firewall", "preferences-system-network", "firewall")
            .with_children(vec![
                TreeNode::new(
                    "firewall-state",
                    "Status",
                    "preferences-system-network",
                    "firewall",
                ),
                TreeNode::new("firewall-zones", "Zones", "folder", "firewall"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        if node_id == "firewall-zones" {
            let zones = cmd("firewall-cmd", &["--get-zones"]);
            let active = cmd("firewall-cmd", &["--get-active-zones"]);
            // firewall-cmd --get-active-zones prints zone names on their own lines,
            // with "interfaces: …" / "sources: …" indented underneath.
            let active_names: Vec<&str> = active
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.contains(':'))
                .collect();
            let rows: Vec<ResultRow> = zones
                .split_whitespace()
                .map(|z| {
                    let is_active = active_names.iter().any(|a| *a == z);
                    ResultRow {
                        id: z.to_string(),
                        icon: if is_active {
                            "security-high".into()
                        } else {
                            "folder".into()
                        },
                        cells: vec![
                            z.to_string(),
                            if is_active {
                                "Active".into()
                            } else {
                                "Available".into()
                            },
                        ],
                    }
                })
                .collect();
            return ResultsView {
                summaries: vec![
                    SummaryChip::new("Zones", rows.len().to_string()),
                    SummaryChip::new("Active", active_names.len().to_string()).tone(Tone::Good),
                ],
                columns: vec!["Zone".into(), "State".into()],
                column_weights: vec![3, 2],
                rows,
                detail: if active.trim().is_empty() {
                    String::new()
                } else {
                    format!("Active zone detail:\n{active}")
                },
                status: "Zones".into(),
                ..Default::default()
            };
        }

        let state = cmd("firewall-cmd", &["--state"]);
        let default_zone = cmd("firewall-cmd", &["--get-default-zone"]);
        let running = state.trim().eq_ignore_ascii_case("running");
        ResultsView {
            summaries: vec![
                SummaryChip::new("Service", if running { "Running" } else { state.trim() })
                    .tone(if running { Tone::Good } else { Tone::Bad }),
                SummaryChip::new("Default zone", default_zone.trim()).tone(Tone::Accent),
            ],
            properties: vec![
                Property::new("State", state.trim()),
                Property::new("Default zone", default_zone.trim()),
                Property::new(
                    "Backend",
                    "firewalld (nftables/iptables via firewall-cmd)",
                ),
            ],
            status: "Firewall".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Refresh"),
            ActionItem::new("reload", "Reload firewall"),
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
            "refresh" => Ok("Refreshed".into()),
            "reload" => {
                let msg = helper::run(&["firewall-reload"])?;
                Ok(msg)
            }
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}

fn cmd(bin: &str, args: &[&str]) -> String {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
        .unwrap_or_else(|e| e.to_string())
}
