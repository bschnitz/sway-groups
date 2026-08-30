//! sway-groups-core library.

pub mod db;
pub mod error;
pub mod notification;
pub mod services;
pub mod sway;

pub use db::database::DatabaseManager;
pub use error::{Error, Result};
