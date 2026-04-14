//! Shared database query helpers.
//!
//! Provides batch-loading utilities and the canonical visibility predicate
//! to avoid N+1 query patterns and logic duplication across services.

use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::db::entities::{
    group, workspace, workspace_group, GroupEntity, WorkspaceEntity, WorkspaceGroupEntity,
};
use crate::error::Result;

// ---------------------------------------------------------------------------
// Batch loaders
// ---------------------------------------------------------------------------

/// Batch-load workspaces by name. Returns a map of `name → model`.
pub(crate) async fn load_workspaces_by_names(
    conn: &DatabaseConnection,
    names: &[String],
) -> Result<HashMap<String, workspace::Model>> {
    if names.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = WorkspaceEntity::find()
        .filter(workspace::Column::Name.is_in(names.iter().cloned()))
        .all(conn)
        .await?;
    Ok(rows.into_iter().map(|w| (w.name.clone(), w)).collect())
}

/// Batch-load workspaces by ID. Returns a map of `id → model`.
pub(crate) async fn load_workspaces_by_ids(
    conn: &DatabaseConnection,
    ws_ids: &[i32],
) -> Result<HashMap<i32, workspace::Model>> {
    if ws_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = WorkspaceEntity::find()
        .filter(workspace::Column::Id.is_in(ws_ids.iter().cloned()))
        .all(conn)
        .await?;
    Ok(rows.into_iter().map(|w| (w.id, w)).collect())
}

/// Batch-load workspace_group memberships for given workspace IDs.
/// Returns a map of `workspace_id → Vec<membership>`.
pub(crate) async fn load_memberships_by_workspace_ids(
    conn: &DatabaseConnection,
    ws_ids: &[i32],
) -> Result<HashMap<i32, Vec<workspace_group::Model>>> {
    if ws_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = WorkspaceGroupEntity::find()
        .filter(workspace_group::Column::WorkspaceId.is_in(ws_ids.iter().cloned()))
        .all(conn)
        .await?;
    let mut map: HashMap<i32, Vec<workspace_group::Model>> = HashMap::new();
    for m in rows {
        map.entry(m.workspace_id).or_default().push(m);
    }
    Ok(map)
}

/// Batch-load workspace_group memberships for given group IDs.
/// Returns a map of `group_id → Vec<membership>`.
pub(crate) async fn load_memberships_by_group_ids(
    conn: &DatabaseConnection,
    group_ids: &[i32],
) -> Result<HashMap<i32, Vec<workspace_group::Model>>> {
    if group_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = WorkspaceGroupEntity::find()
        .filter(workspace_group::Column::GroupId.is_in(group_ids.iter().cloned()))
        .all(conn)
        .await?;
    let mut map: HashMap<i32, Vec<workspace_group::Model>> = HashMap::new();
    for m in rows {
        map.entry(m.group_id).or_default().push(m);
    }
    Ok(map)
}

/// Batch-load group names by ID. Returns a map of `id → name`.
pub(crate) async fn load_group_names_by_ids(
    conn: &DatabaseConnection,
    group_ids: &[i32],
) -> Result<HashMap<i32, String>> {
    if group_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = GroupEntity::find()
        .filter(group::Column::Id.is_in(group_ids.iter().cloned()))
        .all(conn)
        .await?;
    Ok(rows.into_iter().map(|g| (g.id, g.name)).collect())
}

// ---------------------------------------------------------------------------
// Visibility predicate
// ---------------------------------------------------------------------------

/// Canonical visibility rule for a non-global workspace.
///
/// A workspace is visible when:
/// - `is_global` is true, OR
/// - one of `membership_group_names` matches `active_group`, OR
/// - it has no memberships and `active_group` is `None`.
pub(crate) fn is_visible(
    is_global: bool,
    membership_group_names: &[String],
    active_group: Option<&str>,
) -> bool {
    if is_global {
        return true;
    }
    for name in membership_group_names {
        if active_group == Some(name.as_str()) {
            return true;
        }
    }
    membership_group_names.is_empty() && active_group.is_none()
}

// ---------------------------------------------------------------------------
// Composite helper
// ---------------------------------------------------------------------------

/// Compute the visible workspace names from a list of sway workspace names.
///
/// Performs exactly 3 batch DB queries regardless of the number of workspaces.
/// The caller is responsible for pre-filtering `sway_names` to the relevant
/// output(s) if necessary.
pub(crate) async fn compute_visible_workspaces(
    conn: &DatabaseConnection,
    sway_names: &[String],
    active_group: Option<&str>,
) -> Result<Vec<String>> {
    let ws_map = load_workspaces_by_names(conn, sway_names).await?;

    let ws_ids: Vec<i32> = ws_map.values().map(|w| w.id).collect();
    let memberships_map = load_memberships_by_workspace_ids(conn, &ws_ids).await?;

    let group_ids: Vec<i32> = memberships_map
        .values()
        .flat_map(|ms| ms.iter().map(|m| m.group_id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let group_name_map = load_group_names_by_ids(conn, &group_ids).await?;

    let mut visible = Vec::new();
    let mut seen = HashSet::new();

    for name in sway_names {
        if seen.contains(name) {
            continue;
        }
        if let Some(ws) = ws_map.get(name) {
            let memberships = memberships_map.get(&ws.id).map(|v| v.as_slice()).unwrap_or(&[]);
            let membership_group_names: Vec<String> = memberships
                .iter()
                .filter_map(|m| group_name_map.get(&m.group_id).cloned())
                .collect();

            if is_visible(ws.is_global, &membership_group_names, active_group) {
                visible.push(name.clone());
                seen.insert(name.clone());
            }
        }
    }

    visible.sort();
    Ok(visible)
}
