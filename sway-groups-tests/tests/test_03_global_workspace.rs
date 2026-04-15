use std::process::Command;

use sway_groups_tests::common::{
    TestFixture, DummyWindowHandle, get_focused_workspace, swayg_live, swayg_output,
    db_count, db_query, orig_active_group, workspace_count_in_sway, window_count_in_tree,
    output_contains,
};

const TEST_GROUP: &str = "zz_test_global";
const WS1: &str = "zz_test_ws1_glo";
const WS2: &str = "zz_test_ws2_glo";

fn get_active_group(db_path: &std::path::PathBuf, output: &str) -> String {
    swayg_output(db_path, &["group", "active", output])
}

#[tokio::test]
async fn test_03_global_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // --- Precondition checks on REAL db ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
            0,
            "precondition: test group must not exist in real DB"
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "precondition: WS1 must not exist in real DB"
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
            0,
            "precondition: WS2 must not exist in real DB"
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "precondition: WS1 must not exist in sway"
    );
    assert_eq!(
        workspace_count_in_sway(WS2), 0,
        "precondition: WS2 must not exist in sway"
    );

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- 1. Init fresh DB ---
    fixture.init().success();

    let ag_after_init = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after init: active_group = {:?}", ag_after_init);
    assert_eq!(ag_after_init, "", "after init: active_group should be empty (none set)");

    // --- 2. Select test group (with --create) ---
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    let ag_after_select = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select {}: active_group = {:?}", TEST_GROUP, ag_after_select);
    assert_eq!(ag_after_select, TEST_GROUP, "after group select: active_group should be TEST_GROUP");

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        1,
        "group was created"
    );

    // --- 3. Launch dummy window WS1 and move to workspace ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    let ag_after_move1 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after container move WS1 --switch: active_group = {:?}", ag_after_move1);
    assert_eq!(ag_after_move1, TEST_GROUP, "after container move WS1: active_group should still be TEST_GROUP");

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on WS1 after container move"
    );

    // --- 4. Launch dummy window WS2 and move to workspace ---
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS2, "--switch-to-workspace"])
        .success();

    let ag_after_move2 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after container move WS2 --switch: active_group = {:?}", ag_after_move2);
    assert_eq!(ag_after_move2, TEST_GROUP, "after container move WS2: active_group should still be TEST_GROUP");

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS2,
        "focused on WS2 after container move"
    );

    // --- 5. Set WS1 as global ---
    fixture
        .swayg(&["workspace", "global", WS1])
        .success();

    let ag_after_global1 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after workspace global WS1: active_group = {:?}", ag_after_global1);

    let ws1_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_global, "1", "WS1 is global in DB");

    // --- 6. Switch back to original group ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    let ag_after_switch_back = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select 0: active_group = {:?}", ag_after_switch_back);

    // --- 7. Verify global visibility ---
    let visible = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible, WS1),
        "WS1 is visible in group 0 (global)"
    );
    assert!(
        !output_contains(&visible, WS2),
        "WS2 is NOT visible in group 0 (not global)"
    );

    // --- 8. Verify group membership ---
    let group_ws = swayg_output(
        &fixture.db_path,
        &[
            "workspace", "list", "--plain", "--group", TEST_GROUP,
            "--output", &fixture.orig_output,
        ],
    );
    assert!(
        !output_contains(&group_ws, WS1),
        "WS1 is NOT in test group (global, no group membership)"
    );
    assert!(
        output_contains(&group_ws, WS2),
        "WS2 is visible in test group"
    );

    // --- 9. Unglobal WS1 ---
    fixture
        .swayg(&["workspace", "unglobal", WS1])
        .success();

    let ag_after_unglobal = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after workspace unglobal WS1: active_group = {:?}", ag_after_unglobal);

    let ws1_not_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_not_global, "0", "WS1 is no longer global");

    // --- 10. WS1 visible in active group after unglobal ---
    let visible_after_unglobal = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&visible_after_unglobal, WS1),
        "WS1 is visible in group 0 after unglobal"
    );

    // --- 11a. Auto-delete: switch from global workspace ---
    // Switch to test group
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output])
        .success();

    let ag_after_select2 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select TEST_GROUP (11a): active_group = {:?}", ag_after_select2);

    // Set WS2 as global
    fixture
        .swayg(&["workspace", "global", WS2])
        .success();

    let ag_after_global2 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after workspace global WS2: active_group = {:?}", ag_after_global2);

    let ws2_global: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS2),
    );
    assert_eq!(ws2_global, "1", "WS2 is global in DB");

    // Kill dummy window WS1
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1), 0,
        "dummy window WS1 is gone"
    );

    // Switch to WS2 (let sway auto-delete empty WS1)
    let _ = Command::new("swaymsg")
        .args(["workspace", WS2])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        workspace_count_in_sway(WS1), 0,
        "WS1 is gone from sway"
    );

    // Test group still exists (has global workspaces)
    let groups = swayg_output(
        &fixture.db_path,
        &["group", "list", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&groups, TEST_GROUP),
        "test group still exists (has global workspaces)"
    );

    // Switch back from global workspace (should auto-delete test group)
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    let ag_after_autodel1 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select 0 (auto-del 1): active_group = {:?}", ag_after_autodel1);

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        0,
        "test group auto-deleted (switched from global workspace)"
    );

    // --- 11b. Auto-delete: switch from empty workspace (only global workspaces remain) ---

    // Create test group again
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    let ag_after_recreate = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select TEST_GROUP --create (11b): active_group = {:?}", ag_after_recreate);

    // Launch dummy window WS1 again
    let _win1b = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1 (again)");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    let ag_after_move1b = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after container move WS1 --switch (11b): active_group = {:?}", ag_after_move1b);
    assert_eq!(ag_after_move1b, TEST_GROUP, "after container move WS1 (11b): active_group stays as TEST_GROUP (WS1 is global, no group reassignment)");

    fixture
        .swayg(&["workspace", "global", WS1])
        .success();

    let ag_after_global1b = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after workspace global WS1 (11b): active_group = {:?}", ag_after_global1b);

    let ws1_global_b: String = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(ws1_global_b, "1", "WS1 is global in DB (second time)");

    let groups_b = swayg_output(
        &fixture.db_path,
        &["group", "list", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&groups_b, TEST_GROUP),
        "test group still exists (has global workspace)"
    );

    // active_group is "0" (set by guard block), so we need to switch to TEST_GROUP first,
    // then back to "0" to trigger auto-delete
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output])
        .success();

    let ag_after_reselect = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select TEST_GROUP (re-select, 11b): active_group = {:?}", ag_after_reselect);
    assert_eq!(ag_after_reselect, TEST_GROUP, "after re-select TEST_GROUP: active_group should be TEST_GROUP");

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    let ag_after_autodel2 = get_active_group(&fixture.db_path, &fixture.orig_output);
    eprintln!("[DEBUG] after group select 0 (auto-del 2): active_group = {:?}", ag_after_autodel2);

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        0,
        "test group auto-deleted (switched from empty workspace, only global remained)"
    );

    // --- Cleanup: kill remaining windows ---
    drop(_win2);
    drop(_win1b);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(window_count_in_tree(WS1), 0, "WS1 window is gone after cleanup");
    assert_eq!(window_count_in_tree(WS2), 0, "WS2 window is gone after cleanup");

    // --- Post-condition: sync DB and verify no test data ---
    fixture.init().success();

    let group_gone = db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP));
    let ws_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS1, WS2
        ),
    )
    .trim()
    .parse::<i64>()
    .unwrap_or(0);
    let wsgrp_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '{}'",
            TEST_GROUP
        ),
    )
    .trim()
    .parse::<i64>()
    .unwrap_or(0);

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
