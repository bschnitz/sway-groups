//! Sway IPC client implementation.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use super::types::*;
use crate::error::{Error, Result};

/// Sway IPC client for communicating with sway.
#[derive(Clone)]
pub struct SwayIpcClient {
    socket_path: String,
}

impl SwayIpcClient {
    /// Create a new sway IPC client.
    /// Uses the SWAYSOCK environment variable to find the socket.
    pub fn new() -> Result<Self> {
        let socket_path = std::env::var("SWAYSOCK")
            .map_err(|_| Error::SwayNotRunning)?;

        Ok(Self { socket_path })
    }

    /// Create with a specific socket path.
    pub fn with_path<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Connect to sway and return a stream.
    fn connect(&self) -> Result<TcpStream> {
        TcpStream::connect(&self.socket_path)
            .map_err(|_| Error::SwayNotRunning)
    }

    /// Send a command to sway and get the result.
    pub fn run_command(&self, command: &str) -> Result<Vec<CommandResult>> {
        let mut stream = self.connect()?;

        let payload = command.as_bytes();
        let header = IpcHeader::new(SwayMsgType::RunCommand, payload.len() as u32);

        stream.write_all(&header.to_bytes())?;
        stream.write_all(payload)?;
        stream.flush()?;

        // Read response
        let response = Self::read_message(&mut stream)?;

        let results: Vec<CommandResult> = serde_json::from_slice(&response)?;
        Ok(results)
    }

    /// Get all workspaces.
    pub fn get_workspaces(&self) -> Result<Vec<SwayWorkspace>> {
        let mut stream = self.connect()?;

        let header = IpcHeader::new(SwayMsgType::GetWorkspaces, 0);

        stream.write_all(&header.to_bytes())?;
        stream.flush()?;

        let response = Self::read_message(&mut stream)?;

        let workspaces: Vec<SwayWorkspace> = serde_json::from_slice(&response)?;
        Ok(workspaces)
    }

    /// Get all outputs.
    pub fn get_outputs(&self) -> Result<Vec<SwayOutput>> {
        let mut stream = self.connect()?;

        let header = IpcHeader::new(SwayMsgType::GetOutputs, 0);

        stream.write_all(&header.to_bytes())?;
        stream.flush()?;

        let response = Self::read_message(&mut stream)?;

        let outputs: Vec<SwayOutput> = serde_json::from_slice(&response)?;
        Ok(outputs)
    }

    /// Get the focused workspace.
    pub fn get_focused_workspace(&self) -> Result<SwayWorkspace> {
        let workspaces = self.get_workspaces()?;
        workspaces
            .into_iter()
            .find(|w| w.focused)
            .ok_or_else(|| Error::SwayIpc("No focused workspace".to_string()))
    }

    /// Rename a workspace.
    pub fn rename_workspace(&self, old_name: &str, new_name: &str) -> Result<()> {
        let command = format!("rename workspace \"{}\" to \"{}\"", old_name, new_name);
        let results = self.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                Ok(())
            } else {
                Err(Error::SwayIpc(
                    result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
        } else {
            Err(Error::SwayIpc("Empty response".to_string()))
        }
    }

    /// Get current workspace names.
    pub fn get_workspace_names(&self) -> Result<Vec<String>> {
        let workspaces = self.get_workspaces()?;
        Ok(workspaces.into_iter().map(|w| w.name).collect())
    }

    /// Read a message from the stream.
    fn read_message(stream: &mut TcpStream) -> Result<Vec<u8>> {
        let mut header = [0u8; 12];
        stream.read_exact(&mut header)?;

        let ipc_header = IpcHeader::from_bytes(&header);

        // i3-ipc magic is 6 bytes: "i3-ipc"
        if &ipc_header.magic != b"i3-ipc" {
            return Err(Error::SwayIpc("Invalid IPC magic".to_string()));
        }

        let mut payload = vec![0u8; ipc_header.payload_size as usize];
        stream.read_exact(&mut payload)?;

        Ok(payload)
    }
}

impl Default for SwayIpcClient {
    fn default() -> Self {
        Self::new().expect("SWAYSOCK not set")
    }
}
