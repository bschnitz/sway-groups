use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

fn default_state_file() -> PathBuf {
    PathBuf::from("/tmp/swayg-daemon-test.state")
}

fn write_state(state_file: &std::path::Path, state: &str) {
    let tmp = state_file.with_extension("tmp");
    if let Ok(mut f) = std::fs::File::create(&tmp) {
        use std::io::Write;
        let _ = f.write_all(state.as_bytes());
        let _ = f.write_all(b"\n");
        let _ = std::fs::rename(&tmp, state_file);
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let db_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_db_path);

    let state_file: PathBuf = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_state_file);

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

    let config = sway_groups_config::SwaygConfig::load()?;

    let paused = Arc::new(AtomicBool::new(false));

    let paused_loop = Arc::clone(&paused);
    let paused_thread = Arc::clone(&paused);
    let sf = state_file.clone();
    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::signal::SIGUSR1,
        signal_hook::consts::signal::SIGUSR2,
    ])?;
    std::thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                signal_hook::consts::signal::SIGUSR1 => {
                    paused_thread.store(true, Ordering::Relaxed);
                    write_state(&sf, "paused");
                    info!("Daemon paused via SIGUSR1");
                }
                signal_hook::consts::signal::SIGUSR2 => {
                    paused_thread.store(false, Ordering::Relaxed);
                    write_state(&sf, "running");
                    info!("Daemon resumed via SIGUSR2");
                }
                _ => {}
            }
        }
    });

    write_state(&state_file, "running");
    info!("swayg-daemon starting (db={}, state_file={})", db_path.display(), state_file.display());

    let ipc = SwayIpcClient::new()?;

    info!("Subscribing to sway workspace and window events");
    let mut event_stream = ipc.subscribe(&["workspace", "window"])?;

    loop {
        if paused_loop.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            continue;
        }

        match event_stream.read_event() {
            Ok((event_type, payload)) => {
                if paused_loop.load(Ordering::Relaxed) {
                    continue;
                }
                if event_type == sway_groups_core::sway::SwayEventType::Workspace as u32 {
                    handle_workspace_event(&db_path, &ipc, &payload, &config).await;
                } else if event_type == sway_groups_core::sway::SwayEventType::Window as u32 {
                    handle_window_event(&db_path, &ipc, &payload, &config).await;
                }
            }
            Err(e) => {
                error!("Error reading sway event: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_workspace_event(db_path: &Path, ipc: &SwayIpcClient, payload: &[u8], config: &sway_groups_config::SwaygConfig) {
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

    let db = match DatabaseManager::new(db_path.to_path_buf()).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to open DB '{}': {}", db_path.display(), e);
            return;
        }
    };

    if change == "empty" {
        handle_workspace_destroyed(&db, ipc, &ws_name, config).await;
        return;
    }

    handle_workspace_created(&db, ipc, &ws_name, ws_info, config).await;
}

