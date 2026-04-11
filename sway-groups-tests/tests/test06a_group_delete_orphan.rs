//! Test: group delete with multi-group workspace.
//! - Without --force: fails when group has workspaces
//! - With --force: orphaned workspaces move to group "0",
//!   multi-group workspaces keep their other memberships

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_del_a";
const GROUP_B: &str = "zz_test_del_b";
const WS1: &str = "zz_test_ws1_del";  // in GROUP_A + GROUP_B
const WS2: &str = "zz_test_ws2_del";  // in GROUP_A only

#[tokio::test]
async fn test_group_delete_orphan_workspaces() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: GROUP_A contains WS1+WS2, GROUP_B also contains WS1 ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS2)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS2)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // --- Delete GROUP_A without --force should fail ---
    let result = fixture.group_service.delete_group(GROUP_A, false).await;
    assert!(result.is_err(), "delete without force should fail");
    assert_group_exists(&fixture.db, GROUP_A).await;

    // --- Delete GROUP_A with --force ---
    fixture.group_service.delete_group(GROUP_A, true).await.expect("delete with force");
    assert_group_not_exists(&fixture.db, GROUP_A).await;

    // WS1: was in GROUP_A + GROUP_B → still in GROUP_B, NOT moved to "0"
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_not_in_group(&fixture.db, WS1, "0").await;

    // WS2: was only in GROUP_A → moved to group "0"
    assert_workspace_in_group(&fixture.db, WS2, "0").await;

    // Both workspaces still in DB and Sway
    assert_workspace_exists(&fixture.db, WS1).await;
    assert_workspace_exists(&fixture.db, WS2).await;
    assert_sway_workspace_exists(&fixture, WS1);
    assert_sway_workspace_exists(&fixture, WS2);

    // WS1 visible in GROUP_B
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    let visible_b = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible_b.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible_b);
    assert!(!visible_b.contains(&WS2.to_string()), "WS2 NOT visible in GROUP_B: {:?}", visible_b);

    // WS2 visible in group "0"
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    let visible_0 = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible_0.contains(&WS2.to_string()), "WS2 visible in group 0: {:?}", visible_0);

    assert_focused_workspace(&fixture, &orig_ws);

    // --- Cleanup ---
    drop(win1);
    drop(win2);
    fixture.wait_until(Duration::from_secs(2), || {
        let ws = fixture.ipc.get_workspaces().unwrap_or_default();
        !ws.iter().any(|w| w.name == WS1 || w.name == WS2)
    }).expect("windows gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_no_test_data(&fixture.db).await;
}
