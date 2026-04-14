//! swayg CLI - Sway workspace groups management.

mod commands;

use anyhow::Result as AnyResult;
use clap::Parser;
use directories::ProjectDirs;
use std::path::PathBuf;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use sway_groups_core::db::DatabaseManager;
use sway_groups_core::services::{GroupService, NavigationService, WorkspaceService, WaybarSyncService};
use sway_groups_core::sway::{SwayIpcClient, WaybarClient};

fn get_data_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "swayg", "swayg") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.to_path_buf()
    } else {
        PathBuf::from(".")
    }
}

fn get_db_path() -> PathBuf {
    get_data_dir().join("swayg.db")
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let cli = commands::Cli::parse();

    let level = if cli.verbose { "debug" } else { "info" };

    let db_path = cli.db.clone().unwrap_or_else(get_db_path);
    let data_dir = db_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(get_data_dir);
    std::fs::create_dir_all(&data_dir).ok();

    let file_appender = RollingFileAppender::new(Rotation::DAILY, data_dir, "swayg");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .with(EnvFilter::from_default_env()
            .add_directive(format!("swayg={}", level).parse()?)
            .add_directive(format!("sway_groups_core={}", level).parse()?))
        .init();

    info!("Using database at: {}", db_path.display());

    let db: DatabaseManager = DatabaseManager::new(db_path.clone()).await?;
    let ipc_client = SwayIpcClient::new()?;
    let waybar_client = WaybarClient::new();
    let group_service = GroupService::new(db.clone(), ipc_client.clone());
    let workspace_service = WorkspaceService::new(db.clone(), ipc_client.clone());
    let waybar_sync = WaybarSyncService::new(db.clone(), ipc_client.clone(), waybar_client);
    let nav_service = NavigationService::new(db.clone(), ipc_client.clone());

    commands::run(cli, &group_service, &workspace_service, &waybar_sync, &nav_service, &ipc_client, db_path).await?;

    Ok(())
}
