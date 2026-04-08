//! Group state entity for tracking last focused workspace per group per output.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "group_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub output: String,
    pub group_name: String,
    pub last_focused_workspace: Option<String>,
    pub last_visited: Option<DateTime>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_by_output_and_group(output: &str, group_name: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::Output.eq(output))
            .filter(Column::GroupName.eq(group_name))
    }

    pub fn find_by_group_name(group_name: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::GroupName.eq(group_name))
    }

    pub fn find_by_last_focused_workspace(workspace_name: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::LastFocusedWorkspace.eq(workspace_name))
    }
}
