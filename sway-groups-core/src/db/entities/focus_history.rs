//! Focus history entity for workspace navigation.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "focus_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub workspace_name: String,
    pub focused_at: DateTime,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_last_focused(excluding: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
        Self::find()
            .filter(Column::WorkspaceName.ne(excluding))
            .order_by_desc(Column::FocusedAt)
            .limit(1)
            .to_owned()
    }

    pub fn find_by_max_age(max_age: chrono::Duration) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        let cutoff = chrono::Utc::now().naive_utc() - max_age;
        Self::find()
            .filter(Column::FocusedAt.lt(cutoff))
    }

    pub fn find_by_workspace_name(workspace_name: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::WorkspaceName.eq(workspace_name))
    }
}
