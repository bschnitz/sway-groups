use sway_groups_tests::common::{
    db_count, get_focused_workspace, swayg_live, swayg_output, workspace_count_in_sway,
    workspace_of_window, ws_in_group_count, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_group_a";
const GROUP_B: &str = "zz_test_group_b";
const WS1: &str = "zz_test_ws1_cmv";
const WS2: &str = "zz_test_ws2_cmv";

#[tokio::test]
async fn test_05b_multi_group_container_move() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let orig_group = sway_groups_tests::common::orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
            0, "{} must not exist in production DB", GROUP_A
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
            0, "{} must not exist in production DB", GROUP_B
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0, "{} must not exist in production DB", WS1
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS2)),
            0, "{} must not exist in production DB", WS2
        );
    }

    assert_eq!(
        workspace_count_in_sway(WS1),
        0,
        "precondition: {} must not exist in sway",
        WS1
    );
    assert_eq!(
        workspace_count_in_sway(WS2),
        0,
        "precondition: {} must not exist in sway",
        WS2
    );

    // --- Init ---
    fixture.init().success();

    // --- Create Group A ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_A,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group '{}' was created",
        GROUP_A
    );

    let active = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active, GROUP_A, "active group = '{}'", GROUP_A);

    // --- Launch dummy window WS1 ---
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    // --- Move container to WS1 (creates WS1 in sway, adds to Group A) ---
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on '{}'",
        WS1
    );

    assert_eq!(
        workspace_of_window(WS1).as_deref(),
        Some(WS1),
        "window '{}' is on workspace '{}'",
        WS1, WS1
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
        ),
        1,
        "{} still exactly 1 row in DB",
        WS1
    );

    // container move does NOT change group assignments for existing workspaces
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in group '{}'",
        WS1, GROUP_A
    );

    // --- Switch to Group B ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_B,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    let active_b = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active_b, GROUP_B, "active group = '{}'", GROUP_B);

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "{} NOT auto-deleted (still has {})",
        GROUP_A, WS1
    );

    // --- Launch dummy window WS2 ---
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn dummy window WS2");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS2).is_some(),
        "dummy window '{}' exists in sway tree",
        WS2
    );

    // --- container move to WS1 (existing in Group A, not in Group B) ---
    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on '{}'",
        WS1
    );

    assert_eq!(
        workspace_of_window(WS2).as_deref(),
        Some(WS1),
        "window '{}' moved to workspace '{}'",
        WS2, WS1
    );

    // Guard block updates active_group to GROUP_A when switching to WS1 (cross-group move)
    let active_after_move = swayg_output(
        &fixture.db_path,
        &["group", "active", &fixture.orig_output],
    );
    assert_eq!(active_after_move, GROUP_A, "active group updated to GROUP_A (guard block)");

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
        ),
        1,
        "{} still exactly 1 row in DB",
        WS1
    );

    // container move does NOT change group assignments for existing workspaces
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in group '{}'",
        WS1, GROUP_A
    );

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        0,
        "{} NOT added to group '{}' by container move",
        WS1, GROUP_B
    );

    assert_eq!(
        workspace_count_in_sway(WS1),
        1,
        "{} exists exactly once in sway",
        WS1
    );

    // active_group is now GROUP_A, so WS1 IS visible
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
    assert!(
        visible.lines().any(|l| l.contains(WS1)),
        "{} IS visible (active_group = GROUP_A, WS1 in GROUP_A)",
        WS1
    );

    // --- Switch back: first to GROUP_B to trigger its auto-delete ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_B,
            "--output",
            &fixture.orig_output,
        ])
        .success();

    // GROUP_B is empty → auto-deleted when switching away
    fixture
        .swayg(&[
            "group",
            "select",
            "0",
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        0,
        "{} auto-deleted (empty)",
        GROUP_B
    );

    // --- Kill dummy windows ---
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

    // --- Auto-delete Group A (empty after window kill) ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            "0",
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        0,
        "'{}' auto-deleted",
        GROUP_A
    );

    // --- Post-condition: no test data remains ---
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
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
            WS1, WS2
        ),
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
