//! Database connection and management.

use sea_orm::{ConnectOptions, Database, DatabaseConnection, Schema, ConnectionTrait};
use std::path::PathBuf;
use anyhow::Result as AnyResult;

use crate::db::entities::{GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};

/// Database manager for sway-groups.
#[derive(Clone)]
pub struct DatabaseManager {
    conn: DatabaseConnection,
}

impl DatabaseManager {
    /// Create a new database manager with the given database path.
    pub async fn new(db_path: PathBuf) -> AnyResult<Self> {
        let url = format!("sqlite://{}?mode=rwc", db_path.display());

        let mut options = ConnectOptions::new(&url);
        options.sqlx_logging_level(tracing::log::LevelFilter::Debug);

        let conn = Database::connect(options).await?;

        // Initialize schema using SeaORM 2.0 approach
        let backend = conn.get_database_backend();
        let schema = Schema::new(backend);

        // Create tables using create_table_from_entity
        conn.execute(&schema.create_table_from_entity(GroupEntity)).await?;
        conn.execute(&schema.create_table_from_entity(WorkspaceEntity)).await?;
        conn.execute(&schema.create_table_from_entity(WorkspaceGroupEntity)).await?;
        conn.execute(&schema.create_table_from_entity(OutputEntity)).await?;

        Ok(Self { conn })
    }

    /// Get the database connection.
    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}
