//! Waybar-dynamic IPC client.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::error::{Error, Result};
use serde::Serialize;
use tracing::warn;

/// waybar-dynamic instance name used by swayg.
pub const WAYBAR_INSTANCE_NAME: &str = "swayg_workspaces";

/// Operation type for waybar-dynamic IPC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaybarOp {
    SetAll,
    Patch,
    Clear,
}

/// A widget specification sent to waybar-dynamic.
#[derive(Debug, Clone, Serialize)]
pub struct WidgetSpec {
    /// CSS widget name (`#id`).
    pub id: String,
    /// Text to display.
    pub label: String,
    /// CSS classes to apply to the label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Tooltip text shown on hover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Shell command to run on left click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<String>,
}

/// An IPC message sent to waybar-dynamic.
#[derive(Debug, Clone, Serialize)]
pub struct WaybarMessage {
    pub op: WaybarOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widgets: Option<Vec<WidgetSpec>>,
}

impl WaybarMessage {
    pub fn set_all(widgets: Vec<WidgetSpec>) -> Self {
        Self {
            op: WaybarOp::SetAll,
            widgets: Some(widgets),
        }
    }

    pub fn clear() -> Self {
        Self {
            op: WaybarOp::Clear,
            widgets: None,
        }
    }
}

/// Client for communicating with waybar-dynamic via Unix socket.
#[derive(Clone)]
pub struct WaybarClient {
    socket_path: Option<PathBuf>,
}

impl WaybarClient {
    /// Create a new waybar-dynamic client.
    /// If XDG_RUNTIME_DIR is not set or the socket doesn't exist yet,
    /// calls will silently log a warning and return Ok.
    pub fn new() -> Self {
        let socket_path = Self::resolve_socket_path();
        Self { socket_path }
    }

    /// Create a new client with a specific instance name (for testing).
    pub fn with_instance_name(name: &str) -> Self {
        let socket_path = Self::resolve_socket_path_with_name(name);
        Self { socket_path }
    }

    fn resolve_socket_path() -> Option<PathBuf> {
        Self::resolve_socket_path_with_name(WAYBAR_INSTANCE_NAME)
    }

    fn resolve_socket_path_with_name(name: &str) -> Option<PathBuf> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
        let path = PathBuf::from(runtime_dir)
            .join(format!("waybar-dynamic-{}.sock", name));
        Some(path)
    }

    /// Send a message to waybar-dynamic.
    /// If the socket is not available, logs a warning and returns Ok.
    pub fn send(&self, message: &WaybarMessage) -> Result<()> {
        let socket_path = match &self.socket_path {
            Some(p) => p,
            None => {
                warn!("waybar-dynamic: XDG_RUNTIME_DIR not set, skipping send");
                return Ok(());
            }
        };

        if !socket_path.exists() {
            warn!(
                "waybar-dynamic: socket not found at {:?}, skipping send",
                socket_path
            );
            return Ok(());
        }

        let mut stream = UnixStream::connect(socket_path)
            .map_err(|e| Error::Io(e))?;

        let payload = serde_json::to_string(message)?;
        writeln!(stream, "{}", payload)?;
        stream.flush()?;

        Ok(())
    }

    /// Send a set_all message with the given widgets.
    pub fn send_set_all(&self, widgets: Vec<WidgetSpec>) -> Result<()> {
        self.send(&WaybarMessage::set_all(widgets))
    }

    /// Send a clear message.
    pub fn send_clear(&self) -> Result<()> {
        self.send(&WaybarMessage::clear())
    }
}

impl Default for WaybarClient {
    fn default() -> Self {
        Self::new()
    }
}
