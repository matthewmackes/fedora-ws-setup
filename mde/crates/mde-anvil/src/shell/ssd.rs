use smithay::{
    backend::renderer::{
        element::{
            solid::{SolidColorBuffer, SolidColorRenderElement},
            AsRenderElements, Kind,
        },
        Renderer,
    },
    desktop::WindowSurface,
    input::Seat,
    utils::{Logical, Point, Serial},
};

use std::cell::{RefCell, RefMut};

use crate::{state::Backend, AnvilState};

use super::WindowElement;

pub struct WindowState {
    pub is_ssd: bool,
    pub header_bar: HeaderBar,
}

pub struct HeaderBar {
    pub pointer_loc: Option<Point<f64, Logical>>,
    pub width: u32,
    pub maximized: bool,
    /// A018 title-bar pin when always-on-top.
    pub always_on_top: bool,
    pub minimize_button_hover: bool,
    pub maximize_button_hover: bool,
    pub close_button_hover: bool,
    /// Last painted corner radius (A050).
    pub corner_radius: u8,
    pub background: SolidColorBuffer,
    /// Top hairline (Win11-closer: subtle highlight, not a thick underline).
    pub accent_line: SolidColorBuffer,
    pub minimize_bg: SolidColorBuffer,
    pub maximize_bg: SolidColorBuffer,
    pub close_bg: SolidColorBuffer,
    /// Glyph segments as solid rects (keeps decorations on SolidColorRenderElement).
    glyph_parts: Vec<(Point<i32, Logical>, SolidColorBuffer)>,
}

struct HeaderPaint {
    bg: [f32; 4],
    accent: [f32; 4],
    glyph: [f32; 4],
    hover: [f32; 4],
    clear: [f32; 4],
    corner_radius: u8,
}

fn header_paint() -> HeaderPaint {
    let palette = mde_theme::Palette::load();
    let base = mde_theme::Palette::channels(&palette.bg_elevated).unwrap_or([0.17, 0.17, 0.17, 1.0]);
    let accent = mde_theme::Palette::channels(&palette.accent).unwrap_or([0.0, 0.47, 0.83, 1.0]);
    let bg = if palette.accent_on_titlebars {
        [
            base[0] * 0.92 + accent[0] * 0.08,
            base[1] * 0.92 + accent[1] * 0.08,
            base[2] * 0.92 + accent[2] * 0.08,
            1.0,
        ]
    } else {
        base
    };
    let mut bg = bg;
    if palette.ssd_mica && palette.transparency {
        bg[3] = (palette.opacity * 0.82).clamp(0.55, 0.92);
    }
    let hairline = if palette.accent_on_titlebars {
        [
            (base[0] * 0.55 + accent[0] * 0.45).min(1.0),
            (base[1] * 0.55 + accent[1] * 0.45).min(1.0),
            (base[2] * 0.55 + accent[2] * 0.45).min(1.0),
            1.0,
        ]
    } else if palette.dark {
        [
            (base[0] + 0.08).min(1.0),
            (base[1] + 0.08).min(1.0),
            (base[2] + 0.08).min(1.0),
            1.0,
        ]
    } else {
        [
            (base[0] + 0.06).min(1.0),
            (base[1] + 0.06).min(1.0),
            (base[2] + 0.06).min(1.0),
            1.0,
        ]
    };
    let glyph = if palette.dark {
        [0.92, 0.92, 0.92, 1.0]
    } else {
        [0.12, 0.12, 0.12, 1.0]
    };
    let hover = [
        (accent[0] * 0.55 + base[0] * 0.45).min(1.0),
        (accent[1] * 0.55 + base[1] * 0.45).min(1.0),
        (accent[2] * 0.55 + base[2] * 0.45).min(1.0),
        1.0,
    ];
    HeaderPaint {
        bg,
        accent: hairline,
        glyph,
        hover,
        clear: [0.0, 0.0, 0.0, 0.0],
        corner_radius: palette.ssd_corner_radius,
    }
}

