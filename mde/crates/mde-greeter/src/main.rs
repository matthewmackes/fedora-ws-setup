//! Graphical greeter — Windows 11–style sign-in (iced), speak greetd IPC.
//!
//! Invoked by `mde-comp greeter` under cage, or directly:
//! `cage -s -- mde-greeter` / `mde-comp greeter`.

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::widget::container::Style as ContainerStyle;
use iced::{Background, Border, Color, Element, Fill, Subscription, Task, Theme};
use mde_greeter::greetd;
use mde_theme::Palette;
use std::time::Duration;

fn main() -> iced::Result {
    mde_config::supervisor::install_panic_hook();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Headless self-check without a display (install / CI).
    if std::env::args().any(|a| a == "--selftest") {
        greetd::session_command();
        let _ = greetd::default_username();
        println!("{{\"ok\":true,\"bin\":\"mde-greeter\"}}");
        return Ok(());
    }

    let boot_cfg = mde_config::MdeConfig::load().unwrap_or_default();
    let ui = mde_theme::ui_font(&boot_cfg.active().theme).to_string();
    let mono = mde_theme::mono_font(&boot_cfg.active().theme).to_string();
    let mut app = iced::application("Sign in", Greeter::update, Greeter::view)
        .theme(Greeter::theme)
        .subscription(Greeter::subscription)
        .default_font(iced::Font::with_name(Box::leak(ui.clone().into_boxed_str())))
        .window(iced::window::Settings {
            size: iced::Size::new(1280.0, 800.0),
            decorations: false,
            resizable: false,
            ..Default::default()
        });
    for bytes in mde_theme::font_bytes_selected(&ui, &mono) {
        app = app.font(bytes);
    }
    app.run_with(Greeter::new)
}

#[derive(Debug, Clone)]
enum Message {
    Username(String),
    Password(String),
    SignIn,
    AuthDone(Result<(), String>),
    Tick,
    /// Greeter power menu (W129).
    Restart,
    ShutDown,
    UpdateAndRestart,
    UpdateAndShutDown,
    /// Accessibility options stub (W149).
    AccessibilityStub,
    /// Keyboard layout label click stub (W249).
    KeyboardLayoutStub,
    /// Caps Lock key toggled (W229).
    CapsLockToggle,
}

struct Greeter {
    username: String,
    password: String,
    error: String,
    busy: bool,
    clock: String,
    date: String,
    /// Keyboard layout label under the password field (W92).
    kb_layout: String,
    /// Quiet Wi-Fi / wired glyph (W169).
    net_glyph: String,
    /// Battery percent when `/sys/class/power_supply` has a Battery (W189).
    battery_label: Option<String>,
    /// Caps Lock on (tracked via CapsLock key events) (W229).
    caps_on: bool,
    /// A056: greeter high-contrast toggle.
    high_contrast: bool,
    palette: Palette,
    sock: Option<String>,
}

