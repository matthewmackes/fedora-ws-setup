//! `ext-session-lock-v1` compositor support.

use std::collections::HashMap;

use smithay::{
    output::Output,
    reexports::wayland_server::{protocol::wl_output::WlOutput, Resource},
    utils::{Logical, Size},
    wayland::session_lock::{
        LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
    },
};
use tracing::info;

use crate::state::{AnvilState, Backend};

/// Active session lock: one lock surface per output.
#[derive(Debug, Default)]
pub struct ActiveSessionLock {
    pub surfaces: HashMap<Output, LockSurface>,
}

impl ActiveSessionLock {
    pub fn surface_for(&self, output: &Output) -> Option<&LockSurface> {
        self.surfaces.get(output)
    }
}

/// Logical size suggested to a lock client for an output.
pub fn lock_surface_size_for_output(output: &Output, fallback: Size<i32, Logical>) -> Size<u32, Logical> {
    let size = output
        .current_mode()
        .map(|mode| {
            let scale = output.current_scale().fractional_scale();
            let transformed = output.current_transform().transform_size(mode.size);
            Size::from((
                (transformed.w as f64 / scale).round() as i32,
                (transformed.h as f64 / scale).round() as i32,
            ))
        })
        .unwrap_or(fallback);
    Size::from((size.w.max(1) as u32, size.h.max(1) as u32))
}

impl<BackendData: Backend> SessionLockHandler for AnvilState<BackendData> {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_manager_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        if self.session_lock.is_some() {
            info!("ext-session-lock: reject nested lock (already locked)");
            // Dropping confirmation notifies the client (finished).
            return;
        }
        // Confirm immediately (blanked frames follow via render path).
        confirmation.lock();
        self.session_lock = Some(ActiveSessionLock::default());
        info!("ext-session-lock: session locked");

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, None, serial);
        }
    }

    fn unlock(&mut self) {
        self.session_lock = None;
        info!("ext-session-lock: session unlocked");
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, None, serial);
        }
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        let Some(output) = Output::from_resource(&wl_output) else {
            info!("ext-session-lock: new_surface for unknown output");
            return;
        };
        let Some(lock) = self.session_lock.as_mut() else {
            info!("ext-session-lock: new_surface while unlocked — ignoring");
            return;
        };

        let fallback = self
            .space
            .output_geometry(&output)
            .map(|g| g.size)
            .unwrap_or_else(|| Size::from((1920, 1080)));
        let size = lock_surface_size_for_output(&output, fallback);
        surface.with_pending_state(|state| {
            state.size = Some(size);
        });
        surface.send_configure();

        lock.surfaces.insert(output.clone(), surface.clone());
        info!(
            output = output.name(),
            w = size.w,
            h = size.h,
            "ext-session-lock: lock surface configured"
        );

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(
                self,
                Some(crate::focus::KeyboardFocusTarget::LockSurface(surface)),
                serial,
            );
        }
    }
}

smithay::delegate_session_lock!(@<BackendData: Backend + 'static> AnvilState<BackendData>);

#[cfg(test)]
mod tests {
    use super::lock_surface_size_for_output;
    use smithay::{
        output::{Mode, Output, PhysicalProperties, Subpixel},
        utils::{Logical, Size},
    };

    #[test]
    fn lock_surface_size_uses_fallback_without_mode() {
        let output = Output::new(
            "test".into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "mde".into(),
                model: "test".into(),
            },
        );
        let size = lock_surface_size_for_output(&output, Size::<i32, Logical>::from((1280, 800)));
        assert_eq!(size, Size::from((1280, 800)));
    }

    #[test]
    fn lock_surface_size_from_mode() {
        let output = Output::new(
            "test".into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "mde".into(),
                model: "test".into(),
            },
        );
        let mode = Mode {
            size: (1920, 1080).into(),
            refresh: 60_000,
        };
        output.change_current_state(Some(mode), None, None, None);
        let size = lock_surface_size_for_output(&output, Size::<i32, Logical>::from((1, 1)));
        assert_eq!(size, Size::from((1920, 1080)));
    }
}
