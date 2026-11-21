use std::cell::RefCell;

#[cfg(feature = "xwayland")]
use smithay::xwayland::XWaylandClientData;

#[cfg(feature = "udev")]
use smithay::wayland::drm_syncobj::DrmSyncobjCachedState;

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{
        layer_map_for_output, space::SpaceElement, LayerSurface, PopupKind, PopupManager, Space,
        WindowSurfaceType,
    },
    input::pointer::{CursorImageStatus, CursorImageSurfaceData},
    output::{Output, Scale},
    reexports::{
        calloop::Interest,
        wayland_server::{
            protocol::{wl_buffer::WlBuffer, wl_output, wl_surface::WlSurface},
            Client, Resource,
        },
    },
    utils::{IsAlive, Logical, Point, Rectangle, Size},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states,
            with_surface_tree_upward, BufferAssignment, CompositorClientState, CompositorHandler,
            CompositorState, SurfaceAttributes, TraversalAction,
        },
        dmabuf::get_dmabuf,
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
                WlrLayerShellState,
            },
            xdg::XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    state::{AnvilState, Backend},
    ClientState,
};

mod element;
mod grabs;
pub(crate) mod ssd;
pub(crate) mod tabs;
pub(crate) mod workspaces;
#[cfg(feature = "xwayland")]
mod x11;
mod xdg;

pub use self::element::*;
pub use self::grabs::*;
pub use self::tabs::{ensure_tab_strip, OutputTabStrip, TabStrip, TAB_STRIP_HEIGHT};
pub use self::workspaces::{Workspaces, WindowWorkspace, MAX_WORKSPACES};

fn fullscreen_output_geometry(
    wl_surface: &WlSurface,
    wl_output: Option<&wl_output::WlOutput>,
    space: &mut Space<WindowElement>,
) -> Option<Rectangle<i32, Logical>> {
    // First test if a specific output has been requested
    // if the requested output is not found ignore the request
    wl_output
        .and_then(Output::from_resource)
        .or_else(|| {
            let w = space
                .elements()
                .find(|window| window.wl_surface().map(|s| &*s == wl_surface).unwrap_or(false));
            w.and_then(|w| space.outputs_for_element(w).first().cloned())
        })
        .as_ref()
        .and_then(|o| space.output_geometry(o))
}

#[derive(Default)]
pub struct FullscreenSurface(RefCell<Option<WindowElement>>);

impl FullscreenSurface {
    pub fn set(&self, window: WindowElement) {
        *self.0.borrow_mut() = Some(window);
    }

    pub fn get(&self) -> Option<WindowElement> {
        let mut window = self.0.borrow_mut();
        if window.as_ref().map(|w| !w.alive()).unwrap_or(false) {
            *window = None;
        }
        window.clone()
    }

    pub fn clear(&self) -> Option<WindowElement> {
        self.0.borrow_mut().take()
    }
}

/// W412: window stays above others when set.
#[derive(Debug, Default)]
pub struct AlwaysOnTop(RefCell<bool>);

impl AlwaysOnTop {
    pub fn get(&self) -> bool {
        *self.0.borrow()
    }

    pub fn set(&self, on: bool) {
        *self.0.borrow_mut() = on;
    }

    pub fn toggle(&self) -> bool {
        let mut slot = self.0.borrow_mut();
        *slot = !*slot;
        *slot
    }
}

/// W413: per-window render opacity (1.0 / 0.85 / 0.7 cycle).
#[derive(Debug)]
pub struct WindowOpacity(RefCell<f32>);

impl Default for WindowOpacity {
    fn default() -> Self {
        Self(RefCell::new(1.0))
    }
}

impl WindowOpacity {
    pub fn get(&self) -> f32 {
        *self.0.borrow()
    }

    pub fn set(&self, alpha: f32) {
        *self.0.borrow_mut() = alpha.clamp(0.05, 1.0);
    }

    /// Cycle 1.0 → 0.85 → 0.7 → 1.0.
    pub fn cycle(&self) -> f32 {
        let mut slot = self.0.borrow_mut();
        *slot = if (*slot - 1.0).abs() < 0.01 {
            0.85
        } else if (*slot - 0.85).abs() < 0.01 {
            0.7
        } else {
            1.0
        };
        *slot
    }
}

