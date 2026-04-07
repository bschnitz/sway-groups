//! swaygd - swayg daemon for automatic suffix synchronization.

use anyhow::Result as AnyResult;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sway_groups_core::db::DatabaseManager;
use sway_groups_core::sway::SwayIpcClient;
use sway_groups_core::services::{GroupService, SuffixService};

/// Get the database path.
fn get_db_path() -> PathBuf {
    let data_dir = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".local/share"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    data_dir.join("swayg").join("swayg.db")
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting swaygd daemon...");

    // Initialize database
    let db_path = get_db_path();
    let db = DatabaseManager::new(db_path).await?;
    let ipc_client = SwayIpcClient::new()?;
    let suffix_service = SuffixService::new(db, ipc_client);

    info!("Daemon initialized, listening for sway events...");

    // TODO: Implement event subscription loop
    // For now, just sync suffixes and exit
    suffix_service.sync_all_suffixes().await?;

    info!("Daemon exiting...");
    Ok(())
}
