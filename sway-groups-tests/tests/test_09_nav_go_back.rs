use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_nav2";
const WS_A: &str = "zz_tg_one";
const WS_B: &str = "zz_tg_two";

fn db_count(db_path: &std::path::PathBuf, sql: &str) -> i64 {
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
async fn test_09_nav_go_back() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let orig_group = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir().unwrap_or_default().join("swayg").join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
            0,
            "{} must not exist in production DB",
            GROUP
        );
        for ws in [WS_A, WS_B] {
            assert_eq!(
                db_count(
                    &real_db,
                    &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)
                ),
                0,
                "{} must not exist in production DB",
                ws
            );
        }
    }

    for ws in [WS_A, WS_B] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Setup: init + create group + launch 2 dummies + move containers + switch back ---
    fixture.init().success();

    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            GROUP,
            "--create",
        ])
        .success();

    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn dummy window A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();

    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn dummy window B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_B, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            &orig_group,
        ])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' exists",
        GROUP
    );

    assert!(_win_a.exists_in_tree(), "dummy window A is running");
    assert!(_win_b.exists_in_tree(), "dummy window B is running");

    for ws in [WS_A, WS_B] {
        assert!(workspace_exists_in_sway(ws), "'{}' exists in sway", ws);
    }

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- Test: nav go WS_A ---
    fixture.swayg(&["nav", "go", WS_A]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "focused on '{}' after nav go",
        WS_A
    );

    // --- Test: nav go WS_B ---
    fixture.swayg(&["nav", "go", WS_B]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focused on '{}' after nav go",
        WS_B
    );

    // --- Test: nav back (two → one) ---
    fixture.swayg(&["nav", "back"]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "focused on '{}' after nav back",
        WS_A
    );

    // --- Test: nav back (one → two, alternation) ---
    fixture.swayg(&["nav", "back"]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focused on '{}' after nav back (alternation)",
        WS_B
    );

    // --- Test: nav go original workspace ---
    fixture.swayg(&["nav", "go", &orig_ws]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on '{}' after nav go",
        orig_ws
    );

    // --- Test: nav back (orig → two) ---
    fixture.swayg(&["nav", "back"]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focused on '{}' after nav back (from orig)",
        WS_B
    );

    // --- Test: nav back (two → orig) ---
    fixture.swayg(&["nav", "back"]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on '{}' after nav back (from two)",
        orig_ws
    );

    // --- Cleanup: kill dummy windows ---
    drop(_win_a);
    drop(_win_b);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        workspace_of_window(WS_A).is_none(),
        "dummy window '{}' is gone",
        WS_A
    );
    assert!(
        workspace_of_window(WS_B).is_none(),
        "dummy window '{}' is gone",
        WS_B
    );

    // Auto-delete GROUP
    fixture
        .swayg(&["group", "select", &fixture.orig_output, GROUP])
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
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "'{}' auto-deleted",
        GROUP
    );

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS_A, WS_B
        ),
    );
    let wsgrp_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             WHERE g.name = '{}'",
            GROUP
        ),
    );
    assert_eq!(group_gone, 0, "no test groups remain");
    assert_eq!(ws_gone, 0, "no test workspaces remain");
    assert_eq!(wsgrp_gone, 0, "no test workspace_groups remain");
}
