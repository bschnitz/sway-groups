//! Test: move workspace between groups, auto-delete of emptied groups.

use std::time::Duration;

use sway_groups_tests::common::{
    assert_focused_workspace, assert_group_exists, assert_group_not_exists,
    assert_no_test_data, assert_workspace_in_group, assert_workspace_not_in_group,
    switch_group_and_back, DummyWindowHandle, SwayTestFixture,
};

const GROUP_A: &str = "zz_test_move_a";
const GROUP_B: &str = "zz_test_move_b";
const WS1: &str = "zz_test_ws1_mv";

#[tokio::test]
async fn test_workspace_move_between_groups() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: group A + WS1 ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    // --- Move WS1 to GROUP_B ---
    fixture.workspace_service.move_to_groups(WS1, &[GROUP_B]).await.expect("move to group B");

    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_A).await;

    // --- Switch to GROUP_B → GROUP_A should auto-delete (no workspaces) ---
    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_group_not_exists(&fixture.db, GROUP_A).await;

    // WS1 visible in GROUP_B
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible);

    // --- Switch back to orig: GROUP_B survives (WS1 still in Sway) ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_exists(&fixture.db, GROUP_B).await;

    // --- Kill win1, switch to GROUP_B and back → auto-delete ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.expect("switch group B and back");
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_no_test_data(&fixture.db).await;
}