impl Greeter {
    fn new() -> (Self, Task<Message>) {
        // Read theme accent from MDE config when available (W71); keep dark sign-in surfaces.
        let loaded = Palette::load();
        let palette = Palette {
            accent: loaded.accent,
            bg: "#0b1626".into(),
            bg_elevated: "#202020".into(),
            fg: "#ffffff".into(),
            fg_muted: "rgba(255,255,255,0.8)".into(),
            border: "#3f3f3f".into(),
            danger: "#ff99a4".into(),
            dark: true,
            high_contrast: false,
            transparency: loaded.transparency,
            opacity: loaded.opacity,
            dynamic_transparency: loaded.dynamic_transparency,
            blur_preset: loaded.blur_preset,
            accent_on_start_taskbar: loaded.accent_on_start_taskbar,
            accent_on_titlebars: loaded.accent_on_titlebars,
            ssd_corner_radius: loaded.ssd_corner_radius,
            ssd_mica: loaded.ssd_mica,
        };
        (
            Self {
                username: greetd::default_username(),
                password: String::new(),
                error: String::new(),
                busy: false,
                clock: clock_time(),
                date: clock_date(),
                kb_layout: keyboard_layout_label(),
                net_glyph: network_glyph_label(),
                battery_label: battery_glyph_label(),
                caps_on: false,
                high_contrast: false,
                palette,
                sock: std::env::var("GREETD_SOCK").ok(),
            },
            text_input::focus(text_input::Id::new("password")),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            // W229: track CapsLock key presses (iced has no caps lock modifier bit).
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                    if key
                        == iced::keyboard::Key::Named(iced::keyboard::key::Named::CapsLock) =>
                {
                    Some(Message::CapsLockToggle)
                }
                _ => None,
            }),
        ])
    }

    fn theme(&self) -> Theme {
        mde_theme::iced_theme(&self.palette)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Username(s) => {
                self.username = s;
                Task::none()
            }
            Message::Password(s) => {
                self.password = s;
                Task::none()
            }
            Message::Tick => {
                self.clock = clock_time();
                self.date = clock_date();
                self.net_glyph = network_glyph_label();
                self.battery_label = battery_glyph_label();
                Task::none()
            }
            Message::SignIn => {
                if self.busy {
                    return Task::none();
                }
                let Some(sock) = self.sock.clone() else {
                    self.error = "GREETD_SOCK is not set".into();
                    return Task::none();
                };
                if self.username.trim().is_empty() {
                    self.error = "Enter a username".into();
                    return Task::none();
                }
                self.busy = true;
                self.error.clear();
                let user = self.username.trim().to_string();
                let pass = self.password.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || greetd::authenticate(&sock, &user, &pass))
                            .await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| e.to_string())
                    },
                    Message::AuthDone,
                )
            }
            Message::AuthDone(result) => {
                self.busy = false;
                match result {
                    Ok(()) => {
                        // greetd takes over; exit the greeter process cleanly.
                        std::process::exit(0);
                    }
                    Err(e) => {
                        self.password.clear();
                        self.error = e;
                        text_input::focus(text_input::Id::new("password"))
                    }
                }
            }
            Message::Restart => {
                self.error = systemctl_power("reboot");
                Task::none()
            }
            Message::ShutDown => {
                self.error = systemctl_power("poweroff");
                Task::none()
            }
            Message::UpdateAndRestart => {
                self.error = offline_update_power("reboot");
                Task::none()
            }
            Message::UpdateAndShutDown => {
                self.error = offline_update_power("power-off");
                Task::none()
            }
            Message::AccessibilityStub => {
                self.high_contrast = !self.high_contrast;
                if self.high_contrast {
                    self.palette.high_contrast = true;
                    self.palette.bg = "#000000".into();
                    self.palette.bg_elevated = "#000000".into();
                    self.palette.fg = "#ffffff".into();
                    self.palette.fg_muted = "#ffffff".into();
                    self.palette.border = "#ffffff".into();
                    // A056: also raise an on-screen keyboard when available.
                    let _ = std::process::Command::new("wvkbd-mobintl").spawn().or_else(|_| {
                        std::process::Command::new("onboard").spawn()
                    });
                    self.error = "High contrast on · OSK if installed".into();
                } else {
                    let loaded = Palette::load();
                    self.palette.high_contrast = false;
                    self.palette.bg = "#0b1626".into();
                    self.palette.bg_elevated = "#202020".into();
                    self.palette.fg = "#ffffff".into();
                    self.palette.fg_muted = "rgba(255,255,255,0.8)".into();
                    self.palette.border = "#3f3f3f".into();
                    self.palette.accent = loaded.accent;
                    let _ = std::process::Command::new("pkill")
                        .args(["-x", "wvkbd-mobintl"])
                        .status();
                    let _ = std::process::Command::new("pkill")
                        .args(["-x", "onboard"])
                        .status();
                    self.error = "High contrast off".into();
                }
                Task::none()
            }
            Message::KeyboardLayoutStub => {
                let (label, err) = cycle_keyboard_layout();
                self.kb_layout = label;
                self.error = err;
                Task::none()
            }
            Message::CapsLockToggle => {
                self.caps_on = !self.caps_on;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let accent = Palette::parse_rgb(&self.palette.accent)
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.0, 0.47, 0.83));
        let accent_hover = Color::from_rgb(
            (accent.r + 0.08).min(1.0),
            (accent.g + 0.08).min(1.0),
            (accent.b + 0.08).min(1.0),
        );
        let accent_press = Color::from_rgb(
            (accent.r * 0.85).max(0.0),
            (accent.g * 0.85).max(0.0),
            (accent.b * 0.85).max(0.0),
        );
        let mica = Color::from_rgba(0.125, 0.125, 0.125, 0.72);

        let clock_col = column![
            text(&self.clock).size(96).color(Color::WHITE),
            text(&self.date).size(24).color(Color::from_rgba(1.0, 1.0, 1.0, 0.9)),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center);

        let user_field = text_input("Username", &self.username)
            .id(text_input::Id::new("username"))
            .on_input(Message::Username)
            .on_submit(Message::SignIn)
            .padding(12)
            .size(16)
            .width(320);

        let pass_field = text_input("Password", &self.password)
            .id(text_input::Id::new("password"))
            .secure(true)
            .on_input(Message::Password)
            .on_submit(Message::SignIn)
            .padding(12)
            .size(16)
            .width(320);

        let sign_in = button(
            text(if self.busy { "Signing in…" } else { "Sign in" })
                .size(15)
                .color(Color::WHITE),
        )
        .on_press_maybe(if self.busy {
            None
        } else {
            Some(Message::SignIn)
        })
        .padding([12, 36])
        .style(move |theme: &Theme, status| {
            let mut s = iced::widget::button::primary(theme, status);
            s.background = Some(Background::Color(match status {
                iced::widget::button::Status::Hovered => accent_hover,
                iced::widget::button::Status::Pressed => accent_press,
                _ => accent,
            }));
            s.border = Border {
                radius: 4.0.into(),
                ..Default::default()
            };
            s.text_color = Color::WHITE;
            s
        });

        let err = if self.error.is_empty() {
            text(" ").size(13)
        } else {
            text(&self.error)
                .size(13)
                .color(Color::from_rgb(1.0, 0.6, 0.64))
        };

        let power_btn = |label: &'static str, msg: Message| {
            button(text(label).size(13).color(Color::from_rgba(1.0, 1.0, 1.0, 0.9)))
                .on_press(msg)
                .padding([8, 16])
                .style(|_theme: &Theme, status| {
                    let alpha = match status {
                        iced::widget::button::Status::Hovered => 0.18,
                        iced::widget::button::Status::Pressed => 0.28,
                        _ => 0.10,
                    };
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color::from_rgba(
                            1.0, 1.0, 1.0, alpha,
                        ))),
                        text_color: Color::WHITE,
                        border: Border {
                            radius: 4.0.into(),
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                            width: 1.0,
                        },
                        ..Default::default()
                    }
                })
        };

        let power_row = if std::path::Path::new("/system-update").exists() {
            row![
                power_btn("Update and restart", Message::UpdateAndRestart),
                power_btn("Update and shut down", Message::UpdateAndShutDown),
                power_btn("Restart", Message::Restart),
                power_btn("Shut down", Message::ShutDown),
            ]
            .spacing(10)
        } else {
            row![
                power_btn("Restart", Message::Restart),
                power_btn("Shut down", Message::ShutDown),
            ]
            .spacing(10)
        };

        // W149: quiet Accessibility link → session Settings tip.
        let a11y = button(
            text("Accessibility")
                .size(12)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.65)),
        )
        .on_press(Message::AccessibilityStub)
        .padding([4, 8])
        .style(|_theme: &Theme, status| {
            let alpha = match status {
                iced::widget::button::Status::Hovered => 0.14,
                iced::widget::button::Status::Pressed => 0.22,
                _ => 0.0,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, alpha))),
                text_color: Color::from_rgba(1.0, 1.0, 1.0, 0.75),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

        let card = container(
            column![
                text(&self.username)
                    .size(28)
                    .color(Color::WHITE),
                Space::with_height(16),
                user_field,
                Space::with_height(10),
                pass_field,
                Space::with_height(8),
                // W229: CAPS when CapsLock toggled on; else quiet Caps Lock tip.
                if self.caps_on {
                    text("CAPS")
                        .size(12)
                        .color(Color::from_rgb(1.0, 0.85, 0.4))
                } else {
                    text("Caps Lock")
                        .size(11)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.45))
                },
                Space::with_height(6),
                // W249: click layout label → Settings · Keyboard tip.
                button(
                    text(format!("Keyboard · {}", self.kb_layout))
                        .size(12)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.7)),
                )
                .on_press(Message::KeyboardLayoutStub)
                .padding([2, 6])
                .style(|_theme: &Theme, status| {
                    let alpha = match status {
                        iced::widget::button::Status::Hovered => 0.12,
                        iced::widget::button::Status::Pressed => 0.20,
                        _ => 0.0,
                    };
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color::from_rgba(
                            1.0, 1.0, 1.0, alpha,
                        ))),
                        text_color: Color::from_rgba(1.0, 1.0, 1.0, 0.7),
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
                Space::with_height(6),
                // W209: session picker stub — single quiet “MDE” label (no multi-session).
                text("Session · MDE")
                    .size(12)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.55)),
                Space::with_height(4),
                // W269: fingerprint sign-in stub (not wired to fprintd).
                text("Fingerprint · not configured")
                    .size(11)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.40)),
                Space::with_height(16),
                sign_in,
                Space::with_height(14),
                power_row,
                Space::with_height(8),
                a11y,
                Space::with_height(10),
                err,
            ]
            .spacing(0)
            .align_x(iced::Alignment::Center)
            .padding(28),
        )
        .style(move |_| ContainerStyle {
            background: Some(Background::Color(mica)),
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

        let ribbon = container(Space::with_height(3))
            .width(180)
            .style(move |_| ContainerStyle {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let body = column![
            Space::with_height(Fill),
            clock_col,
            Space::with_height(12),
            ribbon,
            Space::with_height(36),
            card,
            Space::with_height(Fill),
            // W169 / W189: quiet network + optional battery glyph.
            {
                let mut footer = row![
                    Space::with_width(16),
                    text(&self.net_glyph)
                        .size(13)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.55)),
                ]
                .spacing(12);
                if let Some(bat) = &self.battery_label {
                    footer = footer.push(
                        text(bat)
                            .size(13)
                            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.55)),
                    );
                }
                footer = footer
                    .push(Space::with_width(Fill))
                    .push(
                        text(mde_theme::PRODUCT)
                            .size(12)
                            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.55)),
                    )
                    .push(Space::with_width(16));
                footer.padding(16)
            },
        ]
        .width(Fill)
        .height(Fill)
        .align_x(iced::Alignment::Center);

        container(body)
            .width(Fill)
            .height(Fill)
            .style(move |_| ContainerStyle {
                background: Some(Background::Color(Color::from_rgb(
                    0.043, 0.086, 0.149, // #0b1626
                ))),
                ..Default::default()
            })
            .into()
    }
}

