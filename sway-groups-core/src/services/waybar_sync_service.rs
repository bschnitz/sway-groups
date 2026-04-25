use std::collections::HashSet;

use crate::db::entities::{GroupEntity, OutputEntity};
use crate::db::DatabaseManager;
use crate::error::Result;
use crate::sway::waybar_client::{WidgetSpec, WaybarClient, WaybarMessage};
use crate::sway::SwayIpcClient;
use sea_orm::EntityTrait;
use tracing::info;

use sway_groups_config::BarDisplay;

/// Resolve the absolute path to the `swayg` binary so click handlers work
/// regardless of waybar's PATH.
fn swayg_bin() -> String {
    // Try same directory as the currently running binary (swayg or swayg-daemon).
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.with_file_name("swayg");
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    // Fallback: $HOME/.cargo/bin/swayg
    if let Ok(home) = std::env::var("HOME") {
        let candidate = std::path::PathBuf::from(home).join(".cargo/bin/swayg");
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    "swayg".to_string()
}

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

    pub fn with_config(
        db: DatabaseManager,
        ipc_client: SwayIpcClient,
        config: &sway_groups_config::SwaygConfig,
    ) -> Self {
        let waybar_client =
            WaybarClient::with_instance_name(&config.bar.workspaces.socket_instance);
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
        self.update_waybar_groups_inner(0, std::time::Duration::ZERO)
            .await
    }

    pub async fn update_waybar_with_retry(
        &self,
        retries: u32,
        delay: std::time::Duration,
    ) -> Result<()> {
        self.update_waybar_inner(retries, delay).await
    }

    pub async fn update_waybar_groups_with_retry(
        &self,
        retries: u32,
        delay: std::time::Duration,
    ) -> Result<()> {
        self.update_waybar_groups_inner(retries, delay).await
    }

    async fn update_waybar_inner(&self, retries: u32, delay: std::time::Duration) -> Result<()> {
        if self.workspaces_display == BarDisplay::None {
            return Ok(());
        }

        let outputs = self.ipc_client.get_outputs()?;
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let focused_output = self.ipc_client.get_primary_output().ok();

        // Batch load all DB data up front.
        let all_names: Vec<String> = sway_workspaces.iter().map(|w| w.name.clone()).collect();
        let ws_map =
            crate::db::queries::load_workspaces_by_names(self.db.conn(), &all_names).await?;

        let ws_ids: Vec<i32> = ws_map.values().map(|w| w.id).collect();
        let memberships_map =
            crate::db::queries::load_memberships_by_workspace_ids(self.db.conn(), &ws_ids).await?;

        let group_ids: Vec<i32> = memberships_map
            .values()
            .flat_map(|ms| ms.iter().map(|m| m.group_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let group_name_map =
            crate::db::queries::load_group_names_by_ids(self.db.conn(), &group_ids).await?;

        // Hidden-workspaces state
        let show_hidden = crate::db::queries::get_bool_setting(
            self.db.conn(),
            crate::db::entities::setting::SHOW_HIDDEN_WORKSPACES,
            false,
        )
        .await?;
        let hidden_pairs = crate::db::queries::load_hidden_pairs(self.db.conn()).await?;

        // Name -> id map for all groups (needed to resolve active group names).
        let all_groups = GroupEntity::find().all(self.db.conn()).await?;
        let group_id_by_name: std::collections::HashMap<String, i32> =
            all_groups.iter().map(|g| (g.name.clone(), g.id)).collect();

        let mut widgets = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for output in &outputs {
            let active_group = OutputEntity::find_by_name(&output.name)
                .one(self.db.conn())
                .await?
                .map(|o| o.active_group)
                .unwrap_or(None);

            let active_group_id = active_group
                .as_deref()
                .and_then(|n| group_id_by_name.get(n).copied());

            let is_output_focused = focused_output.as_deref() == Some(&output.name);

            for sway_ws in sway_workspaces.iter().filter(|w| w.output == output.name) {
                if seen.contains(&sway_ws.name) {
                    continue;
                }

                if let Some(ws) = ws_map.get(&sway_ws.name) {
                    let is_hidden_here = match active_group_id {
                        Some(gid) => hidden_pairs.contains(&(ws.id, gid)),
                        None => false,
                    };

                    if is_hidden_here && !show_hidden {
                        continue;
                    }

                    if ws.is_global {
                        if !self.workspaces_show_global {
                            continue;
                        }
                        let mut classes = vec!["global".to_string()];
                        if sway_ws.focused {
                            classes.push("focused".to_string());
                        }
                        if sway_ws.urgent {
                            classes.push("urgent".to_string());
                        }
                        if is_hidden_here {
                            classes.push("hidden".to_string());
                        }
                        widgets.push(Self::make_widget(&sway_ws.name, &classes));
                        seen.insert(sway_ws.name.clone());
                        continue;
                    }

                    let memberships =
                        memberships_map.get(&ws.id).map(|v| v.as_slice()).unwrap_or(&[]);
                    let membership_group_names: Vec<String> = memberships
                        .iter()
                        .filter_map(|m| group_name_map.get(&m.group_id).cloned())
                        .collect();

                    if !crate::db::queries::is_visible(
                        false,
                        &membership_group_names,
                        active_group.as_deref(),
                        is_hidden_here,
                        show_hidden,
                    ) {
                        continue;
                    }

                    let mut classes = Vec::new();
                    if sway_ws.focused {
                        classes.push("focused".to_string());
                    } else if sway_ws.visible && is_output_focused {
                        classes.push("visible".to_string());
                    }
                    if sway_ws.urgent {
                        classes.push("urgent".to_string());
                    }
                    if is_hidden_here {
                        classes.push("hidden".to_string());
                    }

                    widgets.push(Self::make_widget(&sway_ws.name, &classes));
                    seen.insert(sway_ws.name.clone());
                }
            }
        }

        widgets.sort_by(|a, b| a.label.cmp(&b.label));

        let widget_names: Vec<&str> = widgets.iter().map(|w| w.label.as_str()).collect();
        info!(
            "waybar sync: sending {} widgets: {:?}",
            widgets.len(),
            widget_names
        );
        if retries > 0 {
            self.waybar_client
                .send_with_retry(&WaybarMessage::set_all(widgets), retries, delay)?;
        } else {
            self.waybar_client.send_set_all(widgets)?;
        }

        Ok(())
    }

    async fn update_waybar_groups_inner(
        &self,
        retries: u32,
        delay: std::time::Duration,
    ) -> Result<()> {
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

        let groups = GroupEntity::find().all(self.db.conn()).await?;

        // Load memberships for all groups (needed for empty-filtering and urgent detection)
        let group_ids: Vec<i32> = groups.iter().map(|g| g.id).collect();
        let memberships_map =
            crate::db::queries::load_memberships_by_group_ids(self.db.conn(), &group_ids)
                .await?;

        let all_ws_ids: Vec<i32> = memberships_map
            .values()
            .flat_map(|ms| ms.iter().map(|m| m.workspace_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let ws_map =
            crate::db::queries::load_workspaces_by_ids(self.db.conn(), &all_ws_ids).await?;

        // Build non-empty set if needed
        let non_empty_group_ids: Option<HashSet<i32>> = if !self.groups_show_empty {
            let ids = groups
                .iter()
                .filter(|g| {
                    memberships_map
                        .get(&g.id)
                        .map(|ms| ms.iter().any(|m| !ws_map.get(&m.workspace_id).map(|w| w.is_global).unwrap_or(true)))
                        .unwrap_or(false)
                })
                .map(|g| g.id)
                .collect::<HashSet<_>>();
            Some(ids)
        } else {
            None
        };

        // Build set of urgent workspace names from sway
        let sway_workspaces = self.ipc_client.get_workspaces()?;
        let urgent_ws_names: HashSet<String> = sway_workspaces
            .iter()
            .filter(|w| w.urgent)
            .map(|w| w.name.clone())
            .collect();

        let mut widgets = Vec::new();

        for group in &groups {
            if let Some(ref non_empty) = non_empty_group_ids
                && !non_empty.contains(&group.id) {
                    continue;
                }

            let is_active = active_group.as_deref() == Some(&group.name);

            match self.groups_display {
                BarDisplay::Active if !is_active => continue,
                _ => {}
            }

            // A group is urgent if any of its member workspaces is urgent in sway
            let is_urgent = memberships_map
                .get(&group.id)
                .map(|ms| {
                    ms.iter().any(|m| {
                        ws_map
                            .get(&m.workspace_id)
                            .map(|ws| urgent_ws_names.contains(&ws.name))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            let mut classes = Vec::new();
            if is_active {
                classes.push("active".to_string());
            }
            if is_urgent {
                classes.push("urgent".to_string());
            }

            widgets.push(Self::make_group_widget(&group.name, &classes));
        }

        widgets.sort_by(|a, b| a.label.cmp(&b.label));

        let widget_names: Vec<&str> = widgets.iter().map(|w| w.label.as_str()).collect();
        info!(
            "waybar groups sync: sending {} groups: {:?}",
            widgets.len(),
            widget_names
        );
        if retries > 0 {
            self.groups_client
                .send_with_retry(&WaybarMessage::set_all(widgets), retries, delay)?;
        } else {
            self.groups_client.send_set_all(widgets)?;
        }

        Ok(())
    }

    fn make_widget(name: &str, classes: &[String]) -> WidgetSpec {
        let label = name.to_string();
        let id = format!("ws-{}", name);
        let swayg = swayg_bin();
        let on_click = format!("{} nav go \"{}\"", swayg, name);

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
        let swayg = swayg_bin();
        let on_click = format!("{} group select \"{}\"", swayg, name);
        let on_right_click = format!("{} group prev-on-output -w", swayg);
        let on_middle_click = format!("{} group next-on-output -w", swayg);

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
