//! Test: unglobal on a workspace that was previously in multiple groups.
//! After unglobal, workspace is added only to the current active group (not the deleted ones).

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_ung_a";
const GROUP_B: &str = "zz_test_ung_b";
const WS1: &str = "zz_test_ws1_ung";

#[tokio::test]
async fn test_unglobal_multi_group_workspace() {
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

    // --- Set global (removes from both groups, auto-deletes GROUP_A) ---
    fixture.workspace_service.set_global(WS1, true).await.unwrap();
    assert_workspace_global(&fixture.db, WS1, true).await;
    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_exists(&fixture.db, GROUP_B).await; // active group survives

    // --- Unglobal: WS1 added to GROUP_B (active) only, NOT GROUP_A (deleted) ---
    fixture.workspace_service.set_global(WS1, false).await.expect("unglobal");
    assert_workspace_global(&fixture.db, WS1, false).await;

    // Exactly 1 membership
    let ws = sway_groups_core::db::entities::WorkspaceEntity::find_by_name(WS1)
        .one(fixture.db.conn()).await.unwrap().unwrap();
    let memberships = sway_groups_core::db::entities::WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(fixture.db.conn()).await.unwrap();
    assert_eq!(memberships.len(), 1, "Should have exactly 1 membership after unglobal");

    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_A).await;

    // GROUP_A was NOT resurrected
    assert_group_not_exists(&fixture.db, GROUP_A).await;

    // WS1 visible in GROUP_B
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible in GROUP_B: {:?}", visible);

    // --- Switch back to "0": GROUP_B should NOT auto-delete (WS1 still in Sway) ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_exists(&fixture.db, GROUP_B).await;

    // --- Kill, now GROUP_B should auto-delete ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    switch_group_and_back(&fixture, GROUP_B, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_no_test_data(&fixture.db).await;
}