fn clock_time() -> String {
    // W109: optional seconds on greeter clock (tick already refreshes).
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn clock_date() -> String {
    chrono::Local::now().format("%A, %B %-d").to_string()
}

/// A056: cycle common layouts via setxkbmap (best effort).
fn cycle_keyboard_layout() -> (String, String) {
    const PAIRS: &[(&str, &str)] = &[
        ("us", "English (US)"),
        ("gb", "English (UK)"),
        ("de", "German"),
        ("fr", "French"),
        ("es", "Spanish"),
    ];
    let current = keyboard_layout_label();
    let cur_code = current.split_whitespace().next().unwrap_or("us");
    let next_idx = PAIRS
        .iter()
        .position(|(c, _)| *c == cur_code)
        .map(|i| (i + 1) % PAIRS.len())
        .unwrap_or(0);
    let (code, label) = PAIRS[next_idx];
    match std::process::Command::new("setxkbmap").arg(code).status() {
        Ok(s) if s.success() => (label.to_string(), format!("Keyboard · {label}")),
        Ok(_) => (
            label.to_string(),
            format!("setxkbmap {code} failed — install xkeyboard-config"),
        ),
        Err(e) => (current, format!("Layout switch needs setxkbmap ({e})")),
    }
}

/// Best-effort layout name from `setxkbmap -query` or `localectl` (W92).
fn keyboard_layout_label() -> String {
    if let Ok(out) = std::process::Command::new("setxkbmap")
        .arg("-query")
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut layout = String::new();
            let mut variant = String::new();
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("layout:") {
                    layout = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("variant:") {
                    variant = v.trim().to_string();
                }
            }
            if !layout.is_empty() {
                return if variant.is_empty() {
                    layout
                } else {
                    format!("{layout} ({variant})")
                };
            }
        }
    }
    if let Ok(out) = std::process::Command::new("localectl").arg("status").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Some(rest) = line.split("X11 Layout:").nth(1) {
                    let layout = rest.trim();
                    if !layout.is_empty() {
                        return layout.to_string();
                    }
                }
                if let Some(rest) = line.split("VC Keymap:").nth(1) {
                    let layout = rest.trim();
                    if !layout.is_empty() && layout != "n/a" {
                        return layout.to_string();
                    }
                }
            }
        }
    }
    "us".into()
}

