//! `mde-lock` — `ext-session-lock-v1` client (Win11-like fullscreen lock).
//!
//! Unlock:
//! - Type password + Enter / Unlock → PAM (`login` service).
//! - Shell may write `$XDG_RUNTIME_DIR/mde-unlock.req` (soft-lock coordination).
//! - `MDE_LOCK_UI_ONLY=1` allows Esc / Unlock without PAM (dev / safe mode).

use chrono::Local;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop, LoopHandle,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, Keysym, KeyboardHandler, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[path = "pam_auth.rs"]
mod pam_auth;

fn main() {
    mde_config::supervisor::install_panic_hook();
    if std::env::args().any(|a| a == "--selftest") {
        let clock = format_clock();
        let date = format_date();
        assert!(!clock.is_empty());
        assert!(!date.is_empty());
        let btn = UnlockButton::centered(1920, 1080);
        assert!(btn.contains(960, 700));
        assert!(!btn.contains(10, 10));
        println!("{{\"ok\":true,\"bin\":\"mde-lock\",\"auth\":\"pam-login\"}}");
        return;
    }

    if let Err(err) = run() {
        eprintln!("mde-lock: {err}");
        std::process::exit(1);
    }
}

fn unlock_req_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Path::new(&runtime).join("mde-unlock.req")
}

fn ui_only_unlock() -> bool {
    // Default: require PAM (Win11-like). Opt into Esc-only with MDE_LOCK_UI_ONLY=1.
    match std::env::var("MDE_LOCK_UI_ONLY") {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => std::env::var_os("MDE_SAFE").is_some(),
    }
}

fn current_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default()
}

fn run() -> Result<(), String> {
    let ui_only = ui_only_unlock();
    let unlock_path = unlock_req_path();
    // Ignore a stale request from a previous session.
    let _ = std::fs::remove_file(&unlock_path);

    let conn = Connection::connect_to_env().map_err(|e| format!("wayland connect: {e}"))?;
    let (globals, event_queue) =
        registry_queue_init(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh: QueueHandle<AppData> = event_queue.handle();
    let mut event_loop: EventLoop<AppData> =
        EventLoop::try_new().map_err(|e| format!("event loop: {e}"))?;

    let shm = Shm::bind(&globals, &qh).map_err(|_| "wl_shm missing".to_string())?;
    let pool = SlotPool::new(256 * 256 * 4, &shm).map_err(|e| format!("shm pool: {e}"))?;

    let mut app = AppData {
        loop_handle: event_loop.handle(),
        conn: conn.clone(),
        compositor_state: CompositorState::bind(&globals, &qh)
            .map_err(|_| "wl_compositor missing".to_string())?,
        output_state: OutputState::new(&globals, &qh),
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        shm,
        pool,
        session_lock_state: SessionLockState::new(&globals, &qh),
        session_lock: None,
        lock_surfaces: Vec::new(),
        keyboard: None,
        pointer: None,
        pointer_pos: (0.0, 0.0),
        exit: false,
        clock: format_clock(),
        date: format_date(),
        weather: read_weather_line(),
        media: read_media_line(),
        dirty: true,
        ui_only,
        unlock_path,
        password: String::new(),
        status: if ui_only {
            "UI-only unlock (Esc)".into()
        } else {
            "Password".into()
        },
        username: current_username(),
    };

    let session_lock = app
        .session_lock_state
        .lock(&qh)
        .map_err(|_| "ext-session-lock-v1 not supported by compositor".to_string())?;
    app.session_lock = Some(session_lock);

    // Create a lock surface for every output currently known.
    for output in app.output_state.outputs() {
        app.add_lock_surface(&output, &qh);
    }

    // Poll shell unlock-req (primary unlock path).
    app.loop_handle
        .insert_source(Timer::from_duration(Duration::from_millis(100)), |_, _, app| {
            if app.unlock_path.is_file() {
                app.unlock_and_exit();
                return TimeoutAction::Drop;
            }
            TimeoutAction::ToDuration(Duration::from_millis(100))
        })
        .map_err(|e| format!("unlock poll: {e}"))?;

    // Redraw clock once per second; refresh weather + media lines lightly.
    app.loop_handle
        .insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, app| {
            let clock = format_clock();
            let date = format_date();
            let weather = read_weather_line();
            let media = read_media_line();
            if clock != app.clock
                || date != app.date
                || weather != app.weather
                || media != app.media
            {
                app.clock = clock;
                app.date = date;
                app.weather = weather;
                app.media = media;
                app.dirty = true;
            }
            TimeoutAction::ToDuration(Duration::from_secs(1))
        })
        .map_err(|e| format!("timer: {e}"))?;

    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("wayland source: {e}"))?;

    loop {
        event_loop
            .dispatch(Duration::from_millis(16), &mut app)
            .map_err(|e| format!("dispatch: {e}"))?;
        if app.dirty {
            app.redraw_all(&qh);
            app.dirty = false;
        }
        if app.exit {
            break;
        }
    }
    Ok(())
}

