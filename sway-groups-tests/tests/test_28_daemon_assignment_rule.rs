use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    DummyWindowHandle, TestFixture, db_query, get_focused_workspace, pause_test_daemon,
    resume_test_daemon, start_test_daemon, swayg_output, ws_in_group_count,
};

const TEST_GROUP: &str = "zz_test_assign_group";
const APP_ID: &str = "assignment-test-id";
const ASSIGNED_WS: &str = "test_workspace_1";

#[tokio::test]
async fn test_28_daemon_with_assignment_rule() {
    // The rule under test lives in this compositor's own config, so the
    // test does not depend on anything in the developer's sway setup.
    let fixture = TestFixture::with_sway_config(
        "for_window [app_id=\"assignment-test-id\"] workspace test_workspace_1",
    )
    .await
    .expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();
    let orig_output = fixture.orig_output.clone();

    fixture.init().success();
    start_test_daemon();
    resume_test_daemon();

    fixture
        .swayg(&[
            "group",
            "select",
            TEST_GROUP,
            "--output",
            &orig_output,
            "--create",
        ])
        .success();

    assert_eq!(
        db_query(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)
        ),
        "1",
        "group was created"
    );

    let _win = DummyWindowHandle::spawn(APP_ID).expect("spawn dummy window with assignment app_id");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let focused = get_focused_workspace().expect("get focused workspace after assignment");
    assert_eq!(
        focused, ASSIGNED_WS,
        "assignment rule moved window to '{}'",
        ASSIGNED_WS
    );

    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_count = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name = '{}'",
            ASSIGNED_WS
        ),
    );
    assert_eq!(
        ws_count, "1",
        "daemon should have added assigned workspace '{}' to DB",
        ASSIGNED_WS
    );

    let _active_group = swayg_output(&fixture.db_path, &["group", "active", &orig_output]);

    fixture
        .swayg(&["workspace", "move", ASSIGNED_WS, "--groups", TEST_GROUP])
        .success();

    std::thread::sleep(std::time::Duration::from_millis(500));

    let in_group = ws_in_group_count(&fixture.db_path, ASSIGNED_WS, TEST_GROUP);
    assert_eq!(
        in_group, 1,
        "workspace '{}' should be in group '{}' after move",
        ASSIGNED_WS, TEST_GROUP
    );

    let active_after_move = swayg_output(&fixture.db_path, &["group", "active", &orig_output]);
    assert_eq!(
        active_after_move, TEST_GROUP,
        "active_group should still be TEST_GROUP after workspace move"
    );

    pause_test_daemon();

    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
