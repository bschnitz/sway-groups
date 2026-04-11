//! Test: workspace add for existing workspace in another group.
//! A workspace can be a member of multiple groups simultaneously.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_mg_add_a";
const GROUP_B: &str = "zz_test_mg_add_b";
const WS1: &str = "zz_test_ws1_mga";

#[tokio::test]
async fn test_workspace_add_existing_to_second_group() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: GROUP_A + WS1 ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    // --- Switch to GROUP_B ---
    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    assert_group_exists(&fixture.db, GROUP_A).await; // not auto-deleted, WS1 still in sway

    // --- Add WS1 to GROUP_B (it already exists in GROUP_A) ---
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.expect("add to group B");

    // WS1 in both groups, exactly one DB row
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_exists(&fixture.db, WS1).await;

    // WS1 exists exactly once in Sway
    let sway_count = fixture.ipc.get_workspaces().unwrap().into_iter().filter(|w| w.name == WS1).count();
    assert_eq!(sway_count, 1, "WS1 should exist exactly once in Sway");

    // Visible in GROUP_B
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible);

    // --- Switch back to orig ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    // --- Kill, auto-delete both groups ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    switch_group_and_back(&fixture, GROUP_A, "0").await.unwrap();

    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_no_test_data(&fixture.db).await;
}
