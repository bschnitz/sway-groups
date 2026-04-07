//! swaygd - swayg daemon for automatic suffix synchronization.

use anyhow::Result as AnyResult;
use directories::ProjectDirs;
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sway_groups_core::db::DatabaseManager;
use sway_groups_core::sway::{EventStream, SwayEventType, SwayIpcClient};
use sway_groups_core::services::SuffixService;

fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "swayg", "swayg") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("swayg.db")
    } else {
        PathBuf::from("swayg.db")
    }
}

async fn handle_event(
    event_type: u32,
    _payload: &[u8],
    suffix_service: &SuffixService,
) -> AnyResult<()> {
    match event_type {
        t if t == SwayEventType::Workspace as u32 => {
            info!("Workspace event received, syncing suffixes...");
            suffix_service.sync_all_suffixes().await?;
        }
        t if t == SwayEventType::Output as u32 => {
            info!("Output event received, syncing suffixes...");
            suffix_service.sync_all_suffixes().await?;
        }
        t if t == SwayEventType::Shutdown as u32 => {
            warn!("Sway is shutting down, daemon exiting.");
            std::process::exit(0);
        }
        _ => {}
    }
    Ok(())
}

async fn event_loop(mut event_stream: EventStream, suffix_service: SuffixService) -> AnyResult<()> {
    info!("Listening for sway events...");

    loop {
        match event_stream.read_event() {
            Ok((event_type, payload)) => {
                if let Err(e) = handle_event(event_type, &payload, &suffix_service).await {
                    error!("Error handling event (type={}): {}", event_type, e);
                }
            }
            Err(e) => {
                error!("Error reading event from sway IPC: {}", e);
                // Back off before reconnecting
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                return Err(e.into());
            }
        }
    }
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("swaygd=info".parse()?),
        )
        .init();

    info!("Starting swaygd daemon...");

    let db_path = get_db_path();
    let db = DatabaseManager::new(db_path).await?;
    let ipc_client = SwayIpcClient::new()?;
    let suffix_service = SuffixService::new(db.clone(), ipc_client.clone());

    suffix_service.sync_all_suffixes().await?;

    let event_stream = ipc_client.subscribe(&["workspace", "output", "shutdown"])?;
    event_loop(event_stream, suffix_service).await?;

    Ok(())
}
