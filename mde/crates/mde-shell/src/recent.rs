//! Recently used apps/files for Start Recommended (W26).
//! Launch counts in `launches.json` power Start “Most used” (W76).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RecentItem {
    /// Launch target: desktop id (e.g. `firefox.desktop`) or file path.
    pub id: String,
    pub label: String,
    pub icon: String,
    pub is_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RecentFile {
    #[serde(default)]
    pub items: Vec<RecentStored>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentStored {
    pub id: String,
    #[serde(default)]
    pub label: String,
}

fn recent_json_path() -> PathBuf {
    first_existing(&[
        ".local/state/mde/recent.json",
        ".local/share/mde/recent.json",
    ])
    .unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/mde/recent.json")
    })
}

fn first_existing(rel: &[&str]) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    for r in rel {
        let p = PathBuf::from(&home).join(r);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn dismiss_json_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/mde/dismiss.json")
}

fn recently_used_xbel() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/recently-used.xbel")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DismissFile {
    #[serde(default)]
    pub ids: Vec<String>,
}

/// Ids the user dismissed from Start Recommended (W85).
pub fn dismissed_ids() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dismiss_json_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<DismissFile>(&text)
        .map(|f| f.ids)
        .unwrap_or_default()
}

/// Persist a dismissed recommended id so it won't show again (W85).
pub fn dismiss(id: &str) {
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    let path = dismiss_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<DismissFile>(&t).ok())
        .unwrap_or_default();
    if !file.ids.iter().any(|e| e == id) {
        file.ids.push(id.into());
    }
    file.ids.truncate(128);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
}

/// Dismiss every currently recommended id via `dismiss.json` (W133).
pub fn dismiss_all(ids: &[String]) -> usize {
    let path = dismiss_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<DismissFile>(&t).ok())
        .unwrap_or_default();
    let mut added = 0usize;
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        if !file.ids.iter().any(|e| e == id) {
            file.ids.push(id.into());
            added += 1;
        }
    }
    file.ids.truncate(128);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
    added
}

/// Load recommended items from `recent.json`, then GTK `recently-used.xbel`.
pub fn load(limit: usize) -> Vec<RecentItem> {
    let dismissed = dismissed_ids();
    let mut out = Vec::new();
    push_from_json(&mut out, limit + dismissed.len().max(8));
    if out.len() < limit + dismissed.len() {
        push_from_xbel(&mut out, limit + dismissed.len());
    }
    out.retain(|item| !dismissed.iter().any(|d| d == &item.id));
    out.truncate(limit);
    out
}

fn push_from_json(out: &mut Vec<RecentItem>, limit: usize) {
    let Ok(text) = std::fs::read_to_string(recent_json_path()) else {
        return;
    };
    let Ok(file) = serde_json::from_str::<RecentFile>(&text) else {
        return;
    };
    for item in file.items {
        if out.len() >= limit {
            break;
        }
        let id = item.id.trim();
        if id.is_empty() || out.iter().any(|e| e.id == id) {
            continue;
        }
        let is_file = Path::new(id).is_absolute() || id.contains('/');
        let label = if item.label.trim().is_empty() {
            default_label(id, is_file)
        } else {
            item.label
        };
        let icon = if is_file {
            "document-open".into()
        } else {
            crate::apps::desktop_icon(id).unwrap_or_else(|| "application-x-executable".into())
        };
        out.push(RecentItem {
            id: id.into(),
            label,
            icon,
            is_file,
        });
    }
}

fn push_from_xbel(out: &mut Vec<RecentItem>, limit: usize) {
    let Ok(text) = std::fs::read_to_string(recently_used_xbel()) else {
        return;
    };
    // Lightweight scrape: href="file://..." and optional <bookmark:application name="..."/>
    for line in text.lines() {
        if out.len() >= limit {
            break;
        }
        let Some(href_start) = line.find("href=\"") else {
            continue;
        };
        let rest = &line[href_start + 6..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let href = &rest[..end];
        let path = if let Some(p) = href.strip_prefix("file://") {
            urlencoding_decode(p)
        } else {
            continue;
        };
        if path.is_empty() || out.iter().any(|e| e.id == path) {
            continue;
        }
        // Prefer .desktop bookmarks as apps; otherwise treat as files.
        if path.ends_with(".desktop") {
            let id = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(path.clone());
            if out.iter().any(|e| e.id == id) {
                continue;
            }
            let label = crate::apps::resolve_desktop(&id)
                .map(|p| crate::apps::desktop_name(&p))
                .unwrap_or_else(|| id.trim_end_matches(".desktop").to_string());
            let icon = crate::apps::desktop_icon(&id)
                .unwrap_or_else(|| "application-x-executable".into());
            out.push(RecentItem {
                id,
                label,
                icon,
                is_file: false,
            });
        } else if Path::new(&path).is_file() || path.contains('/') {
            let label = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            out.push(RecentItem {
                id: path,
                label,
                icon: "document-open".into(),
                is_file: true,
            });
        }
    }
}

fn default_label(id: &str, is_file: bool) -> String {
    if is_file {
        Path::new(id)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| id.to_string())
    } else {
        crate::apps::resolve_desktop(id)
            .map(|p| crate::apps::desktop_name(&p))
            .unwrap_or_else(|| id.trim_end_matches(".desktop").to_string())
    }
}

