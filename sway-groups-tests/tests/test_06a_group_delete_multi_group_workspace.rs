use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{get_focused_workspace, swayg_output, swayg_live, DummyWindowHandle, TestFixture};

const GROUP_A: &str = "zz_test_grp_a_06a";
const GROUP_B: &str = "zz_test_grp_b_06a";
const WS1: &str = "zz_test_ws1_06a";
const WS2: &str = "zz_test_ws2_06a";

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

fn ws_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
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

fn orig_active_group(output_name: &str) -> String {
    let out = Command::new("swayg")
        .args(["group", "active", output_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swayg group active failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[tokio::test]
async fn test_06a_group_delete_multi_group_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
            0,
            "precondition: {} must not exist in production DB",
            GROUP_A
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
            0,
            "precondition: {} must not exist in production DB",
            GROUP_B
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "precondition: {} must not exist in production DB",
            WS1
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
            0,
            "precondition: {} must not exist in production DB",
            WS2
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);
    assert!(!workspace_exists_in_sway(WS2), "precondition: {} must not exist in sway", WS2);

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    eprintln!("  Original group: '{}'", orig_group);
    eprintln!("  Original workspace: '{}'", orig_ws);

    // --- Init ---
    fixture.init().success();

    // --- Setup: create groups A and B, add workspaces, set up memberships ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS2, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    // --- Verify setup ---
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "group '{}' exists",
        GROUP_A
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
        1,
        "group '{}' exists",
        GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group '{}'",
        WS1, GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} is in group '{}'",
        WS1, GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS2, GROUP_A),
        1,
        "{} is in group '{}'",
        WS2, GROUP_A
    );
    assert!(workspace_exists_in_sway(WS1), "{} is in sway", WS1);
    assert!(workspace_exists_in_sway(WS2), "{} is in sway", WS2);

    // --- Test: delete without --force should fail (group still exists) ---
    fixture
        .swayg(&["group", "delete", GROUP_A])
        .failure();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "{} NOT deleted (no --force)",
        GROUP_A
    );

    // --- Test: delete with --force ---
    fixture
        .swayg(&["group", "delete", GROUP_A, "--force"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        0,
        "{} deleted",
        GROUP_A
    );

    // --- Verify: WS1 (multi-group) still in Group B, WS2 (single-group) moved to group 0 ---
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} still in group '{}' (had other membership)",
        WS1, GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, "0"),
        0,
        "{} NOT in group '0' (still in Group B)",
        WS1
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS2, "0"),
        1,
        "{} moved to group '0' (was only in Group A)",
        WS2
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        1,
        "{} still in DB",
        WS1
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
        1,
        "{} still in DB",
        WS2
    );
    assert!(workspace_exists_in_sway(WS1), "{} still in sway", WS1);
    assert!(workspace_exists_in_sway(WS2), "{} still in sway", WS2);

    // --- Verify visibility in Group B ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    let visible_gb = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        visible_gb.lines().any(|l| l.contains(WS1)),
        "{} visible in Group B",
        WS1
    );
    assert!(
        !visible_gb.lines().any(|l| l.contains(WS2)),
        "{} NOT visible in Group B (moved to group 0)",
        WS2
    );

    // --- Verify visibility in group 0 ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    let visible_0 = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        visible_0.lines().any(|l| l.contains(WS2)),
        "{} visible in group '0'",
        WS2
    );

    // --- Switch back to original group ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    // --- Cleanup: kill dummy windows ---
    drop(_win1);
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Switch to Group B then back (auto-delete Group B)
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
        0,
        "{} auto-deleted",
        GROUP_B
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP_A, GROUP_B),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')", WS1, WS2),
    );
    let wsgrp_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('{}', '{}')",
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
