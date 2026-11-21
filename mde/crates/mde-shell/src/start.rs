//! Surface views moved from main.rs. Behavior is unchanged.
use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn view_search_type_chips(&self) -> Element<'_, Message> {
        let chip = |id: &str, label: &str| {
            let on = self.search_type == id;
            let label = if on {
                format!("[{label}]")
            } else {
                label.to_string()
            };
            quiet(text(label).size(11))
                .on_press(Message::SetSearchType(id.to_string()))
                .padding([4, 8])
        };
        // A084: local-only search — Web chip removed.
        row![
            chip("apps", "Apps"),
            chip("documents", "Documents"),
        ]
        .spacing(6)
        .into()
    }

    fn menu_line(label: &str, message: Message) -> Element<'static, Message> {
        quiet(text(label.to_string()).size(13))
            .on_press(message)
            .width(Fill)
            .padding([8, 10])
            .into()
    }

    /// Account button on the left, power button on the right. Menus open above that row.
    pub(super) fn view_start_footer(&self) -> Element<'_, Message> {
        let user = std::env::var("USER").unwrap_or_else(|_| "User".into());
        let mut col = column![].spacing(6).width(Fill);

        if self.account_menu_open {
            let menu = column![
                Self::menu_line("Change picture", Message::AccountPictureStub),
                Self::menu_line("Sign in options", Message::AccountSignInStub),
                Self::menu_line("Lock", Message::Power(PowerAction::Lock)),
                // A057: single-user lock — Switch user signs out to greeter (not multi-seat).
                Self::menu_line("Sign out to switch user", Message::SwitchUser),
                Self::menu_line("Sign out", Message::Power(PowerAction::SignOut)),
            ]
            .spacing(2);
            col = col.push(row![container(menu).width(260), Space::with_width(Fill)].width(Fill));
        }

        if self.power_menu_open || self.power_confirm.is_some() {
            let menu: Element<'_, Message> = if let Some(pending) = self.power_confirm {
                let prompt = match pending {
                    PowerAction::Suspend => "Sleep now?",
                    PowerAction::Reboot => "Restart now?",
                    PowerAction::PowerOff => "Shut down now?",
                    _ => "Confirm?",
                };
                column![
                    text(prompt).size(13),
                    row![
                        quiet(text("Yes").size(13))
                            .on_press(Message::ConfirmPower)
                            .padding([6, 12]),
                        quiet(text("No").size(13))
                            .on_press(Message::CancelPowerConfirm)
                            .padding([6, 12]),
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .into()
            } else {
                // Win11 order when offline updates are prepared: Update actions first.
                let mut lines = column![].spacing(2);
                if crate::updates_busy() {
                    lines = lines.push(Self::menu_line(
                        "Updates in progress…",
                        Message::Dismiss,
                    ));
                } else {
                    if crate::offline_update_pending() {
                        lines = lines.push(Self::menu_line(
                            "Update and restart",
                            Message::UpdateAndRestart,
                        ));
                        lines = lines.push(Self::menu_line(
                            "Update and shut down",
                            Message::UpdateAndShutDown,
                        ));
                    }
                    lines = lines.push(Self::menu_line(
                        "Sleep",
                        Message::Power(PowerAction::Suspend),
                    ));
                    lines = lines.push(Self::menu_line(
                        "Hibernate",
                        Message::Power(PowerAction::Hibernate),
                    ));
                    lines = lines.push(Self::menu_line(
                        "Shut down",
                        Message::Power(PowerAction::PowerOff),
                    ));
                    lines = lines.push(Self::menu_line(
                        "Restart",
                        Message::Power(PowerAction::Reboot),
                    ));
                }
                lines.into()
            };
            col = col.push(row![Space::with_width(Fill), container(menu).width(240)].width(Fill));
        }

        let power_style = if self.power_menu_open || self.power_confirm.is_some() {
            accent_pill
        } else {
            glass_button
        };
        // W451: folder glyphs left of power from `start.folders` Settings toggles.
        let mut folder_row = row![].spacing(2);
        let folders = &self.cfg.active().start.folders;
        // A011: Win11 checklist parity — Settings + folders beside power.
        let folder_btns: &[(&str, &str, &str)] = &[
            ("settings", "preferences-system", "Settings"),
            ("documents", "folder-documents", "Documents"),
            ("downloads", "folder-download", "Downloads"),
            ("music", "folder-music", "Music"),
            ("pictures", "folder-pictures", "Pictures"),
            ("videos", "folder-videos", "Videos"),
            ("home", "user-home", "Home"),
            ("desktop", "user-desktop", "Desktop"),
            ("network", "network-workgroup", "Network"),
            ("explorer", "system-file-manager", "File Explorer"),
        ];
        for (id, icon, tip) in folder_btns {
            if folders.iter().any(|f| f.eq_ignore_ascii_case(id)) {
                let kind = (*id).to_string();
                let msg = match *id {
                    "explorer" => Message::OpenFileExplorer,
                    "settings" => Message::OpenControl(String::new()),
                    "network" => Message::OpenControl("network".into()),
                    _ => Message::OpenUserFolder(kind),
                };
                folder_row = folder_row.push(
                    chrome_btn(icon, tip, 16, self.palette.dark).on_press(msg),
                );
            }
        }

        col = col.push(
            row![
                quiet(
                    row![
                        app_face("system-users", user.clone(), 18, false, self.palette.dark),
                        text(user).size(13),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::ToggleAccountMenu)
                .padding([4, 8]),
                Space::with_width(Fill),
                folder_row,
                chrome_btn("system-shutdown", "Power", 18, self.palette.dark)
                    .on_press(Message::TogglePowerMenu)
                    .style(power_style),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .width(Fill),
        );
        col.into()
    }

    /// Search stays pinned. The list scrolls. Account and power stay on the bottom edge.
    pub(super) fn start_frame<'a>(
        &'a self,
        search: Element<'a, Message>,
        body: Element<'a, Message>,
    ) -> Element<'a, Message> {
        container(
            column![
                search,
                scrollable(body).height(Fill),
                hairline(&self.palette),
                self.view_start_footer(),
            ]
            .spacing(10)
            .padding([16, START_PAD_X])
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// W264: pin ~/Documents (or XDG) path string into start.pins.

    /// W461: taskbar Search — providers without Pinned home chrome.
    pub(super) fn view_search_surface(&self) -> Element<'_, Message> {
        let q = self.start_query.to_lowercase();
        if q.chars().count() > 2 {
            return self.view_start();
        }

        let search_flash = self
            .search_highlight_until
            .is_some_and(|u| Instant::now() < u);
        let accent_rgb = Palette::parse_rgb(&self.palette.accent)
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.0, 0.47, 0.83));
        let search_mic = chrome_btn("audio-input-microphone", "Mic", 16, self.palette.dark)
            .on_press(Message::VoiceSearchStub);
        let search = container(
            row![
                text_input("Type here to search", &self.start_query)
                    .id(start_search_id())
                    .on_input(Message::StartQuery)
                    .padding(12)
                    .size(14),
                search_mic,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .style(move |_| {
            if search_flash {
                iced::widget::container::Style {
                    border: Border {
                        color: accent_rgb,
                        width: 2.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            } else {
                iced::widget::container::Style::default()
            }
        });

        let mut body = column![
            self.view_search_type_chips(),
            section_label("Quick"),
            row![
                quiet(text("Settings").size(12))
                    .on_press(Message::OpenControl(String::new()))
                    .padding([6, 8]),
                quiet(text("All apps").size(12))
                    .on_press(Message::ToggleStart)
                    .padding([6, 8]),
            ]
            .spacing(6),
            section_label("Libraries"),
            row![
                quiet(text("Photos").size(12))
                    .on_press(Message::OpenUserFolder("pictures".into()))
                    .padding([6, 8]),
                quiet(text("Music").size(12))
                    .on_press(Message::OpenUserFolder("music".into()))
                    .padding([6, 8]),
                quiet(text("Documents").size(12))
                    .on_press(Message::OpenUserFolder("documents".into()))
                    .padding([6, 8]),
                quiet(text("Videos").size(12))
                    .on_press(Message::OpenUserFolder("videos".into()))
                    .padding([6, 8]),
            ]
            .spacing(6),
        ]
        .spacing(10)
        .width(Fill);

        if !self.recent_settings.is_empty() {
            body = body.push(section_label("Recent settings"));
            for page in &self.recent_settings {
                let label = settings_page_label(page);
                let page_id = page.clone();
                body = body.push(
                    quiet(text(label).size(12))
                        .on_press(Message::OpenControl(page_id))
                        .width(Fill)
                        .padding([4, 8]),
                );
            }
        }

        self.start_frame(search.into(), body.into())
    }

    pub(super) fn view_start(&self) -> Element<'_, Message> {
        let profile = self.cfg.active();
        let q = self.start_query.to_lowercase();

        let search_flash = self
            .search_highlight_until
            .is_some_and(|u| Instant::now() < u);
        let accent_rgb = Palette::parse_rgb(&self.palette.accent)
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.0, 0.47, 0.83));
        // W273: mic icon → Voice search · coming soon.
        let search_mic = chrome_btn("audio-input-microphone", "Mic", 16, self.palette.dark)
            .on_press(Message::VoiceSearchStub);
        let search = container(
            row![
                text_input("Search for apps, settings, and documents", &self.start_query)
                    .id(start_search_id())
                    .on_input(Message::StartQuery)
                    .padding(12)
                    .size(14),
                search_mic,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .style(move |_| {
            // W186: one-shot accent border when opened from taskbar search pill.
            if search_flash {
                iced::widget::container::Style {
                    border: Border {
                        color: accent_rgb,
                        width: 2.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            } else {
                iced::widget::container::Style::default()
            }
        });

        if self.start_all_apps {
            let advanced = &self.cfg.active().start;
            // W144: sparse A–Z jump letters present in the installed list.
            let mut letter_set = [false; 26];
            for app in &self.apps {
                if advanced.is_advanced_app(&app.id) {
                    continue;
                }
                if let Some(c) = app
                    .name
                    .chars()
                    .find(|ch| ch.is_ascii_alphabetic())
                    .map(|ch| ch.to_ascii_uppercase())
                {
                    let idx = (c as u8 - b'A') as usize;
                    if idx < 26 {
                        letter_set[idx] = true;
                    }
                }
            }
            let mut jump_row = row![].spacing(2);
            for i in 0u8..26 {
                if !letter_set[i as usize] {
                    continue;
                }
                let ch = (b'A' + i) as char;
                let active = self.all_apps_jump == Some(ch);
                let btn = quiet(text(ch.to_string()).size(11))
                    .on_press(Message::AllAppsJump(ch))
                    .padding([2, 4]);
                jump_row = jump_row.push(if active {
                    btn.style(accent_pill)
                } else {
                    btn
                });
            }
            // W455: Recently added (by .desktop mtime) when Settings toggle is on.
            let mut list = column![].spacing(2);
            if self.cfg.active().start.recently_added && self.all_apps_jump.is_none() {
                let mut recent_apps: Vec<&apps::AppEntry> = self.apps.iter().collect();
                recent_apps.sort_by(|a, b| {
                    let ma = apps::resolve_desktop(&a.id)
                        .and_then(|p| std::fs::metadata(&p).ok())
                        .and_then(|m| m.modified().ok());
                    let mb = apps::resolve_desktop(&b.id)
                        .and_then(|p| std::fs::metadata(&p).ok())
                        .and_then(|m| m.modified().ok());
                    mb.cmp(&ma)
                });
                let mut shown = 0usize;
                let mut rec_col = column![section_label("Recently added")].spacing(2);
                for app in recent_apps.into_iter() {
                    if advanced.is_advanced_app(&app.id) {
                        continue;
                    }
                    if !q.is_empty()
                        && !app.name.to_lowercase().contains(&q)
                        && !app.id.to_lowercase().contains(&q)
                    {
                        continue;
                    }
                    if shown >= 8 {
                        break;
                    }
                    let id = app.id.clone();
                    rec_col = rec_col.push(
                        mouse_area(
                            quiet(app_face(&app.icon, app.name.clone(), 22, true, self.palette.dark))
                                .on_press(Message::LaunchPin(id.clone()))
                                .width(Fill)
                                .padding([8, 10]),
                        )
                        .on_right_press(Message::OpenAppCtx {
                            kind: AppCtxKind::AllApps,
                            id,
                        }),
                    );
                    shown += 1;
                }
                if shown > 0 {
                    list = list.push(rec_col).push(Space::with_height(8));
                }
            }
            let mut count = 0usize;
            for app in &self.apps {
                if advanced.is_advanced_app(&app.id) {
                    continue;
                }
                if !q.is_empty()
                    && !app.name.to_lowercase().contains(&q)
                    && !app.id.to_lowercase().contains(&q)
                {
                    continue;
                }
                if let Some(jump) = self.all_apps_jump {
                    let starts = app
                        .name
                        .chars()
                        .find(|ch| ch.is_ascii_alphabetic())
                        .map(|ch| ch.to_ascii_uppercase() == jump)
                        .unwrap_or(false);
                    if !starts {
                        continue;
                    }
                }
                // W384: right-click → All apps context menu (not one-shot pin).
                let id = app.id.clone();
                let already = self.cfg.active().shell.pins.iter().any(|p| p == &app.id);
                let face = quiet(app_face(&app.icon, app.name.clone(), 22, true, self.palette.dark))
                    .on_press(Message::LaunchPin(id.clone()))
                    .width(Fill)
                    .padding([8, 10]);
                let row_el: Element<'_, Message> = row![
                    mouse_area(face).on_right_press(Message::OpenAppCtx {
                        kind: AppCtxKind::AllApps,
                        id: id.clone(),
                    }),
                    if already {
                        text("Pinned").size(11)
                    } else {
                        text("").size(11)
                    },
                    quiet(text("Pin").size(11))
                        .on_press(Message::PinToTaskbar(id))
                        .padding([4, 8]),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into();
                list = list.push(row_el);
                count += 1;
            }
            if count == 0 {
                list = list.push(text("No matching apps").size(13));
            }
            let body = column![
                jump_row,
                row![
                    quiet(text("< Back").size(12))
                        .on_press(Message::AllAppsBack)
                        .padding([4, 8]),
                    section_label("All apps"),
                    Space::with_width(Fill),
                    text(format!("{count}")).size(11),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                list,
            ]
            .spacing(8)
            .width(Fill);
            return self.start_frame(search.into(), body.into());
        }

        // W135: Settings category chips when query empty or short (≤2).
        // W235: recent OpenControl pages (≤3) under Settings when Start opens.
        let settings_chips: Element<'_, Message> = if q.chars().count() <= 2 {
            let mut settings_col = column![
                section_label("Settings"),
                row![
                    quiet(text("System").size(11))
                        .on_press(Message::OpenControl("system".into()))
                        .padding([4, 8]),
                    quiet(text("Personalization").size(11))
                        .on_press(Message::OpenControl("personalization".into()))
                        .padding([4, 8]),
                    quiet(text("Network").size(11))
                        .on_press(Message::OpenControl("network".into()))
                        .padding([4, 8]),
                    quiet(text("Accounts").size(11))
                        .on_press(Message::OpenControl("accounts".into()))
                        .padding([4, 8]),
                ]
                .spacing(6),
            ]
            .spacing(4);
            if !self.recent_settings.is_empty() {
                settings_col = settings_col.push(text("Recent").size(11));
                for page in &self.recent_settings {
                    let label = settings_page_label(page);
                    let page_id = page.clone();
                    settings_col = settings_col.push(
                        quiet(text(label).size(12))
                            .on_press(Message::OpenControl(page_id))
                            .width(Fill)
                            .padding([4, 8]),
                    );
                }
            }
            settings_col.into()
        } else {
            Space::with_height(0).into()
        };

        // Win11-like ranked search sections when the query is longer (W19 / W55).
        // Short queries stay on the home Start layout with W135 chips.
        if q.chars().count() > 2 {
            let mut apps_col = column![section_label("Apps")].spacing(2);
            let mut app_n = 0usize;
            for pin in profile.start.pins.iter().chain(profile.shell.pins.iter()) {
                if app_n >= 8 {
                    break;
                }
                if profile.start.pin_is_advanced(pin) {
                    continue;
                }
                let label = apps::resolve_desktop(pin)
                    .map(|p| apps::desktop_name(&p))
                    .unwrap_or_else(|| pin.clone());
                if !(pin.to_lowercase().contains(&q) || label.to_lowercase().contains(&q)) {
                    continue;
                }
                let icon = apps::desktop_icon(pin).unwrap_or_default();
                let shown = highlight_match(&label, &q);
                apps_col = apps_col.push(
                    quiet(app_face(&icon, shown, 22, true, self.palette.dark))
                        .on_press(Message::LaunchPin(pin.clone()))
                        .width(Fill)
                        .padding([8, 10]),
                );
                app_n += 1;
            }
            for app in &self.apps {
                if app_n >= 10 {
                    break;
                }
                if profile.start.is_advanced_app(&app.id) {
                    continue;
                }
                if !app.name.to_lowercase().contains(&q) && !app.id.to_lowercase().contains(&q) {
                    continue;
                }
                if profile
                    .start
                    .pins
                    .iter()
                    .chain(profile.shell.pins.iter())
                    .any(|p| p == &app.id)
                {
                    continue;
                }
                let shown = highlight_match(&app.name, &q);
                apps_col = apps_col.push(
                    quiet(app_face(&app.icon, shown, 22, true, self.palette.dark))
                        .on_press(Message::LaunchPin(app.id.clone()))
                        .width(Fill)
                        .padding([8, 10]),
                );
                app_n += 1;
            }

            let mut files_col = column![section_label("Files")].spacing(2);
            let file_n = self.file_hits.len();
            for path in &self.file_hits {
                let label = std::path::Path::new(path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                let shown = highlight_match(&label, &q);
                files_col = files_col.push(
                    quiet(text(shown).size(13))
                        .on_press(Message::OpenFile(path.clone()))
                        .width(Fill)
                        .padding([6, 10]),
                );
            }
            // W406: content search under filename hits.
            let content_hits = files::search_file_contents(&q, 5);
            if !content_hits.is_empty() {
                files_col = files_col.push(
                    text("In file contents")
                        .size(11)
                        .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
                );
                for path in content_hits {
                    let label = std::path::Path::new(&path)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    let shown = highlight_match(&label, &q);
                    files_col = files_col.push(
                        quiet(text(shown).size(13))
                            .on_press(Message::OpenFile(path))
                            .width(Fill)
                            .padding([6, 10]),
                    );
                }
            }

            const SETTINGS_PAGES: &[(&str, &str)] = &[
                ("settings", "Open Settings"),
                ("system", "System"),
                ("display", "Display"),
                ("sound", "Sound"),
                ("devices", "Bluetooth & devices"),
                ("bluetooth", "Bluetooth"),
                ("network", "Network & internet"),
                ("wifi", "Wi-Fi"),
                ("personalization", "Personalization"),
                ("apps", "Apps"),
                ("accounts", "Accounts"),
                ("time", "Time & language"),
                ("gaming", "Gaming"),
                ("accessibility", "Accessibility"),
                ("privacy", "Privacy & security"),
                ("update", "Windows Update"),
                ("advanced", "Advanced"),
                ("night", "Night light"),
            ];
            let mut settings_col = column![section_label("Settings")].spacing(2);
            let mut set_n = 0usize;
            for (key, label) in SETTINGS_PAGES {
                if key.contains(q.as_str()) || label.to_lowercase().contains(&q) {
                    let shown = highlight_match(label, &q);
                    let page = settings_page_id(key).to_string();
                    settings_col = settings_col.push(
                        quiet(text(shown).size(13))
                            .on_press(Message::OpenControl(page))
                            .width(Fill)
                            .padding([6, 10]),
                    );
                    set_n += 1;
                }
            }

            let total_hits = app_n + file_n + set_n;
            // W175: Start search math calculator (safe tiny parser).
            let math_row: Element<'_, Message> =
                if let Some(v) = try_eval_math(self.start_query.trim()) {
                    text(format_math_result(v)).size(18).into()
                } else {
                    Space::with_height(0).into()
                };
            // W215: session Calculator history (last ≤3 results).
            let calc_hist: Element<'_, Message> = if self.calc_history.is_empty() {
                Space::with_height(0).into()
            } else {
                let mut col = column![section_label("Calculator")].spacing(2);
                for line in &self.calc_history {
                    col = col.push(text(line.clone()).size(12));
                }
                col.width(Fill).into()
            };
            // A084: local-only Start search (no web chip / DuckDuckGo).
            // W255: type chips filter which sections show.
            let show_apps = self.search_shows("apps");
            let show_docs = self.search_shows("documents");
            let show_settings = self.search_type.is_empty();
            let type_chips = self.view_search_type_chips();
            // W195: emoji shortcode / single emoji → tip to open emoji panel.
            let emoji_row: Element<'_, Message> = if query_wants_emoji(&self.start_query) {
                quiet(text("Emoji panel · Super+. · Shell emoji").size(12))
                    .on_press(Message::OpenEmojiPanel)
                    .width(Fill)
                    .padding([6, 10])
                    .into()
            } else {
                Space::with_height(0).into()
            };
            let has_math = try_eval_math(self.start_query.trim()).is_some();
            let filtered_hits = match self.search_type.as_str() {
                "apps" => app_n,
                "documents" => file_n,
                _ => total_hits,
            };
            let results: Element<'_, Message> = if filtered_hits == 0 && has_math && show_apps {
                column![
                    math_row,
                    calc_hist,
                    Space::with_height(8),
                    emoji_row,
                ]
                .spacing(6)
                .width(Fill)
                .into()
            } else if filtered_hits == 0 {
                column![
                    math_row,
                    calc_hist,
                    Space::with_height(24),
                    text("No results").size(18),
                    text(format!("Nothing matched “{}”", self.start_query.trim())).size(13),
                    text("Try another name, or open Settings / All apps.").size(12),
                    Space::with_height(12),
                    row![
                        quiet(text("Open Settings").size(13))
                            .on_press(Message::OpenControl(String::new()))
                            .padding([8, 12]),
                        quiet(text("All apps").size(13))
                            .on_press(Message::ToggleAllApps)
                            .padding([8, 12]),
                    ]
                    .spacing(8),
                    Space::with_height(8),
                    emoji_row,
                ]
                .spacing(6)
                .align_x(iced::Alignment::Center)
                .width(Fill)
                .into()
            } else {
                if show_apps && app_n == 0 {
                    apps_col = apps_col.push(text("No matching apps").size(12));
                }
                if show_docs && file_n == 0 {
                    files_col = files_col.push(text("No matching files").size(12));
                }
                if show_settings && set_n == 0 {
                    settings_col = settings_col.push(
                        quiet(text("Open Settings").size(13))
                            .on_press(Message::OpenControl(String::new()))
                            .width(Fill)
                            .padding([6, 10]),
                    );
                }
                let mut col = column![math_row, calc_hist].spacing(0).width(Fill);
                if show_apps {
                    col = col.push(apps_col).push(Space::with_height(10));
                }
                if show_docs {
                    col = col.push(files_col).push(Space::with_height(10));
                }
                if show_settings {
                    col = col.push(settings_col).push(Space::with_height(10));
                }
                col = col.push(emoji_row);
                col.into()
            };

            let body = column![type_chips, results].spacing(10).width(Fill);
            return self.start_frame(search.into(), body.into());
        }

        // W454: Pinned grid = start.pins only (not taskbar pins).
        // Advanced membership hides pins/folders from Start (Settings › Advanced).
        let mut pin_ids: Vec<String> = Vec::new();
        for pin in &profile.start.pins {
            if profile.start.pin_is_advanced(pin) {
                continue;
            }
            if pin_ids.iter().any(|p| p == pin) {
                continue;
            }
            let label = apps::resolve_desktop(pin)
                .map(|p| apps::desktop_name(&p))
                .unwrap_or_else(|| pin.clone());
            if q.is_empty()
                || pin.to_lowercase().contains(&q)
                || label.to_lowercase().contains(&q)
            {
                pin_ids.push(pin.clone());
            }
        }

        // W450: layout density — default vs more_pins (no Recommended mode).
        let pin_cap = self.start_pin_cap();
        let mut pinned_grid = column![].spacing(START_PIN_GAP);
        let mut row_el = row![].spacing(START_PIN_GAP);
        let cols = 6usize;
        let mut col_used = 0usize;
        for pin in pin_ids.iter().take(pin_cap) {
            let label = apps::resolve_desktop(pin)
                .map(|p| apps::desktop_name(&p))
                .unwrap_or_else(|| pin.clone());
            let icon = apps::desktop_icon(pin).unwrap_or_default();
            let large = self.start_pin_is_large(pin);
            let span = if large { 2 } else { 1 };
            if col_used > 0 && col_used + span > cols {
                pinned_grid = pinned_grid.push(row_el);
                row_el = row![].spacing(START_PIN_GAP);
                col_used = 0;
            }
            let is_folder = pin.starts_with("mde-folder:");
            let (icon, label) = if is_folder {
                let name = pin.strip_prefix("mde-folder:").unwrap_or(pin.as_str());
                ("folder".to_string(), name.to_string())
            } else {
                (icon, label)
            };
            let press = if is_folder {
                Message::ToggleStartFolderExpand(pin.clone())
            } else {
                Message::LaunchPin(pin.clone())
            };
            let tile = mouse_area(start_pin_tile(&icon, label, press, large || is_folder, self.palette.dark))
                .on_right_press(Message::OpenAppCtx {
                    kind: AppCtxKind::StartPin,
                    id: pin.clone(),
                })
                .on_enter(Message::StartPinEnter(pin.clone()))
                .on_exit(Message::StartPinExit);
            row_el = row_el.push(tile);
            col_used += span;
            if col_used >= cols {
                pinned_grid = pinned_grid.push(row_el);
                row_el = row![].spacing(START_PIN_GAP);
                col_used = 0;
            }
        }
        if pin_ids.is_empty() {
            pinned_grid = pinned_grid.push(text("Pin apps to see them here").size(13));
        } else if col_used != 0 {
            pinned_grid = pinned_grid.push(row_el);
        }

        // W195: emoji tip on home for short `:smile:` / single-emoji queries.
        let home_emoji: Element<'_, Message> = if query_wants_emoji(&self.start_query) {
            quiet(text("Emoji panel · Super+. · Shell emoji").size(12))
                .on_press(Message::OpenEmojiPanel)
                .width(Fill)
                .padding([4, 8])
                .into()
        } else {
            Space::with_height(0).into()
        };

        // W124: chevron collapses/expands pinned grid.
        let pin_chevron = if self.pins_collapsed { "▸" } else { "▾" };
        let mut pinned_section = column![
            row![
                quiet(text(format!("{pin_chevron} Pinned")).size(14))
                    .on_press(Message::TogglePinnedCollapse)
                    .padding([4, 4]),
                Space::with_width(Fill),
                quiet(text("All apps >").size(13))
                    .on_press(Message::ToggleAllApps)
                    .padding([4, 8]),
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(8);
        if !self.pins_collapsed {
            pinned_section = pinned_section.push(pinned_grid);
            if let Some(fid) = &self.start_folder_open {
                let fname = fid.strip_prefix("mde-folder:").unwrap_or(fid.as_str());
                let mut fold = column![text(format!("Folder · {fname}")).size(14)].spacing(4);
                for app_id in self.start_folder_apps(fid) {
                    let label = apps::resolve_desktop(&app_id)
                        .map(|p| apps::desktop_name(&p))
                        .unwrap_or_else(|| app_id.clone());
                    let icon = apps::desktop_icon(&app_id).unwrap_or_default();
                    fold = fold.push(
                        quiet(app_face(&icon, label, 22, true, self.palette.dark))
                            .on_press(Message::LaunchPin(app_id))
                            .width(Fill)
                            .padding([6, 8]),
                    );
                }
                pinned_section = pinned_section.push(fold);
            }
        }

        // A013: Classic vertical = All apps list left, pins right (StartAllBack-style).
        let classic = {
            let layout = self
                .cfg
                .active()
                .start
                .layout
                .to_lowercase()
                .replace(' ', "_")
                .replace('-', "_");
            layout == "classic" || layout == "vertical"
        };
        let body: Element<'_, Message> = if classic {
            let mut apps_col = column![text("All apps").size(14)].spacing(4);
            for app in self.apps.iter().take(24) {
                let icon = apps::desktop_icon(&app.id).unwrap_or_default();
                apps_col = apps_col.push(
                    quiet(app_face(&icon, app.name.clone(), 18, true, self.palette.dark))
                        .on_press(Message::LaunchPin(app.id.clone()))
                        .width(Fill)
                        .padding([4, 6]),
                );
            }
            column![
                home_emoji,
                row![
                    scrollable(apps_col).height(Fill).width(180),
                    Space::with_width(12),
                    column![pinned_section, settings_chips]
                        .spacing(12)
                        .width(Fill),
                ]
                .spacing(0)
                .width(Fill)
                .height(Fill),
            ]
            .spacing(8)
            .width(Fill)
            .into()
        } else {
            // W456/W463: home = search + Pinned + All apps + footer — no Recommended / More farm.
            column![home_emoji, pinned_section, settings_chips]
                .spacing(12)
                .width(Fill)
                .into()
        };
        self.start_frame(search.into(), body)
    }
}
