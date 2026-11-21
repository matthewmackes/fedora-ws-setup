//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_notifications(&self) -> Element<'_, Message> {
        let clear_row: Element<'_, Message> = if self.notif_clear_confirm {
            row![
                text("Clear all notifications?").size(12),
                Space::with_width(Fill),
                quiet(text("Yes").size(12))
                    .on_press(Message::ConfirmClearNotifs)
                    .padding([4, 10]),
                quiet(text("No").size(12))
                    .on_press(Message::CancelClearNotifs)
                    .padding([4, 10]),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            row![
                text("Notifications").size(15),
                Space::with_width(Fill),
                quiet(text("Demo").size(12))
                    .on_press(Message::DemoNotify)
                    .padding([4, 8]),
                quiet(text("Clear all").size(12)).on_press(Message::ClearNotifs),
            ]
            .spacing(8)
            .into()
        };

        // Chrome stays pinned so Focus / Priority / Sounds / banner / quiet hours
        // never require scrolling (Win11 NC: only the notification list scrolls).
        // A098: up to 2 world clocks under the local clock.
        let world_row: Element<'_, Message> = {
            let zones = &self.cfg.active().shell.world_clocks;
            if zones.is_empty() {
                Space::with_height(0).into()
            } else {
                let mut col = column![].spacing(2);
                for tz in zones.iter().take(2) {
                    let label = world_clock_label(tz);
                    col = col.push(text(label).size(12));
                }
                col.into()
            }
        };

        let chrome = column![
            text(clock_long_date()).size(18),
            text(clock_time()).size(13),
            world_row,
            Space::with_height(4),
            hairline(&self.palette),
            clear_row,
            row![
                // Label matches ToggleDnd (not Focus sessions).
                qs_tile(
                    "notification-disabled",
                    "DND",
                    self.notifs.dnd,
                    Message::ToggleDnd,
                    QS_TILE_W,
                    self.palette.dark),
                qs_tile(
                    "emblem-important",
                    "Priority",
                    self.notifs.priority,
                    Message::TogglePriority,
                    QS_TILE_W,
                    self.palette.dark),
                qs_tile(
                    "audio-volume-high",
                    "Sounds",
                    self.notif_sounds_enabled(),
                    Message::ToggleNotifSounds,
                    QS_TILE_W,
                    self.palette.dark),
            ]
            .spacing(QS_GAP)
            .align_y(iced::Alignment::Center),
            // W205: banner duration ± (same extras as QS).
            {
                let secs = self.toast_secs();
                row![
                    text(format!("Banner · {secs}s")).size(11),
                    quiet(text("−").size(14)).on_press(Message::ToastSecsDelta(-1)),
                    quiet(text("+").size(14)).on_press(Message::ToastSecsDelta(1)),
                    quiet(text("Settings").size(11))
                        .on_press(Message::OpenControl("notifications".into()))
                        .padding([2, 6]),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
            },
            // W276: quiet hours quick presets.
            {
                let qf = self.cfg.active().notifications.quiet_from.clone();
                let qt = self.cfg.active().notifications.quiet_to.clone();
                let label = if qf.trim().is_empty() || qt.trim().is_empty() {
                    "Quiet hours · Off".into()
                } else {
                    format!("Quiet hours · {qf}–{qt}")
                };
                row![
                    text(label).size(11),
                    quiet(text("Off").size(11))
                        .on_press(Message::QuietHoursPreset(String::new()))
                        .padding([2, 6]),
                    quiet(text("22–07").size(11))
                        .on_press(Message::QuietHoursPreset("22:00-07:00".into()))
                        .padding([2, 6]),
                    quiet(text("23–06").size(11))
                        .on_press(Message::QuietHoursPreset("23:00-06:00".into()))
                        .padding([2, 6]),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
            },
        ]
        .spacing(8);

        let mut items = column![].spacing(8);

        if self.notifs.items.is_empty() {
            items = items.push(
                container(
                    column![
                        text("No new notifications").size(14),
                        text("You’re all caught up.").size(12),
                    ]
                    .spacing(4)
                    .align_x(iced::Alignment::Center),
                )
                .center_x(Fill)
                .padding([16, 12]),
            );
        } else {
            // Group by app_name (W36), preserving first-seen order.
            let mut groups: Vec<(String, Vec<&notifications::Notification>)> = Vec::new();
            for n in &self.notifs.items {
                let key = if n.app_name.trim().is_empty() {
                    "App".to_string()
                } else {
                    n.app_name.clone()
                };
                if let Some((_, list)) = groups.iter_mut().find(|(k, _)| k == &key) {
                    list.push(n);
                } else {
                    groups.push((key, vec![n]));
                }
            }
            for (app, notes) in groups {
                let mute_label = app.clone();
                let clear_label = app.clone();
                let priority_label = app.clone();
                let settings_label = app.clone();
                let toggle_label = app.clone();
                // W225: chevron collapses/expands this app's items.
                let collapsed = self.nc_collapsed.contains(&app);
                let chevron = if collapsed { "▸" } else { "▾" };
                items = items.push(
                    row![
                        quiet(text(format!("{chevron} {app}")).size(12))
                            .on_press(Message::ToggleNotifGroup(toggle_label))
                            .padding([2, 4]),
                        Space::with_width(Fill),
                        text(format!("{}", notes.len())).size(11),
                        // W185: app notification settings deep link.
                        quiet(text("Settings").size(11))
                            .on_press(Message::NotifAppSettings(settings_label))
                            .padding([2, 8]),
                        // W156: elevate app into priority-apps.txt (unmute if muted).
                        quiet(text("Priority").size(11))
                            .on_press(Message::ElevateNotifPriority(priority_label))
                            .padding([2, 8]),
                        // W165: Clear clears only this app_name group.
                        quiet(text("Clear").size(11))
                            .on_press(Message::ClearNotifGroup(clear_label))
                            .padding([2, 8]),
                        quiet(text("Mute").size(11))
                            .on_press(Message::MuteNotifApp(mute_label))
                            .padding([2, 8]),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
                if collapsed {
                    continue;
                }
                for n in notes {
                    let mut body = column![
                        text(n.summary.clone()).size(14),
                        text(n.body.clone()).size(12),
                    ]
                    .spacing(2)
                    .width(Fill);
                    if !n.actions.is_empty() {
                        let mut actions = row![].spacing(6);
                        for (key, label) in n.actions.iter().take(4) {
                            let shown = if label.is_empty() {
                                key.clone()
                            } else {
                                label.clone()
                            };
                            let action_key = key.clone();
                            let id = n.id;
                            actions = actions.push(
                                quiet(text(shown).size(12))
                                    .on_press(Message::NotifAction(id, action_key)),
                            );
                        }
                        body = body.push(actions);
                    }
                    // W125: per-notif snooze 15m / 1h.
                    let sid = n.id;
                    body = body.push(
                        row![
                            quiet(text("Snooze 15m").size(11))
                                .on_press(Message::SnoozeNotif {
                                    id: sid,
                                    secs: 15 * 60,
                                })
                                .padding([2, 6]),
                            quiet(text("Snooze 1h").size(11))
                                .on_press(Message::SnoozeNotif {
                                    id: sid,
                                    secs: 60 * 60,
                                })
                                .padding([2, 6]),
                        ]
                        .spacing(6),
                    );
                    let card =
                        row![body, quiet(text("×").size(14)).on_press(Message::DismissNotif(n.id))]
                            .spacing(10);
                    items = items.push(flyout_inset(card, &self.palette));
                }
            }
        }

        // W105: show muted apps with Unmute.
        if !self.notifs.muted_apps.is_empty() {
            items = items.push(Space::with_height(4));
            items = items.push(text("Muted apps").size(12));
            for app in &self.notifs.muted_apps {
                let name = app.clone();
                items = items.push(
                    row![
                        text(app.clone()).size(12),
                        Space::with_width(Fill),
                        quiet(text("Unmute").size(11))
                            .on_press(Message::UnmuteNotifApp(name))
                            .padding([2, 8]),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
        }

        // Only the notification list scrolls; chrome + calendar stay visible.
        let list_area: Element<'_, Message> = if self.notifs.items.is_empty()
            && self.notifs.muted_apps.is_empty()
        {
            // Empty state fits without a scroll viewport.
            items.into()
        } else {
            scrollable(items).height(Fill).into()
        };

        container(
            column![
                chrome,
                list_area,
                Space::with_height(6),
                hairline(&self.palette),
                self.view_month_calendar(),
            ]
            .spacing(6),
        )
        .padding([12, 16])
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_toast_stack(&self) -> Element<'_, Message> {
        // W196: ≤2 stacked toasts — primary on top, second under first.
        // W236: Dismiss all when a second toast is stacked.
        let mut col = column![].spacing(8);
        if self.toast.is_some() && self.toast_stack.is_some() {
            col = col.push(
                row![
                    Space::with_width(Fill),
                    quiet(text("Dismiss all").size(12))
                        .on_press(Message::DismissAllToasts)
                        .padding([4, 8]),
                ]
                .spacing(8),
            );
        }
        if let Some(n) = &self.toast {
            col = col.push(self.view_toast(n, true));
        }
        if let Some(n) = &self.toast_stack {
            col = col.push(self.view_toast(n, false));
        }
        container(col)
            .padding([8, 10])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_toast(&self, n: &notifications::Notification, primary: bool) -> Element<'_, Message> {
        // W136: collapsed = one line; Expand / … toggles full body (primary only).
        let expanded = primary && self.toast_expanded;
        let body_text: Element<'_, Message> = if n.body.trim().is_empty() {
            Space::with_height(0).into()
        } else if expanded {
            text(n.body.clone()).size(12).into()
        } else {
            let one: String = n.body.chars().take(72).collect();
            let shown = if n.body.chars().count() > 72 {
                format!("{one}…")
            } else {
                one
            };
            text(shown).size(12).into()
        };
        let expand_label = if expanded { "Collapse" } else { "…" };
        let mut head = row![text(n.app_name.clone()).size(11), Space::with_width(Fill)].spacing(8);
        if primary {
            head = head.push(
                quiet(text(expand_label).size(12)).on_press(Message::ToggleToastExpand),
            );
        }
        head = head.push(quiet(text("×").size(14)).on_press(Message::DismissToastId(n.id)));
        let mut body = column![
            head,
            text(n.summary.clone()).size(14),
            body_text,
        ]
        .spacing(4)
        .width(Fill);
        // W47: up to 2 action buttons on the toast card.
        if !n.actions.is_empty() {
            let mut actions = row![].spacing(6);
            for (key, label) in n.actions.iter().take(2) {
                let shown = if label.is_empty() {
                    key.clone()
                } else {
                    label.clone()
                };
                let action_key = key.clone();
                let id = n.id;
                actions = actions.push(
                    quiet(text(shown).size(12)).on_press(Message::NotifAction(id, action_key)),
                );
            }
            body = body.push(actions);
        }
        // W438: progress bar only from real Notify hints (no keyword heuristic).
        if let Some(fill) = n.progress {
            body = body.push(Space::with_height(4));
            body = body.push(thin_progress(fill.clamp(0.0, 1.0), &self.palette));
        }
        flyout_inset(body, &self.palette)
    }

    pub(super) fn view_clear_undo_toast(&self) -> Element<'_, Message> {
        let n = self
            .cleared_stash
            .as_ref()
            .map(|v| v.len())
            .unwrap_or(0);
        container(
            row![
                text(format!("Cleared {n} notifications")).size(13),
                Space::with_width(Fill),
                quiet(text("Undo").size(13))
                    .on_press(Message::UndoClearNotifs)
                    .style(accent_pill)
                    .padding([6, 14]),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .padding([12, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_clipboard(&self) -> Element<'_, Message> {
        let clear_row: Element<'_, Message> = if self.clip_clear_confirm {
            row![
                text("Clear unpinned clips?").size(12),
                Space::with_width(Fill),
                quiet(text("Yes").size(12))
                    .on_press(Message::ConfirmClearClip)
                    .padding([4, 10]),
                quiet(text("No").size(12))
                    .on_press(Message::CancelClearClip)
                    .padding([4, 10]),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            row![
                text("Clipboard history").size(15),
                Space::with_width(Fill),
                quiet(text("Clear all").size(12)).on_press(Message::ClearClip),
            ]
            .spacing(8)
            .into()
        };
        let filter = text_input("Filter clipboard", &self.clip_query)
            .on_input(Message::ClipQuery)
            .padding(8)
            .size(13);
        // W163: pause cloud sync (clears/skips folder write when path exists).
        let sync_label = if self.clipboard.sync_paused {
            "Resume sync"
        } else {
            "Pause sync"
        };
        // W283: max history items (extras clipboard.max_items).
        let max_items = self.clip_max_items();
        let max_row = row![
            text(format!("Max items · {max_items}")).size(11),
            quiet(text("−").size(14)).on_press(Message::ClipMaxItemsDelta(-5)),
            quiet(text("+").size(14)).on_press(Message::ClipMaxItemsDelta(5)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let sync_row = row![
            text(if self.clipboard.sync_path_active() {
                "Cloud clipboard"
            } else {
                "Cloud clipboard · off"
            })
            .size(12),
            Space::with_width(Fill),
            quiet(text(sync_label).size(12))
                .on_press(Message::ToggleClipSyncPause)
                .padding([4, 10]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let mut list = column![
            clear_row,
            max_row,
            sync_row,
            filter,
            text("Click an item to paste it.").size(12),
            hairline(&self.palette),
        ]
        .spacing(10);

        if self.clipboard.items.is_empty() {
            list = list.push(
                container(
                    column![
                        text("Nothing here yet").size(14),
                        text("Copy text or images to build history.").size(12),
                    ]
                    .spacing(4)
                    .align_x(iced::Alignment::Center),
                )
                .center_x(Fill)
                .padding([40, 12]),
            );
        }
        let clip_q = self.clip_query.trim().to_lowercase();
        let mut shown = 0usize;
        for e in &self.clipboard.items {
            if !clip_q.is_empty() {
                let hay = format!("{} {} {}", e.preview, e.full, e.mime).to_lowercase();
                if !hay.contains(&clip_q) {
                    continue;
                }
            }
            shown += 1;
            let mark = if e.pinned { "Unpin" } else { "Pin" };
            let preview: String = e.preview.chars().take(120).collect();
            let age = relative_age(e.created);
            let kind = if !e.image_file.is_empty() {
                let name = std::path::Path::new(&e.image_file)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "image".into());
                if e.pinned {
                    format!("Pinned · Image · {name} · {age}")
                } else {
                    format!("Image · {name} · {age}")
                }
            } else if e.mime == "text/html" {
                if e.pinned {
                    format!("Pinned · HTML · {age}")
                } else {
                    format!("HTML · {age}")
                }
            } else if e.mime == "text/uri-list" {
                if e.pinned {
                    format!("Pinned · Links · {age}")
                } else {
                    format!("Links · {age}")
                }
            } else if e.pinned {
                format!("Pinned · {age}")
            } else {
                format!("Text · {age}")
            };
            let is_textish = e.image_file.is_empty();
            let mut actions = row![quiet(text(mark).size(12)).on_press(Message::PinClip(e.id))];
            // W203: ↑/↓ reorder among pinned clipboard items.
            if e.pinned {
                let id_up = e.id;
                let id_dn = e.id;
                actions = actions.push(
                    quiet(text("↑").size(11)).on_press(Message::ReorderPinnedClip {
                        id: id_up,
                        delta: -1,
                    }),
                );
                actions = actions.push(
                    quiet(text("↓").size(11)).on_press(Message::ReorderPinnedClip {
                        id: id_dn,
                        delta: 1,
                    }),
                );
            }
            // W123 / A041: strip formatting and copy plain via wl-copy.
            if is_textish {
                actions = actions.push(
                    quiet(text("Paste as text").size(11))
                        .on_press(Message::PasteClipPlain(e.id)),
                );
                // A042: Advanced Paste transforms.
                if self.cfg.active().tools.advanced_paste {
                    actions = actions.push(
                        quiet(text("MD").size(11))
                            .on_press(Message::AdvancedPaste(e.id, "markdown".into())),
                    );
                    actions = actions.push(
                        quiet(text("HTML").size(11))
                            .on_press(Message::AdvancedPaste(e.id, "html".into())),
                    );
                    actions = actions.push(
                        quiet(text("JSON").size(11))
                            .on_press(Message::AdvancedPaste(e.id, "json".into())),
                    );
                    actions = actions.push(
                        quiet(text("Aa").size(11))
                            .on_press(Message::AdvancedPaste(e.id, "upper".into())),
                    );
                }
            } else {
                // W243: Save image → ~/Pictures/Clipboard PNG.
                actions = actions.push(
                    quiet(text("Save").size(11)).on_press(Message::SaveClipImage(e.id)),
                );
            }
            // W183: paste as file stub.
            actions = actions.push(
                quiet(text("Paste as file").size(11))
                    .on_press(Message::PasteClipAsFileStub),
            );
            // W263: Share → Nearby Share stub.
            actions = actions.push(
                quiet(text("Share").size(11)).on_press(Message::ClipShareNearbyStub),
            );
            let card = row![
                quiet(
                    column![
                        text(kind).size(11),
                        text(preview).size(13),
                    ]
                    .spacing(3)
                    .width(Fill),
                )
                .on_press(Message::PasteClip(e.id))
                .width(Fill)
                .padding([4, 2]),
                actions.spacing(6),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);
            list = list.push(flyout_inset(card, &self.palette));
        }
        if !self.clipboard.items.is_empty() && shown == 0 && !clip_q.is_empty() {
            list = list.push(text("No matching clips").size(12));
        }

        container(scrollable(list).height(Fill))
            .padding([16, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }
}
