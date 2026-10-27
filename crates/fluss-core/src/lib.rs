//! Fluss core: everything that is pure logic and shared by the daemon, the CLI
//! and the settings app. No platform APIs live here, so it is fully unit-tested.

pub mod chunk;
pub mod config;
pub mod continuation;
pub mod hotkey;
pub mod ipc;
pub mod models;
pub mod paths;
pub mod protocol;
pub mod rules;
pub mod text;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "Fluss";
