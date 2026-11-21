//! A073: PowerToys-style shortcut guide overlay (Super+/).

use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Element, Fill};

use crate::{quiet, Message, Shell};

const ROWS: &[(&str, &str)] = &[
    ("Settings", "Super+I"),
    ("Accessibility", "Super+U"),
    ("Start", "Super"),
    ("Task View", "Super+Tab"),
    ("Performance", "Ctrl+Shift+Esc"),
    ("Widgets", "Super+W"),
    ("Lock", "Super+L"),
    ("Clipboard", "Super+V"),
    ("Search", "Super+S"),
    ("Command Palette", "Super+Alt+Space"),
    ("Shortcut Guide", "Super+/"),
    ("Run", "Super+R"),
    ("Project displays", "Super+P"),
    ("Magnifier", "Super+Plus"),
    ("Screenshot", "Super+Shift+S"),
    ("Color picker", "Super+Shift+C"),
    ("Always on top", "Win+Ctrl+T"),
    ("File Explorer", "Super+E"),
];

impl Shell {
    pub(super) fn view_shortcut_guide(&self) -> Element<'_, Message> {
        let mut col = column![
            row![
                text("Shortcut Guide").size(18),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            text("Hold Super or press Super+/ to review bindings").size(12),
        ]
        .spacing(8);
        for (label, keys) in ROWS {
            col = col.push(
                row![
                    text(*label).size(14).width(Fill),
                    text(*keys).size(13),
                ]
                .spacing(12)
                .padding([4, 0]),
            );
        }
        container(
            scrollable(col.padding([16, 20])).height(420).width(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }
}