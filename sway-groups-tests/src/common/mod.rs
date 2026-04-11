//! Common test infrastructure for sway-groups integration tests.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use sea_orm::EntityTrait;
use sway_groups_core::db::DatabaseManager;
use sway_groups_core::db::entities::{GroupEntity, WorkspaceEntity, WorkspaceGroupEntity};
use sway_groups_core::services::{GroupService, WaybarSyncService, WorkspaceService};
use sway_groups_core::sway::{SwayIpcClient, WaybarClient};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Isolated test database — never touches the production DB.
pub const TEST_DB_PATH: &str = "/tmp/swayg-integration-test.db";

/// Prefix for all test-created groups and workspaces.
pub const TEST_PREFIX: &str = "zz_test_";

// ---------------------------------------------------------------------------
// SwayTestFixture
// ---------------------------------------------------------------------------

/// RAII guard that sets up and tears down a single integration test.
pub struct SwayTestFixture {
    pub ipc: SwayIpcClient,
    pub db: DatabaseManager,
    pub db_path: PathBuf,
    pub orig_workspace: String,
    pub orig_output: String,
    pub group_service: GroupService,
    pub workspace_service: WorkspaceService,
    pub waybar_sync: WaybarSyncService,
}

impl SwayTestFixture {
    /// Create a new fixture. Must be called inside a `#[tokio::test]` runtime.
    pub async fn new() -> Result<Self> {
        let ipc = SwayIpcClient::new().context("SWAYSOCK not set — is Sway running?")?;

        let focused = ipc.get_focused_workspace().context("No focused workspace")?;
        let orig_workspace = focused.name.clone();
        let orig_output = focused.output.clone();

        let db_path = PathBuf::from(TEST_DB_PATH);
        if db_path.exists() {
            std::fs::remove_file(&db_path).context("Failed to remove stale test DB")?;
        }

        let db = DatabaseManager::new(db_path.clone())
            .await
            .context("Failed to create test DB")?;

        let waybar_client = WaybarClient::new();
        let group_service = GroupService::new(db.clone(), ipc.clone());
        let workspace_service = WorkspaceService::new(db.clone(), ipc.clone());
        let waybar_sync = WaybarSyncService::new(db.clone(), ipc.clone(), waybar_client);

        group_service
            .ensure_default_group()
            .await
            .context("ensure_default_group failed")?;
        workspace_service
            .sync_from_sway()
            .await
            .context("sync_from_sway failed")?;

        Ok(Self {
            ipc,
            db,
            db_path,
            orig_workspace,
            orig_output,
            group_service,
            workspace_service,
            waybar_sync,
        })
    }