/// Geometry to restore after Super+Down from a snap/maximize (W142).
///
/// `save` stores the pre-snap/pre-maximize rect once; Super+Down from maximized
/// or snapped calls `restore` via `take_snap_restore` in `input_handler`.
#[derive(Debug, Default)]
pub struct SnapRestoreGeo(RefCell<Option<Rectangle<i32, Logical>>>);

impl SnapRestoreGeo {
    /// Remember the floating geometry before the first snap/maximize (W142).
    pub fn save(&self, geo: Rectangle<i32, Logical>) {
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(geo);
        }
    }

    pub fn restore(&self) -> Option<Rectangle<i32, Logical>> {
        self.0.borrow_mut().take()
    }

    pub fn has_saved(&self) -> bool {
        self.0.borrow().is_some()
    }
}

impl<BackendData: Backend> BufferHandler for AnvilState<BackendData> {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl<BackendData: Backend> CompositorHandler for AnvilState<BackendData> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        #[cfg(feature = "xwayland")]
        if let Some(state) = client.get_data::<XWaylandClientData>() {
            return &state.compositor_state;
        }
        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }
        panic!("Unknown client data type")
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, move |state, _dh, surface| {
            #[cfg(feature = "udev")]
            let mut acquire_point = None;
            let maybe_dmabuf = with_states(surface, |surface_data| {
                #[cfg(feature = "udev")]
                acquire_point.clone_from(
                    &surface_data
                        .cached_state
                        .get::<DrmSyncobjCachedState>()
                        .pending()
                        .acquire_point,
                );
                surface_data
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .pending()
                    .buffer
                    .as_ref()
                    .and_then(|assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    })
            });
            if let Some(dmabuf) = maybe_dmabuf {
                #[cfg(feature = "udev")]
                if let Some(acquire_point) = acquire_point {
                    if let Ok((blocker, source)) = acquire_point.generate_blocker() {
                        let client = surface.client().unwrap();
                        let res = state.handle.insert_source(source, move |_, _, data| {
                            let dh = data.display_handle.clone();
                            data.client_compositor_state(&client).blocker_cleared(data, &dh);
                            Ok(())
                        });
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                            return;
                        }
                    }
                }
                if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                    if let Some(client) = surface.client() {
                        let res = state.handle.insert_source(source, move |_, _, data| {
                            let dh = data.display_handle.clone();
                            data.client_compositor_state(&client).blocker_cleared(data, &dh);
                            Ok(())
                        });
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                        }
                    }
                }
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.backend_data.early_import(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_for_surface(&root) {
                window.0.on_commit();

                if &root == surface {
                    let buffer_offset = with_states(surface, |states| {
                        states
                            .cached_state
                            .get::<SurfaceAttributes>()
                            .current()
                            .buffer_delta
                            .take()
                    });

                    if let Some(buffer_offset) = buffer_offset {
                        let current_loc = self.space.element_location(&window).unwrap();
                        self.space.map_element(window, current_loc + buffer_offset, false);
                    }
                }
            }
        }
        self.popups.commit(surface);

        if matches!(&self.cursor_status, CursorImageStatus::Surface(cursor_surface) if cursor_surface == surface)
        {
            with_states(surface, |states| {
                let cursor_image_attributes = states.data_map.get::<CursorImageSurfaceData>();

                if let Some(mut cursor_image_attributes) =
                    cursor_image_attributes.map(|attrs| attrs.lock().unwrap())
                {
                    let buffer_delta = states
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .current()
                        .buffer_delta
                        .take();
                    if let Some(buffer_delta) = buffer_delta {
                        tracing::trace!(hotspot = ?cursor_image_attributes.hotspot, ?buffer_delta, "decrementing cursor hotspot");
                        cursor_image_attributes.hotspot -= buffer_delta;
                    }
                }
            });
        }

        if matches!(&self.dnd_icon, Some(icon) if &icon.surface == surface) {
            let dnd_icon = self.dnd_icon.as_mut().unwrap();
            with_states(&dnd_icon.surface, |states| {
                let buffer_delta = states
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .current()
                    .buffer_delta
                    .take()
                    .unwrap_or_default();
                tracing::trace!(offset = ?dnd_icon.offset, ?buffer_delta, "moving dnd offset");
                dnd_icon.offset += buffer_delta;
            });
        }

        ensure_initial_configure(surface, &self.space, &mut self.popups)
    }
}

