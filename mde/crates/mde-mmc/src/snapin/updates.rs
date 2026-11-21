//! Software Updates snap-in (W483) — coordinates with PackageKit / dnf.

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultsView, TreeNode};

pub struct Updates;

impl SnapIn for Updates {
    fn key(&self) -> &'static str {
        "updates"
    }
    fn name(&self) -> &'static str {
        "Software Updates"
    }
    fn icon(&self) -> &'static str {
        "system-software-update"
    }
    fn description(&self) -> &'static str {
        "Check and prepare system updates (with Settings › Windows Update)"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("updates", "Software Updates", "system-software-update", "updates")
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        ResultsView {
            detail: format!(
                "dnf check-update (excerpt):\n{}\n\nOpen Settings › Windows Update for the consumer UX; this snap-in is the admin path.",
                cmd("dnf", &["check-update", "-q"])
            ),
            status: "Updates".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _: &str, _: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Check for updates"),
            ActionItem::new("upgrade", "Upgrade all").destructive(),
            ActionItem::new("settings", "Open Windows Update page"),
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
            "refresh" => Ok(cmd("dnf", &["check-update", "-q"])),
            "upgrade" => {
                if confirm_token != Some("UPGRADE") {
                    return Err("Type UPGRADE to confirm".into());
                }
                helper::run(&["dnf-upgrade"])
            }
            "settings" => {
                let _ = Command::new("mde-control").arg("update").spawn();
                Ok("Opened Settings › Windows Update".into())
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
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            if s.trim().is_empty() {
                s = String::from_utf8_lossy(&o.stderr).into_owned();
            }
            // dnf check-update exits 100 when updates available
            s.chars().take(4000).collect()
        })
        .unwrap_or_else(|e| e.to_string())
}