/// Minimal `%20` / `+` decode for file URIs (no extra crate).
fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(h, 16) {
                    out.push(v as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

/// Up to `limit` recent *files* associated with a taskbar/Start pin (jump list).
/// Prefers GTK `recently-used.xbel` bookmarks whose application name/exec matches
/// the desktop entry; falls back to absolute paths from `recent.json`.
pub fn files_for_app(desktop_id: &str, limit: usize) -> Vec<RecentItem> {
    if limit == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (app_name, exec_base) = app_match_keys(desktop_id);
    push_files_from_xbel(&mut out, limit, &app_name, &exec_base);
    if out.len() < limit {
        // Fallback: absolute paths recorded in recent.json (no app filter).
        let Ok(text) = std::fs::read_to_string(recent_json_path()) else {
            return out;
        };
        let Ok(file) = serde_json::from_str::<RecentFile>(&text) else {
            return out;
        };
        for item in file.items {
            if out.len() >= limit {
                break;
            }
            let id = item.id.trim();
            let is_file = Path::new(id).is_absolute() || id.contains('/');
            if !is_file || id.is_empty() || out.iter().any(|e| e.id == id) {
                continue;
            }
            let label = if item.label.trim().is_empty() {
                default_label(id, true)
            } else {
                item.label
            };
            out.push(RecentItem {
                id: id.into(),
                label,
                icon: "document-open".into(),
                is_file: true,
            });
        }
    }
    out.truncate(limit);
    out
}

fn app_match_keys(desktop_id: &str) -> (String, String) {
    let path = crate::apps::resolve_desktop(desktop_id);
    let name = path
        .as_ref()
        .map(|p| crate::apps::desktop_name(p))
        .unwrap_or_else(|| desktop_id.trim_end_matches(".desktop").to_string());
    let exec_base = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("Exec=")
                    .map(|e| e.trim().to_string())
            })
        })
        .and_then(|exec| {
            let first = exec.split_whitespace().next()?.to_string();
            Path::new(&first)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| desktop_id.trim_end_matches(".desktop").to_string());
    (name.to_lowercase(), exec_base.to_lowercase())
}

fn push_files_from_xbel(
    out: &mut Vec<RecentItem>,
    limit: usize,
    app_name: &str,
    exec_base: &str,
) {
    let Ok(text) = std::fs::read_to_string(recently_used_xbel()) else {
        return;
    };
    // Walk bookmarks; association lines usually follow href within a few lines.
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && out.len() < limit {
        let line = lines[i];
        let Some(href_start) = line.find("href=\"") else {
            i += 1;
            continue;
        };
        let rest = &line[href_start + 6..];
        let Some(end) = rest.find('"') else {
            i += 1;
            continue;
        };
        let href = &rest[..end];
        let path = if let Some(p) = href.strip_prefix("file://") {
            urlencoding_decode(p)
        } else {
            i += 1;
            continue;
        };
        if path.is_empty() || path.ends_with(".desktop") || out.iter().any(|e| e.id == path) {
            i += 1;
            continue;
        }
        // Peek ahead for application name=/exec= within this bookmark block.
        let mut matched = app_name.is_empty() && exec_base.is_empty();
        for j in (i + 1)..(i + 24).min(lines.len()) {
            let l = lines[j];
            if l.contains("<bookmark ") || l.contains("</bookmark>") {
                break;
            }
            if let Some(n) = attr_value(l, "name=\"") {
                if !app_name.is_empty() && n.to_lowercase() == app_name {
                    matched = true;
                    break;
                }
            }
            if let Some(ex) = attr_value(l, "exec=\"") {
                let ex_l = ex.to_lowercase();
                if (!exec_base.is_empty() && ex_l.contains(exec_base))
                    || (!app_name.is_empty() && ex_l.contains(app_name))
                {
                    matched = true;
                    break;
                }
            }
        }
        if matched && (Path::new(&path).is_file() || path.contains('/')) {
            let label = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            out.push(RecentItem {
                id: path,
                label,
                icon: "document-open".into(),
                is_file: true,
            });
        }
        i += 1;
    }
}

fn attr_value(line: &str, key: &str) -> Option<String> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    let raw = &rest[..end];
    // xbel often wraps exec in &apos;...&apos;
    let cleaned = raw
        .replace("&apos;", "")
        .replace("&quot;", "")
        .replace("&amp;", "&");
    Some(cleaned)
}

