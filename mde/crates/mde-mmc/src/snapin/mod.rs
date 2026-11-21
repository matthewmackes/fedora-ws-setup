//! Snap-in trait and registry.

pub mod devices;
pub mod disks;
pub mod events;
pub mod firewall;
pub mod network;
pub mod packages;
pub mod perfmon;
pub mod certs;
pub mod containers;
pub mod kernel;
pub mod logging;
pub mod selinux;
pub mod services;
pub mod tasks;
pub mod time_sync;
pub mod tuned;
pub mod updates;
pub mod users;
pub mod virt;

use crate::model::{ActionItem, ResultsView, TreeNode};

pub trait SnapIn: Send + Sync {
    fn key(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn icon(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn root(&self) -> TreeNode;
    fn results(&self, node_id: &str) -> ResultsView;
    fn actions(&self, node_id: &str, selection: Option<&str>) -> Vec<ActionItem>;
    fn run_action(
        &self,
        node_id: &str,
        action_id: &str,
        selection: Option<&str>,
        confirm_token: Option<&str>,
    ) -> Result<String, String>;
}

pub struct SnapMeta {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
}

pub fn all_meta() -> Vec<SnapMeta> {
    vec![
        SnapMeta {
            key: "compmgmt",
            name: "Computer Management",
            icon: "computer",
            description: "System tools, storage, services",
        },
        SnapMeta {
            key: "devmgmt",
            name: "Device Manager",
            icon: "video-display",
            description: "Hardware and drivers",
        },
        SnapMeta {
            key: "diskmgmt",
            name: "Disk Management",
            icon: "drive-harddisk",
            description: "Disks, volumes, encryption, remote storage",
        },
        SnapMeta {
            key: "lusrmgr",
            name: "Local Users and Groups",
            icon: "system-users",
            description: "Users, groups, sudoers, identity",
        },
        SnapMeta {
            key: "services",
            name: "Services",
            icon: "system-run",
            description: "Start, stop and configure services",
        },
        SnapMeta {
            key: "eventvwr",
            name: "Event Viewer",
            icon: "document-open-recent",
            description: "Journal, audit, SELinux events",
        },
        SnapMeta {
            key: "taskschd",
            name: "Task Scheduler",
            icon: "x-office-calendar",
            description: "Timers, cron, remote tasks",
        },
        SnapMeta {
            key: "perfmon",
            name: "Performance Monitor",
            icon: "utilities-system-monitor",
            description: "Counters, collectors, alerts",
        },
        SnapMeta {
            key: "network",
            name: "Network",
            icon: "network-workgroup",
            description: "NetworkManager connections and devices",
        },
        SnapMeta {
            key: "firewall",
            name: "Firewall",
            icon: "preferences-system-network",
            description: "firewalld zones and rules",
        },
        SnapMeta {
            key: "packages",
            name: "Package Management",
            icon: "system-software-install",
            description: "Search, install, remove, repos, history",
        },
        SnapMeta {
            key: "selinux",
            name: "SELinux",
            icon: "security-medium",
            description: "Mode, booleans, AVC triage",
        },
        SnapMeta {
            key: "containers",
            name: "Containers",
            icon: "package-x-generic",
            description: "Podman pods, images, volumes",
        },
        SnapMeta {
            key: "timesync",
            name: "Time & Sync",
            icon: "preferences-date-and-time",
            description: "chrony / NTP and timezone",
        },
        SnapMeta {
            key: "updates",
            name: "Software Updates",
            icon: "system-software-update",
            description: "Admin update path (with Settings Update)",
        },
        SnapMeta {
            key: "virtualization",
            name: "Virtualization",
            icon: "virtual-machine",
            description: "libvirt VMs, networks, storage",
        },
        SnapMeta {
            key: "certificates",
            name: "Certificates",
            icon: "application-certificate",
            description: "System and user trust stores",
        },
        SnapMeta {
            key: "logging",
            name: "Logging Config",
            icon: "accessories-text-editor",
            description: "journald persistence and vacuum",
        },
        SnapMeta {
            key: "kernel",
            name: "Kernel & Modules",
            icon: "applications-system",
            description: "Modules, blacklist, cmdline",
        },
        SnapMeta {
            key: "tuned",
            name: "Tuned / Power",
            icon: "battery",
            description: "tuned profiles",
        },
    ]
}

pub fn get(key: &str) -> Option<Box<dyn SnapIn>> {
    Some(match key {
        "compmgmt" => Box::new(CompMgmt),
        "devmgmt" => Box::new(devices::Devices),
        "diskmgmt" => Box::new(disks::Disks),
        "lusrmgr" => Box::new(users::Users),
        "services" => Box::new(services::Services),
        "eventvwr" => Box::new(events::Events),
        "taskschd" => Box::new(tasks::Tasks),
        "perfmon" => Box::new(perfmon::Perfmon),
        "network" => Box::new(network::Network),
        "firewall" => Box::new(firewall::Firewall),
        "packages" => Box::new(packages::Packages),
        "selinux" => Box::new(selinux::Selinux),
        "containers" => Box::new(containers::Containers),
        "timesync" => Box::new(time_sync::TimeSync),
        "updates" => Box::new(updates::Updates),
        "virtualization" => Box::new(virt::Virt),
        "certificates" => Box::new(certs::Certs),
        "logging" => Box::new(logging::Logging),
        "kernel" => Box::new(kernel::Kernel),
        "tuned" => Box::new(tuned::Tuned),
        "subscriptions" => Box::new(Placeholder {
            key: "subscriptions",
        }),
        _ => return None,
    })
}

pub struct CompMgmt;

impl SnapIn for CompMgmt {
    fn key(&self) -> &'static str {
        "compmgmt"
    }
    fn name(&self) -> &'static str {
        "Computer Management"
    }
    fn icon(&self) -> &'static str {
        "computer"
    }
    fn description(&self) -> &'static str {
        "Manage this computer"
    }
    fn root(&self) -> TreeNode {
        let kids = |keys: &[&str]| {
            keys.iter()
                .filter_map(|k| get(k).map(|s| s.root()))
                .collect::<Vec<_>>()
        };
        TreeNode::new("compmgmt", "Computer Management (Local)", "computer", "compmgmt")
            .with_desc("Manage this computer: system tools, storage, services")
            .with_children(vec![
                TreeNode::new("systemtools", "System Tools", "applications-system", "compmgmt")
                    .with_desc("Tools for managing this computer")
                    .with_children(kids(&[
                        "taskschd", "eventvwr", "lusrmgr", "perfmon", "devmgmt",
                    ])),
                TreeNode::new("storage", "Storage", "drive-harddisk", "compmgmt")
                    .with_desc("Disk drives and volumes")
                    .with_children(kids(&["diskmgmt"])),
                TreeNode::new("svcapps", "Services and Applications", "system-run", "compmgmt")
                    .with_desc("Services that run on this computer")
                    .with_children(kids(&["services"])),
            ])
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        ResultsView {
            detail: "Select a snap-in under Computer Management.".into(),
            status: "Computer Management".into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![ActionItem::new("refresh", "Refresh")]
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
            _ => Err(format!("Unknown action {action_id}")),
        }
    }
}

struct Placeholder {
    key: &'static str,
}

impl SnapIn for Placeholder {
    fn key(&self) -> &'static str {
        self.key
    }
    fn name(&self) -> &'static str {
        self.key
    }
    fn icon(&self) -> &'static str {
        "applications-system"
    }
    fn description(&self) -> &'static str {
        "Linux admin snap-in (Phase 3)"
    }
    fn root(&self) -> TreeNode {
        TreeNode::new(self.key, self.key, "applications-system", self.key)
            .with_desc("Scaffold — writable implementation in Phase 3 backlog")
    }
    fn results(&self, _node_id: &str) -> ResultsView {
        ResultsView {
            detail: format!(
                "{} snap-in is registered. Full writable UI lands in W480–W493.",
                self.key
            ),
            status: self.key.into(),
            ..Default::default()
        }
    }
    fn actions(&self, _node_id: &str, _selection: Option<&str>) -> Vec<ActionItem> {
        vec![]
    }
    fn run_action(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<String, String> {
        Err("Not implemented yet".into())
    }
}
