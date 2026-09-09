//! Database connection and management.

use anyhow::Result as AnyResult;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Schema};
use std::path::PathBuf;
use tracing::info;

use crate::db::entities::{
    FocusHistoryEntity, GroupEntity, GroupStateEntity, HiddenWorkspaceEntity, OutputEntity,
    PendingWorkspaceEventEntity, SettingEntity, WorkspaceEntity, WorkspaceGroupEntity,
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

        // A workspace is in a group or it is not -- twice means nothing. The
        // CLI and the daemon both file a workspace (the daemon from an
        // `[[assign]]` rule when sway creates it, the CLI when a jump finds the
        // membership missing), so a check-then-insert in either of them can be
        // overtaken by the other. Only the database can rule the duplicate out.
        //
        // Older databases predate the index and may still hold duplicates, so
        // the rows have to go before the index can be built. Keeping the oldest
        // of each set preserves the original `created_at`.
        conn.execute_unprepared(
            "DELETE FROM workspace_groups WHERE id NOT IN              (SELECT MIN(id) FROM workspace_groups GROUP BY workspace_id, group_id)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_groups_membership              ON workspace_groups (workspace_id, group_id)",
        )
        .await?;
        info!("Ensured unique membership index on 'workspace_groups'");

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

        let mut stmt = schema.create_table_from_entity(HiddenWorkspaceEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'hidden_workspaces' exists");

        let mut stmt = schema.create_table_from_entity(SettingEntity);
        stmt.if_not_exists();
        conn.execute(&stmt).await?;
        info!("Ensured table 'settings' exists");

        Ok(Self { conn })
    }

    /// Get the database connection.
    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}
