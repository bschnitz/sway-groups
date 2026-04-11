//! Test: global workspaces — visibility, unglobal, auto-delete of all-global groups.

use std::time::Duration;

use sway_groups_tests::common::{
    assert_focused_workspace, assert_group_not_exists, assert_no_test_data,
    assert_workspace_global, assert_workspace_in_group, assert_workspace_not_in_group,
    DummyWindowHandle, SwayTestFixture,
};

const TEST_GROUP: &str = "zz_test_global_ws";
const WS1: &str = "zz_test_ws1_glb";
const WS2: &str = "zz_test_ws2_glb";

#[tokio::test]
async fn test_global_workspace_visibility() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup ---
    fixture.group_service.get_or_create_group(TEST_GROUP).await.expect("create group");
    fixture.group_service.set_active_group(&output, TEST_GROUP).await.expect("set active");

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS2)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS2)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // --- Set WS1 global ---
    fixture.workspace_service.set_global(WS1, true).await.expect("set global");
    assert_workspace_global(&fixture.db, WS1, true).await;

    // --- Switch to orig group: WS1 (global) should be visible, WS2 not ---
    fixture.group_service.set_active_group(&output, "0").await.expect("switch back");
    std::thread::sleep(Duration::from_millis(150));

    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.expect("list visible");
    assert!(visible.contains(&WS1.to_string()), "WS1 should be visible (global): {:?}", visible);
    assert!(!visible.contains(&WS2.to_string()), "WS2 should NOT be visible (not global): {:?}", visible);

    // WS1 should have no group membership (global removes memberships)
    assert_workspace_not_in_group(&fixture.db, WS1, TEST_GROUP).await;
    // WS2 still in TEST_GROUP
    assert_workspace_in_group(&fixture.db, WS2, TEST_GROUP).await;

    // --- Unglobal WS1 → added to current active group ---
    fixture.workspace_service.set_global(WS1, false).await.expect("unglobal");
    assert_workspace_global(&fixture.db, WS1, false).await;
    std::thread::sleep(Duration::from_millis(100));

    let visible2 = fixture.workspace_service.list_visible_workspaces(&output).await.expect("list visible 2");
    assert!(visible2.contains(&WS1.to_string()), "WS1 visible after unglobal in group '0': {:?}", visible2);

    // --- Auto-delete: make WS2 global, kill WS1, switch groups ---
    fixture.group_service.set_active_group(&output, TEST_GROUP).await.unwrap();
    fixture.workspace_service.set_global(WS2, true).await.expect("set WS2 global");
    assert_workspace_global(&fixture.db, WS2, true).await;

    // Kill win1 so WS1 disappears from Sway
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone from sway");

    // Switch from TEST_GROUP (now only global workspaces remain) → auto-delete
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;

    // Cleanup win2
    drop(win2);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS2)).unwrap_or(false)
    }).expect("WS2 gone");

    assert_no_test_data(&fixture.db).await;
}
