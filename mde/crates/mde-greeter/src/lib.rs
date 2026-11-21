//! Greetd client helpers shared by `mde-greeter` (iced UI) and `mde-comp greeter`
//! (headless / smoke).

pub mod greetd;

pub use greetd::{authenticate, default_username, list_human_users, session_command};