fn launches_json_path() -> PathBuf {
    first_existing(&[
        ".local/state/mde/launches.json",
        ".local/share/mde/launches.json",
    ])
    .unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/mde/launches.json")
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LaunchesFile {
    #[serde(default)]
    pub counts: Vec<LaunchCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LaunchCount {
    pub id: String,
    #[serde(default)]
    pub count: u64,
}

/// Record a launched desktop id into recent.json + bump launches.json (best-effort).
pub fn record_launch(id: &str) {
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    let path = recent_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<RecentFile>(&t).ok())
        .unwrap_or_default();
    file.items.retain(|e| e.id != id);
    file.items.insert(
        0,
        RecentStored {
            id: id.into(),
            label: default_label(id, false),
        },
    );
    file.items.truncate(24);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
    bump_launch_count(id);
}

fn bump_launch_count(id: &str) {
    let path = launches_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<LaunchesFile>(&t).ok())
        .unwrap_or_default();
    if let Some(slot) = file.counts.iter_mut().find(|e| e.id == id) {
        slot.count = slot.count.saturating_add(1);
    } else {
        file.counts.push(LaunchCount {
            id: id.into(),
            count: 1,
        });
    }
    file.counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    file.counts.truncate(48);
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
}

/// Up to `limit` recent shell/launch commands for Start search empty state (W115).
/// Prefers chronological `recent.json`, then `launches.json` ids.
pub fn recent_commands(limit: usize) -> Vec<RecentItem> {
    if limit == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    push_from_json(&mut out, limit);
    if out.len() < limit {
        for item in most_used(limit) {
            if out.len() >= limit {
                break;
            }
            if out.iter().any(|e| e.id == item.id) {
                continue;
            }
            out.push(item);
        }
    }
    out.truncate(limit);
    out
}

/// Up to `limit` frequent directories from recent.json / recently-used.xbel (W275).
/// Prefers parent folders of recent files, ranked by occurrence.
pub fn frequent_places(limit: usize) -> Vec<RecentItem> {
    if limit == 0 {
        return Vec::new();
    }
    let mut counts: Vec<(String, u32)> = Vec::new();
    let mut bump = |dir: String| {
        if dir.is_empty() {
            return;
        }
        if let Some((_, n)) = counts.iter_mut().find(|(d, _)| d == &dir) {
            *n = n.saturating_add(1);
        } else {
            counts.push((dir, 1));
        }
    };
    // recent.json absolute paths → parent dirs.
    if let Ok(text) = std::fs::read_to_string(recent_json_path()) {
        if let Ok(file) = serde_json::from_str::<RecentFile>(&text) {
            for item in file.items {
                let id = item.id.trim();
                if id.is_empty() {
                    continue;
                }
                let path = Path::new(id);
                if path.is_dir() {
                    bump(path.to_string_lossy().into_owned());
                } else if path.is_absolute() || id.contains('/') {
                    if let Some(parent) = path.parent() {
                        if parent.as_os_str().len() > 1 {
                            bump(parent.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
    }
    // xbel file:// bookmarks → parent dirs.
    if let Ok(text) = std::fs::read_to_string(recently_used_xbel()) {
        for line in text.lines() {
            let Some(href_start) = line.find("href=\"") else {
                continue;
            };
            let rest = &line[href_start + 6..];
            let Some(end) = rest.find('"') else {
                continue;
            };
            let href = &rest[..end];
            let Some(raw) = href.strip_prefix("file://") else {
                continue;
            };
            let path_s = urlencoding_decode(raw);
            let path = Path::new(&path_s);
            if path_s.ends_with(".desktop") {
                continue;
            }
            if path.is_dir() {
                bump(path_s);
            } else if let Some(parent) = path.parent() {
                if parent.as_os_str().len() > 1 {
                    bump(parent.to_string_lossy().into_owned());
                }
            }
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut out = Vec::new();
    for (dir, _) in counts {
        if out.len() >= limit {
            break;
        }
        if !Path::new(&dir).is_dir() && !dir.contains('/') {
            continue;
        }
        let label = Path::new(&dir)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| dir.clone());
        out.push(RecentItem {
            id: dir,
            label,
            icon: "folder".into(),
            is_file: true,
        });
    }
    out
}

/// Top `limit` apps by launch count (Start “Most used”, W76).
pub fn most_used(limit: usize) -> Vec<RecentItem> {
    if limit == 0 {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(launches_json_path()) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_str::<LaunchesFile>(&text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in file.counts {
        if out.len() >= limit {
            break;
        }
        let id = entry.id.trim();
        if id.is_empty() || entry.count == 0 || out.iter().any(|e: &RecentItem| e.id == id) {
            continue;
        }
        // Skip absolute file paths — Most used is for apps.
        if Path::new(id).is_absolute() || id.contains('/') {
            continue;
        }
        let label = default_label(id, false);
        let icon = crate::apps::desktop_icon(id)
            .unwrap_or_else(|| "application-x-executable".into());
        out.push(RecentItem {
            id: id.into(),
            label,
            icon,
            is_file: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_spaces() {
        assert_eq!(urlencoding_decode("a%20b"), "a b");
    }
}
