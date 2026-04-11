use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_nav";
const WS_A: &str = "zz_tg_alpha";
const WS_B: &str = "zz_tg_beta";
const WS_C: &str = "zz_tg_gamma";

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

fn workspace_in_group_count(db_path: &std::path::PathBuf, ws: &str, group: &str) -> i64 {
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
async fn test_08_nav_next_prev() {
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
        for ws in [WS_A, WS_B, WS_C] {
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

    for ws in [WS_A, WS_B, WS_C] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Setup: init + create group + launch 3 dummies + move containers + focus alpha ---
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

    let _win_c = DummyWindowHandle::spawn(WS_C).expect("spawn dummy window C");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_C, "--switch-to-workspace"])
        .success();

    Command::new("swaymsg")
        .args(["workspace", WS_A])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace failed");
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

    for ws in [WS_A, WS_B, WS_C] {
        assert!(workspace_exists_in_sway(ws), "'{}' exists in sway", ws);
    }

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_A, GROUP),
        1,
        "'{}' in group '{}'",
        WS_A,
        GROUP
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_B, GROUP),
        1,
        "'{}' in group '{}'",
        WS_B,
        GROUP
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_C, GROUP),
        1,
        "'{}' in group '{}'",
        WS_C,
        GROUP
    );

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
    assert!(visible.contains(WS_A), "'{}' in visible list", WS_A);
    assert!(visible.contains(WS_B), "'{}' in visible list", WS_B);
    assert!(visible.contains(WS_C), "'{}' in visible list", WS_C);

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "focused on '{}'",
        WS_A
    );

    // --- Test: nav next (alpha → beta) ---
    fixture
        .swayg(&["nav", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focused on '{}' after nav next",
        WS_B
    );

    // --- Test: nav next (beta → gamma) ---
    fixture
        .swayg(&["nav", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_C,
        "focused on '{}' after nav next",
        WS_C
    );

    // --- Test: nav next without wrap at boundary (gamma → stays) ---
    fixture
        .swayg(&["nav", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_C,
        "still on '{}' (no wrap, at boundary)",
        WS_C
    );

    // --- Test: nav next with wrap at boundary (gamma → alpha) ---
    fixture
        .swayg(&[
            "nav",
            "next",
            "--output",
            &fixture.orig_output,
            "--wrap",
        ])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "focused on '{}' after wrap",
        WS_A
    );

    // --- Position on beta via swaymsg for prev tests ---
    Command::new("swaymsg")
        .args(["workspace", WS_B])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace failed");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focused on '{}'",
        WS_B
    );

    // --- Test: nav prev (beta → alpha) ---
    fixture
        .swayg(&["nav", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "focused on '{}' after nav prev",
        WS_A
    );

    // --- Test: nav prev without wrap at boundary (alpha → stays) ---
    fixture
        .swayg(&["nav", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "still on '{}' (no wrap, at boundary)",
        WS_A
    );

    // --- Test: nav prev with wrap at boundary (alpha → gamma) ---
    fixture
        .swayg(&[
            "nav",
            "prev",
            "--output",
            &fixture.orig_output,
            "--wrap",
        ])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_C,
        "focused on '{}' after wrap prev",
        WS_C
    );

    // --- Cleanup: switch back to original group ---
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

    // Kill dummy windows
    drop(_win_a);
    drop(_win_b);
    drop(_win_c);
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
    assert!(
        workspace_of_window(WS_C).is_none(),
        "dummy window '{}' is gone",
        WS_C
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

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}', '{}')",
            WS_A, WS_B, WS_C
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
