//! Test: workspace groups — list group memberships for a workspace.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_wg_a";
const GROUP_B: &str = "zz_test_wg_b";
const GROUP_C: &str = "zz_test_wg_c";
const WS1: &str = "zz_test_ws1_wg";

#[tokio::test]
async fn test_workspace_groups_listing() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: WS1 in GROUP_A + GROUP_B, NOT in GROUP_C ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.create_group(GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();
    fixture.group_service.create_group(GROUP_C).await.unwrap();
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // --- Test: get_groups_for_workspace ---
    let groups = fixture.workspace_service.get_groups_for_workspace(WS1).await.expect("get groups");

    assert!(groups.contains(&GROUP_A.to_string()), "groups should contain GROUP_A: {:?}", groups);
    assert!(groups.contains(&GROUP_B.to_string()), "groups should contain GROUP_B: {:?}", groups);
    assert!(!groups.contains(&GROUP_C.to_string()), "groups should NOT contain GROUP_C: {:?}", groups);

    // --- Cleanup ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    for g in [GROUP_A, GROUP_B, GROUP_C] {
        switch_group_and_back(&fixture, g, "0").await.unwrap();
    }

    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_group_not_exists(&fixture.db, GROUP_C).await;
    assert_no_test_data(&fixture.db).await;
}
