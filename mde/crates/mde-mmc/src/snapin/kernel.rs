//! Kernel & Modules snap-in (W490).

use std::process::Command;

use super::SnapIn;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Kernel;

impl SnapIn for Kernel {
    fn key(&self) -> &'static str {
        "kernel"
    }
    fn name(&self) -> &'static str {
        "Kernel & Modules"
    }
    fn icon(&self) -> &'static str {
        "applications-system"
    }
    fn description(&self) -> &'static str {
        "Loaded modules, blacklist, cmdline"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("kernel", "Kernel & Modules", "applications-system", "kernel")
            .with_children(vec![
                TreeNode::new("kernel-mods", "Loaded modules", "view-list", "kernel"),
                TreeNode::new("kernel-cmd", "Cmdline", "text-x-generic", "kernel"),
                TreeNode::new("kernel-black", "Blacklist", "dialog-warning", "kernel"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "kernel-cmd" => ResultsView {
                detail: std::fs::read_to_string("/proc/cmdline").unwrap_or_default(),
                status: "cmdline".into(),
                ..Default::default()
            },
            "kernel-black" => ResultsView {
                detail: cmd("ls", &["-la", "/etc/modprobe.d"]),
                status: "blacklist".into(),
                ..Default::default()
            },
            _ => {
                let text = cmd("lsmod", &[]);
                let rows: Vec<ResultRow> = text
                    .lines()
                    .skip(1)
                    .take(400)
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        let p: Vec<&str> = l.split_whitespace().collect();
                        ResultRow {
                            id: p.first().unwrap_or(&"").to_string(),
                            icon: "applications-system".into(),
                            cells: vec![
                                p.first().unwrap_or(&"").to_string(),
                                p.get(1).unwrap_or(&"").to_string(),
                                p.get(2).unwrap_or(&"").to_string(),
                            ],
                        }
                    })
                    .collect();
                ResultsView {
                    columns: vec!["Module".into(), "Size".into(), "Used".into()],
                    column_weights: vec![3, 1, 2],
                    rows,
                    status: "lsmod".into(),
                    detail: String::new(),
                    ..Default::default()
                }
            }
        }
    }
    fn actions(&self, _: &str, _: Option<&str>) -> Vec<ActionItem> {
        vec![ActionItem::new("refresh", "Refresh")]
    }
    fn run_action(
        &self,
        _: &str,
        action_id: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
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
