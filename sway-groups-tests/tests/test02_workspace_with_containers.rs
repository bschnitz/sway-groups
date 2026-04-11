//! Test: create workspaces with containers, navigate, cleanup.
//!
//! Covers:
//! - container move to a new workspace → workspace created in Sway + DB
//! - workspace is added to the active group
//! - group is NOT auto-deleted while workspaces still exist in Sway
//! - group IS auto-deleted after all workspaces are gone and switching back

use std::time::Duration;

use sway_groups_tests::common::{
    assert_active_group_orig, assert_focused_workspace, assert_group_exists,
    assert_group_not_exists, assert_no_test_data, assert_workspace_exists,
    assert_workspace_in_group, assert_window_not_in_tree, DummyWindowHandle, SwayTestFixture,
};

const TEST_GROUP: &str = "zz_test_ws_containers";
const WS1: &str = "zz_test_ws1_cnt";
const WS2: &str = "zz_test_ws2_cnt";

#[tokio::test]
async fn test_workspace_with_containers() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Preconditions ---
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;

    // --- Setup: create test group ---
    fixture.group_service.get_or_create_group(TEST_GROUP).await.expect("create group");
    fixture.group_service.set_active_group(&output, TEST_GROUP).await.expect("set active group");
    assert_group_exists(&fixture.db, TEST_GROUP).await;
    assert_active_group_orig(&fixture, TEST_GROUP).await;

    // --- Spawn window 1, move container to WS1 ---
    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).expect("move to WS1");
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).expect("focus WS1");
    std::thread::sleep(Duration::from_millis(150));

    assert_focused_workspace(&fixture, WS1);
    assert_workspace_exists(&fixture.db, WS1).await;
    assert_workspace_in_group(&fixture.db, WS1, TEST_GROUP).await;

    // --- Spawn window 2, move container to WS2 ---
    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS2)).expect("move to WS2");
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS2)).expect("focus WS2");
    std::thread::sleep(Duration::from_millis(150));

    assert_focused_workspace(&fixture, WS2);
    assert_workspace_exists(&fixture.db, WS2).await;
    assert_workspace_in_group(&fixture.db, WS2, TEST_GROUP).await;

    // --- Switch back to original group: group should NOT be auto-deleted (windows exist) ---
    fixture.group_service.set_active_group(&output, "0").await.expect("switch back");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_exists(&fixture.db, TEST_GROUP).await;

    // --- Kill windows, then switch to test group and back → auto-delete ---
    drop(win1);
    drop(win2);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces()
            .map(|ws| !ws.iter().any(|w| w.name == WS1 || w.name == WS2))
            .unwrap_or(false)
    }).expect("workspaces disappeared");

    fixture.group_service.set_active_group(&output, TEST_GROUP).await.expect("switch to test group");
    std::thread::sleep(Duration::from_millis(100));
    fixture.group_service.set_active_group(&output, "0").await.expect("switch back");
    std::thread::sleep(Duration::from_millis(100));

    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;
    assert_no_test_data(&fixture.db).await;
}
