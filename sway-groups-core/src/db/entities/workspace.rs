//! Workspace entity for sway-groups.

use sea_orm::entity::prelude::*;

/// Workspace model representing a sway workspace.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    #[sea_orm(unique)]
    pub name: String,

    #[sea_orm(nullable)]
    pub number: Option<i32>,

    #[sea_orm(nullable)]
    pub output: Option<String>,

    #[sea_orm(default = false)]
    pub is_global: bool,

    #[sea_orm(nullable)]
    pub created_at: Option<DateTime>,

    #[sea_orm(nullable)]
    pub updated_at: Option<DateTime>,
}

/// Active model for workspace.
impl ActiveModelBehavior for ActiveModel {}

/// Extension methods for workspace queries.
impl Entity {
    /// Find workspace by number.
    pub fn find_by_number(number: i32) -> Select<Self> {
        Self::find().filter(Column::Number.eq(number))
    }

    /// Find all workspaces on an output.
    pub fn find_by_output(output: &str) -> Select<Self> {
        Self::find().filter(Column::Output.eq(output))
    }

    /// Find all global workspaces.
    pub fn find_global() -> Select<Self> {
        Self::find().filter(Column::IsGlobal.eq(true))
    }
}
