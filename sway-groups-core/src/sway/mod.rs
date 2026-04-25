//! Sway IPC module.

pub mod client;
pub mod pid_resolver;
pub mod types;
pub mod waybar_client;

pub use client::{EventStream, SwayIpcClient};
pub use types::{SwayEventType, SwayWorkspace};
pub use waybar_client::{WaybarClient, WaybarMessage, WidgetSpec};
