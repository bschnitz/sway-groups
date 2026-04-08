//! Suffix management service.

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::sway::SwayIpcClient;
use sea_orm::entity::prelude::*;
use tracing::info;

/// Service for managing workspace suffixes.
#[derive(Clone)]
pub struct SuffixService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
}

/// Suffix constants.
pub const SUFFIX_HIDDEN: &str = "_class_hidden";
pub const SUFFIX_GLOBAL: &str = "_class_global";

impl SuffixService {
    /// Create a new suffix service.
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self { db, ipc_client }
    }

    /// Calculate the appropriate suffix for a workspace.
    pub fn calculate_suffix(
        &self,
        _workspace_name: &str,
        active_group: &str,
        workspace_groups: &[String],
        is_global: bool,
    ) -> Option<&'static str> {
        // Global workspaces always get global suffix
        if is_global {
            return Some(SUFFIX_GLOBAL);
        }

        // If workspace is in the active group, no suffix
        if workspace_groups.iter().any(|g| g == active_group) {
            return None;
        }

        // If workspace is in any group but not active group, hide it
        if !workspace_groups.is_empty() {
            return Some(SUFFIX_HIDDEN);
        }

        // No group membership: visible only in group "0"
        if active_group == "0" {
            None
        } else {
            Some(SUFFIX_HIDDEN)
        }
    }

    /// Apply a suffix to a workspace name.
    pub fn apply_suffix(&self, workspace_name: &str, suffix: Option<&str>) -> String {
        // Remove any existing suffixes first
        let base_name = self.strip_suffix(workspace_name);

        match suffix {
            Some(s) => format!("{}{}", base_name, s),
            None => base_name,
        }
    }

    /// Strip any swayg suffixes from a workspace name.
    pub fn strip_suffix(&self, workspace_name: &str) -> String {
        let base = workspace_name
            .strip_suffix(SUFFIX_HIDDEN)
            .or_else(|| workspace_name.strip_suffix(SUFFIX_GLOBAL));

        base.map(String::from).unwrap_or_else(|| workspace_name.to_string())
    }

    /// Get the base workspace name without suffix.
    pub fn get_base_name(&self, workspace_name: &str) -> String {
        self.strip_suffix(workspace_name)
    }

    /// Check if a workspace should be hidden based on suffix.
    pub fn is_hidden(&self, workspace_name: &str) -> bool {
        workspace_name.ends_with(SUFFIX_HIDDEN)
    }

    /// Check if a workspace is global based on suffix.
    pub fn is_global(&self, workspace_name: &str) -> bool {
        workspace_name.ends_with(SUFFIX_GLOBAL)
    }

    /// Sync suffixes for all workspaces on an output.
    pub async fn sync_suffixes_for_output(&self, output_name: &str) -> Result<()> {
        let active_group = OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or_else(|| "0".to_string());
        info!("sync_suffixes: output={}, active_group='{}'", output_name, active_group);

        let sway_workspaces = self.ipc_client.get_workspaces()?;

        for sway_ws in sway_workspaces.iter().filter(|w| w.output == output_name) {
            let base_name = self.get_base_name(&sway_ws.name);

            if let Some(workspace) = WorkspaceEntity::find_by_name(&base_name)
                .one(self.db.conn())
                .await?
            {
                let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
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

                let target_suffix = self.calculate_suffix(
                    &base_name,
                    &active_group,
                    &group_names,
                    workspace.is_global,
                );

                let target_name = self.apply_suffix(&base_name, target_suffix);
                if target_name != sway_ws.name {
                    let target_exists = sway_workspaces.iter().any(|w| w.name == target_name);
                    if target_exists {
                        info!(
                            "sync_suffixes: SKIP '{}' -> '{}', target already exists",
                            sway_ws.name, target_name
                        );
                        continue;
                    }
                    info!("sync_suffixes: RENAME '{}' -> '{}'", sway_ws.name, target_name);
                    self.ipc_client.rename_workspace(&sway_ws.name, &target_name)?;
                }
            }
        }

        Ok(())
    }

    /// Sync all suffixes for all outputs.
    pub async fn sync_all_suffixes(&self) -> Result<()> {
        let outputs = self.ipc_client.get_outputs()?;

        for output in outputs {
            self.sync_suffixes_for_output(&output.name).await?;
        }

        info!("Synced all suffixes");
        Ok(())
    }
}