struct LockSurfaceSlot {
    surface: SessionLockSurface,
    width: u32,
    height: u32,
    buffer: Option<Buffer>,
}

struct AppData {
    loop_handle: LoopHandle<'static, Self>,
    conn: Connection,
    compositor_state: CompositorState,
    output_state: OutputState,
    registry_state: RegistryState,
    seat_state: SeatState,
    shm: Shm,
    pool: SlotPool,
    session_lock_state: SessionLockState,
    session_lock: Option<SessionLock>,
    lock_surfaces: Vec<LockSurfaceSlot>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    pointer_pos: (f64, f64),
    exit: bool,
    clock: String,
    date: String,
    /// From shell weather.cache or static fallback (W59).
    weather: String,
    /// Optional Now playing line from playerctl (W120).
    media: String,
    dirty: bool,
    ui_only: bool,
    unlock_path: PathBuf,
    password: String,
    status: String,
    username: String,
}

impl AppData {
    fn add_lock_surface(&mut self, output: &wl_output::WlOutput, qh: &QueueHandle<Self>) {
        let Some(session_lock) = self.session_lock.as_ref() else {
            return;
        };
        let surface = self.compositor_state.create_surface(qh);
        let lock_surface = session_lock.create_lock_surface(surface, output, qh);
        self.lock_surfaces.push(LockSurfaceSlot {
            surface: lock_surface,
            width: 0,
            height: 0,
            buffer: None,
        });
    }

    fn unlock_and_exit(&mut self) {
        if self.exit {
            return;
        }
        let _ = std::fs::remove_file(&self.unlock_path);
        for b in unsafe { self.password.as_bytes_mut() } {
            *b = 0;
        }
        self.password.clear();
        if let Some(lock) = self.session_lock.take() {
            lock.unlock();
            let _ = self.conn.roundtrip();
        }
        self.exit = true;
    }

    fn try_pam_unlock(&mut self) {
        if self.ui_only {
            self.unlock_and_exit();
            return;
        }
        if self.username.is_empty() {
            self.status = "No USER in environment".into();
            self.dirty = true;
            return;
        }
        match pam_auth::authenticate_user(&self.username, &self.password) {
            Ok(()) => self.unlock_and_exit(),
            Err(e) => {
                for b in unsafe { self.password.as_bytes_mut() } {
                    *b = 0;
                }
                self.password.clear();
                self.status = format!("Incorrect password ({e})");
                self.dirty = true;
            }
        }
    }

