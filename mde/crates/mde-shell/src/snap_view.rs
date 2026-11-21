//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_snap_assist(&self) -> Element<'_, Message> {
        let side = self.snap_assist_side.as_deref().unwrap_or("left");
        let opposite = if side == "left" { "right" } else { "left" };
        let mut list = column![
            row![
                text("Snap assist").size(15),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(format!("Pick a window to fill the {opposite} half")).size(12),
            Space::with_height(6),
        ]
        .spacing(6);
        // W242: list other windows by title → focus + snap remaining half.
        let mut shown = 0usize;
        for (index, win) in self
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| w.workspace == self.desktop_active)
        {
            if shown >= 8 {
                break;
            }
            let title = if win.title.trim().is_empty() {
                if win.app_id.is_empty() {
                    format!("Window {index}")
                } else {
                    win.app_id.clone()
                }
            } else {
                win.title.clone()
            };
            let shown_title: String = title.chars().take(40).collect();
            list = list.push(
                quiet(text(shown_title).size(13))
                    .on_press(Message::SnapAssistPick(index))
                    .width(Fill)
                    .padding([8, 10]),
            );
            shown += 1;
        }
        if shown == 0 {
            list = list.push(text("No other windows").size(13));
        }
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_snapflyout(&self) -> Element<'_, Message> {
        let tile = |label: &'static str, kind: &'static str| {
            quiet(
                column![
                    container(Space::with_width(Fill).height(36))
                        .width(Fill)
                        .height(40)
                        .style(|_| ContainerStyle {
                            background: Some(Background::Color(Color::from_rgba(
                                0.0, 0.47, 0.83, 0.35,
                            ))),
                            border: Border {
                                color: Color::from_rgb(0.0, 0.47, 0.83),
                                width: 1.0,
                                radius: mde_theme::CONTROL_RADIUS.into(),
                            },
                            ..Default::default()
                        }),
                    text(label).size(11),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center)
                .width(72),
            )
            .on_press(Message::SnapLayout(kind.into()))
            .padding([8, 6])
        };

        container(
            column![
                row![
                    text("Snap layouts").size(15),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                text("Super+Z · pick a layout for the focused window").size(11),
                // W444: ultrawide presets (thirds + vertical halves).
                text("Ultrawide").size(11),
                row![
                    tile("L 1/3", "left-1/3"),
                    tile("C 1/3", "center-1/3"),
                    tile("R 1/3", "right-1/3"),
                ]
                .spacing(8),
                row![
                    tile("Top", "top-half"),
                    tile("Bottom", "bottom-half"),
                ]
                .spacing(8),
                Space::with_height(6),
                row![
                    tile("Left", "left"),
                    tile("Right", "right"),
                    tile("Maximize", "maximize"),
                ]
                .spacing(8),
                // W282: quarter corners TL/TR/BL/BR → mde-snap.req.
                text("Corners").size(11),
                row![
                    tile("TL", "top-left"),
                    tile("TR", "top-right"),
                    tile("BL", "bottom-left"),
                    tile("BR", "bottom-right"),
                ]
                .spacing(8),
                row![
                    tile("Left 2/3", "left-2/3"),
                    tile("Right 2/3", "right-2/3"),
                ]
                .spacing(8),
                row![
                    tile("Left 1/3", "left-1/3"),
                    tile("Center 1/3", "center-1/3"),
                    tile("Right 1/3", "right-1/3"),
                ]
                .spacing(8),
                // W182: vertical half tiles (Super+Alt+Up/Down).
                row![
                    tile("Top half", "top-half"),
                    tile("Bottom half", "bottom-half"),
                ]
                .spacing(8),
                self.view_fancy_zones_row(),
                row![
                    quiet(text("Snap Groups…").size(12))
                        .on_press(Message::OpenSnapGroups)
                        .padding([6, 8]),
                    quiet(text("Save group").size(12))
                        .on_press(Message::SaveCurrentSnapGroup)
                        .padding([6, 8]),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .padding([14, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn view_fancy_zones_row(&self) -> Element<'_, Message> {
        if !self.cfg.active().tools.fancy_zones {
            return Space::with_height(0).into();
        }
        let tools = &self.cfg.active().tools;
        let layout = tools
            .zone_layouts
            .iter()
            .find(|l| l.name == tools.active_zone_layout)
            .or_else(|| tools.zone_layouts.first());
        let mut col = column![
            text(format!(
                "FancyZones · {}",
                layout.map(|l| l.name.as_str()).unwrap_or("none")
            ))
            .size(11),
        ]
        .spacing(6);
        if let Some(layout) = layout {
            let mut row_els = row![].spacing(8);
            for (i, _z) in layout.zones.iter().enumerate() {
                row_els = row_els.push(
                    quiet(text(format!("Z{}", i + 1)).size(11))
                        .on_press(Message::ApplyZone(i))
                        .padding([8, 10]),
                );
            }
            col = col.push(row_els);
        }
        col.into()
    }
}
