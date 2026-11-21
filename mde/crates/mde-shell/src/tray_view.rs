//! Tray overflow moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_tray(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("System tray").size(16),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8),
        ]
        .spacing(6);
        let hidden_ids = self.tray_hidden_ids();
        // W442: honest empty tray — no fake mde.stub SNI.
        if self.tray.items.is_empty() {
            list = list.push(
                text("No tray icons")
                    .size(13)
                    .color(Color::from_rgba(0.55, 0.55, 0.55, 1.0)),
            );
            list = list.push(
                text("Apps that use StatusNotifier will appear here.")
                    .size(11)
                    .color(Color::from_rgba(0.45, 0.45, 0.45, 1.0)),
            );
        }
        for item in &self.tray.items {
            let title = if item.title.is_empty() {
                item.icon.clone()
            } else {
                item.title.clone()
            };
            let is_hidden = hidden_ids.iter().any(|h| h == &item.id);
            let head: Element<'_, Message> = if item.icon_path.is_empty() {
                text(if is_hidden {
                    format!("{title} · hidden")
                } else {
                    title
                })
                .size(13)
                .into()
            } else {
                row![
                    image(&item.icon_path).width(18).height(18),
                    text(if is_hidden {
                        format!("{title} · hidden")
                    } else {
                        title
                    })
                    .size(13)
                ]
                .spacing(8)
                .into()
            };
            list = list.push(flyout_inset(head, &self.palette));
            if !item.bus.is_empty() || item.id.starts_with("sni:") {
                let open_id = item.id.clone();
                let menu_id = item.id.clone();
                let right_id = item.id.clone();
                let hide_id = item.id.clone();
                let hide_label = if is_hidden { "Show" } else { "Hide" };
                // W386/W398: overflow rows support Menu + Hide + right-click ContextMenu.
                list = list.push(
                    mouse_area(
                        row![
                            quiet(text("Open").size(12)).on_press(Message::ActivateTray(open_id)),
                            quiet(text("Menu").size(12)).on_press(Message::TrayMenu(menu_id)),
                            quiet(text(hide_label).size(12))
                                .on_press(Message::ToggleTrayHidden(hide_id)),
                        ]
                        .spacing(8),
                    )
                    .on_right_press(Message::TrayMenu(right_id)),
                );
                if let Some(menu) = self.tray_menu.as_ref().filter(|menu| menu.item_id == item.id)
                {
                    if !menu.note.is_empty() {
                        list = list.push(text(menu.note.clone()).size(12));
                    }
                    for entry in &menu.entries {
                        let indent = "  ".repeat(usize::from(entry.depth.min(4)));
                        let label = format!("{indent}{}", entry.label);
                        if entry.enabled {
                            list = list.push(
                                quiet(text(label).size(13)).on_press(Message::TrayMenuClick(
                                    item.id.clone(),
                                    entry.id,
                                )),
                            );
                        } else {
                            list = list.push(text(label).size(13));
                        }
                    }
                }
            }
        }
        container(scrollable(list).height(Fill))
            .padding(14)
            .width(Fill)
            .height(Fill)
            .into()
    }
}
