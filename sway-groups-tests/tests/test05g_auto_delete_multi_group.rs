//! Test: auto-delete with multi-group workspace.
//! A group with a multi-group workspace is only auto-deleted once ALL its
//! non-global workspaces have disappeared from Sway.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_ad_a";
const GROUP_B: &str = "zz_test_ad_b";
const WS1: &str = "zz_test_ws1_ad";
const WS2: &str = "zz_test_ws2_ad";

#[tokio::test]
async fn test_auto_delete_with_multi_group_workspace() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: WS1 in GROUP_A + GROUP_B, WS2 in GROUP_B only ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();

    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS2)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS2)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // Switch back to "0"
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // --- GROUP_A should NOT auto-delete (WS1 still in Sway) ---
    switch_group_and_back(&fixture, GROUP_A, "0").await.unwrap();
    assert_group_exists(&fixture.db, GROUP_A).await;

    // --- Kill WS1, now GROUP_A SHOULD auto-delete ---
    drop(win1);
    // Wait for Sway to remove empty WS1 workspace
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone from sway");

    switch_group_and_back(&fixture, GROUP_A, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP_A).await;

    // --- GROUP_B should NOT auto-delete yet (WS2 still in Sway) ---
    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    assert_group_exists(&fixture.db, GROUP_B).await;

    // --- Kill WS2, GROUP_B SHOULD auto-delete ---
    drop(win2);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS2)).unwrap_or(false)
    }).expect("WS2 gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP_B).await;

    assert_focused_workspace(&fixture, &orig_ws);
    assert_no_test_data(&fixture.db).await;
}
