//! Context / jump / Win+X / desktop / system menus (W381–W390).
use super::*;
use iced::widget::column;

impl Shell {
    fn menu_item<'a>(label: String, message: Message) -> Element<'a, Message> {
        quiet(text(label).size(13))
            .on_press(message)
            .width(Fill)
            .padding([8, 10])
            .into()
    }

    fn menu_header(&self, title: String) -> Element<'_, Message> {
        let _ = self;
        row![
            text(title).size(14),
            Space::with_width(Fill),
            quiet(text("Close").size(12)).on_press(Message::Dismiss),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }

    /// W381: pinned-app jump list — Open, Unpin, End task / Close all, Recent.
    pub(super) fn view_jump_list(&self) -> Element<'_, Message> {
        let id = self.jump_list_pin.clone().unwrap_or_default();
        let label = apps::resolve_desktop(&id)
            .map(|p| apps::desktop_name(&p))
            .unwrap_or_else(|| id.clone());
        let indices = self.window_indices_for_pin(&id);

        let mut list = column![self.menu_header(label)]
            .spacing(4)
            .push(Self::menu_item(
                "Open".to_string(),
                Message::LaunchPin(id.clone()),
            ))
            .push(Self::menu_item(
                "Unpin from taskbar".to_string(),
                Message::UnpinTaskbar(id.clone()),
            ));

        // A082: End task always when For developers toggle is on (default).
        let end_always = self.cfg.active().shell.end_task_always;
        if indices.len() > 1 {
            list = list.push(Self::menu_item(
                "Close all windows".to_string(),
                Message::EndTasks(indices.clone()),
            ));
        }
        if end_always {
            if let Some(&idx) = indices.first() {
                list = list.push(Self::menu_item(
                    "End task".to_string(),
                    Message::EndTask(idx),
                ));
            }
        } else if let Some(&idx) = indices.first() {
            list = list.push(Self::menu_item(
                "Close window".to_string(),
                Message::EndTask(idx),
            ));
        }

