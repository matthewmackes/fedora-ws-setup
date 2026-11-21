//! Win11 Snipping Tool overlay + post-capture thumbnail toast (W515).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{column, container, image, mouse_area, row, text, Space};
use iced::{Background, Border, Color, Element, Fill, Length};
use mde_protocol::ScreenshotMode;

use super::*;

/// How long the thumbnail toast stays visible.
pub(crate) const SCREENSHOT_TOAST_SECS: u64 = 5;

#[derive(Debug, Clone)]
pub(crate) struct ScreenshotToast {
    pub path: PathBuf,
    pub until: Instant,
}

impl ScreenshotToast {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            until: Instant::now() + Duration::from_secs(SCREENSHOT_TOAST_SECS),
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.until
    }
}

fn mode_btn<'a>(
    label: &'a str,
    icon: &'a str,
    mode: ScreenshotMode,
    selected: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let fg = if selected {
        Color::WHITE
    } else {
        fg_color(palette)
    };
    let bg = if selected {
        Background::Color(accent_color(palette))
    } else {
        Background::Color(Color::TRANSPARENT)
    };
    let content = column![
        app_face(icon, String::new(), 22, false, palette.dark),
        text(label).size(11).color(fg),
    ]
    .spacing(4)
    .align_x(iced::Alignment::Center);
    container(
        quiet(content)
            .on_press(Message::SnipCapture(mode))
            .padding([10, 14]),
    )
    .style(move |_| ContainerStyle {
        background: Some(bg),
        border: Border {
            color: if selected {
                accent_color(palette)
            } else {
                Color::TRANSPARENT
            },
            width: 0.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}

impl Shell {
    /// Dimmed backdrop + top Win11-style snip toolbar.
    pub(super) fn view_snip_overlay(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let selected = self
            .snip_preselect
            .unwrap_or(ScreenshotMode::Area);
        let toolbar = container(
            row![
                mode_btn(
                    "Rectangular",
                    "edit-select-all",
                    ScreenshotMode::Area,
                    matches!(selected, ScreenshotMode::Area | ScreenshotMode::Snip),
                    p,
                ),
                mode_btn(
                    "Window",
                    "window",
                    ScreenshotMode::Window,
                    matches!(selected, ScreenshotMode::Window),
                    p,
                ),
                mode_btn(
                    "Full screen",
                    "view-fullscreen",
                    ScreenshotMode::Screen,
                    matches!(selected, ScreenshotMode::Screen),
                    p,
                ),
                Space::with_width(8),
                quiet(text("✕").size(16))
                    .on_press(Message::SnipCancel)
                    .padding([10, 12]),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([6, 10]),
        )
        .style(move |_| {
            let fill = mde_theme::flyout_fill(p);
            let edge = mde_theme::flyout_edge(p);
            ContainerStyle {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: edge,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            }
        });

        let dim = Color::from_rgba(0.0, 0.0, 0.0, 0.35);
        let backdrop = mouse_area(
            container(Space::new(Fill, Fill))
                .width(Fill)
                .height(Fill)
                .style(move |_| ContainerStyle {
                    background: Some(Background::Color(dim)),
                    ..Default::default()
                }),
        )
        .on_press(Message::SnipCancel);

        // Stack: dimmer fills; toolbar docks at top center (Win11).
        stack![
            backdrop,
            column![
                Space::with_height(24),
                row![Space::with_width(Fill), toolbar, Space::with_width(Fill)],
                Space::with_height(Fill),
            ]
            .width(Fill)
            .height(Fill),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// Bottom-right thumbnail toast — click opens swappy.
    pub(super) fn view_screenshot_toast(&self) -> Element<'_, Message> {
        let Some(toast) = &self.screenshot_toast else {
            return Space::new(0, 0).into();
        };
        let name = toast
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Screenshot")
            .to_string();
        let thumb = image(toast.path.clone())
            .width(Length::Fixed(72.0))
            .height(Length::Fixed(48.0));
        let card = container(
            row![
                thumb,
                column![
                    text("Screenshot saved").size(13),
                    text(name).size(11).color(caption_color(&self.palette)),
                ]
                .spacing(2)
                .width(Fill),
            ]
            .spacing(12)
            .padding(12)
            .align_y(iced::Alignment::Center),
        )
        .width(300)
        .style(move |_| {
            let fill = mde_theme::flyout_fill(&self.palette);
            let edge = mde_theme::flyout_edge(&self.palette);
            ContainerStyle {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: edge,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 6.0),
                    blur_radius: 18.0,
                },
                ..Default::default()
            }
        });
        column![
            Space::with_height(Fill),
            row![
                Space::with_width(Fill),
                quiet(card).on_press(Message::ScreenshotToastOpen),
            ]
            .padding([0, 16]),
            Space::with_height(56),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }
}

fn fg_color(palette: &Palette) -> Color {
    match Palette::parse_rgb(&palette.fg) {
        Some((r, g, b)) => Color::from_rgb(r, g, b),
        None => Color::from_rgb(0.1, 0.1, 0.1),
    }
}

fn accent_color(palette: &Palette) -> Color {
    match Palette::parse_rgb(&palette.accent) {
        Some((r, g, b)) => Color::from_rgb(r, g, b),
        None => Color::from_rgb(0.0, 0.47, 0.83),
    }
}

fn caption_color(palette: &Palette) -> Color {
    if palette.dark {
        Color::from_rgb(0.62, 0.62, 0.62)
    } else {
        Color::from_rgb(0.36, 0.36, 0.36)
    }
}
