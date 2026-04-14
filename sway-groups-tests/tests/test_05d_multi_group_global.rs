use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{DummyWindowHandle, TestFixture, get_focused_workspace, swayg_live, workspace_of_window};

const GROUP_A: &str = "zz_test_group_a__05d";
const GROUP_B: &str = "zz_test_group_b__05d";
const WS1: &str = "zz_test_ws1__05d";

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
async fn test_05d_multi_group_global() {
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
    }

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "precondition: {} must not exist in sway",
        WS1
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

    // --- Init fresh DB ---
    fixture.init().success();

    // --- Create Group A ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_A),
        1,
        "group {} was created",
        GROUP_A
    );

    // --- Launch dummy window WS1, move to WS1 ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    assert!(
        window_count_in_tree(WS1) >= 1,
        "dummy window {} is running",
        WS1
    );

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on {} after container move",
        WS1
    );

    let ws1_in_ga: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS1, GROUP_A
        ),
    );
    assert_eq!(ws1_in_ga, "1", "{} is in group {}", WS1, GROUP_A);

    // --- Switch to Group B ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    let active = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP_B, "active group = {}", GROUP_B);

    // --- Add WS1 to Group B (multi-group) ---
    fixture
        .swayg(&["workspace", "add", WS1])
        .success();

    let ws1_still_in_ga: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS1, GROUP_A
        ),
    );
    assert_eq!(ws1_still_in_ga, "1", "{} still in group {}", WS1, GROUP_A);

    let ws1_in_gb: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
            WS1, GROUP_B
        ),
    );
    assert_eq!(ws1_in_gb, "1", "{} is in group {}", WS1, GROUP_B);

    // --- Set WS1 global ---
    fixture
        .swayg(&["workspace", "global", WS1])
        .success();

    let ws1_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_global, "1", "{} is global in DB", WS1);

    let ws1_wsgrp: String = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}'",
            WS1
        ),
    );
    assert_eq!(ws1_wsgrp, "0", "{} has no workspace_groups entries", WS1);

    let visible = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible, WS1),
        "{} is visible (global)",
        WS1
    );

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_A),
        0,
        "{} auto-deleted (not active, no non-global workspaces)",
        GROUP_A
    );

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_B),
        1,
        "{} still exists (is active group)",
        GROUP_B
    );

    let visible_gb = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible_gb, WS1),
        "{} visible in Group B (global)",
        WS1
    );

    // --- Switch back to original group ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", GROUP_B),
        0,
        "{} auto-deleted",
        GROUP_B
    );

    // --- Kill dummy window ---
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1), 0,
        "dummy window {} is gone",
        WS1
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
        &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1),
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