    /// Return the active group name for the originally focused output.
    pub async fn active_group(&self) -> Result<String> {
        self.group_service
            .get_active_group(&self.orig_output)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Switch Sway focus to the given workspace.
    pub fn focus_workspace(&self, name: &str) -> Result<()> {
        let results = self
            .ipc
            .run_command(&format!("workspace \"{}\"", name))
            .map_err(|e| anyhow::anyhow!(e))?;
        if results.first().map(|r| r.success) == Some(true) {
            Ok(())
        } else {
            bail!("sway refused to focus workspace '{}'", name)
        }
    }

    /// Return the name of the currently focused workspace.
    pub fn focused_workspace(&self) -> Result<String> {
        Ok(self
            .ipc
            .get_focused_workspace()
            .map_err(|e| anyhow::anyhow!(e))?
            .name)
    }

    /// Block until a condition is true or the timeout is exceeded.
    pub fn wait_until(&self, timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("Timeout waiting for condition");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for SwayTestFixture {
    fn drop(&mut self) {
        let _ = self
            .ipc
            .run_command(&format!("workspace \"{}\"", self.orig_workspace));
    }
}

// ---------------------------------------------------------------------------
// DummyWindowHandle
// ---------------------------------------------------------------------------

/// RAII wrapper around a spawned `sway-dummy-window` process.
pub struct DummyWindowHandle {
    child: Child,
    pub app_id: String,
}

impl DummyWindowHandle {
    /// Spawn a new dummy window with the given `app_id` and wait until it
    /// appears in the Sway tree (up to 2 seconds).
    pub fn spawn(fixture: &SwayTestFixture, app_id: &str) -> Result<Self> {
        let binary = dummy_window_binary();
        let child = Command::new(&binary)
            .arg(app_id)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn '{}'", binary.display()))?;

        let handle = Self {
            child,
            app_id: app_id.to_string(),
        };

        let ipc = fixture.ipc.clone();
        let id = app_id.to_string();
        fixture
            .wait_until(Duration::from_secs(2), move || {
                window_exists_in_tree(&ipc, &id)
            })
            .with_context(|| {
                format!("Dummy window '{}' never appeared in Sway tree", app_id)
            })?;

        Ok(handle)
    }

    /// Returns `true` if the window is currently visible in the Sway tree.
    pub fn exists_in_tree(&self, fixture: &SwayTestFixture) -> bool {
        window_exists_in_tree(&fixture.ipc, &self.app_id)
    }
}

impl Drop for DummyWindowHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn window_exists_in_tree(ipc: &SwayIpcClient, app_id: &str) -> bool {
    let Ok(bytes) = ipc.get_tree() else { return false };
    let Ok(tree) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    find_app_id(&tree, app_id)
}

fn find_app_id(node: &serde_json::Value, app_id: &str) -> bool {
    if node
        .get("app_id")
        .and_then(|v: &serde_json::Value| v.as_str())
        == Some(app_id)
    {
        return true;
    }
    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node
            .get(key)
            .and_then(|v: &serde_json::Value| v.as_array())
        {
            if children.iter().any(|c| find_app_id(c, app_id)) {
                return true;
            }
        }
    }
    false
}

fn dummy_window_binary() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sway-dummy-window") {
        return PathBuf::from(path);
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            manifest_dir
                .parent()
                .unwrap_or(&manifest_dir)
                .join("target")
        });

    let candidate = target_dir.join("debug").join("sway-dummy-window");
    if candidate.exists() {
        return candidate;
    }

    if let Ok(mut exe) = std::env::current_exe() {
        exe.pop();
        let candidate = exe.join("sway-dummy-window");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("sway-dummy-window")
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Assert that a group with the given name exists in the test DB.
pub async fn assert_group_exists(db: &DatabaseManager, name: &str) {
    let found = GroupEntity::find_by_name(name)
        .one(db.conn())
        .await
        .unwrap_or(None);
    assert!(
        found.is_some(),
        "Expected group '{}' to exist in DB, but it does not",
        name
    );
}

/// Assert that no group with the given name exists in the test DB.
pub async fn assert_group_not_exists(db: &DatabaseManager, name: &str) {
    let found = GroupEntity::find_by_name(name)
        .one(db.conn())
        .await
        .unwrap_or(None);
    assert!(
        found.is_none(),
        "Expected group '{}' NOT to exist in DB, but it does",
        name
    );
}

/// Assert that the active group for the given output matches `expected`.
pub async fn assert_active_group(fixture: &SwayTestFixture, output: &str, expected: &str) {
    let actual = fixture
        .group_service
        .get_active_group(output)
        .await
        .unwrap_or_else(|_| "(error)".to_string());
    assert_eq!(
        actual, expected,
        "Active group on '{}': expected '{}', got '{}'",
        output, expected, actual
    );
}

/// Assert that the currently focused Sway workspace matches `expected`.
pub fn assert_focused_workspace(fixture: &SwayTestFixture, expected: &str) {
    let actual = fixture
        .ipc
        .get_focused_workspace()
        .map(|ws| ws.name)
        .unwrap_or_else(|_| "(error)".to_string());
    assert_eq!(
        actual, expected,
        "Focused workspace: expected '{}', got '{}'",
        expected, actual
    );
}

/// Assert that a workspace with the given name exists in the test DB.
pub async fn assert_workspace_exists(db: &DatabaseManager, name: &str) {
    let found = WorkspaceEntity::find_by_name(name)
        .one(db.conn())
        .await
        .unwrap_or(None);
    assert!(
        found.is_some(),
        "Expected workspace '{}' to exist in DB, but it does not",
        name
    );
}

/// Assert that no workspace with the given name exists in the test DB.
pub async fn assert_workspace_not_exists(db: &DatabaseManager, name: &str) {
    let found = WorkspaceEntity::find_by_name(name)
        .one(db.conn())
        .await
        .unwrap_or(None);
    assert!(
        found.is_none(),
        "Expected workspace '{}' NOT to exist in DB, but it does",
        name
    );
}

/// Assert that a workspace is a member of the given group in the test DB.
pub async fn assert_workspace_in_group(db: &DatabaseManager, workspace: &str, group: &str) {
    let ws = WorkspaceEntity::find_by_name(workspace)
        .one(db.conn())
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| panic!("Workspace '{}' not found in DB", workspace));

    let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(db.conn())
        .await
        .unwrap_or_default();

    // Resolve group names for each membership.
    let mut in_group = false;
    for m in &memberships {
        let group_model = sway_groups_core::db::entities::GroupEntity::find_by_id(m.group_id)
            .one(db.conn())
            .await
            .unwrap_or(None);
        if let Some(g) = group_model {
            if g.name == group {
                in_group = true;
                break;
            }
        }
    }

    assert!(
        in_group,
        "Expected workspace '{}' to be in group '{}', but it is not",
        workspace, group
    );
}

