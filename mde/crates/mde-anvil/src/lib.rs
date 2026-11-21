//! MDE DRM compositor core (adapted from smithay anvil, MIT).
#![allow(rust_2018_idioms)]
#![allow(dead_code, unused_imports)]

pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod input_handler;
pub mod ipc;
pub mod render;
pub mod screencopy;
pub mod session_lock;
pub mod shell;
pub mod state;
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub use state::{AnvilState, ClientState};
pub use udev::run_udev;
#[cfg(feature = "winit")]
pub use winit::run_winit;