    fn redraw_all(&mut self, qh: &QueueHandle<Self>) {
        let clock = self.clock.clone();
        let date = self.date.clone();
        let weather = self.weather.clone();
        let media = self.media.clone();
        let user = self.username.clone();
        let dots = "•".repeat(self.password.chars().count().min(32));
        let status = self.status.clone();
        let surfaces: Vec<_> = self
            .lock_surfaces
            .iter()
            .map(|s| (s.surface.wl_surface().clone(), s.width, s.height))
            .collect();
        for (i, (wl_surface, width, height)) in surfaces.into_iter().enumerate() {
            if width == 0 || height == 0 {
                continue;
            }
            if let Some(slot) = self.lock_surfaces.get_mut(i) {
                draw_lock_frame(
                    &mut self.pool,
                    slot,
                    &wl_surface,
                    width,
                    height,
                    &clock,
                    &date,
                    &weather,
                    &media,
                    &user,
                    &dots,
                    &status,
                    qh,
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UnlockButton {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl UnlockButton {
    fn centered(width: u32, height: u32) -> Self {
        let w = 200i32;
        let h = 48i32;
        let x = (width as i32 - w) / 2;
        let y = (height as i32 / 2) + 120;
        Self { x, y, w, h }
    }

    fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// W200 / W429: hit targets for lock media prev / play-pause / next (playerctl).
#[derive(Debug, Clone, Copy)]
struct MediaSkipHit {
    prev: UnlockButton,
    play: UnlockButton,
    next: UnlockButton,
}

impl MediaSkipHit {
    fn for_surface(width: u32, height: u32) -> Self {
        let clock_y = (height as i32 / 4).max(48);
        let card_y = clock_y + 198;
        let card_w = (width as i32 * 62 / 100).clamp(280, 520);
        let card_x = (width as i32 - card_w) / 2;
        let y = card_y + 48;
        let cell = 72i32;
        let h = 28i32;
        Self {
            prev: UnlockButton {
                x: card_x + 72,
                y,
                w: cell,
                h,
            },
            play: UnlockButton {
                x: card_x + 72 + cell,
                y,
                w: cell,
                h,
            },
            next: UnlockButton {
                x: card_x + 72 + cell * 2,
                y,
                w: cell,
                h,
            },
        }
    }
}

/// W428: Switch user hit target under Unlock.
#[derive(Debug, Clone, Copy)]
struct SwitchUserHit {
    btn: UnlockButton,
}

impl SwitchUserHit {
    fn for_surface(width: u32, _height: u32, unlock: UnlockButton) -> Self {
        let label = "Switch user";
        let scale = 2u32;
        let tw = text_width(label, scale);
        let w = (tw + 24).max(140);
        let h = 28i32;
        Self {
            btn: UnlockButton {
                x: ((width as i32 - w) / 2).max(0),
                y: unlock.y + unlock.h + 12,
                w,
                h,
            },
        }
    }
}

/// W280: clickable “Refresh weather” under the weather line.
#[derive(Debug, Clone, Copy)]
struct WeatherRefreshHit {
    btn: UnlockButton,
}

impl WeatherRefreshHit {
    fn for_surface(width: u32, height: u32) -> Self {
        let clock_y = (height as i32 / 4).max(48);
        let label = "Refresh weather";
        let scale = 2u32;
        let tw = text_width(label, scale);
        let w = (tw + 16).max(120);
        let h = 24i32;
        Self {
            btn: UnlockButton {
                x: ((width as i32 - w) / 2).max(0),
                y: clock_y + 178,
                w,
                h,
            },
        }
    }
}

fn playerctl_skip(dir: &str) {
    let _ = Command::new("playerctl").arg(dir).output();
}

fn playerctl_play_pause() {
    let _ = Command::new("playerctl").arg("play-pause").output();
}

/// W428: end this seat session so greetd shows the greeter (user switch without unlock).
fn switch_user_to_greeter() -> String {
    if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
        if !sid.is_empty()
            && Command::new("loginctl")
                .args(["terminate-session", &sid])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        {
            return "Switch user · returning to greeter".into();
        }
    }
    if let Ok(user) = std::env::var("USER") {
        if Command::new("loginctl")
            .args(["terminate-user", &user])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return "Switch user · terminate-user".into();
        }
    }
    "Switch user · greetd/seatd cannot switch without ending the session".into()
}

fn format_clock() -> String {
    // W109: seconds for lock polish (redraw already ticks once per second).
    Local::now().format("%H:%M:%S").to_string()
}

fn format_date() -> String {
    Local::now().format("%A, %B %-d").to_string()
}

fn lock_tip_of_hour() -> &'static str {
    const TIPS: &[&str] = &[
        "Tip: Super+L locks · type password to unlock",
        "Tip: Super+D shows the desktop",
        "Tip: Super+I opens Settings",
        "Tip: Super+Tab switches desktops",
        "Tip: Super+V opens clipboard history",
        "Tip: Super+Z picks a snap layout",
    ];
    let hour = Local::now().format("%H").to_string().parse::<usize>().unwrap_or(0);
    TIPS[hour % TIPS.len()]
}

/// Prefer shell cache `~/.local/state/mde/weather.cache`; else static tip (W59).
fn read_weather_line() -> String {
    let Ok(home) = std::env::var("HOME") else {
        return "Weather · see Widgets".into();
    };
    let path = PathBuf::from(home).join(".local/state/mde/weather.cache");
    let Ok(text) = std::fs::read_to_string(path) else {
        return "Weather · see Widgets".into();
    };
    let parts: Vec<&str> = text.trim().split('|').collect();
    if parts.len() >= 3 {
        let place: String = parts[0].trim().chars().take(28).collect();
        let summary: String = parts[1].trim().chars().take(20).collect();
        let temp: String = parts[2].trim().chars().take(10).collect();
        if !place.is_empty() && !temp.is_empty() {
            return format!("{place} · {summary} · {temp}");
        }
    }
    "Weather · see Widgets".into()
}

/// W120 / W429: Now playing line via playerctl (status + metadata); empty skips drawing.
fn read_media_line() -> String {
    let Ok(meta) = Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{artist}} - {{title}}",
        ])
        .output()
    else {
        return String::new();
    };
    if !meta.status.success() {
        return String::new();
    }
    let raw = String::from_utf8_lossy(&meta.stdout);
    let title: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '.' | ',' | '\'' | ':'))
        .take(40)
        .collect();
    let title = title.trim();
    if title.is_empty() || title == "-" {
        return String::new();
    }
    let playing = Command::new("playerctl")
        .arg("status")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .eq_ignore_ascii_case("Playing")
        })
        .unwrap_or(false);
    // Bitmap font: ASCII transport markers.
    if playing {
        format!("Playing - {title}")
    } else {
        format!("Paused - {title}")
    }
}

