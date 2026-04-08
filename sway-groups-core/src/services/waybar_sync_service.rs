//! Waybar sync service — sends workspace widget state to waybar-dynamic.

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::strip_legacy_suffix;
use crate::sway::waybar_client::{WidgetSpec, WaybarClient};
use crate::sway::SwayIpcClient;
use sea_orm::EntityTrait;
use tracing::info;

/// Service for synchronizing workspace state to waybar-dynamic.
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

    /// Update waybar-dynamic with the current workspace state for all outputs.
    pub async fn update_waybar(&self) -> Result<()> {
        let outputs = self.ipc_client.get_outputs()?;
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let focused_output = self.ipc_client.get_primary_output().ok();

        let mut widgets = Vec::new();

        for output in &outputs {
            let active_group = OutputEntity::find_by_name(&output.name)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or_else(|| "0".to_string());

            let is_output_focused = focused_output.as_deref() == Some(&output.name);

            for sway_ws in sway_workspaces.iter().filter(|w| w.output == output.name) {
                let base_name = strip_legacy_suffix(&sway_ws.name);

                if let Some(workspace) = WorkspaceEntity::find_by_name(&base_name)
                    .one(self.db.conn())
                    .await?
                {
                    let is_global = workspace.is_global;

                    if is_global {
                        let mut classes = vec!["global".to_string()];
                        if sway_ws.focused {
                            classes.push("focused".to_string());
                        }
                        widgets.push(self.make_widget(&base_name, &classes));
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

                    widgets.push(self.make_widget(&base_name, &classes));
                }
            }
        }

        widgets.sort_by(|a, b| {
            let a_num: Option<i64> = a.label.parse().ok();
            let b_num: Option<i64> = b.label.parse().ok();
            match (a_num, b_num) {
                (Some(an), Some(bn)) => an.cmp(&bn),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.label.cmp(&b.label),
            }
        });

        info!("waybar sync: sending {} widgets", widgets.len());
        self.waybar_client.send_set_all(widgets)?;

        Ok(())
    }

    fn make_widget(&self, name: &str, classes: &[String]) -> WidgetSpec {
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
