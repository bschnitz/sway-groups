//! Database connection and management.

use sea_orm::{ConnectOptions, Database, DatabaseConnection, Schema, ConnectionTrait};
use std::path::PathBuf;
use anyhow::Result as AnyResult;

use crate::db::entities::{FocusHistoryEntity, GroupEntity, OutputEntity, WorkspaceEntity, WorkspaceGroupEntity};

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

        let backend = conn.get_database_backend();
        let schema = Schema::new(backend);

        let mut stmt_group = schema.create_table_from_entity(GroupEntity);
        stmt_group.if_not_exists();
        conn.execute(&stmt_group).await?;

        let mut stmt_workspace = schema.create_table_from_entity(WorkspaceEntity);
        stmt_workspace.if_not_exists();
        conn.execute(&stmt_workspace).await?;

        let mut stmt_wg = schema.create_table_from_entity(WorkspaceGroupEntity);
        stmt_wg.if_not_exists();
        conn.execute(&stmt_wg).await?;

        let mut stmt_output = schema.create_table_from_entity(OutputEntity);
        stmt_output.if_not_exists();
        conn.execute(&stmt_output).await?;

        let mut stmt_focus_history = schema.create_table_from_entity(FocusHistoryEntity);
        stmt_focus_history.if_not_exists();
        conn.execute(&stmt_focus_history).await?;

        Ok(Self { conn })
    }

    /// Get the database connection.
    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}
