use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, swayg_output, workspace_of_window,
    ws_in_group_count, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_cm_og_a";
const GROUP_B: &str = "zz_test_cm_og_b";
const WS_A: &str = "zz_tg_cm_og_wsa";
const WS_B: &str = "zz_tg_cm_og_wsb";

#[tokio::test]
async fn test_26_container_move_switch_to_other_group() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        for g in [GROUP_A, GROUP_B] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0
            );
        }
        for ws in [WS_A, WS_B] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0
            );
        }
    }

    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group not empty");
    let orig_ws = get_focused_workspace().expect("focused ws");

    fixture.init().success();

    // Setup: GROUP_A with WS_A, GROUP_B with WS_B
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS_A, "--switch-to-workspace"]).success();

    assert_eq!(get_focused_workspace().unwrap(), WS_A, "on WS_A");
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_A, GROUP_A), 1, "WS_A in GROUP_A");

    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn WS_B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS_B, "--switch-to-workspace"]).success();

    assert_eq!(get_focused_workspace().unwrap(), WS_B, "on WS_B");
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_B, GROUP_B), 1, "WS_B in GROUP_B");

    // Switch back to GROUP_A, focus WS_A
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();
    fixture.swayg(&["nav", "go", WS_A]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(get_focused_workspace().unwrap(), WS_A, "back on WS_A");

    // --- Test: container move --switch-to-workspace to WS_B (in GROUP_B) ---
    // Create a new window and move it to WS_B (cross-group)
    let _win_mover = DummyWindowHandle::spawn("zz_tg_cm_og_mover").expect("spawn mover");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture.swayg(&["container", "move", WS_B, "--switch-to-workspace"]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Sway: focused on WS_B
    assert_eq!(get_focused_workspace().unwrap(), WS_B, "focused on WS_B");

    // Sway: container moved to WS_B
    // (The mover window's app_id is zz_tg_cm_og_mover but it's on workspace WS_B)
    let mover_ws = workspace_of_window("zz_tg_cm_og_mover");
    assert_eq!(mover_ws.as_deref(), Some(WS_B), "mover window on WS_B");

    // DB: active group on orig_output should be GROUP_B (because we switched to WS_B)
    // The window was moved to WS_B which is in GROUP_B.
    // navigate_to_workspace (from --switch-to-workspace via focus_workspace) should have
    // updated the active group to GROUP_B and the workspace should be properly tracked.
    let active = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active, GROUP_B, "active group = GROUP_B after switching to WS_B");

    // The mover window's workspace association:
    // WS_B should be in GROUP_B (it was already)
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_B, GROUP_B), 1, "WS_B still in GROUP_B");

    // WS_A (the mover window's app_id) is NOT a workspace in sway - the window just has
    // that app_id. The actual workspace it's on is WS_B.

    // --- Cleanup ---
    fixture.swayg(&["nav", "go", &orig_ws]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    drop(_win_a);
    drop(_win_b);
    drop(_win_mover);
    std::thread::sleep(std::time::Duration::from_millis(500));

    for g in [GROUP_A, GROUP_B] {
        fixture.swayg(&["group", "select", g, "--output", &fixture.orig_output]).success();
        fixture.swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output, "--create"]).success();
    }

    fixture.init().success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name IN ('{}','{}')", GROUP_A, GROUP_B)),
        0
    );
}
