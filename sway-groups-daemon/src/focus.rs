//! Following focus changes that did not go through swayg.
//!
//! A `swaymsg workspace …`, a `[app_id=…] focus`, a click on a notification:
//! sway moves the focus and the DB never hears of it. The workspaces bar then
//! still lists the old group, and the group bar still marks it active. This
//! module makes the DB follow: when the focused workspace is not part of its
//! output's active group, the group it was visited in last becomes active.
//!
//! swayg's own commands move the focus too, and they write the DB around the
//! sway command — some before it, some after. So a focus event is not acted on
//! at once: the follower waits until the focus has been still for
//! [`SETTLE`], then asks sway where the focus is *now* and compares that with
//! the DB. A command that finished in that time has already left the DB
//! consistent, and the follower only redraws. It never issues a sway command,
//! so nothing it does produces another focus event.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use sway_groups_config::SwaygConfig;
use sway_groups_core::db::DatabaseManager;
use sway_groups_core::db::entities::{
    GroupEntity, GroupStateEntity, OutputEntity, PendingWorkspaceEventEntity, WorkspaceEntity,
    WorkspaceGroupEntity, group_state,
};
use sway_groups_core::services::{GroupService, WaybarSyncService};
use sway_groups_core::sway::SwayIpcClient;
use tracing::{error, info, warn};

/// How long the focus has to stay put before the follower looks at it.
const SETTLE: Duration = Duration::from_millis(150);

/// Pending events older than this are leftovers of a command that died, not a
/// command still running (the same limit the workspace handler applies).
const PENDING_TIMEOUT_SECS: i64 = 5;

#[derive(Clone)]
pub struct FocusFollower {
    db_path: PathBuf,
    ipc: SwayIpcClient,
    config: Arc<SwaygConfig>,
    /// Bumped by every focus event; a settled check that finds it moved on
    /// leaves the work to the check that belongs to the newer event.
    generation: Arc<AtomicU64>,
    /// The workspace the focus left at the start of the current burst of focus
    /// changes: where the user was in the group that is about to be left.
    left: Arc<Mutex<Option<String>>>,
}

impl FocusFollower {
    pub fn new(db_path: PathBuf, ipc: SwayIpcClient, config: Arc<SwaygConfig>) -> Self {
        Self {
            db_path,
            ipc,
            config,
            generation: Arc::new(AtomicU64::new(0)),
            left: Arc::new(Mutex::new(None)),
        }
    }

    /// Note a `focus` workspace event; `old` is the workspace sway reports the
    /// focus left.
    pub fn focused(&self, old: Option<String>) {
        {
            let mut left = self.left.lock().unwrap();
            if left.is_none() {
                *left = old;
            }
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let follower = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(SETTLE).await;
            if follower.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let left = follower.left.lock().unwrap().take();
            follower.follow(left).await;
        });
    }

    async fn follow(&self, left: Option<String>) {
        let focused = match self.ipc.get_focused_workspace() {
            Ok(ws) => ws,
            Err(e) => {
                warn!(
                    "Focus changed, but sway reports no focused workspace: {}",
                    e
                );
                return;
            }
        };

        let db = match DatabaseManager::new(self.db_path.clone()).await {
            Ok(db) => db,
            Err(e) => {
                error!("Failed to open DB '{}': {}", self.db_path.display(), e);
                return;
            }
        };

        // A swayg command is still at work and will redraw when it is done.
        let cutoff =
            chrono::Utc::now().naive_utc() - chrono::Duration::seconds(PENDING_TIMEOUT_SECS);
        let running = PendingWorkspaceEventEntity::find()
            .all(db.conn())
            .await
            .unwrap_or_default()
            .iter()
            .any(|p| p.created_at >= cutoff);
        if running {
            return;
        }

        if let Some(group) = self
            .group_to_follow(&db, &focused.name, &focused.output)
            .await
        {
            let old_group = OutputEntity::find_by_name(&focused.output)
                .one(db.conn())
                .await
                .unwrap_or(None)
                .and_then(|o| o.active_group);
            let groups = GroupService::with_config(db.clone(), self.ipc.clone(), &self.config);
            if let Err(e) = groups
                .set_active_group_db_only(&focused.output, &group)
                .await
            {
                warn!("Failed to follow focus into group '{}': {}", group, e);
                return;
            }
            // `set_active_group_db_only` records the focused workspace as the
            // one the old group was left on — right for a container moved out
            // of a group, wrong here: the focus is already elsewhere. Put back
            // the workspace the user actually left, if it belongs there.
            if let (Some(old_group), Some(left)) = (old_group, left) {
                remember_left(&db, &focused.output, &old_group, &left).await;
            }
            info!(
                "Focus moved to '{}' outside swayg, followed into group '{}'",
                focused.name, group
            );
        }

        let waybar = WaybarSyncService::with_config(db.clone(), self.ipc.clone(), &self.config);
        if let Err(e) = waybar.update_waybar().await {
            warn!("Failed to update waybar workspaces: {}", e);
        }
        if let Err(e) = waybar.update_waybar_groups().await {
            warn!("Failed to update waybar groups: {}", e);
        }
    }

    /// The group the DB has to switch to for `ws_name` to be in view, or `None`
    /// when it already is (or when there is nothing to switch to).
    async fn group_to_follow(
        &self,
        db: &DatabaseManager,
        ws_name: &str,
        output: &str,
    ) -> Option<String> {
        let ws = WorkspaceEntity::find_by_name(ws_name)
            .one(db.conn())
            .await
            .ok()??;
        if ws.is_global {
            return None;
        }

        let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
            .all(db.conn())
            .await
            .ok()?;
        let mut groups = Vec::new();
        for m in memberships {
            if let Ok(Some(g)) = GroupEntity::find_by_id(m.group_id).one(db.conn()).await {
                groups.push(g);
            }
        }

        let active = OutputEntity::find_by_name(output)
            .one(db.conn())
            .await
            .ok()
            .flatten()
            .and_then(|o| o.active_group);
        if groups.iter().any(|g| Some(&g.name) == active.as_ref()) {
            return None;
        }

        // The group the user was in last; never visited counts as oldest, and
        // the name breaks a tie so the choice does not depend on row order.
        groups
            .into_iter()
            .max_by(|a, b| {
                a.last_visited
                    .cmp(&b.last_visited)
                    .then_with(|| b.name.cmp(&a.name))
            })
            .map(|g| g.name)
    }
}

/// Record `ws_name` as the workspace `group` was left on, if it is a member of
/// that group.
async fn remember_left(db: &DatabaseManager, output: &str, group: &str, ws_name: &str) {
    let Ok(Some(ws)) = WorkspaceEntity::find_by_name(ws_name).one(db.conn()).await else {
        return;
    };
    let Ok(Some(g)) = GroupEntity::find_by_name(group).one(db.conn()).await else {
        return;
    };
    let member = WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(db.conn())
        .await
        .unwrap_or_default()
        .iter()
        .any(|m| m.group_id == g.id);
    if !member {
        return;
    }
    let Ok(Some(state)) = GroupStateEntity::find_by_output_and_group(output, group)
        .one(db.conn())
        .await
    else {
        return;
    };
    let mut active: group_state::ActiveModel = state.into_active_model();
    active.last_focused_workspace = Set(Some(ws_name.to_string()));
    if let Err(e) = active.update(db.conn()).await {
        warn!(
            "Failed to remember '{}' for group '{}': {}",
            ws_name, group, e
        );
    }
}
