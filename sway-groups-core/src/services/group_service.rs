//! Group management service.

use crate::db::entities::{
    group, group_state, hidden_workspace, output, workspace_group, FocusHistoryEntity, GroupEntity,
    GroupStateEntity, HiddenWorkspaceEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity,
};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::sway::SwayIpcClient;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, Set,
    TransactionTrait,
};
use tracing::{debug, info, warn};

/// Group information for display.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    pub id: i32,
    pub name: String,
    pub workspaces: Vec<String>,
}

/// Service for group operations.
pub struct GroupService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
    default_group: String,
    default_workspace: String,
}

impl GroupService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self {
            db,
            ipc_client,
            default_group: "0".to_string(),
            default_workspace: "0".to_string(),
        }
    }

    pub fn with_config(
        db: DatabaseManager,
        ipc_client: SwayIpcClient,
        config: &sway_groups_config::SwaygConfig,
    ) -> Self {
        Self {
            db,
            ipc_client,
            default_group: config.defaults.default_group.clone(),
            default_workspace: config.defaults.default_workspace.clone(),
        }
    }

    pub fn default_group(&self) -> &str {
        &self.default_group
    }

    pub fn default_workspace(&self) -> &str {
        &self.default_workspace
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// List all groups with their workspaces. Uses 3 batch queries instead of N+1.
    pub async fn list_groups(&self, output_filter: Option<&str>) -> Result<Vec<GroupInfo>> {
        let groups = GroupEntity::find_all_ordered().all(self.db.conn()).await?;

        let group_ids: Vec<i32> = groups.iter().map(|g| g.id).collect();
        let memberships_by_group =
            crate::db::queries::load_memberships_by_group_ids(self.db.conn(), &group_ids).await?;

        let all_ws_ids: Vec<i32> = memberships_by_group
            .values()
            .flat_map(|ms| ms.iter().map(|m| m.workspace_id))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let workspaces =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), &all_ws_ids).await?;

        let mut result = Vec::new();
        for group in groups {
            let memberships = memberships_by_group
                .get(&group.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let mut workspace_names: Vec<String> = memberships
                .iter()
                .filter_map(|m| workspaces.get(&m.workspace_id))
                .filter(|ws| {
                    output_filter.is_none() || ws.output.as_deref() == output_filter
                })
                .map(|ws| ws.name.clone())
                .collect();
            workspace_names.sort();

            result.push(GroupInfo {
                id: group.id,
                name: group.name,
                workspaces: workspace_names,
            });
        }

        Ok(result)
    }

    /// List all group names alphabetically, without workspace details.
    pub async fn list_all_group_names(&self) -> Result<Vec<String>> {
        let groups = GroupEntity::find_all_ordered().all(self.db.conn()).await?;
        Ok(groups.into_iter().map(|g| g.name).collect())
    }

    /// List non-empty group names for a specific output, alphabetically.
    pub async fn list_group_names_on_output(&self, output: &str) -> Result<Vec<String>> {
        let groups = self.list_groups(Some(output)).await?;
        Ok(groups
            .into_iter()
            .filter(|g| !g.workspaces.is_empty())
            .map(|g| g.name)
            .collect())
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Create a new group. Returns an error if the name is empty or already exists.
    pub async fn create_group(&self, name: &str) -> Result<group::Model> {
        if name.trim().is_empty() {
            return Err(Error::InvalidArgs("Group name must not be empty".into()));
        }

        if GroupEntity::find_by_name(name)
            .one(self.db.conn())
            .await?
            .is_some()
        {
            return Err(Error::InvalidArgs(format!("Group '{}' already exists", name)));
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
        if let Some(g) = GroupEntity::find_by_name(name).one(self.db.conn()).await? {
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

    /// Delete a group. All updates are wrapped in a transaction.
    pub async fn delete_group(&self, name: &str, force: bool) -> Result<()> {
        let group = GroupEntity::find_by_name(name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(name.to_string()))?;

        let memberships = WorkspaceGroupEntity::find_by_group(group.id)
            .all(self.db.conn())
            .await?;

        if !memberships.is_empty() && !force {
            return Err(Error::InvalidArgs(format!(
                "Group '{}' has {} workspaces. Use --force to delete anyway.",
                name,
                memberships.len()
            )));
        }

        let ws_ids: Vec<i32> = memberships.iter().map(|m| m.workspace_id).collect();

        // Atomically delete memberships, hidden entries, and the group
        let txn = self.db.conn().begin().await?;
        for membership in memberships {
            membership.delete(&txn).await?;
        }
        HiddenWorkspaceEntity::delete_many()
            .filter(hidden_workspace::Column::GroupId.eq(group.id))
            .exec(&txn)
            .await?;
        group.delete(&txn).await?;
        txn.commit().await?;

        info!("Deleted group: {}", name);

        self.handle_orphaned_workspaces(&ws_ids).await?;
        Ok(())
    }

    /// Move workspaces that became orphaned after a group deletion to the default group,
    /// or remove them from the DB if they no longer exist in sway.
    async fn handle_orphaned_workspaces(&self, ws_ids: &[i32]) -> Result<()> {
        if ws_ids.is_empty() {
            return Ok(());
        }

        let sway_workspaces = match self.ipc_client.get_workspaces() {
            Ok(ws) => ws,
            Err(e) => {
                warn!(
                    "Could not fetch workspaces from sway: {}. Skipping orphan cleanup.",
                    e
                );
                return Ok(());
            }
        };
        let sway_names: std::collections::HashSet<String> =
            sway_workspaces.iter().map(|w| w.name.clone()).collect();

        let default_group = match GroupEntity::find_by_name(&self.default_group)
            .one(self.db.conn())
            .await?
        {
            Some(g) => g,
            None => {
                warn!(
                    "Default group '{}' not found, cannot reassign orphaned workspaces",
                    self.default_group
                );
                return Ok(());
            }
        };

        let ws_map =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), ws_ids).await?;
        let memberships_map =
            crate::db::queries::load_memberships_by_workspace_ids(self.db.conn(), ws_ids).await?;

        let now = chrono::Utc::now().naive_utc();

        for ws_id in ws_ids {
            let still_has_group = memberships_map
                .get(ws_id)
                .map(|v| !v.is_empty())
                .unwrap_or(false);

            if still_has_group {
                continue;
            }

            if let Some(ws) = ws_map.get(ws_id) {
                if sway_names.contains(&ws.name) {
                    let membership = workspace_group::ActiveModel {
                        workspace_id: Set(*ws_id),
                        group_id: Set(default_group.id),
                        created_at: Set(Some(now)),
                        ..Default::default()
                    };
                    membership.insert(self.db.conn()).await?;
                    info!(
                        "Moved orphaned workspace '{}' to group '{}'",
                        ws.name, self.default_group
                    );
                } else {
                    if let Ok(histories) =
                        FocusHistoryEntity::find_by_workspace_name(&ws.name)
                            .all(self.db.conn())
                            .await
                    {
                        for h in histories {
                            h.delete(self.db.conn()).await.ok();
                        }
                    }
                    info!("Removing orphaned workspace '{}'", ws.name);
                    ws.clone().delete(self.db.conn()).await?;
                }
            }
        }

        Ok(())
    }

    /// Rename a group. All updates (group, outputs, group_state) run in a transaction.
    pub async fn rename_group(&self, old_name: &str, new_name: &str) -> Result<()> {
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

        let group = GroupEntity::find_by_name(old_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(old_name.to_string()))?;

        let affected_outputs = OutputEntity::find_by_active_group(&Some(old_name.to_string()))
            .all(self.db.conn())
            .await?;
        let affected_states = GroupStateEntity::find_by_group_name(old_name)
            .all(self.db.conn())
            .await?;

        let txn = self.db.conn().begin().await?;
        let now = chrono::Utc::now().naive_utc();

        let mut active_group = group.into_active_model();
        active_group.name = Set(new_name.to_string());
        active_group.updated_at = Set(Some(now));
        active_group.update(&txn).await?;

        for output in affected_outputs {
            let mut active = output.into_active_model();
            active.active_group = Set(Some(new_name.to_string()));
            active.updated_at = Set(Some(now));
            active.update(&txn).await?;
        }

        for state in affected_states {
            let mut active = state.into_active_model();
            active.group_name = Set(new_name.to_string());
            active.update(&txn).await?;
        }

        txn.commit().await?;
        info!("Renamed group: {} -> {}", old_name, new_name);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Active group
    // -----------------------------------------------------------------------

    /// Get the active group for an output.
    pub async fn get_active_group(&self, output: &str) -> Result<Option<String>> {
        let output = OutputEntity::find_by_name(output)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::OutputNotFound(output.to_string()))?;

        Ok(output.active_group)
    }

    /// Set the active group for an output, update sway workspace focus, and
    /// persist group-visit state.
    pub async fn set_active_group(&self, output: &str, group: &str) -> Result<()> {
        if GroupEntity::find_by_name(group)
            .one(self.db.conn())
            .await?
            .is_none()
        {
            return Err(Error::GroupNotFound(group.to_string()));
        }

        let old_group = self.get_active_group(output).await.unwrap_or(None);
        if old_group.as_deref() != Some(group)
            && let Some(ref og) = old_group {
                self.save_current_workspace(output, og).await?;
            }
        debug!(
            "set_active_group: output={}, old_group={:?}, new_group='{}'",
            output, old_group, group
        );

        let old_group_needs_cleanup =
            old_group.is_some() && old_group.as_deref() != Some(group);

        self.upsert_output_active_group(output, group).await?;

        // Note: waybar sync is handled by the caller (commands.rs)

        // Switch to the target output first so workspace focus lands correctly
        self.focus_output(output)?;

        let group_workspaces = self.get_workspaces_for_group_on_output(group, output).await?;
        debug!(
            "set_active_group: workspaces in group '{}' on '{}': {:?}",
            group, output, group_workspaces
        );

        if group_workspaces.is_empty() {
            let dw = self.default_workspace.clone();
            // Case 1: Group has no workspaces → focus default workspace.
            // If default workspace already exists on a different output, move focus
            // away from it first so sway creates it on the target output instead.
            if let Ok(all_ws) = self.ipc_client.get_workspaces() {
                for ws in &all_ws {
                    if ws.name == dw && ws.output != output && !ws.output.is_empty() {
                        let _ = self
                            .ipc_client
                            .run_command(&format!("workspace \"{}\"", ws.output));
                        let _ = self.ipc_client.run_command("workspace back_and_forth");
                    }
                }
            }
            debug!(
                "set_active_group: case 1 (empty group), focusing workspace '{}'",
                dw
            );
            self.focus_workspace(&dw)?;
            self.ensure_workspace_in_group(&dw, group, output).await?;
        } else {
            let last_focused = self.get_last_focused_workspace(output, group).await?;
            debug!(
                "set_active_group: last_focused_workspace = {:?}",
                last_focused
            );

            if let Some(ref ws_name) = last_focused {
                // Case 3: Previously visited → restore last focused workspace
                if group_workspaces.iter().any(|w| w == ws_name) {
                    debug!("set_active_group: case 3 (revisit), focusing '{}'", ws_name);
                    self.focus_workspace(ws_name)?;
                } else {
                    debug!(
                        "set_active_group: case 3 fallback (workspace no longer in group), focusing '{}'",
                        group_workspaces[0]
                    );
                    self.focus_workspace(&group_workspaces[0])?;
                }
            } else {
                // Case 2: First visit → focus first workspace alphabetically
                debug!(
                    "set_active_group: case 2 (first visit), focusing '{}'",
                    group_workspaces[0]
                );
                self.focus_workspace(&group_workspaces[0])?;
            }
        }

        let focused = self
            .ipc_client
            .get_focused_workspace()
            .ok()
            .map(|ws| ws.name);
        debug!(
            "set_active_group: sway focused workspace after switch = {:?}",
            focused
        );

        self.save_current_workspace(output, group).await?;
        self.update_group_last_visited(group, output).await?;

        info!("Set active group for {} to '{}'", output, group);

        if old_group_needs_cleanup
            && let Some(ref old) = old_group {
                match self.should_delete_old_group(old).await {
                    Ok(true) => {
                        self.delete_group(old, true).await?;
                        info!(
                            "Auto-removed empty group '{}' after switch (no workspaces left in sway)",
                            old
                        );
                    }
                    Ok(false) => {
                        debug!(
                            "set_active_group: old group '{}' still has workspaces in sway, not deleting",
                            old
                        );
                    }
                    Err(e) => {
                        debug!(
                            "set_active_group: error checking old group '{}': {}",
                            old, e
                        );
                    }
                }
            }

        Ok(())
    }

    /// Update the active group in the DB only, without changing sway focus or
    /// triggering waybar updates. Used when a container is moved cross-group.
    pub async fn set_active_group_db_only(&self, output: &str, group: &str) -> Result<()> {
        let old_group = self.get_active_group(output).await.unwrap_or(None);
        if old_group.as_deref() != Some(group)
            && let Some(ref og) = old_group {
                self.save_current_workspace(output, og).await?;
            }

        self.upsert_output_active_group(output, group).await?;
        self.update_group_last_visited(group, output).await?;

        info!("Updated active group for {} to '{}' (db-only)", output, group);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Navigation: next / prev group
    // -----------------------------------------------------------------------

    /// Switch to the next group alphabetically (across all groups).
    pub async fn next_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let next_name = self.next_group_name(&output, wrap).await?;
        if let Some(ref name) = next_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(next_name)
    }

    /// Switch to the next non-empty group on a specific output.
    pub async fn next_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        if let Some(name) = self.navigate_group_name(&output, wrap, true, true).await? {
            self.set_active_group(&output, &name).await?;
            return Ok(Some(name));
        }
        Ok(None)
    }

    /// Switch to the previous group alphabetically (across all groups).
    pub async fn prev_group(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        let prev_name = self.prev_group_name(&output, wrap).await?;
        if let Some(ref name) = prev_name {
            self.set_active_group(&output, name).await?;
        }
        Ok(prev_name)
    }

    /// Switch to the previous non-empty group on a specific output.
    pub async fn prev_group_on_output(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let output = self.resolve_output(output).await?;
        if let Some(name) = self.navigate_group_name(&output, wrap, true, false).await? {
            self.set_active_group(&output, &name).await?;
            return Ok(Some(name));
        }
        Ok(None)
    }

    /// Return the name of the next group alphabetically (all groups, no switch).
    pub async fn next_group_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        self.navigate_group_name(output, wrap, false, true).await
    }

    /// Return the name of the next non-empty group on an output (no switch).
    pub async fn next_group_on_output_name(
        &self,
        output: &str,
        wrap: bool,
    ) -> Result<Option<String>> {
        self.navigate_group_name(output, wrap, true, true).await
    }

    /// Return the name of the previous group alphabetically (all groups, no switch).
    pub async fn prev_group_name(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        self.navigate_group_name(output, wrap, false, false).await
    }

    /// Return the name of the previous non-empty group on an output (no switch).
    pub async fn prev_group_on_output_name(
        &self,
        output: &str,
        wrap: bool,
    ) -> Result<Option<String>> {
        self.navigate_group_name(output, wrap, true, false).await
    }

    /// Internal helper for all next/prev group navigation.
    ///
    /// `output_only`: restrict to non-empty groups on this output.
    /// `forward`: true = next, false = prev.
    async fn navigate_group_name(
        &self,
        output: &str,
        wrap: bool,
        output_only: bool,
        forward: bool,
    ) -> Result<Option<String>> {
        let current = self.get_active_group(output).await.unwrap_or(None);
        let group_names = if output_only {
            self.list_group_names_on_output(output).await?
        } else {
            self.list_all_group_names().await?
        };

        if group_names.is_empty() {
            return Ok(None);
        }

        let current_idx = current
            .as_ref()
            .and_then(|c| group_names.iter().position(|g| g == c));

        let idx = if forward {
            Self::compute_next_idx(current_idx, group_names.len(), wrap)
        } else {
            Self::compute_prev_idx(current_idx, group_names.len(), wrap)
        };

        Ok(idx.map(|i| group_names[i].clone()))
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

    // -----------------------------------------------------------------------
    // Prune
    // -----------------------------------------------------------------------

    /// Remove empty groups (those with no non-global workspaces).
    pub async fn prune_groups(&self, keep: &[String]) -> Result<usize> {
        let groups = self.list_groups(None).await?;
        let mut removed = 0;

        for group in groups {
            if keep.iter().any(|k| k == &group.name) {
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

    // -----------------------------------------------------------------------
    // Cross-output resolution
    // -----------------------------------------------------------------------

    pub async fn find_last_visited_output(&self, group: &str) -> Result<Option<String>> {
        let group_model = GroupEntity::find_by_name(group)
            .one(self.db.conn())
            .await?;
        Ok(group_model.and_then(|g| g.last_active_output))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn resolve_output(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            Ok(self.ipc_client.get_primary_output()?)
        } else {
            Ok(output.to_string())
        }
    }

    /// Save the currently focused workspace as the last-focused for a group/output pair.
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

    async fn get_last_focused_workspace(
        &self,
        output: &str,
        group_name: &str,
    ) -> Result<Option<String>> {
        let state = GroupStateEntity::find_by_output_and_group(output, group_name)
            .one(self.db.conn())
            .await?;
        Ok(state.and_then(|s| s.last_focused_workspace))
    }

    fn focus_output(&self, output_name: &str) -> Result<()> {
        let command = format!("focus output \"{}\"", output_name);
        let results = self.ipc_client.run_command(&command)?;
        if let Some(result) = results.first()
            && result.success {
                return Ok(());
            }
        Ok(())
    }

    fn focus_workspace(&self, workspace_name: &str) -> Result<()> {
        let command = format!("workspace \"{}\"", workspace_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                info!("Focused workspace '{}'", workspace_name);
                return Ok(());
            } else {
                return Err(Error::SwayIpc(
                    result
                        .error
                        .clone()
                        .unwrap_or_else(|| "Unknown error".to_string()),
                ));
            }
        }
        Err(Error::SwayIpc("Empty response from sway".to_string()))
    }

    /// Get workspaces belonging to a group on a specific output, sorted.
    /// Uses a batch query for workspace loading.
    async fn get_workspaces_for_group_on_output(
        &self,
        group_name: &str,
        output: &str,
    ) -> Result<Vec<String>> {
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

        if memberships.is_empty() {
            return Ok(Vec::new());
        }

        let ws_ids: Vec<i32> = memberships.iter().map(|m| m.workspace_id).collect();
        let workspaces =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), &ws_ids).await?;

        let mut ws_names: Vec<String> = workspaces
            .values()
            .filter(|ws| ws.output.as_deref() == Some(output))
            .map(|ws| ws.name.clone())
            .collect();

        ws_names.sort();
        Ok(ws_names)
    }

    /// Upsert the active_group field for an output record.
    async fn upsert_output_active_group(&self, output: &str, group: &str) -> Result<()> {
        let now = chrono::Utc::now().naive_utc();

        if let Some(existing) = OutputEntity::find_by_name(output)
            .one(self.db.conn())
            .await?
        {
            let mut active = existing.into_active_model();
            active.active_group = Set(Some(group.to_string()));
            active.updated_at = Set(Some(now));
            active.update(self.db.conn()).await?;
        } else {
            let active = output::ActiveModel {
                name: Set(output.to_string()),
                active_group: Set(Some(group.to_string())),
                created_at: Set(Some(now)),
                updated_at: Set(Some(now)),
                ..Default::default()
            };
            active.insert(self.db.conn()).await?;
        }

        Ok(())
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

    /// Ensure a workspace exists in a group, creating both if necessary.
    async fn ensure_workspace_in_group(
        &self,
        ws_name: &str,
        group_name: &str,
        output: &str,
    ) -> Result<()> {
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
            use crate::db::entities::workspace;
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

        // Batch check: is workspace already in this group?
        let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
            .all(self.db.conn())
            .await?;
        let group_ids: Vec<i32> = memberships.iter().map(|m| m.group_id).collect();
        let group_name_map =
            crate::db::queries::load_group_names_by_ids(self.db.conn(), &group_ids).await?;
        let already_in_group = group_name_map.values().any(|n| n == group_name);

        if !already_in_group
            && let Some(group) = GroupEntity::find_by_name(group_name)
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

        if memberships.is_empty() {
            return Ok(true);
        }

        let ws_ids: Vec<i32> = memberships.iter().map(|m| m.workspace_id).collect();
        let workspaces =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), &ws_ids).await?;

        Ok(workspaces.values().all(|ws| ws.is_global))
    }

    /// Check if all non-global workspaces of a group have been removed from sway.
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

        if memberships.is_empty() {
            return Ok(true);
        }

        let ws_ids: Vec<i32> = memberships.iter().map(|m| m.workspace_id).collect();
        let workspaces =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), &ws_ids).await?;

        for ws in workspaces.values() {
            if !ws.is_global && sway_non_empty.contains(&ws.name) {
                debug!(
                    "should_delete_old_group: workspace '{}' still exists and is non-empty in sway",
                    ws.name
                );
                return Ok(false);
            }
        }

        Ok(true)
    }
}