impl<BackendData: Backend> WlrLayerShellHandler for AnvilState<BackendData> {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<wl_output::WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .unwrap_or_else(|| self.space.outputs().next().unwrap().clone());
        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace)).unwrap();
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some((mut map, layer)) = self.space.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map
                .layers()
                .find(|&layer| layer.layer_surface() == &surface)
                .cloned();
            layer.map(|layer| (map, layer))
        }) {
            map.unmap_layer(&layer);
        }
    }
}

impl<BackendData: Backend> AnvilState<BackendData> {
    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<WindowElement> {
        self.space
            .elements()
            .find(|window| window.wl_surface().map(|s| &*s == surface).unwrap_or(false))
            .cloned()
    }
}

#[derive(Default)]
pub struct SurfaceData {
    pub geometry: Option<Rectangle<i32, Logical>>,
    pub resize_state: ResizeState,
}

fn ensure_initial_configure(surface: &WlSurface, space: &Space<WindowElement>, popups: &mut PopupManager) {
    with_surface_tree_upward(
        surface,
        (),
        |_, _, _| TraversalAction::DoChildren(()),
        |_, states, _| {
            states
                .data_map
                .insert_if_missing(|| RefCell::new(SurfaceData::default()));
        },
        |_, _, _| true,
    );

    if let Some(window) = space
        .elements()
        .find(|window| window.wl_surface().map(|s| &*s == surface).unwrap_or(false))
        .cloned()
    {
        // send the initial configure if relevant
        #[cfg_attr(not(feature = "xwayland"), allow(irrefutable_let_patterns))]
        if let Some(toplevel) = window.0.toplevel() {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });
            if !initial_configure_sent {
                toplevel.send_configure();
            }
        }

        with_states(surface, |states| {
            let mut data = states
                .data_map
                .get::<RefCell<SurfaceData>>()
                .unwrap()
                .borrow_mut();

            // Finish resizing.
            if let ResizeState::WaitingForCommit(_) = data.resize_state {
                data.resize_state = ResizeState::NotResizing;
            }
        });

        return;
    }

    if let Some(popup) = popups.find_popup(surface) {
        let popup = match popup {
            PopupKind::Xdg(ref popup) => popup,
            // Doesn't require configure
            PopupKind::InputMethod(ref _input_popup) => {
                return;
            }
        };

        if !popup.is_initial_configure_sent() {
            // NOTE: This should never fail as the initial configure is always
            // allowed.
            popup.send_configure().expect("initial configure failed");
        }

        return;
    };

    if let Some(output) = space.outputs().find(|o| {
        let map = layer_map_for_output(o);
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .is_some()
    }) {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        let mut map = layer_map_for_output(output);

        // arrange the layers before sending the initial configure
        // to respect any size the client may have sent
        map.arrange();
        // send the initial configure if relevant
        if !initial_configure_sent {
            let layer = map
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .unwrap();

            layer.layer_surface().send_configure();
        }
    };
}

fn place_new_window(
    space: &mut Space<WindowElement>,
    pointer_location: Point<f64, Logical>,
    window: &WindowElement,
    activate: bool,
) {
    // place the window at a random location on same output as pointer
    // or if there is not output in a [0;800]x[0;800] square
    use rand::distributions::{Distribution, Uniform};

    let output = space
        .output_under(pointer_location)
        .next()
        .or_else(|| space.outputs().next())
        .cloned();
    let output_geometry = output
        .and_then(|o| {
            let geo = space.output_geometry(&o)?;
            let map = layer_map_for_output(&o);
            let zone = map.non_exclusive_zone();
            Some(Rectangle::new(geo.loc + zone.loc, zone.size))
        })
        .unwrap_or_else(|| Rectangle::from_size((800, 800).into()));

    // set the initial toplevel bounds
    #[allow(irrefutable_let_patterns)]
    if let Some(toplevel) = window.0.toplevel() {
        toplevel.with_pending_state(|state| {
            state.bounds = Some(output_geometry.size);
        });
    }

    let max_x = output_geometry.loc.x + (((output_geometry.size.w as f32) / 3.0) * 2.0) as i32;
    let max_y = output_geometry.loc.y + (((output_geometry.size.h as f32) / 3.0) * 2.0) as i32;
    let x_range = Uniform::new(output_geometry.loc.x, max_x);
    let y_range = Uniform::new(output_geometry.loc.y, max_y);
    let mut rng = rand::thread_rng();
    let x = x_range.sample(&mut rng);
    let y = y_range.sample(&mut rng);

    space.map_element(window.clone(), (x, y), activate);
}

