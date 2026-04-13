//! Database entities for sway-groups.

pub mod focus_history;
pub mod group;
pub mod group_state;
pub mod output;
pub mod pending_workspace_event;
pub mod workspace;
pub mod workspace_group;

// Re-export entities
pub use focus_history::Entity as FocusHistoryEntity;
pub use group::Entity as GroupEntity;
pub use group_state::Entity as GroupStateEntity;
pub use output::Entity as OutputEntity;
pub use pending_workspace_event::Entity as PendingWorkspaceEventEntity;
pub use workspace::Entity as WorkspaceEntity;
pub use workspace_group::Entity as WorkspaceGroupEntity;
