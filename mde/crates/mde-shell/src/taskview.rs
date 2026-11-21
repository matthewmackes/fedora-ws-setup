//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_taskview(&self) -> Element<'_, Message> {
        let count = self.desktop_count.max(1);
        let mut desktop_cols = row![].spacing(14);

        for desk in 0..count {
            let mut col_wins = column![].spacing(8);
            let wins_here: Vec<(usize, &WindowInfo)> = self
                .windows
                .iter()
                .enumerate()
                .filter(|(_, w)| w.workspace == desk)
                .collect();

            let win_count = wins_here.len();
            if wins_here.is_empty() {
                col_wins = col_wins.push(text("No windows").size(12));
            } else {
                for (index, win) in &wins_here {
                    let icon = if win.app_id.is_empty() {
                        "application-x-executable"
                    } else {
                        win.app_id.as_str()
                    };
                    let title_short: String = win.title.chars().take(22).collect();
                    let app_name = if win.app_id.is_empty() {
                        "App".to_string()
                    } else {
                        win.app_id
                            .rsplit('.')
                            .next()
                            .unwrap_or(win.app_id.as_str())
                            .to_string()
                    };
                    let mut move_row = row![].spacing(4);
                    for dest in 0..count {
                        if dest == desk {
                            continue;
                        }
                        move_row = move_row.push(
                            quiet(text(format!("→{}", dest + 1)).size(10))
                                .on_press(Message::MoveWindowToDesktop {
                                    window: *index,
                                    desktop: dest,
                                })
                                .padding([2, 4]),
                        );
                    }
                    let tile = quiet(
                        column![
                            container(app_face(icon, win.title.clone(), 48, false, self.palette.dark))
                                .width(Fill)
                                .center_x(Fill)
                                .padding([18, 10]),
                            text(title_short).size(13),
                            text(app_name).size(11),
                            move_row,
                        ]
                        .spacing(3)
                        .width(176)
                        .align_x(iced::Alignment::Center),
                    )
                    .on_press(Message::FocusWindow(*index))
                    .style(glass_button)
                    .padding([10, 8]);
                    col_wins = col_wins.push(flyout_inset(tile, &self.palette));
                }
            }

            let badge = format!("{win_count}");
            let desk_name = self.cfg.active().desktops.name_for(desk);
            let header = if desk == self.desktop_active {
                quiet(
                    column![
                        row![
                            text(desk_name.clone()).size(14),
                            Space::with_width(6),
                            text(badge.clone()).size(12),
                        ]
                        .align_y(iced::Alignment::Center),
                        text("Current desktop").size(11),
                    ]
                    .spacing(2)
                    .align_x(iced::Alignment::Center)
                    .width(Fill),
                )
                .style(accent_pill)
                .padding([12, 14])
                .on_press(Message::SwitchDesktop(desk))
            } else {
                quiet(
                    column![
                        row![
                            text(desk_name).size(14),
                            Space::with_width(6),
                            text(badge).size(12),
                        ]
                        .align_y(iced::Alignment::Center),
                        text("Switch").size(11),
                    ]
                    .spacing(2)
                    .align_x(iced::Alignment::Center)
                    .width(Fill),
                )
                .style(glass_button)
                .padding([12, 14])
                .on_press(Message::SwitchDesktop(desk))
            };

            let card = column![header, Space::with_height(8), col_wins]
                .spacing(4)
                .width(200);
            desktop_cols = desktop_cols.push(flyout_inset(card, &self.palette));
        }

        let new_btn = quiet(text("+ New desktop").size(12))
            .on_press(Message::NewDesktop)
            .style(glass_button)
            .padding([10, 14]);
        let close_btn = quiet(text("Close desktop").size(12))
            .on_press(Message::CloseDesktop)
            .style(glass_button)
            .padding([10, 14]);

        container(
            column![
                row![
                    text("Task View").size(20),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                text("Click a desktop or window · →N moves a window").size(12),
                Space::with_height(8),
                scrollable(desktop_cols).height(Fill),
                Space::with_height(10),
                hairline(&self.palette),
                Space::with_height(8),
                row![new_btn, close_btn, Space::with_width(Fill),]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
            ]
            .spacing(6),
        )
        .padding([20, 22])
        .width(Fill)
        .height(Fill)
        .into()
    }
}
