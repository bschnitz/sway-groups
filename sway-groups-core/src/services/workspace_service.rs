//! Workspace management service.

use crate::db::entities::{group, output, workspace, workspace_group};
use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity, FocusHistoryEntity, GroupStateEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::sway::SwayIpcClient;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, ModelTrait, Set};
use tracing::info;

/// Workspace information for display.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub id: i32,
    pub name: String,
    pub number: Option<i32>,
    pub output: Option<String>,
    pub is_global: bool,
    pub groups: Vec<String>,
}

/// Service for workspace operations.
pub struct WorkspaceService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
}

impl WorkspaceService {
    /// Create a new workspace service.
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self { db, ipc_client }
    }

    /// List workspace names visible in the active group on an output.
    pub async fn list_visible_workspaces(&self, output_name: &str) -> Result<Vec<String>> {
        let active_group = OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or_else(|| "0".to_string());

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let mut visible = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for sway_ws in sway_workspaces.iter().filter(|w| w.output == output_name) {
            let base_name = sway_ws.name.clone();

            if seen.contains(&base_name) {
                continue;
            }

            if let Some(workspace) = WorkspaceEntity::find_by_name(&base_name)
                .one(self.db.conn())
                .await?
            {
                if workspace.is_global {
                    visible.push(base_name.clone());
                    seen.insert(base_name);
                    continue;
                }

                let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
                    .all(self.db.conn())
                    .await?;

                let mut found = false;
                for m in &memberships {
                    if let Some(group) = GroupEntity::find_by_id(m.group_id)
                        .one(self.db.conn())
                        .await?
                        && group.name == active_group {
                            visible.push(base_name.clone());
                            found = true;
                            break;
                        }
                }

                if !found && memberships.is_empty() && active_group == "0" {
                    visible.push(base_name.clone());
                    found = true;
                }

                if found {
                    seen.insert(base_name);
                }
            }
        }

        visible.sort();
        Ok(visible)
    }

    /// List all workspaces with their group memberships.
    pub async fn list_workspaces(
        &self,
        output_filter: Option<&str>,
        group_filter: Option<&str>,
    ) -> Result<Vec<WorkspaceInfo>> {
        let workspaces = WorkspaceEntity::find()
            .all(self.db.conn())
            .await?;

        let mut result = Vec::new();

        for ws in workspaces {
            // Filter by output if specified
            if let Some(output) = output_filter
                && ws.output.as_ref() != Some(&output.to_string()) {
                    continue;
                }

            // Get group memberships
            let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
                .all(self.db.conn())
                .await?;

            let mut group_names = Vec::new();
            for m in memberships {
                if let Some(group) = GroupEntity::find_by_id(m.group_id)
                    .one(self.db.conn())
                    .await?
                {
                    group_names.push(group.name);
                }
            }

            // Filter by group if specified
            if let Some(group_name) = group_filter
                && !group_names.iter().any(|g| g == group_name) {
                    continue;
                }

            result.push(WorkspaceInfo {
                id: ws.id,
                name: ws.name,
                number: ws.number,
                output: ws.output,
                is_global: ws.is_global,
                groups: group_names,
            });
        }

        Ok(result)
    }

    /// Get a workspace by name.
    pub async fn get_workspace(&self, name: &str) -> Result<Option<workspace::Model>> {
        Ok(WorkspaceEntity::find_by_name(name)
            .one(self.db.conn())
            .await?)
    }

    /// Ensure a workspace exists in DB, creating it in sway if necessary.
    async fn ensure_workspace(&self, workspace_name: &str) -> Result<workspace::Model> {
        if let Some(ws) = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
        {
            return Ok(ws);
        }

        // Not in DB — check sway
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_ws = sway_workspaces
            .iter()
            .find(|w| w.name == workspace_name || w.num.map(|n| n.to_string()) == Some(workspace_name.to_string()));

        match sway_ws {
            Some(ws) => {
                let number = ws.num.map(|n| n as i32);
                let now = chrono::Utc::now().naive_utc();

                let active = workspace::ActiveModel {
                    name: Set(ws.name.clone()),
                    number: Set(number),
                    output: Set(Some(ws.output.clone())),
                    is_global: Set(false),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                Ok(active.insert(self.db.conn()).await?)
            }
            None => {
                // Not in sway either — create it via swaymsg
                info!("Workspace '{}' not found in sway, creating it", workspace_name);
                self.ipc_client.run_command(&format!("workspace {}", workspace_name))?;

                let sway_workspaces = self.ipc_client.get_workspaces()?;
                let sway_ws = sway_workspaces
                    .iter()
                    .find(|w| w.name == workspace_name);

                match sway_ws {
                    Some(ws) => {
                        let number = ws.num.map(|n| n as i32);
                        let now = chrono::Utc::now().naive_utc();

                        let active = workspace::ActiveModel {
                            name: Set(ws.name.clone()),
                            number: Set(number),
                            output: Set(Some(ws.output.clone())),
                            is_global: Set(false),
                            created_at: Set(Some(now)),
                            updated_at: Set(Some(now)),
                            ..Default::default()
                        };
                        Ok(active.insert(self.db.conn()).await?)
                    }
                    None => {
                        Err(Error::WorkspaceNotFound(workspace_name.to_string()))
                    }
                }
            }
        }
    }

    /// Add a workspace to a group.
    pub async fn add_to_group(&self, workspace_name: &str, group_name: &str) -> Result<()> {
        let workspace = self.ensure_workspace(workspace_name).await?;

        // Get group
        let group = GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(group_name.to_string()))?;

        // Check if membership already exists
        let existing = WorkspaceGroupEntity::find_membership(workspace.id, group.id)
            .one(self.db.conn())
            .await?;

        if existing.is_some() {
            return Err(Error::InvalidArgs(format!(
                "Workspace '{}' is already in group '{}'",
                workspace_name, group_name
            )));
        }

        // Create membership
        let now = chrono::Utc::now().naive_utc();
        let membership = workspace_group::ActiveModel {
            workspace_id: Set(workspace.id),
            group_id: Set(group.id),
            created_at: Set(Some(now)),
            ..Default::default()
        };
        membership.insert(self.db.conn()).await?;

        info!(
            "Added workspace '{}' to group '{}'",
            workspace_name, group_name
        );
        Ok(())
    }

    /// Remove a workspace from a group.
    pub async fn remove_from_group(
        &self,
        workspace_name: &str,
        group_name: &str,
    ) -> Result<()> {
        let workspace = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(workspace_name.to_string()))?;

        let group = GroupEntity::find_by_name(group_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::GroupNotFound(group_name.to_string()))?;

        // Find and delete membership
        let membership = WorkspaceGroupEntity::find_membership(workspace.id, group.id)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| {
                Error::InvalidArgs(format!(
                    "Workspace '{}' is not in group '{}'",
                    workspace_name, group_name
                ))
            })?;

        membership.delete(self.db.conn()).await?;

        info!(
            "Removed workspace '{}' from group '{}'",
            workspace_name, group_name
        );
        Ok(())
    }

    /// Move a workspace to specific groups, removing it from all others.
    pub async fn move_to_groups(
        &self,
        workspace_name: &str,
        group_names: &[&str],
    ) -> Result<()> {
        let workspace = match WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
        {
            Some(ws) => ws,
            None => {
                let sway_workspaces = self.ipc_client.get_workspaces()?;
                let sway_ws = sway_workspaces
                    .iter()
                    .find(|w| w.name == workspace_name || w.num.map(|n| n.to_string()) == Some(workspace_name.to_string()));

                match sway_ws {
                    Some(ws) => {
                        let number = ws.num.map(|n| n as i32);
                        let now = chrono::Utc::now().naive_utc();

                        let active = workspace::ActiveModel {
                            name: Set(ws.name.clone()),
                            number: Set(number),
                            output: Set(Some(ws.output.clone())),
                            is_global: Set(false),
                            created_at: Set(Some(now)),
                            updated_at: Set(Some(now)),
                            ..Default::default()
                        };
                        active.insert(self.db.conn()).await?
                    }
                    None => {
                        return Err(Error::WorkspaceNotFound(workspace_name.to_string()));
                    }
                }
            }
        };

        let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
            .all(self.db.conn())
            .await?;

        for m in memberships {
            m.delete(self.db.conn()).await?;
        }

        for group_name in group_names {
            let group = match GroupEntity::find_by_name(*group_name)
                .one(self.db.conn())
                .await?
            {
                Some(g) => g,
                None => {
                    let now = chrono::Utc::now().naive_utc();
                    let active = group::ActiveModel {
                        name: Set(group_name.to_string()),
                        created_at: Set(Some(now)),
                        updated_at: Set(Some(now)),
                        ..Default::default()
                    };
                    let model = active.insert(self.db.conn()).await?;
                    info!("Auto-created group: {}", group_name);
                    model
                }
            };

            let now = chrono::Utc::now().naive_utc();
            let membership = workspace_group::ActiveModel {
                workspace_id: Set(workspace.id),
                group_id: Set(group.id),
                created_at: Set(Some(now)),
                ..Default::default()
            };
            membership.insert(self.db.conn()).await?;
        }

        info!(
            "Moved workspace '{}' to groups: {}",
            workspace_name,
            group_names.join(", ")
        );
        Ok(())
    }

    /// Get groups for a workspace.
    pub async fn get_groups_for_workspace(
        &self,
        workspace_name: &str,
    ) -> Result<Vec<String>> {
        let workspace = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(workspace_name.to_string()))?;

        let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
            .all(self.db.conn())
            .await?;

        let mut groups = Vec::new();
        for m in memberships {
            if let Some(group) = GroupEntity::find_by_id(m.group_id)
                .one(self.db.conn())
                .await?
            {
                groups.push(group.name);
            }
        }

        Ok(groups)
    }

    /// Set workspace global status.
    pub async fn set_global(&self, workspace_name: &str, global: bool) -> Result<()> {
        let workspace = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(workspace_name.to_string()))?;

        let ws_id = workspace.id;
        let mut active = workspace.into_active_model();
        active.is_global = Set(global);
        active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(self.db.conn()).await?;

        if global {
            let memberships = WorkspaceGroupEntity::find_by_workspace(ws_id)
                .all(self.db.conn())
                .await?;
            let count = memberships.len();
            for m in memberships {
                m.delete(self.db.conn()).await?;
            }
            if count > 0 {
                info!(
                    "Removed workspace '{}' from {} group(s) (now global)",
                    workspace_name,
                    count
                );
            }
        } else {
            let sway_workspaces = self.ipc_client.get_workspaces()?;
            let ws_output = sway_workspaces
                .iter()
                .find(|ws| ws.name == workspace_name)
                .map(|ws| ws.output.clone());

            if let Some(output_name) = ws_output {
                let active_group = OutputEntity::find_by_name(&output_name)
                    .one(self.db.conn())
                    .await?
                    .map(|o| o.active_group)
                    .unwrap_or_else(|| "0".to_string());

                let group = GroupEntity::find_by_name(&active_group)
                    .one(self.db.conn())
                    .await?;

                if let Some(group) = group {
                    let existing = WorkspaceGroupEntity::find_membership(ws_id, group.id)
                        .one(self.db.conn())
                        .await?;
                    if existing.is_none() {
                        let now = chrono::Utc::now().naive_utc();
                        let membership = workspace_group::ActiveModel {
                            workspace_id: Set(ws_id),
                            group_id: Set(group.id),
                            created_at: Set(Some(now)),
                            ..Default::default()
                        };
                        membership.insert(self.db.conn()).await?;
                        info!(
                            "Added global workspace '{}' back to group '{}'",
                            workspace_name, active_group
                        );
                    }
                }
            } else {
                info!(
                    "Workspace '{}' not found in sway, cannot reassign to group",
                    workspace_name
                );
            }
        }

        info!(
            "Set workspace '{}' global = {}",
            workspace_name, global
        );
        Ok(())
    }

    /// Rename a workspace. Returns whether it was a simple rename or a merge.
    pub async fn rename_workspace(&self, old_name: &str, new_name: &str) -> Result<bool> {
        let target_exists = WorkspaceEntity::find_by_name(new_name)
            .one(self.db.conn())
            .await?
            .is_some();

        if target_exists {
            self.merge_workspace(old_name, new_name).await?;
            let focus_cmd = format!("workspace \"{}\"", new_name);
            self.ipc_client.run_command(&focus_cmd)?;
            Ok(true)
        } else {
            self.simple_rename_workspace(old_name, new_name).await?;
            Ok(false)
        }
    }

    async fn simple_rename_workspace(&self, old_name: &str, new_name: &str) -> Result<()> {
        self.ipc_client.rename_workspace(old_name, new_name)?;

        let workspace = WorkspaceEntity::find_by_name(old_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(old_name.to_string()))?;

        let mut active = workspace.into_active_model();
        active.name = Set(new_name.to_string());
        active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(self.db.conn()).await?;

        info!("Renamed workspace '{}' to '{}'", old_name, new_name);
        Ok(())
    }

    async fn merge_workspace(&self, old_name: &str, new_name: &str) -> Result<()> {
        let old_ws = WorkspaceEntity::find_by_name(old_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(old_name.to_string()))?;

        let new_ws = WorkspaceEntity::find_by_name(new_name)
            .one(self.db.conn())
            .await?
            .ok_or_else(|| Error::WorkspaceNotFound(new_name.to_string()))?;

        let old_memberships = WorkspaceGroupEntity::find_by_workspace(old_ws.id)
            .all(self.db.conn())
            .await?;

        let new_memberships = WorkspaceGroupEntity::find_by_workspace(new_ws.id)
            .all(self.db.conn())
            .await?;

        let now = chrono::Utc::now().naive_utc();

        for old_m in &old_memberships {
            let already = new_memberships.iter().any(|nm| nm.group_id == old_m.group_id);
            if !already {
                let membership = workspace_group::ActiveModel {
                    workspace_id: Set(new_ws.id),
                    group_id: Set(old_m.group_id),
                    created_at: Set(Some(now)),
                    ..Default::default()
                };
                membership.insert(self.db.conn()).await?;
            }
        }

        let tree_payload = self.ipc_client.get_tree()?;
        let tree: serde_json::Value = serde_json::from_slice(&tree_payload)?;

        fn collect_containers(node: &serde_json::Value, target_ws: &str, ids: &mut Vec<i64>) {
            let node_type = node.get("type").and_then(|t| t.as_str());
            let node_name = node.get("name").and_then(|n| n.as_str());

            if node_type == Some("workspace") && node_name == Some(target_ws) {
                if let Some(nodes) = node.get("nodes").and_then(|n| n.as_array()) {
                    for child in nodes {
                        if child.get("type").and_then(|t| t.as_str()) == Some("con") {
                            if let Some(id) = child.get("id").and_then(|i| i.as_i64()) {
                                ids.push(id);
                            }
                        }
                    }
                }
                if let Some(nodes) = node.get("floating_nodes").and_then(|n| n.as_array()) {
                    for child in nodes {
                        if child.get("type").and_then(|t| t.as_str()) == Some("floating_con") {
                            if let Some(id) = child.get("id").and_then(|i| i.as_i64()) {
                                ids.push(id);
                            }
                        }
                    }
                }
                return;
            }

            if let Some(nodes) = node.get("nodes").and_then(|n| n.as_array()) {
                for child in nodes {
                    collect_containers(child, target_ws, ids);
                }
            }
            if let Some(nodes) = node.get("floating_nodes").and_then(|n| n.as_array()) {
                for child in nodes {
                    collect_containers(child, target_ws, ids);
                }
            }
        }

        let mut container_ids = Vec::new();
        collect_containers(&tree, old_name, &mut container_ids);

        for id in &container_ids {
            let command = format!("[con_id={}] move to workspace \"{}\"", id, new_name);
            info!("merge: moving con_id={} to workspace '{}'", id, new_name);
            self.ipc_client.run_command(&command)?;
        }

        for m in old_memberships {
            m.delete(self.db.conn()).await?;
        }

        if let Ok(histories) = FocusHistoryEntity::find_by_workspace_name(old_name)
            .all(self.db.conn())
            .await
        {
            for h in histories {
                h.delete(self.db.conn()).await.ok();
            }
        }

        if let Ok(states) = GroupStateEntity::find_by_last_focused_workspace(old_name)
            .all(self.db.conn())
            .await
        {
            for s in states {
                s.delete(self.db.conn()).await.ok();
            }
        }

        old_ws.delete(self.db.conn()).await?;

        info!("Merged workspace '{}' into '{}'", old_name, new_name);
        Ok(())
    }

    /// Sync workspaces from sway.
    pub async fn sync_from_sway(&self) -> Result<()> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_outputs = self.ipc_client.get_outputs()?;
        let now = chrono::Utc::now().naive_utc();

        // Sync outputs
        for sway_out in &sway_outputs {
            let existing = OutputEntity::find_by_name(&sway_out.name)
                .one(self.db.conn())
                .await?;

            if let Some(out) = existing {
                let mut active = out.into_active_model();
                active.updated_at = Set(Some(now));
                active.update(self.db.conn()).await?;
            } else {
                let active = output::ActiveModel {
                    name: Set(sway_out.name.clone()),
                    active_group: Set("0".to_string()),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                active.insert(self.db.conn()).await?;
                info!("Created output '{}' with default group '0'", sway_out.name);
            }
        }

        let sway_names: std::collections::HashSet<String> = sway_workspaces
            .iter()
            .map(|w| w.name.clone())
            .collect();

        for sway_ws in sway_workspaces {
            let base_name = sway_ws.name.clone();
            let existing = WorkspaceEntity::find_by_name(&base_name)
                .one(self.db.conn())
                .await?;

            if let Some(ws) = existing {
                let mut active = ws.into_active_model();
                active.number = Set(sway_ws.num.map(|n| n as i32));
                active.output = Set(Some(sway_ws.output));
                active.updated_at = Set(Some(now));
                active.update(self.db.conn()).await?;
            } else {
                let ws_output = sway_ws.output.clone();

                // Determine group: prefer output's active_group from DB
                let active_group = {
                    let mut group_name = "0".to_string();
                    if let Some(output) = OutputEntity::find_by_name(&ws_output)
                        .one(self.db.conn())
                        .await
                        .ok()
                        .flatten()
                    {
                        group_name = output.active_group;
                    }

                    group_name
                };

                let number = sway_ws.num.map(|n| n as i32);
                let active = workspace::ActiveModel {
                    name: Set(base_name.clone()),
                    number: Set(number),
                    output: Set(Some(ws_output)),
                    is_global: Set(false),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                let ws = active.insert(self.db.conn()).await?;

                if let Some(group) = GroupEntity::find_by_name(&active_group)
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
                }
            }
        }

        // Remove workspaces that no longer exist in sway
        let db_workspaces = WorkspaceEntity::find()
            .all(self.db.conn())
            .await?;

        for ws in &db_workspaces {
            if !sway_names.contains(&ws.name) {
                // Remove group memberships
                let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
                    .all(self.db.conn())
                    .await?;
                for m in memberships {
                    m.delete(self.db.conn()).await?;
                }
                // Remove focus history entries
                if let Ok(histories) = FocusHistoryEntity::find_by_workspace_name(&ws.name)
                    .all(self.db.conn())
                    .await
                {
                    for h in histories {
                        h.delete(self.db.conn()).await.ok();
                    }
                }
                // Remove group_state entries referencing this workspace
                if let Ok(states) = GroupStateEntity::find_by_last_focused_workspace(&ws.name)
                    .all(self.db.conn())
                    .await
                {
                    for s in states {
                        s.delete(self.db.conn()).await.ok();
                    }
                }
                // Remove the workspace itself
                ws.clone().delete(self.db.conn()).await?;
                info!("Removed workspace '{}' (no longer in sway)", ws.name);
            }
        }

        info!("Synced workspaces from sway");
        Ok(())
    }

    /// Repair the database by reconciling with sway's actual state.
    /// Returns (removed_workspaces, added_workspaces, removed_groups).
    pub async fn repair(
        &self,
        group_service: &crate::services::GroupService,
    ) -> Result<(usize, usize, usize)> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_outputs = self.ipc_client.get_outputs()?;
        let now = chrono::Utc::now().naive_utc();

        let sway_names: std::collections::HashSet<String> = sway_workspaces
            .iter()
            .map(|w| w.name.clone())
            .collect();

        let sway_output_names: std::collections::HashSet<String> = sway_outputs
            .iter()
            .map(|o| o.name.clone())
            .collect();

        let mut removed_ws = 0usize;
        let mut added_ws = 0usize;

        // --- Sync outputs ---
        let db_outputs = OutputEntity::find()
            .all(self.db.conn())
            .await?;

        for db_out in &db_outputs {
            if !sway_output_names.contains(&db_out.name) {
                db_out.clone().delete(self.db.conn()).await.ok();
                info!("repair: removed output '{}' from DB", db_out.name);
            }
        }

        for sway_out in &sway_outputs {
            let existing = OutputEntity::find_by_name(&sway_out.name)
                .one(self.db.conn())
                .await?;

            if let Some(out) = existing {
                let mut active = out.into_active_model();
                active.updated_at = Set(Some(now));
                active.update(self.db.conn()).await?;
            } else {
                let active = output::ActiveModel {
                    name: Set(sway_out.name.clone()),
                    active_group: Set("0".to_string()),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                active.insert(self.db.conn()).await?;
                info!("repair: created output '{}'", sway_out.name);
            }
        }

        // --- Remove workspaces from DB that are not in sway ---
        let db_workspaces = WorkspaceEntity::find()
            .all(self.db.conn())
            .await?;

        for ws in &db_workspaces {
            if !sway_names.contains(&ws.name) {
                let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
                    .all(self.db.conn())
                    .await?;
                for m in memberships {
                    m.delete(self.db.conn()).await.ok();
                }

                if let Ok(histories) = FocusHistoryEntity::find_by_workspace_name(&ws.name)
                    .all(self.db.conn())
                    .await
                {
                    for h in histories {
                        h.delete(self.db.conn()).await.ok();
                    }
                }

                ws.clone().delete(self.db.conn()).await?;
                info!("repair: removed workspace '{}' from DB (not in sway)", ws.name);
                removed_ws += 1;
            }
        }

        // --- Add sway workspaces to DB that are not yet tracked ---
        for sway_ws in &sway_workspaces {
            let existing = WorkspaceEntity::find_by_name(&sway_ws.name)
                .one(self.db.conn())
                .await?;

            if existing.is_none() {
                let active = workspace::ActiveModel {
                    name: Set(sway_ws.name.clone()),
                    number: Set(sway_ws.num.map(|n| n as i32)),
                    output: Set(Some(sway_ws.output.clone())),
                    is_global: Set(false),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                let ws = active.insert(self.db.conn()).await?;

                if let Some(group) = GroupEntity::find_by_name("0")
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
                }

                info!("repair: added workspace '{}' to group '0'", sway_ws.name);
                added_ws += 1;
            }
        }

        // --- Prune empty groups ---
        let removed_groups = group_service.prune_groups(&[]).await.unwrap_or(0);

        info!("repair: removed {} stale workspaces, added {} new workspaces, pruned {} empty groups",
              removed_ws, added_ws, removed_groups);

        Ok((removed_ws, added_ws, removed_groups))
    }

}
