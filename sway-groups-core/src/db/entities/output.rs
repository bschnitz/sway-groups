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

    pub active_group: String,

    #[sea_orm(nullable)]
    pub created_at: Option<DateTime>,

    #[sea_orm(nullable)]
    pub updated_at: Option<DateTime>,
}

/// Active model for output.
impl ActiveModelBehavior for ActiveModel {}
