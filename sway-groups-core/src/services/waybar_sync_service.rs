//! Waybar sync service — sends workspace widget state to waybar-dynamic.

use std::collections::HashSet;

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::strip_legacy_suffix;
use crate::sway::{SwayIpcClient, SwayWorkspace};
use crate::sway::waybar_client::{WidgetSpec, WaybarClient};
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
        let mut seen_base_names: HashSet<String> = HashSet::new();

        for output in &outputs {
            let active_group = OutputEntity::find_by_name(&output.name)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or_else(|| "0".to_string());

            let is_output_focused = focused_output.as_deref() == Some(&output.name);

            // Collect sway workspaces for this output, preferring names without legacy suffixes
            let output_workspaces: Vec<_> = sway_workspaces.iter()
                .filter(|w| w.output == output.name)
                .collect();

            // Deduplicate by base name: if both "foo" and "foo_class_hidden" exist,
            // prefer the one without suffix (it's the "real" one)
            let mut preferred: Vec<&SwayWorkspace> = Vec::new();
            let mut base_to_ws: std::collections::HashMap<String, SwayWorkspace> = std::collections::HashMap::new();

            for sway_ws in &output_workspaces {
                let base_name = strip_legacy_suffix(&sway_ws.name);
                if sway_ws.name.contains("_class_") {
                    base_to_ws.entry(base_name.clone())
                        .or_insert_with(|| (*sway_ws).clone());
                } else {
                    base_to_ws.insert(base_name.clone(), (*sway_ws).clone());
                }
            }

            for (base_name, sway_ws) in &base_to_ws {
                if seen_base_names.contains(base_name) {
                    continue;
                }

                if let Some(workspace) = WorkspaceEntity::find_by_name(base_name)
                    .one(self.db.conn())
                    .await?
                {
                    let is_global = workspace.is_global;

                    if is_global {
                        let mut classes = vec!["global".to_string()];
                        if sway_ws.focused {
                            classes.push("focused".to_string());
                        }
                        widgets.push(Self::make_widget(base_name, &classes));
                        seen_base_names.insert(base_name.clone());
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

                    widgets.push(Self::make_widget(base_name, &classes));
                    seen_base_names.insert(base_name.clone());
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
