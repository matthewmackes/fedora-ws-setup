//! Network snap-in (NetworkManager).

use std::process::Command;

use super::SnapIn;
use crate::model::{ActionItem, ResultRow, ResultsView, SummaryChip, Tone, TreeNode};

pub struct Network;

impl SnapIn for Network {
    fn key(&self) -> &'static str {
        "network"
    }
    fn name(&self) -> &'static str {
        "Network"
    }
    fn icon(&self) -> &'static str {
        "network-workgroup"
    }
    fn description(&self) -> &'static str {
        "NetworkManager connections and devices"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("network", "Network", "network-workgroup", "network")
            .with_children(vec![
                TreeNode::new("network-dev", "Devices", "network-wired", "network"),
                TreeNode::new("network-conn", "Connections", "network-workgroup", "network"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        if node_id == "network-conn" {
            let text = cmd("nmcli", &["-t", "-f", "NAME,UUID,TYPE,DEVICE", "connection", "show"]);
            let rows: Vec<ResultRow> = text
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    let p: Vec<&str> = l.split(':').collect();
                    ResultRow {
                        id: p.get(1).unwrap_or(&"").to_string(),
                        icon: "network-workgroup".into(),
                        cells: vec![
                            p.first().unwrap_or(&"").to_string(),
                            p.get(2).unwrap_or(&"").to_string(),
                            p.get(3).unwrap_or(&"").to_string(),
                        ],
                    }
                })
                .collect();
            return ResultsView {
                summaries: vec![SummaryChip::new("Connections", rows.len().to_string())],
                columns: vec!["Name".into(), "Type".into(), "Device".into()],
                column_weights: vec![3, 2, 2],
                rows,
                status: "Connections".into(),
                detail: String::new(),
                ..Default::default()
            };
        }
        let text = cmd("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device", "status"]);
        let rows: Vec<ResultRow> = text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let p: Vec<&str> = l.split(':').collect();
                ResultRow {
                    id: p.first().unwrap_or(&"").to_string(),
                    icon: "network-wired".into(),
                    cells: vec![
                        p.first().unwrap_or(&"").to_string(),
                        p.get(1).unwrap_or(&"").to_string(),
                        p.get(2).unwrap_or(&"").to_string(),
                        p.get(3).unwrap_or(&"").to_string(),
                    ],
                }
            })
            .collect();
        let connected = rows
            .iter()
            .filter(|r| {
                r.cells
                    .get(2)
                    .map(|s| s.eq_ignore_ascii_case("connected"))
                    .unwrap_or(false)
            })
            .count();
        ResultsView {
            summaries: vec![
                SummaryChip::new("Devices", rows.len().to_string()),
                SummaryChip::new("Connected", connected.to_string()).tone(Tone::Good),
            ],
            columns: vec![
                "Device".into(),
                "Type".into(),
                "State".into(),
                "Connection".into(),
            ],
            column_weights: vec![2, 2, 2, 3],
            rows,
            status: "Devices".into(),
            detail: String::new(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("connect", "Connect"));
            a.push(ActionItem::new("disconnect", "Disconnect"));
        }
        a
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        selection: Option<&str>,
        _confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "connect" => {
                let id = selection.ok_or("Select a connection/device")?;
                run_nm(&["connection", "up", id])
            }
            "disconnect" => {
                let id = selection.ok_or("Select a connection/device")?;
                run_nm(&["device", "disconnect", id])
                    .or_else(|_| run_nm(&["connection", "down", id]))
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
        .unwrap_or_default()
}

fn run_nm(args: &[&str]) -> Result<String, String> {
    let o = Command::new("nmcli")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().into())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().into())
    }
}
