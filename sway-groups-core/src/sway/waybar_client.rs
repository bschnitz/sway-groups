//! Waybar-dynamic IPC client.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::error::{Error, Result};
use serde::Serialize;
use tracing::{info, warn};

/// waybar-dynamic instance name used by swayg.
pub const WAYBAR_INSTANCE_NAME: &str = "swayg_workspaces";
pub const WAYBAR_GROUPS_INSTANCE_NAME: &str = "swayg_groups";

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
    /// Shell command to run on right click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_right_click: Option<String>,
    /// Shell command to run on middle click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_middle_click: Option<String>,
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

    /// Create a client for the groups instance.
    pub fn new_groups() -> Self {
        let socket_path = Self::resolve_socket_path_with_name(WAYBAR_GROUPS_INSTANCE_NAME);
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
        self.send_inner(message, 0, std::time::Duration::ZERO)
    }

    /// Send a message with retry. Retries up to `retries` times with `delay` between attempts.
    /// Only retries if the socket file does not exist.
    pub fn send_with_retry(&self, message: &WaybarMessage, retries: u32, delay: std::time::Duration) -> Result<()> {
        self.send_inner(message, retries, delay)
    }

    fn send_inner(&self, message: &WaybarMessage, retries: u32, delay: std::time::Duration) -> Result<()> {
        let socket_path = match &self.socket_path {
            Some(p) => p,
            None => {
                warn!("waybar-dynamic: XDG_RUNTIME_DIR not set, skipping send");
                return Ok(());
            }
        };

        let mut attempts = retries + 1;
        loop {
            if !socket_path.exists() {
                if attempts <= 1 {
                    warn!(
                        "waybar-dynamic: socket not found at {:?}, skipping send",
                        socket_path
                    );
                    return Ok(());
                }
                attempts -= 1;
                info!(
                    "waybar-dynamic: socket not found at {:?}, retrying in {}ms ({} attempts left)",
                    socket_path,
                    delay.as_millis(),
                    attempts
                );
                std::thread::sleep(delay);
                continue;
            }

            match UnixStream::connect(socket_path) {
                Ok(mut stream) => {
                    let payload = serde_json::to_string(message)?;
                    writeln!(stream, "{}", payload)?;
                    stream.flush()?;
                    info!("waybar-dynamic: sent message to {:?} successfully", socket_path);
                    return Ok(());
                }
                Err(e) if e.raw_os_error() == Some(111) && attempts > 1 => {
                    attempts -= 1;
                    info!(
                        "waybar-dynamic: connection refused at {:?}, retrying in {}ms ({} attempts left)",
                        socket_path,
                        delay.as_millis(),
                        attempts
                    );
                    std::thread::sleep(delay);
                }
                Err(e) => {
                    warn!("waybar-dynamic: error at {:?}: {}", socket_path, e);
                    return Err(Error::Io(e));
                }
            }
        }
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
