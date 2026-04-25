//! sway-groups-core library.

pub mod error;
pub mod db;
pub mod notification;
pub mod sway;
pub mod services;

pub use error::{Error, Result};
pub use db::database::DatabaseManager;
