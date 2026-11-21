//! W449 Connect / Wireless display flyout + Project layout helpers.

use iced::widget::{column, container, row, text, Space};
use iced::{Element, Fill};

use mde_protocol::OutputInfo;

use crate::{
    chrome_btn, quiet, qs_tile_off, qs_tile_on, Message, Shell,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectMode {
    PcOnly,
    Extend,
    Duplicate,
    SecondOnly,
}

impl ProjectMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::PcOnly => "PC screen only",
            Self::Extend => "Extend",
            Self::Duplicate => "Duplicate",
            Self::SecondOnly => "Second screen only",
        }
    }
}

/// Write `mde-output-layout.txt` for Extend / Duplicate / Second-only.
pub fn apply_project(outputs: &[OutputInfo], mode: ProjectMode) -> Result<String, String> {
    if outputs.is_empty() {
        return Err("No displays detected".into());
    }
    if matches!(mode, ProjectMode::SecondOnly) && outputs.len() < 2 {
        return Err("Second screen only needs another display".into());
    }
    let mut body = String::new();
    match mode {
        ProjectMode::PcOnly => {
            for (i, o) in outputs.iter().enumerate() {
                let enabled = if i == 0 { 1 } else { 0 };
                body.push_str(&format!("{} 0 0 {} {enabled}\n", o.name, o.scale));
            }
        }
        ProjectMode::Extend => {
            let mut x = 0i32;
            for o in outputs {
                body.push_str(&format!("{} {x} 0 {} 1\n", o.name, o.scale));
                x = x.saturating_add(o.width.max(1));
            }
        }
        ProjectMode::Duplicate => {
            for o in outputs {
                body.push_str(&format!("{} 0 0 {} 1\n", o.name, o.scale));
            }
        }
        ProjectMode::SecondOnly => {
            for (i, o) in outputs.iter().enumerate() {
                let enabled = if i == 0 { 0 } else { 1 };
                body.push_str(&format!("{} 0 0 {} {enabled}\n", o.name, o.scale));
            }
        }
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::Path::new(&runtime).join("mde-output-layout.txt");
    std::fs::write(&path, &body).map_err(|e| format!("layout file: {e}"))?;
    let _ = std::process::Command::new("mde-comp")
        .args(["ctl", "apply-profile", "layout"])
        .output();
    Ok(format!("Project · {}", mode.label()))
}

impl Shell {
    pub(super) fn view_connect(&self) -> Element<'_, Message> {
        let outputs = &self.outputs;
        let selected = self
            .share_output
            .clone()
            .or_else(|| outputs.first().map(|o| o.name.clone()))
            .unwrap_or_default();

        let mut out_col = column![text("Share this display").size(12)].spacing(4);
        if outputs.is_empty() {
            out_col = out_col.push(text("No outputs yet · waiting for compositor").size(11));
        } else {
            for o in outputs {
                let on = o.name == selected;
                let label = if o.model.is_empty() {
                    format!("{} · {}×{}", o.name, o.width, o.height)
                } else {
                    format!("{} · {} · {}×{}", o.name, o.model, o.width, o.height)
                };
                let msg = Message::ConnectSelectOutput(o.name.clone());
                out_col = out_col.push(
                    quiet(text(format!("{} {label}", if on { "●" } else { "○" })).size(12))
                        .on_press(msg)
                        .width(Fill),
                );
            }
        }

        let audio_on = self.share_include_audio;
        let audio_btn = quiet(text(if audio_on {
            "☑ Include system audio"
        } else {
            "☐ Include system audio"
        })
        .size(12))
        .on_press(Message::ConnectToggleAudio);

        let share_btn = if self.share_active {
            quiet(text("Stop sharing").size(13))
                .on_press(Message::StopShare)
                .style(qs_tile_on)
                .padding([10, 14])
        } else {
            quiet(text("Start sharing").size(13))
                .on_press(Message::StartShare)
                .style(qs_tile_off)
                .padding([10, 14])
        };

        let project = column![
            text("Project").size(13),
            text("Arrange external displays").size(11),
            row![
                quiet(text("PC only").size(12))
                    .on_press(Message::ProjectMode(ProjectMode::PcOnly))
                    .padding([8, 10]),
                quiet(text("Extend").size(12))
                    .on_press(Message::ProjectMode(ProjectMode::Extend))
                    .padding([8, 10]),
                quiet(text("Duplicate").size(12))
                    .on_press(Message::ProjectMode(ProjectMode::Duplicate))
                    .padding([8, 10]),
            ]
            .spacing(6),
            quiet(text("Second screen only").size(12))
                .on_press(Message::ProjectMode(ProjectMode::SecondOnly))
                .padding([8, 10])
                .width(Fill),
        ]
        .spacing(6);

        container(
            column![
                row![
                    chrome_btn("video-display", "Cast", 20, self.palette.dark),
                    column![
                        text("Wireless display").size(15),
                        text(if self.share_active {
                            "Sharing"
                        } else {
                            "Share or project"
                        })
                        .size(11),
                    ]
                    .spacing(2),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                Space::with_height(8),
                out_col,
                audio_btn,
                share_btn.width(Fill),
                Space::with_height(10),
                project,
            ]
            .spacing(8)
            .padding([14, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_share_pill(&self) -> Element<'_, Message> {
        let name = self
            .share_output
            .as_deref()
            .unwrap_or("display");
        let audio = if self.share_include_audio {
            "audio on"
        } else {
            "audio off"
        };
        container(
            row![
                chrome_btn("video-display", "Share", 18, self.palette.dark),
                column![
                    text(format!("Sharing · {name}")).size(13),
                    text(audio).size(11),
                ]
                .spacing(1)
                .width(Fill),
                quiet(text(if self.share_include_audio {
                    "Mute"
                } else {
                    "Audio"
                })
                .size(12))
                .on_press(Message::ConnectToggleAudio),
                quiet(text("Stop").size(12))
                    .on_press(Message::StopShare)
                    .style(qs_tile_on)
                    .padding([8, 12]),
            ]
            .spacing(10)
            .padding([10, 14])
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mde_protocol::OutputInfo;

    fn out(name: &str, w: i32, x: i32) -> OutputInfo {
        OutputInfo {
            name: name.into(),
            make: String::new(),
            model: String::new(),
            width: w,
            height: 1080,
            scale: 1.0,
            x,
            y: 0,
        }
    }

    #[test]
    fn extend_places_side_by_side() {
        let outs = vec![out("eDP-1", 1920, 0), out("HDMI-A-1", 1920, 0)];
        let runtime = std::env::temp_dir().join(format!("mde-connect-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&runtime);
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        apply_project(&outs, ProjectMode::Extend).unwrap();
        let text = std::fs::read_to_string(runtime.join("mde-output-layout.txt")).unwrap();
        assert!(text.contains("eDP-1 0 0 1 1"));
        assert!(text.contains("HDMI-A-1 1920 0 1 1"));
    }

    #[test]
    fn connect_card_size() {
        assert_eq!(crate::CONNECT_CARD_W, 400);
        assert_eq!(crate::CONNECT_CARD_H, 420);
    }
}
