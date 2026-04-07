//! Database entities for sway-groups.

pub mod group;
pub mod workspace;
pub mod workspace_group;
pub mod output;
pub mod focus_history;

// Re-export entities
pub use group::Entity as GroupEntity;
pub use workspace::Entity as WorkspaceEntity;
pub use workspace_group::Entity as WorkspaceGroupEntity;
pub use output::Entity as OutputEntity;
pub use focus_history::Entity as FocusHistoryEntity;
