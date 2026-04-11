//! Test: workspace rename where target already exists → merge.
//! Containers are moved from old workspace to new, group memberships are merged.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_mrg_a";
const GROUP_B: &str = "zz_test_mrg_b";
const WS1: &str = "zz_test_ws1_mrg";
const WS2: &str = "zz_test_ws2_mrg";

#[tokio::test]
async fn test_workspace_rename_merge() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: GROUP_A + WS1, GROUP_B + WS2 ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn win1");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();

    let win2 = DummyWindowHandle::spawn(&fixture, WS2).expect("spawn win2");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS2)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS2)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_workspace_in_group(&fixture.db, WS2, GROUP_B).await;

    // Verify initial: WS1 not in GROUP_B, WS2 not in GROUP_A
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_not_in_group(&fixture.db, WS2, GROUP_A).await;

    // --- Rename WS2 → WS1 (merge, WS1 already exists) ---
    let merged = fixture.workspace_service.rename_workspace(WS2, WS1).await.expect("rename");
    assert!(merged, "Expected merge (rename to existing workspace)");
    std::thread::sleep(Duration::from_millis(150));

    // WS2 gone from DB, WS1 still exists
    assert_workspace_not_exists(&fixture.db, WS2).await;
    assert_workspace_exists(&fixture.db, WS1).await;

    // WS1 in both groups (union of memberships)
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;

    // Focused on WS1 after merge
    assert_focused_workspace(&fixture, WS1);

    // Both windows on WS1
    assert_window_on_workspace(&fixture, WS1, WS1);
    assert_window_on_workspace(&fixture, WS2, WS1);

    // WS2 does not exist in Sway anymore
    fixture.ipc.run_command(&format!("workspace \"{}\"", orig_ws)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_sway_workspace_not_exists(&fixture, WS2);
    assert_sway_workspace_exists(&fixture, WS1);

    // WS1 visible in GROUP_A
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();
    let visible_a = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible_a.contains(&WS1.to_string()), "WS1 visible in GROUP_A: {:?}", visible_a);

    // WS1 visible in GROUP_B
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    let visible_b = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible_b.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible_b);

    // --- Switch back ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

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