fn window_publish_fields(window: &WindowElement) -> Option<(String, String, bool)> {
    let surface = window.wl_surface()?;
    Some(with_states(surface.as_ref(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .map(|data| {
                let guard = data.lock().unwrap();
                let fullscreen = guard.current.states.contains(
                    smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Fullscreen,
                );
                (
                    guard.title.clone().unwrap_or_else(|| "window".into()),
                    guard.app_id.clone().unwrap_or_default(),
                    fullscreen,
                )
            })
            .unwrap_or_else(|| ("window".into(), String::new(), false))
    }))
}

/// Snapshot toplevels (active + other desktops) for the shell (`mde-windows.json` + protocol).
pub fn publish_window_list(
    space: &Space<WindowElement>,
    workspaces: &workspaces::Workspaces,
    bus: Option<&mde_protocol::CompBus>,
) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    let now = Instant::now();
    if last.is_some_and(|t| now.duration_since(t) < Duration::from_millis(800)) {
        return;
    }
    *last = Some(now);
    drop(last);

    let mut windows = Vec::new();
    for window in workspaces.all_windows(space) {
        let Some((title, app_id, fullscreen)) = window_publish_fields(&window) else {
            continue;
        };
        let ws = workspaces::Workspaces::window_workspace(&window);
        let id = crate::ipc::ensure_window_id(&window);
        windows.push(mde_protocol::WindowInfo {
            id,
            title,
            app_id,
            fullscreen,
            workspace: ws,
        });
    }
    let snapshot = mde_protocol::WindowsSnapshot {
        active: workspaces.active,
        count: workspaces.count,
        windows,
    };

    if let Ok(body) = serde_json::to_string(&snapshot) {
        let path = mde_protocol::windows_json_path();
        let _ = std::fs::write(path, body);
    }

    // Compact workspace summary for shell / tools.
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let ws_path = std::path::Path::new(&runtime).join("mde-workspaces.json");
    let ws_body = format!(
        "{{\"active\":{},\"count\":{}}}",
        workspaces.active, workspaces.count
    );
    let _ = std::fs::write(ws_path, ws_body);

    if let Some(bus) = bus {
        bus.push(mde_protocol::CompToShell::WindowsSnapshot(snapshot.clone()));
        bus.push(mde_protocol::CompToShell::WorkspaceState {
            active: workspaces.active,
            count: workspaces.count,
        });
    }
}

struct SavedHead {
    name: String,
    x: i32,
    y: i32,
    scale: f64,
    enabled: bool,
}

fn layout_file() -> Option<std::path::PathBuf> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    Some(std::path::Path::new(&runtime).join("mde-output-layout.txt"))
}

fn read_output_layout() -> Vec<SavedHead> {
    let Some(path) = layout_file() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_layout(&text)
}

fn parse_layout(text: &str) -> Vec<SavedHead> {
    let mut heads = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let Ok(x) = parts.next().unwrap_or("0").parse::<i32>() else { continue };
        let Ok(y) = parts.next().unwrap_or("0").parse::<i32>() else { continue };
        let Ok(scale) = parts.next().unwrap_or("1").parse::<f64>() else { continue };
        let enabled = parts.next().unwrap_or("1") != "0";
        heads.push(SavedHead {
            name: name.to_string(),
            x,
            y,
            scale,
            enabled,
        });
    }
    heads
}

/// Place outputs from `mde-output-layout.txt` when that file lists them.
/// Other outputs stay in a row to the right.
pub fn place_outputs(space: &mut Space<WindowElement>) {
    let saved = read_output_layout();
    let outputs: Vec<Output> = space.outputs().cloned().collect();
    let mut fallback_x = 0i32;
    for output in outputs {
        let name = output.name();
        if let Some(head) = saved.iter().find(|h| h.name == name) {
            if !head.enabled && space.outputs().count() > 1 {
                space.unmap_output(&output);
                continue;
            }
            let scale = if head.scale.is_finite() && (0.5..=4.0).contains(&head.scale) {
                head.scale
            } else {
                1.0
            };
            let pos = Point::<i32, Logical>::from((head.x, head.y));
            output.change_current_state(None, None, Some(Scale::Fractional(scale)), Some(pos));
            space.map_output(&output, pos);
            layer_map_for_output(&output).arrange();
        } else {
            let size = space
                .output_geometry(&output)
                .map(|geo| geo.size.w)
                .unwrap_or(0);
            let pos = Point::<i32, Logical>::from((fallback_x, 0));
            space.map_output(&output, pos);
            layer_map_for_output(&output).arrange();
            fallback_x += size;
        }
    }
}

