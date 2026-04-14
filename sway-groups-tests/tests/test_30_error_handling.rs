use std::path::PathBuf;
use std::process::{Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;
use sway_groups_tests::common::{get_focused_workspace, swayg_live, DummyWindowHandle, TestFixture};

const GROUP_A: &str = "zz_test_grp_err_a_30";
const GROUP_B: &str = "zz_test_grp_err_b_30";
const WS1: &str = "zz_test_ws_err_30";

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

fn swayg_stderr(db_path: &PathBuf, args: &[&str]) -> String {
    let output = Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .arg("--db")
        .arg(db_path)
        .args(args)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .expect("swayg command failed");
    String::from_utf8_lossy(&output.stderr).to_string()
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
async fn test_30_error_handling() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = get_focused_workspace().expect("get focused workspace");
    let orig_group = {
        let out = Command::new("swayg")
            .args(["group", "active", &fixture.orig_output])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .expect("swayg group active failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP_A, GROUP_B)),
            0,
            "precondition: test groups must not exist in production DB"
        );
    }
    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);

    // --- Init ---
    fixture.init().success();

    // --- Test: group create "" → validation error (new validation added during refactoring) ---
    fixture.swayg(&["group", "create", ""]).failure();

    let stderr = swayg_stderr(&fixture.db_path, &["group", "create", ""]);
    assert!(
        stderr.contains("must not be empty"),
        "error message contains 'must not be empty' (stderr: {})",
        stderr
    );

    assert_eq!(
        db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = ''"),
        0,
        "no empty-name group created in DB"
    );

    // --- Setup for remaining tests: create GROUP_A, spawn WS1, add to group ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert!(workspace_exists_in_sway(WS1), "{} must exist in sway", WS1);
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group '{}'",
        WS1, GROUP_A
    );

    // --- Test: workspace add WS --group <explicit> ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1, "--group", GROUP_B])
        .success();

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in '{}' (membership preserved)",
        WS1, GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} now also in '{}' via explicit --group",
        WS1, GROUP_B
    );

    // --- Test: workspace remove WS --group <explicit> ---
    fixture
        .swayg(&["workspace", "remove", WS1, "--group", GROUP_B])
        .success();

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        0,
        "{} removed from '{}' via explicit --group",
        WS1, GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in '{}' (other membership untouched)",
        WS1, GROUP_A
    );

    // --- Test: group delete without --force on non-empty group ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture.swayg(&["group", "delete", GROUP_A]).failure();

    let stderr_delete = swayg_stderr(&fixture.db_path, &["group", "delete", GROUP_A]);
    assert!(
        stderr_delete.to_lowercase().contains("force"),
        "error message hints at --force (stderr: {})",
        stderr_delete
    );

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "{} NOT deleted (no --force)",
        GROUP_A
    );

    // --- Cleanup: kill dummy window, auto-delete GROUP_A ---
    // GROUP_B was auto-deleted already when we switched to GROUP_A above.
    // Switching to "0" first keeps GROUP_A alive (WS1 still in sway).
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Switch to GROUP_A then away: GROUP_A becomes effectively empty → auto-delete.
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP_A, GROUP_B),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        0,
        "no test workspace remains"
    );

    // --- Restore original state ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
