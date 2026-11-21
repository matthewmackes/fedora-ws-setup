//! Widgets board — quiet defaults + per-card toggles + third-party JSON cards (A043).
use super::*;
use iced::widget::column;
use std::path::PathBuf;

#[derive(Clone)]
struct ThirdPartyCard {
    title: String,
    body: String,
}

fn load_third_party_cards() -> Vec<ThirdPartyCard> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let dir = PathBuf::from(home).join(".local/share/mde/widgets");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    for ent in rd.flatten().take(8) {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Minimal JSON: {"title":"…","body":"…"} — no serde_json dep; tiny scan.
        let title = json_string_field(&text, "title").unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Widget".into())
        });
        let body = json_string_field(&text, "body").unwrap_or_default();
        cards.push(ThirdPartyCard { title, body });
    }
    cards
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = text.find(&needle)?;
    let after = &text[idx + needle.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest.chars().skip(1);
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
            continue;
        }
        if c == '"' {
            break;
        }
        out.push(c);
    }
    Some(out)
}

impl Shell {
    pub(super) fn view_widgets(&self) -> Element<'_, Message> {
        let w = &self.cfg.active().widgets;
        let empty = || Space::with_height(0).into();

        let weather: Element<'_, Message> = if w.weather {
            flyout_inset(
                column![
                    row![
                        text("Weather").size(13),
                        Space::with_width(Fill),
                        quiet(text("Refresh").size(11)).on_press(Message::RefreshWeather),
                    ]
                    .spacing(8),
                    text(&self.weather.temp_c).size(28),
                    text(&self.weather.summary).size(12),
                    text(&self.weather.place).size(11),
                    text(widgets::updated_ago(self.weather.fetched_at)).size(10),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        let tz_row = widgets::timezone_row_label();
        let clock: Element<'_, Message> = if w.clock {
            flyout_inset(
                column![
                    text("Clock").size(13),
                    text(clock_time()).size(28),
                    text(clock_long_date()).size(12),
                    text(tz_row)
                        .size(10)
                        .color(Color::from_rgba(0.5, 0.5, 0.5, 1.0)),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        // A031: calendar mini is month/day label only (no agenda list).
        let calendar_mini: Element<'_, Message> = if w.calendar {
            let day = self.cal_selected.unwrap_or_else(|| {
                let now = mde_theme::now();
                chrono::Datelike::day(&now)
            });
            flyout_inset(
                column![
                    text("Calendar").size(13),
                    text(format!("{day}")).size(28),
                    text(clock_long_date()).size(11),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        let system: Element<'_, Message> = if w.system {
            flyout_inset(
                column![
                    text("System").size(13),
                    text(&self.system_card.cpu_pct).size(18),
                    text(&self.system_card.memory).size(11),
                    text(&self.system_card.load).size(11),
                    text(widgets::updated_ago(self.system_card.fetched_at)).size(10),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        let tip_opt_in = self.cfg.active().start.tips_opt_in;
        let tip: Element<'_, Message> = if w.tip && tip_opt_in {
            flyout_inset(
                column![
                    text("Tip").size(13),
                    text(widgets::tip_of_day()).size(12),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        let news: Element<'_, Message> = if w.news {
            let mut news_col = column![
                row![
                    text("News").size(13),
                    Space::with_width(Fill),
                    quiet(text("Refresh").size(11)).on_press(Message::RefreshNews),
                ]
                .spacing(8),
            ]
            .spacing(2);
            if !self.news_card.error.is_empty() && self.news_card.headlines.is_empty() {
                news_col = news_col.push(text(&self.news_card.error).size(11));
            } else if self.news_card.headlines.is_empty() {
                news_col = news_col.push(text("No headlines yet").size(11));
            } else {
                for (title, link) in self.news_card.headlines.iter().take(5) {
                    let label = text(title.clone()).size(11);
                    news_col = if link.is_empty() {
                        news_col.push(label)
                    } else {
                        news_col.push(
                            quiet(label)
                                .on_press(Message::OpenNewsLink(link.clone()))
                                .padding([1, 0]),
                        )
                    };
                }
                news_col = news_col.push(
                    text(widgets::updated_ago(self.news_card.fetched_at)).size(10),
                );
            }
            flyout_inset(news_col.width(Fill), &self.palette)
        } else {
            empty()
        };

        let markets: Element<'_, Message> = if w.markets {
            let mut markets_col = column![
                row![
                    text("Markets").size(13),
                    Space::with_width(Fill),
                    quiet(text("Refresh").size(11)).on_press(Message::RefreshMarkets),
                ]
                .spacing(8),
            ]
            .spacing(2);
            if !self.markets_card.error.is_empty() && self.markets_card.rows.is_empty() {
                markets_col = markets_col.push(text(&self.markets_card.error).size(11));
            } else if self.markets_card.rows.is_empty() {
                markets_col = markets_col.push(text("No quotes yet").size(11));
            } else {
                for (sym, price) in self.markets_card.rows.iter().take(6) {
                    markets_col = markets_col.push(
                        row![
                            text(sym.clone()).size(11),
                            Space::with_width(Fill),
                            text(price.clone()).size(11),
                        ]
                        .spacing(4),
                    );
                }
                markets_col = markets_col.push(
                    text(widgets::updated_ago(self.markets_card.fetched_at)).size(10),
                );
            }
            flyout_inset(markets_col.width(Fill), &self.palette)
        } else {
            empty()
        };

        let traffic: Element<'_, Message> = if w.traffic {
            let mut traffic_col = column![
                row![
                    text("Traffic").size(13),
                    Space::with_width(Fill),
                    quiet(text("Refresh").size(11)).on_press(Message::RefreshTraffic),
                ]
                .spacing(8),
            ]
            .spacing(2);
            if self.traffic_card.error == "not configured" {
                traffic_col = traffic_col
                    .push(text(&self.traffic_card.eta).size(22))
                    .push(text(&self.traffic_card.detail).size(10));
            } else if !self.traffic_card.error.is_empty()
                && (self.traffic_card.eta.is_empty() || self.traffic_card.eta == "—")
            {
                traffic_col = traffic_col.push(text(&self.traffic_card.error).size(11));
            } else {
                if !self.traffic_card.label.is_empty() {
                    traffic_col = traffic_col.push(text(&self.traffic_card.label).size(11));
                }
                traffic_col = traffic_col
                    .push(text(&self.traffic_card.eta).size(22))
                    .push(text(&self.traffic_card.detail).size(11))
                    .push(text(widgets::updated_ago(self.traffic_card.fetched_at)).size(10));
            }
            flyout_inset(traffic_col.width(Fill), &self.palette)
        } else {
            empty()
        };

        let background: Element<'_, Message> = if w.background {
            let wall = self.cfg.active().theme.wallpaper.clone();
            let wall_label = if wall.trim().is_empty() {
                "Not set · open Personalization".into()
            } else {
                wall.chars()
                    .rev()
                    .take(42)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            };
            flyout_inset(
                column![
                    text("Background").size(13),
                    text(wall_label).size(11),
                    quiet(text("Change wallpaper…").size(12))
                        .on_press(Message::OpenBackground)
                        .padding([2, 0]),
                ]
                .spacing(2)
                .width(Fill),
                &self.palette,
            )
        } else {
            empty()
        };

        let photos: Element<'_, Message> = if w.photos {
            match self.photos_path.clone().or_else(widgets::random_picture) {
                Some(path) => {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    flyout_inset(
                        quiet(
                            column![
                                text("Photos").size(13),
                                image(path.clone()).width(Fill).height(72),
                                text(name).size(10),
                                text("Click for next").size(10),
                            ]
                            .spacing(2)
                            .width(Fill),
                        )
                        .on_press(Message::CyclePhotos)
                        .width(Fill),
                        &self.palette,
                    )
                }
                None => flyout_inset(
                    column![
                        text("Photos").size(13),
                        text("No images in ~/Pictures").size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    &self.palette,
                ),
            }
        } else {
            empty()
        };

        let mut board = column![
            row![
                text("Widgets").size(16),
                Space::with_width(Fill),
                quiet(text("Close").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(6);

        if w.weather || w.clock {
            board = board.push(row![weather, clock].spacing(8));
        }
        if w.calendar || w.system {
            board = board.push(row![calendar_mini, system].spacing(8));
        }
        if w.tip && tip_opt_in {
            board = board.push(tip);
        }
        if w.photos || w.background {
            board = board.push(row![photos, background].spacing(8));
        }
        if w.news || w.markets {
            board = board.push(row![news, markets].spacing(8));
        }
        if w.traffic {
            board = board.push(traffic);
        }

        if w.third_party {
            for card in load_third_party_cards() {
                board = board.push(flyout_inset(
                    column![
                        text(card.title).size(13),
                        text(card.body).size(11),
                    ]
                    .spacing(2)
                    .width(Fill),
                    &self.palette,
                ));
            }
        }

        board
            .padding([12, 12])
            .width(Fill)
            .height(Fill)
            .into()
    }
}
