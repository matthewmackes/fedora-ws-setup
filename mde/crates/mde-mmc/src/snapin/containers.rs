//! Containers snap-in — podman (W485).

use std::process::Command;

use super::SnapIn;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Containers;

impl SnapIn for Containers {
    fn key(&self) -> &'static str {
        "containers"
    }
    fn name(&self) -> &'static str {
        "Containers"
    }
    fn icon(&self) -> &'static str {
        "package-x-generic"
    }
    fn description(&self) -> &'static str {
        "Podman pods, images, and volumes"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("containers", "Containers", "package-x-generic", "containers")
            .with_children(vec![
                TreeNode::new("containers-ps", "Containers", "view-list", "containers"),
                TreeNode::new("containers-img", "Images", "folder", "containers"),
                TreeNode::new("containers-vol", "Volumes", "drive-harddisk", "containers"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        let (bin_args, cols) = match node_id {
            "containers-img" => (
                vec!["images", "--format", "{{.ID}}\\t{{.Repository}}:{{.Tag}}\\t{{.Size}}"],
                vec!["ID", "Image", "Size"],
            ),
            "containers-vol" => (
                vec!["volume", "ls", "--format", "{{.Name}}\\t{{.Driver}}\\t{{.Mountpoint}}"],
                vec!["Name", "Driver", "Mount"],
            ),
            _ => (
                vec![
                    "ps",
                    "-a",
                    "--format",
                    "{{.ID}}\\t{{.Names}}\\t{{.Status}}\\t{{.Image}}",
                ],
                vec!["ID", "Name", "Status", "Image"],
            ),
        };
        let text = cmd("podman", &bin_args);
        let rows: Vec<ResultRow> = text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let cells: Vec<String> = l.split('\t').map(str::to_string).collect();
                ResultRow {
                    id: cells.first().cloned().unwrap_or_default(),
                    icon: "package-x-generic".into(),
                    cells,
                }
            })
            .collect();
        ResultsView {
            columns: cols.into_iter().map(str::to_string).collect(),
            rows,
            status: "podman".into(),
            detail: if text.contains("command not found") || text.contains("No such file") {
                "Install podman to manage containers here.".into()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("start", "Start"));
            a.push(ActionItem::new("stop", "Stop"));
            a.push(ActionItem::new("rm", "Remove").destructive());
        }
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
            "refresh" => Ok("Refreshed".into()),
            "start" | "stop" => {
                let id = selection.ok_or("Select a container")?;
                run_podman(&[action_id, id])
            }
            "rm" => {
                let id = selection.ok_or("Select a container")?;
                if confirm_token != Some(id) {
                    return Err(format!("Type “{id}” to confirm remove"));
                }
                run_podman(&["rm", "-f", id])
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

fn run_podman(args: &[&str]) -> Result<String, String> {
    let o = Command::new("podman")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().into())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().into())
    }
}