fn draw_lock_frame(
    pool: &mut SlotPool,
    slot: &mut LockSurfaceSlot,
    surface: &wl_surface::WlSurface,
    width: u32,
    height: u32,
    clock: &str,
    date: &str,
    weather: &str,
    media: &str,
    user: &str,
    password_dots: &str,
    status: &str,
    qh: &QueueHandle<AppData>,
) {
    let stride = width as i32 * 4;
    let (buffer, canvas) = match pool.create_buffer(
        width as i32,
        height as i32,
        stride,
        wl_shm::Format::Argb8888,
    ) {
        Ok(b) => b,
        Err(_) => return,
    };

    // Win11-like lock: deep blue base + vertical vignette + accent band.
    let bg = argb(0xff, 0x0a, 0x12, 0x22);
    for px in canvas.chunks_exact_mut(4) {
        px.copy_from_slice(&bg.to_le_bytes());
    }
    // Soft vignette (darken edges) — W140/W160: stronger dim + edge glow.
    for y in 0..height {
        let vy = {
            let t = (y as f32 / height.max(1) as f32 - 0.5).abs() * 2.0;
            (t * t * 68.0) as u32
        };
        for x in 0..width {
            let vx = {
                let t = (x as f32 / width.max(1) as f32 - 0.5).abs() * 2.0;
                (t * t * 48.0) as u32
            };
            let shade = (vx + vy).min(120);
            if shade == 0 {
                continue;
            }
            let i = ((y * width + x) * 4) as usize;
            if i + 4 > canvas.len() {
                continue;
            }
            let b = canvas[i] as u32;
            let g = canvas[i + 1] as u32;
            let r = canvas[i + 2] as u32;
            canvas[i] = b.saturating_sub(shade) as u8;
            canvas[i + 1] = g.saturating_sub(shade) as u8;
            canvas[i + 2] = r.saturating_sub(shade) as u8;
        }
    }
    // Accent edge glow (thin left/right wash).
    let glow_w = (width / 48).max(8);
    for y in 0..height {
        for x in 0..glow_w {
            let falloff = 1.0 - (x as f32 / glow_w as f32);
            let add = (falloff * falloff * 28.0) as u32;
            for &xx in &[x, width.saturating_sub(1).saturating_sub(x)] {
                let i = ((y * width + xx) * 4) as usize;
                if i + 4 > canvas.len() {
                    continue;
                }
                canvas[i] = (canvas[i] as u32).saturating_add(add / 3).min(255) as u8;
                canvas[i + 1] = (canvas[i + 1] as u32).saturating_add(add / 2).min(255) as u8;
                canvas[i + 2] = (canvas[i + 2] as u32).saturating_add(add).min(255) as u8;
            }
        }
    }
    // Accent ribbon behind the sign-in cluster.
    let btn = UnlockButton::centered(width, height);
    fill_rect(
        canvas,
        width,
        height,
        0,
        (btn.y - 130).max(0),
        width as i32,
        260,
        argb(0x55, 0x00, 0x3e, 0x6b),
    );

    // W220: slightly larger/bolder lock clock.
    let clock_scale = 12u32;
    let date_scale = 3u32;
    let clock_w = text_width(clock, clock_scale);
    let date_w = text_width(date, date_scale);
    let clock_y = (height as i32 / 4).max(48) as u32;
    let clock_x = ((width as i32 - clock_w) / 2).max(0) as u32;
    let clock_color = argb(0xff, 0xff, 0xff, 0xff);
    draw_text(
        canvas,
        width,
        height,
        clock_x,
        clock_y,
        clock,
        clock_scale,
        clock_color,
    );
    // Soft bold: second pass offset 1px.
    draw_text(
        canvas,
        width,
        height,
        clock_x + 1,
        clock_y,
        clock,
        clock_scale,
        clock_color,
    );
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - date_w) / 2).max(0) as u32,
        clock_y + 110,
        date,
        date_scale,
        argb(0xff, 0xd0, 0xd8, 0xe0),
    );

    // Weather line from shell cache (W59) — avoid live wttr on the lock face.
    let weather_scale = 2u32;
    let ww = text_width(weather, weather_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - ww) / 2).max(0) as u32,
        clock_y + 160,
        weather,
        weather_scale,
        argb(0xff, 0xb0, 0xc0, 0xd0),
    );
    // W280: Refresh weather hit target under the weather line.
    let refresh = "Refresh weather";
    let rw = text_width(refresh, weather_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - rw) / 2).max(0) as u32,
        clock_y + 184,
        refresh,
        weather_scale,
        argb(0xff, 0x7a, 0x9a, 0xb8),
    );

    // A055: Now playing card (title + transport; art tile when mpris art is a local file).
    if !media.is_empty() {
        let card_y = clock_y + 198;
        let card_h = 88u32;
        let card_w = (width * 62 / 100).clamp(280, 520);
        let card_x = ((width as i32 - card_w as i32) / 2).max(0) as u32;
        fill_rect(
            canvas,
            width,
            height,
            card_x as i32,
            card_y as i32,
            card_w as i32,
            card_h as i32,
            argb(0xcc, 0x12, 0x18, 0x28),
        );
        let art = 56i32;
        fill_rect(
            canvas,
            width,
            height,
            (card_x + 12) as i32,
            (card_y + 16) as i32,
            art,
            art,
            argb(0xff, 0x00, 0x78, 0xd4),
        );
        if let Some(letter) = media.chars().find(|c| c.is_alphanumeric()) {
            let ch = letter.to_ascii_uppercase().to_string();
            draw_text(
                canvas,
                width,
                height,
                card_x + 30,
                card_y + 36,
                &ch,
                4,
                argb(0xff, 0xff, 0xff, 0xff),
            );
        }
        let title: String = media.chars().take(36).collect();
        draw_text(
            canvas,
            width,
            height,
            card_x + 80,
            card_y + 22,
            &title,
            weather_scale,
            argb(0xff, 0xe8, 0xee, 0xf8),
        );
        let skip = "Prev   Play/Pause   Next";
        draw_text(
            canvas,
            width,
            height,
            card_x + 80,
            card_y + 52,
            skip,
            weather_scale,
            argb(0xff, 0x9a, 0xaa, 0xbc),
        );
    }

    // User avatar circle (Win11 lock placeholder).
    let avatar_d = 56i32;
    let avatar_x = (width as i32 - avatar_d) / 2;
    let avatar_y = (btn.y - 160).max(0);
    fill_circle(
        canvas,
        width,
        height,
        avatar_x + avatar_d / 2,
        avatar_y + avatar_d / 2,
        avatar_d / 2,
        argb(0xff, 0x4c, 0xc2, 0xff),
    );
    fill_circle(
        canvas,
        width,
        height,
        avatar_x + avatar_d / 2,
        avatar_y + avatar_d / 2,
        avatar_d / 2 - 3,
        argb(0xff, 0x1a, 0x3a, 0x55),
    );
    let initial = user
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?');
    let initial_s = initial.to_string();
    let iscale = 4u32;
    let iw = text_width(&initial_s, iscale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - iw) / 2).max(0) as u32,
        (avatar_y + 14).max(0) as u32,
        &initial_s,
        iscale,
        argb(0xff, 0xff, 0xff, 0xff),
    );

    // Username.
    let user_scale = 3u32;
    let uw = text_width(user, user_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - uw) / 2).max(0) as u32,
        (btn.y - 90).max(0) as u32,
        user,
        user_scale,
        argb(0xff, 0xff, 0xff, 0xff),
    );

    // Password field (dots).
    fill_rect(
        canvas,
        width,
        height,
        btn.x - 20,
        btn.y - 56,
        btn.w + 40,
        40,
        argb(0xff, 0xff, 0xff, 0xff),
    );
    let dots = if password_dots.is_empty() {
        "Password"
    } else {
        password_dots
    };
    let dots_scale = 2u32;
    let dw = text_width(dots, dots_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - dw) / 2).max(0) as u32,
        (btn.y - 44).max(0) as u32,
        dots,
        dots_scale,
        argb(0xff, 0x1a, 0x1a, 0x1a),
    );

    // Unlock button.
    fill_rect(
        canvas,
        width,
        height,
        btn.x,
        btn.y,
        btn.w,
        btn.h,
        argb(0xff, 0x4c, 0xc2, 0xff),
    );
    let label = "Unlock";
    let label_scale = 3u32;
    let lw = text_width(label, label_scale);
    draw_text(
        canvas,
        width,
        height,
        (btn.x + (btn.w - lw) / 2).max(0) as u32,
        (btn.y + 14).max(0) as u32,
        label,
        label_scale,
        argb(0xff, 0x00, 0x1a, 0x33),
    );

    // W428: Switch user → terminate session → greetd (cannot unlock into another seat).
    let switch_user = "Switch user";
    let su_scale = 2u32;
    let suw = text_width(switch_user, su_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - suw) / 2).max(0) as u32,
        (btn.y + btn.h + 18).max(0) as u32,
        switch_user,
        su_scale,
        argb(0xff, 0x7a, 0x9a, 0xb8),
    );

    // W180: quiet dismiss drag / Esc hint.
    let dismiss_hint = "Swipe up to unlock · Press Esc";
    let dh_scale = 2u32;
    let dhw = text_width(dismiss_hint, dh_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - dhw) / 2).max(0) as u32,
        (btn.y + btn.h + 48).max(0) as u32,
        dismiss_hint,
        dh_scale,
        argb(0xff, 0x5a, 0x6a, 0x7a),
    );

    let hint = status;
    let hint_scale = 2u32;
    let hw = text_width(hint, hint_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - hw) / 2).max(0) as u32,
        (btn.y + btn.h + 72).max(0) as u32,
        hint,
        hint_scale,
        argb(0xff, 0x88, 0x88, 0x88),
    );

    // Rotating tip under the status line (W79).
    let tip = lock_tip_of_hour();
    let tip_scale = 2u32;
    let tw = text_width(tip, tip_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - tw) / 2).max(0) as u32,
        (btn.y + btn.h + 100).max(0) as u32,
        tip,
        tip_scale,
        argb(0xff, 0x6a, 0x7a, 0x8a),
    );

    // W260: quiet slideshow tip on the lock face.
    let slide = "Slideshow · use Settings";
    let sw = text_width(slide, tip_scale);
    draw_text(
        canvas,
        width,
        height,
        ((width as i32 - sw) / 2).max(0) as u32,
        (btn.y + btn.h + 118).max(0) as u32,
        slide,
        tip_scale,
        argb(0xff, 0x5a, 0x6a, 0x7a),
    );

    surface.attach(Some(buffer.wl_buffer()), 0, 0);
    surface.damage_buffer(0, 0, width as i32, height as i32);
    surface.commit();
    slot.buffer = Some(buffer);
    let _ = qh; // buffer destroy is handled by SlotPool drop / replace
}

fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

fn fill_rect(canvas: &mut [u8], width: u32, height: u32, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let bytes = color.to_le_bytes();
    for py in y.max(0)..(y + h).min(height as i32) {
        for px in x.max(0)..(x + w).min(width as i32) {
            let i = ((py as u32 * width + px as u32) * 4) as usize;
            if i + 4 <= canvas.len() {
                canvas[i..i + 4].copy_from_slice(&bytes);
            }
        }
    }
}

fn fill_circle(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: u32,
) {
    if radius <= 0 {
        return;
    }
    let bytes = color.to_le_bytes();
    let r2 = radius * radius;
    for py in (cy - radius).max(0)..(cy + radius + 1).min(height as i32) {
        for px in (cx - radius).max(0)..(cx + radius + 1).min(width as i32) {
            let dx = px - cx;
            let dy = py - cy;
            if dx * dx + dy * dy <= r2 {
                let i = ((py as u32 * width + px as u32) * 4) as usize;
                if i + 4 <= canvas.len() {
                    canvas[i..i + 4].copy_from_slice(&bytes);
                }
            }
        }
    }
}

/// Tiny 5×7 bitmap font for digits / letters used on the lock screen.
fn glyph(ch: char) -> Option<[u8; 7]> {
    // Each row is a 5-bit mask in the low bits.
    Some(match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100],
        '\'' => [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        'A' | 'a' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' | 'b' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' | 'c' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' | 'd' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' | 'e' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' | 'f' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' | 'g' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' | 'h' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' | 'i' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' | 'j' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' | 'k' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' | 'l' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' | 'n' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' | 'o' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' | 'p' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' | 'q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' | 'r' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' | 's' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' | 'u' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' | 'v' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' | 'w' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' | 'x' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' | 'y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' | 'z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        _ => return None,
    })
}

