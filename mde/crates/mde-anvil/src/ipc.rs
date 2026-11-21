//! Compositor side of `mde-protocol` (Unix socket hub + window ids).

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use mde_protocol::CompBus;

use crate::shell::WindowElement;

/// Process-wide window id counter for protocol focus.
static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct ProtocolWindowId(pub u64);

/// Assign a stable id the first time a window is published.
pub fn ensure_window_id(window: &WindowElement) -> u64 {
    window.user_data().insert_if_missing(|| {
        ProtocolWindowId(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    });
    window
        .user_data()
        .get::<ProtocolWindowId>()
        .map(|p| p.0)
        .unwrap_or(0)
}

pub fn start_bus() -> Arc<CompBus> {
    CompBus::start()
}
