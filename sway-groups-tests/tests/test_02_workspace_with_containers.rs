use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{get_focused_workspace, workspace_of_window, DummyWindowHandle, TestFixture};

const GROUP: &str = "zz_test_ws_containers";
const WS1: &str = "zz_test_ws1_cnt";
const WS2: &str = "zz_test_ws2_cnt";

fn db_count(db_path: &PathBuf, sql: &str) -> i64 {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0)
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
async fn test_02_workspace_with_containers() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
            0,
            "test group must not exist in production DB"
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "WS1 must not exist in production DB"
        );
    assert_eq!(
        db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
        0,
        "WS2 must not exist in production DB"
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);
    assert!(!workspace_exists_in_sway(WS2), "precondition: {} must not exist in sway", WS2);

    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    eprintln!("  Original group: '{}'", orig_group);
    eprintln!("  Original workspace: '{}'", orig_ws);

    // --- Init ---
    fixture.init().success();

    // --- Select test group with --create ---
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        1,
        "group was created"
    );

    let active = sway_groups_tests::common::swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP, "active group changed to test group");

    // --- Launch dummy window with app_id WS1 ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    // --- Move container to WS1 and switch ---
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on WS1"
    );

    assert_eq!(
        workspace_of_window(WS1).as_deref(),
        Some(WS1),
        "window '{}' is on workspace '{}'",
        WS1, WS1
    );

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        1,
        "WS1 is in DB"
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
                WS1, GROUP
            )
        ),
        1,
        "WS1 is in group"
    );

    // --- Launch dummy window with app_id WS2 ---
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS2).is_some(),
        "dummy window '{}' exists in sway tree",
        WS2
    );

    // --- Move container to WS2 and switch ---
    fixture
        .swayg(&["container", "move", WS2, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS2,
        "focused on WS2"
    );

    assert_eq!(
        workspace_of_window(WS2).as_deref(),
        Some(WS2),
        "window '{}' is on workspace '{}'",
        WS2, WS2
    );

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
        1,
        "WS2 is in DB"
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{}' AND g.name = '{}'",
                WS2, GROUP
            )
        ),
        1,
        "WS2 is in group"
    );

    // --- Switch back to default group on test DB ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        1,
        "test group NOT auto-deleted (still has workspaces)"
    );

    // --- Kill test windows ---
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

    // --- Ensure test workspaces are gone from sway (switch away to trigger auto-remove) ---
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
    for ws_name in [WS1, WS2] {
        if workspace_exists_in_sway(ws_name) {
            let _ = std::process::Command::new("swaymsg")
                .args(["workspace", ws_name])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = std::process::Command::new("swaymsg")
                .args(["workspace", &orig_ws])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    assert!(!workspace_exists_in_sway(WS1), "cleanup: {} gone from sway", WS1);
    assert!(!workspace_exists_in_sway(WS2), "cleanup: {} gone from sway", WS2);

    // --- Switch to test group then back (auto-delete on test DB) ---
    // Note: GROUP may already be auto-deleted by the daemon during window cleanup.
    // --create ensures GROUP exists for the auto-delete trigger.
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    // Switch away to trigger auto-delete of the test group
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0,
        "test group was auto-deleted"
    );

    // --- Cleanup: restore original group on live DB ---
    use sway_groups_tests::common::swayg_live;
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // --- Post-condition: no test data remains ---
    let group_gone = db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP));
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')", WS1, WS2),
    );
    let ws_group_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '{}'",
            GROUP
        ),
    );

    assert_eq!(group_gone, 0, "no test group remains");
    assert_eq!(ws_gone, 0, "no test workspaces remain");
    assert_eq!(ws_group_gone, 0, "no test workspace_groups remain");
}
