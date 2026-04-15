//! Hidden workspace-per-group entity.
//!
//! Presence-based: a row exists iff the workspace is marked as hidden in that
//! specific group. The composite (workspace_id, group_id) is the primary key.
//! Independent from `workspace_groups` memberships — works for global
//! workspaces too (they have no membership but can still be hidden per group).

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "hidden_workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub group_id: i32,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_by_workspace(workspace_id: i32) -> Select<Self> {
        Self::find().filter(Column::WorkspaceId.eq(workspace_id))
    }

    pub fn find_by_group(group_id: i32) -> Select<Self> {
        Self::find().filter(Column::GroupId.eq(group_id))
    }

    pub fn find_entry(workspace_id: i32, group_id: i32) -> Select<Self> {
        Self::find()
            .filter(Column::WorkspaceId.eq(workspace_id))
            .filter(Column::GroupId.eq(group_id))
    }
}
