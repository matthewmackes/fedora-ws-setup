//! Surface views moved from main.rs. Win11-parity taskbar layout and behaviour.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_bar(&self) -> Element<'_, Message> {
        // Auto-hide: collapsed hit strip until pointer reaches the bottom edge.
        if self.bar_visually_hidden() {
            return mouse_area(
                container(Space::with_height(Self::BAR_HIT as u16))
                    .width(Fill)
                    .height(Self::BAR_HIT as u16)
                    .style(|_| ContainerStyle {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.01))),
                        ..Default::default()
                    }),
            )
            .on_enter(Message::BarHoverEnter)
            .into();
        }

        // Win11 order: Widgets · Start · Task View · combined apps · tray · clock · show desktop.
        let accent_chrome = self.palette.accent_on_start_taskbar;
        let start = mouse_area(
            chrome_btn("view-grid", "Start", 22, self.palette.dark)
                .on_press(Message::ToggleStart)
                .style(if self.start_open && accent_chrome {
                    accent_pill
                } else {
                    glass_button
                }),
        )
        .on_right_press(Message::ToggleQuickLink);

        let task_view = chrome_btn("focus-windows", "Task View", 18, self.palette.dark)
            .on_press(Message::ToggleTaskView)
            .style(if self.taskview_open && accent_chrome {
                accent_pill
            } else {
                glass_button
            });

        let show_widgets = self.cfg.active().shell.widgets_btn;
        let search_mode = self.cfg.active().shell.taskbar_search.to_lowercase();
        let search_btn: Option<Element<'_, Message>> = match search_mode.as_str() {
            "icon" => Some(
                chrome_btn("system-search", "Search", 18, self.palette.dark)
                    .on_press(Message::OpenStartSearch)
                    .style(if self.search_open && accent_chrome {
                        accent_pill
                    } else {
                        glass_button
                    })
                    .into(),
            ),
            "box" | "search" | "bar" => {
                let face = row![
                    app_face("system-search", "Search".into(), 16, false, self.palette.dark),
                    text("Search").size(13),
                ]
                .spacing(8)
                .padding([4, 12])
                .align_y(iced::Alignment::Center);
                Some(
                    quiet(face)
                        .on_press(Message::OpenStartSearch)
                        .style(if self.search_open && accent_chrome {
                            accent_pill
                        } else {
                            glass_button
                        })
                        .into(),
                )
            }
            _ => None, // hide (default)
        };

        // Combined app buttons: pinned first, then unpinned running apps (by app_id).
        let entries = self.taskbar_app_entries();
        let show_labels = self.taskbar_labels();
        let max_icons = if show_labels { 8usize } else { 12usize };
        let mut apps_row = row![].spacing(2);
        for entry in entries.iter().take(max_icons) {
            let icon = if !entry.icon.is_empty() {
                entry.icon.as_str()
            } else {
                "application-x-executable"
            };
            let tip = if entry.indices.len() > 1 {
                format!("{} · {} windows", entry.label, entry.indices.len())
            } else {
                entry.label.clone()
            };
            let face = bar_icon_btn(
                icon,
                entry.label.clone(),
                entry.running,
                entry.progress,
                entry.unread,
                entry.indices.len(),
                show_labels,
                Message::ActivateTaskbarApp {
                    pin: entry.pin.clone(),
                    indices: entry.indices.clone(),
                },
                self.palette.dark);
            if let Some(pin) = entry.pin.clone() {
                // Pins: drag-reorder on the bar (press → drag across pins → release).
                let mid = entry
                    .indices
                    .first()
                    .copied()
                    .map(Message::CloseWindow)
                    .unwrap_or_else(|| Message::HoverStatus(tip.clone()));
                apps_row = apps_row.push(
                    mouse_area(face)
                        .on_enter(Message::PinBarEnter(pin.clone()))
                        .on_exit(Message::PinBarExit)
                        .on_right_press(Message::OpenJumpList(pin))
                        .on_middle_press(mid)
                        .interaction(iced::mouse::Interaction::Pointer),
                );
            } else {
                let mid = entry
                    .indices
                    .first()
                    .copied()
                    .map(Message::CloseWindow)
                    .unwrap_or_else(|| Message::HoverStatus(tip.clone()));
                let right = entry
                    .indices
                    .first()
                    .copied()
                    .map(Message::OpenWinJump)
                    .unwrap_or_else(|| Message::HoverStatus(tip.clone()));
                apps_row = apps_row.push(
                    mouse_area(face)
                        .on_enter(Message::HoverStatus(tip))
                        .on_middle_press(mid)
                        .on_right_press(right),
                );
            }
        }
        if entries.len() > max_icons {
            apps_row = apps_row.push(
                quiet(text(format!("+{}", entries.len() - max_icons)).size(11))
                    .on_press(Message::TogglePinOverflow)
                    .style(if self.pin_overflow_open {
                        accent_pill
                    } else {
                        glass_button
                    })
                    .padding([6, 8]),
            );
        }

        let tray_hidden = self.tray_hidden_ids();
        let sni_all: Vec<&tray::TrayItem> = self
            .tray
            .items
            .iter()
            .filter(|item| item.id.starts_with("sni:"))
            .filter(|item| !tray_hidden.iter().any(|h| h == &item.id))
            .collect();
        let mut sni_labels = row![].spacing(2);
        for item in sni_all.iter().take(4) {
            let face: Element<'_, Message> = if item.icon_path.is_empty() {
                text(item.title.chars().next().unwrap_or('•').to_string())
                    .size(12)
                    .into()
            } else {
                image(&item.icon_path).width(16).height(16).into()
            };
            let open_id = item.id.clone();
            let menu_id = item.id.clone();
            let mid_id = item.id.clone();
            sni_labels = sni_labels.push(
                mouse_area(quiet(face).on_press(Message::ActivateTray(open_id)))
                    .on_right_press(Message::TrayMenu(menu_id))
                    .on_middle_press(Message::TraySecondary(mid_id)),
            );
        }
        if sni_all.len() > 4 {
            sni_labels = sni_labels.push(
                quiet(text("›").size(12))
                    .on_press(Message::TrayOverflowStub)
                    .padding([4, 6]),
            );
        }

        let date_label = clock_date_cfg(self.cfg.active().shell.clock_date_iso);
        let show_secs = self.cfg.active().shell.clock_seconds;
        let clock = mouse_area(
            quiet(
                column![
                    text(clock_time_cfg(show_secs)).size(12),
                    text(date_label).size(10),
                ]
                .align_x(iced::Alignment::End)
                .spacing(0),
            )
            .on_press(Message::ToggleNotifications)
            .padding([4, 10]),
        )
        .on_right_press(Message::OpenTaskbarMenu);

        let overflow = chrome_btn("pan-up", "^", 14, self.palette.dark)
            .on_press(Message::ToggleTray)
            .style(if self.tray_open {
                accent_pill
            } else {
                glass_button
            });

        // Win11 status cluster: net / volume / battery → Quick Settings.
        let net_icon = if self.airplane {
            "airplane-mode"
        } else if self.wifi_on {
            "network-wireless"
        } else {
            "network-offline"
        };
        let vol_icon = if self.vol_muted {
            "audio-volume-muted"
        } else if self.vol_level < 0.05 {
            "audio-volume-muted"
        } else if self.vol_level < 0.34 {
            "audio-volume-low"
        } else if self.vol_level < 0.67 {
            "audio-volume-medium"
        } else {
            "audio-volume-high"
        };
        let mute_flash = self
            .vol_mute_flash_until
            .is_some_and(|u| Instant::now() < u);
        let net_btn = chrome_btn(net_icon, "Net", 16, self.palette.dark)
            .on_press(Message::ToggleQuick)
            .style(if self.quick_open {
                accent_pill
            } else {
                glass_button
            });
        let vol_btn = chrome_btn(vol_icon, "Vol", 16, self.palette.dark)
            .on_press(Message::ToggleQuick)
            .style(if mute_flash {
                mute_flash_style
            } else if self.vol_muted {
                mute_persistent_style
            } else if self.quick_open {
                accent_pill
            } else {
                glass_button
            });
        let mut status_glyphs = row![].spacing(2).push(net_btn).push(vol_btn);
        if self.cfg.active().accessibility.privacy_indicators && !self.privacy_uses.is_empty() {
            let n = self.privacy_uses.len();
            status_glyphs = status_glyphs.push(
                chrome_btn("camera-web", &format!("Priv{n}"), 16, self.palette.dark)
                    .on_press(Message::OpenControl("privacy".into()))
                    .style(glass_button),
            );
        }
        if let Some(pct) = self.battery_pct {
            let bat_icon = battery_icon_name(pct, self.battery_charging);
            status_glyphs = status_glyphs.push(
                chrome_btn(bat_icon, &format!("{pct}%"), 16, self.palette.dark)
                    .on_press(Message::ToggleQuick)
                    .style(if self.quick_open {
                        accent_pill
                    } else {
                        glass_button
                    }),
            );
        }

        // Win11: hidden icons · net/vol/bat · clock (notif/calendar) · show desktop.
        let right = row![
            sni_labels,
            overflow,
            status_glyphs,
            clock,
            show_desktop_strip(),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center);

        let left_align = self.taskbar_left_align();
        let mut center = row![start].spacing(2).align_y(iced::Alignment::Center);
        if let Some(search) = search_btn {
            center = center.push(search);
        }
        let center = center
            .push(task_view)
            .push(apps_row)
            .spacing(2)
            .align_y(iced::Alignment::Center);

        // Win11 taskbar has no status/debug chip between apps and tray.
        let bar_row = {
            let mut r = if !left_align {
                row![].spacing(6).padding([2, 6]).align_y(iced::Alignment::Center)
            } else {
                row![].spacing(4).padding([2, 6]).align_y(iced::Alignment::Center)
            };
            if show_widgets {
                let face: Element<'_, Message> = match widgets::read_weather_cache_temp() {
                    Some(temp) => {
                        let label: String = temp.chars().take(8).collect();
                        row![
                            app_face("weather-few-clouds", "Widgets".into(), 18, false, self.palette.dark),
                            text(label).size(12),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center)
                        .into()
                    }
                    None => app_face("weather-few-clouds", "Widgets".into(), 18, false, self.palette.dark),
                };
                r = r.push(
                    quiet(face)
                        .on_press(Message::ToggleWidgets)
                        .style(if self.widgets_open {
                            accent_pill
                        } else {
                            glass_button
                        })
                        .padding([4, 10]),
                );
            }
            if !left_align {
                r = r.push(Space::with_width(Fill));
            }
            r.push(center)
                .push(Space::with_width(Fill))
                .push(right)
        };

        // A007: more opaque when windows are present on the active desktop.
        let over_content = self
            .windows
            .iter()
            .any(|w| w.workspace == self.desktop_active);
        let fill = mde_theme::chrome_over(&self.palette, over_content);
        let edge = mde_theme::flyout_edge(&self.palette);
        let critical_led = self
            .critical_led_until
            .is_some_and(|u| Instant::now() < u);
        let accent_rgb = Palette::parse_rgb(&self.palette.accent)
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.0, 0.47, 0.83));
        let hairline = container(Space::with_height(1))
            .width(Fill)
            .height(1)
            .style(move |_| ContainerStyle {
                background: Some(Background::Color(if critical_led {
                    accent_rgb
                } else {
                    edge
                })),
                ..Default::default()
            });
        let body = container(bar_row)
            .width(Fill)
            .height((self.bar_height() as u16).saturating_sub(1))
            .style(move |_| {
                if critical_led {
                    ContainerStyle {
                        background: Some(Background::Color(Color {
                            a: 0.55,
                            ..accent_rgb
                        })),
                        ..Default::default()
                    }
                } else {
                    ContainerStyle {
                        background: Some(Background::Color(fill)),
                        ..Default::default()
                    }
                }
            });
        let bar = container(column![hairline, body])
            .width(Fill)
            .height(self.bar_height() as u16);
        mouse_area(bar)
            .on_enter(Message::BarHoverEnter)
            .on_exit(Message::BarHoverExit)
            .on_right_press(Message::OpenTaskbarMenu)
            .into()
    }

    pub(super) fn view_taskbar_menu(&self) -> Element<'_, Message> {
        // W387: Win11-shaped taskbar menu — clock seconds/ISO live in Settings (W391).
        let item = |label: String, message: Message| {
            quiet(text(label).size(13))
                .on_press(message)
                .width(Fill)
                .padding([8, 10])
        };
        container(
            column![
                text("Taskbar").size(15),
                item(
                    "Taskbar settings".into(),
                    Message::OpenControl("taskbar".into()),
                ),
                item("Task manager".into(), Message::OpenTaskManager),
                item(
                    if self.cfg.active().shell.taskbar_autohide {
                        "Disable auto-hide".into()
                    } else {
                        "Auto-hide taskbar".into()
                    },
                    Message::ToggleAutoHide,
                ),
                item(
                    if self.taskbar_locked() {
                        "Unlock the taskbar".into()
                    } else {
                        "Lock the taskbar".into()
                    },
                    Message::ToggleTaskbarLock,
                ),
                item(
                    if self.cfg.active().shell.widgets_btn {
                        "Hide Widgets button".into()
                    } else {
                        "Show Widgets button".into()
                    },
                    Message::ToggleWidgetsBtnFromMenu,
                ),
                item(
                    "Date & time settings".into(),
                    Message::OpenControl("time".into()),
                ),
                item("Show desktop".into(), Message::ShowDesktop),
                item("Task View".into(), Message::ToggleTaskView),
                item(
                    "Search on taskbar".into(),
                    Message::OpenControl("taskbar".into()),
                ),
                item(
                    "Notification settings".into(),
                    Message::OpenControl("notifications".into()),
                ),
                item(
                    "Personalization".into(),
                    Message::OpenControl("personalization".into()),
                ),
                item("Shortcut Guide".into(), Message::OpenShortcutGuide),
            ]
            .spacing(4)
            .padding([14, 16]),
        )
        .into()
    }

    pub(super) fn view_pin_overflow(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("More apps").size(16),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text("Pins and running apps beyond the taskbar · drag pins on the bar to reorder")
                .size(11),
            Space::with_height(6),
        ]
        .spacing(4);
        for entry in self.taskbar_app_entries().into_iter().skip(12) {
            let icon = if entry.icon.is_empty() {
                "application-x-executable".into()
            } else {
                entry.icon
            };
            let face = quiet(app_face(&icon, entry.label, 22, true, self.palette.dark))
                .on_press(Message::ActivateTaskbarApp {
                    pin: entry.pin.clone(),
                    indices: entry.indices.clone(),
                })
                .width(Fill)
                .padding([8, 10]);
            // W386: right-click Unpin (or jump list) in overflow.
            let row: Element<'_, Message> = if let Some(pin) = entry.pin.clone() {
                mouse_area(face)
                    .on_right_press(Message::OpenJumpList(pin))
                    .into()
            } else if let Some(&idx) = entry.indices.first() {
                mouse_area(face)
                    .on_right_press(Message::OpenWinJump(idx))
                    .into()
            } else {
                face.into()
            };
            list = list.push(row);
        }
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_win_overflow(&self) -> Element<'_, Message> {
        // Combined mode: reuse pin overflow for overflow apps.
        self.view_pin_overflow()
    }
}