/// Assert that no test data (groups/workspaces with the test prefix) remains.
pub async fn assert_no_test_data(db: &DatabaseManager) {
    let groups = GroupEntity::find().all(db.conn()).await.unwrap_or_default();
    let test_groups: Vec<_> = groups
        .iter()
        .filter(|g| g.name.starts_with(TEST_PREFIX))
        .collect();
    assert!(
        test_groups.is_empty(),
        "Test groups still in DB: {:?}",
        test_groups.iter().map(|g| &g.name).collect::<Vec<_>>()
    );

    let workspaces = WorkspaceEntity::find()
        .all(db.conn())
        .await
        .unwrap_or_default();
    let test_ws: Vec<_> = workspaces
        .iter()
        .filter(|w| w.name.starts_with(TEST_PREFIX))
        .collect();
    assert!(
        test_ws.is_empty(),
        "Test workspaces still in DB: {:?}",
        test_ws.iter().map(|w| &w.name).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Additional assertion helpers
// ---------------------------------------------------------------------------

/// Assert that a workspace is NOT a member of the given group.
pub async fn assert_workspace_not_in_group(db: &DatabaseManager, workspace: &str, group: &str) {
    let ws = WorkspaceEntity::find_by_name(workspace)
        .one(db.conn())
        .await
        .unwrap_or(None);

    if ws.is_none() {
        // Workspace doesn't exist at all → trivially not in group
        return;
    }
    let ws = ws.unwrap();

    let memberships = WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(db.conn())
        .await
        .unwrap_or_default();

    let in_group = memberships.iter().any(|m| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sway_groups_core::db::entities::GroupEntity::find_by_id(m.group_id)
                    .one(db.conn())
                    .await
                    .unwrap_or(None)
                    .map(|g| g.name == group)
                    .unwrap_or(false)
            })
        })
    });

    assert!(
        !in_group,
        "Expected workspace '{}' NOT to be in group '{}', but it is",
        workspace, group
    );
}

/// Assert that the workspace's `is_global` flag matches `expected`.
pub async fn assert_workspace_global(db: &DatabaseManager, name: &str, expected: bool) {
    let found = WorkspaceEntity::find_by_name(name)
        .one(db.conn())
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| panic!("Workspace '{}' not found in DB", name));
    assert_eq!(
        found.is_global, expected,
        "Workspace '{}' is_global: expected {}, got {}",
        name, expected, found.is_global
    );
}

