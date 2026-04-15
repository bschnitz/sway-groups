use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, db_exec, get_focused_workspace, orig_active_group, swayg_output,
    ws_in_group_count, TestFixture,
};

const GROUP_A: &str = "zz_test_grp_a_07";
const GROUP_B: &str = "zz_test_grp_b_07";
const GROUP_RENAMED: &str = "zz_test_grp_a_renamed_07";
const WS1: &str = "zz_test_ws1_07";

#[tokio::test]
async fn test_07_group_rename() {
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
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_RENAMED)),
            0,
            "precondition: {} must not exist in production DB",
            GROUP_RENAMED
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "precondition: {} must not exist in production DB",
            WS1
        );
    }

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    eprintln!("  Original group: '{}'", orig_group);
    eprintln!("  Original workspace: '{}'", orig_ws);

    // --- Init ---
    fixture.init().success();

    // --- Setup: create test groups, workspace, memberships, group_state via direct DB inserts ---
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO groups (name, created_at, updated_at) VALUES ('{}', datetime('now'), datetime('now'));",
            GROUP_A
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO groups (name, created_at, updated_at) VALUES ('{}', datetime('now'), datetime('now'));",
            GROUP_B
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!(
            "UPDATE outputs SET active_group = '{}' WHERE name = '{}';",
            GROUP_A, fixture.orig_output
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO workspaces (name, is_global, created_at, updated_at) VALUES ('{}', 0, datetime('now'), datetime('now'));",
            WS1
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO workspace_groups (workspace_id, group_id, created_at) \
             SELECT w.id, g.id, datetime('now') FROM workspaces w, groups g \
             WHERE w.name = '{}' AND g.name = '{}';",
            WS1, GROUP_A
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO group_state (output, group_name, last_focused_workspace, last_visited) \
             VALUES ('{}', '{}', '{}', datetime('now'));",
            fixture.orig_output, GROUP_A, WS1
        ),
    );

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
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        1,
        "'{}' in DB",
        WS1
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' in group '{}'",
        WS1, GROUP_A
    );

    let active = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active, GROUP_A, "output active_group = '{}'", GROUP_A);

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE output = '{}' AND group_name = '{}'",
                fixture.orig_output, GROUP_A
            )
        ),
        1,
        "group_state entry for '{}' exists",
        GROUP_A
    );

    // --- Test: rename A -> A_renamed (success) ---
    fixture
        .swayg(&["group", "rename", GROUP_A, GROUP_RENAMED])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        0,
        "'{}' gone from DB",
        GROUP_A
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_RENAMED)),
        1,
        "'{}' exists in DB",
        GROUP_RENAMED
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_RENAMED),
        1,
        "'{}' membership updated to '{}'",
        WS1, GROUP_RENAMED
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        0,
        "'{}' NOT in old group name '{}'",
        WS1, GROUP_A
    );

    let active_after = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active_after, GROUP_RENAMED, "output active_group updated to '{}'", GROUP_RENAMED);

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE output = '{}' AND group_name = '{}'",
                fixture.orig_output, GROUP_RENAMED
            )
        ),
        1,
        "group_state updated to '{}'",
        GROUP_RENAMED
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE output = '{}' AND group_name = '{}'",
                fixture.orig_output, GROUP_A
            )
        ),
        0,
        "group_state old entry for '{}' gone",
        GROUP_A
    );

    let ws_list = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain", "--group", GROUP_RENAMED],
    );
    assert!(
        ws_list.lines().any(|l| l.contains(WS1)),
        "'{}' listed in renamed group via workspace list",
        WS1
    );

    // --- Test: rename to existing name (error) ---
    fixture
        .swayg(&["group", "rename", GROUP_RENAMED, GROUP_B])
        .failure();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_RENAMED)),
        1,
        "'{}' NOT renamed (target exists)",
        GROUP_RENAMED
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
        1,
        "'{}' unchanged",
        GROUP_B
    );

    // --- Test: rename nonexistent group (error) ---
    fixture
        .swayg(&["group", "rename", "nonexistent_zz_test__", GROUP_RENAMED])
        .failure();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_RENAMED)),
        1,
        "'{}' unchanged (nonexistent source)",
        GROUP_RENAMED
    );
    assert_eq!(
        db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = 'nonexistent_zz_test__'"),
        0,
        "no group created for nonexistent source"
    );

    // --- Test: rename group "0" (now allowed — no special protection) ---
    fixture
        .swayg(&["group", "rename", "0", "should_work_zz__"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = '0'"),
        0,
        "group '0' renamed away"
    );
    assert_eq!(
        db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = 'should_work_zz__'"),
        1,
        "'should_work_zz__' created from renamed '0'"
    );

    // --- Cleanup: switch back to original workspace ---
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

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
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}', 'should_work_zz__')",
            GROUP_A, GROUP_B, GROUP_RENAMED
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
            GROUP_A, GROUP_B, GROUP_RENAMED
        ),
    );
    assert_eq!(
        (group_gone, ws_gone, wsgrp_gone),
        (0, 0, 0),
        "no test data remains in DB"
    );
}
