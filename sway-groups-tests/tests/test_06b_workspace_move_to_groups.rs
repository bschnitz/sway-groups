use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{get_focused_workspace, DummyWindowHandle, TestFixture};

const GROUP_A: &str = "zz_test_grp_a_06b";
const GROUP_B: &str = "zz_test_grp_b_06b";
const GROUP_C: &str = "zz_test_grp_c_06b";
const WS1: &str = "zz_test_ws1_06b";

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
async fn test_06b_workspace_move_to_groups() {
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
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_C)),
            0,
            "precondition: {} must not exist in production DB",
            GROUP_C
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "precondition: {} must not exist in production DB",
            WS1
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    eprintln!("  Original group: '{}'", orig_group);
    eprintln!("  Original workspace: '{}'", orig_ws);

    // --- Init ---
    fixture.init().success();

    // --- Setup: create groups A and B, add workspace to both ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_A, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_B, "--create"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1])
        .success();

    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
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
    assert!(workspace_exists_in_sway(WS1), "{} is in sway", WS1);

    // --- Test: move WS1 to Group C (doesn't exist, should auto-create) ---
    fixture
        .swayg(&["workspace", "move", WS1, "--groups", GROUP_C])
        .success();

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        0,
        "{} NOT in group '{}' (removed from all)",
        WS1, GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        0,
        "{} NOT in group '{}' (removed from all)",
        WS1, GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_C),
        1,
        "{} is in group '{}'",
        WS1, GROUP_C
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_C)),
        1,
        "group '{}' auto-created",
        GROUP_C
    );

    // --- Test: move WS1 to Group A and Group B (comma-separated) ---
    fixture
        .swayg(&["workspace", "move", WS1, "--groups", &format!("{},{}", GROUP_A, GROUP_B)])
        .success();

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
        ws_in_group_count(&fixture.db_path, WS1, GROUP_C),
        0,
        "{} NOT in group '{}' (removed)",
        WS1, GROUP_C
    );

    // --- Cleanup ---
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();

    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS1), "{} is gone from sway after kill", WS1);

    // Auto-delete Group C
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_C])
        .success();
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_C)),
        0,
        "{} auto-deleted",
        GROUP_C
    );

    // Auto-delete Group A
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_A])
        .success();
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        0,
        "{} auto-deleted",
        GROUP_A
    );

    // Auto-delete Group B
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP_B])
        .success();
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
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
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}')",
            GROUP_A, GROUP_B, GROUP_C
        ),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1),
    );
    let wsgrp_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('{}', '{}', '{}')",
            GROUP_A, GROUP_B, GROUP_C
        ),
    );
    assert_eq!(
        (group_gone, ws_gone, wsgrp_gone),
        (0, 0, 0),
        "no test data remains in DB"
    );
}
