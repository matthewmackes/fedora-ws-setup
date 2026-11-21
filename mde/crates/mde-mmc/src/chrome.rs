//! Win11 / classic MMC chrome helpers for the iced host (UI polish passes).

use iced::widget::button;
use iced::widget::{column, container, image, row, text, Space};
use iced::{Background, Border, Color, Element, Fill, Length, Padding, Theme};
use mde_theme::Palette;

use crate::model::Tone;
use crate::Message;

pub const TREE_W: f32 = 280.0;
pub const ACTIONS_W: f32 = 260.0;
pub const ICON_TREE: u16 = 16;
pub const ICON_ROW: u16 = 16;
pub const ICON_TOOL: u16 = 16;

pub fn pad() -> f32 {
    mde_theme::space::grid(2)
}

pub fn gap() -> f32 {
    mde_theme::space::grid(2)
}

pub fn control_radius() -> f32 {
    mde_theme::CONTROL_RADIUS
}

pub fn pane_radius() -> f32 {
    6.0
}

pub fn canvas(palette: &Palette) -> Color {
    if palette.high_contrast {
        if palette.dark {
            Color::BLACK
        } else {
            Color::WHITE
        }
    } else if palette.dark {
        // Win11 dark Settings canvas ~#202020
        parse(&palette.bg).unwrap_or(Color::from_rgb(0.125, 0.125, 0.125))
    } else {
        Color::from_rgb(0.953, 0.953, 0.953) // #f3f3f3
    }
}

pub fn pane_fill(palette: &Palette) -> Color {
    if palette.high_contrast {
        if palette.dark {
            Color::BLACK
        } else {
            Color::WHITE
        }
    } else {
        parse(&palette.bg_elevated).unwrap_or(if palette.dark {
            Color::from_rgb(0.173, 0.173, 0.173) // #2c2c2c
        } else {
            Color::WHITE
        })
    }
}

pub fn actions_fill(palette: &Palette) -> Color {
    if palette.high_contrast {
        pane_fill(palette)
    } else if palette.dark {
        Color::from_rgb(0.15, 0.15, 0.15)
    } else {
        Color::from_rgb(0.984, 0.984, 0.984) // #fbfbfb
    }
}

pub fn fg(palette: &Palette) -> Color {
    parse(&palette.fg).unwrap_or(if palette.dark {
        Color::WHITE
    } else {
        Color::from_rgb(0.102, 0.102, 0.102)
    })
}

pub fn muted(palette: &Palette) -> Color {
    mde_theme::muted(palette)
}

pub fn accent(palette: &Palette) -> Color {
    mde_theme::accent(palette)
}

pub fn border(palette: &Palette) -> Color {
    if palette.high_contrast {
        if palette.dark {
            Color::WHITE
        } else {
            Color::BLACK
        }
    } else {
        parse(&palette.border).unwrap_or(mde_theme::flyout_edge(palette))
    }
}

pub fn parse(hex: &str) -> Option<Color> {
    Palette::parse_rgb(hex).map(|(r, g, b)| Color::from_rgb(r, g, b))
}

pub fn icon_face(name: &str, px: u16) -> Element<'static, Message> {
    // Default: dark glyphs for light canvas (classic MMC / Settings light).
    icon_face_themed(name, px, false)
}

/// When `dark_ui` is true, use Mackes-Carbon-Light (light glyphs on dark canvas).
pub fn icon_face_themed(name: &str, px: u16, dark_ui: bool) -> Element<'static, Message> {
    let path = if name.is_empty() {
        None
    } else {
        mde_theme::icons::for_ui(name, u32::from(px), dark_ui)
    };
    match path {
        Some(p) => image(p).width(px).height(px).into(),
        None => {
            let mark = name.chars().next().unwrap_or('•').to_string();
            text(mark).size(f32::from(px) * 0.7).into()
        }
    }
}

pub fn icon_for(palette: &Palette, name: &str, px: u16) -> Element<'static, Message> {
    // Include HC dark: light glyphs stay required on black canvases.
    icon_face_themed(name, px, palette.dark)
}

