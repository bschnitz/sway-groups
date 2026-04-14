use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, swayg_live, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_rem_a";
const GROUP_B: &str = "zz_test_rem_b";
const WS1: &str = "zz_tg_ws1_rem";

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

fn workspace_exists_in_sway(ws: &str) -> bool {
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
        .any(|w| w.get("name").and_then(|n| n.as_str()) == Some(ws))
}

#[tokio::test]
async fn test_05f_multi_group_workspace_remove() {
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

    // --- Precondition: no test data in production DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        for g in [GROUP_A, GROUP_B] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0,
                "{} must not exist in production DB",
                g
            );
        }
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "{} must not exist in production DB",
            WS1
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "{} must not exist in sway", WS1);

    // --- Setup: init + Group A + WS1 in Group A ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "group '{}' was created",
        GROUP_A
    );

    let active = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active, GROUP_A, "active group = '{}'", GROUP_A);

    let _win = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(get_focused_workspace().unwrap(), WS1, "focused on '{}'", WS1);
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' in group '{}'",
        WS1, GROUP_A
    );

    // --- Switch to Group B + add WS1 (multi-group membership) ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    let active_b = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active_b, GROUP_B, "active group = '{}'", GROUP_B);

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "'{}' NOT auto-deleted (still has {})",
        GROUP_A, WS1
    );

    fixture.swayg(&["workspace", "add", WS1]).success();

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' still in group '{}'",
        WS1, GROUP_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "'{}' now in group '{}'",
        WS1, GROUP_B
    );

    // --- TEST: workspace remove WS1 from Group B (active) ---
    fixture.swayg(&["workspace", "remove", WS1]).success();

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_B),
        0,
        "'{}' removed from group '{}'",
        WS1, GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' still in group '{}'",
        WS1, GROUP_A
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        1,
        "'{}' still in DB",
        WS1
    );
    assert!(workspace_exists_in_sway(WS1), "'{}' still in sway", WS1);
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "'{}' still exists",
        GROUP_A
    );

    // --- Verify WS1 NOT visible in Group B ---
    let visible_b = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        !visible_b.lines().any(|l| l.contains(WS1)),
        "'{}' NOT visible in Group B",
        WS1
    );

    // --- Switch to Group A, verify WS1 visible ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    let active_a = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active_a, GROUP_A, "active group = '{}'", GROUP_A);

    let visible_a = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        visible_a.lines().any(|l| l.contains(WS1)),
        "'{}' visible in Group A",
        WS1
    );

    // --- Switch back to orig group (Group A should NOT auto-delete, WS1 still in sway) ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "'{}' NOT auto-deleted ({} still in sway)",
        GROUP_A, WS1
    );

    // --- Kill window on orig_group, then auto-delete Group A ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        workspace_of_window(WS1).is_none(),
        "dummy window '{}' is gone",
        WS1
    );

    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        0,
        "'{}' auto-deleted ({} gone from sway)",
        GROUP_A, WS1
    );

    // --- Post-condition: init to sync DB state ---
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
