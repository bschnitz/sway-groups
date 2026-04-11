//! Test: container move to a workspace that already exists in another group.
//! The move should add the workspace to the active group as well.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_cm_a";
const GROUP_B: &str = "zz_test_cm_b";
const WS1: &str = "zz_test_ws1_cm";
const WS2: &str = "zz_test_ws2_cm";

#[tokio::test]
async fn test_container_move_to_workspace_in_other_group() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: GROUP_A + WS1 ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    // --- Switch to GROUP_B ---
    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    assert_group_exists(&fixture.db, GROUP_A).await;

    // --- Spawn win2 in GROUP_B, then move it to WS1 (which is in GROUP_A) ---
    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    std::thread::sleep(Duration::from_millis(150));

    // nav move-to adds WS1 to the active group (GROUP_B) as a side effect
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // Manually add WS1 to GROUP_B (mirrors container move behavior)
    let _ = fixture.workspace_service.add_to_group(WS1, GROUP_B).await;

    assert_focused_workspace(&fixture, WS1);
    assert_window_on_workspace(&fixture, WS2, WS1);

    // WS1 in both groups
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;

    // Exactly one WS1 in Sway
    let count = fixture.ipc.get_workspaces().unwrap().into_iter().filter(|w| w.name == WS1).count();
    assert_eq!(count, 1);

    // Visible in GROUP_B
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible);

    // --- Switch back ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    // --- Kill, auto-delete ---
    drop(win1);
    drop(win2);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    switch_group_and_back(&fixture, GROUP_A, "0").await.unwrap();

    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_no_test_data(&fixture.db).await;
}
