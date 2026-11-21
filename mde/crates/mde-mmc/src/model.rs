//! Console tree model shared by host and snap-ins.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleSpec {
    pub name: String,
    #[serde(default)]
    pub snapins: Vec<String>,
    #[serde(default = "default_true")]
    pub tree: bool,
    #[serde(default = "default_true")]
    pub actions: bool,
    #[serde(default)]
    pub builtin: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub console: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for ConsoleSpec {
    fn default() -> Self {
        Self {
            name: "Console1".into(),
            snapins: Vec::new(),
            tree: true,
            actions: true,
            builtin: None,
            path: None,
            console: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub description: String,
    pub snapin: String,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        snapin: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: icon.into(),
            description: String::new(),
            snapin: snapin.into(),
            children: Vec::new(),
        }
    }

    pub fn with_desc(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }

    pub fn with_children(mut self, kids: Vec<TreeNode>) -> Self {
        self.children = kids;
        self
    }

    pub fn find<'a>(&'a self, id: &str) -> Option<&'a TreeNode> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    pub fn parent_of<'a>(&'a self, id: &str) -> Option<&'a TreeNode> {
        for c in &self.children {
            if c.id == id {
                return Some(self);
            }
            if let Some(p) = c.parent_of(id) {
                return Some(p);
            }
        }
        None
    }

    pub fn flatten(&self, depth: usize, out: &mut Vec<(usize, TreeNode)>) {
        out.push((depth, self.clone()));
        for c in &self.children {
            c.flatten(depth + 1, out);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub destructive: bool,
}

impl ActionItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            destructive: false,
        }
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ResultRow {
    pub id: String,
    pub cells: Vec<String>,
    pub icon: String,
}

/// Semantic tone for summary chips, gauges, and status-colored cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Good,
    Warn,
    Bad,
    Accent,
}

/// Compact KPI strip under the results banner (Win11 Settings-style facts).
#[derive(Debug, Clone)]
pub struct SummaryChip {
    pub label: String,
    pub value: String,
    pub tone: Tone,
}

impl SummaryChip {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            tone: Tone::Neutral,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// Horizontal usage / level bar for live metrics (PerfMon-style).
#[derive(Debug, Clone)]
pub struct Gauge {
    pub label: String,
    /// 0.0 ..= 100.0 (clamped when drawn)
    pub value: f32,
    pub display: String,
    pub tone: Tone,
}

impl Gauge {
    pub fn new(label: impl Into<String>, value: f32, display: impl Into<String>) -> Self {
        let tone = if value >= 90.0 {
            Tone::Bad
        } else if value >= 75.0 {
            Tone::Warn
        } else {
            Tone::Good
        };
        Self {
            label: label.into(),
            value,
            display: display.into(),
            tone,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// Key/value property sheet block — preferred over raw monospace dumps.
#[derive(Debug, Clone)]
pub struct Property {
    pub key: String,
    pub value: String,
}

impl Property {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResultsView {
    pub columns: Vec<String>,
    /// Relative flex weights per column; empty = equal Fill widths.
    pub column_weights: Vec<u16>,
    pub rows: Vec<ResultRow>,
    pub detail: String,
    pub status: String,
    pub summaries: Vec<SummaryChip>,
    pub gauges: Vec<Gauge>,
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub title: String,
    pub progress: Option<f32>,
    pub cancellable: bool,
    pub done: bool,
    pub failed: bool,
    pub message: String,
}