pub const HEADER_BAR_HEIGHT: i32 = 32;
const ACCENT_LINE_HEIGHT: i32 = 1;
const BUTTON_HEIGHT: u32 = HEADER_BAR_HEIGHT as u32;
const BUTTON_WIDTH: u32 = 46;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptionButton {
    Minimize,
    Maximize,
    Close,
}

impl HeaderBar {
    pub fn pointer_enter(&mut self, loc: Point<f64, Logical>) {
        self.pointer_loc = Some(loc);
    }

    pub fn pointer_leave(&mut self) {
        self.pointer_loc = None;
    }

    fn hit_button(&self, x: f64) -> Option<CaptionButton> {
        let w = self.width as f64;
        let bw = BUTTON_WIDTH as f64;
        if x >= w - bw {
            Some(CaptionButton::Close)
        } else if x >= w - bw * 2.0 {
            Some(CaptionButton::Maximize)
        } else if x >= w - bw * 3.0 {
            Some(CaptionButton::Minimize)
        } else {
            None
        }
    }

    /// True when the pointer is over min/max/close (W389).
    pub fn on_caption_button(&self) -> bool {
        self.pointer_loc
            .as_ref()
            .and_then(|loc| self.hit_button(loc.x))
            .is_some()
    }

    pub fn clicked<BackendData: Backend>(
        &mut self,
        seat: &Seat<AnvilState<BackendData>>,
        state: &mut AnvilState<BackendData>,
        window: &WindowElement,
        serial: Serial,
    ) {
        match self.pointer_loc.as_ref().and_then(|loc| self.hit_button(loc.x)) {
            Some(CaptionButton::Close) => match window.0.underlying_surface() {
                WindowSurface::Wayland(w) => w.send_close(),
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(w) => {
                    let _ = w.close();
                }
            },
            Some(CaptionButton::Maximize) => {
                state.caption_toggle_maximize(window);
            }
            Some(CaptionButton::Minimize) => {
                state.caption_minimize(window);
            }
            None => match window.0.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    let seat = seat.clone();
                    let toplevel = w.clone();
                    state
                        .handle
                        .insert_idle(move |data| data.move_request_xdg(&toplevel, &seat, serial));
                }
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(w) => {
                    let window = w.clone();
                    state
                        .handle
                        .insert_idle(move |data| data.move_request_x11(&window));
                }
            },
        }
    }

    pub fn touch_down<BackendData: Backend>(
        &mut self,
        seat: &Seat<AnvilState<BackendData>>,
        state: &mut AnvilState<BackendData>,
        window: &WindowElement,
        serial: Serial,
    ) {
        if self
            .pointer_loc
            .as_ref()
            .and_then(|loc| self.hit_button(loc.x))
            .is_some()
        {
            return;
        }
        match window.0.underlying_surface() {
            WindowSurface::Wayland(w) => {
                let seat = seat.clone();
                let toplevel = w.clone();
                state
                    .handle
                    .insert_idle(move |data| data.move_request_xdg(&toplevel, &seat, serial));
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(w) => {
                let window = w.clone();
                state
                    .handle
                    .insert_idle(move |data| data.move_request_x11(&window));
            }
        }
    }

    pub fn touch_up<BackendData: Backend>(
        &mut self,
        _seat: &Seat<AnvilState<BackendData>>,
        state: &mut AnvilState<BackendData>,
        window: &WindowElement,
        _serial: Serial,
    ) {
        match self.pointer_loc.as_ref().and_then(|loc| self.hit_button(loc.x)) {
            Some(CaptionButton::Close) => match window.0.underlying_surface() {
                WindowSurface::Wayland(w) => w.send_close(),
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(w) => {
                    let _ = w.close();
                }
            },
            Some(CaptionButton::Maximize) => {
                state.caption_toggle_maximize(window);
            }
            Some(CaptionButton::Minimize) => {
                state.caption_minimize(window);
            }
            None => {}
        }
    }

    pub fn redraw(&mut self, width: u32, maximized: bool, always_on_top: bool) {
        if width == 0 {
            self.width = 0;
            self.glyph_parts.clear();
            return;
        }

        let paint = header_paint();
        let inset = (paint.corner_radius as i32).min((width / 4) as i32).max(0);
        let bar_w = width.saturating_sub((inset * 2) as u32).max(1);
        self.background
            .update((bar_w as i32, HEADER_BAR_HEIGHT), paint.bg);
        self.accent_line
            .update((bar_w as i32, ACCENT_LINE_HEIGHT), paint.accent);

        let min_hover = self
            .pointer_loc
            .as_ref()
            .map(|l| self.hit_button(l.x) == Some(CaptionButton::Minimize))
            .unwrap_or(false);
        let max_hover = self
            .pointer_loc
            .as_ref()
            .map(|l| self.hit_button(l.x) == Some(CaptionButton::Maximize))
            .unwrap_or(false);
        let close_hover = self
            .pointer_loc
            .as_ref()
            .map(|l| self.hit_button(l.x) == Some(CaptionButton::Close))
            .unwrap_or(false);

        let needs_buttons = width != self.width
            || maximized != self.maximized
            || always_on_top != self.always_on_top
            || paint.corner_radius != self.corner_radius
            || min_hover != self.minimize_button_hover
            || max_hover != self.maximize_button_hover
            || close_hover != self.close_button_hover;

        self.width = width;
        self.maximized = maximized;
        self.always_on_top = always_on_top;
        self.corner_radius = paint.corner_radius;
        self.minimize_button_hover = min_hover;
        self.maximize_button_hover = max_hover;
        self.close_button_hover = close_hover;

        if !needs_buttons {
            return;
        }

        let bw = BUTTON_WIDTH as i32;
        let bh = BUTTON_HEIGHT as i32;
        self.minimize_bg.update(
            (bw, bh),
            if min_hover { paint.hover } else { paint.clear },
        );
        self.maximize_bg.update(
            (bw, bh),
            if max_hover { paint.hover } else { paint.clear },
        );
        self.close_bg.update(
            (bw, bh),
            if close_hover { paint.hover } else { paint.clear },
        );

        self.glyph_parts.clear();
        if inset > 0 {
            push_rounded_top_corners(
                &mut self.glyph_parts,
                inset,
                paint.corner_radius,
                paint.bg,
            );
        }
        let min_origin = Point::from((width as i32 - bw * 3, 0));
        let max_origin = Point::from((width as i32 - bw * 2, 0));
        let close_origin = Point::from((width as i32 - bw, 0));
        push_minimize_glyph(&mut self.glyph_parts, min_origin, paint.glyph);
        if maximized {
            push_restore_glyph(&mut self.glyph_parts, max_origin, paint.glyph);
        } else {
            push_maximize_glyph(&mut self.glyph_parts, max_origin, paint.glyph);
        }
        push_close_glyph(&mut self.glyph_parts, close_origin, paint.glyph);
        // A018: pin glyph left of caption buttons when always-on-top.
        if always_on_top {
            let pin_origin = Point::from((width as i32 - bw * 4, 0));
            push_aot_pin_glyph(&mut self.glyph_parts, pin_origin, paint.accent);
        }
    }
}

