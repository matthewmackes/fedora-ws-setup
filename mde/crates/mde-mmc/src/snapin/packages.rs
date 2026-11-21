//! Package Management snap-in (dnf).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Packages;

impl SnapIn for Packages {
    fn key(&self) -> &'static str {
        "packages"
    }
    fn name(&self) -> &'static str {
        "Package Management"
    }
    fn icon(&self) -> &'static str {
        "system-software-install"
    }
    fn description(&self) -> &'static str {
        "Search, install, remove, repos, history"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("packages", "Package Management", "system-software-install", "packages")
            .with_children(vec![
                TreeNode::new("packages-installed", "Installed", "view-list", "packages"),
                TreeNode::new("packages-repos", "Repositories", "folder", "packages"),
                TreeNode::new("packages-history", "History", "document-open-recent", "packages"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "packages-repos" => ResultsView {
                detail: cmd("dnf", &["repolist", "-q"]),
                status: "Repos".into(),
                ..Default::default()
            },
            "packages-history" => ResultsView {
                detail: cmd("dnf", &["history", "list", "-q"]),
                status: "History".into(),
                ..Default::default()
            },
            _ => {
                // Cap list for UI responsiveness.
                let text = cmd("rpm", &["-qa", "--qf", "%{NAME}\\t%{VERSION}-%{RELEASE}\\t%{ARCH}\\n"]);
                let mut rows: Vec<ResultRow> = text
                    .lines()
                    .take(500)
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        let p: Vec<&str> = l.split('\t').collect();
                        ResultRow {
                            id: p.first().unwrap_or(&"").to_string(),
                            icon: "package-x-generic".into(),
                            cells: vec![
                                p.first().unwrap_or(&"").to_string(),
                                p.get(1).unwrap_or(&"").to_string(),
                                p.get(2).unwrap_or(&"").to_string(),
                            ],
                        }
                    })
                    .collect();
                rows.sort_by(|a, b| a.cells[0].cmp(&b.cells[0]));
                let n = rows.len();
                ResultsView {
                    columns: vec!["Name".into(), "Version".into(), "Arch".into()],
                    column_weights: vec![3, 2, 1],
                    rows,
                    status: format!("Installed (showing {n})"),
                    detail: String::new(),
                    ..Default::default()
                }
            }
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("remove", "Remove…").destructive());
        }
        a.push(ActionItem::new("upgrade", "Upgrade all"));
        a
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        selection: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String> {
        let who = std::env::var("USER").unwrap_or_else(|_| "?".into());
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "remove" => {
                let pkg = selection.ok_or("Select a package")?;
                if confirm_token != Some(pkg) {
                    return Err(format!("Type the package name “{pkg}” to confirm remove"));
                }
                let msg = helper::run(&["dnf-remove", pkg])?;
                helper::audit(&who, "dnf-remove", pkg, "packages");
                Ok(msg)
            }
            "upgrade" => {
                if confirm_token != Some("UPGRADE") {
                    return Err("Type UPGRADE to confirm system upgrade".into());
                }
                let msg = helper::run(&["dnf-upgrade"])?;
                helper::audit(&who, "dnf-upgrade", "-", "packages");
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
