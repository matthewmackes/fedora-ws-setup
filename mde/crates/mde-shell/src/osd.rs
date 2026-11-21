//! Volume / brightness OSD surface state.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdKind {
    Volume,
    Brightness,
    #[allow(dead_code)]
    Mic,
    /// W424 voice typing pill.
    Dictation,
}

#[derive(Debug, Clone)]
pub struct OsdState {
    pub kind: OsdKind,
    pub level: f32,
    pub muted: bool,
}

impl OsdState {
    pub fn volume(level: f32, muted: bool) -> Self {
        Self {
            kind: OsdKind::Volume,
            level: level.clamp(0.0, 1.0),
            muted,
        }
    }

    pub fn mic(muted: bool) -> Self {
        Self {
            kind: OsdKind::Mic,
            level: if muted { 0.0 } else { 1.0 },
            muted,
        }
    }

    pub fn dictation() -> Self {
        Self {
            kind: OsdKind::Dictation,
            level: 1.0,
            muted: false,
        }
    }

    pub fn label(&self) -> String {
        let pct = (self.level * 100.0).round() as i32;
        match self.kind {
            OsdKind::Volume if self.muted => format!("Volume muted ({pct}%)"),
            OsdKind::Volume => format!("Volume {pct}%"),
            OsdKind::Brightness => format!("Brightness {pct}%"),
            OsdKind::Mic if self.muted => "Microphone muted".into(),
            OsdKind::Mic => format!("Microphone {pct}%"),
            OsdKind::Dictation => "Listening… · Super+H to stop".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_label() {
        assert!(OsdState::volume(0.5, false).label().contains("50"));
    }
}
