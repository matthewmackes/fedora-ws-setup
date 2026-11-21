//! org.freedesktop.Notifications host (in-process with the shell).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: u8,
    #[serde(default)]
    pub created_ms: u64,
    /// Freedesktop action pairs: `(key, label)`.
    #[serde(default)]
    pub actions: Vec<(String, String)>,
    /// Optional 0.0–1.0 progress from Notify hints (`value` / `percentage`) — W438.
    #[serde(default)]
    pub progress: Option<f32>,
    /// Freedesktop `desktop-entry` hint (A004 badge protocol).
    #[serde(default)]
    pub desktop_entry: Option<String>,
}

/// Notification held out of the list until `until` (W125).
#[derive(Debug, Clone)]
pub struct SnoozedNotification {
    pub notif: Notification,
    pub until: Instant,
}

#[derive(Debug, Default)]
pub struct NotificationCentre {
    pub items: Vec<Notification>,
    pub next_id: u32,
    pub dnd: bool,
    /// Focus assist priority-only (W86): allow listed apps + critical.
    pub priority: bool,
    /// Lowercase app name / id fragments allowed while priority is on.
    pub priority_apps: Vec<String>,
    /// Per-app mute list (W105); filtered from toasts/list until unmuted.
    pub muted_apps: Vec<String>,
    /// Snoozed items re-pushed after `until` (W125).
    pub snoozed: Vec<SnoozedNotification>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistFile {
    next_id: u32,
    items: Vec<Notification>,
}

impl NotificationCentre {
    /// `~/.local/state/mde/notifications.json` (W96).
    pub fn persist_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".local/state/mde/notifications.json"))
    }

    /// Reload history from disk (cap 50). Does not touch dnd / priority / mute.
    pub fn load_persisted(&mut self) {
        let Some(path) = Self::persist_path() else {
            return;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            return;
        };
        let Ok(file) = serde_json::from_str::<PersistFile>(&text) else {
            return;
        };
        let mut items = file.items;
        if items.len() > 50 {
            items.truncate(50);
        }
        let max_id = items.iter().map(|n| n.id).max().unwrap_or(0);
        self.next_id = file.next_id.max(max_id);
        self.items = items;
    }

    pub fn persist(&self) {
        let Some(path) = Self::persist_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut items = self.items.clone();
        if items.len() > 50 {
            items.truncate(50);
        }
        let file = PersistFile {
            next_id: self.next_id,
            items,
        };
        if let Ok(text) = serde_json::to_string_pretty(&file) {
            let _ = fs::write(path, text);
        }
    }

    pub fn push(&mut self, app_name: &str, summary: &str, body: &str, urgency: u8) -> Option<u32> {
        if self.dnd && urgency < 2 {
            return None;
        }
        if is_muted_app(app_name, &self.muted_apps) {
            return None;
        }
        if self.priority && urgency < 2 && !is_priority_app(app_name, &self.priority_apps) {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.items.insert(
            0,
            Notification {
                id,
                app_name: app_name.into(),
                summary: summary.into(),
                body: body.into(),
                urgency,
                created_ms: now_ms(),
                actions: Vec::new(),
                progress: None,
                desktop_entry: None,
            },
        );
        if self.items.len() > 50 {
            self.items.truncate(50);
        }
        self.persist();
        Some(id)
    }

    pub fn push_with_actions(
        &mut self,
        app_name: &str,
        summary: &str,
        body: &str,
        urgency: u8,
        actions: Vec<(String, String)>,
    ) -> Option<u32> {
        let id = self.push(app_name, summary, body, urgency)?;
        if let Some(item) = self.items.iter_mut().find(|n| n.id == id) {
            item.actions = actions;
        }
        self.persist();
        Some(id)
    }

    pub fn dismiss(&mut self, id: u32) {
        self.items.retain(|n| n.id != id);
        self.persist();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.persist();
    }

    /// Clear only notifications whose `app_name` matches (W165). Empty/`App` → blank names.
    pub fn clear_group(&mut self, app_name: &str) -> Vec<u32> {
        let key = app_name.trim();
        let match_empty = key.is_empty() || key.eq_ignore_ascii_case("App");
        let mut removed = Vec::new();
        self.items.retain(|n| {
            let name = n.app_name.trim();
            let hit = if match_empty {
                name.is_empty()
            } else {
                name.eq_ignore_ascii_case(key)
            };
            if hit {
                removed.push(n.id);
                false
            } else {
                true
            }
        });
        if !removed.is_empty() {
            self.persist();
        }
        removed
    }

    /// Remove from list and re-push after `for_secs` (W125).
    pub fn snooze(&mut self, id: u32, for_secs: u64) -> bool {
        let Some(pos) = self.items.iter().position(|n| n.id == id) else {
            return false;
        };
        let notif = self.items.remove(pos);
        self.snoozed.push(SnoozedNotification {
            notif,
            until: Instant::now() + Duration::from_secs(for_secs),
        });
        self.persist();
        true
    }

    /// Re-insert snoozed notifications whose `until` has elapsed (W125).
    pub fn wake_snoozed(&mut self) -> usize {
        let now = Instant::now();
        let mut woke = 0usize;
        let mut still = Vec::new();
        for entry in self.snoozed.drain(..) {
            if entry.until <= now {
                self.items.insert(0, entry.notif);
                woke += 1;
            } else {
                still.push(entry);
            }
        }
        self.snoozed = still;
        if woke > 0 {
            if self.items.len() > 50 {
                self.items.truncate(50);
            }
            self.persist();
        }
        woke
    }

    /// Drop normal notifications older than `max_age`. Critical (urgency >= 2) stays.
    pub fn expire(&mut self, max_age: std::time::Duration) -> Vec<u32> {
        let now = now_ms();
        let max = max_age.as_millis() as u64;
        let mut removed = Vec::new();
        self.items.retain(|n| {
            let keep = n.urgency >= 2 || now.saturating_sub(n.created_ms) < max;
            if !keep {
                removed.push(n.id);
            }
            keep
        });
        if !removed.is_empty() {
            self.persist();
        }
        removed
    }
}

