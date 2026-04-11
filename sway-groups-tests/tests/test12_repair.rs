//! Test: repair — DB↔Sway reconciliation.
//! - Stale DB workspace (not in Sway) → removed
//! - Sway workspace not in DB → added to group "0"
//! - Empty groups → pruned

use std::time::Duration;
use sea_orm::ActiveModelTrait;
use sway_groups_core::db::entities::{workspace, workspace_group};
use sway_groups_tests::common::*;

const GROUP: &str = "zz_test_repair";
const GROUP_EMPTY: &str = "zz_test_repair_empty";
const WS1: &str = "zz_test_ws1_rep";
const WS_STALE: &str = "zz_test_stale_rep";

#[tokio::test]
async fn test_repair() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let _orig_ws = fixture.orig_workspace.clone();

    // --- Setup: GROUP + WS1 in Sway, plus a stale DB-only workspace ---
    fixture.group_service.get_or_create_group(GROUP).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Create an empty group
    fixture.group_service.create_group(GROUP_EMPTY).await.unwrap();

    // Insert stale workspace directly into DB (does NOT exist in Sway)
    let now = chrono::Utc::now().naive_utc();
    let stale_ws = workspace::ActiveModel {
        name: sea_orm::Set(WS_STALE.to_string()),
        number: sea_orm::Set(None),
        output: sea_orm::Set(Some(output.clone())),
        is_global: sea_orm::Set(false),
        created_at: sea_orm::Set(Some(now)),
        updated_at: sea_orm::Set(Some(now)),
        ..Default::default()
    }.insert(fixture.db.conn()).await.unwrap();

    let group = sway_groups_core::db::entities::GroupEntity::find_by_name(GROUP)
        .one(fixture.db.conn()).await.unwrap().unwrap();
    workspace_group::ActiveModel {
        workspace_id: sea_orm::Set(stale_ws.id),
        group_id: sea_orm::Set(group.id),
        created_at: sea_orm::Set(Some(now)),
        ..Default::default()
    }.insert(fixture.db.conn()).await.unwrap();

    // Remove WS1 from DB (but it's still in Sway)
    let ws1_model = sway_groups_core::db::entities::WorkspaceEntity::find_by_name(WS1)
        .one(fixture.db.conn()).await.unwrap().unwrap();
    let memberships = sway_groups_core::db::entities::WorkspaceGroupEntity::find_by_workspace(ws1_model.id)
        .all(fixture.db.conn()).await.unwrap();
    for m in memberships {
        use sea_orm::ModelTrait;
        m.delete(fixture.db.conn()).await.unwrap();
    }
    use sea_orm::ModelTrait;
    ws1_model.delete(fixture.db.conn()).await.unwrap();

    // Verify setup state
    assert_workspace_exists(&fixture.db, WS_STALE).await;
    assert_workspace_not_exists(&fixture.db, WS1).await;
    assert_sway_workspace_exists(&fixture, WS1);
    assert_group_exists(&fixture.db, GROUP_EMPTY).await;

    // --- Repair ---
    let (_removed, _added, _pruned) = fixture.workspace_service
        .repair(&fixture.group_service)
        .await
        .expect("repair");

    // WS_STALE removed (was in DB, not in Sway)
    assert_workspace_not_exists(&fixture.db, WS_STALE).await;

    // WS1 re-added (was in Sway, not in DB) and placed in group "0"
    assert_workspace_exists(&fixture.db, WS1).await;
    assert_workspace_in_group(&fixture.db, WS1, "0").await;

    // GROUP_EMPTY pruned (no non-global workspaces)
    assert_group_not_exists(&fixture.db, GROUP_EMPTY).await;

    // GROUP was effectively empty after stale removal → also pruned
    assert_group_not_exists(&fixture.db, GROUP).await;

    // WS1 visible after repair
    let visible = fixture.workspace_service.list_visible_workspaces(&output).await.unwrap();
    assert!(visible.contains(&WS1.to_string()), "WS1 visible after repair: {:?}", visible);

    // --- Cleanup ---
    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    assert_no_test_data(&fixture.db).await;
}
