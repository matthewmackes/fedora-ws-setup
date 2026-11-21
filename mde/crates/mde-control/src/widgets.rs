//! Win11 Settings design system (W285–W309).
//! Canvas, cards, rows, nav, heroes, and controls shared by every page.

use iced::widget::{button, column, container, image, row, text, toggler, Space};
use iced::widget::container::Style as ContainerStyle;
use iced::{Background, Border, Color, Element, Fill, Length, Padding, Theme};
use mde_theme::Palette;

use crate::Message;

/// Light canvas `#f3f3f3` / dark `#202020` (W285).
pub fn canvas_color(palette: &Palette) -> Color {
    if palette.high_contrast {
        hex_color(if palette.dark { "#000000" } else { "#ffffff" })
    } else if palette.dark {
        hex_color("#202020")
    } else {
        hex_color("#f3f3f3")
    }
}

pub fn card_fill(palette: &Palette) -> Color {
    if palette.high_contrast {
        hex_color(if palette.dark { "#000000" } else { "#ffffff" })
    } else if palette.dark {
        hex_color("#2c2c2c")
    } else {
        hex_color("#fbfbfb")
    }
}

pub fn card_border(palette: &Palette) -> Color {
    if palette.high_contrast {
        hex_color(if palette.dark { "#ffffff" } else { "#000000" })
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, if palette.dark { 0.18 } else { 0.06 })
    }
}

pub fn caption_color(palette: &Palette) -> Color {
    if palette.high_contrast {
        hex_color(&palette.fg)
    } else if palette.dark {
        hex_color("#9d9d9d")
    } else {
        hex_color("#5d5d5d")
    }
}

pub fn fg_color(palette: &Palette) -> Color {
    hex_color(&palette.fg)
}

pub fn accent_color(palette: &Palette) -> Color {
    hex_color(&palette.accent)
}

pub fn hex_color(hex: &str) -> Color {
    match Palette::parse_rgb(hex) {
        Some((r, g, b)) => Color::from_rgb(r, g, b),
        None => Color::from_rgb(0.5, 0.5, 0.5),
    }
}

fn hover_wash(palette: &Palette) -> Color {
    if palette.high_contrast {
        Color::TRANSPARENT
    } else if palette.dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.06)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.04)
    }
}

/// Page shell: title + optional caption + body, padded content (W285–W287).
pub fn page_frame<'a>(
    palette: &Palette,
    title: &'a str,
    caption: Option<&'a str>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let fg = fg_color(palette);
    let cap = caption_color(palette);
    let mut col = column![text(title.to_string()).size(28).color(fg)].spacing(8);
    if let Some(c) = caption {
        col = col.push(text(c.to_string()).size(13).color(cap));
    }
    col = col.push(Space::with_height(8));
    col = col.push(body);
    container(col.spacing(12))
        .padding(Padding {
            top: 8.0,
            right: 40.0,
            bottom: 40.0,
            left: 28.0,
        })
        .width(Length::Fill)
        .max_width(1000.0)
        .into()
}

/// Section label (W287).
pub fn section_label(palette: &Palette, title: &str) -> Element<'static, Message> {
    container(text(title.to_string()).size(14).color(fg_color(palette)))
        .padding(Padding {
            top: 24.0,
            right: 0.0,
            bottom: 8.0,
            left: 2.0,
        })
        .into()
}

/// Settings card chrome (W288).
pub fn card<'a>(palette: &Palette, content: Element<'a, Message>) -> Element<'a, Message> {
    let fill = card_fill(palette);
    let border = card_border(palette);
    let radius = if palette.high_contrast { 0.0 } else { 4.0 };
    container(content)
        .padding([0, 16])
        .width(Fill)
        .style(move |_theme| ContainerStyle {
            background: Some(Background::Color(fill)),
            border: Border {
                color: border,
                width: 1.0,
                radius: radius.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Hero card with 8px radius (W296).
pub fn hero<'a>(palette: &Palette, content: Element<'a, Message>) -> Element<'a, Message> {
    let fill = card_fill(palette);
    let border = card_border(palette);
    let radius = if palette.high_contrast { 0.0 } else { 8.0 };
    container(content)
        .padding(16)
        .width(Fill)
        .style(move |_theme| ContainerStyle {
            background: Some(Background::Color(fill)),
            border: Border {
                color: border,
                width: 1.0,
                radius: radius.into(),
            },
            ..Default::default()
        })
        .into()
}

fn icon_face<'a>(palette: &Palette, icon: &str, size: u16) -> Element<'a, Message> {
    if let Some(path) = mde_theme::icons::for_ui(icon, u32::from(size), palette.dark) {
        image(path).width(size).height(size).into()
    } else {
        let mark = icon.chars().next().unwrap_or('•').to_string();
        text(mark).size(f32::from(size) * 0.7).into()
    }
}

/// Settings row: icon + title/caption | control (W289).
pub fn settings_row<'a>(
    palette: &Palette,
    icon: &str,
    title: impl Into<String>,
    caption: Option<impl Into<String>>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let fg = fg_color(palette);
    let cap_c = caption_color(palette);
    let mut left = column![text(title.into()).size(14).color(fg)].spacing(2);
    if let Some(c) = caption {
        left = left.push(text(c.into()).size(12).color(cap_c));
    }
    let inner = row![
        icon_face(palette, icon, 16),
        left.width(Fill),
        control,
    ]
    .spacing(14)
    .padding([14, 0])
    .align_y(iced::Alignment::Center);
    card(palette, inner.into())
}

