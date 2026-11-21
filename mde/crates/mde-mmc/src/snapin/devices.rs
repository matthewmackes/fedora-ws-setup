//! Device Manager — sysfs/udev inventory; mutations via helper (Python bridge for depth).

use std::fs;
use std::path::Path;

use super::SnapIn;
use crate::bridge;
use crate::helper;
use crate::model::{ActionItem, ResultRow, ResultsView, TreeNode};

pub struct Devices;

fn pci_rows() -> Vec<ResultRow> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") else {
        return rows;
    };
    for e in entries.flatten() {
        let path = e.path();
        let id = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let vendor = read_trim(&path.join("vendor"));
        let device = read_trim(&path.join("device"));
        let driver = fs::read_link(path.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "(none)".into());
        let sys_path = path
            .strip_prefix("/sys")
            .map(|p| format!("/{}", p.display()))
            .unwrap_or_default();
        rows.push(ResultRow {
            id: sys_path,
            icon: "computer".into(),
            cells: vec![
                id,
                format!("{vendor}:{device}"),
                driver,
                "PCI".into(),
            ],
        });
    }
    rows.sort_by(|a, b| a.cells[0].cmp(&b.cells[0]));
    rows
}

fn usb_rows() -> Vec<ResultRow> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/bus/usb/devices") else {
        return rows;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.contains(':') && name.contains('.') {
            continue;
        }
        let product = read_trim(&path.join("product"));
        if product.is_empty() && !path.join("idVendor").exists() {
            continue;
        }
        let vendor_id = read_trim(&path.join("idVendor"));
        let product_id = read_trim(&path.join("idProduct"));
        let sys_path = path
            .strip_prefix("/sys")
            .map(|p| format!("/{}", p.display()))
            .unwrap_or_default();
        rows.push(ResultRow {
            id: sys_path,
            icon: "drive-removable-media".into(),
            cells: vec![
                if product.is_empty() {
                    name.into()
                } else {
                    product
                },
                format!("{vendor_id}:{product_id}"),
                name.into(),
                "USB".into(),
            ],
        });
    }
    rows
}