/// Battery percent from `/sys/class/power_supply/*/capacity` when present (W189).
fn battery_glyph_label() -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for ent in dir.flatten() {
        let path = ent.path();
        let typ = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if !typ.trim().eq_ignore_ascii_case("Battery") {
            continue;
        }
        let cap = std::fs::read_to_string(path.join("capacity")).ok()?;
        if let Ok(v) = cap.trim().parse::<u8>() {
            return Some(format!("Battery · {}%", v.min(100)));
        }
    }
    None
}

/// Quiet Wi-Fi / wired / offline glyph via nmcli or `/sys/class/net` (W169).
fn network_glyph_label() -> String {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["-t", "-f", "TYPE,STATE", "dev", "status"])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut wifi_conn = false;
            let mut wifi_avail = false;
            let mut wired_conn = false;
            for line in text.lines() {
                let mut parts = line.splitn(2, ':');
                let typ = parts.next().unwrap_or("").trim().to_ascii_lowercase();
                let state = parts.next().unwrap_or("").trim().to_ascii_lowercase();
                let connected = state.contains("connected") && !state.contains("disconnected");
                if typ.contains("wifi") || typ.contains("wlan") {
                    wifi_avail = true;
                    if connected {
                        wifi_conn = true;
                    }
                } else if typ.contains("ethernet") || typ.contains("wire") {
                    if connected {
                        wired_conn = true;
                    }
                }
            }
            if wifi_conn {
                return "Wi-Fi".into();
            }
            if wired_conn {
                return "Wired".into();
            }
            if wifi_avail {
                return "Wi-Fi · off".into();
            }
        }
    }
    // Fallback: /sys/class/net operstate (skip lo).
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else {
        return "Net · ?".into();
    };
    let mut wifi_up = false;
    let mut wired_up = false;
    let mut any_wifi = false;
    for ent in dir.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let path = ent.path();
        let oper = std::fs::read_to_string(path.join("operstate")).unwrap_or_default();
        let up = oper.trim().eq_ignore_ascii_case("up");
        let is_wifi = path.join("wireless").exists()
            || name.starts_with("wl")
            || name.starts_with("wlan");
        if is_wifi {
            any_wifi = true;
            if up {
                wifi_up = true;
            }
        } else if up {
            wired_up = true;
        }
    }
    if wifi_up {
        "Wi-Fi".into()
    } else if wired_up {
        "Wired".into()
    } else if any_wifi {
        "Wi-Fi · off".into()
    } else {
        "Offline".into()
    }
}

/// Best-effort `systemctl reboot|poweroff` from the greeter (W129). May need polkit.
fn systemctl_power(action: &str) -> String {
    match std::process::Command::new("systemctl").arg(action).output() {
        Ok(out) if out.status.success() => String::new(),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if err.is_empty() {
                format!("systemctl {action} failed")
            } else {
                err.chars().take(120).collect()
            }
        }
        Err(e) => format!("systemctl {action}: {e}"),
    }
}

/// PackageKit offline trigger then reboot (mode: reboot | power-off).
fn offline_update_power(mode: &str) -> String {
    let trigger = std::process::Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            "org.freedesktop.PackageKit",
            "--object-path",
            "/org/freedesktop/PackageKit",
            "--method",
            "org.freedesktop.PackageKit.Offline.Trigger",
            mode,
        ])
        .status();
    match trigger {
        Ok(s) if s.success() => systemctl_power("reboot"),
        Ok(_) => "Couldn’t prepare offline update".into(),
        Err(e) => format!("Offline update: {e}"),
    }
}
