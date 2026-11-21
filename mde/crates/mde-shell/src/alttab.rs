//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_alttab(&self) -> Element<'_, Message> {
        let indices = self.active_window_indices();
        let desktop_slot = self.alttab_shows_desktop();
        let mut strip = row![].spacing(10);
        let win_take = if desktop_slot { 6 } else { 7 };
        for (local, &global) in indices.iter().enumerate().take(win_take) {
            let win = &self.windows[global];
            let mark: String = win.title.chars().take(16).collect();
            let icon = if win.app_id.is_empty() {
                "application-x-executable"
            } else {
                win.app_id.as_str()
            };
            let face = column![
                app_face(icon, win.title.clone(), 32, false, self.palette.dark),
                text(mark).size(11),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center)
            .width(100);
            let btn = if local == self.alttab_index {
                quiet(face)
                    .on_press(Message::FocusWindow(global))
                    .style(accent_pill)
                    .padding([12, 10])
            } else {
                quiet(face)
                    .on_press(Message::FocusWindow(global))
                    .padding([12, 10])
            };
            strip = strip.push(btn);
        }
        // W208: synthetic Desktop entry when toggle on.
        if desktop_slot {
            let desk = column![
                app_face("user-desktop", "Desktop".into(), 32, false, self.palette.dark),
                text("Desktop").size(11),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center)
            .width(100);
            let selected = self.alttab_index == indices.len();
            strip = strip.push(if selected {
                quiet(desk)
                    .on_press(Message::ShowDesktop)
                    .style(accent_pill)
                    .padding([12, 10])
            } else {
                quiet(desk)
                    .on_press(Message::ShowDesktop)
                    .padding([12, 10])
            });
        }
        if indices.is_empty() && !desktop_slot {
            strip = strip.push(text("No windows").size(14));
        }
        container(
            column![
                container(strip).center_x(Fill),
                text("Alt+Tab · Enter confirms · Esc dismisses").size(11),
            ]
            .spacing(10)
            .align_x(iced::Alignment::Center),
        )
        .padding([18, 20])
        .width(Fill)
        .height(Fill)
        .center_y(Fill)
        .into()
    }
}
