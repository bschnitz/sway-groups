//! Workspace management service.

use crate::db::entities::{workspace, workspace_group};
use crate::db::entities::{GroupEntity, WorkspaceEntity, WorkspaceGroupEntity};
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

    /// Add a workspace to a group.
    pub async fn add_to_group(&self, workspace_name: &str, group_name: &str) -> Result<()> {
        // Get or create workspace
        let workspace = match WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?
        {
            Some(ws) => ws,
            None => {
                // Try to create from sway if it exists
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

        let mut active = workspace.into_active_model();
        active.is_global = Set(global);
        active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(self.db.conn()).await?;

        info!(
            "Set workspace '{}' global = {}",
            workspace_name, global
        );
        Ok(())
    }

    /// Sync workspaces from sway.
    pub async fn sync_from_sway(&self) -> Result<()> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let now = chrono::Utc::now().naive_utc();

        for sway_ws in sway_workspaces {
            let base_name = Self::strip_suffix(&sway_ws.name);
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
                let number = sway_ws.num.map(|n| n as i32);
                let active = workspace::ActiveModel {
                    name: Set(base_name.clone()),
                    number: Set(number),
                    output: Set(Some(sway_ws.output)),
                    is_global: Set(false),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    ..Default::default()
                };
                let ws = active.insert(self.db.conn()).await?;

                if let Some(group_0) = GroupEntity::find_by_name("0")
                    .one(self.db.conn())
                    .await?
                {
                    let membership = workspace_group::ActiveModel {
                        workspace_id: Set(ws.id),
                        group_id: Set(group_0.id),
                        created_at: Set(Some(now)),
                        ..Default::default()
                    };
                    membership.insert(self.db.conn()).await?;
                }
            }
        }

        info!("Synced workspaces from sway");
        Ok(())
    }

    /// Strip swayg suffixes from a workspace name.
    fn strip_suffix(name: &str) -> String {
        name.strip_suffix("_class_hidden")
            .or_else(|| name.strip_suffix("_class_global"))
            .map(String::from)
            .unwrap_or_else(|| name.to_string())
    }
}
