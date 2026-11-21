//! Launch `.desktop` pins / Start entries.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn launch_desktop(id: &str) -> Result<(), String> {
    let path = resolve_desktop(id).ok_or_else(|| format!("desktop entry not found: {id}"))?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let exec = parse_exec(&text).ok_or_else(|| format!("no Exec= in {}", path.display()))?;
    let argv = expand_exec(&exec);
    if argv.is_empty() {
        return Err("empty Exec".into());
    }
    Command::new(&argv[0])
        .args(&argv[1..])
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", argv[0]))?;
    Ok(())
}

pub fn resolve_desktop(id: &str) -> Option<PathBuf> {
    // Absolute / existing path (Desktop shortcuts, Open location).
    let as_path = PathBuf::from(id);
    if as_path.is_file() {
        return Some(as_path);
    }
    let name = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
        dirs.push(
            PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"),
        );
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for part in xdg.split(':').filter(|s| !s.is_empty()) {
            dirs.push(PathBuf::from(part).join("applications"));
        }
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from(
        "/var/lib/flatpak/exports/share/applications",
    ));
    for dir in dirs {
        let p = dir.join(&name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Package backend for Uninstall (W421): flatpak vs rpm vs unsupported.
#[derive(Debug, Clone)]
pub enum UninstallKind {
    Flatpak(String),
    Rpm(String),
    Unsupported(String),
}

/// Detect uninstall target from a desktop id / path (W421).
pub fn detect_uninstall(id: &str) -> (UninstallKind, String) {
    let Some(path) = resolve_desktop(id) else {
        return (
            UninstallKind::Unsupported("desktop entry not found".into()),
            id.to_string(),
        );
    };
    let display = desktop_name(&path);
    let path_s = path.display().to_string();
    let text = fs::read_to_string(&path).unwrap_or_default();
    if let Some(fp) = key(&text, "X-Flatpak") {
        if !fp.is_empty() {
            return (UninstallKind::Flatpak(fp), display);
        }
    }
    if path_s.contains("/flatpak/exports/") || path_s.contains("/flatpak/app/") {
        let app_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| display.clone());
        return (UninstallKind::Flatpak(app_id), display);
    }
    // rpm -qf the desktop file (system packages under /usr).
    if path_s.starts_with("/usr/") {
        if let Ok(out) = Command::new("rpm")
            .args(["-qf", "--qf", "%{NAME}", &path_s])
            .output()
        {
            if out.status.success() {
                let pkg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !pkg.is_empty() && !pkg.contains("not owned") {
                    return (UninstallKind::Rpm(pkg), display);
                }
            }
        }
    }
    (
        UninstallKind::Unsupported(
            "not a Flatpak or RPM package (local / user desktop entry)".into(),
        ),
        display,
    )
}

/// Run Flatpak / pkcon / dnf uninstall; returns status text (W421).
pub fn run_uninstall(kind: &UninstallKind) -> String {
    match kind {
        UninstallKind::Unsupported(reason) => format!("Uninstall · {reason}"),
        UninstallKind::Flatpak(pkg) => {
            let out = Command::new("flatpak")
                .args(["uninstall", "-y", "--noninteractive", pkg])
                .output();
            match out {
                Ok(o) if o.status.success() => format!("Uninstalled Flatpak · {pkg}"),
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    let err = err.trim();
                    if looks_like_polkit(&err) || looks_like_polkit(&String::from_utf8_lossy(&o.stdout))
                    {
                        format!("Uninstall needs authentication (polkit) · flatpak {pkg}")
                    } else if err.is_empty() {
                        format!("Uninstall failed · flatpak {pkg}")
                    } else {
                        format!("Uninstall failed · {}", truncate_err(err))
                    }
                }
                Err(e) => format!("Uninstall · flatpak unavailable ({e})"),
            }
        }
        UninstallKind::Rpm(pkg) => {
            // Prefer pkcon (PackageKit / polkit), then dnf.
            if which("pkcon") {
                let out = Command::new("pkcon")
                    .args(["remove", "-y", "--plain", pkg])
                    .output();
                return match out {
                    Ok(o) if o.status.success() => format!("Uninstalled · {pkg}"),
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr);
                        let out_s = String::from_utf8_lossy(&o.stdout);
                        if looks_like_polkit(&err) || looks_like_polkit(&out_s) {
                            format!("Uninstall needs authentication (polkit) · {pkg}")
                        } else {
                            let msg = err.trim();
                            if msg.is_empty() {
                                format!("Uninstall failed · pkcon {pkg}")
                            } else {
                                format!("Uninstall failed · {}", truncate_err(msg))
                            }
                        }
                    }
                    Err(e) => format!("Uninstall · pkcon error ({e})"),
                };
            }
            if which("dnf") {
                let out = Command::new("dnf")
                    .args(["remove", "-y", pkg])
                    .output();
                return match out {
                    Ok(o) if o.status.success() => format!("Uninstalled · {pkg}"),
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr);
                        if looks_like_polkit(&err) || err.to_lowercase().contains("root") {
                            format!("Uninstall needs authentication (polkit/root) · {pkg}")
                        } else {
                            format!("Uninstall failed · {}", truncate_err(err.trim()))
                        }
                    }
                    Err(e) => format!("Uninstall · dnf error ({e})"),
                };
            }
            format!("Uninstall · neither pkcon nor dnf found for {pkg}")
        }
    }
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn looks_like_polkit(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("polkit")
        || l.contains("not authorized")
        || l.contains("authentication")
        || l.contains("auth_error")
        || l.contains("permission denied")
        || l.contains("interactive authentication")
}

fn truncate_err(s: &str) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    if line.chars().count() > 120 {
        format!("{}…", line.chars().take(117).collect::<String>())
    } else {
        line.to_string()
    }
}

fn parse_exec(desktop: &str) -> Option<String> {
    for line in desktop.lines() {
        if let Some(rest) = line.strip_prefix("Exec=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Expand Exec= field: drop field codes (%u etc.) and split on spaces (simple).
fn expand_exec(exec: &str) -> Vec<String> {
    let cleaned: String = exec
        .split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ");
    shell_words(&cleaned)
}

fn shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        match (in_quote, c) {
            (Some(q), c) if c == q => in_quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => in_quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub icon: String,
}

pub fn list_apps(limit: usize) -> Vec<AppEntry> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if key(&text, "NoDisplay").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                continue;
            }
            if key(&text, "Type").is_some_and(|v| !v.eq_ignore_ascii_case("Application")) {
                continue;
            }
            let Some(name) = key(&text, "Name") else {
                continue;
            };
            if key(&text, "Exec").is_none() {
                continue;
            }
            let Some(id) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            out.push(AppEntry {
                id: id.to_string(),
                name,
                icon: key(&text, "Icon").unwrap_or_default(),
            });
            if out.len() >= limit {
                break;
            }
        }
        if out.len() >= limit {
            break;
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn key(desktop: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    for line in desktop.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

pub fn desktop_icon(id: &str) -> Option<String> {
    let path = resolve_desktop(id)?;
    let text = fs::read_to_string(path).ok()?;
    let icon = key(&text, "Icon")?;
    if icon.is_empty() {
        None
    } else {
        Some(icon)
    }
}

pub fn desktop_name(path: &Path) -> String {
    if let Ok(text) = fs::read_to_string(path) {
        for line in text.lines() {
            if let Some(n) = line.strip_prefix("Name=") {
                return n.trim().into();
            }
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_strips_codes() {
        let v = expand_exec("firefox %u");
        assert_eq!(v, vec!["firefox".to_string()]);
    }
}