        list = list.push(Space::with_height(4));
        // A030: forever-pinned first, then deeper recent.
        let max = self.cfg.active().tools.jump_list_max.max(3) as usize;
        let forever = tools::jump_pinned_files(&id);
        if !forever.is_empty() {
            list = list.push(text("Pinned").size(11));
            for path in forever.iter().take(max) {
                let open = path.clone();
                let unpin = path.clone();
                let name: String = std::path::Path::new(path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone())
                    .chars()
                    .take(36)
                    .collect();
                list = list.push(
                    row![
                        quiet(text(name).size(13))
                            .on_press(Message::JumpListOpenFile(open))
                            .width(Fill)
                            .padding([6, 6]),
                        quiet(text("Unpin").size(11))
                            .on_press(Message::JumpListUnpinRecent(unpin))
                            .padding([4, 6]),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                );
            }
        }
        let recent_files = recent::files_for_app(&id, max);
        list = list.push(text("Recent").size(11));
        if recent_files.is_empty() {
            list = list.push(
                text("No recent items")
                    .size(12)
                    .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
            );
        } else {
            for item in recent_files {
                let path = item.id.clone();
                let pin_path = item.id.clone();
                let unpin_path = item.id.clone();
                let name: String = item.label.chars().take(36).collect();
                list = list.push(
                    row![
                        quiet(text(name).size(13))
                            .on_press(Message::JumpListOpenFile(path))
                            .width(Fill)
                            .padding([6, 6]),
                        quiet(text("Pin").size(11))
                            .on_press(Message::JumpListPinForever(pin_path))
                            .padding([4, 6]),
                        quiet(text("×").size(11))
                            .on_press(Message::JumpListUnpinRecent(unpin_path))
                            .padding([4, 6]),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                );
            }
        }
        list = list.push(
            quiet(text("Clear recent").size(11))
                .on_press(Message::JumpListClearRecent)
                .padding([6, 6]),
        );

        container(list.padding([12, 14]))
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// W382: unpinned running — same shape as pin jump list.
    pub(super) fn view_win_jump(&self) -> Element<'_, Message> {
        let index = self.jump_list_win.unwrap_or(0);
        let win = self.windows.get(index);
        let app_id = win.map(|w| w.app_id.clone()).unwrap_or_default();
        let title = win
            .map(|w| {
                let t: String = w.title.chars().take(36).collect();
                if t.is_empty() {
                    w.app_id.clone()
                } else {
                    t
                }
            })
            .unwrap_or_else(|| "Window".into());

        let indices: Vec<usize> = if app_id.is_empty() {
            vec![index]
        } else {
            self.windows
                .iter()
                .enumerate()
                .filter(|(_, w)| {
                    w.workspace == self.desktop_active
                        && w.app_id.eq_ignore_ascii_case(&app_id)
                })
                .map(|(i, _)| i)
                .collect()
        };

        let pin_id = if app_id.is_empty() {
            None
        } else if app_id.ends_with(".desktop") {
            Some(app_id.clone())
        } else {
            Some(format!("{app_id}.desktop"))
        };
        let already_pinned = pin_id
            .as_ref()
            .is_some_and(|p| self.cfg.active().shell.pins.iter().any(|x| x == p || x == &app_id));
        let recent_files = pin_id
            .as_ref()
            .map(|p| recent::files_for_app(p, 3))
            .unwrap_or_default();

        let mut list = column![self.menu_header(title)].spacing(4);

        if let Some(pid) = pin_id.clone() {
            list = list.push(Self::menu_item(
                "Open".to_string(),
                Message::LaunchPin(pid.clone()),
            ));
            if already_pinned {
                list = list.push(Self::menu_item(
                    "Unpin from taskbar".to_string(),
                    Message::UnpinTaskbar(pid),
                ));
            } else {
                list = list.push(Self::menu_item(
                    "Pin to taskbar".to_string(),
                    Message::PinToTaskbar(pid),
                ));
            }
        }

        // A082: End task (developer) vs Close window.
        let end_always = self.cfg.active().shell.end_task_always;
        if indices.len() > 1 {
            list = list.push(Self::menu_item(
                "Close all windows".to_string(),
                Message::EndTasks(indices),
            ));
        }
        list = list.push(Self::menu_item(
            if end_always {
                "End task".to_string()
            } else {
                "Close window".to_string()
            },
            Message::EndTask(index),
        ));

        list = list.push(Space::with_height(4));
        list = list.push(text("Recent").size(11));
        if recent_files.is_empty() {
            list = list.push(
                text("No recent items")
                    .size(12)
                    .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
            );
        } else {
            for item in recent_files {
                let path = item.id.clone();
                let unpin_path = item.id.clone();
                let name: String = item.label.chars().take(36).collect();
                list = list.push(
                    row![
                        quiet(text(name).size(13))
                            .on_press(Message::JumpListOpenFile(path))
                            .width(Fill)
                            .padding([6, 6]),
                        quiet(text("Unpin").size(11))
                            .on_press(Message::JumpListUnpinRecent(unpin_path))
                            .padding([4, 6]),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                );
            }
        }

        container(list.padding([12, 14]))
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// W383–W385: Start / All apps / Recommended context menu.
    pub(super) fn view_app_ctx(&self) -> Element<'_, Message> {
        let (kind, id) = match &self.app_ctx {
            Some(ctx) => (ctx.kind, ctx.id.clone()),
            None => (AppCtxKind::AllApps, String::new()),
        };
        let label = apps::resolve_desktop(&id)
            .map(|p| apps::desktop_name(&p))
            .unwrap_or_else(|| id.clone());
        let on_start = self.cfg.active().start.pins.iter().any(|p| p == &id);
        let on_taskbar = self.cfg.active().shell.pins.iter().any(|p| p == &id);

        let mut list = column![self.menu_header(label)].spacing(4);
        list = list.push(Self::menu_item(
            "Open".to_string(),
            Message::LaunchPin(id.clone()),
        ));
        match kind {
            AppCtxKind::StartPin => {
                list = list.push(Self::menu_item(
                    "Unpin from Start".to_string(),
                    Message::UnpinStart(id.clone()),
                ));
                list = list.push(Self::menu_item(
                    "Move to Advanced".to_string(),
                    Message::MoveToAdvanced(id.clone()),
                ));
                if id.starts_with("mde-folder:") {
                    list = list.push(Self::menu_item(
                        "Dissolve folder".to_string(),
                        Message::DissolveStartFolder(id.clone()),
                    ));
                } else {
                    list = list.push(Self::menu_item(
                        "Create folder".to_string(),
                        Message::CreateStartFolder(id.clone()),
                    ));
                }
            }
            AppCtxKind::AllApps | AppCtxKind::Recommended => {
                if !on_start {
                    list = list.push(Self::menu_item(
                        "Pin to Start".to_string(),
                        Message::PinToStart(id.clone()),
                    ));
                } else {
                    list = list.push(Self::menu_item(
                        "Unpin from Start".to_string(),
                        Message::UnpinStart(id.clone()),
                    ));
                }
                if !self.cfg.active().start.is_advanced_app(&id) {
                    list = list.push(Self::menu_item(
                        "Move to Advanced".to_string(),
                        Message::MoveToAdvanced(id.clone()),
                    ));
                }
            }
        }
        if on_taskbar {
            list = list.push(Self::menu_item(
                "Unpin from taskbar".to_string(),
                Message::UnpinTaskbar(id.clone()),
            ));
        } else {
            list = list.push(Self::menu_item(
                "Pin to taskbar".to_string(),
                Message::PinToTaskbar(id.clone()),
            ));
        }
        list = list.push(Self::menu_item(
            "Open file location".to_string(),
            Message::OpenRecommendedLocation(id),
        ));
        if matches!(kind, AppCtxKind::Recommended | AppCtxKind::AllApps | AppCtxKind::StartPin) {
            list = list.push(Self::menu_item(
                "Uninstall".to_string(),
                Message::UninstallPrompt,
            ));
        }

        container(list.padding([12, 14]))
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_winmenu(&self) -> Element<'_, Message> {
        container(
            column![
                row![
                    text("Window").size(15),
                    Space::with_width(Fill),
                    quiet(text("Close menu").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8),
                text("Alt+Space · focused window").size(11),
                Space::with_height(4),
                quiet(text("Minimize").size(13))
                    .on_press(Message::WindowAction("minimize".into()))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Maximize").size(13))
                    .on_press(Message::WindowAction("maximize".into()))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Center").size(13))
                    .on_press(Message::CenterWindow)
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Close").size(13))
                    .on_press(Message::WindowAction("close".into()))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Undo snap").size(13))
                    .on_press(Message::UndoSnap)
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Always on top").size(13))
                    .on_press(Message::WindowAction("always-on-top".into()))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Opacity").size(13))
                    .on_press(Message::WindowAction("opacity".into()))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Cascade windows").size(13))
                    .on_press(Message::WindowAction("cascade".into()))
                    .width(Fill)
                    .padding([8, 10]),
                text("Nudge 20px").size(11),
                row![
                    quiet(text("←").size(13))
                        .on_press(Message::NudgeWindow("left"))
                        .padding([6, 10]),
                    quiet(text("→").size(13))
                        .on_press(Message::NudgeWindow("right"))
                        .padding([6, 10]),
                    quiet(text("↑").size(13))
                        .on_press(Message::NudgeWindow("up"))
                        .padding([6, 10]),
                    quiet(text("↓").size(13))
                        .on_press(Message::NudgeWindow("down"))
                        .padding([6, 10]),
                ]
                .spacing(4),
                text("Super+Alt+←/→ · ↑/↓ via menu").size(11),
                text("Move to monitor · Super+Shift+←/→").size(11),
                Space::with_height(4),
                text("Move to desktop").size(11),
                quiet(text("Desktop 1").size(13))
                    .on_press(Message::WinMenuMoveDesktop(0))
                    .width(Fill)
                    .padding([8, 10]),
                quiet(text("Desktop 2").size(13))
                    .on_press(Message::WinMenuMoveDesktop(1))
                    .width(Fill)
                    .padding([8, 10]),
            ]
            .spacing(4)
            .padding([12, 14]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// W389: native title-bar system menu (separate from Alt+Space winmenu).
    pub(super) fn view_sysmenu(&self) -> Element<'_, Message> {
        container(
            column![
                self.menu_header("System menu".into()),
                text("Title bar").size(11),
                Space::with_height(4),
                Self::menu_item("Restore".into(), Message::WindowAction("restore".into())),
                Self::menu_item("Move".into(), Message::WindowAction("move".into())),
                Self::menu_item("Size".into(), Message::WindowAction("size".into())),
                Self::menu_item(
                    "Minimize".into(),
                    Message::WindowAction("minimize".into()),
                ),
                Self::menu_item(
                    "Maximize".into(),
                    Message::WindowAction("maximize".into()),
                ),
                Self::menu_item("Close".into(), Message::WindowAction("close".into())),
            ]
            .spacing(4)
            .padding([12, 14]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// W388: empty-desktop context menu.
    pub(super) fn view_desktop_menu(&self) -> Element<'_, Message> {
        container(
            column![
                self.menu_header("Desktop".into()),
                text("View").size(11),
                Self::menu_item(
                    "Large icons".into(),
                    Message::DesktopIconSize("large"),
                ),
                Self::menu_item(
                    "Medium icons".into(),
                    Message::DesktopIconSize("medium"),
                ),
                Self::menu_item(
                    "Small icons".into(),
                    Message::DesktopIconSize("small"),
                ),
                text("Sort by").size(11),
                Self::menu_item("Name".into(), Message::DesktopSortBy("name")),
                Self::menu_item(
                    "Date modified".into(),
                    Message::DesktopSortBy("date"),
                ),
                Self::menu_item("Refresh".into(), Message::DesktopMenuRefresh),
                text("New").size(11),
                Self::menu_item("Folder".into(), Message::DesktopNewFolder),
                Self::menu_item(
                    "Shortcut".into(),
                    Message::DesktopNewShortcut,
                ),
                Self::menu_item(
                    "Personalize".into(),
                    Message::OpenControl("personalization".into()),
                ),
                Self::menu_item(
                    "Display settings".into(),
                    Message::OpenControl("display".into()),
                ),
            ]
            .spacing(4)
            .padding([12, 14]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_run(&self) -> Element<'_, Message> {
        let palette_on = self.cfg.active().tools.command_palette;
        let mut body = column![
            row![
                text(if palette_on {
                    "Command Palette"
                } else {
                    "Run"
                })
                .size(15),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(if palette_on {
                "Apps · files · calc · shell · Super+Alt+Space"
            } else {
                "Type a command, .desktop id, or path"
            })
            .size(11),
            text_input("Open:", &self.run_query)
                .on_input(Message::RunQuery)
                .on_submit(Message::RunSubmit)
                .padding(10)
                .size(14),
        ]
        .spacing(8);
        // A015: plugin results
        if palette_on && !self.palette_hits.is_empty() {
            body = body.push(hairline(&self.palette));
            for hit in self.palette_hits.iter().take(8) {
                let action = hit.action.clone();
                body = body.push(
                    quiet(
                        text(format!("[{}] {}", hit.kind, hit.label))
                            .size(12),
                    )
                    .on_press(Message::PaletteSelect(action))
                    .width(Fill)
                    .padding([6, 4]),
                );
            }
        }
        body = body.push(row![
            Space::with_width(Fill),
            quiet(text("OK").size(12))
                .on_press(Message::RunSubmit)
                .padding([6, 14]),
        ]);
        container(body.padding([14, 16]))
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// W390: Win+X Quick Link expansion (also Start right-click).
    pub(super) fn view_quicklink(&self) -> Element<'_, Message> {
        let row_btn = |label: &'static str, kind: &'static str| {
            quiet(text(label).size(13))
                .on_press(Message::QuickLink(kind))
                .width(Fill)
                .padding([8, 12])
        };
        // No scrollable — flyout_stage sizes QuickLink to fit this list.
        let mut power = column![
            text("Power").size(11),
            row_btn("Sign out", "signout"),
        ]
        .spacing(2);
        if crate::offline_update_pending() {
            power = power
                .push(row_btn("Update and restart", "update-restart"))
                .push(row_btn("Update and shut down", "update-shutdown"));
        }
        power = power
            .push(row_btn("Sleep", "sleep"))
            .push(row_btn("Shut down", "shutdown"))
            .push(row_btn("Restart", "restart"));
        container(
            column![
                row![
                    text("Quick link").size(15),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                text("Super+X").size(11),
                Space::with_height(4),
                row_btn("Terminal", "terminal"),
                row_btn("Settings", "settings"),
                row_btn("Explorer", "explorer"),
                row_btn("Run", "run"),
                row_btn("Desktop folder", "desktop"),
                row_btn("Performance", "taskmgr"),
                power,
                text("Tools").size(11),
                row_btn("Computer Management", "compmgmt"),
                row_btn("Device Manager", "devmgmt"),
                row_btn("Event Viewer", "eventvwr"),
                row_btn("Disks", "disks"),
                row_btn("Services", "services"),
                row_btn("Network connections", "netcon"),
            ]
            .spacing(2)
            .padding([14, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }
}
