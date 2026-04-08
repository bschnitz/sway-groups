//! Group management service.

use crate::db::entities::{group, group_state, output, GroupEntity, GroupStateEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::services::suffix_service::SuffixService;
use crate::sway::SwayIpcClient;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, ModelTrait, Set};
use tracing::{info, warn, debug};

/// Group information for display.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    pub id: i32,
    pub name: String,
    pub workspace_count: usize,
    pub workspaces: Vec<String>,
}

/// Service for group operations.
pub struct GroupService {
    db: DatabaseManager,
    suffix_service: SuffixService,
    ipc_client: SwayIpcClient,
}

impl GroupService {
    /// Create a new group service.
    pub fn new(db: DatabaseManager, suffix_service: SuffixService, ipc_client: SwayIpcClient) -> Self {
        Self { db, suffix_service, ipc_client }
    }

    /// List all groups with their workspaces.
    pub async fn list_groups(
        &self,
        output_filter: Option<&str>,
    ) -> Result<Vec<GroupInfo>> {
        let groups = GroupEntity::find_all_ordered()
            .all(self.db.conn())
            .await?;

        let mut result = Vec::new();

        for group in groups {
            let memberships = WorkspaceGroupEntity::find_by_group(group.id)
                .all(self.db.conn())
                .await?;

            let mut workspace_names = Vec::new();
            for membership in memberships {
                if let Some(ws) = WorkspaceEntity::find_by_id(membership.workspace_id)
                    .one(self.db.conn())
                    .await?
                {
                    // Filter by output if specified
                    if let Some(output) = output_filter
                        && ws.output.as_ref() != Some(&output.to_string()) {
                        continue;
                    }
                    workspace_names.push(ws.name);
                }
            }

            result.push(GroupInfo {
                id: group.id,
                name: group.name,
                workspace_count: workspace_names.len(),
                workspaces: workspace_names,
            });
        }

        Ok(result)
    }

    /// List all group names alphabetically, without workspace details.
    pub async fn list_all_group_names(&self) -> Result<Vec<String>> {
        let groups = GroupEntity::find_all_ordered()
            .all(self.db.conn())
            .await?;

        Ok(groups.into_iter().map(|g| g.name).collect())
    }

    /// List non-empty group names for a specific output, alphabetically.
    pub async fn list_group_names_on_output(&self, output: &str) -> Result<Vec<String>> {
        let groups = self.list_groups(Some(output)).await?;
        Ok(groups
            .into_iter()
            .filter(|g| g.workspace_count > 0)
            .map(|g| g.name)
            .collect())
    }

    /// Create a new group.
    pub async fn create_group(&self, name: &str) -> Result<group::Model> {
        // Check if group already exists
        if GroupEntity::find_by_name(name)
            .one(self.db.conn())
            .await?
            .is_some()
        {
            return Err(Error::InvalidArgs(format!(
                "Group '{}' already exists",
                name
            )));
        }

        let now = chrono::Utc::now().naive_utc();
        let active = group::ActiveModel {
            name: Set(name.to_string()),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };

        let model = active.insert(self.db.conn()).await?;
        info!("Created group: {}", name);

        Ok(model)
    }

    /// Get a group by name, creating it if it doesn't exist.
    pub async fn get_or_create_group(&self, name: &str) -> Result<group::Model> {
        if let Some(g) = GroupEntity::find_by_name(name)
            .one(self.db.conn())
            .await?
        {
            return Ok(g);
        }

        let now = chrono::Utc::now().naive_utc();
        let active = group::ActiveModel {
            name: Set(name.to_string()),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };

        let model = active.insert(self.db.conn()).await?;
        info!("Auto-created group: {}", name);

        Ok(model)
    }

