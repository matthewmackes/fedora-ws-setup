//! Local agenda for Notification Center calendar (Win11-like strip).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaEvent {
    /// Day of month 1–31 within the stored month, or 0 = any/today display only.
    pub day: u32,
    pub title: String,
    #[serde(default)]
    pub when: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgendaFile {
    #[serde(default)]
    pub events: Vec<AgendaEvent>,
}

fn agenda_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".local/share/mde/agenda.json")
}

pub fn load() -> AgendaFile {
    let path = agenda_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AgendaFile::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Events for a given day-of-month; empty → caller shows “No events”.
pub fn for_day(day: u32) -> Vec<AgendaEvent> {
    load()
        .events
        .into_iter()
        .filter(|e| e.day == 0 || e.day == day)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_missing() {
        // Should not panic if file absent.
        let _ = load();
    }
}
