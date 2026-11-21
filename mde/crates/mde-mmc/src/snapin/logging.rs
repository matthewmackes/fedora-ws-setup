//! Logging Config snap-in (W489).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultsView, TreeNode};

pub struct Logging;

impl SnapIn for Logging {
    fn key(&self) -> &'static str {
        "logging"
    }
    fn name(&self) -> &'static str {
        "Logging Config"
    }
    fn icon(&self) -> &'static str {
        "accessories-text-editor"
    }
    fn description(&self) -> &'static str {
        "journald persistence, vacuum, forward"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("logging", "Logging Config", "accessories-text-editor", "logging")
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        ResultsView {
            detail: format!(
                "journalctl --disk-usage:\n{}\n\n/etc/systemd/journald.conf:\n{}",
                cmd("journalctl", &["--disk-usage"]),
                std::fs::read_to_string("/etc/systemd/journald.conf").unwrap_or_default()
            ),
            status: "journald".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Refresh"),
            ActionItem::new("vacuum", "Vacuum to 200M").destructive(),
        ]
    }
    fn run_action(
        &self,
        _: &str,
        action_id: &str,
        _: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "vacuum" => {
                if confirm_token != Some("VACUUM") {
                    return Err("Type VACUUM to confirm journal vacuum".into());
                }
                helper::run(&["journal-vacuum", "200M"])
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