    /// Delete a group.
    pub async fn delete_group(&self, name: &str, force: bool) -> Result<()> {
        // Cannot delete the default group "0"
        if name == "0" {
            return Err(Error::InvalidArgs("Cannot delete the default group '0'".to_string()));
        }

        let group = GroupEntity::find_by_name(name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(name.to_string()))?;

        // Get memberships
        let memberships = WorkspaceGroupEntity::find_by_group(group.id)
            .all(self.db.conn())
            .await?;

        if !memberships.is_empty() && !force {
            warn!(
                "Group '{}' has {} workspaces. Use --force to delete anyway.",
                name,
                memberships.len()
            );
            return Err(Error::InvalidArgs(format!(
                "Group '{}' has {} workspaces. Use --force to delete anyway.",
                name,
                memberships.len()
            )));
        }

        // If we have memberships and force is true, remove them
        for membership in memberships {
            membership.delete(self.db.conn()).await?;
        }

        // Delete the group
        group.delete(self.db.conn()).await?;
        info!("Deleted group: {}", name);

        Ok(())
    }

    /// Rename a group.
    pub async fn rename_group(&self, old_name: &str, new_name: &str) -> Result<()> {
        // Cannot rename the default group "0"
        if old_name == "0" {
            return Err(Error::InvalidArgs("Cannot rename the default group '0'".to_string()));
        }

        let mut group = GroupEntity::find_by_name(old_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(old_name.to_string()))?
            .into_active_model();

        // Check if new name already exists
        if GroupEntity::find_by_name(new_name)
            .one(self.db.conn())
            .await?
            .is_some()
        {
            return Err(Error::InvalidArgs(format!(
                "Group '{}' already exists",
                new_name
            )));
        }

        group.name = Set(new_name.to_string());
        group.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        group.update(self.db.conn()).await?;

        // Update outputs that reference the old group name
        let affected_outputs = OutputEntity::find_by_active_group(old_name)
            .all(self.db.conn())
            .await?;
        for output in affected_outputs {
            let mut active = output.into_active_model();
            active.active_group = Set(new_name.to_string());
            active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
            active.update(self.db.conn()).await?;
        }

        // Update group_state entries that reference the old group name
        let affected_states = GroupStateEntity::find_by_group_name(old_name)
            .all(self.db.conn())
            .await?;
        for state in affected_states {
            let mut active = state.into_active_model();
            active.group_name = Set(new_name.to_string());
            active.update(self.db.conn()).await?;
        }

        info!("Renamed group: {} -> {}", old_name, new_name);
        Ok(())
    }

    /// Get the active group for an output.
    pub async fn get_active_group(&self, output: &str) -> Result<String> {
        let output = OutputEntity::find_by_name(output)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::OutputNotFound(output.to_string()))?;

