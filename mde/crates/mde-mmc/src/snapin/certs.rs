//! Certificates snap-in (W487).

use std::process::Command;

use super::SnapIn;
use crate::model::{ActionItem, ResultsView, TreeNode};

pub struct Certs;

impl SnapIn for Certs {
    fn key(&self) -> &'static str {
        "certificates"
    }
    fn name(&self) -> &'static str {
        "Certificates"
    }
    fn icon(&self) -> &'static str {
        "application-certificate"
    }
    fn description(&self) -> &'static str {
        "System and user trust stores"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("certificates", "Certificates", "application-certificate", "certificates")
            .with_children(vec![
                TreeNode::new("certs-system", "System trust", "application-certificate", "certificates"),
                TreeNode::new("certs-user", "User NSS", "system-users", "certificates"),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        let detail = if node_id == "certs-user" {
            cmd("ls", &["-la", &format!("{}/.pki/nssdb", dirs::home_dir().unwrap_or_default().display())])
        } else {
            format!(
                "ca-trust extract anchors (names):\n{}\n\n/etc/pki/ca-trust/source/anchors:\n{}",
                cmd("trust", &["list"]),
                cmd("ls", &["-la", "/etc/pki/ca-trust/source/anchors"])
            )
        };
        ResultsView {
            detail,
            status: "Certificates".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![ActionItem::new("refresh", "Refresh")]
    }
    fn run_action(
        &self,
        _: &str,
        action_id: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
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