async fn handle_workspace_created(db: &DatabaseManager, ipc: &SwayIpcClient, ws_name: &str, ws_info: &serde_json::Map<String, serde_json::Value>, config: &sway_groups_config::SwaygConfig) {
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

    if let Some(ref ws) = existing {
        // Workspace already in DB. Check if it has any group membership or is global.
        // If orphaned (e.g. after a sway restart with a stale DB), adopt it into
        // the active group so it becomes visible again.
        if !ws.is_global {
            let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
                .all(db.conn())
                .await
                .unwrap_or_default();
            if memberships.is_empty() {
                info!("Workspace '{}' exists but is orphaned (no group, not global), adopting", ws_name);
                adopt_into_active_group(db, ws.id, &ws_output, now).await;
                let waybar_sync = sway_groups_core::services::WaybarSyncService::with_config(db.clone(), ipc.clone(), config);
                if let Err(e) = waybar_sync.update_waybar().await {
                    warn!("Failed to update waybar: {}", e);
                }
            }
        }
        return;
    }

    info!("External workspace detected: '{}' on output '{}'", ws_name, ws_output);

    // Check config assignment rules before inserting.
    let rules = config.matching_rules(ws_name);
    let make_global = rules.iter().any(|r| r.global);
    let rule_groups: Vec<String> = rules
        .iter()
        .flat_map(|r| r.groups.iter().cloned())
        .collect();
    let has_rule_groups = !rule_groups.is_empty();

    let ws_active = workspace::ActiveModel {
        name: Set(ws_name.to_string()),
        number: Set(ws_num.map(|n| n as i32)),
        output: Set(Some(ws_output.clone())),
        is_global: Set(make_global),
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

    if make_global {
        info!("Marked workspace '{}' as global (config rule)", ws_name);
    }

    if has_rule_groups {
        // Assignment rules specify groups — use those instead of active group.
        for group_name in &rule_groups {
            if let Some(group) = GroupEntity::find_by_name(group_name)
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
                    error!("Failed to add workspace '{}' to group '{}': {}", ws_name, group_name, e);
                } else {
                    info!("Added workspace '{}' to group '{}' (config rule)", ws_name, group_name);
                }
            } else {
                warn!("Config rule references group '{}' which does not exist, skipping", group_name);
            }
        }
    } else {
        // No rule groups — fall back to adding to the active group.
        let output_model = OutputEntity::find_by_name(&ws_output)
            .one(db.conn())
            .await
            .unwrap_or(None);

        let active_group = output_model
            .as_ref()
            .and_then(|o| o.active_group.clone());

        if let Some(ref ag) = active_group {
            if let Some(group) = GroupEntity::find_by_name(ag)
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
                    error!("Failed to add workspace '{}' to group '{}': {}", ws_name, ag, e);
                    return;
                }
                info!("Added external workspace '{}' to group '{}'", ws_name, ag);
            } else {
                warn!("Active group '{}' not found for output '{}', workspace '{}' not assigned to any group", ag, ws_output, ws_name);
            }
        } else {
            info!("No active group for output '{}', workspace '{}' not assigned", ws_output, ws_name);
        }
    }

    let waybar_sync = sway_groups_core::services::WaybarSyncService::with_config(db.clone(), ipc.clone(), config);
    if let Err(e) = waybar_sync.update_waybar().await {
        warn!("Failed to update waybar: {}", e);
    }
}

async fn handle_workspace_destroyed(db: &DatabaseManager, ipc: &SwayIpcClient, ws_name: &str, config: &sway_groups_config::SwaygConfig) {
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

    let waybar_sync = sway_groups_core::services::WaybarSyncService::with_config(db.clone(), ipc.clone(), config);
    if let Err(e) = waybar_sync.update_waybar().await {
        warn!("Failed to update waybar: {}", e);
    }
}

async fn handle_window_event(db_path: &Path, ipc: &SwayIpcClient, payload: &[u8], config: &sway_groups_config::SwaygConfig) {
    let event: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(v) => v,
        Err(_) => return,
    };

    let change = event.get("change").and_then(|v| v.as_str()).unwrap_or("");
    if change != "urgent" {
        return;
    }

    info!("Window urgency change detected, updating waybar");

    let db = match DatabaseManager::new(db_path.to_path_buf()).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to open DB '{}': {}", db_path.display(), e);
            return;
        }
    };

    let waybar_sync = sway_groups_core::services::WaybarSyncService::with_config(db.clone(), ipc.clone(), config);
    if let Err(e) = waybar_sync.update_waybar().await {
        warn!("Failed to update waybar workspaces: {}", e);
    }
    if let Err(e) = waybar_sync.update_waybar_groups().await {
        warn!("Failed to update waybar groups: {}", e);
    }
}

/// Add an existing workspace to the active group of the given output.
async fn adopt_into_active_group(
    db: &DatabaseManager,
    ws_id: i32,
    ws_output: &str,
    now: chrono::NaiveDateTime,
) {
    let output_model = OutputEntity::find_by_name(ws_output)
        .one(db.conn())
        .await
        .unwrap_or(None);

    let active_group = output_model
        .as_ref()
        .and_then(|o| o.active_group.clone());

    if let Some(ref ag) = active_group {
        if let Some(group) = GroupEntity::find_by_name(ag)
            .one(db.conn())
            .await
            .unwrap_or(None)
        {
            let membership = workspace_group::ActiveModel {
                workspace_id: Set(ws_id),
                group_id: Set(group.id),
                created_at: Set(Some(now)),
                ..Default::default()
            };
            if let Err(e) = membership.insert(db.conn()).await {
                error!("Failed to adopt workspace into group '{}': {}", ag, e);
            } else {
                info!("Adopted orphaned workspace into group '{}'", ag);
            }
        }
    }
}
