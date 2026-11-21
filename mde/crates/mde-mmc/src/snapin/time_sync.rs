//! Time & Sync snap-in (W488).

use std::process::Command;

use super::SnapIn;
use crate::helper;
use crate::model::{
    ActionItem, Property, ResultsView, SummaryChip, Tone, TreeNode,
};

pub struct TimeSync;

impl SnapIn for TimeSync {
    fn key(&self) -> &'static str {
        "timesync"
    }
    fn name(&self) -> &'static str {
        "Time & Sync"
    }
    fn icon(&self) -> &'static str {
        "preferences-date-and-time"
    }
    fn description(&self) -> &'static str {
        "chrony / NTP and timezone"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("timesync", "Time & Sync", "preferences-date-and-time", "timesync")
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        let td = cmd("timedatectl", &["show", "--no-pager"]);
        let mut props = Vec::new();
        let mut timezone = String::new();
        let mut ntp = String::new();
        let mut synchronized = String::new();
        let mut local_rtc = String::new();
        for line in td.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k {
                    "Timezone" => timezone = v.to_string(),
                    "NTP" => ntp = v.to_string(),
                    "NTPSynchronized" => synchronized = v.to_string(),
                    "LocalRTC" => local_rtc = v.to_string(),
                    "TimeUSec" | "RTCTimeUSec" => {}
                    _ => {}
                }
                if matches!(
                    k,
                    "Timezone" | "NTP" | "NTPSynchronized" | "LocalRTC" | "CanNTP"
                ) {
                    props.push(Property::new(k, v));
                }
            }
        }
        if props.is_empty() {
            // Fallback: plain timedatectl dump as properties-ish detail
            let plain = cmd("timedatectl", &[]);
            for line in plain.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    props.push(Property::new(k.trim(), v.trim()));
                }
            }
        }

        let chrony = cmd("chronyc", &["tracking"]);
        let mut chrony_props = Vec::new();
        for line in chrony.lines() {
            if let Some((k, v)) = line.split_once(':') {
                chrony_props.push(Property::new(k.trim(), v.trim()));
            }
        }
        props.extend(chrony_props);

        let ntp_on = ntp.eq_ignore_ascii_case("yes") || ntp.eq_ignore_ascii_case("true");
        let sync_on = synchronized.eq_ignore_ascii_case("yes")
            || synchronized.eq_ignore_ascii_case("true");

        ResultsView {
            summaries: vec![
                SummaryChip::new(
                    "Timezone",
                    if timezone.is_empty() {
                        "—".into()
                    } else {
                        timezone
                    },
                ),
                SummaryChip::new("NTP", if ntp_on { "On" } else { "Off" }).tone(if ntp_on {
                    Tone::Good
                } else {
                    Tone::Warn
                }),
                SummaryChip::new("Synchronized", if sync_on { "Yes" } else { "No" }).tone(
                    if sync_on {
                        Tone::Good
                    } else {
                        Tone::Warn
                    },
                ),
            ],
            properties: props,
            detail: if local_rtc.is_empty() {
                String::new()
            } else {
                format!("RTC in local TZ: {local_rtc}")
            },
            status: "Time".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![
            ActionItem::new("refresh", "Refresh"),
            ActionItem::new("ntp-on", "Enable NTP"),
            ActionItem::new("ntp-off", "Disable NTP"),
        ]
    }
    fn run_action(
        &self,
        _node_id: &str,
        action_id: &str,
        _selection: Option<&str>,
        _confirm_token: Option<&str>,
    ) -> Result<String, String> {
        match action_id {
            "refresh" => Ok("Refreshed".into()),
            "ntp-on" => helper::run(&["timedatectl", "set-ntp", "true"]),
            "ntp-off" => helper::run(&["timedatectl", "set-ntp", "false"]),
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
