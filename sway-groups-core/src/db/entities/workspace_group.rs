//! Workspace-Group membership entity.

use sea_orm::entity::prelude::*;

/// Workspace-Group membership model.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspace_groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    #[sea_orm(index)]
    pub workspace_id: i32,

    #[sea_orm(index)]
    pub group_id: i32,

    #[sea_orm(nullable)]
    pub created_at: Option<DateTime>,
}

/// Active model for workspace-group membership.
impl ActiveModelBehavior for ActiveModel {}

/// Extension methods for workspace-group queries.
impl Entity {
    /// Find memberships by workspace ID.
    pub fn find_by_workspace(workspace_id: i32) -> Select<Self> {
        Self::find().filter(Column::WorkspaceId.eq(workspace_id))
    }

    /// Find memberships by group ID.
    pub fn find_by_group(group_id: i32) -> Select<Self> {
        Self::find().filter(Column::GroupId.eq(group_id))
    }

    /// Find membership by workspace and group.
    pub fn find_membership(workspace_id: i32, group_id: i32) -> Select<Self> {
        Self::find()
            .filter(Column::WorkspaceId.eq(workspace_id))
            .filter(Column::GroupId.eq(group_id))
    }
}