pub fn action_icon(id: &str) -> &'static str {
    match id {
        "refresh" | "rescan" => "view-refresh",
        "start" | "enable" | "ntp-on" | "unmask" => "media-playback-start",
        "stop" | "disable" | "ntp-off" | "shutdown" | "mask" => "media-playback-stop",
        "restart" => "view-refresh",
        "remove" | "rm" | "destroy" | "vacuum" | "upgrade" => "edit-delete",
        "driver-info" | "deps" | "dropin" | "recovery" | "driver-update" => "dialog-information",
        "power" | "profile" | "settings" => "preferences-system",
        _ => "application-x-executable",
    }
}

pub fn tool_icon(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Back => "go-previous",
        ToolKind::Forward => "go-next",
        ToolKind::Up => "go-up",
        ToolKind::Tree => "view-list",
        ToolKind::Refresh => "view-refresh",
        ToolKind::Actions => "view-grid",
        ToolKind::Properties => "document-properties",
        ToolKind::Help => "help-about",
    }
}

#[derive(Clone, Copy)]
pub enum ToolKind {
    Back,
    Forward,
    Up,
    Tree,
    Refresh,
    Actions,
    Properties,
    Help,
}

pub fn pane_style(palette: &Palette) -> impl Fn(&Theme) -> container::Style {
    let bg = pane_fill(palette);
    let edge = border(palette);
    let radius = pane_radius();
    move |_theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: edge,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

pub fn actions_pane_style(palette: &Palette) -> impl Fn(&Theme) -> container::Style {
    let bg = actions_fill(palette);
    let edge = border(palette);
    let radius = pane_radius();
    move |_theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: edge,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

pub fn canvas_style(palette: &Palette) -> impl Fn(&Theme) -> container::Style {
    let bg = canvas(palette);
    move |_theme| container::Style {
        background: Some(Background::Color(bg)),
        ..Default::default()
    }
}

pub fn select_row_style(
    palette: &Palette,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    let fg = fg(palette);
    let accent = accent(palette);
    let wash = mde_theme::hover_wash(palette);
    let hc = palette.high_contrast;
    let dark = palette.dark;
    let radius = if hc { 0.0 } else { control_radius() * 0.5 };
    move |_theme, status| {
        let background = if selected {
            Some(Background::Color(if hc {
                accent
            } else {
                Color {
                    a: if dark { 0.28 } else { 0.18 },
                    ..accent
                }
            }))
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(wash))
                }
                _ => None,
            }
        };
        let text_color = if selected && hc {
            if accent.r + accent.g + accent.b > 1.5 {
                Color::BLACK
            } else {
                Color::WHITE
            }
        } else {
            fg
        };
        button::Style {
            background,
            text_color,
            border: Border {
                radius: radius.into(),
                width: if selected && !hc { 1.0 } else if selected && hc { 2.0 } else { 0.0 },
                color: if selected {
                    if hc {
                        fg
                    } else {
                        Color { a: 0.35, ..accent }
                    }
                } else {
                    Color::TRANSPARENT
                },
            },
            ..Default::default()
        }
    }
}

pub fn quiet_btn_style(palette: &Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    let fg = fg(palette);
    let wash = mde_theme::hover_wash(palette);
    let radius = control_radius() * 0.5;
    move |_theme, status| {
        let background = match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(wash)),
            _ => None,
        };
        button::Style {
            background,
            text_color: fg,
            border: Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub fn dialog_card_style(palette: &Palette) -> impl Fn(&Theme) -> container::Style {
    let bg = pane_fill(palette);
    let edge = border(palette);
    move |_theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: edge,
            width: 1.0,
            radius: mde_theme::FLYOUT_RADIUS.into(),
        },
        ..Default::default()
    }
}

pub fn dim_style() -> impl Fn(&Theme) -> container::Style {
    dim_style_for(false)
}

pub fn dim_style_for(dark: bool) -> impl Fn(&Theme) -> container::Style {
    let a = if dark { 0.55 } else { 0.35 };
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, a))),
        ..Default::default()
    }
}

