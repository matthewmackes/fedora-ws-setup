//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_quick(&self) -> Element<'_, Message> {
        if self.qs_edit {
            return self.view_quick_edit();
        }

        let tiles = self.qs_tile_grid(false);
        let vol_icon = if self.vol_muted {
            "audio-volume-muted"
        } else {
            "audio-volume-high"
        };
        let vol_pct = if self.vol_muted {
            0u32
        } else {
            (self.vol_level.clamp(0.0, 1.0) * 100.0).round() as u32
        };
        let bright_pct = (self.bright_level.clamp(0.0, 1.0) * 100.0).round() as u32;
        let volume = column![
            row![
                chrome_btn(vol_icon, "Vol", 18, self.palette.dark).on_press(Message::VolMute),
                column![
                    text("Volume").size(12),
                    level_track(
                        if self.vol_muted { 0.0 } else { self.vol_level },
                        &self.palette,
                    ),
                ]
                .spacing(4)
                .width(Fill),
                text(format!("{vol_pct}%")).size(12),
                quiet(text("−").size(16)).on_press(Message::VolDown),
                quiet(text("+").size(16)).on_press(Message::VolUp),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            // W118: multi-app mixer stub · W238: classic mixer.
            // A036: discoverable mixer entry from the volume row.
            row![
                quiet(text("Mixer").size(11))
                    .on_press(Message::OpenAppMixer)
                    .padding([2, 4]),
                quiet(text("App volume").size(11))
                    .on_press(Message::OpenAppMixer)
                    .padding([2, 4]),
                quiet(text("Settings…").size(11))
                    .on_press(Message::OpenControl("mixer".into()))
                    .padding([2, 4]),
                quiet(text("Classic…").size(11))
                    .on_press(Message::OpenVolumeMixer)
                    .padding([2, 4]),
            ]
            .spacing(8),
        ]
        .spacing(4);

        let brightness = column![
            row![
                chrome_btn("display-brightness", "Bright", 18, self.palette.dark).on_press(Message::BrightUp),
                column![
                    text("Brightness").size(12),
                    level_track(self.bright_level, &self.palette),
                ]
                .spacing(4)
                .width(Fill),
                text(format!("{bright_pct}%")).size(12),
                quiet(text("−").size(16)).on_press(Message::BrightDown),
                quiet(text("+").size(16)).on_press(Message::BrightUp),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(4);

        let mut body = column![tiles, volume, brightness].spacing(12);

        // W49: Now Playing + Play/Pause when playerctl exists.
        if self.media_available {
            let title = if self.media_title.is_empty() {
                "Not playing".to_string()
            } else {
                self.media_title.clone()
            };
            let btn = if self.media_playing { "Pause" } else { "Play" };
            body = body.push(
                row![
                    chrome_btn("audio-x-generic", "Media", 18, self.palette.dark).on_press(Message::MediaPlayPause),
                    column![text("Now playing").size(11), text(title).size(12)]
                        .spacing(2)
                        .width(Fill),
                    quiet(text(btn).size(12))
                        .on_press(Message::MediaPlayPause)
                        .padding([6, 10]),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }

        let battery: Element<'_, Message> = if let Some(pct) = self.battery_pct {
            let label = if self.battery_charging {
                format!("{pct}% · charging")
            } else {
                format!("{pct}%")
            };
            text(label).size(12).into()
        } else {
            Space::with_width(Fill).into()
        };
        let footer = row![
            battery,
            Space::with_width(Fill),
            quiet(text("Edit").size(12))
                .on_press(Message::EditQsTiles)
                .padding([4, 8]),
            chrome_btn("edit-copy", "Clip", 16, self.palette.dark).on_press(Message::ToggleClipboard),
            chrome_btn("org.gnome.Settings", "Settings", 16, self.palette.dark)
                .on_press(Message::OpenControl(String::new())),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        // No scrollable — QS_CARD_H sizes the card to fit this compact layout.
        container(
            column![body, footer]
                .spacing(12)
                .padding([16, SIDE_PAD_X])
                .width(Fill)
                .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// W98: add/remove pinned QS tiles; Done returns to the compact surface.
    fn view_quick_edit(&self) -> Element<'_, Message> {
        let pinned = self.qs_tiles();
        let pinned_grid = self.qs_tile_grid(true);

        let mut available = column![
            text("Add a quick setting").size(13),
        ]
        .spacing(6);
        let mut any = false;
        for (id, icon, label) in QS_CATALOG {
            if pinned.iter().any(|t| t == id) {
                continue;
            }
            any = true;
            let id = (*id).to_string();
            available = available.push(
                row![
                    chrome_btn(icon, label, 16, self.palette.dark),
                    text(*label).size(12).width(Fill),
                    quiet(text("Add").size(12))
                        .on_press(Message::QsAddTile(id))
                        .padding([4, 8]),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if !any {
            available = available.push(
                text("All tiles are pinned")
                    .size(11)
                    .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
            );
        }

        let header = row![
            text("Edit quick settings").size(15),
            Space::with_width(Fill),
            quiet(text("Done").size(13))
                .on_press(Message::QsEditDone)
                .padding([4, 10]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        // Scroll only the add list — pinned grid stays visible (Win11 edit).
        container(
            column![
                header,
                pinned_grid,
                hairline(&self.palette),
                scrollable(available).height(Fill),
            ]
            .spacing(12)
            .padding([16, SIDE_PAD_X])
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn qs_tile_grid(&self, editing: bool) -> Element<'_, Message> {
        let ids = self.qs_tiles();
        let mut rows = column![].spacing(QS_GAP);
        let mut row_els: Vec<Element<'_, Message>> = Vec::new();
        for id in &ids {
            let tile = if editing {
                self.qs_edit_tile(id)
            } else {
                self.qs_live_tile(id)
            };
            row_els.push(tile);
            if row_els.len() == 3 {
                let mut r = row![].spacing(QS_GAP);
                for el in row_els.drain(..) {
                    r = r.push(el);
                }
                rows = rows.push(r);
            }
        }
        if !row_els.is_empty() {
            let n = row_els.len();
            let mut r = row![].spacing(QS_GAP);
            for el in row_els {
                r = r.push(el);
            }
            // Pad incomplete rows so tiles keep QS_TILE_W alignment.
            for _ in n..3 {
                r = r.push(Space::with_width(QS_TILE_W));
            }
            rows = rows.push(r);
        }
        rows.into()
    }

    fn qs_edit_tile(&self, id: &str) -> Element<'static, Message> {
        let icon = qs_catalog_icon(id).unwrap_or("preferences-system");
        let label = qs_catalog_label(id).unwrap_or(id);
        let id_up = id.to_string();
        let id_down = id.to_string();
        let id_rm = id.to_string();
        // A035: reorder with ↑/↓ plus remove (gallery polish).
        container(
            column![
                chrome_btn(icon, label, 16, self.palette.dark),
                text(label.to_string()).size(11),
                row![
                    quiet(text("↑").size(12))
                        .on_press(Message::QsMoveTile(id_up, -1))
                        .padding([2, 6]),
                    quiet(text("↓").size(12))
                        .on_press(Message::QsMoveTile(id_down, 1))
                        .padding([2, 6]),
                    quiet(text("−").size(12))
                        .on_press(Message::QsRemoveTile(id_rm))
                        .padding([2, 6]),
                ]
                .spacing(4),
            ]
            .spacing(4)
            .align_x(iced::Alignment::Center)
            .padding(8),
        )
        .width(QS_TILE_W)
        .into()
    }

    fn qs_live_tile(&self, id: &str) -> Element<'_, Message> {
        let tile_w = QS_TILE_W;
        match id {
            "wifi" => {
                if self.airplane {
                    qs_tile_dim(
                        "network-wireless",
                        "Wi-Fi",
                        Message::SetWifi(true),
                        tile_w,
                        self.palette.dark)
                } else {
                    qs_tile(
                        "network-wireless",
                        "Wi-Fi",
                        self.wifi_on,
                        Message::SetWifi(!self.wifi_on),
                        tile_w,
                        self.palette.dark)
                }
            }
            "bluetooth" => {
                if self.airplane {
                    qs_tile_dim("bluetooth", "Bluetooth", Message::SetBt(true), tile_w, self.palette.dark)
                } else if self.bt_on {
                    // A037: expand device list / pair wizard when radio is on.
                    qs_tile(
                        "bluetooth",
                        "Bluetooth",
                        true,
                        Message::OpenBtFlyout,
                        tile_w,
                        self.palette.dark)
                } else {
                    qs_tile(
                        "bluetooth",
                        "Bluetooth",
                        false,
                        Message::SetBt(true),
                        tile_w,
                        self.palette.dark)
                }
            }
            "airplane" => qs_tile(
                "airplane-mode",
                "Airplane",
                self.airplane,
                Message::SetAirplane(!self.airplane),
                tile_w,
                self.palette.dark),
            "dnd" => qs_tile(
                "notification-disabled",
                "DND",
                self.notifs.dnd,
                Message::ToggleDnd,
                tile_w,
                self.palette.dark),
            "night_light" => qs_tile(
                "night-light",
                "Eye comfort",
                self.cfg.active().session.night_light,
                Message::ToggleNightLight,
                tile_w,
                self.palette.dark),
            "dark" => qs_dark_tile(
                if self.palette.dark {
                    "weather-clear-night"
                } else {
                    "weather-clear"
                },
                self.palette.dark,
                self.cfg.active().theme.dark_schedule,
                Message::ToggleThemeDark,
                tile_w,
                self.palette.dark,
            ),
            "accessibility" => qs_tile(
                "preferences-desktop-accessibility",
                "Accessibility",
                false,
                Message::AccessibilityStub,
                tile_w,
                self.palette.dark),
            "timer" => qs_tile(
                "appointment-soon",
                "Focus timer",
                self.focus_timer_active(),
                Message::ToggleFocusTimer,
                tile_w,
                self.palette.dark),
            "awake" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let on = tools::awake_is_active(&self.awake_state, now);
                qs_tile("alarm", "Keep awake", on, Message::ToggleAwake, tile_w, self.palette.dark)
            }
            "keyboard" => qs_tile(
                "input-keyboard",
                "Keyboard",
                self.keyboard_open,
                Message::ToggleKeyboard,
                tile_w,
                self.palette.dark),
            "project" => qs_tile(
                "preferences-desktop-display",
                "Project",
                self.connect_open || self.share_active,
                Message::ToggleConnect,
                tile_w,
                self.palette.dark),
            "priority" => qs_tile(
                "emblem-important",
                "Priority",
                self.notifs.priority,
                Message::TogglePriority,
                tile_w,
                self.palette.dark),
            "auto_brightness" => qs_tile(
                "weather-clear",
                "Dynamic brightness",
                self.auto_bright_enabled(),
                Message::ToggleAutoBright,
                tile_w,
                self.palette.dark),
            "cast" => qs_tile(
                "video-display",
                "Wireless display",
                self.connect_open || self.share_active,
                Message::ToggleConnect,
                tile_w,
                self.palette.dark),
            "nearby" => qs_tile(
                "network-wireless-connected-symbolic",
                "Nearby Share",
                self.nearby_flyout_open,
                Message::ToggleNearbyFlyout,
                tile_w,
                self.palette.dark),
            "autohide" => qs_tile(
                "view-paged-symbolic",
                "Auto-hide",
                self.cfg.active().shell.taskbar_autohide,
                Message::ToggleAutoHide,
                tile_w,
                self.palette.dark),
            "eject" => qs_tile(
                "media-eject",
                "Eject tip",
                false,
                Message::SafeRemoveUsb,
                tile_w,
                self.palette.dark),
            "hotspot" => qs_tile(
                "network-wireless",
                "Mobile hotspot",
                self.hotspot_flyout_open || self.hotspot.active,
                Message::ToggleHotspotFlyout,
                tile_w,
                self.palette.dark),
            "notif_sounds" => qs_tile(
                "audio-volume-high",
                "Notif sounds",
                self.notif_sounds_enabled(),
                Message::ToggleNotifSounds,
                tile_w,
                self.palette.dark),
            "taskbar_left" => qs_tile(
                "object-align-left",
                "Taskbar left",
                self.cfg.active().shell.taskbar_left,
                Message::ToggleTaskbarLeft,
                tile_w,
                self.palette.dark),
            "hidden_icons" => qs_tile(
                "view-more-symbolic",
                "Hidden icons",
                self.tray_open,
                Message::OpenHiddenIcons,
                tile_w,
                self.palette.dark),
            "combine" => qs_tile(
                "view-grid-symbolic",
                "Combine",
                self.cfg
                    .active()
                    .shell
                    .taskbar_combine
                    .eq_ignore_ascii_case("always"),
                Message::ToggleTaskbarCombine,
                tile_w,
                self.palette.dark),
            "vpn" => qs_tile(
                "network-vpn",
                "VPN",
                self.vpn_flyout_open || self.vpn_list.iter().any(|v| v.active),
                Message::ToggleVpnFlyout,
                tile_w,
                self.palette.dark),
            "show_labels" => qs_tile(
                "format-text-symbolic",
                "Show labels",
                self.taskbar_labels(),
                Message::ToggleTaskbarLabels,
                tile_w,
                self.palette.dark),
            "alttab" => qs_tile(
                "user-desktop",
                "Alt+Tab",
                self.alttab_shows_desktop(),
                Message::ToggleAlttabDesktop,
                tile_w,
                self.palette.dark),
            other => {
                let icon = qs_catalog_icon(other).unwrap_or("preferences-system");
                let label = qs_catalog_label(other).unwrap_or("Tile");
                qs_stub_tile(icon, label, tile_w, self.palette.dark)
            }
        }
    }

    pub(super) fn view_volume_flyout(&self) -> Element<'_, Message> {
        let vol_icon = if self.vol_muted {
            "audio-volume-muted"
        } else {
            "audio-volume-high"
        };
        container(
            column![
                row![
                    text("Volume").size(15),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                Space::with_height(6),
                row![
                    chrome_btn(vol_icon, "Vol", 18, self.palette.dark).on_press(Message::VolMute),
                    column![
                        text(format!(
                            "{}%",
                            ((if self.vol_muted { 0.0 } else { self.vol_level }) * 100.0).round()
                                as i32
                        ))
                        .size(12),
                        level_track(
                            if self.vol_muted { 0.0 } else { self.vol_level },
                            &self.palette,
                        ),
                    ]
                    .spacing(4)
                    .width(Fill),
                    quiet(text("−").size(16)).on_press(Message::VolDown),
                    quiet(text("+").size(16)).on_press(Message::VolUp),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(8)
            .padding([14, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_battery_flyout(&self) -> Element<'_, Message> {
        let pct = self.battery_pct.unwrap_or(0);
        let state = if self.battery_charging {
            "Charging"
        } else if pct <= 15 {
            "Low"
        } else {
            "On battery"
        };
        let estimate = system::battery_time_remaining();
        let bat_icon = battery_icon_name(pct, self.battery_charging);
        container(
            column![
                row![
                    text("Battery").size(15),
                    Space::with_width(Fill),
                    quiet(text("Close").size(12)).on_press(Message::Dismiss),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                Space::with_height(6),
                row![
                    chrome_btn(bat_icon, "Bat", 22, self.palette.dark),
                    {
                        let mut col = column![
                            text(format!("{pct}%")).size(22),
                            text(state).size(12),
                        ]
                        .spacing(2)
                        .width(Fill);
                        if let Some(est) = &estimate {
                            col = col.push(text(est.clone()).size(11));
                        }
                        col
                    },
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
                level_track((pct as f32) / 100.0, &self.palette),
                // W400: battery saver toggle.
                quiet(text(if self.battery_saver_on() {
                    "Battery saver · On"
                } else {
                    "Battery saver · Off"
                })
                .size(11))
                .on_press(Message::ToggleBatterySaver)
                .padding([4, 0]),
                // A089: Win11-like power mode row via power-profiles-daemon.
                {
                    let cur = system::power_profile().unwrap_or_default();
                    let modes = [
                        ("power-saver", "Best efficiency"),
                        ("balanced", "Balanced"),
                        ("performance", "Best performance"),
                    ];
                    let mut row_modes = row![].spacing(6);
                    for (id, label) in modes {
                        let on = cur.eq_ignore_ascii_case(id);
                        row_modes = row_modes.push(
                            quiet(text(if on {
                                format!("✓ {label}")
                            } else {
                                label.to_string()
                            })
                            .size(11))
                            .on_press(Message::SetPowerProfile(id.to_string()))
                            .padding([4, 6]),
                        );
                    }
                    column![
                        text("Power mode").size(12),
                        row_modes,
                    ]
                    .spacing(4)
                },
            ]
            .spacing(8)
            .padding([14, 16]),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub(super) fn view_network_flyout(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("Network").size(15),
                Space::with_width(Fill),
                quiet(text("Refresh").size(12)).on_press(Message::RefreshWifi),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text("Quick connect · top SSIDs").size(11),
            hairline(&self.palette),
        ]
        .spacing(8);
        if self.wifi_aps.is_empty() {
            list = list.push(text("No networks found (nmcli)").size(13));
        }
        for ap in &self.wifi_aps {
            let mark = if ap.connected { "Connected" } else { "Connect" };
            let label = format!("{} · {}%", ap.ssid, ap.signal);
            let ssid = ap.ssid.clone();
            list = list.push(flyout_inset(
                row![
                    column![text(label).size(13), text(mark).size(11)]
                        .spacing(2)
                        .width(Fill),
                    quiet(text(if ap.connected { "OK" } else { "Join" }).size(12))
                        .on_press(Message::ConnectWifi(ssid)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                &self.palette,
            ));
        }
        // A038: inline password join for unknown SSIDs.
        if !self.wifi_password_ssid.is_empty() {
            list = list.push(hairline(&self.palette));
            list = list.push(
                text(format!("Password for {}", self.wifi_password_ssid)).size(12),
            );
            list = list.push(
                text_input("Wi-Fi password", &self.wifi_password)
                    .on_input(Message::WifiPasswordInput)
                    .on_submit(Message::WifiJoinWithPassword)
                    .secure(true)
                    .padding(8)
                    .size(13),
            );
            list = list.push(
                row![
                    quiet(text("Join").size(12))
                        .on_press(Message::WifiJoinWithPassword)
                        .padding([6, 10]),
                    quiet(text("Cancel").size(12))
                        .on_press(Message::WifiAskPassword(String::new()))
                        .padding([6, 10]),
                ]
                .spacing(8),
            );
        }
        list = list.push(
            quiet(text("More network settings…").size(12))
                .on_press(Message::OpenControl("wifi".into()))
                .padding([6, 4]),
        );
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_app_mixer(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("Volume mixer").size(15),
                Space::with_width(Fill),
                quiet(text("Settings…").size(12)).on_press(Message::OpenControl("mixer".into())),
                quiet(text("Classic…").size(12)).on_press(Message::OpenVolumeMixer),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            // A036: parity with Settings — system row + % labels + mute.
            row![
                chrome_btn(
                    if self.vol_muted {
                        "audio-volume-muted"
                    } else {
                        "audio-volume-high"
                    },
                    "Vol",
                    18,
                    self.palette.dark)
                .on_press(Message::VolMute),
                column![
                    text("System").size(12),
                    level_track(
                        if self.vol_muted { 0.0 } else { self.vol_level },
                        &self.palette,
                    ),
                ]
                .spacing(4)
                .width(Fill),
                text(format!(
                    "{}%",
                    ((if self.vol_muted { 0.0 } else { self.vol_level }) * 100.0).round() as i32
                ))
                .size(12),
                quiet(text("−").size(14)).on_press(Message::VolDown),
                quiet(text("+").size(14)).on_press(Message::VolUp),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            hairline(&self.palette),
            text("Apps").size(11),
        ]
        .spacing(8);
        if self.app_streams.is_empty() {
            list = list.push(text("No application streams (wpctl)").size(13));
        }
        for stream in &self.app_streams {
            let id = stream.id;
            list = list.push(flyout_inset(
                row![
                    column![
                        text(stream.label.clone()).size(13),
                        text(format!("node {id}")).size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    quiet(text("−").size(14)).on_press(Message::AppStreamVol(id, -5)),
                    quiet(text("+").size(14)).on_press(Message::AppStreamVol(id, 5)),
                    quiet(text("Mute").size(12)).on_press(Message::AppStreamMute(id)),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
                &self.palette,
            ));
        }
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_vpn_flyout(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("VPN").size(15),
                Space::with_width(Fill),
                quiet(text("Refresh").size(12)).on_press(Message::RefreshVpn),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text("NetworkManager VPN / WireGuard profiles").size(11),
            hairline(&self.palette),
        ]
        .spacing(8);
        if self.vpn_list.is_empty() {
            list = list.push(text("No VPN profiles (nmcli)").size(13));
            list = list.push(
                quiet(text("Add in nm-connection-editor…").size(12))
                    .on_press(Message::OpenControl("vpn".into()))
                    .padding([6, 4]),
            );
        }
        for vpn in &self.vpn_list {
            let mark = if vpn.active {
                "Connected".to_string()
            } else {
                vpn.kind.clone()
            };
            let name = vpn.name.clone();
            let action = if vpn.active {
                quiet(text("Disconnect").size(12)).on_press(Message::VpnDisconnect(name))
            } else {
                quiet(text("Connect").size(12)).on_press(Message::VpnConnect(name))
            };
            list = list.push(flyout_inset(
                row![
                    column![text(vpn.name.clone()).size(13), text(mark).size(11)]
                        .spacing(2)
                        .width(Fill),
                    action,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                &self.palette,
            ));
        }
        list = list.push(
            quiet(text("More VPN settings…").size(12))
                .on_press(Message::OpenControl("vpn".into()))
                .padding([6, 4]),
        );
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    pub(super) fn view_hotspot_flyout(&self) -> Element<'_, Message> {
        let hs = &self.hotspot;
        let state = if hs.active { "On" } else { "Off" };
        let ssid = if hs.ssid.is_empty() {
            "MDE-Hotspot".to_string()
        } else {
            hs.ssid.clone()
        };
        let mut list = column![
            row![
                text("Mobile hotspot").size(15),
                Space::with_width(Fill),
                quiet(text("Refresh").size(12)).on_press(Message::RefreshHotspot),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(format!("Status · {state}")).size(11),
            hairline(&self.palette),
            flyout_inset(
                column![
                    text(format!("SSID · {ssid}")).size(13),
                    text(if hs.password.is_empty() {
                        "Password · set on Start".into()
                    } else {
                        format!("Password · {}", hs.password)
                    })
                    .size(12),
                    text(if hs.device.is_empty() {
                        "Device · auto".into()
                    } else {
                        format!("Device · {}", hs.device)
                    })
                    .size(11),
                ]
                .spacing(4)
                .width(Fill),
                &self.palette,
            ),
        ]
        .spacing(8);
        if hs.active {
            list = list.push(
                quiet(text("Stop hotspot").size(13))
                    .on_press(Message::HotspotStop)
                    .padding([8, 4]),
            );
        } else {
            list = list.push(
                quiet(text("Start hotspot").size(13))
                    .on_press(Message::HotspotStart)
                    .padding([8, 4]),
            );
        }
        list = list.push(
            text("Uses NetworkManager Wi-Fi AP / shared connection")
                .size(11)
                .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
        );
        container(list)
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// W395 / W448: Nearby Share (KDE Connect) device list + pairing + share actions.
    pub(super) fn view_nearby_flyout(&self) -> Element<'_, Message> {
        let st = &self.nearby;
        let mut list = column![
            row![
                text("Nearby Share").size(15),
                Space::with_width(Fill),
                quiet(text("Refresh").size(12)).on_press(Message::RefreshNearby),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(&st.hint).size(11),
            hairline(&self.palette),
        ]
        .spacing(8);

        if let Some(pending) = &self.nearby_pending_clip {
            let preview: String = pending.chars().take(48).collect();
            list = list.push(flyout_inset(
                column![
                    text("Send clipboard").size(12),
                    text(preview).size(11),
                    text("Pick a paired device below").size(10),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            ));
        }

        if !st.installed {
            list = list.push(flyout_inset(
                column![
                    text("KDE Connect not installed").size(13),
                    text("sudo dnf install kde-connect").size(11),
                    text("Firewall · ports 1716–1764 · docs/kde-connect.md").size(10),
                ]
                .spacing(4)
                .width(Fill),
                &self.palette,
            ));
        } else if st.devices.is_empty() {
            list = list.push(flyout_inset(
                column![
                    text("No devices yet").size(13),
                    text("Open KDE Connect on your phone on the same LAN.").size(11),
                    text("Unpaired devices appear here for Pair.").size(10),
                ]
                .spacing(4)
                .width(Fill),
                &self.palette,
            ));
        }

        for d in &st.devices {
            let mut state = match (d.paired, d.reachable) {
                (true, true) => "Paired · reachable",
                (true, false) => "Paired · offline",
                (false, true) => "Unpaired · visible",
                (false, false) => "Unpaired · offline",
            }
            .to_string();
            if d.paired {
                if let Some((pct, chg)) = kdeconnect::device_battery(&d.id) {
                    state.push_str(&format!(
                        " · phone {pct}%{}",
                        if chg { " charging" } else { "" }
                    ));
                }
                if kdeconnect::notifications_plugin_active(&d.id) {
                    state.push_str(" · notif mirror");
                }
            }
            let id = d.id.clone();
            let mut actions = row![].spacing(6);
            if self.nearby_pending_clip.is_some() && d.paired {
                actions = actions.push(
                    quiet(text("Send clip").size(11))
                        .on_press(Message::NearbyPickShare(id.clone())),
                );
            }
            if !d.paired {
                actions = actions.push(
                    quiet(text("Pair").size(11)).on_press(Message::NearbyPair(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Accept").size(11))
                        .on_press(Message::NearbyAcceptPair(id.clone())),
                );
            } else {
                actions = actions.push(
                    quiet(text("Ping").size(11)).on_press(Message::NearbyPing(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Ring").size(11)).on_press(Message::NearbyRing(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Clip").size(11))
                        .on_press(Message::NearbyShareClipboard(id.clone())),
                );
                actions = actions.push(
                    quiet(text("File").size(11)).on_press(Message::NearbyShareFile(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Media").size(11)).on_press(Message::NearbyMedia(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Lock").size(11)).on_press(Message::NearbyLock(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Key").size(11)).on_press(Message::NearbySendKeys(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Present").size(11)).on_press(Message::NearbyPresenter(id.clone())),
                );
                actions = actions.push(
                    quiet(text("Unpair").size(11)).on_press(Message::NearbyUnpair(id.clone())),
                );
            }
            list = list.push(flyout_inset(
                column![
                    row![
                        column![
                            text(d.name.clone()).size(13),
                            text(format!("{state} · {}", d.id)).size(10),
                        ]
                        .spacing(2)
                        .width(Fill),
                    ]
                    .spacing(8),
                    actions,
                ]
                .spacing(6)
                .width(Fill),
                &self.palette,
            ));
        }

        list = list.push(
            quiet(text("Nearby settings…").size(12))
                .on_press(Message::OpenControl("nearby".into()))
                .padding([6, 4]),
        );
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }
}

impl Shell {
    /// A037: QS Bluetooth device list + pair wizard.
    pub(super) fn view_bt_flyout(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("Bluetooth").size(15),
                Space::with_width(Fill),
                quiet(text("Pair new").size(12)).on_press(Message::BtStartPair),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(if self.bt_pairing {
                "Scanning for devices…"
            } else {
                "Known devices · tap Connect / Disconnect"
            })
            .size(11),
            hairline(&self.palette),
        ]
        .spacing(8);
        if self.bt_devices.is_empty() {
            list = list.push(text("No devices (bluetoothctl)").size(13));
        }
        for dev in &self.bt_devices {
            let mac = dev.mac.clone();
            let mac2 = dev.mac.clone();
            let action = if self.bt_pairing && !dev.connected {
                quiet(text("Pair").size(12)).on_press(Message::BtPairDevice(mac))
            } else if dev.connected {
                quiet(text("Disconnect").size(12)).on_press(Message::BtDisconnect(mac))
            } else {
                quiet(text("Connect").size(12)).on_press(Message::BtConnect(mac2))
            };
            list = list.push(flyout_inset(
                row![
                    column![
                        text(dev.name.clone()).size(13),
                        text(dev.mac.clone()).size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    action,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                &self.palette,
            ));
        }
        list = list.push(
            quiet(text("More Bluetooth settings…").size(12))
                .on_press(Message::OpenControl("bluetooth".into()))
                .padding([6, 4]),
        );
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// A019: color picker history.
    pub(super) fn view_color_history(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("Color picker").size(15),
                Space::with_width(Fill),
                quiet(text("Pick…").size(12)).on_press(Message::OpenColorPickerHistory),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text("Recent colours · copy hex / rgb / hsl").size(11),
            hairline(&self.palette),
        ]
        .spacing(8);
        // Re-open picker via colorpick shell cmd effect: use a dedicated message path
        // by toggling color history closed and spawning picker through OpenControl stub —
        // instead write a synthetic status spawn: use Message from existing ColorPick path.
        if self.color_history.is_empty() {
            list = list.push(text("No colours yet · Super+Shift+C to pick").size(13));
        }
        for c in &self.color_history {
            let hex = c.hex.clone();
            let rgb = c.rgb.clone();
            let hsl = c.hsl.clone();
            list = list.push(flyout_inset(
                column![
                    text(c.hex.clone()).size(14),
                    row![
                        quiet(text("Hex").size(11))
                            .on_press(Message::CopyColorSample(hex))
                            .padding([2, 6]),
                        quiet(text("RGB").size(11))
                            .on_press(Message::CopyColorSample(rgb))
                            .padding([2, 6]),
                        quiet(text("HSL").size(11))
                            .on_press(Message::CopyColorSample(hsl))
                            .padding([2, 6]),
                    ]
                    .spacing(6),
                ]
                .spacing(4),
                &self.palette,
            ));
        }
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// A017: Snap Groups restore list.
    pub(super) fn view_snap_groups(&self) -> Element<'_, Message> {
        let mut list = column![
            row![
                text("Snap Groups").size(15),
                Space::with_width(Fill),
                quiet(text("Save").size(12)).on_press(Message::SaveCurrentSnapGroup),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text("Restore a saved snapped layout").size(11),
            hairline(&self.palette),
        ]
        .spacing(8);
        if self.snap_groups.is_empty() {
            list = list.push(text("No groups yet · snap windows then Save").size(13));
        }
        for g in &self.snap_groups {
            let id = g.id.clone();
            list = list.push(flyout_inset(
                row![
                    column![
                        text(g.label.clone()).size(13),
                        text(format!("{} windows", g.members.len())).size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    quiet(text("Restore").size(12)).on_press(Message::RestoreSnapGroup(id)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                &self.palette,
            ));
        }
        container(scrollable(list).height(Fill))
            .padding([14, 16])
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// A024: Find My Mouse spotlight overlay.
    pub(super) fn view_find_mouse(&self) -> Element<'_, Message> {
        container(
            column![
                text("●").size(64),
                text("Find My Mouse").size(18),
                text("Move the pointer — spotlight fades shortly").size(12),
            ]
            .spacing(8)
            .align_x(iced::Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .into()
    }
}
