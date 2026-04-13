//! Pending workspace events entity.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "pending_workspace_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub workspace_name: String,
    pub event_type: String,
    pub created_at: DateTime,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_stale(timeout: chrono::Duration) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        let cutoff = chrono::Utc::now().naive_utc() - timeout;
        Self::find()
            .filter(Column::CreatedAt.lt(cutoff))
    }

    pub fn find_by_name(name: &str) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        Self::find()
            .filter(Column::WorkspaceName.eq(name))
    }
}
