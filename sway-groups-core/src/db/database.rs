//! Database connection and management.

use sea_orm::{ConnectOptions, Database, DatabaseConnection, Schema, ConnectionTrait};
use std::path::PathBuf;
use anyhow::Result as AnyResult;
use tracing::info;

use crate::db::entities::{
    FocusHistoryEntity, GroupEntity, GroupStateEntity,
    OutputEntity, PendingWorkspaceEventEntity,
    WorkspaceEntity, WorkspaceGroupEntity,
};

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

        // Enable WAL mode before schema creation for better concurrent read/write performance
        conn.execute_unprepared("PRAGMA journal_mode=WAL").await?;

        let backend = conn.get_database_backend();
        let schema = Schema::new(backend);

        let mut stmt = schema.create_table_from_entity(GroupEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'groups' exists");

        let mut stmt = schema.create_table_from_entity(WorkspaceEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'workspaces' exists");

        let mut stmt = schema.create_table_from_entity(WorkspaceGroupEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'workspace_groups' exists");

        let mut stmt = schema.create_table_from_entity(OutputEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'outputs' exists");

        let mut stmt = schema.create_table_from_entity(FocusHistoryEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'focus_history' exists");

        let mut stmt = schema.create_table_from_entity(GroupStateEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'group_state' exists");

        let mut stmt = schema.create_table_from_entity(PendingWorkspaceEventEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'pending_workspace_events' exists");

        Ok(Self { conn })
    }

    /// Get the database connection.
    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}
