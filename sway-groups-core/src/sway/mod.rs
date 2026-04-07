//! Sway IPC module.

pub mod client;
pub mod types;

pub use client::{EventStream, SwayIpcClient};
pub use types::SwayEventType;