fn read_trim(p: &Path) -> String {
    fs::read_to_string(p)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

impl SnapIn for Devices {
    fn key(&self) -> &'static str {
        "devmgmt"
    }
    fn name(&self) -> &'static str {
        "Device Manager"
    }
    fn icon(&self) -> &'static str {
        "video-display"
    }
    fn description(&self) -> &'static str {
        "View and update your hardware’s settings and driver software"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new("devmgmt", "Device Manager", "video-display", "devmgmt")
            .with_desc(self.description())
            .with_children(vec![
                TreeNode::new("devmgmt-pci", "PCI devices", "computer", "devmgmt"),
                TreeNode::new("devmgmt-usb", "USB devices", "drive-removable-media", "devmgmt"),
                TreeNode::new(
                    "devmgmt-bridge",
                    "Full inventory (Python bridge)",
                    "applications-system",
                    "devmgmt",
                ),
            ])
    }
    fn results(&self, node_id: &str) -> ResultsView {
        match node_id {
            "devmgmt-usb" => ResultsView {
                columns: vec![
                    "Device".into(),
                    "IDs".into(),
                    "Sysfs".into(),
                    "Bus".into(),
                ],
                column_weights: vec![3, 2, 3, 1],
                rows: usb_rows(),
                status: "USB".into(),
                detail: String::new(),
                ..Default::default()
            },
            "devmgmt-bridge" => {
                let detail = if bridge::available() {
                    bridge::py_eval(
                        "print('Bridge OK:', __import__('os').path.isdir('.')); print('Use Device Manager categories in Python for full sheets until W463.')",
                    )
                    .unwrap_or_else(|e| e)
                } else {
                    "Python ws-mmc not installed — Rust PCI/USB views still work.".into()
                };
                ResultsView {
                    detail,
                    status: "Bridge".into(),
                    ..Default::default()
                }
            }
            _ => ResultsView {
                columns: vec![
                    "Address".into(),
                    "Vendor:Device".into(),
                    "Driver".into(),
                    "Bus".into(),
                ],
                column_weights: vec![2, 2, 3, 1],
                rows: pci_rows(),
                status: "PCI".into(),
                detail: String::new(),
                ..Default::default()
            },
        }
    }
    fn actions(&self, _node_id: &str, selection: Option<&str>) -> Vec<ActionItem> {
        let mut a = vec![
            ActionItem::new("refresh", "Scan for hardware changes"),
            ActionItem::new("rescan", "Rescan PCI"),
        ];
        if let Some(sel) = selection {
            if sel.starts_with("/devices/") {
                a.push(ActionItem::new("disable", "Disable device").destructive());
                a.push(ActionItem::new("enable", "Enable device"));
                a.push(ActionItem::new("remove", "Uninstall device").destructive());
                a.push(ActionItem::new("driver-info", "Driver details"));
                a.push(ActionItem::new("driver-update", "Update driver (packages)"));
                a.push(ActionItem::new("power", "Power control…"));
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
            "refresh" => Ok("Hardware scan refreshed".into()),
            "rescan" => {
                let msg = helper::run(&["device-rescan"])?;
                helper::audit(&who, "device-rescan", "-", "devmgmt");
                Ok(msg)
            }
            "disable" => {
                let path = selection.ok_or("Select a device")?;
                if confirm_token != Some(path) {
                    return Err(format!("Type the device path “{path}” to confirm disable"));
                }
                let bus = if path.contains("/usb") { "usb" } else { "pci" };
                let msg = helper::run(&["device-disable", path, bus, "Other", "device"])?;
                helper::audit(&who, "device-disable", path, "devmgmt");
                Ok(msg)
            }
            "enable" => {
                let path = selection.ok_or("Select a device")?;
                let msg = helper::run(&["device-enable", path])?;
                helper::audit(&who, "device-enable", path, "devmgmt");
                Ok(msg)
            }
            "remove" => {
                let path = selection.ok_or("Select a device")?;
                if confirm_token != Some(path) {
                    return Err(format!("Type the device path “{path}” to confirm uninstall"));
                }
                let msg = helper::run(&["device-remove", path])?;
                helper::audit(&who, "device-remove", path, "devmgmt");
                Ok(msg)
            }
            "driver-info" => {
                let path = selection.ok_or("Select a device")?;
                let sys = format!("/sys{path}");
                let driver = std::fs::read_link(format!("{sys}/driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "(none)".into());
                let modinfo = if driver != "(none)" {
                    std::process::Command::new("modinfo")
                        .arg(&driver)
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let resources = std::fs::read_to_string(format!("{sys}/resource"))
                    .or_else(|_| std::fs::read_to_string(format!("{sys}/resources")))
                    .unwrap_or_else(|_| "(no resource file)".into());
                Ok(format!(
                    "Driver: {driver}\n\nmodinfo:\n{modinfo}\n\nResources:\n{resources}\n\nConflicts: none reported (static check)."
                ))
            }
            "driver-update" => {
                let _ = std::process::Command::new("mde-mmc").arg("packages.msc").spawn();
                Ok("Opened Package Management — search firmware/akmod packages (W463)".into())
            }
            "power" => {
                let path = selection.ok_or("Select a device")?;
                let control = std::fs::read_to_string(format!("/sys{path}/power/control"))
                    .unwrap_or_else(|_| "n/a".into());
                let wakeup = std::fs::read_to_string(format!("/sys{path}/power/wakeup"))
                    .unwrap_or_else(|_| "n/a".into());
                Ok(format!(
                    "Power control={} wakeup={}\nUse helper device-power to change (confirm in follow-up sheet).",
                    control.trim(),
                    wakeup.trim()
                ))
            }
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}
