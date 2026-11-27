//! Network Neighbourhood — shared types, mounts, discovery, and D-Bus API.

pub mod config;
pub mod connect;
pub mod dbus;
pub mod diagnose;
pub mod discovery;
pub mod keys;
pub mod media;
pub mod mount;
pub mod place;
pub mod status;
pub mod ui;

pub use config::{Config, CONFIG_DIR, CREDS_DIR, MOUNT_ROOT};
pub use place::{Place, Protocol};

pub const APP_ID: &str = "org.mde.NetworkNeighbourhood";
pub const APP_NAME: &str = "Network Neighbourhood";
pub const DBUS_NAME: &str = "org.mde.NetworkNeighbourhood1";
pub const DBUS_PATH: &str = "/org/mde/NetworkNeighbourhood1";
