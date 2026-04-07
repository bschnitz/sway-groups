//! swayg CLI - Sway workspace groups management.

mod commands;

use anyhow::Result as AnyResult;
use clap::Parser;
use directories::ProjectDirs;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sway_groups_core::db::DatabaseManager;
use sway_groups_core::sway::SwayIpcClient;
use sway_groups_core::services::{GroupService, NavigationService, SuffixService, WorkspaceService};

fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "swayg", "swayg") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("swayg.db")
    } else {
        PathBuf::from("swayg.db")
    }
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("swayg=info".parse()?))
        .init();

    let db_path = get_db_path();
    info!("Using database at: {}", db_path.display());

    let db: DatabaseManager = DatabaseManager::new(db_path).await?;
    let ipc_client = SwayIpcClient::new()?;
    let suffix_service = SuffixService::new(db.clone(), ipc_client.clone());
    let group_service = GroupService::new(db.clone(), suffix_service.clone());
    let workspace_service = WorkspaceService::new(db.clone(), ipc_client.clone());
    let nav_service = NavigationService::new(db.clone(), ipc_client.clone(), suffix_service.clone());

    group_service.ensure_default_group().await?;

    let cli = commands::Cli::parse();
    commands::run(cli, &group_service, &workspace_service, &suffix_service, &nav_service, &ipc_client).await?;

    Ok(())
}
