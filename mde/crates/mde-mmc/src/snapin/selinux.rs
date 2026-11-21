//! SELinux snap-in (W484).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{
    ActionItem, Property, ResultRow, ResultsView, SummaryChip, Tone, TreeNode,
};

pub struct Selinux;

impl SnapIn for Selinux {
    fn key(&self) -> &'static str {
        "selinux"
    }
    fn name(&self) -> &'static str {
        "SELinux"
    }
    fn icon(&self) -> &'static str {
        "security-medium"
    }
    fn description(&self) -> &'static str {
        "Mode, booleans, file contexts, AVC triage"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("selinux", "SELinux", "security-medium", "selinux")
            .with_children(vec![
                TreeNode::new("selinux-mode", "Mode", "security-high", "selinux"),
                TreeNode::new("selinux-bool", "Booleans", "view-list", "selinux"),
                TreeNode::new("selinux-avc", "AVC denials", "dialog-warning", "selinux"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "selinux-bool" => {
                let text = cmd("getsebool", &["-a"]);
                let rows: Vec<ResultRow> = text
                    .lines()
                    .filter(|l| l.contains("-->"))
                    .map(|l| {
                        let (name, state) = l
                            .split_once("-->")
                            .map(|(a, b)| (a.trim(), b.trim()))
                            .unwrap_or((l, ""));
                        ResultRow {
                            id: name.to_string(),
                            icon: if state == "on" {
                                "security-high".into()
                            } else {
                                "security-low".into()
                            },
                            cells: vec![name.to_string(), state.to_string()],
                        }
                    })
                    .collect();
                let on = rows
                    .iter()
                    .filter(|r| r.cells.get(1).map(|s| s.as_str()) == Some("on"))
                    .count();
                let n = rows.len();
                ResultsView {
                    summaries: vec![
                        SummaryChip::new("Booleans", n.to_string()),
                        SummaryChip::new("On", on.to_string()).tone(Tone::Good),
                        SummaryChip::new("Off", (n.saturating_sub(on)).to_string()),
                    ],
                    columns: vec!["Boolean".into(), "State".into()],
                    column_weights: vec![4, 1],
                    rows,
                    status: "Booleans".into(),
                    detail: if n == 0 {
                        if text.trim().is_empty() {
                            "getsebool returned no data (is SELinux userspace installed?)".into()
                        } else {
                            text
                        }
                    } else {
                        String::new()
                    },
                    ..Default::default()
                }
            }
            "selinux-avc" => {
                let text = cmd("ausearch", &["-m", "avc", "-ts", "recent"]);
                let rows: Vec<ResultRow> = text
                    .lines()
                    .filter(|l| !l.trim().is_empty() && !l.starts_with("----"))
                    .enumerate()
                    .map(|(i, line)| ResultRow {
                        id: format!("avc:{i}"),
                        icon: "dialog-warning".into(),
                        cells: vec![line.to_string()],
                    })
                    .collect();
                let n = rows.len();
                ResultsView {
                    summaries: vec![SummaryChip::new("AVC lines", n.to_string()).tone(
                        if n > 0 {
                            Tone::Warn
                        } else {
                            Tone::Good
                        },
                    )],
                    columns: vec!["Record".into()],
                    rows,
                    status: "AVC".into(),
                    detail: if n == 0 {
                        "No recent AVC denials (or ausearch unavailable).".into()
                    } else {
                        String::new()
                    },
                    ..Default::default()
                }
            }
            _ => {
                let mode = cmd("getenforce", &[]).trim().to_string();
                let config = std::fs::read_to_string("/etc/selinux/config").unwrap_or_default();
                let mut props = vec![Property::new("Current mode", &mode)];
                for line in config.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        props.push(Property::new(k.trim(), v.trim()));
                    }
                }
                let tone = match mode.to_ascii_lowercase().as_str() {
                    "enforcing" => Tone::Good,
                    "permissive" => Tone::Warn,
                    "disabled" => Tone::Bad,
                    _ => Tone::Neutral,
                };
                ResultsView {
                    summaries: vec![SummaryChip::new("Mode", mode).tone(tone)],
                    properties: props,
                    status: "SELinux".into(),
                    ..Default::default()
                }
            }
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Refresh"),
            ActionItem::new("permissive", "Set permissive (until reboot)"),
            ActionItem::new("enforcing", "Set enforcing"),
        ]
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        _selection: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String> {
        let who = std::env::var("USER").unwrap_or_else(|_| "?".into());
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "permissive" | "enforcing" => {
                if confirm_token != Some(action_id) {
                    return Err(format!("Type “{action_id}” to confirm mode change"));
                }
                let msg = helper::run(&["selinux-mode", action_id])?;
                helper::audit(&who, "selinux-mode", action_id, "selinux");
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
        .map(|o| {
            if o.stdout.is_empty() {
                String::from_utf8_lossy(&o.stderr).into()
            } else {
                String::from_utf8_lossy(&o.stdout).into()
            }
        })
        .unwrap_or_else(|e| e.to_string())
}
