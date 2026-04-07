//! Error types for sway-groups.

use thiserror::Error;

/// Result type alias using our error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error enum for sway-groups.
#[derive(Error, Debug)]
pub enum Error {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    /// Sway IPC error
    #[error("Sway IPC error: {0}")]
    SwayIpc(String),

    /// Workspace not found
    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),

    /// Group not found
    #[error("Group not found: {0}")]
    GroupNotFound(String),

    /// Output not found
    #[error("Output not found: {0}")]
    OutputNotFound(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid arguments
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),

    /// Sway not running
    #[error("Sway is not running or IPC socket not available")]
    SwayNotRunning,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
