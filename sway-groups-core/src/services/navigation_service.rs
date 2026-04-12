//! Workspace navigation service.

use crate::db::entities::{focus_history, workspace, workspace_group, GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity, FocusHistoryEntity};
use crate::db::DatabaseManager;
use crate::error::{Error, Result};
use crate::sway::SwayIpcClient;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, ModelTrait, Set};
use tracing::info;

pub struct NavigationService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
}

impl NavigationService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient) -> Self {
        Self { db, ipc_client }
    }

    pub async fn get_visible_workspaces(&self, output_name: &str) -> Result<Vec<String>> {
        let active_group = OutputEntity::find_by_name(output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or_else(|| "0".to_string());

        info!("get_visible_workspaces: output={}, active_group={}", output_name, active_group);

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let mut visible = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for sway_ws in sway_workspaces.iter().filter(|w| w.output == output_name) {
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
                        && group.name == active_group {
                            visible.push(sway_ws.name.clone());
                            found = true;
                            break;
                        }
                }

                if !found && memberships.is_empty() && active_group == "0" {
                    visible.push(sway_ws.name.clone());
                    found = true;
                }

                if found {
                    seen.insert(sway_ws.name.clone());
                }
            }
        }

        visible.sort();
        Ok(visible)
    }

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

    pub async fn next_workspace(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces(output).await?;
        let current = self.ipc_client.get_focused_workspace()?;
        info!("nav next: output={}, active_group visible={:?}, current={}, wrap={}", output, visible, current.name, wrap);

        let next = find_next(&visible, &current.name, wrap);
        if let Some(ref target) = next {
            self.navigate_to_workspace(target).await?;
        } else {
            info!("nav next: no next workspace found");
        }
        Ok(next)
    }

    pub async fn next_workspace_global(&self, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces_global().await?;
        let current = self.ipc_client.get_focused_workspace()?;
        info!("nav next-on-output: visible={:?}, current={}, wrap={}", visible, current.name, wrap);

        let next = find_next(&visible, &current.name, wrap);
        if let Some(ref target) = next {
            self.navigate_to_workspace(target).await?;
        } else {
            info!("nav next-on-output: no next workspace found");
        }
        Ok(next)
    }

    pub async fn prev_workspace(&self, output: &str, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces(output).await?;
        let current = self.ipc_client.get_focused_workspace()?;
        info!("nav prev: output={}, visible={:?}, current={}, wrap={}", output, visible, current.name, wrap);

        let prev = find_prev(&visible, &current.name, wrap);
        if let Some(ref target) = prev {
            self.navigate_to_workspace(target).await?;
        } else {
            info!("nav prev: no prev workspace found");
        }
        Ok(prev)
    }

    pub async fn prev_workspace_global(&self, wrap: bool) -> Result<Option<String>> {
        let visible = self.get_visible_workspaces_global().await?;
        let current = self.ipc_client.get_focused_workspace()?;
        info!("nav prev-on-output: visible={:?}, current={}, wrap={}", visible, current.name, wrap);

        let prev = find_prev(&visible, &current.name, wrap);
        if let Some(ref target) = prev {
            self.navigate_to_workspace(target).await?;
        } else {
            info!("nav prev-on-output: no prev workspace found");
        }
        Ok(prev)
    }

    pub async fn go_workspace(&self, workspace: &str) -> Result<()> {
        info!("nav go: workspace={}", workspace);
        self.navigate_to_workspace(workspace).await
    }

    pub async fn focus_workspace(&self, workspace_name: &str) -> Result<()> {
        let sway_name = self.resolve_sway_workspace_name(workspace_name)?;
        let command = format!("workspace \"{}\"", sway_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                self.record_focus(&sway_name).await?;
                info!("Focused workspace '{}'", sway_name);
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

    pub async fn move_to_workspace(&self, workspace_name: &str) -> Result<()> {
        let command = format!("move container to workspace \"{}\"", workspace_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if !result.success {
                return Err(Error::SwayIpc(
                    result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                ));
            }
        } else {
            return Err(Error::SwayIpc("Empty response from sway".to_string()));
        }

        self.ensure_workspace_exists_in_db(workspace_name).await?;

        info!("Moved container to workspace '{}'", workspace_name);
        Ok(())
    }

    pub async fn go_back(&self) -> Result<Option<String>> {
        let current = self.ipc_client.get_focused_workspace()?;

        let last = FocusHistoryEntity::find_last_focused(&current.name)
            .one(self.db.conn())
            .await?;

        if let Some(entry) = last {
            self.navigate_to_workspace(&entry.workspace_name).await?;
            return Ok(Some(entry.workspace_name));
        }

        Ok(None)
    }

    pub async fn record_focus(&self, workspace_name: &str) -> Result<()> {
        let now = chrono::Utc::now().naive_utc();

        let active = focus_history::ActiveModel {
            workspace_name: Set(workspace_name.to_string()),
            focused_at: Set(now),
            ..Default::default()
        };
        active.insert(self.db.conn()).await?;

        self.prune_focus_history().await?;

        Ok(())
    }

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

    async fn navigate_to_workspace(&self, workspace_name: &str) -> Result<()> {
        let sway_name = self.resolve_sway_workspace_name(workspace_name)?;

        let command = format!("workspace \"{}\"", sway_name);
        let results = self.ipc_client.run_command(&command)?;

        if let Some(result) = results.first() {
            if result.success {
                self.record_focus(&sway_name).await?;

                self.upsert_workspace_in_db(&sway_name).await?;
                self.ensure_workspace_in_active_group(&sway_name).await?;

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

    async fn upsert_workspace_in_db(&self, workspace_name: &str) -> Result<()> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_ws = sway_workspaces.iter().find(|w| w.name == workspace_name);

        let now = chrono::Utc::now().naive_utc();

        let existing = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?;

        match existing {
            Some(ws) => {
                let mut active = ws.into_active_model();
                if let Some(sway_ws) = sway_ws {
                    active.output = Set(Some(sway_ws.output.clone()));
                }
                active.updated_at = Set(Some(now));
                active.update(self.db.conn()).await?;
            }
            None => {
                if let Some(sway_ws) = sway_ws {
                    let active = workspace::ActiveModel {
                        name: Set(sway_ws.name.clone()),
                        number: Set(sway_ws.num.map(|n| n as i32)),
                        output: Set(Some(sway_ws.output.clone())),
                        is_global: Set(false),
                        created_at: Set(Some(now)),
                        updated_at: Set(Some(now)),
                        ..Default::default()
                    };
                    active.insert(self.db.conn()).await?;
                    info!("Created workspace '{}' in DB (from navigate)", workspace_name);
                }
            }
        }
        Ok(())
    }

    async fn ensure_workspace_in_active_group(&self, workspace_name: &str) -> Result<()> {
        let ws = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?;

        let ws = match ws {
            Some(ws) => ws,
            None => return Ok(()),
        };

        let output_name = self.ipc_client.get_primary_output().unwrap_or_default();
        let active_group = OutputEntity::find_by_name(&output_name)
            .one(self.db.conn())
            .await?
            .map(|o| o.active_group)
            .unwrap_or_else(|| "0".to_string());

        let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
            .all(self.db.conn())
            .await?;

        let in_group = {
            let mut found = false;
            for m in &memberships {
                if let Some(group) = GroupEntity::find_by_id(m.group_id)
                    .one(self.db.conn())
                    .await?
                {
                    if group.name == active_group {
                        found = true;
                        break;
                    }
                }
            }
            found
        };

        if !in_group {
            if let Some(group) = GroupEntity::find_by_name(&active_group)
                .one(self.db.conn())
                .await?
            {
                let now = chrono::Utc::now().naive_utc();
                let membership = workspace_group::ActiveModel {
                    workspace_id: Set(ws.id),
                    group_id: Set(group.id),
                    created_at: Set(Some(now)),
                    ..Default::default()
                };
                membership.insert(self.db.conn()).await?;
                info!("Added workspace '{}' to active group '{}'", workspace_name, active_group);
            }
        }
        Ok(())
    }

    async fn ensure_workspace_exists_in_db(&self, workspace_name: &str) -> Result<()> {
        let existing = WorkspaceEntity::find_by_name(workspace_name)
            .one(self.db.conn())
            .await?;

        if existing.is_some() {
            return Ok(());
        }

        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let sway_ws = sway_workspaces.iter().find(|w| w.name == workspace_name);

        let now = chrono::Utc::now().naive_utc();

        if let Some(sway_ws) = sway_ws {
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
            info!("Created workspace '{}' in DB (from container move)", workspace_name);

            let output_name = self.ipc_client.get_primary_output().unwrap_or_default();
            let active_group = OutputEntity::find_by_name(&output_name)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or_else(|| "0".to_string());

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
                info!("Added workspace '{}' to active group '{}'", workspace_name, active_group);
            }
        }

        Ok(())
    }

    fn resolve_sway_workspace_name(&self, workspace_name: &str) -> Result<String> {
        let sway_workspaces = self.ipc_client.get_workspaces()?;

        for ws in &sway_workspaces {
            if ws.name == workspace_name {
                return Ok(ws.name.clone());
            }
        }

        Ok(workspace_name.to_string())
    }
}

fn find_next(items: &[String], current: &str, wrap: bool) -> Option<String> {
    let idx = items.iter().position(|i| i == current);

    match idx {
        Some(i) if i + 1 < items.len() => Some(items[i + 1].clone()),
        Some(_) if wrap => items.first().cloned(),
        _ => None,
    }
}

fn find_prev(items: &[String], current: &str, wrap: bool) -> Option<String> {
    let idx = items.iter().position(|i| i == current);

    match idx {
        Some(i) if i > 0 => Some(items[i - 1].clone()),
        Some(_) if wrap => items.last().cloned(),
        _ => None,
    }
}
