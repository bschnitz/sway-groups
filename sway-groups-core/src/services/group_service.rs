//! Group management service.

use crate::db::entities::{group, group_state, output, workspace, workspace_group, FocusHistoryEntity, GroupEntity, GroupStateEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
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
    ipc_client: SwayIpcClient,
}

impl GroupService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self { db, ipc_client }
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

        // Remove memberships, keep workspace_ids for cleanup
        let mut ws_ids = Vec::new();
        for membership in memberships {
            ws_ids.push(membership.workspace_id);
            membership.delete(self.db.conn()).await?;
        }

        // Delete the group
        group.delete(self.db.conn()).await?;
        info!("Deleted group: {}", name);

        // Clean up orphaned workspaces: move to group "0" if still in sway, delete if not
        let sway_workspaces = match self.ipc_client.get_workspaces() {
            Ok(ws) => ws,
            Err(e) => {
                tracing::warn!("Could not fetch workspaces from sway: {}. Proceeding with empty list.", e);
                Vec::new()
            }
        };
        let sway_names: std::collections::HashSet<String> = sway_workspaces
            .iter()
            .map(|w| w.name.clone())
            .collect();

        let default_group = match GroupEntity::find_by_name("0")
            .one(self.db.conn())
            .await?
        {
            Some(g) => g,
            None => {
                warn!("Default group '0' not found, cannot reassign orphaned workspaces");
                return Ok(());
            }
        };

        for ws_id in &ws_ids {
            let remaining = WorkspaceGroupEntity::find_by_workspace(*ws_id)
                .all(self.db.conn())
                .await?;

            if remaining.is_empty() {
                if let Some(ws) = WorkspaceEntity::find_by_id(*ws_id)
                    .one(self.db.conn())
                    .await?
                {
                    if sway_names.contains(&ws.name) {
                        let now = chrono::Utc::now().naive_utc();
                        let membership = workspace_group::ActiveModel {
                            workspace_id: Set(*ws_id),
                            group_id: Set(default_group.id),
                            created_at: Set(Some(now)),
                            ..Default::default()
                        };
                        membership.insert(self.db.conn()).await?;
                        info!("Moved orphaned workspace '{}' to group '0'", ws.name);
                    } else {
                        if let Ok(histories) = FocusHistoryEntity::find_by_workspace_name(&ws.name)
                            .all(self.db.conn())
                            .await
                        {
                            for h in histories {
                                h.delete(self.db.conn()).await.ok();
                            }
                        }
                        info!("Removed orphaned workspace '{}'", ws.name);
                        ws.delete(self.db.conn()).await?;
                    }
                }
            }
        }

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

        let base_name = current_ws.name;
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

    /// Switch focus to an output via sway IPC.
    fn focus_output(&self, output_name: &str) -> Result<()> {
        let command = format!("focus output \"{}\"", output_name);
        let results = self.ipc_client.run_command(&command)?;
        if let Some(result) = results.first() {
            if result.success {
                return Ok(());
            }
        }
        Ok(())
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

        let old_group_needs_cleanup = old_group != group && old_group != "0";

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

        // Note: waybar sync is handled by the caller (commands.rs)

        // Switch to the target output first so workspace focus lands on the correct output
        self.focus_output(output)?;

        // Handle workspace focus for the new group
        let group_workspaces = self.get_workspaces_for_group_on_output(group, output).await?;
        debug!("set_active_group: workspaces in group '{}' on '{}': {:?}", group, output, group_workspaces);

        if group_workspaces.is_empty() {
            // Case 1: Group has no workspaces -> focus workspace "0"
            // If workspace "0" exists on a different output, sway would switch
            // to that output instead of creating it on the target output.
            // Remove it from sway first so it gets created on the target output.
            if let Ok(all_ws) = self.ipc_client.get_workspaces() {
                for ws in &all_ws {
                    if ws.name == "0" && ws.output != output && ws.output.is_empty() == false {
                        let _ = self.ipc_client.run_command(
                            &format!("workspace \"{}\"", ws.output),
                        );
                        let _ = self.ipc_client.run_command("workspace back_and_forth");
                    }
                }
            }
            debug!("set_active_group: case 1 (empty group), focusing workspace '0'");
            self.focus_workspace("0")?;

            // Ensure workspace "0" exists in DB and is in this group
            self.ensure_workspace_in_group("0", group, output).await?;
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

        // Update group-level last_visited (independent of output, used for cross-output resolution)
        self.update_group_last_visited(group, output).await?;

        info!("Set active group for {} to '{}'", output, group);

        // After switching: if old group needs cleanup, check if any non-global
        // workspaces from old group still exist in sway. If none do, delete group.
        if old_group_needs_cleanup {
            match self.should_delete_old_group(&old_group).await {
                Ok(true) => {
                    self.delete_group(&old_group, true).await?;
                    info!("Auto-removed empty group '{}' after switch (no workspaces left in sway)", old_group);
                }
                Ok(false) => {
                    debug!("set_active_group: old group '{}' still has workspaces in sway, not deleting", old_group);
                }
                Err(e) => {
                    debug!("set_active_group: error checking old group '{}': {}", old_group, e);
                }
            }
        }

        Ok(())
    }

    /// Update active group in DB only, without touching sway focus or visibility.
    /// Used by container move --switch-to-workspace when cross-group switching.
    pub async fn update_active_group_quiet(
        &self,
        _output: &str,
        _group: &str,
    ) -> Result<()> {
        let old_group = self.get_active_group(_output).await.unwrap_or_else(|_| "0".to_string());
        if old_group != _group {
            self.save_current_workspace(_output, &old_group).await?;
        }

        let output_model = OutputEntity::find_by_name(_output)
            .one(self.db.conn())
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(existing) = output_model {
            let mut active = existing.into_active_model();
            active.active_group = Set(_group.to_string());
            active.updated_at = Set(Some(now));
            active.update(self.db.conn()).await?;
        } else {
            let active = output::ActiveModel {
                name: Set(_output.to_string()),
                active_group: Set(_group.to_string()),
                created_at: Set(Some(now)),
                updated_at: Set(Some(now)),
                ..Default::default()
            };
            active.insert(self.db.conn()).await?;
        }

        self.update_group_last_visited(_group, _output).await?;

        info!("Updated active group for {} to '{}' (quiet)", _output, _group);

        Ok(())
    }

    /// Switch to next group alphabetically (all groups).
    pub async fn next_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let next_name = self.next_group_name(&output, wrap).await?;
        if let Some(ref name) = next_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(next_name)
    }

    /// Switch to next non-empty group on a specific output.
    pub async fn next_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let next_name = self.next_group_on_output_name(&output, wrap).await?;
        if let Some(ref name) = next_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(next_name)
    }

    /// Switch to previous group alphabetically (all groups).
    pub async fn prev_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let prev_name = self.prev_group_name(&output, wrap).await?;
        if let Some(ref name) = prev_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(prev_name)
    }

    /// Switch to previous non-empty group on a specific output.
    pub async fn prev_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let prev_name = self.prev_group_on_output_name(&output, wrap).await?;
        if let Some(ref name) = prev_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(prev_name)
    }

    pub async fn find_last_visited_output(&self, group: &str) -> Result<Option<String>> {
        let group_model = GroupEntity::find_by_name(group)
            .one(self.db.conn())
            .await?;
        Ok(group_model.and_then(|g| g.last_active_output))
    }

    async fn resolve_output(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            Ok(self.ipc_client.get_primary_output()?)
        } else {
            Ok(output.to_string())
        }
    }

    fn compute_next_idx(current_idx: Option<usize>, len: usize, wrap: bool) -> Option<usize> {
        match current_idx {
            Some(idx) if idx + 1 < len => Some(idx + 1),
            Some(_) if wrap => Some(0),
            Some(_) => None,
            None => Some(0),
        }
    }

    fn compute_prev_idx(current_idx: Option<usize>, len: usize, wrap: bool) -> Option<usize> {
        match current_idx {
            Some(idx) if idx > 0 => Some(idx - 1),
            Some(_) if wrap => Some(len - 1),
            Some(_) => None,
            None if len > 0 => Some(len - 1),
            None => None,
        }
    }

    pub async fn next_group_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_all_group_names().await?;
        if group_names.is_empty() {
            return Ok(None);
        }
        let current_idx = group_names.iter().position(|g| g == &current);
        let next_idx = Self::compute_next_idx(current_idx, group_names.len(), wrap);
        Ok(next_idx.map(|i| group_names[i].clone()))
    }

    pub async fn next_group_on_output_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_group_names_on_output(output).await?;
        if group_names.is_empty() {
            return Ok(None);
        }
        let current_idx = group_names.iter().position(|g| g == &current);
        let next_idx = Self::compute_next_idx(current_idx, group_names.len(), wrap);
        Ok(next_idx.map(|i| group_names[i].clone()))
    }

    pub async fn prev_group_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_all_group_names().await?;
        if group_names.is_empty() {
            return Ok(None);
        }
        let current_idx = group_names.iter().position(|g| g == &current);
        let prev_idx = Self::compute_prev_idx(current_idx, group_names.len(), wrap);
        Ok(prev_idx.map(|i| group_names[i].clone()))
    }

    pub async fn prev_group_on_output_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or_else(|_| "0".to_string());
        let group_names = self.list_group_names_on_output(output).await?;
        if group_names.is_empty() {
            return Ok(None);
        }
        let current_idx = group_names.iter().position(|g| g == &current);
        let prev_idx = Self::compute_prev_idx(current_idx, group_names.len(), wrap);
        Ok(prev_idx.map(|i| group_names[i].clone()))
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

            if self.is_effectively_empty(&group.name).await? {
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

    /// Check if a group has no non-global workspaces.
    async fn is_effectively_empty(&self, group_name: &str) -> Result<bool> {
        let group = match GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
        {
            Some(g) => g,
            None => return Ok(true),
        };

        let memberships = WorkspaceGroupEntity::find_by_group(group.id)
            .all(self.db.conn())
            .await?;

        for membership in memberships {
            if let Some(ws) = WorkspaceEntity::find_by_id(membership.workspace_id)
                .one(self.db.conn())
                .await?
            {
                if !ws.is_global {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Check if all non-global workspaces of a group have been removed from sway.
    /// Used after switching away from a group to decide if it should be auto-deleted.
    async fn should_delete_old_group(&self, group_name: &str) -> Result<bool> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_non_empty: std::collections::HashSet<String> = sway_workspaces
            .iter()
            .filter(|w| w.representation.is_some())
            .map(|w| w.name.clone())
            .collect();

        let group = match GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
        {
            Some(g) => g,
            None => return Ok(true),
        };

        let memberships = WorkspaceGroupEntity::find_by_group(group.id)
            .all(self.db.conn())
            .await?;

        for membership in memberships {
            if let Some(ws) = WorkspaceEntity::find_by_id(membership.workspace_id)
                .one(self.db.conn())
                .await?
            {
                if !ws.is_global && sway_non_empty.contains(&ws.name) {
                    debug!("should_delete_old_group: workspace '{}' still exists and is non-empty in sway", ws.name);
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    async fn update_group_last_visited(&self, group_name: &str, output: &str) -> Result<()> {
        if let Some(group_model) = GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
        {
            let now = chrono::Utc::now().naive_utc();
            let mut active = group_model.into_active_model();
            active.last_visited = Set(Some(now));
            active.last_active_output = Set(Some(output.to_string()));
            active.update(self.db.conn()).await?;
        }
        Ok(())
    }

    async fn ensure_workspace_in_group(&self, ws_name: &str, group_name: &str, output: &str) -> Result<()> {
        let now = chrono::Utc::now().naive_utc();

        let ws = if let Some(existing) = WorkspaceEntity::find_by_name(ws_name)
            .one(self.db.conn())
            .await?
        {
            let ws_id = existing.id;
            let mut active = existing.into_active_model();
            active.output = Set(Some(output.to_string()));
            active.updated_at = Set(Some(now));
            active.update(self.db.conn()).await?;
            WorkspaceEntity::find_by_id(ws_id)
                .one(self.db.conn())
                .await?
                .unwrap()
        } else {
            let active = workspace::ActiveModel {
                name: Set(ws_name.to_string()),
                number: Set(None),
                output: Set(Some(output.to_string())),
                is_global: Set(false),
                created_at: Set(Some(now)),
                updated_at: Set(Some(now)),
                ..Default::default()
            };
            active.insert(self.db.conn()).await?
        };

        let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
            .all(self.db.conn())
            .await?;

        let mut already_in_group = false;
        for m in &memberships {
            if let Some(g) = GroupEntity::find_by_id(m.group_id)
                .one(self.db.conn())
                .await?
            {
                if g.name == group_name {
                    already_in_group = true;
                    break;
                }
            }
        }

        if !already_in_group {
            if let Some(group) = GroupEntity::find_by_name(group_name)
                .one(self.db.conn())
                .await?
            {
                let membership = workspace_group::ActiveModel {
                    workspace_id: Set(ws.id),
                    group_id: Set(group.id),
                    created_at: Set(Some(now)),
                    ..Default::default()
                };
                membership.insert(self.db.conn()).await?;
                info!("Added workspace '{}' to group '{}'", ws_name, group_name);
            }
        }

        Ok(())
    }
}
