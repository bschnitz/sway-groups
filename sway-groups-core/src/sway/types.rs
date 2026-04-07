//! Sway IPC types and data structures.

use serde::{Deserialize, Serialize};

/// Sway IPC message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SwayMsgType {
    RunCommand = 0,
    GetWorkspaces = 1,
    Subscribe = 2,
    GetOutputs = 3,
    GetTree = 4,
    GetMarks = 5,
    GetBarIds = 6,
    GetBarConfig = 7,
    GetVersions = 8,
    GetConfig = 9,
    SendTick = 10,
    GetBindings = 11,
    GetInput = 12,
    GetSeats = 100,
}

/// Sway IPC event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SwayEventType {
    Workspace = 0,
    Output = 1,
    Mode = 2,
    Window = 3,
    BarConfigUpdate = 4,
    BindingInfo = 5,
    Shutdown = 6,
    Tick = 7,
}

/// Workspace representation from sway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayWorkspace {
    pub id: i64,
    pub num: Option<i64>,
    pub name: String,
    pub visible: bool,
    pub focused: bool,
    pub urgent: bool,
    pub output: String,
    pub representation: String,
    pub layout: String,
    #[serde(rename = "type")]
    pub node_type: String,
}

/// Output representation from sway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayOutput {
    pub id: i64,
    pub name: String,
    pub active: bool,
    pub visible: bool,
    pub focused: bool,
    pub primary: bool,
    pub layout: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub rect: SwayRect,
    pub output_mode: Option<SwayOutputMode>,
}

/// Rectangle structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// Output mode information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayOutputMode {
    pub width: i64,
    pub height: i64,
    pub refresh: i64,
}

/// Command result from sway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
}

/// IPC message header.
#[derive(Debug, Clone)]
pub struct IpcHeader {
    pub magic: [u8; 6],
    pub message_type: u32,
    pub payload_size: u32,
}

impl IpcHeader {
    /// Parse header from bytes.
    pub fn from_bytes(bytes: &[u8; 12]) -> Self {
        let mut magic = [0u8; 6];
        magic.copy_from_slice(&bytes[0..6]);

        let message_type = u32::from_ne_bytes(bytes[6..10].try_into().unwrap());
        let payload_size = u32::from_ne_bytes(bytes[10..12].try_into().unwrap());

        Self {
            magic,
            message_type,
            payload_size,
        }
    }

    /// Create header for a message.
    pub fn new(message_type: SwayMsgType, payload_size: u32) -> Self {
        Self {
            magic: *b"i3-ipc",
            message_type: message_type as u32,
            payload_size,
        }
    }

    /// Serialize header to bytes.
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut bytes = [0u8; 12];
        bytes[0..6].copy_from_slice(&self.magic);
        bytes[6..10].copy_from_slice(&u32::to_ne_bytes(self.message_type));
        bytes[10..12].copy_from_slice(&u32::to_ne_bytes(self.payload_size));
        bytes
    }
}