/// A050: approximate top corner rounding via stepped cut-ins.
fn push_rounded_top_corners(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    inset: i32,
    radius: u8,
    color: [f32; 4],
) {
    let r = radius.min(24) as i32;
    if r <= 0 {
        return;
    }
    for i in 0..r {
        let w = r - i;
        if w <= 0 {
            continue;
        }
        let mut buf = SolidColorBuffer::default();
        buf.update((w, 1), color);
        parts.push((Point::from((i, i)), buf.clone()));
        parts.push((Point::from((inset * 2 + r - i - w, i)), buf));
    }
}

fn push_aot_pin_glyph(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    color: [f32; 4],
) {
    // Simple pin: circle + stem
    push_seg(parts, origin, 20, 10, 6, 6, color);
    push_seg(parts, origin, 22, 16, 2, 8, color);
}

fn push_seg(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: [f32; 4],
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let mut buf = SolidColorBuffer::default();
    buf.update((w, h), color);
    parts.push((origin + Point::from((x, y)), buf));
}

fn push_minimize_glyph(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    color: [f32; 4],
) {
    // − centred in 46×32
    push_seg(parts, origin, 18, 15, 10, 2, color);
}

fn push_maximize_glyph(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    color: [f32; 4],
) {
    // □
    push_seg(parts, origin, 18, 11, 10, 1, color);
    push_seg(parts, origin, 18, 20, 10, 1, color);
    push_seg(parts, origin, 18, 11, 1, 10, color);
    push_seg(parts, origin, 27, 11, 1, 10, color);
}