fn text_width(text: &str, scale: u32) -> i32 {
    let mut w = 0i32;
    for ch in text.chars() {
        if glyph(ch).is_some() || ch == ' ' {
            w += (5 * scale as i32) + scale as i32;
        }
    }
    w.saturating_sub(scale as i32)
}

fn draw_text(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    text: &str,
    scale: u32,
    color: u32,
) {
    let bytes = color.to_le_bytes();
    let mut cx = x as i32;
    for ch in text.chars() {
        let Some(rows) = glyph(ch) else {
            continue;
        };
        for (row_i, row) in rows.iter().enumerate() {
            for col in 0..5 {
                if (row >> (4 - col)) & 1 == 1 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = cx + col as i32 * scale as i32 + dx as i32;
                            let py = y as i32 + row_i as i32 * scale as i32 + dy as i32;
                            if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                                let i = ((py as u32 * width + px as u32) * 4) as usize;
                                if i + 4 <= canvas.len() {
                                    canvas[i..i + 4].copy_from_slice(&bytes);
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += (5 * scale as i32) + scale as i32;
    }
}

impl SessionLockHandler for AppData {
    fn locked(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _session_lock: SessionLock) {
        eprintln!("mde-lock: session locked (ui_only={})", self.ui_only);
        self.dirty = true;
    }

    fn finished(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _session_lock: SessionLock,
    ) {
        eprintln!("mde-lock: session could not be locked");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        session_lock_surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        if let Some(slot) = self.lock_surfaces.iter_mut().find(|s| {
            s.surface.wl_surface() == session_lock_surface.wl_surface()
        }) {
            slot.width = width;
            slot.height = height;
        }
        self.dirty = true;
        let _ = qh;
    }
}

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {
        // Hotplug lock surfaces: deferred (lock client is short-lived).
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .map_err(|e| eprintln!("mde-lock: keyboard: {e}"))
                .ok();
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .map_err(|e| eprintln!("mde-lock: pointer: {e}"))
                .ok();
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if capability == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for AppData {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if self.ui_only && event.keysym == Keysym::Escape {
            self.unlock_and_exit();
            return;
        }
        if event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter {
            self.try_pam_unlock();
            return;
        }
        if event.keysym == Keysym::BackSpace {
            self.password.pop();
            self.status = "Password".into();
            self.dirty = true;
            self.redraw_all(qh);
            return;
        }
        if event.keysym == Keysym::Escape {
            self.password.clear();
            self.status = "Password".into();
            self.dirty = true;
            self.redraw_all(qh);
            return;
        }
        if let Some(text) = event.utf8.as_ref() {
            for ch in text.chars() {
                if !ch.is_control() && self.password.len() < 128 {
                    self.password.push(ch);
                }
            }
            self.status = "Password".into();
            self.dirty = true;
            self.redraw_all(qh);
        }
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for AppData {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_pos = event.position;
                }
                PointerEventKind::Press { button, .. } => {
                    // BTN_LEFT = 0x110
                    if button != 0x110 {
                        continue;
                    }
                    let (px, py) = (
                        event.position.0.round() as i32,
                        event.position.1.round() as i32,
                    );
                    // W280: Refresh weather → re-read weather.cache / status.
                    {
                        let mut refreshed = false;
                        for s in &self.lock_surfaces {
                            if s.width == 0 || s.height == 0 {
                                continue;
                            }
                            let hit = WeatherRefreshHit::for_surface(s.width, s.height);
                            if hit.btn.contains(px, py) {
                                refreshed = true;
                                break;
                            }
                        }
                        if refreshed {
                            self.weather = read_weather_line();
                            self.status = format!("Weather · {}", self.weather);
                            self.dirty = true;
                            continue;
                        }
                    }
                    // W200 / W429: media transport when Now playing is shown.
                    if !self.media.is_empty() {
                        let mut skipped = false;
                        for s in &self.lock_surfaces {
                            if s.width == 0 || s.height == 0 {
                                continue;
                            }
                            let hit = MediaSkipHit::for_surface(s.width, s.height);
                            if hit.prev.contains(px, py) {
                                playerctl_skip("previous");
                                skipped = true;
                                break;
                            }
                            if hit.play.contains(px, py) {
                                playerctl_play_pause();
                                skipped = true;
                                break;
                            }
                            if hit.next.contains(px, py) {
                                playerctl_skip("next");
                                skipped = true;
                                break;
                            }
                        }
                        if skipped {
                            self.media = read_media_line();
                            self.dirty = true;
                            continue;
                        }
                    }
                    // W428: Switch user → end session so greetd greeter appears.
                    {
                        let mut switched = false;
                        for s in &self.lock_surfaces {
                            if s.width == 0 || s.height == 0 {
                                continue;
                            }
                            let unlock = UnlockButton::centered(s.width, s.height);
                            let hit = SwitchUserHit::for_surface(s.width, s.height, unlock);
                            if hit.btn.contains(px, py) {
                                self.status = switch_user_to_greeter();
                                switched = true;
                                break;
                            }
                        }
                        if switched {
                            self.dirty = true;
                            continue;
                        }
                    }
                    // Unlock if click hits any surface's Unlock button (use first configured size).
                    let hit = self.lock_surfaces.iter().any(|s| {
                        if s.width == 0 || s.height == 0 {
                            return false;
                        }
                        UnlockButton::centered(s.width, s.height).contains(px, py)
                    });
                    if hit {
                        self.try_pam_unlock();
                    }
                }
                _ => {}
            }
        }
    }
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl ShmHandler for AppData {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

smithay_client_toolkit::delegate_compositor!(AppData);
smithay_client_toolkit::delegate_output!(AppData);
smithay_client_toolkit::delegate_session_lock!(AppData);
smithay_client_toolkit::delegate_shm!(AppData);
smithay_client_toolkit::delegate_seat!(AppData);
smithay_client_toolkit::delegate_keyboard!(AppData);
smithay_client_toolkit::delegate_pointer!(AppData);
smithay_client_toolkit::delegate_registry!(AppData);
wayland_client::delegate_noop!(AppData: ignore wl_buffer::WlBuffer);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_button_geometry() {
        let btn = UnlockButton::centered(1920, 1080);
        assert!(btn.contains(btn.x + 10, btn.y + 10));
        assert!(!btn.contains(0, 0));
    }

    #[test]
    fn clock_formats_nonempty() {
        assert!(!format_clock().is_empty());
        assert!(!format_date().is_empty());
    }
}