        Ok(output.active_group)
    }

    /// Save the currently focused workspace for a group on an output.
    async fn save_current_workspace(&self, output: &str, group_name: &str) -> Result<()> {
        let current_ws = match self.ipc_client.get_focused_workspace() {
            Ok(ws) => ws,
            Err(_) => return Ok(()),
        };

        let base_name = self.suffix_service.get_base_name(&current_ws.name);
        let now = chrono::Utc::now().naive_utc();

        let existing = GroupStateEntity::find_by_output_and_group(output, group_name)
            .one(self.db.conn())
            .await?;

        if let Some(state) = existing {
            let mut active = state.into_active_model();
            active.last_focused_workspace = Set(Some(base_name));
            active.last_visited = Set(Some(now));
            active.update(self.db.conn()).await?;
        } else {
            let active = group_state::ActiveModel {
                output: Set(output.to_string()),
                group_name: Set(group_name.to_string()),
                last_focused_workspace: Set(Some(base_name)),
                last_visited: Set(Some(now)),
                ..Default::default()
            };
            active.insert(self.db.conn()).await?;
        }

        Ok(())
    }

    /// Get the last focused workspace for a group on an output.
    async fn get_last_focused_workspace(&self, output: &str, group_name: &str) -> Result<Option<String>> {
        let state = GroupStateEntity::find_by_output_and_group(output, group_name)
            .one(self.db.conn())
            .await?;

        Ok(state.and_then(|s| s.last_focused_workspace))
    }

    /// Switch focus to a workspace via sway IPC.
    fn focus_workspace(&self, workspace_name: &str) -> Result<()> {
        let command = format!("workspace \"{}\"", workspace_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                info!("Focused workspace '{}'", workspace_name);
                return Ok(());
            } else {
                return Err(Error::SwayIpc(
                    result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                ));
            }
        }
        Err(Error::SwayIpc("Empty response from sway".to_string()))
    }

    /// Get workspaces belonging to a group on a specific output, sorted alphabetically.
    async fn get_workspaces_for_group_on_output(&self, group_name: &str, output: &str) -> Result<Vec<String>> {
        let group = match GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
        {
            Some(g) => g,
            None => return Ok(Vec::new()),
        };

        let memberships = WorkspaceGroupEntity::find_by_group(group.id)
            .all(self.db.conn())
            .await?;

        let mut ws_names = Vec::new();
        for membership in memberships {
            if let Some(ws) = WorkspaceEntity::find_by_id(membership.workspace_id)
                .one(self.db.conn())
                .await?
                && let Some(ref ws_output) = ws.output
                    && ws_output == output {
                        ws_names.push(ws.name);
                    }
        }

        ws_names.sort();
        Ok(ws_names)
    }

    /// Set the active group for an output and switch workspace focus.
    pub async fn set_active_group(&self, output: &str, group: &str) -> Result<()> {
        // Verify group exists
        if GroupEntity::find_by_name(group)
            .one(self.db.conn())
            .await?
            .is_none()
        {
            return Err(Error::GroupNotFound(group.to_string()));
        }

        // Save currently focused workspace for the old group
        let old_group = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        if old_group != group {
            self.save_current_workspace(output, &old_group).await?;
        }
        debug!("set_active_group: output={}, old_group='{}', new_group='{}'", output, old_group, group);

        // Get or create output
        let output_model = OutputEntity::find_by_name(output)
            .one(self.db.conn())
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(existing) = output_model {
            let mut active = existing.into_active_model();
            active.active_group = Set(group.to_string());
            active.updated_at = Set(Some(now));
            active.update(self.db.conn()).await?;
        } else {
            let active = output::ActiveModel {
                name: Set(output.to_string()),
                active_group: Set(group.to_string()),
                created_at: Set(Some(now)),
                updated_at: Set(Some(now)),
                ..Default::default()
            };
            active.insert(self.db.conn()).await?;
        }

        // Sync suffixes for all outputs
        self.suffix_service.sync_all_suffixes().await?;

        // Handle workspace focus for the new group
        let group_workspaces = self.get_workspaces_for_group_on_output(group, output).await?;
        debug!("set_active_group: workspaces in group '{}' on '{}': {:?}", group, output, group_workspaces);

        if group_workspaces.is_empty() {
            // Case 1: Group has no workspaces -> focus workspace "0"
            debug!("set_active_group: case 1 (empty group), focusing workspace '0'");
            self.focus_workspace("0")?;
        } else {
            // Check if this group was previously visited on this output
            let last_focused = self.get_last_focused_workspace(output, group).await?;
            debug!("set_active_group: last_focused_workspace = {:?}", last_focused);

            if let Some(ref ws_name) = last_focused {
                // Case 3: Group was visited before -> restore last focused workspace
                // Verify it still exists in the group
                if group_workspaces.iter().any(|w| w == ws_name) {
                    debug!("set_active_group: case 3 (revisit), focusing '{}'", ws_name);
                    self.focus_workspace(ws_name)?;
                } else {
                    debug!("set_active_group: case 3 fallback (workspace no longer in group), focusing '{}'", group_workspaces[0]);
                    self.focus_workspace(&group_workspaces[0])?;
                }
            } else {
                // Case 2: First visit -> focus first workspace alphabetically
                debug!("set_active_group: case 2 (first visit), focusing '{}'", group_workspaces[0]);
                self.focus_workspace(&group_workspaces[0])?;
            }
        }

        let focused = self.ipc_client.get_focused_workspace().ok().map(|ws| ws.name);
        debug!("set_active_group: sway focused workspace after switch = {:?}", focused);

        // Record this visit
        self.save_current_workspace(output, group).await?;

        info!("Set active group for {} to '{}'", output, group);

        if old_group != "0" && old_group != group {
            let groups = self.list_groups(None).await?;
            if let Some(g) = groups.iter().find(|g| g.name == old_group) && g.workspace_count == 0 {
                self.delete_group(&old_group, true).await?;
                info!("Auto-removed empty group '{}' after switch", old_group);
            }
        }

        Ok(())
    }

    /// Switch to next group alphabetically (all groups).
    pub async fn next_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_all_group_names().await?;

        if group_names.is_empty() {
            return Ok(None);
        }

        let current_idx = group_names.iter().position(|g| g == &current);
        let next_idx = match current_idx {
            Some(idx) if idx + 1 < group_names.len() => idx + 1,
            Some(_) if wrap => 0,
            Some(_) => return Ok(None),
            None => 0,
        };

        let next_name = group_names[next_idx].clone();
        self.set_active_group(output, &next_name).await?;
        Ok(Some(next_name))
    }

    /// Switch to next non-empty group on a specific output.
    pub async fn next_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_group_names_on_output(output).await?;

        if group_names.is_empty() {
            return Ok(None);
        }

        let current_idx = group_names.iter().position(|g| g == &current);
        let next_idx = match current_idx {
            Some(idx) if idx + 1 < group_names.len() => idx + 1,
            Some(_) if wrap => 0,
            Some(_) => return Ok(None),
            None => 0,
        };

        let next_name = group_names[next_idx].clone();
        self.set_active_group(output, &next_name).await?;
        Ok(Some(next_name))
    }

    /// Switch to previous group alphabetically (all groups).
    pub async fn prev_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_all_group_names().await?;

        if group_names.is_empty() {
            return Ok(None);
        }

        let current_idx = group_names.iter().position(|g| g == &current);
        let prev_idx = match current_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) if wrap => group_names.len() - 1,
            Some(_) => return Ok(None),
            None => group_names.len() - 1,
        };

        let prev_name = group_names[prev_idx].clone();
        self.set_active_group(output, &prev_name).await?;
        Ok(Some(prev_name))
    }

    /// Switch to previous non-empty group on a specific output.
    pub async fn prev_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_group_names_on_output(output).await?;

        if group_names.is_empty() {
            return Ok(None);
        }

        let current_idx = group_names.iter().position(|g| g == &current);
        let prev_idx = match current_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) if wrap => group_names.len() - 1,
            Some(_) => return Ok(None),
            None => group_names.len() - 1,
        };

        let prev_name = group_names[prev_idx].clone();
        self.set_active_group(output, &prev_name).await?;
        Ok(Some(prev_name))
    }

    /// Remove empty groups.
    pub async fn prune_groups(&self, keep: &[String]) -> Result<usize> {
        let groups = self.list_groups(None).await?;
        let mut removed = 0;

        for group in groups {
            // Skip if in keep list
            if keep.iter().any(|k| k == &group.name) {
                continue;
            }

            // Skip default group "0"
            if group.name == "0" {
                continue;
            }

            if group.workspace_count == 0 {
                self.delete_group(&group.name, true).await?;
                removed += 1;
            }
        }

        info!("Pruned {} empty groups", removed);
        Ok(removed)
    }

    /// Ensure the default group "0" exists.
    pub async fn ensure_default_group(&self) -> Result<()> {
        if !GroupEntity::has_default_group(self.db.conn()).await? {
            self.create_group("0").await?;
            info!("Created default group '0'");
        }
        Ok(())
    }
}
