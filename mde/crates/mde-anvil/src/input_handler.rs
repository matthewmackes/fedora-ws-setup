use std::{convert::TryInto, process::Command, sync::atomic::Ordering};

use std::cell::RefCell;

use crate::{
    focus::{KeyboardFocusTarget, PointerFocusTarget},
    shell::{
        AlwaysOnTop, FullscreenSurface, PointerMoveSurfaceGrab, PointerResizeSurfaceGrab,
        ResizeData, ResizeEdge, ResizeState, SnapRestoreGeo, SurfaceData, WindowElement,
        WindowOpacity,
    },
    AnvilState,
};

#[cfg(feature = "udev")]
use crate::udev::UdevData;
#[cfg(feature = "udev")]
use smithay::backend::renderer::DebugFlags;

use smithay::{
    backend::input::{
        self, Axis, AxisSource, Event, InputBackend, InputEvent, Keycode, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
    },
    desktop::{layer_map_for_output, space::SpaceElement, WindowSurface, WindowSurfaceType},
    input::{
        keyboard::{keysyms as xkb, FilterResult, Keysym, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent,
        },
    },
    output::{Output, Scale},
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1, shell::server::xdg_toplevel,
        },
        wayland_server::protocol::wl_pointer,
    },
    utils::{IsAlive, Logical, Point, Rectangle, Serial, Size, Transform, SERIAL_COUNTER as SCOUNTER},
    wayland::{
        compositor::with_states,
        input_method::InputMethodSeat,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat,
        shell::{
            wlr_layer::{KeyboardInteractivity, Layer as WlrLayer, LayerSurfaceCachedState},
            xdg::XdgShellHandler,
        },
    },
};

#[cfg(any(feature = "winit", feature = "x11", feature = "udev"))]
use smithay::backend::input::AbsolutePositionEvent;

use tracing::{debug, error, info};

use crate::state::Backend;
#[cfg(feature = "udev")]
use smithay::{
    backend::{
        input::{
            Device, DeviceCapability, GestureBeginEvent, GestureEndEvent, GesturePinchUpdateEvent as _,
            GestureSwipeUpdateEvent as _, PointerMotionEvent, ProximityState, TabletToolButtonEvent,
            TabletToolEvent, TabletToolProximityEvent, TabletToolTipEvent, TabletToolTipState, TouchEvent,
        },
        session::Session,
    },
    input::{
        pointer::{
            GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
            GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
            RelativeMotionEvent,
        },
        touch::{DownEvent, UpEvent},
    },
    reexports::wayland_server::DisplayHandle,
    wayland::{
        pointer_constraints::{with_pointer_constraint, PointerConstraint},
        seat::WaylandFocus,
        tablet_manager::{TabletDescriptor, TabletSeatTrait},
    },
};

impl<BackendData: Backend> AnvilState<BackendData> {
    fn process_common_key_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::None => (),

            KeyAction::Quit => {
                info!("Quitting.");
                self.running.store(false, Ordering::SeqCst);
            }

            KeyAction::Shell(cmd) => {
                let parsed = mde_protocol::ShellCmd::parse(&cmd);
                self.shell_bus
                    .shell_cmd(parsed, mde_protocol::legacy_enabled());
            }

            KeyAction::ScreenshotWindow => {
                // W515: Super+PrtSc opens snip overlay with Window preselected.
                let cmd = mde_protocol::ShellCmd::Screenshot {
                    mode: mde_protocol::ScreenshotMode::Window,
                    geom: None,
                };
                self.shell_bus
                    .shell_cmd(cmd, mde_protocol::legacy_enabled());
            }

            KeyAction::Run(cmd) => {
                info!(cmd, "Starting program");

                if let Err(e) = Command::new(&cmd)
                    .envs(
                        self.socket_name
                            .clone()
                            .map(|v| ("WAYLAND_DISPLAY", v))
                            .into_iter()
                            .chain(
                                #[cfg(feature = "xwayland")]
                                self.xdisplay.map(|v| ("DISPLAY", format!(":{}", v))),
                                #[cfg(not(feature = "xwayland"))]
                                None,
                            ),
                    )
                    .spawn()
                {
                    error!(cmd, err = %e, "Failed to start program");
                }
            }

            KeyAction::Lock => {
                // W121: shell engages mde-lock (or loginctl / soft overlay fallback).
                self.shell_bus
                    .shell_cmd(mde_protocol::ShellCmd::Lock, mde_protocol::legacy_enabled());
            }

            KeyAction::TogglePreview => {
                self.show_window_preview = !self.show_window_preview;
            }

            // SSD-only standard: ClientSide toggle removed.
            KeyAction::ToggleDecorations => {}

            KeyAction::Snap(dir) => self.snap_focused(dir),
            KeyAction::MoveWindowOutput(delta) => self.move_focused_window_output(delta),
            KeyAction::Nudge(dx, dy) => {
                if self.session_lock.is_none() {
                    self.nudge_focused(dx, dy);
                }
            }
            KeyAction::WorkspacePrev => {
                if self.session_lock.is_none() {
                    let before = self.workspaces.active;
                    self.workspaces.prev(&mut self.space);
                    self.focus_after_workspace_key();
                    // W168: notify shell for Desktop N · animated stub tip.
                    if self.workspaces.active != before {
                        Self::write_workspace_anim_notify(self.workspaces.active);
                    }
                }
            }
            KeyAction::WorkspaceNext => {
                if self.session_lock.is_none() {
                    let before = self.workspaces.active;
                    self.workspaces.next(&mut self.space);
                    self.focus_after_workspace_key();
                    // W168: notify shell for Desktop N · animated stub tip.
                    if self.workspaces.active != before {
                        Self::write_workspace_anim_notify(self.workspaces.active);
                    }
                }
            }
            KeyAction::WorkspaceNew => {
                if self.session_lock.is_none() {
                    if self.workspaces.new_desktop(&mut self.space) {
                        self.focus_after_workspace_key();
                        // W188: Win+Ctrl+D → shell tip “Desktop N · new”.
                        Self::write_workspace_new_notify(self.workspaces.active);
                    }
                }
            }
            KeyAction::WorkspaceClose => {
                if self.session_lock.is_none() {
                    let _ = self.workspaces.close_current(&mut self.space);
                    self.focus_after_workspace_key();
                }
            }
            KeyAction::MinimizeOthers => {
                if self.session_lock.is_none() {
                    self.minimize_other_windows();
                    // W268: tip shell status — Minimize others · Super+Home.
                    self.shell_bus.shell_cmd(
                        mde_protocol::ShellCmd::MinimizeOthersTip,
                        mde_protocol::legacy_enabled(),
                    );
                }
            }
            KeyAction::UnminimizeLast => {
                if self.session_lock.is_none() {
                    self.unminimize_last();
                }
            }
            KeyAction::AlwaysOnTopToggle => {
                self.set_always_on_top_focused(mde_protocol::AlwaysOnTopMode::Toggle);
            }
            KeyAction::Close => {
                if self.session_lock.is_none() {
                    self.close_focused_window();
                }
            }

