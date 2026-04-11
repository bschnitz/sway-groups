use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_group_a";
const GROUP_B: &str = "zz_test_group_b";
const WS1: &str = "zz_test_ws1_add";

fn db_count(db_path: &PathBuf, sql: &str) -> i64 {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn workspace_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
    db_count(
        db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{}' AND g.name = '{}'",
            ws, group
        ),
    )
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

#[tokio::test]
async fn test_05a_multi_group_workspace_add() {
    let fixture = TestFixture::new().await.expect("fixture setup");

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

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
            0,
            "{} must not exist in production DB",
            GROUP_A
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
            0,
            "{} must not exist in production DB",
            GROUP_B
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "{} must not exist in production DB",
            WS1
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1),
        0,
        "precondition: {} must not exist in sway",
        WS1
    );

    // --- Init ---
    fixture.init().success();

    // --- Create Group A ---
    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            GROUP_A,
            "--create",
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group '{}' was created",
        GROUP_A
    );

    let active = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP_A, "active group = '{}'", GROUP_A);

    // --- Launch dummy window WS1 ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    // --- Move container to WS1 (creates WS1 in sway, adds to Group A) ---
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on '{}'",
        WS1
    );

    assert_eq!(
        sway_groups_tests::common::workspace_of_window(WS1).as_deref(),
        Some(WS1),
        "window '{}' is on workspace '{}'",
        WS1, WS1
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
        ),
        1,
        "{} is in DB",
        WS1
    );

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group '{}'",
        WS1, GROUP_A
    );

    // --- Switch to Group B ---
    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            GROUP_B,
            "--create",
        ])
        .success();

    let active_b = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active_b, GROUP_B, "active group = '{}'", GROUP_B);

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "{} NOT auto-deleted (still has {})",
        GROUP_A, WS1
    );

    // --- workspace add for existing WS1 (already in Group A, adding to Group B) ---
    fixture
        .swayg(&["workspace", "add", WS1])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
        ),
        1,
        "{} still exactly 1 row in DB",
        WS1
    );

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group '{}'",
        WS1, GROUP_A
    );

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} is in group '{}'",
        WS1, GROUP_B
    );

    assert_eq!(
        workspace_count_in_sway(WS1),
        1,
        "{} exists exactly once in sway",
        WS1
    );

    // --- Verify visibility in Group B ---
    let visible = swayg_output(
        &fixture.db_path,
        &[
            "workspace",
            "list",
            "--visible",
            "--plain",
            "--output",
            &fixture.orig_output,
        ],
    );
    assert!(
        visible.lines().any(|l| l.contains(WS1)),
        "{} is visible in Group B",
        WS1
    );

    // --- Switch back to original group ---
    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            &orig_group,
        ])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- Kill dummy window (WS1 disappears from sway) ---
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        sway_groups_tests::common::workspace_of_window(WS1).is_none(),
        "window '{}' is gone",
        WS1
    );

    // --- Auto-delete Group B (WS1 gone from sway) ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_B])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            &orig_group,
        ])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on '{}' after Group B cleanup",
        orig_ws
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        0,
        "{} auto-deleted",
        GROUP_B
    );

    // --- Auto-delete Group A (WS1 gone from sway) ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_A])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            &orig_group,
        ])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on '{}' after Group A cleanup",
        orig_ws
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        0,
        "{} auto-deleted",
        GROUP_A
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        ),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1),
    );
    let wsgrp_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             WHERE g.name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        ),
    );

    assert_eq!(
        (group_gone, ws_gone, wsgrp_gone),
        (0, 0, 0),
        "no test data remains in DB"
    );
}
