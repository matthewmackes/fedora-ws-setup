//! Event Viewer — journalctl views (writable clear via helper).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, SummaryChip, Tone, TreeNode};

pub struct Events;

fn query_for(node_id: &str) -> Vec<&'static str> {
    match node_id {
        "eventvwr-app" => vec!["-b", "-p", "err..alert", "-n", "200"],
        "eventvwr-sys" => vec!["-b", "-n", "200"],
        "eventvwr-sec" => vec!["_TRANSPORT=audit", "-n", "200"],
        "eventvwr-setup" => vec!["-u", "dnf*", "-u", "packagekit*", "-n", "100"],
        "eventvwr-selinux" => vec!["_TRANSPORT=audit", "MESSAGE=AVC*", "-n", "100"],
        "eventvwr-admin" => vec!["-b", "-p", "warning..alert", "-n", "200"],
        _ => vec!["-b", "-n", "100"],
    }
}

fn fetch(node_id: &str) -> ResultsView {
    let mut args = vec![
        "--no-pager",
        "-o",
        "short-iso",
    ];
    let q = query_for(node_id);
    args.extend(q);
    // Prefer system journal; fall back to user.
    let out = Command::new("journalctl").args(&args).output();
    let text = match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            String::from_utf8_lossy(&o.stdout).into_owned()
        }
        Ok(o) if !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            let mut uargs = args.clone();
            uargs.insert(0, "--user");
            Command::new("journalctl")
                .args(&uargs)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default()
        }
    };
    let mut err = 0usize;
    let mut warn = 0usize;
    let rows: Vec<ResultRow> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            let (level, icon) = if line.contains(" error ")
                || line.contains("Failed")
                || line.contains(" err ")
                || line.contains("CRIT")
            {
                err += 1;
                ("Error", "dialog-error")
            } else if line.contains(" warning ")
                || line.contains("Warning")
                || line.contains(" warn ")
            {
                warn += 1;
                ("Warning", "dialog-warning")
            } else {
                ("Info", "dialog-information")
            };
            let (time, source, message) = split_journal_line(line);
            ResultRow {
                id: format!("{node_id}:{i}"),
                icon: icon.into(),
                cells: vec![level.into(), time, source, message],
            }
        })
        .collect();
    let n = rows.len();
    ResultsView {
        summaries: vec![
            SummaryChip::new("Events", n.to_string()),
            SummaryChip::new("Warnings", warn.to_string()).tone(if warn > 0 {
                Tone::Warn
            } else {
                Tone::Neutral
            }),
            SummaryChip::new("Errors", err.to_string()).tone(if err > 0 {
                Tone::Bad
            } else {
                Tone::Neutral
            }),
        ],
        columns: vec![
            "Level".into(),
            "Date and Time".into(),
            "Source".into(),
            "Message".into(),
        ],
        column_weights: vec![1, 2, 2, 5],
        rows,
        detail: String::new(),
        status: format!("{n} events"),
        ..Default::default()
    }
}

/// journalctl short-iso: `2024-01-15T10:30:45-0500 host unit[pid]: message`
fn split_journal_line(line: &str) -> (String, String, String) {
    let mut parts = line.splitn(3, char::is_whitespace);
    let time = parts.next().unwrap_or("").to_string();
    let _host = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    if let Some((src, msg)) = rest.split_once(": ") {
        (time, src.to_string(), msg.to_string())
    } else if rest.is_empty() {
        (time, String::new(), line.to_string())
    } else {
        (time, String::new(), rest.to_string())
    }
}

impl SnapIn for Events {
    fn key(&self) -> &'static str {
        "eventvwr"
    }
    fn name(&self) -> &'static str {
        "Event Viewer"
    }
    fn icon(&self) -> &'static str {
        "document-open-recent"
    }
    fn description(&self) -> &'static str {
        "View events that have happened on this computer"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("eventvwr", "Event Viewer (Local)", "document-open-recent", "eventvwr")
            .with_desc(self.description())
            .with_children(vec![
                TreeNode::new(
                    "eventvwr-custom",
                    "Custom Views",
                    "folder",
                    "eventvwr",
                )
                .with_children(vec![TreeNode::new(
                    "eventvwr-admin",
                    "Administrative Events",
                    "dialog-warning",
                    "eventvwr",
                )]),
                TreeNode::new("eventvwr-logs", "Windows Logs", "folder", "eventvwr").with_children(
                    vec![
                        TreeNode::new("eventvwr-app", "Application", "text-x-generic", "eventvwr"),
                        TreeNode::new("eventvwr-sec", "Security", "security-high", "eventvwr"),
                        TreeNode::new("eventvwr-setup", "Setup", "system-software-update", "eventvwr"),
                        TreeNode::new("eventvwr-sys", "System", "computer", "eventvwr"),
                    ],
                ),
                TreeNode::new(
                    "eventvwr-selinux",
                    "SELinux AVC",
                    "security-medium",
                    "eventvwr",
                ),
                TreeNode::new(
                    "eventvwr-remote",
                    "Connect to another computer…",
                    "network-server",
                    "eventvwr",
                ),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        if node_id == "eventvwr-remote" {
            return ResultsView {
                detail: "Remote journals use mde-mmc-agent (W476/W477).\n\
Run `mde-mmc-agent serve` on the target host, then Connect from Actions.\n\
Socket default: $XDG_RUNTIME_DIR/mde-mmc-agent.sock"
                    .into(),
                status: "Remote".into(),
                ..Default::default()
            };
        }
        if node_id == "eventvwr" || node_id == "eventvwr-logs" || node_id == "eventvwr-custom" {
            return ResultsView {
                detail: "Select a log under Event Viewer.".into(),
                status: "Event Viewer".into(),
                ..Default::default()
            };
        }
        fetch(node_id)
    }
    fn actions(&self, node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![
            ActionItem::new("refresh", "Refresh"),
            ActionItem::new("follow", "Follow (live)").disabled(),
        ];
        if matches!(
            node_id,
            "eventvwr-app" | "eventvwr-sys" | "eventvwr-admin"
        ) {
            a.push(ActionItem::new("clear", "Clear Log…").destructive());
        }
        a
    }
    fn run_action(
        &self,
        node_id: &str,
        action_id: &str,
        _selection: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed events".into()),
            "clear" => {
                if confirm_token != Some("CLEAR") {
                    return Err("Type CLEAR to confirm clearing this log".into());
                }
                let scope = if node_id.contains("app") {
                    "user"
                } else {
                    "system"
                };
                let uid = users::get_current_uid().to_string();
                let msg = if scope == "user" {
                    helper::run(&["journal-clear", "user", &uid])?
                } else {
                    helper::run(&["journal-clear", "system"])?
                };
                helper::audit(
                    &std::env::var("USER").unwrap_or_else(|_| "?".into()),
                    "journal-clear",
                    scope,
                    node_id,
                );
                Ok(msg)
            }
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}

mod users {
    pub fn get_current_uid() -> u32 {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(1000)
    }
}