            _ => unreachable!(
                "Common key action handler encountered backend specific action {:?}",
                action
            ),
        }
    }

    fn focus_after_workspace_key(&mut self) {
        if let Some(keyboard) = self.seat.get_keyboard() {
            let serial = SCOUNTER.next_serial();
            let first = self.space.elements().next().cloned();
            if let Some(window) = first {
                self.space.raise_element(&window, true);
                keyboard.set_focus(self, Some(window.into()), serial);
            } else {
                keyboard.set_focus(self, None, serial);
            }
        }
    }

    /// W168: `$XDG_RUNTIME_DIR/mde-workspace.notify` — shell shows swipe animation tip.
    fn write_workspace_anim_notify(active: usize) {
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let path = std::path::Path::new(&runtime).join("mde-workspace.notify");
            let _ = std::fs::write(path, format!("anim {active}"));
        }
    }

    /// W188: Win+Ctrl+D new desktop tip (`new N`).
    fn write_workspace_new_notify(active: usize) {
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let path = std::path::Path::new(&runtime).join("mde-workspace.notify");
            let _ = std::fs::write(path, format!("new {active}"));
        }
    }

    fn focused_window_element(&self) -> Option<WindowElement> {
        if let Some(KeyboardFocusTarget::Window(window)) =
            self.seat.get_keyboard().and_then(|kb| kb.current_focus())
        {
            return Some(WindowElement(window));
        }
        self.space
            .element_under(self.pointer.current_location())
            .map(|(window, _)| window.clone())
    }

    fn output_work_area(&self, output: &Output) -> Rectangle<i32, Logical> {
        let geo = self.space.output_geometry(output).unwrap();
        let map = layer_map_for_output(output);
        let zone = map.non_exclusive_zone();
        let mut work = Rectangle::new(geo.loc + zone.loc, zone.size);
        // If the panel has not reserved an exclusive zone yet, leave taskbar margin.
        if work.size.h >= geo.size.h {
            work.size.h = (work.size.h - mde_theme::PANEL_HEIGHT).max(1);
        }
        work
    }

    fn work_area_for_window(&self, window: &WindowElement) -> Option<(Output, Rectangle<i32, Logical>)> {
        let output = self
            .space
            .outputs_for_element(window)
            .into_iter()
            .next()
            .or_else(|| {
                self.space
                    .output_under(self.pointer.current_location())
                    .next()
                    .cloned()
            })
            .or_else(|| self.space.outputs().next().cloned())?;
        let work = self.output_work_area(&output);
        Some((output, work))
    }

    fn save_snap_restore(&self, window: &WindowElement) {
        let Some(geo) = self.space.element_geometry(window) else {
            return;
        };
        window.user_data().insert_if_missing(SnapRestoreGeo::default);
        window.user_data().get::<SnapRestoreGeo>().unwrap().save(geo);
    }

    fn take_snap_restore(
        &self,
        window: &WindowElement,
        work: Rectangle<i32, Logical>,
    ) -> Rectangle<i32, Logical> {
        window
            .user_data()
            .get::<SnapRestoreGeo>()
            .and_then(|g| g.restore())
            .unwrap_or_else(|| {
                let size = Size::from((work.size.w * 2 / 3, work.size.h * 2 / 3));
                let loc = Point::from((
                    work.loc.x + (work.size.w - size.w) / 2,
                    work.loc.y + (work.size.h - size.h) / 2,
                ));
                Rectangle::new(loc, size)
            })
    }

    fn window_is_maximized(&self, window: &WindowElement) -> bool {
        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => toplevel
                .current_state()
                .states
                .contains(xdg_toplevel::State::Maximized),
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => surface.is_maximized(),
        }
    }

    fn window_is_snapped(&self, window: &WindowElement) -> bool {
        window
            .user_data()
            .get::<SnapRestoreGeo>()
            .map(|g| g.has_saved())
            .unwrap_or(false)
    }

    /// True when window fills the left half of the work area (W262).
    fn window_is_left_half_snapped(
        &self,
        window: &WindowElement,
        work: Rectangle<i32, Logical>,
    ) -> bool {
        let Some(geo) = self.space.element_geometry(window) else {
            return false;
        };
        let half = work.size.w / 2;
        (geo.loc.x - work.loc.x).abs() <= 2
            && (geo.loc.y - work.loc.y).abs() <= 2
            && (geo.size.h - work.size.h).abs() <= 2
            && (geo.size.w - half).abs() <= 4
    }

    /// Next Super+Left snap: half→2/3→1/3→half; None if tip snapflyout (W262).
    fn left_snap_cycle_next(
        &self,
        window: &WindowElement,
        work: Rectangle<i32, Logical>,
    ) -> Option<SnapDir> {
        let Some(geo) = self.space.element_geometry(window) else {
            return Some(SnapDir::Left);
        };
        // Only cycle when already a full-height left-aligned snap.
        if (geo.loc.x - work.loc.x).abs() > 2
            || (geo.loc.y - work.loc.y).abs() > 2
            || (geo.size.h - work.size.h).abs() > 2
        {
            return Some(SnapDir::Left);
        }
        let half = work.size.w / 2;
        let two = (work.size.w * 2) / 3;
        let third = work.size.w / 3;
        let w = geo.size.w;
        if (w - half).abs() <= 4 {
            // Need room for meaningful thirds growth.
            if work.size.w < 720 {
                return None;
            }
            Some(SnapDir::LeftTwoThirds)
        } else if (w - two).abs() <= 4 {
            Some(SnapDir::LeftThird)
        } else if (w - third).abs() <= 4 {
            Some(SnapDir::Left)
        } else {
            Some(SnapDir::Left)
        }
    }

    fn apply_window_geometry(&mut self, window: &WindowElement, geo: Rectangle<i32, Logical>) {
        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                toplevel.with_pending_state(|state| {
                    state.states.unset(xdg_toplevel::State::Maximized);
                    state.states.unset(xdg_toplevel::State::Fullscreen);
                    state.size = Some(geo.size);
                });
                if toplevel.is_initial_configure_sent() {
                    toplevel.send_configure();
                }
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => {
                let _ = surface.set_maximized(false);
                let _ = surface.configure(Some(geo));
            }
        }
        self.space.map_element(window.clone(), geo.loc, true);
    }

    /// SSD caption Minimize — park the window (same path as Super+Down / winmenu).
    pub(crate) fn caption_minimize(&mut self, window: &WindowElement) {
        self.minimize_window(window);
    }

    /// SSD caption Maximize/Restore toggle (Win11: □ ↔ overlapping squares).
    pub(crate) fn caption_toggle_maximize(&mut self, window: &WindowElement) {
        if self.window_is_maximized(window) {
            let Some((_output, work)) = self.work_area_for_window(window) else {
                return;
            };
            let restore = self.take_snap_restore(window, work);
            self.apply_window_geometry(window, restore);
            return;
        }
        self.save_snap_restore(window);
        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                self.maximize_request(toplevel.clone());
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => {
                let Some((_output, work)) = self.work_area_for_window(window) else {
                    return;
                };
                let _ = surface.set_maximized(true);
                let _ = surface.configure(Some(work));
                self.space.map_element(window.clone(), work.loc, true);
            }
        }
    }

    fn minimize_window(&mut self, window: &WindowElement) {
        let loc = self
            .space
            .element_location(window)
            .unwrap_or_else(|| Point::from((0, 0)));
        self.space.unmap_elem(window);
        #[cfg(feature = "xwayland")]
        if let WindowSurface::X11(surface) = window.0.underlying_surface() {
            let _ = surface.set_suspended(true);
            let _ = surface.set_mapped(false);
        }
        self.minimize_stash.retain(|p| &p.window != window);
        self.minimize_stash.push(crate::shell::workspaces::ParkedWindow {
            window: window.clone(),
            loc,
        });
        if let Some(keyboard) = self.seat.get_keyboard() {
            let serial = SCOUNTER.next_serial();
            if matches!(
                keyboard.current_focus(),
                Some(KeyboardFocusTarget::Window(ref w)) if w == &window.0
            ) {
                keyboard.set_focus(self, None, serial);
            }
        }
    }

    /// Super+Shift+M — restore most recently minimized window (W232).
    fn unminimize_last(&mut self) {
        self.minimize_stash.retain(|p| p.window.alive());
        let Some(p) = self.minimize_stash.pop() else {
            return;
        };
        #[cfg(feature = "xwayland")]
        if let WindowSurface::X11(surface) = p.window.0.underlying_surface() {
            let _ = surface.set_mapped(true);
            let _ = surface.set_suspended(false);
        }
        self.space.map_element(p.window.clone(), p.loc, true);
        if let Some(keyboard) = self.seat.get_keyboard() {
            let serial = SCOUNTER.next_serial();
            keyboard.set_focus(
                self,
                Some(KeyboardFocusTarget::Window(p.window.0.clone())),
                serial,
            );
        }
    }

    /// Snap a window when a move grab ends near a work-area edge (W91).
    pub fn snap_window_if_at_edge(&mut self, window: &WindowElement) {
        if self.session_lock.is_some() {
            return;
        }
        let Some((_output, work)) = self.work_area_for_window(window) else {
            return;
        };
        let Some(geo) = self.space.element_geometry(window) else {
            return;
        };
        const EDGE: i32 = 24;
        let left = geo.loc.x <= work.loc.x + EDGE;
        let right = geo.loc.x + geo.size.w >= work.loc.x + work.size.w - EDGE;
        let top = geo.loc.y <= work.loc.y + EDGE;
        // Prefer maximize on top edge; otherwise left/right half.
        let dir = if top && !left && !right {
            SnapDir::Up
        } else if left && !right {
            SnapDir::Left
        } else if right && !left {
            SnapDir::Right
        } else {
            return;
        };
        self.snap_window(window, dir);
    }

    fn snap_window(&mut self, window: &WindowElement, dir: SnapDir) {
        let Some((_output, work)) = self.work_area_for_window(window) else {
            return;
        };
        let maximized = self.window_is_maximized(window);
        let snapped = self.window_is_snapped(window);
        match dir {
            SnapDir::Up => {
                if maximized {
                    return;
                }
                self.save_snap_restore(window);
                match window.0.underlying_surface() {
                    WindowSurface::Wayland(toplevel) => {
                        self.maximize_request(toplevel.clone());
                    }
                    #[cfg(feature = "xwayland")]
                    WindowSurface::X11(surface) => {
                        let _ = surface.set_maximized(true);
                        let _ = surface.configure(Some(work));
                        self.space.map_element(window.clone(), work.loc, true);
                    }
                }
            }
            SnapDir::Left | SnapDir::Right => {
                self.save_snap_restore(window);
                let half_w = work.size.w / 2;
                let (loc_x, width) = if matches!(dir, SnapDir::Left) {
                    (work.loc.x, half_w)
                } else {
                    (work.loc.x + half_w, work.size.w - half_w)
                };
                let geo = Rectangle::new(
                    Point::from((loc_x, work.loc.y)),
                    Size::from((width, work.size.h)),
                );
                self.apply_window_geometry(window, geo);
                let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                let side = if matches!(dir, SnapDir::Left) {
                    "left"
                } else {
                    "right"
                };
                let _ = std::fs::write(
                    std::path::Path::new(&runtime).join("mde-snap-assist.req"),
                    format!("{side}\n"),
                );
            }
            _ => {
                let _ = (maximized, snapped, window, work);
            }
        }
    }

    fn snap_focused(&mut self, dir: SnapDir) {
        if self.session_lock.is_some() {
            return;
        }

        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some((_output, work)) = self.work_area_for_window(&window) else {
            return;
        };

        let maximized = self.window_is_maximized(&window);
        let snapped = self.window_is_snapped(&window);

        match dir {
            SnapDir::Up => {
                if maximized {
                    return;
                }
                self.save_snap_restore(&window);
                match window.0.underlying_surface() {
                    WindowSurface::Wayland(toplevel) => {
                        self.maximize_request(toplevel.clone());
                    }
                    #[cfg(feature = "xwayland")]
                    WindowSurface::X11(surface) => {
                        // Maximize into the work area (exclude taskbar), not full output.
                        let _ = surface.set_maximized(true);
                        let _ = surface.configure(Some(work));
                        self.space.map_element(window, work.loc, true);
                    }
                }
            }
            // W142: Super+Down from maximized/snapped restores remembered size.
            SnapDir::Down => {
                if maximized || snapped {
                    let restore = self.take_snap_restore(&window, work);
                    self.apply_window_geometry(&window, restore);
                } else {
                    self.minimize_window(&window);
                }
            }
            SnapDir::Left | SnapDir::Right => {
                // W262: repeated Super+Left cycles half → 2/3 → 1/3 when feasible.
                if matches!(dir, SnapDir::Left) {
                    if let Some(next) = self.left_snap_cycle_next(&window, work) {
                        match next {
                            SnapDir::LeftTwoThirds | SnapDir::LeftThird => {
                                self.snap_focused(next);
                                return;
                            }
                            SnapDir::Left => {
                                // cycle back to half — fall through
                            }
                            _ => {}
                        }
                    } else if self.window_is_left_half_snapped(&window, work) {
                        // Too narrow for thirds — tip snapflyout (W262).
                        self.shell_bus.shell_cmd(
                            mde_protocol::ShellCmd::ToggleSnapFlyout,
                            mde_protocol::legacy_enabled(),
                        );
                        return;
                    }
                }
                self.save_snap_restore(&window);
                let half_w = work.size.w / 2;
                let (loc_x, width) = if matches!(dir, SnapDir::Left) {
                    (work.loc.x, half_w)
                } else {
                    (work.loc.x + half_w, work.size.w - half_w)
                };
                let geo = Rectangle::new(
                    Point::from((loc_x, work.loc.y)),
                    Size::from((width, work.size.h)),
                );
                self.apply_window_geometry(&window, geo);
                // Hint shell to open Snap assist for the opposite half (W24).
                let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                let side = if matches!(dir, SnapDir::Left) {
                    "left"
                } else {
                    "right"
                };
                let _ = std::fs::write(
                    std::path::Path::new(&runtime).join("mde-snap-assist.req"),
                    format!("{side}\n"),
                );
            }
            SnapDir::TopLeft
            | SnapDir::TopRight
            | SnapDir::BottomLeft
            | SnapDir::BottomRight => {
                self.save_snap_restore(&window);
                let half_w = work.size.w / 2;
                let half_h = work.size.h / 2;
                let (loc_x, loc_y, width, height) = match dir {
                    SnapDir::TopLeft => (work.loc.x, work.loc.y, half_w, half_h),
                    SnapDir::TopRight => (
                        work.loc.x + half_w,
                        work.loc.y,
                        work.size.w - half_w,
                        half_h,
                    ),
                    SnapDir::BottomLeft => (
                        work.loc.x,
                        work.loc.y + half_h,
                        half_w,
                        work.size.h - half_h,
                    ),
                    SnapDir::BottomRight => (
                        work.loc.x + half_w,
                        work.loc.y + half_h,
                        work.size.w - half_w,
                        work.size.h - half_h,
                    ),
                    _ => unreachable!(),
                };
                let geo = Rectangle::new(
                    Point::from((loc_x, loc_y)),
                    Size::from((width, height)),
                );
                self.apply_window_geometry(&window, geo);
                // W81: light remaining-corner hint after a quarter snap.
                let taken = match dir {
                    SnapDir::TopLeft => "top-left",
                    SnapDir::TopRight => "top-right",
                    SnapDir::BottomLeft => "bottom-left",
                    SnapDir::BottomRight => "bottom-right",
                    _ => unreachable!(),
                };
                let remaining = match dir {
                    SnapDir::TopLeft => "top-right · bottom-left · bottom-right",
                    SnapDir::TopRight => "top-left · bottom-left · bottom-right",
                    SnapDir::BottomLeft => "top-left · top-right · bottom-right",
                    SnapDir::BottomRight => "top-left · top-right · bottom-left",
                    _ => unreachable!(),
                };
                let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                let _ = std::fs::write(
                    std::path::Path::new(&runtime).join("mde-snap-assist.req"),
                    format!("quadrant {taken}\n{remaining}\n"),
                );
            }
            SnapDir::LeftTwoThirds | SnapDir::RightTwoThirds => {
                self.save_snap_restore(&window);
                let two = (work.size.w * 2) / 3;
                let (loc_x, width) = if matches!(dir, SnapDir::LeftTwoThirds) {
                    (work.loc.x, two)
                } else {
                    (work.loc.x + (work.size.w - two), two)
                };
                let geo = Rectangle::new(
                    Point::from((loc_x, work.loc.y)),
                    Size::from((width, work.size.h)),
                );
                self.apply_window_geometry(&window, geo);
            }
            SnapDir::LeftThird | SnapDir::CenterThird | SnapDir::RightThird => {
                self.save_snap_restore(&window);
                let third = work.size.w / 3;
                let (loc_x, width) = match dir {
                    SnapDir::LeftThird => (work.loc.x, third),
                    SnapDir::CenterThird => (work.loc.x + third, third),
                    SnapDir::RightThird => {
                        let x = work.loc.x + 2 * third;
                        (x, work.size.w - (x - work.loc.x))
                    }
                    _ => unreachable!(),
                };
                let geo = Rectangle::new(
                    Point::from((loc_x, work.loc.y)),
                    Size::from((width, work.size.h)),
                );
                self.apply_window_geometry(&window, geo);
                // W122: remaining-region hint after left/right 1/3 snap.
                if matches!(dir, SnapDir::LeftThird | SnapDir::RightThird) {
                    let (taken, remaining) = match dir {
                        SnapDir::LeftThird => ("left-1/3", "center-1/3 · right-1/3"),
                        SnapDir::RightThird => ("right-1/3", "left-1/3 · center-1/3"),
                        _ => unreachable!(),
                    };
                    let runtime =
                        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                    let _ = std::fs::write(
                        std::path::Path::new(&runtime).join("mde-snap-assist.req"),
                        format!("thirds {taken}\n{remaining}\n"),
                    );
                }
            }
            // W182: vertical half snap (top / bottom of work area).
            SnapDir::TopHalf | SnapDir::BottomHalf => {
                self.save_snap_restore(&window);
                let half_h = work.size.h / 2;
                let (loc_y, height) = if matches!(dir, SnapDir::TopHalf) {
                    (work.loc.y, half_h)
                } else {
                    (work.loc.y + half_h, work.size.h - half_h)
                };
                let geo = Rectangle::new(
                    Point::from((work.loc.x, loc_y)),
                    Size::from((work.size.w, height)),
                );
                self.apply_window_geometry(&window, geo);
            }
            // W231: center focused window in work area (keep size; unmaximize if needed).
            SnapDir::Center => {
                let size = if maximized || snapped {
                    self.take_snap_restore(&window, work).size
                } else {
                    self.space
                        .element_geometry(&window)
                        .map(|g| g.size)
                        .unwrap_or_else(|| Size::from((work.size.w * 2 / 3, work.size.h * 2 / 3)))
                };
                let loc = Point::from((
                    work.loc.x + (work.size.w - size.w).max(0) / 2,
                    work.loc.y + (work.size.h - size.h).max(0) / 2,
                ));
                self.apply_window_geometry(&window, Rectangle::new(loc, size));
            }
        }
    }

    /// Drain typed `mde-protocol` messages from the shell.
    pub fn apply_shell_protocol(&mut self) {
        let msgs = self.shell_bus.drain();
        for msg in msgs {
            match msg {
                mde_protocol::ClientToComp::FocusWindow { id } => {
                    self.focus_window_by_id(id);
                }
                mde_protocol::ClientToComp::FocusIndex { index } => {
                    self.focus_window_by_index(index);
                }
                mde_protocol::ClientToComp::Snap { layout } => {
                    self.snap_focused(snap_layout_to_dir(layout));
                }
                mde_protocol::ClientToComp::WindowAction { action } => {
                    if self.session_lock.is_some() {
                        continue;
                    }
                    self.dispatch_win_action(action);
                }
                mde_protocol::ClientToComp::Workspace { op } => {
                    if self.session_lock.is_some() {
                        continue;
                    }
                    match op {
                        mde_protocol::WorkspaceOp::Switch(n) => {
                            self.workspaces.switch_to(&mut self.space, n);
                            self.clear_focus_after_workspace_switch();
                        }
                        mde_protocol::WorkspaceOp::New => {
                            let _ = self.workspaces.new_desktop(&mut self.space);
                            self.clear_focus_after_workspace_switch();
                        }
                        mde_protocol::WorkspaceOp::Close => {
                            let _ = self.workspaces.close_current(&mut self.space);
                            self.clear_focus_after_workspace_switch();
                        }
                        mde_protocol::WorkspaceOp::MoveFocused(n) => {
                            if let Some(window) = self.focused_window_for_workspace() {
                                self.workspaces
                                    .move_window_to(&mut self.space, &window, n);
                                if crate::shell::Workspaces::window_workspace(&window)
                                    != self.workspaces.active
                                {
                                    self.clear_focus_after_workspace_switch();
                                }
                            }
                        }
                    }
                }
                mde_protocol::ClientToComp::ShowDesktop => {
                    if self.session_lock.is_none() {
                        self.toggle_show_desktop();
                    }
                }
                mde_protocol::ClientToComp::FocusedWindowGeom => {
                    let geom = self.focused_window_element().and_then(|window| {
                        let geo = self.space.element_geometry(&window)?;
                        Some(format!(
                            "{},{} {}x{}",
                            geo.loc.x, geo.loc.y, geo.size.w, geo.size.h
                        ))
                    });
                    self.shell_bus
                        .push(mde_protocol::CompToShell::FocusedWindowGeom(geom));
                }
            }
        }
    }

    /// Shell writes `$XDG_RUNTIME_DIR/mde-snap.req` with a layout id (W14).
    /// A016 also writes `zone <idx> <x> <y> <w> <h>` fractions of the work area.
    pub fn apply_snap_request(&mut self) {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let path = std::path::Path::new(&runtime).join("mde-snap.req");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let _ = std::fs::remove_file(&path);
        let trimmed = text.trim();
        if let Some(rest) = trimmed.strip_prefix("zone ") {
            // zone <idx> <x> <y> <w> <h>
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 5 {
                if let (Ok(_idx), Ok(x), Ok(y), Ok(w), Ok(h)) = (
                    parts[0].parse::<usize>(),
                    parts[1].parse::<f32>(),
                    parts[2].parse::<f32>(),
                    parts[3].parse::<f32>(),
                    parts[4].parse::<f32>(),
                ) {
                    self.snap_focused_zone(x, y, w, h);
                    return;
                }
            }
            return;
        }
        let dir = match trimmed {
            "left" => SnapDir::Left,
            "right" => SnapDir::Right,
            "top-left" => SnapDir::TopLeft,
            "top-right" => SnapDir::TopRight,
            "bottom-left" => SnapDir::BottomLeft,
            "bottom-right" => SnapDir::BottomRight,
            "left-2/3" | "left-two-thirds" => SnapDir::LeftTwoThirds,
            "right-2/3" | "right-two-thirds" => SnapDir::RightTwoThirds,
            "left-1/3" | "left-third" => SnapDir::LeftThird,
            "center-1/3" | "center-third" => SnapDir::CenterThird,
            "right-1/3" | "right-third" => SnapDir::RightThird,
            "maximize" | "up" => SnapDir::Up,
            "restore" | "down" => SnapDir::Down,
            "top" | "top-half" => SnapDir::TopHalf,
            "bottom" | "bottom-half" => SnapDir::BottomHalf,
            // W231: center in work area (winmenu Center).
            "center" => SnapDir::Center,
            _ => return,
        };
        self.snap_focused(dir);
    }

    /// A016: place focused window into a fractional FancyZones rect.
    fn snap_focused_zone(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some((_output, work)) = self.work_area_for_window(&window) else {
            return;
        };
        self.save_snap_restore(&window);
        let gx = work.loc.x + (work.size.w as f32 * x.clamp(0.0, 1.0)).round() as i32;
        let gy = work.loc.y + (work.size.h as f32 * y.clamp(0.0, 1.0)).round() as i32;
        let gw = ((work.size.w as f32) * w.clamp(0.05, 1.0)).round().max(64.0) as i32;
        let gh = ((work.size.h as f32) * h.clamp(0.05, 1.0)).round().max(64.0) as i32;
        let geo = Rectangle::new(Point::from((gx, gy)), (gw, gh).into());
        self.apply_window_geometry(&window, geo);
    }

    /// Shell writes `$XDG_RUNTIME_DIR/mde-window-action.req`: minimize | maximize | close | …
    pub fn apply_window_action_req(&mut self) {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let path = std::path::Path::new(&runtime).join("mde-window-action.req");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let _ = std::fs::remove_file(&path);
        if self.session_lock.is_some() {
            return;
        }
        let Some(action) = mde_protocol::WinAction::parse(text.trim()) else {
            return;
        };
        self.dispatch_win_action(action);
    }

    fn dispatch_win_action(&mut self, action: mde_protocol::WinAction) {
        match action {
            mde_protocol::WinAction::Minimize => {
                if let Some(window) = self.focused_window_element() {
                    self.minimize_window(&window);
                }
            }
            mde_protocol::WinAction::Maximize => self.snap_focused(SnapDir::Up),
            mde_protocol::WinAction::Close => self.close_focused_window(),
            mde_protocol::WinAction::MinimizeOthers => self.minimize_other_windows(),
            // W211: undo last snap/maximize when SnapRestoreGeo exists (no minimize fallback).
            mde_protocol::WinAction::Restore => self.undo_snap_focused(),
            // W232: restore most recently minimized.
            mde_protocol::WinAction::Unminimize => self.unminimize_last(),
            // W251: pixel nudge from winmenu / shell.
            mde_protocol::WinAction::Nudge { dx, dy } => self.nudge_focused(dx, dy),
            // W412
            mde_protocol::WinAction::AlwaysOnTop(mode) => self.set_always_on_top_focused(mode),
            // W413
            mde_protocol::WinAction::Opacity => self.cycle_opacity_focused(),
            // W414
            mde_protocol::WinAction::Cascade => self.cascade_workspace_windows(),
            // W415
            mde_protocol::WinAction::Move => self.start_pointer_move_focused(),
            // W416
            mde_protocol::WinAction::Size => self.start_pointer_resize_focused(),
        }
    }

    /// W412: toggle / set always-on-top on the focused window.
    fn set_always_on_top_focused(&mut self, mode: mde_protocol::AlwaysOnTopMode) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        window.user_data().insert_if_missing(AlwaysOnTop::default);
        let slot = window.user_data().get::<AlwaysOnTop>().unwrap();
        let on = match mode {
            mde_protocol::AlwaysOnTopMode::Toggle => slot.toggle(),
            mde_protocol::AlwaysOnTopMode::On => {
                slot.set(true);
                true
            }
            mde_protocol::AlwaysOnTopMode::Off => {
                slot.set(false);
                false
            }
        };
        if on {
            self.space.raise_element(&window, true);
        }
        self.raise_always_on_top_windows();
    }

    /// Re-raise any always-on-top windows above the rest (after focus / map).
    /// Uses `activate = false` so we do not steal keyboard focus every frame.
    pub(crate) fn raise_always_on_top_windows(&mut self) {
        let aot: Vec<_> = self
            .space
            .elements()
            .filter(|w| {
                w.user_data()
                    .get::<AlwaysOnTop>()
                    .map(|f| f.get())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        for window in aot {
            self.space.raise_element(&window, false);
        }
    }

    /// W413: cycle focused window opacity 1.0 → 0.85 → 0.7 → 1.0.
    fn cycle_opacity_focused(&mut self) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        window.user_data().insert_if_missing(WindowOpacity::default);
        let _ = window.user_data().get::<WindowOpacity>().unwrap().cycle();
    }

    /// W414: cascade mapped windows on the active workspace (~32px offset each).
    fn cascade_workspace_windows(&mut self) {
        const STEP: i32 = 32;
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if windows.is_empty() {
            return;
        }
        // Anchor cascade in the work area of the first (or focused) window's output.
        let anchor = self
            .focused_window_element()
            .or_else(|| windows.first().cloned());
        let Some(anchor) = anchor else {
            return;
        };
        let Some((_output, work)) = self.work_area_for_window(&anchor) else {
            return;
        };
        for (i, window) in windows.into_iter().enumerate() {
            let Some(geo) = self.space.element_geometry(&window) else {
                continue;
            };
            let offset = (i as i32) * STEP;
            let loc = Point::from((work.loc.x + offset, work.loc.y + offset));
            // Keep size; only rearrange.
            let new_geo = Rectangle::new(loc, geo.size);
            self.apply_window_geometry(&window, new_geo);
        }
        self.raise_always_on_top_windows();
    }

    /// W415: start a pointer move grab on the focused window (sysmenu Move).
    fn start_pointer_move_focused(&mut self) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some(mut initial_window_location) = self.space.element_location(&window) else {
            return;
        };
        let pointer = self.pointer.clone();
        let location = pointer.current_location();
        let serial = SCOUNTER.next_serial();

        // Unmaximize so move is free (same idea as xdg move_request).
        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                let current = toplevel.current_state();
                if current.states.contains(xdg_toplevel::State::Maximized) {
                    toplevel.with_pending_state(|state| {
                        state.states.unset(xdg_toplevel::State::Maximized);
                        state.size = None;
                    });
                    toplevel.send_configure();
                    initial_window_location = (location.x as i32, location.y as i32).into();
                }
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => {
                if surface.is_maximized() {
                    let _ = surface.set_maximized(false);
                    initial_window_location = (location.x as i32, location.y as i32).into();
                }
            }
        }

        let start_data = PointerGrabStartData {
            focus: None,
            button: 0x110, // BTN_LEFT
            location,
        };
        let grab = PointerMoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// W416: start a bottom-right resize grab on the focused window (sysmenu Size).
    fn start_pointer_resize_focused(&mut self) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some(initial_window_location) = self.space.element_location(&window) else {
            return;
        };
        let geometry = window.geometry();
        let initial_window_size = geometry.size;
        let edges = ResizeEdge::BOTTOM_RIGHT;
        let pointer = self.pointer.clone();
        let location = pointer.current_location();
        let serial = SCOUNTER.next_serial();

        if let Some(surface) = window.wl_surface() {
            with_states(surface.as_ref(), |states| {
                states
                    .data_map
                    .insert_if_missing(|| RefCell::new(SurfaceData::default()));
                states
                    .data_map
                    .get::<RefCell<SurfaceData>>()
                    .unwrap()
                    .borrow_mut()
                    .resize_state = ResizeState::Resizing(ResizeData {
                    edges,
                    initial_window_location,
                    initial_window_size,
                });
            });
        }

        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                toplevel.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Resizing);
                    state.size = Some(initial_window_size);
                });
                toplevel.send_pending_configure();
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(_) => {}
        }

        let start_data = PointerGrabStartData {
            focus: None,
            button: 0x110, // BTN_LEFT
            location,
        };
        let grab = PointerResizeSurfaceGrab {
            start_data,
            window,
            edges,
            initial_window_location,
            initial_window_size,
            last_window_size: initial_window_size,
        };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Move focused window by (dx, dy) pixels (W251).
    fn nudge_focused(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some(geo) = self.space.element_geometry(&window) else {
            return;
        };
        let loc = Point::from((geo.loc.x + dx, geo.loc.y + dy));
        self.space.map_element(window, loc, true);
    }

    /// Restore pre-snap geometry if saved; no-op when floating (W211).
    fn undo_snap_focused(&mut self) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let Some((_output, work)) = self.work_area_for_window(&window) else {
            return;
        };
        if !(self.window_is_maximized(&window) || self.window_is_snapped(&window)) {
            return;
        }
        let restore = self.take_snap_restore(&window, work);
        self.apply_window_geometry(&window, restore);
    }

    /// Shell / Super+D writes `$XDG_RUNTIME_DIR/mde-show-desktop.req` (W23/W80).
    pub fn apply_show_desktop_req(&mut self) {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let path = std::path::Path::new(&runtime).join("mde-show-desktop.req");
        if !path.is_file() {
            return;
        }
        let _ = std::fs::remove_file(&path);
        if self.session_lock.is_some() {
            return;
        }
        self.toggle_show_desktop();
    }

    fn toggle_show_desktop(&mut self) {
        if !self.show_desktop_stash.is_empty() {
            for p in self.show_desktop_stash.drain(..) {
                if !p.window.alive() {
                    continue;
                }
                #[cfg(feature = "xwayland")]
                if let WindowSurface::X11(surface) = p.window.0.underlying_surface() {
                    let _ = surface.set_mapped(true);
                    let _ = surface.set_suspended(false);
                }
                self.space.map_element(p.window, p.loc, false);
            }
            return;
        }
        let mapped: Vec<_> = self.space.elements().cloned().collect();
        for window in mapped {
            let loc = self
                .space
                .element_location(&window)
                .unwrap_or_else(|| Point::from((0, 0)));
            self.space.unmap_elem(&window);
            #[cfg(feature = "xwayland")]
            if let WindowSurface::X11(surface) = window.0.underlying_surface() {
                let _ = surface.set_suspended(true);
                let _ = surface.set_mapped(false);
            }
            self.show_desktop_stash
                .push(crate::shell::workspaces::ParkedWindow { window, loc });
        }
        if let Some(keyboard) = self.seat.get_keyboard() {
            let serial = SCOUNTER.next_serial();
            keyboard.set_focus(self, None, serial);
        }
    }

    fn minimize_other_windows(&mut self) {
        let focused = self.focused_window_element();
        let mapped: Vec<_> = self.space.elements().cloned().collect();
        for window in mapped {
            if focused.as_ref().is_some_and(|f| f == &window) {
                continue;
            }
            self.minimize_window(&window);
        }
    }

    fn close_focused_window(&mut self) {
        let Some(window) = self.focused_window_element() else {
            return;
        };
        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => toplevel.send_close(),
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => {
                let _ = surface.close();
            }
        }
    }

    fn move_focused_window_output(&mut self, delta: i32) {
        if self.session_lock.is_some() || delta == 0 {
            return;
        }
        let Some(window) = self.focused_window_element() else {
            return;
        };
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        if outputs.len() < 2 {
            return;
        }
        let Some((current, _work)) = self.work_area_for_window(&window) else {
            return;
        };
        let Some(idx) = outputs.iter().position(|o| o == &current) else {
            return;
        };
        let len = outputs.len() as i32;
        let next = ((idx as i32 + delta).rem_euclid(len)) as usize;
        let target = &outputs[next];
        let work = self.output_work_area(target);
        let maximized = self.window_is_maximized(&window);

        if maximized {
            match window.0.underlying_surface() {
                WindowSurface::Wayland(toplevel) => {
                    toplevel.with_pending_state(|state| {
                        state.states.set(xdg_toplevel::State::Maximized);
                        state.size = Some(work.size);
                    });
                    if toplevel.is_initial_configure_sent() {
                        toplevel.send_configure();
                    }
                    self.space.map_element(window, work.loc, true);
                }
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(surface) => {
                    let _ = surface.set_maximized(true);
                    let _ = surface.configure(Some(work));
                    self.space.map_element(window, work.loc, true);
                }
            }
            return;
        }

        let Some(geo) = self.space.element_geometry(&window) else {
            return;
        };
        let current_work = self.output_work_area(&current);
        // Preserve left/right half snaps across outputs when sizes match.
        let snapped_left = geo.loc.x == current_work.loc.x
            && geo.size.w == current_work.size.w / 2
            && geo.size.h == current_work.size.h;
        let half_w = current_work.size.w - current_work.size.w / 2;
        let snapped_right = geo.loc.x == current_work.loc.x + current_work.size.w / 2
            && geo.size.w == half_w
            && geo.size.h == current_work.size.h;

        let new_geo = if snapped_left {
            Rectangle::new(work.loc, Size::from((work.size.w / 2, work.size.h)))
        } else if snapped_right {
            let w = work.size.w - work.size.w / 2;
            Rectangle::new(
                Point::from((work.loc.x + work.size.w / 2, work.loc.y)),
                Size::from((w, work.size.h)),
            )
        } else {
            let rel_x = (geo.loc.x - current_work.loc.x).max(0);
            let rel_y = (geo.loc.y - current_work.loc.y).max(0);
            let max_x = (work.size.w - geo.size.w).max(0);
            let max_y = (work.size.h - geo.size.h).max(0);
            Rectangle::new(
                Point::from((
                    work.loc.x + rel_x.min(max_x),
                    work.loc.y + rel_y.min(max_y),
                )),
                geo.size,
            )
        };
        self.apply_window_geometry(&window, new_geo);
    }

    fn keyboard_key_to_action<B: InputBackend>(&mut self, evt: B::KeyboardKeyEvent) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        debug!(?keycode, ?state, "key");
        let serial = SCOUNTER.next_serial();
        let time = Event::time_msec(&evt);
        let mut suppressed_keys = self.suppressed_keys.clone();
        let keyboard = self.seat.get_keyboard().unwrap();

        for layer in self.layer_shell_state.layer_surfaces().rev() {
            let data = with_states(layer.wl_surface(), |states| {
                *states.cached_state.get::<LayerSurfaceCachedState>().current()
            });
            if data.keyboard_interactivity == KeyboardInteractivity::Exclusive
                && (data.layer == WlrLayer::Top || data.layer == WlrLayer::Overlay)
            {
                let surface = self.space.outputs().find_map(|o| {
                    let map = layer_map_for_output(o);
                    let cloned = map.layers().find(|l| l.layer_surface() == &layer).cloned();
                    cloned
                });
                if let Some(surface) = surface {
                    keyboard.set_focus(self, Some(surface.into()), serial);
                    keyboard.input::<(), _>(self, keycode, state, serial, time, |_, _, _| {
                        FilterResult::Forward
                    });
                    return KeyAction::None;
                };
            }
        }

        let inhibited = self
            .space
            .element_under(self.pointer.current_location())
            .and_then(|(window, _)| {
                let surface = window.wl_surface()?;
                self.seat.keyboard_shortcuts_inhibitor_for_surface(&surface)
            })
            .map(|inhibitor| inhibitor.is_active())
            .unwrap_or(false);

        let action = keyboard
            .input(self, keycode, state, serial, time, |_, modifiers, handle| {
                let keysym = handle.modified_sym();

                debug!(
                    ?state,
                    mods = ?modifiers,
                    keysym = ::xkbcommon::xkb::keysym_get_name(keysym),
                    "keysym"
                );

                // If the key is pressed and triggered a action
                // we will not forward the key to the client.
                // Additionally add the key to the suppressed keys
                // so that we can decide on a release if the key
                // should be forwarded to the client or not.
                if let KeyState::Pressed = state {
                    if !inhibited {
                        // Normalize PrtSc: hardware KEY_SYSRQ/PRINT, or xkb Print /
                        // Sys_Req / 3270_PrintScreen → treat as Print + real mods so
                        // Shift+PrtSc (often Sys_Req) still hits the area bind.
                        let raw = handle
                            .raw_syms()
                            .first()
                            .copied()
                            .unwrap_or(keysym);
                        let shortcut_sym = if is_print_keycode(keycode)
                            || is_print_screen(raw)
                            || is_print_screen(keysym)
                        {
                            Keysym::Print
                        } else {
                            keysym
                        };
                        let action = process_keyboard_shortcut(*modifiers, shortcut_sym);

                        if action.is_some() {
                            suppressed_keys.push(keysym);
                        }

                        action
                            .map(FilterResult::Intercept)
                            .unwrap_or(FilterResult::Forward)
                    } else {
                        FilterResult::Forward
                    }
                } else {
                    let suppressed = suppressed_keys.contains(&keysym);
                    if suppressed {
                        suppressed_keys.retain(|k| *k != keysym);
                        FilterResult::Intercept(KeyAction::None)
                    } else {
                        FilterResult::Forward
                    }
                }
            })
            .unwrap_or(KeyAction::None);

        self.suppressed_keys = suppressed_keys;
        action
    }

    fn alt_snap_enabled(&self) -> bool {
        mde_config::MdeConfig::load()
            .map(|c| c.active().compositor.alt_snap)
            .unwrap_or(true)
    }

    /// A049: Alt+left-drag move / Alt+right-drag resize from the window body.
    fn try_alt_snap_press(&mut self, button: u32, location: Point<f64, Logical>, serial: Serial) -> bool {
        if !self.alt_snap_enabled() || self.pointer.is_grabbed() {
            return false;
        }
        let keyboard = match self.seat.get_keyboard() {
            Some(k) => k,
            None => return false,
        };
        let mods = keyboard.modifier_state();
        if !mods.alt || mods.logo {
            return false;
        }
        const BTN_LEFT: u32 = 0x110;
        const BTN_RIGHT: u32 = 0x111;
        let resize = button == BTN_RIGHT;
        if button != BTN_LEFT && !resize {
            return false;
        }
        let window = match self.space.element_under(location) {
            Some((w, _)) => w.clone(),
            None => return false,
        };
        self.space.raise_element(&window, true);
        keyboard.set_focus(self, Some(window.clone().into()), serial);
        if resize {
            self.start_pointer_resize_window(window);
        } else {
            self.start_pointer_move_window(window);
        }
        true
    }

    fn start_pointer_move_window(&mut self, window: WindowElement) {
        let Some(mut initial_window_location) = self.space.element_location(&window) else {
            return;
        };
        let pointer = self.pointer.clone();
        let location = pointer.current_location();
        let serial = SCOUNTER.next_serial();

        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                let current = toplevel.current_state();
                if current.states.contains(xdg_toplevel::State::Maximized) {
                    toplevel.with_pending_state(|state| {
                        state.states.unset(xdg_toplevel::State::Maximized);
                        state.size = None;
                    });
                    toplevel.send_configure();
                    initial_window_location = (location.x as i32, location.y as i32).into();
                }
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(surface) => {
                if surface.is_maximized() {
                    let _ = surface.set_maximized(false);
                    initial_window_location = (location.x as i32, location.y as i32).into();
                }
            }
        }

        let start_data = PointerGrabStartData {
            focus: None,
            button: 0x110,
            location,
        };
        let grab = PointerMoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn start_pointer_resize_window(&mut self, window: WindowElement) {
        let Some(initial_window_location) = self.space.element_location(&window) else {
            return;
        };
        let geometry = window.geometry();
        let initial_window_size = geometry.size;
        let edges = ResizeEdge::BOTTOM_RIGHT;
        let pointer = self.pointer.clone();
        let location = pointer.current_location();
        let serial = SCOUNTER.next_serial();

        if let Some(surface) = window.wl_surface() {
            with_states(surface.as_ref(), |states| {
                states
                    .data_map
                    .insert_if_missing(|| RefCell::new(SurfaceData::default()));
                states
                    .data_map
                    .get::<RefCell<SurfaceData>>()
                    .unwrap()
                    .borrow_mut()
                    .resize_state = ResizeState::Resizing(ResizeData {
                    edges,
                    initial_window_location,
                    initial_window_size,
                });
            });
        }

        match window.0.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                toplevel.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Resizing);
                    state.size = Some(initial_window_size);
                });
                toplevel.send_configure();
            }
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(_) => {}
        }

        let start_data = PointerGrabStartData {
            focus: None,
            button: 0x111,
            location,
        };
        let grab = PointerResizeSurfaceGrab {
            start_data,
            window,
            edges,
            initial_window_location,
            initial_window_size,
            last_window_size: initial_window_size,
        };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn on_pointer_button<B: InputBackend>(&mut self, evt: B::PointerButtonEvent) {
        let serial = SCOUNTER.next_serial();
        let button = evt.button_code();

        let state = wl_pointer::ButtonState::from(evt.state());

        if wl_pointer::ButtonState::Pressed == state {
            let location = self.pointer.current_location();
            if self.try_alt_snap_press(button, location, serial) {
                let pointer = self.pointer.clone();
                pointer.frame(self);
                return;
            }
            self.update_keyboard_focus(location, serial);
            // W388: right-click empty desktop → desktop menu (ignore wallpaper/bottom layers).
            const BTN_RIGHT: u32 = 0x111;
            if button == BTN_RIGHT && self.session_lock.is_none() && self.desktop_context_hit(location)
            {
                self.shell_bus.shell_cmd(
                    mde_protocol::ShellCmd::ToggleDesktopMenu,
                    mde_protocol::legacy_enabled(),
                );
                return;
            }
        };
        let pointer = self.pointer.clone();
        pointer.button(
            self,
            &ButtonEvent {
                button,
                state: state.try_into().unwrap(),
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn update_keyboard_focus(&mut self, location: Point<f64, Logical>, serial: Serial) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let touch = self.seat.get_touch();
        let input_method = self.seat.input_method();
        // change the keyboard focus unless the pointer or keyboard is grabbed
        // We test for any matching surface type here but always use the root
        // (in case of a window the toplevel) surface for the focus.
        // So for example if a user clicks on a subsurface or popup the toplevel
        // will receive the keyboard focus. Directly assigning the focus to the
        // matching surface leads to issues with clients dismissing popups and
        // subsurface menus (for example firefox-wayland).
        // see here for a discussion about that issue:
        // https://gitlab.freedesktop.org/wayland/wayland/-/issues/294
        if !self.pointer.is_grabbed()
            && (!keyboard.is_grabbed() || input_method.keyboard_grabbed())
            && !touch.map(|touch| touch.is_grabbed()).unwrap_or(false)
        {
            // Session lock: only the lock surface may receive focus.
            if let Some(lock) = self.session_lock.as_ref() {
                let output = self.space.output_under(location).next().cloned();
                if let Some(output) = output.as_ref() {
                    if let Some(surface) = lock.surface_for(output) {
                        keyboard.set_focus(
                            self,
                            Some(crate::focus::KeyboardFocusTarget::LockSurface(surface.clone())),
                            serial,
                        );
                        return;
                    }
                }
                keyboard.set_focus(self, None, serial);
                return;
            }

            let output = self.space.output_under(location).next().cloned();
            if let Some(output) = output.as_ref() {
                let output_geo = self.space.output_geometry(output).unwrap();
                if let Some(window) = output
                    .user_data()
                    .get::<FullscreenSurface>()
                    .and_then(|f| f.get())
                {
                    if let Some((_, _)) =
                        window.surface_under(location - output_geo.loc.to_f64(), WindowSurfaceType::ALL)
                    {
                        #[cfg(feature = "xwayland")]
                        if let Some(surface) = window.0.x11_surface() {
                            self.xwm.as_mut().unwrap().raise_window(surface).unwrap();
                        }
                        keyboard.set_focus(self, Some(window.into()), serial);
                        return;
                    }
                }

                let layers = layer_map_for_output(output);
                if let Some(layer) = layers
                    .layer_under(WlrLayer::Overlay, location - output_geo.loc.to_f64())
                    .or_else(|| layers.layer_under(WlrLayer::Top, location - output_geo.loc.to_f64()))
                {
                    if layer.can_receive_keyboard_focus() {
                        if let Some((_, _)) = layer.surface_under(
                            location
                                - output_geo.loc.to_f64()
                                - layers.layer_geometry(layer).unwrap().loc.to_f64(),
                            WindowSurfaceType::ALL,
                        ) {
                            keyboard.set_focus(self, Some(layer.clone().into()), serial);
                            return;
                        }
                    }
                }
            }

            if let Some((window, _)) = self.space.element_under(location).map(|(w, p)| (w.clone(), p)) {
                self.space.raise_element(&window, true);
                #[cfg(feature = "xwayland")]
                if let Some(surface) = window.0.x11_surface() {
                    self.xwm.as_mut().unwrap().raise_window(surface).unwrap();
                }
                keyboard.set_focus(self, Some(window.into()), serial);
                // W412: always-on-top windows stay above after focus change.
                self.raise_always_on_top_windows();
                return;
            }

            if let Some(output) = output.as_ref() {
                let output_geo = self.space.output_geometry(output).unwrap();
                let layers = layer_map_for_output(output);
                if let Some(layer) = layers
                    .layer_under(WlrLayer::Bottom, location - output_geo.loc.to_f64())
                    .or_else(|| layers.layer_under(WlrLayer::Background, location - output_geo.loc.to_f64()))
                {
                    if layer.can_receive_keyboard_focus() {
                        if let Some((_, _)) = layer.surface_under(
                            location
                                - output_geo.loc.to_f64()
                                - layers.layer_geometry(layer).unwrap().loc.to_f64(),
                            WindowSurfaceType::ALL,
                        ) {
                            keyboard.set_focus(self, Some(layer.clone().into()), serial);
                        }
                    }
                }
            };
        }
    }

    /// True when a right-click should open the empty-desktop menu (W388).
    fn desktop_context_hit(&self, pos: Point<f64, Logical>) -> bool {
        let Some(output) = self.space.outputs().find(|o| {
            let geometry = self.space.output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        }) else {
            return false;
        };
        let output_geo = self.space.output_geometry(output).unwrap();
        if output
            .user_data()
            .get::<FullscreenSurface>()
            .and_then(|f| f.get())
            .and_then(|w| w.surface_under(pos - output_geo.loc.to_f64(), WindowSurfaceType::ALL))
            .is_some()
        {
            return false;
        }
        let layers = layer_map_for_output(output);
        if layers
            .layer_under(WlrLayer::Overlay, pos - output_geo.loc.to_f64())
            .or_else(|| layers.layer_under(WlrLayer::Top, pos - output_geo.loc.to_f64()))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer.surface_under(
                    pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                    WindowSurfaceType::ALL,
                )
            })
            .is_some()
        {
            return false;
        }
        if self
            .space
            .element_under(pos)
            .and_then(|(window, loc)| {
                window.surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
            })
            .is_some()
        {
            return false;
        }
        true
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
        let output = self.space.outputs().find(|o| {
            let geometry = self.space.output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        })?;
        let output_geo = self.space.output_geometry(output).unwrap();

        // While locked, pointer hits only the lock surface for this output.
        if let Some(lock) = self.session_lock.as_ref() {
            if let Some(lock_surface) = lock.surface_for(output) {
                let surface = lock_surface.wl_surface().clone();
                return Some((PointerFocusTarget::from(surface), output_geo.loc.to_f64()));
            }
            return None;
        }

        let layers = layer_map_for_output(output);

        let mut under = None;
        if let Some((surface, loc)) = output
            .user_data()
            .get::<FullscreenSurface>()
            .and_then(|f| f.get())
            .and_then(|w| w.surface_under(pos - output_geo.loc.to_f64(), WindowSurfaceType::ALL))
        {
            under = Some((surface, loc + output_geo.loc));
        } else if let Some(focus) = layers
            .layer_under(WlrLayer::Overlay, pos - output_geo.loc.to_f64())
            .or_else(|| layers.layer_under(WlrLayer::Top, pos - output_geo.loc.to_f64()))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        } else if let Some(focus) = self.space.element_under(pos).and_then(|(window, loc)| {
            window
                .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
                .map(|(surface, surf_loc)| (surface, surf_loc + loc))
        }) {
            under = Some(focus);
        } else if let Some(focus) = layers
            .layer_under(WlrLayer::Bottom, pos - output_geo.loc.to_f64())
            .or_else(|| layers.layer_under(WlrLayer::Background, pos - output_geo.loc.to_f64()))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        };
        under.map(|(s, l)| (s, l.to_f64()))
    }

    fn on_pointer_axis<B: InputBackend>(&mut self, evt: B::PointerAxisEvent) {
        let horizontal_amount = evt
            .amount(input::Axis::Horizontal)
            .unwrap_or_else(|| evt.amount_v120(input::Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
        let vertical_amount = evt
            .amount(input::Axis::Vertical)
            .unwrap_or_else(|| evt.amount_v120(input::Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
        let horizontal_amount_discrete = evt.amount_v120(input::Axis::Horizontal);
        let vertical_amount_discrete = evt.amount_v120(input::Axis::Vertical);

        {
            let mut frame = AxisFrame::new(evt.time_msec()).source(evt.source());
            if horizontal_amount != 0.0 {
                frame = frame.relative_direction(Axis::Horizontal, evt.relative_direction(Axis::Horizontal));
                frame = frame.value(Axis::Horizontal, horizontal_amount);
                if let Some(discrete) = horizontal_amount_discrete {
                    frame = frame.v120(Axis::Horizontal, discrete as i32);
                }
            }
            if vertical_amount != 0.0 {
                frame = frame.relative_direction(Axis::Vertical, evt.relative_direction(Axis::Vertical));
                frame = frame.value(Axis::Vertical, vertical_amount);
                if let Some(discrete) = vertical_amount_discrete {
                    frame = frame.v120(Axis::Vertical, discrete as i32);
                }
            }
            if evt.source() == AxisSource::Finger {
                if evt.amount(Axis::Horizontal) == Some(0.0) {
                    frame = frame.stop(Axis::Horizontal);
                }
                if evt.amount(Axis::Vertical) == Some(0.0) {
                    frame = frame.stop(Axis::Vertical);
                }
            }
            let pointer = self.pointer.clone();
            pointer.axis(self, frame);
            pointer.frame(self);
        }
    }
}

#[cfg(any(feature = "winit", feature = "x11"))]
impl<BackendData: Backend> AnvilState<BackendData> {
    pub fn process_input_event_windowed<B: InputBackend>(&mut self, event: InputEvent<B>, output_name: &str) {
        match event {
            InputEvent::Keyboard { event } => match self.keyboard_key_to_action::<B>(event) {
                KeyAction::ScaleUp => {
                    let output = self
                        .space
                        .outputs()
                        .find(|o| o.name() == output_name)
                        .unwrap()
                        .clone();

                    let current_scale = output.current_scale().fractional_scale();
                    let new_scale = current_scale + 0.25;
                    output.change_current_state(None, None, Some(Scale::Fractional(new_scale)), None);

                    crate::shell::fixup_positions(&mut self.space, self.pointer.current_location());
                    self.backend_data.reset_buffers(&output);
                }

                KeyAction::ScaleDown => {
                    let output = self
                        .space
                        .outputs()
                        .find(|o| o.name() == output_name)
                        .unwrap()
                        .clone();

                    let current_scale = output.current_scale().fractional_scale();
                    let new_scale = f64::max(1.0, current_scale - 0.25);
                    output.change_current_state(None, None, Some(Scale::Fractional(new_scale)), None);

                    crate::shell::fixup_positions(&mut self.space, self.pointer.current_location());
                    self.backend_data.reset_buffers(&output);
                }

                KeyAction::RotateOutput => {
                    let output = self
                        .space
                        .outputs()
                        .find(|o| o.name() == output_name)
                        .unwrap()
                        .clone();

                    let current_transform = output.current_transform();
                    let new_transform = match current_transform {
                        Transform::Normal => Transform::_90,
                        Transform::_90 => Transform::_180,
                        Transform::_180 => Transform::_270,
                        Transform::_270 => Transform::Flipped,
                        Transform::Flipped => Transform::Flipped90,
                        Transform::Flipped90 => Transform::Flipped180,
                        Transform::Flipped180 => Transform::Flipped270,
                        Transform::Flipped270 => Transform::Normal,
                    };
                    tracing::info!(?current_transform, ?new_transform, output = ?output.name(), "changing output transform");
                    output.change_current_state(None, Some(new_transform), None, None);
                    crate::shell::fixup_positions(&mut self.space, self.pointer.current_location());
                    self.backend_data.reset_buffers(&output);
                }

                action => match action {
                    KeyAction::None
                    | KeyAction::Quit
                    | KeyAction::Run(_)
                    | KeyAction::Shell(_)
                    | KeyAction::ScreenshotWindow
                    | KeyAction::Lock
                    | KeyAction::Close
                    | KeyAction::AlwaysOnTopToggle
                    | KeyAction::Snap(_)
                    | KeyAction::MoveWindowOutput(_)
                    | KeyAction::Nudge(_, _)
                    | KeyAction::TogglePreview
                    | KeyAction::ToggleDecorations
                    | KeyAction::WorkspacePrev
                    | KeyAction::WorkspaceNext
                    | KeyAction::WorkspaceNew
                    | KeyAction::WorkspaceClose
                    | KeyAction::MinimizeOthers
                    | KeyAction::UnminimizeLast => self.process_common_key_action(action),

                    _ => tracing::warn!(
                        ?action,
                        output_name,
                        "Key action unsupported on on output backend.",
                    ),
                },
            },

            InputEvent::PointerMotionAbsolute { event } => {
                let output = self
                    .space
                    .outputs()
                    .find(|o| o.name() == output_name)
                    .unwrap()
                    .clone();
                self.on_pointer_move_absolute_windowed::<B>(event, &output)
            }
            InputEvent::PointerButton { event } => self.on_pointer_button::<B>(event),
            InputEvent::PointerAxis { event } => self.on_pointer_axis::<B>(event),
            _ => (), // other events are not handled in anvil (yet)
        }
    }

    fn on_pointer_move_absolute_windowed<B: InputBackend>(
        &mut self,
        evt: B::PointerMotionAbsoluteEvent,
        output: &Output,
    ) {
        let output_geo = self.space.output_geometry(output).unwrap();

        let pos = evt.position_transformed(output_geo.size) + output_geo.loc.to_f64();
        let serial = SCOUNTER.next_serial();

        let pointer = self.pointer.clone();
        let under = self.surface_under(pos);
        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pos,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    pub fn release_all_keys(&mut self) {
        let keyboard = self.seat.get_keyboard().unwrap();
        for keycode in keyboard.pressed_keys() {
            keyboard.input(
                self,
                keycode,
                KeyState::Released,
                SCOUNTER.next_serial(),
                0,
                |_, _, _| FilterResult::Forward::<bool>,
            );
        }
    }
}

#[cfg(feature = "udev")]
impl AnvilState<UdevData> {
    pub fn process_input_event<B: InputBackend>(&mut self, dh: &DisplayHandle, event: InputEvent<B>) {
        match event {
            InputEvent::Keyboard { event, .. } => match self.keyboard_key_to_action::<B>(event) {
                #[cfg(feature = "udev")]
                KeyAction::VtSwitch(vt) => {
                    info!(to = vt, "Trying to switch vt");
                    if let Err(err) = self.backend_data.session.change_vt(vt) {
                        error!(vt, "Error switching vt: {}", err);
                    }
                }
                KeyAction::Screen(num) => {
                    let geometry = self
                        .space
                        .outputs()
                        .nth(num)
                        .map(|o| self.space.output_geometry(o).unwrap());

                    if let Some(geometry) = geometry {
                        let x = geometry.loc.x as f64 + geometry.size.w as f64 / 2.0;
                        let y = geometry.size.h as f64 / 2.0;
                        let location = (x, y).into();
                        let pointer = self.pointer.clone();
                        let under = self.surface_under(location);
                        pointer.motion(
                            self,
                            under,
                            &MotionEvent {
                                location,
                                serial: SCOUNTER.next_serial(),
                                time: self.clock.now().as_millis(),
                            },
                        );
                        pointer.frame(self);
                    }
                }
                KeyAction::ScaleUp => {
                    let pos = self.pointer.current_location().to_i32_round();
                    let output = self
                        .space
                        .outputs()
                        .find(|o| self.space.output_geometry(o).unwrap().contains(pos))
                        .cloned();

                    if let Some(output) = output {
                        let (output_location, scale) = (
                            self.space.output_geometry(&output).unwrap().loc,
                            output.current_scale().fractional_scale(),
                        );
                        let new_scale = scale + 0.25;
                        output.change_current_state(None, None, Some(Scale::Fractional(new_scale)), None);

                        let rescale = scale / new_scale;
                        let output_location = output_location.to_f64();
                        let mut pointer_output_location = self.pointer.current_location() - output_location;
                        pointer_output_location.x *= rescale;
                        pointer_output_location.y *= rescale;
                        let pointer_location = output_location + pointer_output_location;

                        crate::shell::fixup_positions(&mut self.space, pointer_location);
                        let pointer = self.pointer.clone();
                        let under = self.surface_under(pointer_location);
                        pointer.motion(
                            self,
                            under,
                            &MotionEvent {
                                location: pointer_location,
                                serial: SCOUNTER.next_serial(),
                                time: self.clock.now().as_millis(),
                            },
                        );
                        pointer.frame(self);
                        self.backend_data.reset_buffers(&output);
                    }
                }
                KeyAction::ScaleDown => {
                    let pos = self.pointer.current_location().to_i32_round();
                    let output = self
                        .space
                        .outputs()
                        .find(|o| self.space.output_geometry(o).unwrap().contains(pos))
                        .cloned();

                    if let Some(output) = output {
                        let (output_location, scale) = (
                            self.space.output_geometry(&output).unwrap().loc,
                            output.current_scale().fractional_scale(),
                        );
                        let new_scale = f64::max(1.0, scale - 0.25);
                        output.change_current_state(None, None, Some(Scale::Fractional(new_scale)), None);

                        let rescale = scale / new_scale;
                        let output_location = output_location.to_f64();
                        let mut pointer_output_location = self.pointer.current_location() - output_location;
                        pointer_output_location.x *= rescale;
                        pointer_output_location.y *= rescale;
                        let pointer_location = output_location + pointer_output_location;

                        crate::shell::fixup_positions(&mut self.space, pointer_location);
                        let pointer = self.pointer.clone();
                        let under = self.surface_under(pointer_location);
                        pointer.motion(
                            self,
                            under,
                            &MotionEvent {
                                location: pointer_location,
                                serial: SCOUNTER.next_serial(),
                                time: self.clock.now().as_millis(),
                            },
                        );
                        pointer.frame(self);
                        self.backend_data.reset_buffers(&output);
                    }
                }
                KeyAction::RotateOutput => {
                    let pos = self.pointer.current_location().to_i32_round();
                    let output = self
                        .space
                        .outputs()
                        .find(|o| self.space.output_geometry(o).unwrap().contains(pos))
                        .cloned();

                    if let Some(output) = output {
                        let current_transform = output.current_transform();
                        let new_transform = match current_transform {
                            Transform::Normal => Transform::_90,
                            Transform::_90 => Transform::_180,
                            Transform::_180 => Transform::_270,
                            Transform::_270 => Transform::Flipped,
                            Transform::Flipped => Transform::Flipped90,
                            Transform::Flipped90 => Transform::Flipped180,
                            Transform::Flipped180 => Transform::Flipped270,
                            Transform::Flipped270 => Transform::Normal,
                        };
                        output.change_current_state(None, Some(new_transform), None, None);
                        crate::shell::fixup_positions(&mut self.space, self.pointer.current_location());
                        self.backend_data.reset_buffers(&output);
                    }
                }
                KeyAction::ToggleTint => {
                    let mut debug_flags = self.backend_data.debug_flags();
                    debug_flags.toggle(DebugFlags::TINT);
                    self.backend_data.set_debug_flags(debug_flags);
                }

                action => match action {
                    KeyAction::None
                    | KeyAction::Quit
                    | KeyAction::Run(_)
                    | KeyAction::Shell(_)
                    | KeyAction::ScreenshotWindow
                    | KeyAction::Lock
                    | KeyAction::Close
                    | KeyAction::AlwaysOnTopToggle
                    | KeyAction::Snap(_)
                    | KeyAction::MoveWindowOutput(_)
                    | KeyAction::Nudge(_, _)
                    | KeyAction::TogglePreview
                    | KeyAction::ToggleDecorations
                    | KeyAction::WorkspacePrev
                    | KeyAction::WorkspaceNext
                    | KeyAction::WorkspaceNew
                    | KeyAction::WorkspaceClose
                    | KeyAction::MinimizeOthers
                    | KeyAction::UnminimizeLast => self.process_common_key_action(action),

                    _ => unreachable!(),
                },
            },
            InputEvent::PointerMotion { event, .. } => self.on_pointer_move::<B>(dh, event),
            InputEvent::PointerMotionAbsolute { event, .. } => self.on_pointer_move_absolute::<B>(dh, event),
            InputEvent::PointerButton { event, .. } => self.on_pointer_button::<B>(event),
            InputEvent::PointerAxis { event, .. } => self.on_pointer_axis::<B>(event),
            InputEvent::TabletToolAxis { event, .. } => self.on_tablet_tool_axis::<B>(event),
            InputEvent::TabletToolProximity { event, .. } => self.on_tablet_tool_proximity::<B>(dh, event),
            InputEvent::TabletToolTip { event, .. } => self.on_tablet_tool_tip::<B>(event),
            InputEvent::TabletToolButton { event, .. } => self.on_tablet_button::<B>(event),
            InputEvent::GestureSwipeBegin { event, .. } => self.on_gesture_swipe_begin::<B>(event),
            InputEvent::GestureSwipeUpdate { event, .. } => self.on_gesture_swipe_update::<B>(event),
            InputEvent::GestureSwipeEnd { event, .. } => self.on_gesture_swipe_end::<B>(event),
            InputEvent::GesturePinchBegin { event, .. } => self.on_gesture_pinch_begin::<B>(event),
            InputEvent::GesturePinchUpdate { event, .. } => self.on_gesture_pinch_update::<B>(event),
            InputEvent::GesturePinchEnd { event, .. } => self.on_gesture_pinch_end::<B>(event),
            InputEvent::GestureHoldBegin { event, .. } => self.on_gesture_hold_begin::<B>(event),
            InputEvent::GestureHoldEnd { event, .. } => self.on_gesture_hold_end::<B>(event),

            InputEvent::TouchDown { event } => self.on_touch_down::<B>(event),
            InputEvent::TouchUp { event } => self.on_touch_up::<B>(event),
            InputEvent::TouchMotion { event } => self.on_touch_motion::<B>(event),
            InputEvent::TouchFrame { event } => self.on_touch_frame::<B>(event),
            InputEvent::TouchCancel { event } => self.on_touch_cancel::<B>(event),

            InputEvent::DeviceAdded { device } => {
                if device.has_capability(DeviceCapability::TabletTool) {
                    self.seat
                        .tablet_seat()
                        .add_tablet::<Self>(dh, &TabletDescriptor::from(&device));
                }
                if device.has_capability(DeviceCapability::Touch) && self.seat.get_touch().is_none() {
                    self.seat.add_touch();
                }
            }
            InputEvent::DeviceRemoved { device } => {
                if device.has_capability(DeviceCapability::TabletTool) {
                    let tablet_seat = self.seat.tablet_seat();

                    tablet_seat.remove_tablet(&TabletDescriptor::from(&device));

                    // If there are no tablets in seat we can remove all tools
                    if tablet_seat.count_tablets() == 0 {
                        tablet_seat.clear_tools();
                    }
                }
            }
            _ => {
                // other events are not handled in anvil (yet)
            }
        }
    }

    fn on_pointer_move<B: InputBackend>(&mut self, _dh: &DisplayHandle, evt: B::PointerMotionEvent) {
        let mut pointer_location = self.pointer.current_location();
        let serial = SCOUNTER.next_serial();

        let pointer = self.pointer.clone();
        let under = self.surface_under(pointer_location);

        let mut pointer_locked = false;
        let mut pointer_confined = false;
        let mut confine_region = None;
        if let Some((surface, surface_loc)) = under
            .as_ref()
            .and_then(|(target, l)| Some((target.wl_surface()?, l)))
        {
            with_pointer_constraint(&surface, &pointer, |constraint| match constraint {
                Some(constraint) if constraint.is_active() => {
                    // Constraint does not apply if not within region
                    if !constraint.region().map_or(true, |x| {
                        x.contains((pointer_location - *surface_loc).to_i32_round())
                    }) {
                        return;
                    }
                    match &*constraint {
                        PointerConstraint::Locked(_locked) => {
                            pointer_locked = true;
                        }
                        PointerConstraint::Confined(confine) => {
                            pointer_confined = true;
                            confine_region = confine.region().cloned();
                        }
                    }
                }
                _ => {}
            });
        }

        pointer.relative_motion(
            self,
            under.clone(),
            &RelativeMotionEvent {
                delta: evt.delta(),
                delta_unaccel: evt.delta_unaccel(),
                utime: evt.time(),
            },
        );

        // If pointer is locked, only emit relative motion
        if pointer_locked {
            pointer.frame(self);
            return;
        }

        pointer_location += evt.delta();

        // clamp to screen limits
        // this event is never generated by winit
        pointer_location = self.clamp_coords(pointer_location);

        let new_under = self.surface_under(pointer_location);

        // If confined, don't move pointer if it would go outside surface or region
        if pointer_confined {
            if let Some((surface, surface_loc)) = &under {
                if new_under.as_ref().and_then(|(under, _)| under.wl_surface()) != surface.wl_surface() {
                    pointer.frame(self);
                    return;
                }
                if let Some(region) = confine_region {
                    if !region.contains((pointer_location - *surface_loc).to_i32_round()) {
                        pointer.frame(self);
                        return;
                    }
                }
            }
        }

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pointer_location,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);

        // If pointer is now in a constraint region, activate it
        // TODO Anywhere else pointer is moved needs to do this
        if let Some((under, surface_location)) =
            new_under.and_then(|(target, loc)| Some((target.wl_surface()?.into_owned(), loc)))
        {
            with_pointer_constraint(&under, &pointer, |constraint| match constraint {
                Some(constraint) if !constraint.is_active() => {
                    let point = (pointer_location - surface_location).to_i32_round();
                    if constraint.region().map_or(true, |region| region.contains(point)) {
                        constraint.activate();
                    }
                }
                _ => {}
            });
        }
    }

    fn on_pointer_move_absolute<B: InputBackend>(
        &mut self,
        _dh: &DisplayHandle,
        evt: B::PointerMotionAbsoluteEvent,
    ) {
        let serial = SCOUNTER.next_serial();

        let max_x = self
            .space
            .outputs()
            .fold(0, |acc, o| acc + self.space.output_geometry(o).unwrap().size.w);

        let max_h_output = self
            .space
            .outputs()
            .max_by_key(|o| self.space.output_geometry(o).unwrap().size.h)
            .unwrap();

        let max_y = self.space.output_geometry(max_h_output).unwrap().size.h;

        let mut pointer_location = (evt.x_transformed(max_x), evt.y_transformed(max_y)).into();

        // clamp to screen limits
        pointer_location = self.clamp_coords(pointer_location);

        let pointer = self.pointer.clone();
        let under = self.surface_under(pointer_location);

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pointer_location,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn on_tablet_tool_axis<B: InputBackend>(&mut self, evt: B::TabletToolAxisEvent) {
        let tablet_seat = self.seat.tablet_seat();

        if let Some(pointer_location) = self.touch_location_transformed(&evt) {
            let pointer = self.pointer.clone();
            let under = self.surface_under(pointer_location);
            let tablet = tablet_seat.get_tablet(&TabletDescriptor::from(&evt.device()));
            let tool = tablet_seat.get_tool(&evt.tool());

            pointer.motion(
                self,
                under.clone(),
                &MotionEvent {
                    location: pointer_location,
                    serial: SCOUNTER.next_serial(),
                    time: self.clock.now().as_millis(),
                },
            );

            if let (Some(tablet), Some(tool)) = (tablet, tool) {
                if evt.pressure_has_changed() {
                    tool.pressure(evt.pressure());
                }
                if evt.distance_has_changed() {
                    tool.distance(evt.distance());
                }
                if evt.tilt_has_changed() {
                    tool.tilt(evt.tilt());
                }
                if evt.slider_has_changed() {
                    tool.slider_position(evt.slider_position());
                }
                if evt.rotation_has_changed() {
                    tool.rotation(evt.rotation());
                }
                if evt.wheel_has_changed() {
                    tool.wheel(evt.wheel_delta(), evt.wheel_delta_discrete());
                }

                tool.motion(
                    pointer_location,
                    under.and_then(|(f, loc)| f.wl_surface().map(|s| (s.into_owned(), loc))),
                    &tablet,
                    SCOUNTER.next_serial(),
                    evt.time_msec(),
                );
            }

            pointer.frame(self);
        }
    }

    fn on_tablet_tool_proximity<B: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        evt: B::TabletToolProximityEvent,
    ) {
        let tablet_seat = self.seat.tablet_seat();

        if let Some(pointer_location) = self.touch_location_transformed(&evt) {
            let tool = evt.tool();
            tablet_seat.add_tool::<Self>(self, dh, &tool);

            let pointer = self.pointer.clone();
            let under = self.surface_under(pointer_location);
            let tablet = tablet_seat.get_tablet(&TabletDescriptor::from(&evt.device()));
            let tool = tablet_seat.get_tool(&tool);

            pointer.motion(
                self,
                under.clone(),
                &MotionEvent {
                    location: pointer_location,
                    serial: SCOUNTER.next_serial(),
                    time: evt.time_msec(),
                },
            );
            pointer.frame(self);

            if let (Some(under), Some(tablet), Some(tool)) = (
                under.and_then(|(f, loc)| f.wl_surface().map(|s| (s.into_owned(), loc))),
                tablet,
                tool,
            ) {
                match evt.state() {
                    ProximityState::In => tool.proximity_in(
                        pointer_location,
                        under,
                        &tablet,
                        SCOUNTER.next_serial(),
                        evt.time_msec(),
                    ),
                    ProximityState::Out => tool.proximity_out(evt.time_msec()),
                }
            }
        }
    }

    fn on_tablet_tool_tip<B: InputBackend>(&mut self, evt: B::TabletToolTipEvent) {
        let tool = self.seat.tablet_seat().get_tool(&evt.tool());

        if let Some(tool) = tool {
            match evt.tip_state() {
                TabletToolTipState::Down => {
                    let serial = SCOUNTER.next_serial();
                    tool.tip_down(serial, evt.time_msec());

                    // change the keyboard focus
                    self.update_keyboard_focus(self.pointer.current_location(), serial);
                }
                TabletToolTipState::Up => {
                    tool.tip_up(evt.time_msec());
                }
            }
        }
    }

    fn on_tablet_button<B: InputBackend>(&mut self, evt: B::TabletToolButtonEvent) {
        let tool = self.seat.tablet_seat().get_tool(&evt.tool());

        if let Some(tool) = tool {
            tool.button(
                evt.button(),
                evt.button_state(),
                SCOUNTER.next_serial(),
                evt.time_msec(),
            );
        }
    }

    fn on_gesture_swipe_begin<B: InputBackend>(&mut self, evt: B::GestureSwipeBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_swipe_begin(
            self,
            &GestureSwipeBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_swipe_update<B: InputBackend>(&mut self, evt: B::GestureSwipeUpdateEvent) {
        let pointer = self.pointer.clone();
        pointer.gesture_swipe_update(
            self,
            &GestureSwipeUpdateEvent {
                time: evt.time_msec(),
                delta: evt.delta(),
            },
        );
    }

    fn on_gesture_swipe_end<B: InputBackend>(&mut self, evt: B::GestureSwipeEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_swipe_end(
            self,
            &GestureSwipeEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn on_gesture_pinch_begin<B: InputBackend>(&mut self, evt: B::GesturePinchBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_pinch_begin(
            self,
            &GesturePinchBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_pinch_update<B: InputBackend>(&mut self, evt: B::GesturePinchUpdateEvent) {
        let pointer = self.pointer.clone();
        pointer.gesture_pinch_update(
            self,
            &GesturePinchUpdateEvent {
                time: evt.time_msec(),
                delta: evt.delta(),
                scale: evt.scale(),
                rotation: evt.rotation(),
            },
        );
    }

    fn on_gesture_pinch_end<B: InputBackend>(&mut self, evt: B::GesturePinchEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_pinch_end(
            self,
            &GesturePinchEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn on_gesture_hold_begin<B: InputBackend>(&mut self, evt: B::GestureHoldBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_hold_begin(
            self,
            &GestureHoldBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_hold_end<B: InputBackend>(&mut self, evt: B::GestureHoldEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone();
        pointer.gesture_hold_end(
            self,
            &GestureHoldEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn touch_location_transformed<B: InputBackend, E: AbsolutePositionEvent<B>>(
        &self,
        evt: &E,
    ) -> Option<Point<f64, Logical>> {
        let output = self
            .space
            .outputs()
            .find(|output| output.name().starts_with("eDP"))
            .or_else(|| self.space.outputs().next());

        let output = output?;
        let output_geometry = self.space.output_geometry(output)?;

        let transform = output.current_transform();
        let size = transform.invert().transform_size(output_geometry.size);
        Some(
            transform.transform_point_in(evt.position_transformed(size), &size.to_f64())
                + output_geometry.loc.to_f64(),
        )
    }

    fn on_touch_down<B: InputBackend>(&mut self, evt: B::TouchDownEvent) {
        let Some(handle) = self.seat.get_touch() else {
            return;
        };

        let Some(touch_location) = self.touch_location_transformed(&evt) else {
            return;
        };

        let serial = SCOUNTER.next_serial();
        self.update_keyboard_focus(touch_location, serial);

        let under = self.surface_under(touch_location);
        handle.down(
            self,
            under,
            &DownEvent {
                slot: evt.slot(),
                location: touch_location,
                serial,
                time: evt.time_msec(),
            },
        );
    }
    fn on_touch_up<B: InputBackend>(&mut self, evt: B::TouchUpEvent) {
        let Some(handle) = self.seat.get_touch() else {
            return;
        };
        let serial = SCOUNTER.next_serial();
        handle.up(
            self,
            &UpEvent {
                slot: evt.slot(),
                serial,
                time: evt.time_msec(),
            },
        )
    }
    fn on_touch_motion<B: InputBackend>(&mut self, evt: B::TouchMotionEvent) {
        let Some(handle) = self.seat.get_touch() else {
            return;
        };
        let Some(touch_location) = self.touch_location_transformed(&evt) else {
            return;
        };

        let under = self.surface_under(touch_location);
        handle.motion(
            self,
            under,
            &smithay::input::touch::MotionEvent {
                slot: evt.slot(),
                location: touch_location,
                time: evt.time_msec(),
            },
        );
    }
    fn on_touch_frame<B: InputBackend>(&mut self, _evt: B::TouchFrameEvent) {
        let Some(handle) = self.seat.get_touch() else {
            return;
        };
        handle.frame(self);
    }
    fn on_touch_cancel<B: InputBackend>(&mut self, _evt: B::TouchCancelEvent) {
        let Some(handle) = self.seat.get_touch() else {
            return;
        };
        handle.cancel(self);
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.space.outputs().next().is_none() {
            return pos;
        }

        let (pos_x, pos_y) = pos.into();
        let max_x = self
            .space
            .outputs()
            .fold(0, |acc, o| acc + self.space.output_geometry(o).unwrap().size.w);
        let clamped_x = pos_x.clamp(0.0, max_x as f64);
        let max_y = self
            .space
            .outputs()
            .find(|o| {
                let geo = self.space.output_geometry(o).unwrap();
                geo.contains((clamped_x as i32, 0))
            })
            .map(|o| self.space.output_geometry(o).unwrap().size.h);

        if let Some(max_y) = max_y {
            let clamped_y = pos_y.clamp(0.0, max_y as f64);
            (clamped_x, clamped_y).into()
        } else {
            (clamped_x, pos_y).into()
        }
    }
}

/// Win11-style snap direction (Super+Arrow / snap flyout).
#[derive(Debug, Clone, Copy)]
enum SnapDir {
    Left,
    Right,
    Up,
    Down,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// Left two-thirds (Win11 snap layout).
    LeftTwoThirds,
    /// Right two-thirds.
    RightTwoThirds,
    /// Left / center / right one-third columns (W102).
    LeftThird,
    CenterThird,
    RightThird,
    /// Top / bottom vertical half (W182 · Super+Alt+Up/Down).
    TopHalf,
    BottomHalf,
    /// Center focused window in the work area (W231).
    Center,
}

fn snap_layout_to_dir(layout: mde_protocol::SnapLayout) -> SnapDir {
    match layout {
        mde_protocol::SnapLayout::Left => SnapDir::Left,
        mde_protocol::SnapLayout::Right => SnapDir::Right,
        mde_protocol::SnapLayout::TopLeft => SnapDir::TopLeft,
        mde_protocol::SnapLayout::TopRight => SnapDir::TopRight,
        mde_protocol::SnapLayout::BottomLeft => SnapDir::BottomLeft,
        mde_protocol::SnapLayout::BottomRight => SnapDir::BottomRight,
        mde_protocol::SnapLayout::LeftTwoThirds => SnapDir::LeftTwoThirds,
        mde_protocol::SnapLayout::RightTwoThirds => SnapDir::RightTwoThirds,
        mde_protocol::SnapLayout::LeftThird => SnapDir::LeftThird,
        mde_protocol::SnapLayout::CenterThird => SnapDir::CenterThird,
        mde_protocol::SnapLayout::RightThird => SnapDir::RightThird,
        mde_protocol::SnapLayout::Maximize => SnapDir::Up,
        mde_protocol::SnapLayout::Restore => SnapDir::Down,
        mde_protocol::SnapLayout::TopHalf => SnapDir::TopHalf,
        mde_protocol::SnapLayout::BottomHalf => SnapDir::BottomHalf,
        mde_protocol::SnapLayout::Center => SnapDir::Center,
    }
}

/// Possible results of a keyboard action
#[allow(dead_code)] // some of these are only read if udev is enabled
#[derive(Debug)]
enum KeyAction {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// run a command
    Run(String),
    /// One-shot command for mde-shell (`$XDG_RUNTIME_DIR/mde-shell.cmd`).
    Shell(String),
    /// Super+Print — focused window geometry → screenshot:window (W508).
    ScreenshotWindow,
    /// Switch the current screen
    Screen(usize),
    /// Snap / maximize / restore focused window (Win+Arrow).
    Snap(SnapDir),
    /// Move focused window to adjacent output (Super+Shift+Left/Right) — W5 / W171.
    MoveWindowOutput(i32),
    /// Super+Alt+Left/Right — nudge focused window by 20px (W251).
    Nudge(i32, i32),
    ScaleUp,
    ScaleDown,
    TogglePreview,
    RotateOutput,
    ToggleTint,
    ToggleDecorations,
    /// Super+Ctrl+Left — previous virtual desktop.
    WorkspacePrev,
    /// Super+Ctrl+Right — next virtual desktop.
    WorkspaceNext,
    /// Super+Ctrl+D — create desktop.
    WorkspaceNew,
    /// Super+Ctrl+F4 — close/merge current desktop.
    WorkspaceClose,
    /// Super+Home — minimize all unfocused windows (Aero shake light).
    MinimizeOthers,
    /// Super+Shift+M — restore most recently minimized window (W232).
    UnminimizeLast,
    /// Super+L — lock via mde-lock / loginctl (W121).
    Lock,
    /// Alt+F4 — close focused window (W148).
    Close,
    /// A018: Win+Ctrl+T — toggle always-on-top on the focused window.
    AlwaysOnTopToggle,
    /// Do nothing more
    None,
}

/// PrtSc / Print Screen across layouts: bare key is `Print`, Shift level is often
/// `Sys_Req`, and some maps use `3270_PrintScreen`.
fn is_print_screen(keysym: Keysym) -> bool {
    keysym == Keysym::Print
        || keysym == Keysym::Sys_Req
        || keysym == Keysym::_3270_PrintScreen
        || keysym == Keysym::SUN_Print_Screen
}

/// Linux evdev KEY_SYSRQ (99) / KEY_PRINT (210) as XKB keycodes (evdev + 8).
fn is_print_keycode(keycode: Keycode) -> bool {
    matches!(keycode.raw(), 107 | 218)
}

fn process_keyboard_shortcut(modifiers: ModifiersState, keysym: Keysym) -> Option<KeyAction> {

    // A024: Ctrl pressed twice quickly → Find My Mouse.
    if !modifiers.logo
        && !modifiers.shift
        && !modifiers.alt
        && (keysym == Keysym::Control_L || keysym == Keysym::Control_R)
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static LAST_CTRL_MS: AtomicU64 = AtomicU64::new(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let prev = LAST_CTRL_MS.swap(now, Ordering::SeqCst);
        if prev > 0 && now.saturating_sub(prev) < 400 {
            LAST_CTRL_MS.store(0, Ordering::SeqCst);
            return Some(KeyAction::Shell("findmouse".into()));
        }
    }

    // A022: apply Keyboard Manager remaps from shell-published file.
    if let Some(mapped) = apply_key_remap(keysym) {
        // Re-enter with remapped keysym (no modifier change).
        return process_keyboard_shortcut(modifiers, mapped);
    }
    if modifiers.ctrl && modifiers.alt && keysym == Keysym::BackSpace || modifiers.logo && keysym == Keysym::q
    {
        // ctrl+alt+backspace = quit
        // logo + q = quit
        Some(KeyAction::Quit)
    } else if (xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12).contains(&keysym.raw()) {
        // VTSwitch
        Some(KeyAction::VtSwitch(
            (keysym.raw() - xkb::KEY_XF86Switch_VT_1 + 1) as i32,
        ))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::l
    {
        // W121: Super+L lock screen.
        Some(KeyAction::Lock)
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::h || keysym == Keysym::H)
    {
        // W201: Super+H voice typing / dictation stub → shell.
        Some(KeyAction::Shell("dictation".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::u || keysym == Keysym::U)
    {
        // W221: Super+U Ease of Access → shell opens Accessibility.
        Some(KeyAction::Shell("ease".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::k || keysym == Keysym::K)
    {
        // W241: Super+K Cast / Connect stub → shell.
        Some(KeyAction::Shell("connect".into()))
    } else if modifiers.logo
        && modifiers.ctrl
        && modifiers.shift
        && !modifiers.alt
        && (keysym == Keysym::b || keysym == Keysym::B)
    {
        // W172: Super+Ctrl+Shift+B soft GPU reset tip → shell.
        Some(KeyAction::Shell("gpu-reset".into()))
    } else if modifiers.logo
        && modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Left
    {
        // W171: Super+Shift+Left/Right moves focused window to adjacent monitor.
        Some(KeyAction::MoveWindowOutput(-1))
    } else if modifiers.logo
        && modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Right
    {
        // W171: Super+Shift+Left/Right moves focused window to adjacent monitor.
        Some(KeyAction::MoveWindowOutput(1))
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && keysym == Keysym::Left
    {
        Some(KeyAction::WorkspacePrev)
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && keysym == Keysym::Right
    {
        Some(KeyAction::WorkspaceNext)
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && keysym == Keysym::d
    {
        Some(KeyAction::WorkspaceNew)
    } else if modifiers.logo
        && modifiers.alt
        && !modifiers.ctrl
        && !modifiers.shift
        && (keysym == Keysym::d || keysym == Keysym::D)
    {
        // W192: Super+Alt+D → Notification Centre calendar (was unbound).
        Some(KeyAction::Shell("date".into()))
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && keysym == Keysym::F4
    {
        Some(KeyAction::WorkspaceClose)
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && (keysym == Keysym::f || keysym == Keysym::F)
    {
        // W212: Super+Ctrl+F → Start search (shell OpenStartSearch).
        Some(KeyAction::Shell("startsearch".into()))
    } else if modifiers.alt
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.shift
        && keysym == Keysym::F4
    {
        // W148: Alt+F4 close focused (not Super+Ctrl+F4 desktop close).
        Some(KeyAction::Close)
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Left
    {
        Some(KeyAction::Snap(SnapDir::Left))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Right
    {
        Some(KeyAction::Snap(SnapDir::Right))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Up
    {
        Some(KeyAction::Snap(SnapDir::Up))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Down
    {
        Some(KeyAction::Snap(SnapDir::Down))
    } else if modifiers.logo
        && modifiers.alt
        && !modifiers.shift
        && !modifiers.ctrl
        && keysym == Keysym::Up
    {
        // W182: Super+Alt+Up → top half.
        Some(KeyAction::Snap(SnapDir::TopHalf))
    } else if modifiers.logo
        && modifiers.alt
        && !modifiers.shift
        && !modifiers.ctrl
        && keysym == Keysym::Down
    {
        // W182: Super+Alt+Down → bottom half.
        Some(KeyAction::Snap(SnapDir::BottomHalf))
    } else if modifiers.logo
        && modifiers.alt
        && !modifiers.shift
        && !modifiers.ctrl
        && keysym == Keysym::Left
    {
        // W251: Super+Alt+Left → nudge 20px left (Up/Down remain half-snap).
        Some(KeyAction::Nudge(-20, 0))
    } else if modifiers.logo
        && modifiers.alt
        && !modifiers.shift
        && !modifiers.ctrl
        && keysym == Keysym::Right
    {
        // W251: Super+Alt+Right → nudge 20px right.
        Some(KeyAction::Nudge(20, 0))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && modifiers.alt
        && keysym == Keysym::space
    {
        // A015: Super+Alt+Space → Command Palette (enhanced Run).
        Some(KeyAction::Shell("run".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::space
    {
        // W261: Super+Space → input language tip (Start remains taskbar / other binds).
        Some(KeyAction::Shell("inputlang".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::t || keysym == Keysym::T)
    {
        // A018: Win+Ctrl+T → Always on top toggle.
        Some(KeyAction::AlwaysOnTopToggle)
    } else if modifiers.logo
        && !modifiers.shift
        && modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::f || keysym == Keysym::F)
    {
        // A024: Super+Ctrl+F → Find My Mouse spotlight.
        Some(KeyAction::Shell("findmouse".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::e
    {
        // Win11 Super+E → File Explorer (shell opens Documents/home).
        Some(KeyAction::Shell("explorer".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::i
    {
        // Win11 Super+I → Settings.
        Some(KeyAction::Shell("settings".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym.raw() == xkb::KEY_comma
    {
        // W181: Super+Comma → Settings (unbound elsewhere; Super+I also opens Settings).
        Some(KeyAction::Shell("settings".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::d
    {
        // Win11 Super+D → Show desktop (Super+Ctrl+D remains new desktop).
        Some(KeyAction::Shell("showdesktop".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Home
    {
        // Light Aero Shake: minimize all but focused (W73).
        Some(KeyAction::MinimizeOthers)
    } else if modifiers.logo
        && modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::m || keysym == Keysym::M)
    {
        // W232: Super+Shift+M restore most recently minimized.
        Some(KeyAction::UnminimizeLast)
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::Escape
    {
        // W228: Win+Escape exit magnifier stub → shell.
        Some(KeyAction::Shell("magnifier-off".into()))
    } else if modifiers.logo
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::plus
            || keysym == Keysym::KP_Add
            || keysym == Keysym::equal)
    {
        // W272: Super+Plus / Super+equal magnifier stub → shell.
        Some(KeyAction::Shell("magnifier".into()))
    } else if modifiers.logo
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::slash || keysym == Keysym::question)
    {
        // A073: Super+/ Shortcut Guide (PowerToys-style).
        Some(KeyAction::Shell("shortcutguide".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::z
    {
        // Win11 Super+Z snap layout picker → shell flyout writes mde-snap.req.
        Some(KeyAction::Shell("snapflyout".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::w
    {
        Some(KeyAction::Shell("widgets".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::v
    {
        // Win11 Super+V clipboard history flyout.
        Some(KeyAction::Shell("clipboard".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym.raw() == xkb::KEY_period
    {
        // Win11 Super+. emoji panel stub (W90).
        Some(KeyAction::Shell("emoji".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::a
    {
        // Win11 Super+A Action Center → Quick Settings.
        Some(KeyAction::Shell("quick".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym == Keysym::n
    {
        // Win11 Super+N notification centre.
        Some(KeyAction::Shell("notifications".into()))
    } else if modifiers.alt
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.shift
        && keysym == Keysym::space
    {
        // Win11 Alt+Space → light window actions menu (shell → mde-window-action.req).
        Some(KeyAction::Shell("winmenu".into()))
    } else if modifiers.alt
        && !modifiers.logo
        && !modifiers.ctrl
        && modifiers.shift
        && keysym == Keysym::Tab
    {
        // W248: Alt+Shift+Tab → reverse Alt+Tab cycle.
        Some(KeyAction::Shell("alttab-rev".into()))
    } else if modifiers.alt
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.shift
        && keysym == Keysym::Tab
    {
        // Win11 Alt+Tab app switcher → shell cycles focus (+ brief overlay).
        Some(KeyAction::Shell("alttab".into()))
    } else if modifiers.alt
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.shift
        && keysym == Keysym::Escape
    {
        // Win11 Alt+Esc → quiet cycle (no Alt+Tab overlay) (W108).
        Some(KeyAction::Shell("alttab-quiet".into()))
    } else if modifiers.ctrl
        && modifiers.shift
        && !modifiers.logo
        && !modifiers.alt
        && keysym == Keysym::Escape
    {
        // A081: Ctrl+Shift+Esc → MMC Performance Monitor.
        Some(KeyAction::Shell("perfmon".into()))
    } else if modifiers.logo
        && modifiers.ctrl
        && modifiers.shift
        && modifiers.alt
        && (keysym == Keysym::r || keysym == Keysym::R)
    {
        // W252: Super+Ctrl+Shift+Alt+R → copy diagnostics stub.
        Some(KeyAction::Shell("diag".into()))
    } else if modifiers.logo
        && modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::s || keysym == Keysym::S)
    {
        // Win11 Super+Shift+S snip (W112 / W500).
        Some(KeyAction::Shell("screenshot".into()))
    } else if is_print_screen(keysym)
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.alt
        && modifiers.shift
    {
        // Shift+PrtSc — open snip overlay (rectangular) (W515).
        Some(KeyAction::Shell("screenshot:area".into()))
    } else if is_print_screen(keysym)
        && modifiers.logo
        && !modifiers.ctrl
        && !modifiers.alt
        && !modifiers.shift
    {
        // Super+PrtSc — snip overlay with Window preselected (W515).
        Some(KeyAction::ScreenshotWindow)
    } else if is_print_screen(keysym)
        && !modifiers.logo
        && !modifiers.ctrl
        && !modifiers.alt
        && !modifiers.shift
    {
        // PrtSc — open snip overlay (W515; Win11 “Use Print screen to open snipping”).
        Some(KeyAction::Shell("screenshot".into()))
    } else if modifiers.logo
        && modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::c || keysym == Keysym::C)
    {
        // Win11 Super+Shift+C color picker stub (W152).
        Some(KeyAction::Shell("colorpick".into()))
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && (keysym == Keysym::c || keysym == Keysym::C)
    {
        // W281: Super+Ctrl+C → color filters stub.
        Some(KeyAction::Shell("colorfilter".into()))
    } else if modifiers.logo
        && !modifiers.alt
        && !modifiers.ctrl
        && !modifiers.shift
        && keysym == Keysym::Tab
    {
        // Win11 Super+Tab Task View (W128).
        Some(KeyAction::Shell("taskview".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::p || keysym == Keysym::P)
    {
        // Win11 Super+P project / display modes stub (W132).
        // Super+Shift+P remains ScaleUp.
        Some(KeyAction::Shell("project".into()))
    } else if keysym.raw() == 0x1008FF13 {
        Some(KeyAction::Shell("volup".into()))
    } else if keysym.raw() == 0x1008FF11 {
        Some(KeyAction::Shell("voldown".into()))
    } else if keysym.raw() == 0x1008FF12 {
        Some(KeyAction::Shell("mute".into()))
    } else if keysym.raw() == 0x1008FF02 {
        Some(KeyAction::Shell("brightup".into()))
    } else if keysym.raw() == 0x1008FF03 {
        Some(KeyAction::Shell("brightdown".into()))
    } else if modifiers.logo
        && modifiers.ctrl
        && !modifiers.shift
        && !modifiers.alt
        && keysym == Keysym::Return
    {
        // Win11 Super+Ctrl+Enter → first available terminal (W101).
        Some(KeyAction::Run(first_available_terminal()))
    } else if modifiers.logo && !modifiers.ctrl && keysym == Keysym::Return {
        // Super+Return → weston-terminal
        Some(KeyAction::Run("weston-terminal".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (xkb::KEY_1..=xkb::KEY_9).contains(&keysym.raw())
    {
        // Win11 Super+1..9 → taskbar pin index (0-based) via shell.
        let idx = (keysym.raw() - xkb::KEY_1) as usize;
        Some(KeyAction::Shell(format!("pin {idx}")))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && keysym.raw() == xkb::KEY_0
    {
        // Win11 Super+0 → 10th taskbar pin.
        Some(KeyAction::Shell("pin 9".into()))
    } else if modifiers.logo && modifiers.ctrl && (xkb::KEY_1..=xkb::KEY_9).contains(&keysym.raw()) {
        // Keep output switch on Super+Ctrl+1..9 (was bare Super+N).
        Some(KeyAction::Screen((keysym.raw() - xkb::KEY_1) as usize))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::r || keysym == Keysym::R)
    {
        // Win11 Super+R Run dialog → shell (W141). Super+Shift+R remains RotateOutput.
        Some(KeyAction::Shell("run".into()))
    } else if modifiers.logo
        && !modifiers.shift
        && !modifiers.ctrl
        && !modifiers.alt
        && (keysym == Keysym::x || keysym == Keysym::X)
    {
        // Win11 Super+X quick link menu → shell (W161).
        Some(KeyAction::Shell("quicklink".into()))
    } else if modifiers.logo && modifiers.shift && keysym == Keysym::M {
        Some(KeyAction::ScaleDown)
    } else if modifiers.logo && modifiers.shift && keysym == Keysym::P {
        Some(KeyAction::ScaleUp)
    } else if modifiers.logo && modifiers.shift && keysym == Keysym::W {
        Some(KeyAction::TogglePreview)
    } else if modifiers.logo && modifiers.shift && keysym == Keysym::R {
        Some(KeyAction::RotateOutput)
    } else if modifiers.logo && modifiers.shift && keysym == Keysym::T {
        Some(KeyAction::ToggleTint)
    } else {
        // Super+Shift+D previously toggled CSD/SSD; decorations are SSD-only now.
        None
    }
}

/// Prefer weston-terminal, then kitty / foot / alacritty (W101).
fn first_available_terminal() -> String {
    const CANDIDATES: &[&str] = &["weston-terminal", "kitty", "foot", "alacritty"];
    let path = std::env::var_os("PATH").unwrap_or_default();
    for name in CANDIDATES {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return (*name).to_string();
            }
        }
    }
    "weston-terminal".into()
}

fn apply_key_remap(keysym: Keysym) -> Option<Keysym> {
    use std::sync::OnceLock;
    static REMAPS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let remaps = REMAPS.get_or_init(|| {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let path = std::path::PathBuf::from(runtime).join("mde-key-remaps.txt");
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|l| {
                let (a, b) = l.split_once('=')?;
                Some((a.trim().to_string(), b.trim().to_string()))
            })
            .collect()
    });
    let name = format!("{keysym:?}");
    // Keysym Debug is like "Keysym(0x...)" — also try xkb name via raw match on common ones.
    for (from, to) in remaps {
        if name.eq_ignore_ascii_case(from) || keysym_name_match(keysym, from) {
            return keysym_from_name(to);
        }
    }
    None
}

fn keysym_name_match(keysym: Keysym, name: &str) -> bool {
    let n = name.trim();
    match n {
        "Caps_Lock" | "CapsLock" => keysym == Keysym::Caps_Lock,
        "Escape" | "Esc" => keysym == Keysym::Escape,
        "Return" | "Enter" => keysym == Keysym::Return,
        "Tab" => keysym == Keysym::Tab,
        "Space" | "space" => keysym == Keysym::space,
        "Super_L" => keysym == Keysym::Super_L,
        "Super_R" => keysym == Keysym::Super_R,
        _ if n.len() == 1 => {
            let c = n.chars().next().unwrap().to_ascii_lowercase();
            // a-z
            let code = xkb::KEY_a + (c as u32).saturating_sub(b'a' as u32);
            keysym.raw() == code || keysym.raw() == (code - 0x20)
        }
        _ => false,
    }
}

fn keysym_from_name(name: &str) -> Option<Keysym> {
    let n = name.trim();
    Some(match n {
        "Caps_Lock" | "CapsLock" => Keysym::Caps_Lock,
        "Escape" | "Esc" => Keysym::Escape,
        "Return" | "Enter" => Keysym::Return,
        "Tab" => Keysym::Tab,
        "Space" | "space" => Keysym::space,
        "Super_L" => Keysym::Super_L,
        "Super_R" => Keysym::Super_R,
        _ if n.len() == 1 => {
            let c = n.chars().next()?.to_ascii_lowercase();
            if !c.is_ascii_lowercase() {
                return None;
            }
            Keysym::from(xkb::KEY_a + (c as u32 - b'a' as u32))
        }
        _ => return None,
    })
}
