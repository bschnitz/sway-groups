use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, swayg_output, window_count_in_tree,
    workspace_exists_in_sway, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_nav_move";
const WS_TARGET: &str = "zz_tg_move_target";
const KITTY_APP_ID: &str = "zz_tg_move_kitty";

#[tokio::test]
async fn test_19_nav_move_to() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // Get original group from REAL db (before init)
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- 1. Precondition checks (BEFORE init) ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
            ),
            0,
            "precondition: {} must not exist in real DB",
            GROUP
        );
        assert_eq!(
            db_count(
                &real_db,
                &format!(
                    "SELECT count(*) FROM workspaces WHERE name = '{}'",
                    WS_TARGET
                )
            ),
            0,
            "precondition: {} must not exist in real DB",
            WS_TARGET
        );
    }

    assert!(
        !workspace_exists_in_sway(WS_TARGET),
        "precondition: {} must not exist in sway",
        WS_TARGET
    );

    // --- 2. Setup: init + create group + switch to it + launch kitty ---
    fixture.init().success();

    fixture
        .swayg(&[
            "group",
            "select",
            GROUP,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    let _win = DummyWindowHandle::spawn(KITTY_APP_ID).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        workspace_of_window(KITTY_APP_ID).is_some(),
        "dummy window '{}' exists in sway tree",
        KITTY_APP_ID
    );

    // --- 3. Verify setup ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' exists",
        GROUP
    );

    assert!(
        window_count_in_tree(KITTY_APP_ID) > 0,
        "dummy window is running"
    );

    let active_group = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active_group, GROUP, "active group = '{}'", GROUP);

    // --- 4. Test: container move --switch-to-workspace ---
    fixture
        .swayg(&["container", "move", WS_TARGET, "--switch-to-workspace"])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspaces WHERE name = '{}'",
                WS_TARGET
            )
        ),
        1,
        "'{}' exists in DB",
        WS_TARGET
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspace_groups wg \
                 JOIN groups g ON g.id = wg.group_id \
                 JOIN workspaces w ON w.id = wg.workspace_id \
                 WHERE w.name = '{}' AND g.name = '{}'",
                WS_TARGET, GROUP
            )
        ),
        1,
        "'{}' in group '{}'",
        WS_TARGET,
        GROUP
    );

    assert!(
        workspace_exists_in_sway(WS_TARGET),
        "'{}' exists in sway",
        WS_TARGET
    );

    // --- 5. Test: kitty is now on target workspace ---
    let ws_of_kitty =
        workspace_of_window(KITTY_APP_ID).expect("find workspace of kitty");
    assert_eq!(
        ws_of_kitty, WS_TARGET,
        "kitty is on workspace '{}'",
        WS_TARGET
    );

    // --- 6. Cleanup ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        window_count_in_tree(KITTY_APP_ID) == 0,
        "dummy window '{}' is gone",
        KITTY_APP_ID
    );

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- 7. Post-condition ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name = '{}'",
            WS_TARGET
        ),
    );
    assert_eq!(
        (group_gone, ws_gone),
        (0, 0),
        "no test data remains in DB"
    );
}
