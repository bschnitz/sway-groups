//! Output entity for sway-groups.

use sea_orm::entity::prelude::*;

/// Output model representing a sway output/monitor.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "outputs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    #[sea_orm(unique)]
    pub name: String,

    #[sea_orm(nullable)]
    pub active_group: Option<String>,

    #[sea_orm(nullable)]
    pub created_at: Option<DateTime>,

    #[sea_orm(nullable)]
    pub updated_at: Option<DateTime>,
}

/// Active model for output.
impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_by_active_group(active_group: &Option<String>) -> Select<Self> {
        use sea_orm::{ColumnTrait, QueryFilter};
        match active_group {
            Some(group) => Self::find()
                .filter(Column::ActiveGroup.eq(group.clone())),
            None => Self::find()
                .filter(Column::ActiveGroup.is_null()),
        }
    }

    pub fn find_all_ordered() -> Select<Self> {
        use sea_orm::QueryOrder;
        Self::find().order_by_asc(Column::Name)
    }
}
