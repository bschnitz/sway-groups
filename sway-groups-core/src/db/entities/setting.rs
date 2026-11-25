//! Generic key/value settings entity for DB-stored runtime flags.
//!
//! Used for global flags that need to be togglable at runtime (e.g.
//! `show_hidden_workspaces`). The `active_group` per-output state stays on
//! the `outputs` table.

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub value: String,
}

impl ActiveModelBehavior for ActiveModel {}

/// Key constant for the `show_hidden_workspaces` global flag.
pub const SHOW_HIDDEN_WORKSPACES: &str = "show_hidden_workspaces";

/// Key constant for the `bar_expanded` flag: while set, the workspaces bar
/// shows every workspace instead of folding them by the collapse rules. The
/// daemon sets it while the bar modifier is held.
pub const BAR_EXPANDED: &str = "bar_expanded";

/// Key of the workspace focused last among those of the collapse rule
/// `label`; `nav next`/`nav prev` go there when they stop on its fold.
pub fn collapse_last_key(label: &str) -> String {
    format!("collapse_last:{}", label)
}
