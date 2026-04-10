//! Waybar sync service — sends workspace widget state to waybar-dynamic.

use std::collections::HashSet;

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::sway::waybar_client::{WidgetSpec, WaybarClient};
use crate::sway::SwayIpcClient;
use sea_orm::EntityTrait;
use tracing::info;

#[derive(Clone)]
pub struct WaybarSyncService {
    db: DatabaseManager,
    ipc_client: SwayIpcClient,
    waybar_client: WaybarClient,
}

impl WaybarSyncService {
    pub fn new(db: DatabaseManager, ipc_client: SwayIpcClient, waybar_client: WaybarClient) -> Self {
        Self { db, ipc_client, waybar_client }
    }

    pub async fn update_waybar(&self) -> Result<()> {
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
                .unwrap_or_else(|| "0".to_string());

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
                            && group.name == active_group {
                                in_active_group = true;
                                break;
                            }
                    }

                    let no_group_and_default = memberships.is_empty() && active_group == "0";

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
        self.waybar_client.send_set_all(widgets)?;

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
        }
    }
}
