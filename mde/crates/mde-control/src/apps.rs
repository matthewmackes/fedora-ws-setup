//! Installed `.desktop` app listing for the Apps settings page.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub icon: String,
    #[allow(dead_code)]
    pub exec: String,
}

pub fn list_apps(limit: usize) -> Vec<DesktopApp> {
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
            let Some(app) = parse_desktop(&path) else {
                continue;
            };
            if !seen.insert(app.id.clone()) {
                continue;
            }
            out.push(app);
            if out.len() >= limit {
                out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                return out;
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn launch(id: &str) -> Result<(), String> {
    let path = resolve(id).ok_or_else(|| format!("desktop not found: {id}"))?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let exec = desktop_key(&text, "Exec").ok_or_else(|| format!("no Exec in {id}"))?;
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

/// Display name for a `.desktop` id (falls back to the id).
pub fn desktop_name_for(id: &str) -> String {
    resolve(id)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| desktop_key(&t, "Name"))
        .unwrap_or_else(|| id.to_string())
}

fn resolve(id: &str) -> Option<PathBuf> {
    let name = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    for dir in dirs {
        let p = dir.join(&name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn parse_desktop(path: &Path) -> Option<DesktopApp> {
    let text = fs::read_to_string(path).ok()?;
    if desktop_key(&text, "NoDisplay").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        return None;
    }
    if desktop_key(&text, "Type").is_some_and(|v| !v.eq_ignore_ascii_case("Application")) {
        return None;
    }
    let name = desktop_key(&text, "Name")?;
    let exec = desktop_key(&text, "Exec")?;
    let id = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("app.desktop")
        .to_string();
    Some(DesktopApp {
        id,
        name,
        icon: desktop_key(&text, "Icon").unwrap_or_default(),
        exec,
    })
}

fn desktop_key(desktop: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in desktop.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn expand_exec(exec: &str) -> Vec<String> {
    let cleaned: String = exec
        .split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote: Option<char> = None;
    for c in cleaned.chars() {
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
