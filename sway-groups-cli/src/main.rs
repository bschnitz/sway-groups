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
use sway_groups_core::sway::SwayIpcClient;
use sway_groups_core::services::{GroupService, NavigationService, SuffixService, WorkspaceService};

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

    let file_appender = RollingFileAppender::new(Rotation::DAILY, get_data_dir(), "swayg");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .with(EnvFilter::from_default_env()
            .add_directive(format!("swayg={}", level).parse()?)
            .add_directive(format!("sway_groups_core={}", level).parse()?))
        .init();

    let db_path = get_db_path();
    info!("Using database at: {}", db_path.display());

    let db: DatabaseManager = DatabaseManager::new(db_path).await?;
    let ipc_client = SwayIpcClient::new()?;
    let suffix_service = SuffixService::new(db.clone(), ipc_client.clone());
    let group_service = GroupService::new(db.clone(), suffix_service.clone(), ipc_client.clone());
    let workspace_service = WorkspaceService::new(db.clone(), ipc_client.clone());
    let nav_service = NavigationService::new(db.clone(), ipc_client.clone(), suffix_service.clone());

    group_service.ensure_default_group().await?;

    commands::run(cli, &group_service, &workspace_service, &suffix_service, &nav_service, &ipc_client).await?;

    Ok(())
}
