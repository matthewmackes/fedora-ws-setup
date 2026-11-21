//! Services snap-in — systemd units (writable via helper).

use std::process::Command;

use serde_json::Value;

use super::SnapIn;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, SummaryChip, Tone, TreeNode};

pub struct Services;

#[derive(Clone)]
struct Svc {
    unit: String,
    description: String,
    active: String,
    sub: String,
    startup: String,
}

fn startup_label(state: &str) -> &'static str {
    // Correct Windows mapping (fixes Python bug: disabled→Manual).
    match state {
        "enabled" | "enabled-runtime" => "Automatic",
        "disabled" => "Disabled",
        "masked" | "masked-runtime" => "Disabled (Masked)",
        "static" | "indirect" | "generated" | "transient" | "alias" => "Manual",
        _ => "Manual",
    }
}

fn status_label(active: &str, sub: &str) -> String {
    match active {
        "active" => "Running".into(),
        "activating" => "Start Pending".into(),
        "deactivating" => "Stop Pending".into(),
        "failed" => "Failed".into(),
        _ if !sub.is_empty() => sub.to_string(),
        _ => String::new(),
    }
}

fn list_services() -> Result<Vec<Svc>, String> {
    let files = Command::new("systemctl")
        .args([
            "list-unit-files",
            "--type=service",
            "--no-pager",
            "-o",
            "json",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    let units = Command::new("systemctl")
        .args(["list-units", "--type=service", "--all", "--no-pager", "-o", "json"])
        .output()
        .map_err(|e| e.to_string())?;

    let mut startup = std::collections::HashMap::new();
    if files.status.success() {
        if let Ok(Value::Array(arr)) = serde_json::from_slice(&files.stdout) {
            for u in arr {
                let name = u.get("unit_file").and_then(|v| v.as_str()).unwrap_or("");
                let st = u.get("state").and_then(|v| v.as_str()).unwrap_or("");
                if !name.is_empty() {
                    startup.insert(name.to_string(), startup_label(st).to_string());
                }
            }
        }
    }

    let mut out = Vec::new();
    if units.status.success() {
        if let Ok(Value::Array(arr)) = serde_json::from_slice(&units.stdout) {
            for u in arr {
                let unit = u
                    .get("unit")
                    .or_else(|| u.get("unit_file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if unit.is_empty() || !unit.ends_with(".service") {
                    continue;
                }
                let description = u
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let active = u
                    .get("active")
                    .or_else(|| u.get("active_state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let sub = u
                    .get("sub")
                    .or_else(|| u.get("sub_state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let st = startup
                    .get(&unit)
                    .cloned()
                    .unwrap_or_else(|| "Manual".into());
                out.push(Svc {
                    unit,
                    description,
                    active,
                    sub,
                    startup: st,
                });
            }
        }
    }

    // Include unit-files that are not in list-units.
    for (unit, st) in &startup {
        if !out.iter().any(|s| &s.unit == unit) {
            out.push(Svc {
                unit: unit.clone(),
                description: String::new(),
                active: "inactive".into(),
                sub: "dead".into(),
                startup: st.clone(),
            });
        }
    }

    out.sort_by(|a, b| a.unit.cmp(&b.unit));
    Ok(out)
}

impl SnapIn for Services {
    fn key(&self) -> &'static str {
        "services"
    }
    fn name(&self) -> &'static str {
        "Services"
    }
    fn icon(&self) -> &'static str {
        "system-run"
    }
    fn description(&self) -> &'static str {
        "Start, stop and configure the services on this computer"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("services", "Services (Local)", "system-run", "services")
            .with_desc(self.description())
            .with_children(vec![TreeNode::new(
                "services-deps",
                "Dependencies (selected unit)",
                "view-list",
                "services",
            )])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        if node_id == "services-deps" {
            return ResultsView {
                detail: "Select a service in Services (Local), then open this node after choosing Dependencies from Actions — or run `systemctl list-dependencies UNIT`.\n\nFull graph editor is W467.".into(),
                status: "Dependencies".into(),
                ..Default::default()
            };
        }
        match list_services() {
            Ok(list) => {
                let total = list.len();
                let running = list.iter().filter(|s| s.active == "active").count();
                let failed = list.iter().filter(|s| s.active == "failed").count();
                let stopped = total.saturating_sub(running + failed);
                ResultsView {
                    summaries: vec![
                        SummaryChip::new("Total", total.to_string()),
                        SummaryChip::new("Running", running.to_string()).tone(Tone::Good),
                        SummaryChip::new("Stopped", stopped.to_string()),
                        SummaryChip::new("Failed", failed.to_string()).tone(if failed > 0 {
                            Tone::Bad
                        } else {
                            Tone::Neutral
                        }),
                    ],
                    columns: vec![
                        "Name".into(),
                        "Description".into(),
                        "Status".into(),
                        "Startup Type".into(),
                    ],
                    column_weights: vec![3, 4, 2, 2],
                    rows: list
                        .into_iter()
                        .map(|s| ResultRow {
                            id: s.unit.clone(),
                            icon: if s.active == "active" {
                                "media-playback-start".into()
                            } else if s.active == "failed" {
                                "dialog-error".into()
                            } else {
                                "media-playback-stop".into()
                            },
                            cells: vec![
                                s.unit.trim_end_matches(".service").to_string(),
                                s.description,
                                status_label(&s.active, &s.sub),
                                s.startup,
                            ],
                        })
                        .collect(),
                    detail: String::new(),
                    status: format!("{running} running · {failed} failed"),
                    ..Default::default()
                }
            }
            Err(e) => ResultsView {
                detail: e.clone(),
                status: e,
                ..Default::default()
            },
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![ActionItem::new("refresh", "Refresh")];
        if selection.is_some() {
            a.extend([
                ActionItem::new("start", "Start"),
                ActionItem::new("stop", "Stop"),
                ActionItem::new("restart", "Restart"),
                ActionItem::new("enable", "Set Startup: Automatic"),
                ActionItem::new("disable", "Set Startup: Disabled"),
                ActionItem::new("mask", "Mask").destructive(),
                ActionItem::new("unmask", "Unmask"),
                ActionItem::new("deps", "Show dependencies"),
                ActionItem::new("dropin", "Show drop-in / unit fragment"),
                ActionItem::new("recovery", "Show recovery (Restart=)"),
            ]);
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
            return Ok("Refreshed services".into());
        }
        let unit = selection.ok_or_else(|| "Select a service".to_string())?;
        let unit = if unit.ends_with(".service") {
            unit.to_string()
        } else {
            format!("{unit}.service")
        };
        let who = whoami();
        match action_id {
            "start" | "stop" | "restart" | "enable" | "disable" | "mask" | "unmask" => {
                let msg = helper::systemctl(&[action_id, &unit])?;
                helper::audit(&who, action_id, &unit, "services");
                Ok(msg)
            }
            "deps" => {
                let out = Command::new("systemctl")
                    .args(["list-dependencies", &unit, "--no-pager"])
                    .output()
                    .map_err(|e| e.to_string())?;
                Ok(String::from_utf8_lossy(&out.stdout).trim().into())
            }
            "dropin" => {
                let out = Command::new("systemctl")
                    .args(["cat", &unit, "--no-pager"])
                    .output()
                    .map_err(|e| e.to_string())?;
                Ok(String::from_utf8_lossy(&out.stdout).trim().into())
            }
            "recovery" => {
                let out = Command::new("systemctl")
                    .args(["show", &unit, "-p", "Restart", "-p", "RestartSec", "-p", "StartLimitBurst"])
                    .output()
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "{}\n\nEdit drop-ins under /etc/systemd/system/{}.d/ (W467 editor UI follows).",
                    String::from_utf8_lossy(&out.stdout).trim(),
                    unit
                ))
            }
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}

fn whoami() -> String {
    std::env::var("USER").unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_map() {
        assert_eq!(startup_label("disabled"), "Disabled");
        assert_eq!(startup_label("enabled"), "Automatic");
        assert_eq!(startup_label("masked"), "Disabled (Masked)");
        assert_eq!(startup_label("static"), "Manual");
    }
}