/// Assert that the active group on the fixture's orig_output matches `expected`.
pub async fn assert_active_group_orig(fixture: &SwayTestFixture, expected: &str) {
    assert_active_group(fixture, &fixture.orig_output.clone(), expected).await;
}

/// Assert that a workspace exists exactly once in the Sway tree.
pub fn assert_window_in_tree(fixture: &SwayTestFixture, app_id: &str) {
    assert!(
        window_exists_in_tree(&fixture.ipc, app_id),
        "Expected window '{}' to exist in Sway tree, but it does not",
        app_id
    );
}

/// Assert that a workspace does NOT exist in the Sway tree.
pub fn assert_window_not_in_tree(fixture: &SwayTestFixture, app_id: &str) {
    assert!(
        !window_exists_in_tree(&fixture.ipc, app_id),
        "Expected window '{}' NOT to exist in Sway tree, but it does",
        app_id
    );
}

/// Assert that a workspace with the given name exists in Sway.
pub fn assert_sway_workspace_exists(fixture: &SwayTestFixture, name: &str) {
    let exists = fixture
        .ipc
        .get_workspaces()
        .map(|ws| ws.iter().any(|w| w.name == name))
        .unwrap_or(false);
    assert!(
        exists,
        "Expected workspace '{}' to exist in Sway, but it does not",
        name
    );
}

/// Assert that a workspace does NOT exist in Sway.
pub fn assert_sway_workspace_not_exists(fixture: &SwayTestFixture, name: &str) {
    let exists = fixture
        .ipc
        .get_workspaces()
        .map(|ws| ws.iter().any(|w| w.name == name))
        .unwrap_or(false);
    assert!(
        !exists,
        "Expected workspace '{}' NOT to exist in Sway, but it does",
        name
    );
}

/// Assert the focused workspace is on a specific Sway workspace name (container check).
/// Verifies that a window with app_id `app_id` is on workspace `workspace_name`.
pub fn assert_window_on_workspace(fixture: &SwayTestFixture, app_id: &str, workspace_name: &str) {
    let Ok(bytes) = fixture.ipc.get_tree() else {
        panic!("Failed to get Sway tree");
    };
    let Ok(tree) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        panic!("Failed to parse Sway tree");
    };

    let ws = find_workspace_of_app_id(&tree, app_id);
    assert_eq!(
        ws.as_deref(),
        Some(workspace_name),
        "Expected window '{}' to be on workspace '{}', but got {:?}",
        app_id, workspace_name, ws
    );
}

fn find_workspace_of_app_id(node: &serde_json::Value, app_id: &str) -> Option<String> {
    find_workspace_of_app_id_inner(node, app_id, None)
}

fn find_workspace_of_app_id_inner(
    node: &serde_json::Value,
    app_id: &str,
    current_ws: Option<&str>,
) -> Option<String> {
    let node_type = node.get("type").and_then(|v: &serde_json::Value| v.as_str());
    let node_name = node.get("name").and_then(|v: &serde_json::Value| v.as_str());

    let ws = if node_type == Some("workspace") {
        node_name
    } else {
        current_ws
    };

    if node.get("app_id").and_then(|v: &serde_json::Value| v.as_str()) == Some(app_id) {
        return ws.map(|s| s.to_string());
    }

    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v: &serde_json::Value| v.as_array()) {
            for child in children {
                if let Some(result) = find_workspace_of_app_id_inner(child, app_id, ws) {
                    return Some(result);
                }
            }
        }
    }
    None
}

/// Helper: switch Sway to a group and back, triggering auto-delete logic.
/// Returns the group that was switched to before returning.
pub async fn switch_group_and_back(
    fixture: &SwayTestFixture,
    to_group: &str,
    back_group: &str,
) -> anyhow::Result<()> {
    fixture
        .group_service
        .set_active_group(&fixture.orig_output, to_group)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    fixture
        .group_service
        .set_active_group(&fixture.orig_output, back_group)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
}
