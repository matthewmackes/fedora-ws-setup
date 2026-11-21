//! Tray items. StatusNotifierWatcher registrations arrive as `sni:` ids.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayItem {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub icon: String,
    /// Theme PNG when `icon` matches a hicolor or Adwaita file.
    #[serde(default)]
    pub icon_path: String,
    #[serde(default)]
    pub bus: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Default)]
pub struct TrayHost {
    pub items: Vec<TrayItem>,
}

impl TrayHost {
    pub fn upsert(&mut self, item: TrayItem) {
        if let Some(existing) = self.items.iter_mut().find(|i| i.id == item.id) {
            *existing = item;
        } else {
            self.items.push(item);
        }
    }

    #[allow(dead_code)] // used when full SNI watcher lands (v1.1)
    pub fn remove(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_replace() {
        let mut h = TrayHost::default();
        h.upsert(TrayItem {
            id: "a".into(),
            title: "A".into(),
            status: "Active".into(),
            icon: String::new(),
            icon_path: String::new(),
            bus: String::new(),
            path: String::new(),
        });
        h.upsert(TrayItem {
            id: "a".into(),
            title: "A2".into(),
            status: "NeedsAttention".into(),
            icon: String::new(),
            icon_path: String::new(),
            bus: String::new(),
            path: String::new(),
        });
        assert_eq!(h.items.len(), 1);
        assert_eq!(h.items[0].title, "A2");
    }

    #[test]
    fn empty_by_default() {
        let h = TrayHost::default();
        assert!(h.items.is_empty());
    }
}
