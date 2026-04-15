use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, workspace_exists_in_sway, ws_in_group_count,
    DummyWindowHandle, TestFixture,
};

const WS1: &str = "zz_tg_cm_ws1";
const WS2: &str = "zz_tg_cm_ws2";

#[tokio::test]
async fn test_24_container_move() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        for ws in [WS1, WS2] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0,
                "{} must not exist in production DB",
                ws
            );
        }
    }

    assert!(!workspace_exists_in_sway(WS1), "precondition: {} not in sway", WS1);
    assert!(!workspace_exists_in_sway(WS2), "precondition: {} not in sway", WS2);

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output, "--create"])
        .success();

    // No group creation needed — we test that container move doesn't touch groups

    // --- Create window and move to WS1 via container move + switch ---
    let _win = DummyWindowHandle::spawn(WS1).expect("spawn");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(get_focused_workspace().unwrap(), WS1, "focused on WS1");
    assert!(workspace_exists_in_sway(WS1), "WS1 exists in sway");

    // WS1 is now in DB (via go_workspace) and in orig_group
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, &orig_group),
        1,
        "WS1 in orig_group after container move --switch"
    );

    // --- Test: container move without --switch-to-workspace ---
    // Create a second window on WS1, then move it to WS2
    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn second");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS2])
        .success();

    // WS2 was added to orig_group (new workspace, auto-added by container move)
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS2, &orig_group),
        1,
        "WS2 added to orig_group by container move"
    );

    // WS1 is still in orig_group (unchanged)
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, &orig_group),
        1,
        "WS1 still in orig_group"
    );

    // Focus didn't change (still on WS1)
    assert_eq!(get_focused_workspace().unwrap(), WS1, "still focused on WS1");

    // --- Cleanup: switch back, kill windows ---
    fixture.swayg(&["nav", "go", &orig_ws]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    drop(_win);
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS1), "WS1 gone from sway");
    assert!(!workspace_exists_in_sway(WS2), "WS2 gone from sway");

    // --- Post-condition ---
    fixture.init().success();

    for ws in [WS1, WS2] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
            0,
            "no test workspaces remain"
        );
    }
}
