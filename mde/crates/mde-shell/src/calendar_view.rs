//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_month_calendar(&self) -> Element<'_, Message> {
        let header = row![
            quiet(text("‹").size(16)).on_press(Message::CalPrev),
            Space::with_width(Fill),
            text(&self.calendar.title).size(14),
            Space::with_width(Fill),
            quiet(text("›").size(16)).on_press(Message::CalNext),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        let mut weekdays = row![].spacing(2);
        // Spacer for W277 week-number column.
        weekdays = weekdays.push(container(Space::with_width(1)).width(28));
        for label in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"] {
            weekdays = weekdays.push(
                container(text(label).size(11))
                    .width(32)
                    .center_x(Fill),
            );
        }

        let mut grid = column![].spacing(2);
        for week in self.calendar.cells.chunks(7) {
            let mut row_el = row![].spacing(2);
            // W277: week number on each week row (first day with a number).
            let week_label = week
                .iter()
                .find_map(|c| c.and_then(|d| self.calendar.week_number(d)))
                .map(|w| format!("W{w}"))
                .unwrap_or_default();
            row_el = row_el.push(
                container(text(week_label).size(10))
                    .width(28)
                    .height(28)
                    .center_x(Fill)
                    .center_y(Fill),
            );
            for cell in week {
                let day_btn: Element<'_, Message> = match *cell {
                    None => container(Space::with_width(1)).width(32).height(28).into(),
                    Some(day) => {
                        let is_today = self.calendar.today_day == Some(day);
                        let is_sel = self.cal_selected == Some(day);
                        let style = if is_today || is_sel {
                            accent_pill
                        } else {
                            glass_button
                        };
                        quiet(text(format!("{day}")).size(12).center())
                            .on_press(Message::CalSelect(day))
                            .style(style)
                            .padding(0)
                            .width(32)
                            .height(28)
                            .into()
                    }
                };
                row_el = row_el.push(day_btn);
            }
            grid = grid.push(row_el);
        }

        let week_chip = self.calendar.current_week_label();
        // A031: month grid only — agenda strip intentionally removed.
        let status = match self.cal_selected {
            Some(d) => format!(
                "Selected: {} {} {} · {}",
                self.calendar
                    .title
                    .split_whitespace()
                    .next()
                    .unwrap_or("Day"),
                d,
                self.calendar.year,
                week_chip
            ),
            None => {
                if week_chip.is_empty() {
                    "Today".into()
                } else {
                    format!("Today · {week_chip}")
                }
            }
        };

        column![
            header,
            weekdays,
            grid,
            Space::with_height(4),
            hairline(&self.palette),
            text(status).size(11),
        ]
        .spacing(4)
        .into()
    }
}
