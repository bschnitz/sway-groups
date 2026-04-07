//! Services module for sway-groups.

pub mod workspace_service;
pub mod group_service;
pub mod suffix_service;
pub mod navigation_service;

pub use workspace_service::WorkspaceService;
pub use group_service::GroupService;
pub use suffix_service::SuffixService;
pub use navigation_service::NavigationService;