/// Navigable hub/nav card with chevron (W307).
pub fn nav_card(
    palette: &Palette,
    icon: &str,
    title: impl Into<String>,
    caption: impl Into<String>,
    message: Message,
) -> Element<'static, Message> {
    let title = title.into();
    let caption = caption.into();
    let fill = card_fill(palette);
    let border = card_border(palette);
    let fg = fg_color(palette);
    let cap = caption_color(palette);
    let wash = hover_wash(palette);
    let radius = if palette.high_contrast { 0.0 } else { 4.0 };
    let face = row![
        icon_face(palette, icon, 20),
        column![
            text(title).size(15).color(fg),
            text(caption).size(12).color(cap),
        ]
        .spacing(2)
        .width(Fill),
        text(">").size(14).color(cap),
    ]
    .spacing(14)
    .padding([16, 4])
    .align_y(iced::Alignment::Center);
    button(face)
        .on_press(message)
        .style(move |_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    // Mix wash over card fill.
                    Some(Background::Color(Color {
                        r: (fill.r * 0.92 + wash.r * wash.a).min(1.0),
                        g: (fill.g * 0.92 + wash.g * wash.a).min(1.0),
                        b: (fill.b * 0.92 + wash.b * wash.a).min(1.0),
                        a: 1.0,
                    }))
                }
                _ => Some(Background::Color(fill)),
            };
            iced::widget::button::Style {
                background: bg,
                text_color: fg,
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            }
        })
        .padding([0, 12])
        .width(Fill)
        .into()
}

/// Child hub rows for a parent category (W307).
pub fn child_nav_rows(palette: &Palette, parent: &str) -> Element<'static, Message> {
    let mut col = column![].spacing(4);
    for child in crate::nav::children(parent) {
        col = col.push(nav_card(
            palette,
            child.icon,
            child.title,
            child.caption,
            Message::Navigate(child.key.into()),
        ));
    }
    col.into()
}

/// Status-only row (value on the right).
pub fn status_row(
    palette: &Palette,
    icon: &str,
    title: &str,
    value: &str,
) -> Element<'static, Message> {
    let cap = caption_color(palette);
    settings_row(
        palette,
        icon,
        title.to_string(),
        None::<String>,
        text(value.to_string()).size(13).color(cap).into(),
    )
}

/// Toggle row that applies immediately (W291).
pub fn toggle_row(
    palette: &Palette,
    icon: &str,
    title: impl Into<String>,
    caption: Option<impl Into<String>>,
    active: bool,
    on_toggle: impl Fn(bool) -> Message + 'static,
) -> Element<'static, Message> {
    settings_row(
        palette,
        icon,
        title,
        caption,
        toggler(active).on_toggle(on_toggle).into(),
    )
}

/// Quiet link / secondary button (W295).
pub fn link_btn(palette: &Palette, label: impl Into<String>, message: Message) -> Element<'static, Message> {
    let accent = accent_color(palette);
    button(text(label.into()).size(13).color(accent))
        .on_press(message)
        .style(button::text)
        .padding([6, 8])
        .into()
}

/// Accent primary button (W295).
pub fn accent_btn(palette: &Palette, label: impl Into<String>, message: Message) -> Element<'static, Message> {
    let accent = accent_color(palette);
    button(text(label.into()).size(13).color(Color::WHITE))
        .on_press(message)
        .style(move |_theme, status| {
            let a = match status {
                button::Status::Hovered | button::Status::Pressed => 0.88,
                _ => 1.0,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(Color { a, ..accent })),
                text_color: Color::WHITE,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .padding([8, 14])
        .into()
}

/// Quiet card-style button (W295).
pub fn quiet_btn(palette: &Palette, label: impl Into<String>, message: Message) -> Element<'static, Message> {
    let fill = card_fill(palette);
    let border = card_border(palette);
    let fg = fg_color(palette);
    button(text(label.into()).size(13).color(fg))
        .on_press(message)
        .style(move |_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(Color::from_rgba(fg.r, fg.g, fg.b, 0.06)))
                }
                _ => Some(Background::Color(fill)),
            };
            iced::widget::button::Style {
                background: bg,
                text_color: fg,
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .padding([6, 12])
        .into()
}

