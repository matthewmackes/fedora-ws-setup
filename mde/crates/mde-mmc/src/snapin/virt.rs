//! Virtualization snap-in — libvirt (W486).

use std::process::Command;

use super::SnapIn;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Virt;

impl SnapIn for Virt {
    fn key(&self) -> &'static str {
        "virtualization"
    }
    fn name(&self) -> &'static str {
        "Virtualization"
    }
    fn icon(&self) -> &'static str {
        "virtual-machine"
    }
    fn description(&self) -> &'static str {
        "libvirt VMs, networks, and storage"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("virtualization", "Virtualization", "virtual-machine", "virtualization")
            .with_children(vec![
                TreeNode::new("virt-vms", "Virtual Machines", "virtual-machine", "virtualization"),
                TreeNode::new("virt-nets", "Networks", "network-workgroup", "virtualization"),
                TreeNode::new("virt-pools", "Storage pools", "drive-harddisk", "virtualization"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        let (args, cols) = match node_id {
            "virt-nets" => (
                vec!["net-list", "--all"],
                vec!["Name", "State", "Autostart", "Persistent"],
            ),
            "virt-pools" => (
                vec!["pool-list", "--all"],
                vec!["Name", "State", "Autostart"],
            ),
            _ => (
                vec!["list", "--all"],
                vec!["Id", "Name", "State"],
            ),
        };
        let text = cmd("virsh", &args);
        let rows: Vec<ResultRow> = text
            .lines()
            .skip(2)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let cells: Vec<String> = l.split_whitespace().map(str::to_string).collect();
                ResultRow {
                    id: cells.get(1).cloned().unwrap_or_else(|| cells.first().cloned().unwrap_or_default()),
                    icon: "virtual-machine".into(),
                    cells,
                }
            })
            .collect();
        ResultsView {
            columns: cols.into_iter().map(str::to_string).collect(),
            rows,
            detail: if text.contains("failed") || text.contains("No such file") {
                "Install libvirt / virsh to manage VMs here.".into()
            } else {
                String::new()
            },
            status: "libvirt".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("start", "Start"));
            a.push(ActionItem::new("shutdown", "Shut down"));
            a.push(ActionItem::new("destroy", "Force off").destructive());
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
            "start" | "shutdown" | "destroy" => {
                let name = selection.ok_or("Select a VM")?;
                if action_id == "destroy" && confirm_token != Some(name) {
                    return Err(format!("Type “{name}” to force off"));
                }
                run_virsh(&[action_id, name])
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

fn run_virsh(args: &[&str]) -> Result<String, String> {
    let o = Command::new("virsh")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().into())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().into())
    }
}