/// Pair the flat freedesktop action list (`key, label, key, label, …`).
pub fn parse_actions(flat: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut iter = flat.iter();
    while let Some(key) = iter.next() {
        let Some(label) = iter.next() else {
            break;
        };
        if key.is_empty() {
            continue;
        }
        out.push((key.clone(), label.clone()));
        if out.len() == 6 {
            break;
        }
    }
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn is_priority_app(app_name: &str, allow: &[String]) -> bool {
    let needle = app_name.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    // Always allow the shell's own demo / system notifies.
    if needle == "mde-shell" || needle == "mde" {
        return true;
    }
    allow.iter().any(|a| {
        let a = a.to_lowercase();
        !a.is_empty() && (needle.contains(&a) || a.contains(&needle))
    })
}

/// Build allowlist from taskbar pins + optional `~/.config/mde/priority-apps.txt`.
pub fn load_priority_apps(pins: &[String]) -> Vec<String> {
    let mut out: Vec<String> = pins
        .iter()
        .map(|p| {
            p.trim_end_matches(".desktop")
                .rsplit('.')
                .next()
                .unwrap_or(p)
                .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::PathBuf::from(home).join(".config/mde/priority-apps.txt");
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let s = line.trim();
            if s.is_empty() || s.starts_with('#') {
                continue;
            }
            let low = s.to_lowercase();
            if !out.iter().any(|e| e == &low) {
                out.push(low);
            }
        }
    }
    out
}

pub fn is_muted_app(app_name: &str, muted: &[String]) -> bool {
    let needle = app_name.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    muted.iter().any(|m| {
        let m = m.to_lowercase();
        !m.is_empty() && (needle == m || needle.contains(&m) || m.contains(&needle))
    })
}

fn muted_apps_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".config/mde/muted-apps.txt")
}

/// Load per-app mute list from `~/.config/mde/muted-apps.txt` (W105).
pub fn load_muted_apps() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string(muted_apps_path()) {
        for line in text.lines() {
            let s = line.trim();
            if s.is_empty() || s.starts_with('#') {
                continue;
            }
            let low = s.to_lowercase();
            if !out.iter().any(|e| e == &low) {
                out.push(low);
            }
        }
    }
    out
}

