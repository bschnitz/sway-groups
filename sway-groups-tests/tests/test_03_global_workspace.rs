use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{TestFixture, DummyWindowHandle, get_focused_workspace};

const TEST_GROUP: &str = "zz_test_global";
const WS1: &str = "zz_test_ws1_glo";
const WS2: &str = "zz_test_ws2_glo";

fn db_count(db_path: &PathBuf, table: &str, column: &str, value: &str) -> i64 {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(format!(
            "SELECT count(*) FROM {} WHERE {} = '{}'",
            table, column, value
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

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

fn workspace_count_in_sway(name: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let workspaces: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse workspaces");
    let count = workspaces
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w.get("name").and_then(|n| n.as_str()) == Some(name))
        .count();
    count as i64
}

fn window_count_in_tree(app_id: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse tree");
    let mut count = 0i64;
    fn find(node: &serde_json::Value, app_id: &str, count: &mut i64) {
        if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
            *count += 1;
        }
        for key in &["nodes", "floating_nodes"] {
            if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
                for child in children {
                    find(child, app_id, count);
                }
            }
        }
    }
    find(&tree, app_id, &mut count);
    count
}

fn output_contains(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|line| line.contains(needle))
}

#[tokio::test]
async fn test_03_global_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // --- Precondition checks on REAL db ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, "groups", "name", TEST_GROUP),
            0,
            "precondition: test group must not exist in real DB"
        );
        assert_eq!(
            db_count(&real_db, "workspaces", "name", WS1),
            0,
            "precondition: WS1 must not exist in real DB"
        );
        assert_eq!(
            db_count(&real_db, "workspaces", "name", WS2),
            0,
            "precondition: WS2 must not exist in real DB"
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "precondition: WS1 must not exist in sway"
    );
    assert_eq!(
        workspace_count_in_sway(WS2), 0,
        "precondition: WS2 must not exist in sway"
    );

    // --- Remember original state ---
    let orig_group = {
        let output = Command::new("swayg")
            .args(["group", "active", &fixture.orig_output])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .expect("swayg group active failed");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- 1. Init fresh DB ---
    fixture.init().success();

    // --- 2. Select test group (with --create) ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, TEST_GROUP, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        1,
        "group was created"
    );

    // --- 3. Launch dummy window WS1 and move to workspace ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on WS1 after container move"
    );

    // --- 4. Launch dummy window WS2 and move to workspace ---
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS2, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS2,
        "focused on WS2 after container move"
    );

    // --- 5. Set WS1 as global ---
    fixture
        .swayg(&["workspace", "global", WS1])
        .success();

    let ws1_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_global, "1", "WS1 is global in DB");

    // --- 6. Switch back to original group ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace after switching back"
    );

    // --- 7. Verify global visibility ---
    let visible = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible, WS1),
        "WS1 is visible in orig group (global)"
    );
    assert!(
        !output_contains(&visible, WS2),
        "WS2 is NOT visible in orig group (not global)"
    );

    // --- 8. Verify group membership ---
    let group_ws = swayg_output(
        &fixture.db_path,
        &[
            "workspace", "list", "--plain", "--group", TEST_GROUP,
            "--output", &fixture.orig_output,
        ],
    );
    assert!(
        !output_contains(&group_ws, WS1),
        "WS1 is NOT in test group (global, no group membership)"
    );
    assert!(
        output_contains(&group_ws, WS2),
        "WS2 is visible in test group"
    );

    // --- 9. Unglobal WS1 ---
    fixture
        .swayg(&["workspace", "unglobal", WS1])
        .success();

    let ws1_not_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_not_global, "0", "WS1 is no longer global");

    // --- 10. WS1 visible in active group after unglobal ---
    let visible_after_unglobal = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible_after_unglobal, WS1),
        "WS1 is visible in orig group after unglobal"
    );

    // --- 11a. Auto-delete: switch from global workspace ---
    // Switch to test group
    fixture
        .swayg(&["group", "select", &fixture.orig_output, TEST_GROUP])
        .success();

    // Set WS2 as global
    fixture
        .swayg(&["workspace", "global", WS2])
        .success();

    let ws2_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS2),
    );
    assert_eq!(ws2_global, "1", "WS2 is global in DB");

    // Kill dummy window WS1
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1), 0,
        "dummy window WS1 is gone"
    );

    // Switch to WS2 (let sway auto-delete empty WS1)
    let _ = Command::new("swaymsg")
        .args(["workspace", WS2])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "WS1 is gone from sway"
    );

    // Test group still exists (has global workspaces)
    let groups = swayg_output(
        &fixture.db_path,
        &["group", "list", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&groups, TEST_GROUP),
        "test group still exists (has global workspaces)"
    );

    // Switch back from global workspace (should auto-delete test group)
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace after auto-delete"
    );

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        0,
        "test group auto-deleted (switched from global workspace)"
    );

    // --- 11b. Auto-delete: switch from empty workspace (only global workspaces remain) ---

    // Create test group again
    fixture
        .swayg(&["group", "select", &fixture.orig_output, TEST_GROUP, "--create"])
        .success();

    // Launch dummy window WS1 again
    let _win1b = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1 (again)");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["workspace", "global", WS1])
        .success();

    let ws1_global_b: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_global_b, "1", "WS1 is global in DB (second time)");

    let groups_b = swayg_output(
        &fixture.db_path,
        &["group", "list", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&groups_b, TEST_GROUP),
        "test group still exists (has global workspace)"
    );

    // Switch back to original group (from empty workspace, should auto-delete)
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace after second auto-delete"
    );

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        0,
        "test group auto-deleted (switched from empty workspace, only global remained)"
    );

    // --- Cleanup: kill remaining windows ---
    drop(_win2);
    drop(_win1b);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(window_count_in_tree(WS1), 0, "WS1 window is gone after cleanup");
    assert_eq!(window_count_in_tree(WS2), 0, "WS2 window is gone after cleanup");

    // --- Post-condition: sync DB and verify no test data ---
    fixture.init().success();

    let group_gone = db_count(&fixture.db_path, "groups", "name", TEST_GROUP);
    let ws_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS1, WS2
        ),
    )
    .trim()
    .parse::<i64>()
    .unwrap_or(0);
    let wsgrp_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '{}'",
            TEST_GROUP
        ),
    )
    .trim()
    .parse::<i64>()
    .unwrap_or(0);

    assert_eq!(
        (group_gone, ws_gone, wsgrp_gone),
        (0, 0, 0),
        "no test data remains in DB"
    );
}
