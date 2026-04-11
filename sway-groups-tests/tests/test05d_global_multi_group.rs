//! Test: set global on a workspace that belongs to multiple groups.
//! Going global removes all group memberships and may auto-delete non-active empty groups.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_glb_mg_a";
const GROUP_B: &str = "zz_test_glb_mg_b";
const WS1: &str = "zz_test_ws1_gmg";

#[tokio::test]
async fn test_global_on_multi_group_workspace() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: WS1 in both GROUP_A and GROUP_B ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;

    // --- Set WS1 global ---
    fixture.workspace_service.set_global(WS1, true).await.expect("set global");

    assert_workspace_global(&fixture.db, WS1, true).await;

    // No group memberships anymore
    let ws = sway_groups_core::db::entities::WorkspaceEntity::find_by_name(WS1)
        .one(fixture.db.conn()).await.unwrap().unwrap();
    let memberships = sway_groups_core::db::entities::WorkspaceGroupEntity::find_by_workspace(ws.id)
        .all(fixture.db.conn()).await.unwrap();
    assert_eq!(memberships.len(), 0, "Global workspace should have no group memberships");

    // Visible (global workspace visible everywhere)
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible (global): {:?}", visible);

    // GROUP_A should be auto-deleted (not active, no non-global workspaces)
    assert_group_not_exists(&fixture.db, GROUP_A).await;
    // GROUP_B still exists (it's the active group)
    assert_group_exists(&fixture.db, GROUP_B).await;

    // --- Switch back: GROUP_B should auto-delete ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);
    assert_group_not_exists(&fixture.db, GROUP_B).await;

    // --- Cleanup ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    assert_no_test_data(&fixture.db).await;
}
