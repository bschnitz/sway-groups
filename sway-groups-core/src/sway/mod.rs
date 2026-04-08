//! Sway IPC module.

pub mod client;
pub mod types;
pub mod waybar_client;

pub use client::{EventStream, SwayIpcClient};
pub use types::SwayEventType;
pub use waybar_client::{WaybarClient, WaybarMessage, WidgetSpec};