/// Publish live outputs for Connect / Settings (JSON + CompBus).
pub fn publish_outputs_snapshot(
    space: &Space<WindowElement>,
    bus: Option<&mde_protocol::CompBus>,
) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    let now = Instant::now();
    if last.is_some_and(|t| now.duration_since(t) < Duration::from_millis(800)) {
        return;
    }
    *last = Some(now);
    drop(last);

    let mut outputs = Vec::new();
    for output in space.outputs() {
        let geo = space.output_geometry(output);
        let (x, y, w, h) = geo
            .map(|g| (g.loc.x, g.loc.y, g.size.w, g.size.h))
            .unwrap_or((0, 0, 0, 0));
        let mode = output.current_mode();
        let (mw, mh) = mode.map(|m| (m.size.w, m.size.h)).unwrap_or((w, h));
        outputs.push(mde_protocol::OutputInfo {
            name: output.name(),
            make: output
                .physical_properties()
                .make
                .clone(),
            model: output.physical_properties().model.clone(),
            width: mw,
            height: mh,
            scale: output.current_scale().fractional_scale(),
            x,
            y,
        });
    }
    let snapshot = mde_protocol::OutputsSnapshot { outputs };
    if let Ok(body) = serde_json::to_string(&snapshot) {
        let _ = std::fs::write(mde_protocol::outputs_json_path(), body);
    }
    if let Some(bus) = bus {
        bus.push(mde_protocol::CompToShell::OutputsSnapshot(snapshot));
    }
}

/// Re-read the layout file when it changes, and publish live geometry for Settings.
pub fn sync_output_layout(space: &mut Space<WindowElement>) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant, SystemTime};
    static LAYOUT_MTIME: Mutex<Option<Option<SystemTime>>> = Mutex::new(None);
    static LAST_PUBLISH: Mutex<Option<Instant>> = Mutex::new(None);

    if let Some(path) = layout_file() {
        let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        let mut guard = LAYOUT_MTIME.lock().ok();
        if let Some(guard) = guard.as_mut() {
            if **guard != Some(mtime) {
                **guard = Some(mtime);
                if mtime.is_some() {
                    place_outputs(space);
                }
            }
        }
    }

    let Ok(mut last) = LAST_PUBLISH.lock() else {
        return;
    };
    let now = Instant::now();
    if last.is_some_and(|t| now.duration_since(t) < Duration::from_millis(800)) {
        return;
    }
    *last = Some(now);
    drop(last);

    let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") else {
        return;
    };
    let mut lines = String::new();
    for output in space.outputs() {
        let geo = space.output_geometry(output);
        let (x, y, w, h) = geo
            .map(|g| (g.loc.x, g.loc.y, g.size.w, g.size.h))
            .unwrap_or((0, 0, 0, 0));
        let scale = output.current_scale().fractional_scale();
        lines.push_str(&format!(
            "{} {x} {y} {w} {h} {scale}\n",
            output.name()
        ));
    }
    let path = std::path::Path::new(&runtime).join("mde-outputs.txt");
    let _ = std::fs::write(path, lines);
}

pub fn fixup_positions(space: &mut Space<WindowElement>, pointer_location: Point<f64, Logical>) {
    place_outputs(space);

    // fixup windows
    let mut orphaned_windows = Vec::new();
    let outputs = space
        .outputs()
        .flat_map(|o| {
            let geo = space.output_geometry(o)?;
            let map = layer_map_for_output(o);
            let zone = map.non_exclusive_zone();
            Some(Rectangle::new(geo.loc + zone.loc, zone.size))
        })
        .collect::<Vec<_>>();
    for window in space.elements() {
        let window_location = match space.element_location(window) {
            Some(loc) => loc,
            None => continue,
        };
        let geo_loc = window.bbox().loc + window_location;

        if !outputs.iter().any(|o_geo| o_geo.contains(geo_loc)) {
            orphaned_windows.push(window.clone());
        }
    }
    for window in orphaned_windows.into_iter() {
        place_new_window(space, pointer_location, &window, false);
    }
}
