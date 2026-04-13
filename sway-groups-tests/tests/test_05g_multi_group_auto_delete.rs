use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{DummyWindowHandle, TestFixture, get_focused_workspace, swayg_live};

const GROUP_A: &str = "zz_test_group_a__05g";
const GROUP_B: &str = "zz_test_group_b__05g";
const WS1: &str = "zz_test_ws1__05g";
const WS2: &str = "zz_test_ws2__05g";

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

#[tokio::test]
async fn test_05g_multi_group_auto_delete() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // --- Precondition checks on REAL db ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, "groups", "name", GROUP_A),
            0,
            "precondition: {} must not exist in real DB",
            GROUP_A
        );
        assert_eq!(
            db_count(&real_db, "groups", "name", GROUP_B),
            0,
            "precondition: {} must not exist in real DB",
            GROUP_B
        );
        assert_eq!(
            db_count(&real_db, "workspaces", "name", WS1),
            0,
            "precondition: {} must not exist in real DB",
            WS1
        );
        assert_eq!(
            db_count(&real_db, "workspaces", "name", WS2),
            0,
            "precondition: {} must not exist in real DB",
            WS2
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "precondition: {} must not exist in sway",
        WS1
    );
    assert_eq!(
        workspace_count_in_sway(WS2), 0,
        "precondition: {} must not exist in sway",
        WS2
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

    // --- Setup: init + create groups + workspaces + dummy windows ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1])
        .success();

    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS2, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    // --- Verify setup ---
    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_A),
        1,
        "group {} exists",
        GROUP_A
    );

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_B),
        1,
        "group {} exists",
        GROUP_B
    );

    let ws1_in_ga: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS1, GROUP_A
        ),
    );
    assert_eq!(ws1_in_ga, "1", "{} is in group {}", WS1, GROUP_A);

    let ws1_in_gb: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS1, GROUP_B
        ),
    );
    assert_eq!(ws1_in_gb, "1", "{} is in group {}", WS1, GROUP_B);

    let ws2_in_gb: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS2, GROUP_B
        ),
    );
    assert_eq!(ws2_in_gb, "1", "{} is in group {}", WS2, GROUP_B);

    assert_eq!(
        workspace_count_in_sway(WS1), 1,
        "{} is in sway",
        WS1
    );

    assert_eq!(
        workspace_count_in_sway(WS2), 1,
        "{} is in sway",
        WS2
    );

    // --- Test: switch to Group A, back — Group A should NOT auto-delete ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    let active = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP_A, "active group = {}", GROUP_A);

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_A),
        1,
        "{} NOT auto-deleted ({} still in sway)",
        GROUP_A, WS1
    );

    // --- Kill dummy window WS1, verify WS1 gone from sway ---
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1), 0,
        "dummy window {} is gone",
        WS1
    );

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "{} gone from sway",
        WS1
    );

    // --- Test: switch to Group A, back — Group A NOW auto-deleted ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    let active = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP_A, "active group = {}", GROUP_A);

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_A),
        0,
        "{} auto-deleted ({} gone from sway, no non-global workspaces)",
        GROUP_A, WS1
    );

    // --- Cleanup: Group B should NOT auto-delete (still has WS2) ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_B),
        1,
        "{} NOT auto-deleted (still has {})",
        GROUP_B, WS2
    );

    // --- Kill dummy window WS2 ---
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS2), 0,
        "dummy window {} is gone",
        WS2
    );

    // --- Switch to Group B then back (NOW auto-delete Group B) ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_B),
        0,
        "{} auto-deleted",
        GROUP_B
    );

    // --- Post-condition: sync DB and verify no test data ---
    fixture.init().success();

    let group_gone: i64 = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        ),
    )
    .trim()
    .parse()
    .unwrap_or(0);
    let ws_gone: i64 = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS1, WS2
        ),
    )
    .trim()
    .parse()
    .unwrap_or(0);
    let wsgrp_gone: i64 = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        ),
    )
    .trim()
    .parse()
    .unwrap_or(0);

    assert_eq!(
        (group_gone, ws_gone, wsgrp_gone),
        (0, 0, 0),
        "no test data remains in DB"
    );

    // --- Cleanup: restore original group on live DB ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
