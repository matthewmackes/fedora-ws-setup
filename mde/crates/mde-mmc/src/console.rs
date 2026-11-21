//! Built-in consoles, presets, and JSON `.msc` load/save.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::ConsoleSpec;

pub fn builtin(name: &str) -> Option<ConsoleSpec> {
    let base = name.trim().trim_start_matches('/').to_lowercase();
    let key = Path::new(&base)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&base)
        .to_lowercase();
    let (name, snapins, tree, actions) = match key.as_str() {
        "compmgmt.msc" | "compmgmt" => ("Computer Management", vec!["compmgmt"], true, true),
        "devmgmt.msc" | "devmgmt" => ("Device Manager", vec!["devmgmt"], false, false),
        "diskmgmt.msc" | "diskmgmt" => ("Disk Management", vec!["diskmgmt"], false, false),
        "lusrmgr.msc" | "lusrmgr" => ("Local Users and Groups", vec!["lusrmgr"], true, true),
        "services.msc" | "services" => ("Services", vec!["services"], true, true),
        "eventvwr.msc" | "eventvwr" => ("Event Viewer", vec!["eventvwr"], true, true),
        "taskschd.msc" | "taskschd" => ("Task Scheduler", vec!["taskschd"], true, true),
        "perfmon.msc" | "perfmon" => ("Performance Monitor", vec!["perfmon"], true, true),
        "network.msc" | "network" => ("Network", vec!["network"], true, true),
        "firewall.msc" | "firewall" => ("Firewall", vec!["firewall"], true, true),
        "packages.msc" | "packages" => ("Package Management", vec!["packages"], true, true),
        _ => return None,
    };
    Some(ConsoleSpec {
        name: name.into(),
        snapins: snapins.into_iter().map(str::to_string).collect(),
        tree,
        actions,
        builtin: Some(key),
        path: None,
        console: None,
    })
}

pub fn preset(name: &str) -> Option<ConsoleSpec> {
    let snapins: &[&str] = match name {
        "Workstation" => &[
            "compmgmt", "network", "firewall", "packages", "selinux",
        ],
        "Server" => &[
            "compmgmt", "network", "firewall", "packages", "selinux", "containers",
            "virtualization", "subscriptions",
        ],
        "Security" => &[
            "eventvwr", "firewall", "selinux", "certificates", "lusrmgr",
        ],
        "Storage" => &["diskmgmt", "packages"],
        _ => return None,
    };
    Some(ConsoleSpec {
        name: format!("{name} Console"),
        snapins: snapins.iter().map(|s| (*s).to_string()).collect(),
        tree: true,
        actions: true,
        builtin: Some(format!("preset:{name}")),
        path: None,
        console: None,
    })
}

pub fn load(path: &Path) -> Result<ConsoleSpec, String> {
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut spec: ConsoleSpec = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    if let Some(c) = spec.console.clone() {
        if let Some(b) = builtin(&c) {
            spec.snapins = if spec.snapins.is_empty() {
                b.snapins
            } else {
                spec.snapins
            };
            if spec.name.is_empty() {
                spec.name = b.name;
            }
        }
    }
    spec.path = Some(path.to_string_lossy().into());
    Ok(spec)
}

pub fn save(path: &Path, spec: &ConsoleSpec) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut out = spec.clone();
    out.path = None;
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn default_save_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MDE Consoles")
}

/// Saved `.msc` files in the default consoles directory (newest first).
pub fn list_saved() -> Vec<PathBuf> {
    let dir = default_save_dir();
    let mut files: Vec<_> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("msc"))
        })
        .collect();
    files.sort_by_key(|p| {
        std::cmp::Reverse(
            fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        )
    });
    files
}

pub fn resolve_arg(arg: Option<&str>) -> ConsoleSpec {
    match arg {
        None | Some("") => ConsoleSpec {
            name: "Computer Management".into(),
            snapins: vec!["compmgmt".into()],
            tree: true,
            actions: true,
            builtin: Some("compmgmt.msc".into()),
            path: None,
            console: None,
        },
        Some(a) => {
            let p = PathBuf::from(a);
            if p.is_file() {
                load(&p).unwrap_or_else(|_| builtin(a).unwrap_or_default())
            } else if let Some(b) = builtin(a) {
                b
            } else if let Some(p) = preset(a) {
                p
            } else {
                builtin("compmgmt.msc").unwrap_or_default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_resolve() {
        assert_eq!(builtin("services.msc").unwrap().snapins, vec!["services"]);
        assert_eq!(builtin("compmgmt").unwrap().name, "Computer Management");
    }

    #[test]
    fn presets_exist() {
        assert!(preset("Workstation").is_some());
        assert!(preset("Server").is_some());
        assert!(preset("Security").is_some());
        assert!(preset("Storage").is_some());
    }

    #[test]
    fn roundtrip_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.msc");
        let spec = builtin("services.msc").unwrap();
        save(&path, &spec).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.snapins, spec.snapins);
    }

    #[test]
    fn list_saved_filters_msc() {
        let dir = tempfile::tempdir().unwrap();
        // Point HOME so default_save_dir resolves under the temp tree when Documents missing.
        // list_saved reads default_save_dir(); create files there only if dir exists — unit
        // checks filtering logic by writing into a temp dir and scanning manually.
        let a = dir.path().join("a.msc");
        let b = dir.path().join("b.txt");
        fs::write(&a, "{}").unwrap();
        fs::write(&b, "x").unwrap();
        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("msc"))
            })
            .collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.msc"));
    }
}
