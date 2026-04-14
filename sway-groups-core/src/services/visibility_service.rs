use anyhow::Result;

use sea_orm::EntityTrait;

use crate::db::entities::{
    GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity,
};
use crate::sway::SwayIpcClient;
use crate::db::DatabaseManager;

pub struct VisibilityService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
    default_group: String,
}

impl VisibilityService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self {
            db,
            ipc_client,
            default_group: "0".to_string(),
        }
    }

    pub fn with_config(db: DatabaseManager, ipc_client: SwayIpcClient, config: &sway_groups_config::SwaygConfig) -> Self {
        Self {
            db,
            ipc_client,
            default_group: config.defaults.default_group.clone(),
        }
    }

    /// Returns all workspace names visible on an output's active group.
    ///
    /// A workspace is visible if:
    /// - It is global (`is_global = true`), OR
    /// - It belongs to the active group for this output, OR
    /// - No group is active and the workspace has no group memberships
    ///
    /// Results are sorted alphabetically.
    pub async fn get_visible(&self, output_name: &str) -> Result<Vec<String>> {
        let active_group = self.resolve_active_group(output_name).await?;

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let mut visible = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for sway_ws in sway_workspaces.iter().filter(|w| w.output == output_name) {
            let base_name = &sway_ws.name;

            if seen.contains(base_name) {
                continue;
            }

            if let Some(workspace) = WorkspaceEntity::find_by_name(base_name)
                .one(self.db.conn())
                .await?
            {
                if workspace.is_global {
                    visible.push(base_name.clone());
                    seen.insert(base_name.clone());
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
                        && active_group.as_deref() == Some(&group.name)
                    {
                        visible.push(base_name.clone());
                        found = true;
                        break;
                    }
                }

                if !found && memberships.is_empty() && active_group.is_none() {
                    visible.push(base_name.clone());
                    found = true;
                }

                if found {
                    seen.insert(base_name.clone());
                }
            }
        }

        visible.sort();
        Ok(visible)
    }

    /// Returns all visible workspaces across all outputs (for global navigation).
    /// Results are sorted and deduplicated.
    pub async fn get_visible_global(&self) -> Result<Vec<String>> {
        let outputs = self.ipc_client.get_outputs()?;
        let mut all = Vec::new();

        for output in outputs {
            let visible = self.get_visible(&output.name).await?;
            all.extend(visible);
        }

        all.sort();
        all.dedup();
        Ok(all)
    }

    /// Returns all workspaces visible on any output, filtered by a specific
    /// active group. Used for navigation that considers workspaces across
    /// all outputs but respects group membership.
    pub async fn get_visible_for_group(&self, _output_name: &str, active_group: &str) -> Result<Vec<String>> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let mut visible = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for sway_ws in sway_workspaces.iter() {
            if seen.contains(&sway_ws.name) {
                continue;
            }

            if let Some(workspace) = WorkspaceEntity::find_by_name(&sway_ws.name)
                .one(self.db.conn())
                .await?
            {
                if workspace.is_global {
                    visible.push(sway_ws.name.clone());
                    seen.insert(sway_ws.name.clone());
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
                        && group.name == active_group
                    {
                        visible.push(sway_ws.name.clone());
                        found = true;
                        break;
                    }
                }

                if !found && memberships.is_empty() && active_group == self.default_group {
                    visible.push(sway_ws.name.clone());
                }
            }
        }

        visible.sort();
        Ok(visible)
    }

    /// Resolves the active group for a given output.
    async fn resolve_active_group(&self, output_name: &str) -> Result<Option<String>> {
        Ok(OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or(None))
    }
}
