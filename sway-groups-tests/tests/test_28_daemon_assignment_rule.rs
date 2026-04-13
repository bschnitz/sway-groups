use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use sway_groups_tests::common::{TestFixture, DummyWindowHandle, get_focused_workspace};

const TEST_GROUP: &str = "zz_test_assign_group";
const APP_ID: &str = "assignment-test-id";
const ASSIGNED_WS: &str = "test_workspace_1";

fn db_query(db_path: &PathBuf, sql: &str) -> String {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn swayg_output(db_path: &PathBuf, args: &[&str]) -> String {
    sway_groups_tests::common::swayg_output(db_path, args)
}

struct DaemonHandle {
    child: Child,
}

impl DaemonHandle {
    fn spawn(db_path: &PathBuf) -> Self {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_default();
        let daemon_bin = manifest_dir
            .parent()
            .unwrap_or(&manifest_dir)
            .join("target")
            .join("debug")
            .join("swayg-daemon");
        let log_file = std::fs::File::create("/tmp/swayg-daemon-test.log").expect("create daemon log");
        let child = Command::new(&daemon_bin)
            .arg(db_path)
            .env("RUST_LOG", "sway_groups_daemon=info")
            .stdout(Stdio::null())
            .stderr(Stdio::from(log_file))
            .spawn()
            .expect("Failed to spawn swayg-daemon");
        Self { child }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn check_assignment_rule() {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".config/sway/assignments.conf");

    if !config_path.exists() {
        panic!(
            "ASSIGNMENT CONFIG NOT FOUND at {}.\n\
             Please create it with:\n\
             \n\
             assign [app_id=\"assignment-test-id\"] workspace test_workspace_1\n",
            config_path.display()
        );
    }

    let content = std::fs::read_to_string(&config_path)
        .unwrap_or_default();

    if !content.contains("assignment-test-id") {
        panic!(
            "ASSIGNMENT RULE NOT FOUND in {}.\n\
             Please add the following line:\n\
             \n\
             for_window [app_id=\"assignment-test-id\"] workspace test_workspace_1\n\
             \n\
             Then run 'swaymsg reload'.",
            config_path.display()
        );
    }
}

fn workspace_count_in_sway(name: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let workspaces: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse workspaces");
    workspaces
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w.get("name").and_then(|n| n.as_str()) == Some(name))
        .count() as i64
}

fn workspace_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
    db_query(
        db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{}' AND g.name = '{}'",
            ws, group
        ),
    )
    .parse()
    .unwrap_or(0)
}

#[tokio::test]
async fn test_28_daemon_with_assignment_rule() {
    check_assignment_rule();

    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();
    let orig_output = fixture.orig_output.clone();

    fixture.init().success();

    let _daemon = DaemonHandle::spawn(&fixture.db_path);
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &orig_output, "--create"])
        .success();

    assert_eq!(
        db_query(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        "1",
        "group was created"
    );

    let _win = DummyWindowHandle::spawn(APP_ID).expect("spawn dummy window with assignment app_id");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let focused = get_focused_workspace().expect("get focused workspace after assignment");
    assert_eq!(
        focused, ASSIGNED_WS,
        "assignment rule moved window to '{}'",
        ASSIGNED_WS
    );

    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_count = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", ASSIGNED_WS
    ));
    assert_eq!(
        ws_count, "1",
        "daemon should have added assigned workspace '{}' to DB", ASSIGNED_WS
    );

    let active_group = swayg_output(&fixture.db_path, &["group", "active", &orig_output]);
    eprintln!("[DEBUG] active_group before move: {}", active_group);

    fixture
        .swayg(&["workspace", "move", ASSIGNED_WS, "--groups", TEST_GROUP])
        .success();

    std::thread::sleep(std::time::Duration::from_millis(500));

    let in_group = workspace_in_group_count(&fixture.db_path, ASSIGNED_WS, TEST_GROUP);
    assert_eq!(
        in_group, 1,
        "workspace '{}' should be in group '{}' after move", ASSIGNED_WS, TEST_GROUP
    );

    let active_after_move = swayg_output(&fixture.db_path, &["group", "active", &orig_output]);
    eprintln!("[DEBUG] active_group after move: {}", active_after_move);
    assert_eq!(
        active_after_move, TEST_GROUP,
        "active_group should still be TEST_GROUP after workspace move"
    );

    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