fn push_restore_glyph(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    color: [f32; 4],
) {
    // Overlapping squares (Win11 restore)
    // Back square (offset up-right)
    push_seg(parts, origin, 21, 10, 8, 1, color);
    push_seg(parts, origin, 28, 10, 1, 8, color);
    push_seg(parts, origin, 21, 10, 1, 3, color);
    // Front square
    push_seg(parts, origin, 18, 13, 8, 1, color);
    push_seg(parts, origin, 18, 20, 8, 1, color);
    push_seg(parts, origin, 18, 13, 1, 8, color);
    push_seg(parts, origin, 25, 13, 1, 8, color);
}

fn push_close_glyph(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    color: [f32; 4],
) {
    // ✕ via Bresenham segments (2px thick)
    draw_line_segs(parts, origin, 18, 11, 28, 21, color);
    draw_line_segs(parts, origin, 28, 11, 18, 21, color);
}

fn draw_line_segs(
    parts: &mut Vec<(Point<i32, Logical>, SolidColorBuffer)>,
    origin: Point<i32, Logical>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [f32; 4],
) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        push_seg(parts, origin, x, y, 2, 2, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

impl<R: Renderer> AsRenderElements<R> for HeaderBar {
    type RenderElement = SolidColorRenderElement;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        _renderer: &mut R,
        location: Point<i32, smithay::utils::Physical>,
        scale: smithay::utils::Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        let bw = BUTTON_WIDTH as i32;
        let header_end = Point::from((self.width as i32, 0));
        let button = Point::from((bw, 0));

        let inset = {
            let paint = header_paint();
            (paint.corner_radius as i32)
                .min((self.width / 4) as i32)
                .max(0)
        };
        let inset_phys = Point::from((inset, 0)).to_physical_precise_round(scale);

        let mut out: Vec<C> = Vec::with_capacity(8 + self.glyph_parts.len());
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.background,
                location + inset_phys,
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.accent_line,
                location + inset_phys,
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        // Win11 order right→left packing: min, max, close
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.minimize_bg,
                location + (header_end - button.upscale(3)).to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.maximize_bg,
                location + (header_end - button.upscale(2)).to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.close_bg,
                location + (header_end - button).to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        for (origin, buf) in &self.glyph_parts {
            out.push(
                SolidColorRenderElement::from_buffer(
                    buf,
                    location + origin.to_physical_precise_round(scale),
                    scale,
                    alpha,
                    Kind::Unspecified,
                )
                .into(),
            );
        }
        out
    }
}

impl WindowElement {
    pub fn decoration_state(&self) -> RefMut<'_, WindowState> {
        self.user_data().insert_if_missing(|| {
            RefCell::new(WindowState {
                is_ssd: false,
                header_bar: HeaderBar {
                    pointer_loc: None,
                    width: 0,
                    maximized: false,
                    always_on_top: false,
                    minimize_button_hover: false,
                    maximize_button_hover: false,
                    close_button_hover: false,
                    corner_radius: u8::MAX,
                    background: SolidColorBuffer::default(),
                    accent_line: SolidColorBuffer::default(),
                    minimize_bg: SolidColorBuffer::default(),
                    maximize_bg: SolidColorBuffer::default(),
                    close_bg: SolidColorBuffer::default(),
                    glyph_parts: Vec::new(),
                },
            })
        });

        self.user_data()
            .get::<RefCell<WindowState>>()
            .unwrap()
            .borrow_mut()
    }

    pub fn set_ssd(&self, ssd: bool) {
        self.decoration_state().is_ssd = ssd;
    }
}
