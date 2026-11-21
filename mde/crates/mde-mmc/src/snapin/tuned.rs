//! Tuned / Power snap-in (W491).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Tuned;

impl SnapIn for Tuned {
    fn key(&self) -> &'static str {
        "tuned"
    }
    fn name(&self) -> &'static str {
        "Tuned / Power"
    }
    fn icon(&self) -> &'static str {
        "battery"
    }
    fn description(&self) -> &'static str {
        "tuned profiles and power behaviours"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("tuned", "Tuned / Power", "battery", "tuned")
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        let active = cmd("tuned-adm", &["active"]);
        let list = cmd("tuned-adm", &["list"]);
        let rows: Vec<ResultRow> = list
            .lines()
            .filter(|l| l.starts_with('-'))
            .map(|l| {
                let name = l.trim().trim_start_matches('-').trim().to_string();
                ResultRow {
                    id: name.clone(),
                    icon: "battery".into(),
                    cells: vec![name],
                }
            })
            .collect();
        ResultsView {
            columns: vec!["Profile".into()],
            rows,
            detail: active,
            status: "tuned".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("profile", "Activate profile"));
        }
        a
    }
    fn run_action(
        &self,
        _: &str,
        action_id: &str,
        selection: Option<&str>,
        _: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "profile" => {
                let p = selection.ok_or("Select a profile")?;
                helper::run(&["tuned-profile", p])
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
