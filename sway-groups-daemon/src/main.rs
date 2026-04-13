use std::path::PathBuf;

use anyhow::Result;
use sea_orm::{ActiveModelTrait, ModelTrait, Set};
use sway_groups_core::db::DatabaseManager;
use sway_groups_core::db::entities::{
    workspace, workspace_group,
    GroupEntity, OutputEntity, PendingWorkspaceEventEntity, WorkspaceEntity, WorkspaceGroupEntity,
};
use sway_groups_core::sway::SwayIpcClient;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

fn default_db_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "swayg", "swayg") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("swayg.db")
    } else {
        PathBuf::from("swayg.db")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_parent = default_db_path()
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf();

    std::fs::create_dir_all(&db_parent)?;

    let file_appender = tracing_appender::rolling::daily(&db_parent, "swayg-daemon.log");
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("sway_groups_daemon=info".parse()?))
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    info!("swayg-daemon starting");

    let db_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_db_path);

    let db = DatabaseManager::new(db_path).await?;
    let ipc = SwayIpcClient::new()?;

    info!("Subscribing to sway workspace events");
    let mut event_stream = ipc.subscribe(&["workspace"])?;

    loop {
        match event_stream.read_event() {
            Ok((event_type, payload)) => {
                if event_type == sway_groups_core::sway::SwayEventType::Workspace as u32 {
                    handle_workspace_event(&db, &ipc, &payload).await;
                }
            }
            Err(e) => {
                error!("Error reading sway event: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_workspace_event(db: &DatabaseManager, ipc: &SwayIpcClient, payload: &[u8]) {
    let event: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse workspace event: {}", e);
            return;
        }
    };

    let change = event.get("change").and_then(|v| v.as_str()).unwrap_or("");
    if change != "init" && change != "new" && change != "empty" {
        return;
    }

    let ws_info = match event.get("current").and_then(|v| v.as_object()) {
        Some(v) => v,
        None => return,
    };

    let ws_name = match ws_info.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return,
    };

    info!("Workspace event: change={}, name={}", change, ws_name);

    if change == "empty" {
        handle_workspace_destroyed(db, ipc, &ws_name).await;
        return;
    }

    handle_workspace_created(db, ipc, &ws_name, ws_info).await;
}

async fn handle_workspace_created(db: &DatabaseManager, ipc: &SwayIpcClient, ws_name: &str, ws_info: &serde_json::Map<String, serde_json::Value>) {
    let ws_output = ws_info.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let ws_num = ws_info.get("num").and_then(|v| v.as_i64());
    let now = chrono::Utc::now().naive_utc();

    let timeout = chrono::Duration::seconds(5);

    let stale: Vec<_> = PendingWorkspaceEventEntity::find_stale(timeout)
        .all(db.conn())
        .await
        .unwrap_or_default();

    for entry in &stale {
        let _: Result<_, _> = entry.clone().delete(db.conn()).await;
    }
    if !stale.is_empty() {
        info!("Cleaned up {} stale pending events", stale.len());
    }

    let pending: Vec<_> = PendingWorkspaceEventEntity::find_by_name(ws_name)
        .all(db.conn())
        .await
        .unwrap_or_default();

    if !pending.is_empty() {
        info!("Workspace '{}' is in pending events, skipping (swayg will handle it)", ws_name);
        return;
    }

    let existing = WorkspaceEntity::find_by_name(ws_name)
        .one(db.conn())
        .await
        .unwrap_or(None);

    if existing.is_some() {
        return;
    }

    info!("External workspace detected: '{}' on output '{}', adding to active group", ws_name, ws_output);

    let ws_active = workspace::ActiveModel {
        name: Set(ws_name.to_string()),
        number: Set(ws_num.map(|n| n as i32)),
        output: Set(Some(ws_output.clone())),
        is_global: Set(false),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let ws = match ws_active.insert(db.conn()).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Failed to insert workspace '{}': {}", ws_name, e);
            return;
        }
    };

    let output_model = OutputEntity::find_by_name(&ws_output)
        .one(db.conn())
        .await
        .unwrap_or(None);

    let active_group = output_model
        .as_ref()
        .map(|o| o.active_group.clone())
        .unwrap_or_else(|| "0".to_string());

    if let Some(group) = GroupEntity::find_by_name(&active_group)
        .one(db.conn())
        .await
        .unwrap_or(None)
    {
        let membership = workspace_group::ActiveModel {
            workspace_id: Set(ws.id),
            group_id: Set(group.id),
            created_at: Set(Some(now)),
            ..Default::default()
        };
        if let Err(e) = membership.insert(db.conn()).await {
            error!("Failed to add workspace '{}' to group '{}': {}", ws_name, active_group, e);
            return;
        }
        info!("Added external workspace '{}' to group '{}'", ws_name, active_group);
    } else {
        warn!("Active group '{}' not found for output '{}', workspace '{}' not assigned to any group", active_group, ws_output, ws_name);
    }

    let waybar_client = sway_groups_core::sway::WaybarClient::new();
    let waybar_sync = sway_groups_core::services::WaybarSyncService::new(db.clone(), ipc.clone(), waybar_client);
    if let Err(e) = waybar_sync.update_waybar().await {
        warn!("Failed to update waybar: {}", e);
    }
}

async fn handle_workspace_destroyed(db: &DatabaseManager, ipc: &SwayIpcClient, ws_name: &str) {
    let existing = WorkspaceEntity::find_by_name(ws_name)
        .one(db.conn())
        .await
        .unwrap_or(None);

    if existing.is_none() {
        return;
    }

    let ws = existing.unwrap();

    let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(db.conn())
        .await
        .unwrap_or_default();

    for m in &memberships {
        let _: Result<_, _> = m.clone().delete(db.conn()).await;
    }

    if let Err(e) = ws.delete(db.conn()).await {
        error!("Failed to delete workspace '{}': {}", ws_name, e);
        return;
    }

    info!("Removed destroyed workspace '{}' from DB ({} memberships cleaned)", ws_name, memberships.len());

    let waybar_client = sway_groups_core::sway::WaybarClient::new();
    let waybar_sync = sway_groups_core::services::WaybarSyncService::new(db.clone(), ipc.clone(), waybar_client);
    if let Err(e) = waybar_sync.update_waybar().await {
        warn!("Failed to update waybar: {}", e);
    }
}