fn save_muted_apps(apps: &[String]) {
    let path = muted_apps_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut body = String::from("# MDE muted notification apps (one per line)\n");
    for a in apps {
        body.push_str(a);
        body.push('\n');
    }
    let _ = std::fs::write(path, body);
}

/// Add `app_name` to the mute list and persist (W105).
pub fn mute_app(app_name: &str, muted: &mut Vec<String>) {
    let low = app_name.trim().to_lowercase();
    if low.is_empty() {
        return;
    }
    if !muted.iter().any(|m| m == &low) {
        muted.push(low);
        save_muted_apps(muted);
    }
}

/// Remove `app_name` from the mute list and persist (W105).
pub fn unmute_app(app_name: &str, muted: &mut Vec<String>) {
    let low = app_name.trim().to_lowercase();
    muted.retain(|m| m != &low);
    save_muted_apps(muted);
}

fn priority_apps_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".config/mde/priority-apps.txt")
}

fn save_priority_apps_file(extra: &[String]) {
    let path = priority_apps_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut existing: Vec<String> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines() {
            let s = line.trim();
            if s.is_empty() || s.starts_with('#') {
                continue;
            }
            let low = s.to_lowercase();
            if !existing.iter().any(|e| e == &low) {
                existing.push(low);
            }
        }
    }
    for a in extra {
        let low = a.trim().to_lowercase();
        if !low.is_empty() && !existing.iter().any(|e| e == &low) {
            existing.push(low);
        }
    }
    let mut body = String::from("# MDE priority notification apps (one per line)\n");
    for a in &existing {
        body.push_str(a);
        body.push('\n');
    }
    let _ = std::fs::write(path, body);
}

/// Elevate an app into `priority-apps.txt` and the in-memory allowlist (W156).
pub fn elevate_priority_app(app_name: &str, priority_apps: &mut Vec<String>) {
    let low = app_name.trim().to_lowercase();
    if low.is_empty() {
        return;
    }
    if !priority_apps.iter().any(|p| p == &low) {
        priority_apps.push(low.clone());
    }
    save_priority_apps_file(&[low]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnd_suppresses_normal() {
        let mut c = NotificationCentre {
            dnd: true,
            ..Default::default()
        };
        assert!(c.push("app", "hi", "", 1).is_none());
        assert!(c.push("app", "crit", "", 2).is_some());
    }

    #[test]
    fn priority_allows_listed() {
        let mut c = NotificationCentre {
            priority: true,
            priority_apps: vec!["firefox".into()],
            ..Default::default()
        };
        assert!(c.push("unknown", "hi", "", 1).is_none());
        assert!(c.push("Firefox", "mail", "", 1).is_some());
        assert!(c.push("mde-shell", "demo", "", 1).is_some());
    }

    #[test]
    fn parse_action_pairs() {
        let flat = vec![
            "default".into(),
            "Open".into(),
            "".into(),
            "skip".into(),
            "later".into(),
        ];
        let pairs = parse_actions(&flat);
        assert_eq!(pairs, vec![("default".into(), "Open".into())]);
    }

    #[test]
    fn mute_filters_app() {
        let mut c = NotificationCentre {
            muted_apps: vec!["slack".into()],
            ..Default::default()
        };
        assert!(c.push("Slack", "hi", "", 1).is_none());
        assert!(c.push("firefox", "mail", "", 1).is_some());
    }

    #[test]
    fn snooze_removes_and_wakes() {
        let mut c = NotificationCentre::default();
        let id = c.push("app", "hi", "", 1).unwrap();
        assert!(c.snooze(id, 0));
        assert!(c.items.is_empty());
        assert_eq!(c.snoozed.len(), 1);
        // until is Instant::now()+0 → already elapsed on next wake.
        assert_eq!(c.wake_snoozed(), 1);
        assert_eq!(c.items.len(), 1);
        assert!(c.snoozed.is_empty());
    }
}