pub fn header_cell(palette: &Palette, label: String) -> Element<'static, Message> {
    text(label)
        .size(11)
        .color(muted(palette))
        .width(Fill)
        .into()
}

pub fn tool_btn(palette: &Palette, kind: ToolKind, msg: Message) -> Element<'static, Message> {
    let face = row![icon_for(palette, tool_icon(kind), ICON_TOOL)]
        .padding(6)
        .align_y(iced::Alignment::Center);
    button(face)
        .on_press(msg)
        .style(quiet_btn_style(palette))
        .padding(0)
        .into()
}

pub fn menu_label_btn(
    palette: &Palette,
    label: &'static str,
    open: bool,
    msg: Message,
) -> Element<'static, Message> {
    let fg_c = if open { accent(palette) } else { fg(palette) };
    button(text(label).size(13).color(fg_c))
        .on_press(msg)
        .style(quiet_btn_style(palette))
        .padding(Padding::from([4, 10]))
        .into()
}

pub fn toolbar_sep(palette: &Palette) -> Element<'static, Message> {
    let c = Color {
        a: 0.12,
        ..fg(palette)
    };
    container(Space::new(1, 18))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(c)),
            ..Default::default()
        })
        .into()
}

/// 1px hairline used under the menubar/toolbar and above the status bar.
pub fn chrome_rule(palette: &Palette) -> Element<'static, Message> {
    let c = Color {
        a: if palette.high_contrast { 1.0 } else { 0.08 },
        ..fg(palette)
    };
    container(Space::new(Fill, 1))
        .width(Fill)
        .height(1)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(c)),
            ..Default::default()
        })
        .into()
}

pub fn body_font() -> iced::Font {
    iced::Font::with_name(mde_theme::FONT)
}

pub fn mono_font() -> iced::Font {
    iced::Font::with_name(mde_theme::FONT_MONO)
}

pub fn pad4(top: u16, right: u16, bottom: u16, left: u16) -> Padding {
    Padding {
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
        left: left as f32,
    }
}

pub fn tone_color(palette: &Palette, tone: Tone) -> Color {
    match tone {
        Tone::Neutral => muted(palette),
        Tone::Accent => accent(palette),
        Tone::Good => {
            if palette.dark {
                Color::from_rgb(0.42, 0.78, 0.45)
            } else {
                Color::from_rgb(0.10, 0.55, 0.25)
            }
        }
        Tone::Warn => {
            if palette.dark {
                Color::from_rgb(0.95, 0.75, 0.30)
            } else {
                Color::from_rgb(0.72, 0.48, 0.05)
            }
        }
        Tone::Bad => {
            if palette.dark {
                Color::from_rgb(0.95, 0.45, 0.42)
            } else {
                Color::from_rgb(0.75, 0.15, 0.15)
            }
        }
    }
}

/// Infer a tone from common status words (services, devices, firewall).
pub fn tone_from_status(s: &str) -> Tone {
    let lower = s.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("error")
        || lower.contains("dead")
        || lower.contains("down")
        || lower.contains("disconnected")
        || lower == "disabled"
    {
        Tone::Bad
    } else if lower.contains("warn")
        || lower.contains("pending")
        || lower.contains("masked")
        || lower.contains("degraded")
    {
        Tone::Warn
    } else if lower.contains("run")
        || lower.contains("active")
        || lower.contains("connected")
        || lower.contains("enforcing")
        || lower == "automatic"
        || lower == "up"
        || lower.contains("enabled")
    {
        Tone::Good
    } else {
        Tone::Neutral
    }
}

pub fn col_length(weights: &[u16], index: usize) -> Length {
    let w = weights.get(index).copied().unwrap_or(1).max(1);
    if weights.is_empty() {
        Length::Fill
    } else {
        Length::FillPortion(w)
    }
}

pub fn section_label(palette: &Palette, label: &str) -> Element<'static, Message> {
    text(label.to_string())
        .size(12)
        .color(muted(palette))
        .font(body_font())
        .into()
}

