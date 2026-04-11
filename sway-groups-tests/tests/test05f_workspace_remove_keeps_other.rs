//! Test: workspace remove from one group keeps other memberships intact.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_rm_a";
const GROUP_B: &str = "zz_test_rm_b";
const WS1: &str = "zz_test_ws1_rm";

#[tokio::test]
async fn test_remove_from_one_group_keeps_other() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: WS1 in GROUP_A and GROUP_B ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();

    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;

    // --- Remove WS1 from GROUP_B (active) ---
    fixture.workspace_service.remove_from_group(WS1, GROUP_B).await.expect("remove from group B");

    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;  // still there
    assert_workspace_exists(&fixture.db, WS1).await;             // still in DB
    assert_sway_workspace_exists(&fixture, WS1);                  // still in Sway
    assert_group_exists(&fixture.db, GROUP_A).await;

    // WS1 NOT visible in GROUP_B
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(!visible.contains(&WS1.to_string()), "WS1 should NOT be visible in GROUP_B: {:?}", visible);

    // WS1 IS visible in GROUP_A
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();
    let visible_a = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible_a.contains(&WS1.to_string()), "WS1 visible in GROUP_A: {:?}", visible_a);

    // --- Switch back: GROUP_A should NOT auto-delete (WS1 still in Sway) ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_exists(&fixture.db, GROUP_A).await;

    // --- Kill, auto-delete GROUP_A ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    switch_group_and_back(&fixture, GROUP_A, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_no_test_data(&fixture.db).await;
}
