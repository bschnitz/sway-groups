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
/// These include the IPC_EVENT_MASK (0x80000000) that sway sets on event messages
/// to distinguish them from regular IPC responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SwayEventType {
    Workspace = 0x80000000,
    Output = 0x80000001,
    Mode = 0x80000002,
    Window = 0x80000003,
    BarConfigUpdate = 0x80000004,
    BindingInfo = 0x80000005,
    Shutdown = 0x80000006,
    Tick = 0x80000007,
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
    #[serde(default)]
    pub representation: Option<String>,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default, rename = "type")]
    pub node_type: Option<String>,
}

/// Output representation from sway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayOutput {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub primary: bool,
    pub rect: SwayRect,
}

/// Rectangle structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwayRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// Command result from sway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
}

/// IPC message header (14 bytes: 6 magic + 4 payload_size + 4 message_type).
#[derive(Debug, Clone)]
pub struct IpcHeader {
    pub magic: [u8; 6],
    pub message_type: u32,
    pub payload_size: u32,
}

impl IpcHeader {
    /// Parse header from bytes.
    pub fn from_bytes(bytes: &[u8; 14]) -> Self {
        let mut magic = [0u8; 6];
        magic.copy_from_slice(&bytes[0..6]);

        let payload_size = u32::from_ne_bytes(bytes[6..10].try_into().unwrap());
        let message_type = u32::from_ne_bytes(bytes[10..14].try_into().unwrap());

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
    pub fn to_bytes(&self) -> [u8; 14] {
        let mut bytes = [0u8; 14];
        bytes[0..6].copy_from_slice(&self.magic);
        bytes[6..10].copy_from_slice(&u32::to_ne_bytes(self.payload_size));
        bytes[10..14].copy_from_slice(&u32::to_ne_bytes(self.message_type));
        bytes
    }
}