pub fn summary_chip(
    palette: &Palette,
    label: String,
    value: String,
    tone: Tone,
) -> Element<'static, Message> {
    let edge = border(palette);
    let fill = if palette.dark {
        Color::from_rgb(0.20, 0.20, 0.20)
    } else {
        Color::from_rgb(0.96, 0.96, 0.96)
    };
    let hc = palette.high_contrast;
    let radius = if hc { 0.0 } else { control_radius() };
    let face = column![
        text(label).size(11).color(muted(palette)),
        text(value)
            .size(16)
            .color(tone_color(palette, tone))
            .font(body_font()),
    ]
    .spacing(2)
    .padding(pad4(10, 14, 10, 14));
    container(face)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: Color {
                    a: if hc { 1.0 } else { 0.55 },
                    ..edge
                },
                width: 1.0,
                radius: radius.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn gauge_row(
    palette: &Palette,
    label: String,
    value: f32,
    display: String,
    tone: Tone,
) -> Element<'static, Message> {
    let pct = value.clamp(0.0, 100.0);
    let track = if palette.dark {
        Color::from_rgb(0.28, 0.28, 0.28)
    } else {
        Color::from_rgb(0.90, 0.90, 0.90)
    };
    let fill_c = tone_color(palette, tone);
    let radius = if palette.high_contrast {
        0.0
    } else {
        4.0
    };
    // Approximate fill with FillPortion (no canvas dependency).
    let filled = pct.round() as u16;
    let empty = 100u16.saturating_sub(filled);
    let bar: Element<'static, Message> = if filled == 0 {
        container(Space::new(Fill, 8))
            .width(Fill)
            .height(8)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(track)),
                border: Border {
                    radius: radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else if empty == 0 {
        container(Space::new(Fill, 8))
            .width(Fill)
            .height(8)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(fill_c)),
                border: Border {
                    radius: radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        row![
            container(Space::new(Fill, 8))
                .width(Length::FillPortion(filled))
                .height(8)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(fill_c)),
                    border: Border {
                        radius: radius.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            container(Space::new(Fill, 8))
                .width(Length::FillPortion(empty))
                .height(8)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(track)),
                    border: Border {
                        radius: radius.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(0)
        .width(Fill)
        .into()
    };

    column![
        row![
            text(label)
                .size(13)
                .color(fg(palette))
                .font(body_font())
                .width(Length::Fixed(120.0)),
            text(display)
                .size(13)
                .color(tone_color(palette, tone))
                .font(body_font())
                .width(Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        bar,
    ]
    .spacing(6)
    .width(Fill)
    .into()
}

pub fn property_block(
    palette: &Palette,
    props: &[(String, String)],
) -> Element<'static, Message> {
    let mut col = column![section_label(palette, "Details")].spacing(6);
    for (k, v) in props {
        col = col.push(
            row![
                text(k.clone())
                    .size(12)
                    .color(muted(palette))
                    .width(Length::Fixed(140.0)),
                text(v.clone())
                    .size(12)
                    .color(fg(palette))
                    .font(body_font())
                    .width(Fill),
            ]
            .spacing(8),
        );
    }
    container(col.padding(pad4(8, 16, 12, 16)))
        .width(Fill)
        .style(actions_pane_style(palette))
        .into()
}

/// Color status-like table cells; leave other columns at default fg.
pub fn cell_color(palette: &Palette, column: &str, value: &str) -> Color {
    let c = column.to_ascii_lowercase();
    let tone = if c == "startup type" {
        let lower = value.to_ascii_lowercase();
        if lower.contains("mask") {
            Tone::Warn
        } else if lower.contains("automatic") {
            Tone::Good
        } else {
            Tone::Neutral
        }
    } else if c.contains("status")
        || c.contains("state")
        || c.contains("level")
        || c == "mode"
        || c == "kind"
    {
        tone_from_status(value)
    } else {
        return fg(palette);
    };
    tone_color(palette, tone)
}
