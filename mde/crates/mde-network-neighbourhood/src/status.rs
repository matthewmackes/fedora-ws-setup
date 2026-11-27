//! Connection history (Activity page, "Last connection") and host reachability
//! tracking (Offline badge, last-seen, back-online notification).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATE_DIR: &str = "/var/lib/network-neighbourhood";
const HISTORY_CAP: usize = 200;

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub name: String,
    pub ts: u64,
    pub ok: bool,
    pub phrase: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub cause: String,
    #[serde(default)]
    pub steps: Vec<String>,
}

fn history_path(dir: &Path) -> PathBuf {
    dir.join("history.jsonl")
}

/// Append an entry, trimming to the newest `HISTORY_CAP`.
pub fn record_in(dir: &Path, entry: &HistoryEntry) {
    let _ = fs::create_dir_all(dir);
    let path = history_path(dir);
    let mut lines: Vec<String> = fs::read_to_string(&path)
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default();
    if let Ok(line) = serde_json::to_string(entry) {
        lines.push(line);
    }
    if lines.len() > HISTORY_CAP {
        lines.drain(0..lines.len() - HISTORY_CAP);
    }
    if let Ok(mut f) = fs::File::create(&path) {
        let _ = f.write_all(lines.join("\n").as_bytes());
        let _ = f.write_all(b"\n");
    }
}

/// Newest first, optionally for one place.
pub fn recent_in(dir: &Path, limit: usize, place: Option<&str>) -> Vec<HistoryEntry> {
    let text = fs::read_to_string(history_path(dir)).unwrap_or_default();
    let mut v: Vec<HistoryEntry> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|e: &HistoryEntry| place.map_or(true, |p| e.id == p))
        .collect();
    v.reverse();
    v.truncate(limit);
    v
}

pub fn record(entry: &HistoryEntry) {
    record_in(Path::new(STATE_DIR), entry);
}

pub fn recent(limit: usize, place: Option<&str>) -> Vec<HistoryEntry> {
    recent_in(Path::new(STATE_DIR), limit, place)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Seen {
    pub reachable: bool,
    pub last_seen: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Transition {
    None,
    WentOffline,
    CameBack,
}

/// Fold a fresh probe into the previous state.
pub fn observe(prev: Option<&Seen>, reachable: bool, now: u64) -> (Seen, Transition) {
    let last_seen = if reachable { Some(now) } else { prev.and_then(|p| p.last_seen) };
    let t = match prev {
        Some(p) if p.reachable && !reachable => Transition::WentOffline,
        Some(p) if !p.reachable && reachable && p.last_seen.is_some() => Transition::CameBack,
        _ => Transition::None,
    };
    (Seen { reachable, last_seen }, t)
}

pub fn load_seen_in(dir: &Path) -> HashMap<String, Seen> {
    fs::read_to_string(dir.join("seen.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_seen_in(dir: &Path, map: &HashMap<String, Seen>) {
    let _ = fs::create_dir_all(dir);
    if let Ok(s) = serde_json::to_string(map) {
        let _ = fs::write(dir.join("seen.json"), s);
    }
}

pub fn load_seen() -> HashMap<String, Seen> {
    load_seen_in(Path::new(STATE_DIR))
}

pub fn save_seen(map: &HashMap<String, Seen>) {
    save_seen_in(Path::new(STATE_DIR), map)
}

/// "just now", "5 min ago", "3 h ago", "2 d ago".
pub fn ago(ts: u64, now: u64) -> String {
    let d = now.saturating_sub(ts);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        _ => format!("{} d ago", d / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, ok: bool) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            name: id.into(),
            ts: 1,
            ok,
            phrase: String::new(),
            error: String::new(),
            cause: String::new(),
            steps: vec![],
        }
    }

    #[test]
    fn history_is_newest_first_and_filterable() {
        let d = tempfile::tempdir().unwrap();
        record_in(d.path(), &entry("a", true));
        record_in(d.path(), &entry("b", false));
        record_in(d.path(), &entry("a", false));
        let all = recent_in(d.path(), 10, None);
        assert_eq!(all.len(), 3);
        assert!(!all[0].ok && all[0].id == "a");
        assert_eq!(recent_in(d.path(), 10, Some("a")).len(), 2);
        assert_eq!(recent_in(d.path(), 1, None).len(), 1);
    }

    #[test]
    fn history_is_capped() {
        let d = tempfile::tempdir().unwrap();
        for _ in 0..(HISTORY_CAP + 25) {
            record_in(d.path(), &entry("a", true));
        }
        assert_eq!(recent_in(d.path(), 1000, None).len(), HISTORY_CAP);
    }

    #[test]
    fn offline_then_back() {
        let (s1, t1) = observe(None, true, 100);
        assert_eq!(t1, Transition::None);
        let (s2, t2) = observe(Some(&s1), false, 200);
        assert_eq!(t2, Transition::WentOffline);
        assert_eq!(s2.last_seen, Some(100));
        let (s3, t3) = observe(Some(&s2), true, 300);
        assert_eq!(t3, Transition::CameBack);
        assert_eq!(s3.last_seen, Some(300));
    }

    #[test]
    fn never_seen_host_does_not_announce_return() {
        let (s1, _) = observe(None, false, 100);
        let (_, t) = observe(Some(&s1), true, 200);
        assert_eq!(t, Transition::None);
    }

    #[test]
    fn ago_text() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 300), "5 min ago");
        assert_eq!(ago(0, 7200), "2 h ago");
        assert_eq!(ago(0, 172800), "2 d ago");
    }
}
