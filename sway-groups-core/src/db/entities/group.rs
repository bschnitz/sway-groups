//! Group entity for sway-groups.

use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

/// Group model representing a named collection of workspaces.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    #[sea_orm(unique)]
    pub name: String,

    #[sea_orm(nullable)]
    pub created_at: Option<DateTime>,

    #[sea_orm(nullable)]
    pub updated_at: Option<DateTime>,

    #[sea_orm(nullable)]
    pub last_visited: Option<DateTime>,

    #[sea_orm(nullable)]
    pub last_active_output: Option<String>,
}

/// Active model for group.
impl ActiveModelBehavior for ActiveModel {}

/// Extension methods for group queries.
impl Entity {
    /// Find all groups ordered by name.
    pub fn find_all_ordered() -> Select<Self> {
        use sea_orm::QueryOrder;
        Self::find().order_by_asc(Column::Name)
    }

    /// Check if the default group "0" exists.
    pub async fn has_default_group(db: &DatabaseConnection) -> Result<bool, DbErr> {
        Self::find_by_name("0").one(db).await.map(|opt| opt.is_some())
    }
}
