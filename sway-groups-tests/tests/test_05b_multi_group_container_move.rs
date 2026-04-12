use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_group_a";
const GROUP_B: &str = "zz_test_group_b";
const WS1: &str = "zz_test_ws1_cmv";
const WS2: &str = "zz_test_ws2_cmv";

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
async fn test_05b_multi_group_container_move() {
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
            0, "{} must not exist in production DB", GROUP_A
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
            0, "{} must not exist in production DB", GROUP_B
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0, "{} must not exist in production DB", WS1
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
            0, "{} must not exist in production DB", WS2
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1),
        0,
        "precondition: {} must not exist in sway",
        WS1
    );
    assert_eq!(
        workspace_count_in_sway(WS2),
        0,
        "precondition: {} must not exist in sway",
        WS2
    );

    // --- Init ---
    fixture.init().success();

    // --- Create Group A ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_A,
            "--output",
            &fixture.orig_output,
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
        workspace_of_window(WS1).as_deref(),
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
            GROUP_B,
            "--output",
            &fixture.orig_output,
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

    // --- Launch dummy window WS2 ---
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS2).is_some(),
        "dummy window '{}' exists in sway tree",
        WS2
    );

    // --- container move to WS1 (existing in Group A, not in Group B) ---
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
        workspace_of_window(WS2).as_deref(),
        Some(WS1),
        "window '{}' moved to workspace '{}'",
        WS2, WS1
    );

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
        "{} still in group '{}'",
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
            &orig_group,
            "--output",
            &fixture.orig_output,
        ])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- Kill dummy windows ---
    drop(_win1);
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        workspace_of_window(WS1).is_none(),
        "window '{}' is gone",
        WS1
    );
    assert!(
        workspace_of_window(WS2).is_none(),
        "window '{}' is gone",
        WS2
    );

    // --- Auto-delete Group B ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &orig_group,
            "--output",
            &fixture.orig_output,
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

    // --- Auto-delete Group A ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &orig_group,
            "--output",
            &fixture.orig_output,
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
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS1, WS2
        ),
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
