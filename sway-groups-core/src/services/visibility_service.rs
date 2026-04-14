use anyhow::Result;

use crate::db::entities::OutputEntity;
use crate::db::DatabaseManager;
use crate::sway::SwayIpcClient;

pub struct VisibilityService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
}

impl VisibilityService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self { db, ipc_client }
    }

    pub fn with_config(
        db: DatabaseManager,
        ipc_client: SwayIpcClient,
        _config: &sway_groups_config::SwaygConfig,
    ) -> Self {
        Self { db, ipc_client }
    }

    /// Returns all workspace names visible on an output's active group.
    ///
    /// A workspace is visible if:
    /// - It is global (`is_global = true`), OR
    /// - It belongs to the active group for this output, OR
    /// - No group is active and the workspace has no group memberships.
    ///
    /// Results are sorted alphabetically.
    pub async fn get_visible(&self, output_name: &str) -> Result<Vec<String>> {
        let active_group = self.resolve_active_group(output_name).await?;

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_names: Vec<String> = sway_workspaces
            .iter()
            .filter(|w| w.output == output_name)
            .map(|w| w.name.clone())
            .collect();

        Ok(
            crate::db::queries::compute_visible_workspaces(
                self.db.conn(),
                &sway_names,
                active_group.as_deref(),
            )
            .await?,
        )
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

    /// Returns all workspaces visible on any output under a specific active group.
    /// Used for navigation that considers workspaces across all outputs but
    /// respects group membership.
    pub async fn get_visible_for_group(
        &self,
        _output_name: &str,
        active_group: &str,
    ) -> Result<Vec<String>> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_names: Vec<String> = sway_workspaces.iter().map(|w| w.name.clone()).collect();

        Ok(
            crate::db::queries::compute_visible_workspaces(
                self.db.conn(),
                &sway_names,
                Some(active_group),
            )
            .await?,
        )
    }

    async fn resolve_active_group(&self, output_name: &str) -> Result<Option<String>> {
        Ok(OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or(None))
    }
}
