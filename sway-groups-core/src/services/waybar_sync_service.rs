use std::collections::HashSet;

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::sway::waybar_client::{WidgetSpec, WaybarClient, WaybarMessage};
use crate::sway::SwayIpcClient;
use sea_orm::EntityTrait;
use tracing::info;

use sway_groups_config::BarDisplay;

#[derive(Clone)]
pub struct WaybarSyncService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
    waybar_client: WaybarClient,
    groups_client: WaybarClient,
    workspaces_display: BarDisplay,
    groups_display: BarDisplay,
    workspaces_show_global: bool,
    groups_show_empty: bool,
}

impl WaybarSyncService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient, waybar_client: WaybarClient) -> Self {
        let groups_client = WaybarClient::new_groups();
        Self {
            db,
            ipc_client,
            waybar_client,
            groups_client,
            workspaces_display: BarDisplay::All,
            groups_display: BarDisplay::All,
            workspaces_show_global: true,
            groups_show_empty: true,
        }
    }

    pub fn with_config(db: DatabaseManager, ipc_client: SwayIpcClient, config: &sway_groups_config::SwaygConfig) -> Self {
        let waybar_client = WaybarClient::with_instance_name(&config.bar.workspaces.socket_instance);
        let groups_client = WaybarClient::with_instance_name(&config.bar.groups.socket_instance);
        Self {
            db,
            ipc_client,
            waybar_client,
            groups_client,
            workspaces_display: config.bar.workspaces.display,
            groups_display: config.bar.groups.display,
            workspaces_show_global: config.bar.workspaces.show_global,
            groups_show_empty: config.bar.groups.show_empty,
        }
    }

    pub async fn update_waybar(&self) -> Result<()> {
        self.update_waybar_inner(0, std::time::Duration::ZERO).await
    }

    pub async fn update_waybar_groups(&self) -> Result<()> {
        self.update_waybar_groups_inner(0, std::time::Duration::ZERO).await
    }

    pub async fn update_waybar_with_retry(&self, retries: u32, delay: std::time::Duration) -> Result<()> {
        self.update_waybar_inner(retries, delay).await
    }

    pub async fn update_waybar_groups_with_retry(&self, retries: u32, delay: std::time::Duration) -> Result<()> {
        self.update_waybar_groups_inner(retries, delay).await
    }

    async fn update_waybar_inner(&self, retries: u32, delay: std::time::Duration) -> Result<()> {
        if self.workspaces_display == BarDisplay::None {
            return Ok(());
        }

        let outputs = self.ipc_client.get_outputs()?;
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let focused_output = self.ipc_client.get_primary_output().ok();

        let mut widgets = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for output in &outputs {
            let active_group = OutputEntity::find_by_name(&output.name)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or(None);

            let is_output_focused = focused_output.as_deref() == Some(&output.name);

            for sway_ws in sway_workspaces.iter().filter(|w| w.output == output.name) {
                if seen.contains(&sway_ws.name) {
                    continue;
                }

                if let Some(workspace) = WorkspaceEntity::find_by_name(&sway_ws.name)
                    .one(self.db.conn())
                    .await?
                {
                    let is_global = workspace.is_global;

                    if is_global {
                        if !self.workspaces_show_global {
                            continue;
                        }
                        let mut classes = vec!["global".to_string()];
                        if sway_ws.focused {
                            classes.push("focused".to_string());
                        }
                        widgets.push(Self::make_widget(&sway_ws.name, &classes));
                        seen.insert(sway_ws.name.clone());
                        continue;
                    }

                    let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)
                        .all(self.db.conn())
                        .await?;

                    let mut in_active_group = false;
                    for m in &memberships {
                        if let Some(group) = GroupEntity::find_by_id(m.group_id)
                            .one(self.db.conn())
                            .await?
                            && active_group.as_deref() == Some(&group.name) {
                            in_active_group = true;
                            break;
                        }
                    }

                    let no_group_and_default = memberships.is_empty() && active_group.is_none();

                    if !in_active_group && !no_group_and_default {
                        continue;
                    }

                    let mut classes = Vec::new();
                    if sway_ws.focused {
                        classes.push("focused".to_string());
                    } else if sway_ws.visible && is_output_focused {
                        classes.push("visible".to_string());
                    }

                    widgets.push(Self::make_widget(&sway_ws.name, &classes));
                    seen.insert(sway_ws.name.clone());
                }
            }
        }

        widgets.sort_by(|a, b| a.label.cmp(&b.label));

        let widget_names: Vec<&str> = widgets.iter().map(|w| w.label.as_str()).collect();
        info!("waybar sync: sending {} widgets: {:?}", widgets.len(), widget_names);
        if retries > 0 {
            self.waybar_client.send_with_retry(&WaybarMessage::set_all(widgets), retries, delay)?;
        } else {
            self.waybar_client.send_set_all(widgets)?;
        }

        Ok(())
    }

    async fn update_waybar_groups_inner(&self, retries: u32, delay: std::time::Duration) -> Result<()> {
        if self.groups_display == BarDisplay::None {
            return Ok(());
        }

        let focused_output = self.ipc_client.get_primary_output().ok();

        let active_group = match &focused_output {
            Some(output) => OutputEntity::find_by_name(output)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or(None),
            None => None,
        };

        let groups = GroupEntity::find()
            .all(self.db.conn())
            .await?;

        let mut widgets = Vec::new();

        for group in &groups {
            if !self.groups_show_empty {
                let memberships = WorkspaceGroupEntity::find_by_group(group.id)
                    .all(self.db.conn())
                    .await?;
                let mut has_non_global = false;
                for m in &memberships {
                    if let Some(ws) = WorkspaceEntity::find_by_id(m.workspace_id)
                        .one(self.db.conn())
                        .await?
                    {
                        if !ws.is_global {
                            has_non_global = true;
                            break;
                        }
                    }
                }
                if !has_non_global {
                    continue;
                }
            }

            let is_active = active_group.as_deref() == Some(&group.name);

            match self.groups_display {
                BarDisplay::Active if !is_active => continue,
                _ => {}
            }

            let mut classes = Vec::new();
            if is_active {
                classes.push("active".to_string());
            }

            widgets.push(Self::make_group_widget(&group.name, &classes));
        }

        widgets.sort_by(|a, b| a.label.cmp(&b.label));

        let widget_names: Vec<&str> = widgets.iter().map(|w| w.label.as_str()).collect();
        info!("waybar groups sync: sending {} groups: {:?}", widgets.len(), widget_names);
        if retries > 0 {
            self.groups_client.send_with_retry(&WaybarMessage::set_all(widgets), retries, delay)?;
        } else {
            self.groups_client.send_set_all(widgets)?;
        }

        Ok(())
    }

    fn make_widget(name: &str, classes: &[String]) -> WidgetSpec {
        let label = name.to_string();
        let id = format!("ws-{}", name);
        let on_click = format!("swaymsg workspace \"{}\"", name);

        WidgetSpec {
            id,
            label,
            classes: classes.to_vec(),
            tooltip: None,
            on_click: Some(on_click),
            on_right_click: None,
            on_middle_click: None,
        }
    }

    fn make_group_widget(name: &str, classes: &[String]) -> WidgetSpec {
        let label = name.to_string();
        let id = format!("group-{}", name);
        let on_click = format!("swayg group select \"{}\"", name);
        let on_right_click = "swayg group prev-on-output -w".to_string();
        let on_middle_click = "swayg group next-on-output -w".to_string();

        WidgetSpec {
            id,
            label,
            classes: classes.to_vec(),
            tooltip: None,
            on_click: Some(on_click),
            on_right_click: Some(on_right_click),
            on_middle_click: Some(on_middle_click),
        }
    }
}
