//! sway-groups-core library.

pub mod error;
pub mod db;
pub mod sway;
pub mod services;

pub use error::{Error, Result};
pub use db::database::DatabaseManager;

/// Strip legacy suffixes (`_class_hidden`, `_class_global`) from a sway workspace name.
/// Used during transition period when sway workspaces may still carry old suffixes.
pub fn strip_legacy_suffix(name: &str) -> String {
    name.strip_suffix("_class_hidden")
        .or_else(|| name.strip_suffix("_class_global"))
        .map(String::from)
        .unwrap_or_else(|| name.to_string())
}
