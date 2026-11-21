//! Task Scheduler — systemd timers + cron listing.

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Tasks;

fn timers(user: bool) -> Vec<ResultRow> {
    let mut cmd = Command::new("systemctl");
    if user {
        cmd.arg("--user");
    }
    let out = cmd
        .args(["list-timers", "--all", "--no-pager", "-o", "json"])
        .output();
    let Ok(o) = out else {
        return Vec::new();
    };
    let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout) else {
        return Vec::new();
    };
    arr.into_iter()
        .filter_map(|t| {
            let unit = t.get("unit")?.as_str()?.to_string();
            let next = t
                .get("next")
                .and_then(|v| v.as_str())
                .or_else(|| t.get("next_elapse_realtime_usec").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let last = t
                .get("last")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let activates = t
                .get("activates")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(ResultRow {
                id: format!("{}:{}", if user { "user" } else { "system" }, unit),
                icon: "x-office-calendar".into(),
                cells: vec![
                    unit,
                    if user { "User" } else { "System" }.into(),
                    next,
                    last,
                    activates,
                ],
            })
        })
        .collect()
}

impl SnapIn for Tasks {
    fn key(&self) -> &'static str {
        "taskschd"
    }
    fn name(&self) -> &'static str {
        "Task Scheduler"
    }
    fn icon(&self) -> &'static str {
        "x-office-calendar"
    }
    fn description(&self) -> &'static str {
        "Schedule tasks to run automatically"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("taskschd", "Task Scheduler", "x-office-calendar", "taskschd")
            .with_children(vec![
                TreeNode::new("taskschd-sys", "System timers", "system-run", "taskschd"),
                TreeNode::new("taskschd-user", "User timers", "system-users", "taskschd"),
                TreeNode::new("taskschd-cron", "Cron / anacron", "text-x-generic", "taskschd"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "taskschd-cron" => ResultsView {
                detail: format!(
                    "User crontab:\n{}\n\n/etc/cron.*:\n{}",
                    cmd("crontab", &["-l"]),
                    cmd("ls", &["-la", "/etc/cron.d", "/etc/cron.daily"])
                ),
                status: "Cron".into(),
                ..Default::default()
            },
            "taskschd-user" => ResultsView {
                columns: vec![
                    "Timer".into(),
                    "Scope".into(),
                    "Next".into(),
                    "Last".into(),
                    "Activates".into(),
                ],
                column_weights: vec![3, 1, 2, 2, 3],
                rows: timers(true),
                status: "User timers".into(),
                detail: String::new(),
                ..Default::default()
            },
            _ => ResultsView {
                columns: vec![
                    "Timer".into(),
                    "Scope".into(),
                    "Next".into(),
                    "Last".into(),
                    "Activates".into(),
                ],
                column_weights: vec![3, 1, 2, 2, 3],
                rows: timers(false),
                status: "System timers".into(),
                detail: String::new(),
                ..Default::default()
            },
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.push(ActionItem::new("start", "Run"));
            a.push(ActionItem::new("stop", "Stop"));
            a.push(ActionItem::new("enable", "Enable"));
            a.push(ActionItem::new("disable", "Disable"));
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
        if action_id == "refresh" {
            return Ok("Refreshed timers".into());
        }
        let id = selection.ok_or("Select a timer")?;
        let (scope, unit) = id
            .split_once(':')
            .ok_or("Bad selection")?;
        if scope == "user" {
            // User timers: try without elevation first.
            let st = Command::new("systemctl")
                .arg("--user")
                .args([&action_id, unit])
                .output()
                .map_err(|e| e.to_string())?;
            if st.status.success() {
                return Ok("OK".into());
            }
            return Err(String::from_utf8_lossy(&st.stderr).into());
        }
        let msg = helper::systemctl(&[action_id, unit])?;
        helper::audit(
            &std::env::var("USER").unwrap_or_else(|_| "?".into()),
            action_id,
            unit,
            "taskschd",
        );
        Ok(msg)
    }
}

fn cmd(bin: &str, args: &[&str]) -> String {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).into_owned()
            } else {
                String::from_utf8_lossy(&o.stderr).into_owned()
            }
        })
        .unwrap_or_default()
}