/// Choice chip (selected = accent).
pub fn choice(palette: &Palette, label: &str, selected: bool, message: Message) -> Element<'static, Message> {
    if selected {
        accent_btn(palette, label.to_string(), message)
    } else {
        quiet_btn(palette, label.to_string(), message)
    }
}

/// Sidebar nav button with accent bar when selected (W290).
pub fn nav_btn(
    palette: &Palette,
    icon: &str,
    label: &str,
    target: String,
    current: &str,
) -> Element<'static, Message> {
    let selected = target == current;
    let fg = fg_color(palette);
    let accent = accent_color(palette);
    let wash = hover_wash(palette);
    let dark = palette.dark;
    let face = row![
        icon_face(palette, icon, 18),
        text(label.to_string()).size(13).color(fg),
    ]
    .spacing(12)
    .padding([8, 10])
    .align_y(iced::Alignment::Center);

    let btn = button(face)
        .on_press(Message::Navigate(target))
        .style(move |_theme, status| {
            let background = if selected {
                Some(Background::Color(if dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.06)
                }))
            } else {
                match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(wash))
                    }
                    _ => None,
                }
            };
            iced::widget::button::Style {
                background,
                text_color: fg,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .padding(0)
        .width(Fill);

    let bar = container(Space::new(3, 28)).style(move |_theme| ContainerStyle {
        background: if selected {
            Some(Background::Color(accent))
        } else {
            None
        },
        border: Border {
            radius: 2.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    row![bar, btn]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(Fill)
        .into()
}

pub fn nav_quiet(theme: &Theme, status: button::Status) -> iced::widget::button::Style {
    let text_c = theme.palette().text;
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(Color::from_rgba(
                text_c.r, text_c.g, text_c.b, 0.06,
            )))
        }
        _ => None,
    };
    iced::widget::button::Style {
        background,
        text_color: text_c,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub const ACCENTS: &[(&str, &str)] = &[
    ("Blue", "#0078d4"),
    ("Teal", "#038387"),
    ("Purple", "#8764b8"),
    ("Green", "#0f7b0f"),
    ("Gold", "#c19c00"),
    ("Rose", "#e3008c"),
    ("Orange", "#ca5010"),
    ("Cyan", "#00b7c3"),
    ("Gray", "#7a7574"),
];

/// Accent swatches (W298).
pub fn accent_swatches(palette: &Palette, current: &str) -> Element<'static, Message> {
    let mut choices = row![].spacing(8);
    for (name, hex) in ACCENTS {
        let active = current.eq_ignore_ascii_case(hex);
        let fill = hex_color(hex);
        let border = if active {
            fg_color(palette)
        } else {
            Color::TRANSPARENT
        };
        let swatch = container(Space::new(36, 36)).style(move |_theme| ContainerStyle {
            background: Some(Background::Color(fill)),
            border: Border {
                color: border,
                width: 2.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });
        choices = choices.push(
            button(swatch)
                .on_press(Message::SetAccent((*hex).into()))
                .style(button::text)
                .padding(2),
        );
        let _ = name;
    }
    choices.into()
}

pub fn appearance_card(palette: &Palette) -> Element<'static, Message> {
    let muted = caption_color(palette);
    let fg = fg_color(palette);
    hero(
        palette,
        column![
            text(mde_theme::PRODUCT).size(18).color(fg),
            text(format!(
                "{} · {}",
                mde_theme::SUBTITLE,
                mde_theme::FONT_BRANDING
            ))
            .size(13)
            .color(muted),
        ]
        .spacing(8)
        .into(),
    )
}

pub fn key_value(cfg: &mde_config::MdeConfig, path: &str) -> String {
    let p = cfg.active();
    match path {
        "shell.taskbar_align" => p.shell.taskbar_align.clone(),
        "shell.taskbar_left" => p.shell.taskbar_left.to_string(),
        "shell.taskbar_search" => p.shell.taskbar_search.clone(),
        "shell.taskbar_autohide" => p.shell.taskbar_autohide.to_string(),
        "shell.taskbar_combine" => p.shell.taskbar_combine.clone(),
        "shell.taskbar_labels" => p.shell.taskbar_labels.to_string(),
        "shell.taskbar_size" => p.shell.taskbar_size.clone(),
        "shell.taskbar_badges" => p.shell.taskbar_badges.to_string(),
        "shell.widgets_btn" => p.shell.widgets_btn.to_string(),
        "start.layout" => p.start.layout.clone(),
        "start.recently_added" => p.start.recently_added.to_string(),
        "theme.accent" => p.theme.accent.clone(),
        "theme.mode" => p.theme.mode.clone(),
        "theme.animations" => p.theme.animations.to_string(),
        "theme.transparency" => p.theme.transparency.to_string(),
        "theme.opacity" => p.theme.opacity.to_string(),
        "theme.dynamic_transparency" => p.theme.dynamic_transparency.to_string(),
        "theme.blur_preset" => p.theme.blur_preset.clone(),
        "theme.accent_on_start_taskbar" => p.theme.accent_on_start_taskbar.to_string(),
        "theme.accent_on_titlebars" => p.theme.accent_on_titlebars.to_string(),
        "theme.always_show_scrollbars" => p.theme.always_show_scrollbars.to_string(),
        "theme.wallpaper" => p.theme.wallpaper.clone(),
        "theme.bing_daily" => p.theme.bing_daily.to_string(),
        "theme.bing_title" => p.theme.bing_title.clone(),
        "theme.bing_copyright" => p.theme.bing_copyright.clone(),
        "theme.dark_schedule" => p.theme.dark_schedule.to_string(),
        "theme.dark_from" => p.theme.dark_from.clone(),
        "theme.dark_until" => p.theme.dark_until.clone(),
        "session.night_light" => p.session.night_light.to_string(),
        "displays.main_display" => p.displays.main_display.clone(),
        "displays.allow_fractional_scale" => p.displays.allow_fractional_scale.to_string(),
        "session.idle_lock_seconds" => p.session.idle_lock_seconds.to_string(),
        "session.xwayland" => p.session.xwayland.to_string(),
        "notifications.dnd" => p.notifications.dnd.to_string(),
        "notifications.toast_seconds" => p.notifications.toast_seconds.to_string(),
        "clipboard.history" => p.clipboard.history.to_string(),
        "clipboard.max_items" => p.clipboard.max_items.to_string(),
        "clipboard.cross_device" => p.clipboard.cross_device.to_string(),
        "clipboard.sync_dir" => p.clipboard.sync_dir.clone(),
        "network.wifi" => p.network.wifi.to_string(),
        "network.bluetooth" => p.network.bluetooth.to_string(),
        "network.airplane_mode" => p.network.airplane_mode.to_string(),
        "compositor.layout" => p.compositor.layout.clone(),
        "compositor.decorations" => p.compositor.decorations.clone(),
        "compositor.server_tab_strip" => p.compositor.server_tab_strip.to_string(),
        "tools.command_palette" => p.tools.command_palette.to_string(),
        "tools.fancy_zones" => p.tools.fancy_zones.to_string(),
        "tools.snap_groups" => p.tools.snap_groups.to_string(),
        "tools.always_on_top_shortcut" => p.tools.always_on_top_shortcut.to_string(),
        "tools.always_on_top_indicator" => p.tools.always_on_top_indicator.to_string(),
        "tools.color_picker_history" => p.tools.color_picker_history.to_string(),
        "tools.find_my_mouse" => p.tools.find_my_mouse.to_string(),
        "tools.mouse_highlighter" => p.tools.mouse_highlighter.to_string(),
        "tools.awake_minutes" => p.tools.awake_minutes.to_string(),
        "tools.jump_list_max" => p.tools.jump_list_max.to_string(),
        "tools.advanced_paste" => p.tools.advanced_paste.to_string(),
        _ => cfg
            .extras
            .get(path)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
    }
}

/// Empty / error state row (W299).
pub fn empty_state(
    palette: &Palette,
    title: &str,
    caption: &str,
    action: Option<(String, Message)>,
) -> Element<'static, Message> {
    let mut col = column![
        text(title.to_string()).size(14).color(fg_color(palette)),
        text(caption.to_string()).size(12).color(caption_color(palette)),
    ]
    .spacing(6);
    if let Some((label, msg)) = action {
        col = col.push(accent_btn(palette, label, msg));
    }
    hero(palette, col.into())
}

/// Bottom status toast (W303).
pub fn status_toast(palette: &Palette, status: &str) -> Element<'static, Message> {
    if status.trim().is_empty() {
        return Space::with_height(0).into();
    }
    container(text(status.to_string()).size(12).color(caption_color(palette)))
        .padding([8, 28])
        .width(Fill)
        .into()
}

// Compatibility aliases used by older page code during migration.
pub fn settings_card(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    message: Message,
) -> Element<'static, Message> {
    // Palette-less fallback: light defaults for call sites that have not been migrated.
    let palette = Palette::load();
    nav_card(&palette, "go-next", title, subtitle, message)
}

pub fn accent_choices(current: &str) -> Element<'static, Message> {
    accent_swatches(&Palette::load(), current)
}
