//! Tabs-only windowing: one server-side tab strip per workspace group.

use anyhow::Result;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub app_id: String,
}

#[derive(Debug, Default)]
pub struct TabStrip {
    pub tabs: Vec<Tab>,
    pub active: Option<u64>,
}

impl TabStrip {
    pub fn add(&mut self, tab: Tab) {
        self.active = Some(tab.id);
        self.tabs.push(tab);
    }

    pub fn activate(&mut self, id: u64) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    pub fn close(&mut self, id: u64) {
        self.tabs.retain(|t| t.id != id);
        if self.active == Some(id) {
            self.active = self.tabs.last().map(|t| t.id);
        }
    }
}

pub fn selftest() -> Result<()> {
    let mut strip = TabStrip::default();
    strip.add(Tab {
        id: 1,
        title: "Firefox".into(),
        app_id: "firefox".into(),
    });
    strip.add(Tab {
        id: 2,
        title: "Files".into(),
        app_id: "thunar".into(),
    });
    assert!(strip.activate(1));
    strip.close(1);
    assert_eq!(strip.active, Some(2));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_lifecycle() {
        selftest().unwrap();
    }
}
