//! Workspace navigation service.

use crate::db::entities::{focus_history, GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity, FocusHistoryEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::services::suffix_service::SuffixService;
use crate::sway::SwayIpcClient;
use sea_orm::{ActiveModelTrait, EntityTrait, ModelTrait, Set};
use tracing::info;

/// Service for workspace navigation operations.
pub struct NavigationService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
    suffix_service: SuffixService,
}

impl NavigationService {
    /// Create a new navigation service.
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient, suffix_service: SuffixService) -> Self {
        Self { db, ipc_client, suffix_service }
    }

    /// Get workspaces visible in the active group on an output.
    /// Includes global workspaces.
    pub async fn get_visible_workspaces(&self, output_name: &str) -> Result<Vec<String>> {
        let active_group = OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or_else(|| "0".to_string());

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let mut visible = Vec::new();

        for sway_ws in sway_workspaces.iter().filter(|w| w.output == output_name) {
            let base_name = self.suffix_service.get_base_name(&sway_ws.name);

            if let Some(workspace) = WorkspaceEntity::find_by_name(&base_name)
                .one(self.db.conn())
                .await?
            {
                // Global workspaces are always visible
                if workspace.is_global {
                    visible.push(base_name.clone());
                    continue;
                }

                // Check if workspace is in the active group
                let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
                    .all(self.db.conn())
                    .await?;

                for m in &memberships {
                    if let Some(group) = GroupEntity::find_by_id(m.group_id)
                        .one(self.db.conn())
                        .await?
                        && group.name == active_group {
                            visible.push(base_name.clone());
                            break;
                        }
                }

                // Workspaces not in any group are treated as in group "0"
                if memberships.is_empty() && active_group == "0" {
                    visible.push(base_name);
                }
            }
        }

        visible.sort();
        Ok(visible)
    }

    /// Get all visible workspaces across all outputs.
    pub async fn get_visible_workspaces_global(&self) -> Result<Vec<String>> {
        let outputs = self.ipc_client.get_outputs()?;
        let mut all = Vec::new();

        for output in outputs {
            let ws = self.get_visible_workspaces(&output.name).await?;
            all.extend(ws);
        }

        all.sort();
        all.dedup();
        Ok(all)
    }

    /// Navigate to the next workspace in the active group on an output.
    pub async fn next_workspace(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces(output).await?;
        let current = self.ipc_client.get_focused_workspace()?;
        let current_base = self.suffix_service.get_base_name(&current.name);

        let next = find_next(&visible, &current_base, wrap);
        if let Some(ref target) = next {
            self.navigate_to_workspace(target).await?;
        }
        Ok(next)
    }

    /// Navigate to the next workspace globally (across all outputs).
    pub async fn next_workspace_global(&self, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces_global().await?;
        let current = self.ipc_client.get_focused_workspace()?;
        let current_base = self.suffix_service.get_base_name(&current.name);

        let next = find_next(&visible, &current_base, wrap);
        if let Some(ref target) = next {
            self.navigate_to_workspace(target).await?;
        }
        Ok(next)
    }

    /// Navigate to the previous workspace in the active group on an output.
    pub async fn prev_workspace(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces(output).await?;
        let current = self.ipc_client.get_focused_workspace()?;
        let current_base = self.suffix_service.get_base_name(&current.name);

        let prev = find_prev(&visible, &current_base, wrap);
        if let Some(ref target) = prev {
            self.navigate_to_workspace(target).await?;
        }
        Ok(prev)
    }

    /// Navigate to the previous workspace globally (across all outputs).
    pub async fn prev_workspace_global(&self, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces_global().await?;
        let current = self.ipc_client.get_focused_workspace()?;
        let current_base = self.suffix_service.get_base_name(&current.name);

        let prev = find_prev(&visible, &current_base, wrap);
        if let Some(ref target) = prev {
            self.navigate_to_workspace(target).await?;
        }
        Ok(prev)
    }

    /// Navigate to a specific workspace by name or number.
    pub async fn go_workspace(&self, workspace: &str) -> Result<()> {
        self.navigate_to_workspace(workspace).await
    }

    /// Move the currently focused container to a specific workspace.
    pub async fn move_to_workspace(&self, workspace_name: &str) -> Result<()> {
        let command = format!("move container to workspace \"{}\"", workspace_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                info!("Moved container to workspace '{}'", workspace_name);
                Ok(())
            } else {
                Err(Error::SwayIpc(
                    result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
        } else {
            Err(Error::SwayIpc("Empty response from sway".to_string()))
        }
    }

    /// Navigate back to the previously focused workspace.
    pub async fn go_back(&self) -> Result<Option<String>> {
        let current = self.ipc_client.get_focused_workspace()?;
        let current_base = self.suffix_service.get_base_name(&current.name);

        let last = FocusHistoryEntity::find_last_focused(&current_base)
            .one(self.db.conn())
            .await?;

        if let Some(entry) = last {
            self.navigate_to_workspace(&entry.workspace_name).await?;
            return Ok(Some(entry.workspace_name));
        }

        Ok(None)
    }

    /// Record focus on a workspace (call after navigation).
    pub async fn record_focus(&self, workspace_name: &str) -> Result<()> {
        let now = chrono::Utc::now().naive_utc();

        let active = focus_history::ActiveModel {
            workspace_name: Set(workspace_name.to_string()),
            focused_at: Set(now),
            ..Default::default()
        };
        active.insert(self.db.conn()).await?;

        // Prune entries older than 10 minutes
        self.prune_focus_history().await?;

        Ok(())
    }

    /// Prune focus history entries older than 10 minutes.
    pub async fn prune_focus_history(&self) -> Result<u64> {
        let max_age = chrono::Duration::minutes(10);
        let old_entries = FocusHistoryEntity::find_by_max_age(max_age)
            .all(self.db.conn())
            .await?;

        let count = old_entries.len() as u64;
        for entry in old_entries {
            entry.delete(self.db.conn()).await?;
        }

        if count > 0 {
            info!("Pruned {} old focus history entries", count);
        }

        Ok(count)
    }

    /// Navigate to a workspace via sway IPC and record focus.
    /// If workspace_name is a base name, resolves it to the actual sway name (with suffix).
    async fn navigate_to_workspace(&self, workspace_name: &str) -> Result<()> {
        let sway_name = self.resolve_sway_workspace_name(workspace_name)?;

        let command = format!("workspace \"{}\"", sway_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                let base = self.suffix_service.get_base_name(&sway_name);
                self.record_focus(&base).await?;
                info!("Navigated to workspace '{}'", sway_name);
                Ok(())
            } else {
                Err(Error::SwayIpc(
                    result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
        } else {
            Err(Error::SwayIpc("Empty response from sway".to_string()))
        }
    }

    /// Resolve a workspace name (possibly base) to the actual sway workspace name.
    /// If the name matches an existing sway workspace, returns that name.
    /// Otherwise checks if a sway workspace has this as its base name (e.g. "3" -> "3_class_global").
    fn resolve_sway_workspace_name(&self, workspace_name: &str) -> Result<String> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;

        // Exact match first
        for ws in &sway_workspaces {
            if ws.name == workspace_name {
                return Ok(ws.name.clone());
            }
        }

        // Check if any sway workspace has this as its base name
        for ws in &sway_workspaces {
            if self.suffix_service.get_base_name(&ws.name) == workspace_name {
                return Ok(ws.name.clone());
            }
        }

        // No match found — return as-is (sway will create it)
        Ok(workspace_name.to_string())
    }
}

/// Find the next item in a sorted list.
fn find_next(items: &[String], current: &str, wrap: bool) -> Option<String> {
    let idx = items.iter().position(|i| i == current);

    match idx {
        Some(i) if i + 1 < items.len() => Some(items[i + 1].clone()),
        Some(_) if wrap => items.first().cloned(),
        _ => None,
    }
}

/// Find the previous item in a sorted list.
fn find_prev(items: &[String], current: &str, wrap: bool) -> Option<String> {
    let idx = items.iter().position(|i| i == current);

    match idx {
        Some(i) if i > 0 => Some(items[i - 1].clone()),
        Some(_) if wrap => items.last().cloned(),
        _ => None,
    }
}
