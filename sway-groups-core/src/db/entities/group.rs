//! Group entity for sway-groups.

use sea_orm::entity::prelude::*;

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
}
