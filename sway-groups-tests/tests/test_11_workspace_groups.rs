use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, swayg_output, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_ga";
const GROUP_B: &str = "zz_test_gb";
const GROUP_C: &str = "zz_test_gc";
const WS1: &str = "zz_tg_ws1";

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
async fn test_11_workspace_groups() {
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
        for g in [GROUP_A, GROUP_B, GROUP_C] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0,
                "{} must not exist in production DB",
                g
            );
        }
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
            ),
            0,
            "{} must not exist in production DB",
            WS1
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "{} must not exist in sway", WS1);

    // --- Setup: init + create groups A and C + kitty + move + add to group B ---
    fixture.init().success();

    fixture
        .swayg(&[
            "group",
            "select",
            &fixture.orig_output,
            GROUP_A,
            "--create",
        ])
        .success();

    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();

    let _win = DummyWindowHandle::spawn(WS1).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1, "--group", GROUP_B])
        .success();

    fixture
        .swayg(&["group", "create", GROUP_C])
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
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group '{}' exists",
        GROUP_A
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        1,
        "group '{}' exists",
        GROUP_B
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_C)
        ),
        1,
        "group '{}' exists",
        GROUP_C
    );

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' in group '{}'",
        WS1,
        GROUP_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "'{}' in group '{}'",
        WS1,
        GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_C),
        0,
        "'{}' NOT in group '{}'",
        WS1,
        GROUP_C
    );

    // --- Test: workspace groups WS1 ---
    fixture
        .swayg(&["workspace", "groups", WS1])
        .success();

    let groups_out = swayg_output(&fixture.db_path, &["workspace", "groups", WS1]);

    assert!(
        groups_out.contains(GROUP_A),
        "output contains '{}'",
        GROUP_A
    );
    assert!(
        groups_out.contains(GROUP_B),
        "output contains '{}'",
        GROUP_B
    );
    assert!(
        !groups_out.contains(GROUP_C),
        "output does NOT contain '{}'",
        GROUP_C
    );

    // --- Cleanup: kill dummy window ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS1), "dummy window '{}' is gone", WS1);

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

    // Auto-delete all test groups
    for g in [GROUP_A, GROUP_B, GROUP_C] {
        fixture
            .swayg(&["group", "select", &fixture.orig_output, g])
            .success();
        fixture
            .swayg(&[
                "group",
                "select",
                &fixture.orig_output,
                &orig_group,
            ])
            .success();
    }

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}')",
                GROUP_A, GROUP_B, GROUP_C
            )
        ),
        0,
        "all test groups auto-deleted"
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
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             WHERE g.name IN ('{}', '{}', '{}')",
            GROUP_A, GROUP_B, GROUP_C
        ),
    );
    assert_eq!(group_gone, 0, "no test groups remain");
    assert_eq!(ws_gone, 0, "no test workspaces remain");
    assert_eq!(wsgrp_gone, 0, "no test workspace_groups remain");
}
