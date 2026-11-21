//! Local Users and Groups.

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Users;

fn passwd_users() -> Vec<ResultRow> {
    let Ok(text) = std::fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut p = l.split(':');
            let name = p.next()?.to_string();
            let _pw = p.next()?;
            let uid: u32 = p.next()?.parse().ok()?;
            let gid = p.next()?.to_string();
            let gecos = p.next()?.to_string();
            let home = p.next()?.to_string();
            let shell = p.next()?.to_string();
            if uid < 1000 && name != "root" {
                return None;
            }
            Some(ResultRow {
                id: name.clone(),
                icon: "system-users".into(),
                cells: vec![name, uid.to_string(), gid, gecos, home, shell],
            })
        })
        .collect()
}

fn groups() -> Vec<ResultRow> {
    let Ok(text) = std::fs::read_to_string("/etc/group") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut p = l.split(':');
            let name = p.next()?.to_string();
            let _pw = p.next()?;
            let gid: u32 = p.next()?.parse().ok()?;
            let members = p.next().unwrap_or("").to_string();
            Some(ResultRow {
                id: name.clone(),
                icon: "system-users".into(),
                cells: vec![name, gid.to_string(), members],
            })
        })
        .collect()
}

impl SnapIn for Users {
    fn key(&self) -> &'static str {
        "lusrmgr"
    }
    fn name(&self) -> &'static str {
        "Local Users and Groups"
    }
    fn icon(&self) -> &'static str {
        "system-users"
    }
    fn description(&self) -> &'static str {
        "Manage local users and groups"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("lusrmgr", "Local Users and Groups", "system-users", "lusrmgr")
            .with_children(vec![
                TreeNode::new("lusrmgr-users", "Users", "system-users", "lusrmgr"),
                TreeNode::new("lusrmgr-groups", "Groups", "system-users", "lusrmgr"),
                TreeNode::new("lusrmgr-sudo", "Sudoers / polkit", "security-high", "lusrmgr"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "lusrmgr-groups" => ResultsView {
                columns: vec!["Group".into(), "GID".into(), "Members".into()],
                column_weights: vec![2, 1, 4],
                rows: groups(),
                status: "Groups".into(),
                detail: String::new(),
                ..Default::default()
            },
            "lusrmgr-sudo" => ResultsView {
                detail: format!(
                    "Sudoers (read):\n{}\n\npolkit rules dir listing:\n{}",
                    cmd("cat", &["/etc/sudoers"]),
                    cmd("ls", &["-la", "/etc/polkit-1/rules.d"])
                ),
                status: "Identity policy".into(),
                ..Default::default()
            },
            _ => ResultsView {
                columns: vec![
                    "Name".into(),
                    "UID".into(),
                    "GID".into(),
                    "GECOS".into(),
                    "Home".into(),
                    "Shell".into(),
                ],
                column_weights: vec![2, 1, 1, 3, 3, 2],
                rows: passwd_users(),
                status: "Users".into(),
                detail: String::new(),
                ..Default::default()
            },
        }
    }
    fn actions(&self, node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if node_id == "lusrmgr-groups" {
            a.push(ActionItem::new("group-add", "New Group…"));
            if selection.is_some() {
                a.push(ActionItem::new("group-del", "Delete Group").destructive());
            }
        }
        if matches!(node_id, "lusrmgr-users" | "lusrmgr") {
            if let Some(_) = selection {
                a.push(ActionItem::new("expire", "Force password change"));
                a.push(ActionItem::new("never-expire", "Password never expires"));
            }
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
        let who = std::env::var("USER").unwrap_or_else(|_| "?".into());
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "expire" => {
                let u = selection.ok_or("Select a user")?;
                let msg = helper::run(&["user-expire", u])?;
                helper::audit(&who, "user-expire", u, "lusrmgr");
                Ok(msg)
            }
            "never-expire" => {
                let u = selection.ok_or("Select a user")?;
                let msg = helper::run(&["user-never-expires", u])?;
                helper::audit(&who, "user-never-expires", u, "lusrmgr");
                Ok(msg)
            }
            "group-del" => {
                let g = selection.ok_or("Select a group")?;
                if confirm_token != Some(g) {
                    return Err(format!("Type the group name “{g}” to confirm delete"));
                }
                let msg = helper::run(&["group-del", g])?;
                helper::audit(&who, "group-del", g, "lusrmgr");
                Ok(msg)
            }
            "group-add" => Err("Use Action › New Group with a name in a later sheet (scaffold)".into()),
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}

fn cmd(bin: &str, args: &[&str]) -> String {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}
