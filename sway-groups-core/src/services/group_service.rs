//! Group management service.

use crate::db::entities::{group, output, GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::services::suffix_service::SuffixService;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, ModelTrait, Set};
use tracing::{info, warn};

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
}

impl GroupService {
    /// Create a new group service.
    pub fn new(db: DatabaseManager, suffix_service: SuffixService) -> Self {
        Self { db, suffix_service }
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

    /// Set the active group for an output.
    pub async fn set_active_group(&self, output: &str, group: &str) -> Result<()> {
        // Verify group exists
        if GroupEntity::find_by_name(group)
            .one(self.db.conn())
            .await?
            .is_none()
        {
            return Err(Error::GroupNotFound(group.to_string()));
        }

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

        info!("Set active group for {} to '{}'", output, group);

        // Sync suffixes for all outputs
        self.suffix_service.sync_all_suffixes().await?;

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
