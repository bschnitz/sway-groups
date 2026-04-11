//! Test: group rename — updates DB, output.active_group, group_state. Error cases.

use sway_groups_core::db::entities::{GroupEntity, GroupStateEntity, OutputEntity};
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_rn_a";
const GROUP_B: &str = "zz_test_rn_b";
const GROUP_RENAMED: &str = "zz_test_rn_a2";
const WS1: &str = "zz_test_ws1_rn";

#[tokio::test]
async fn test_group_rename_basic() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();

    // --- Setup via direct DB inserts (no Sway IPC needed for rename logic) ---
    fixture.group_service.create_group(GROUP_A).await.unwrap();
    fixture.group_service.create_group(GROUP_B).await.unwrap();

    // Set output active_group to GROUP_A
    use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
    if let Some(out) = OutputEntity::find_by_name(&output).one(fixture.db.conn()).await.unwrap() {
        let mut active = out.into_active_model();
        active.active_group = Set(GROUP_A.to_string());
        active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(fixture.db.conn()).await.unwrap();
    }

    // Create workspace + membership + group_state
    use sway_groups_core::db::entities::workspace;
    let now = chrono::Utc::now().naive_utc();
    let ws = workspace::ActiveModel {
        name: sea_orm::Set(WS1.to_string()),
        number: sea_orm::Set(None),
        output: sea_orm::Set(Some(output.clone())),
        is_global: sea_orm::Set(false),
        created_at: sea_orm::Set(Some(now)),
        updated_at: sea_orm::Set(Some(now)),
        ..Default::default()
    }.insert(fixture.db.conn()).await.unwrap();

    let group_a = GroupEntity::find_by_name(GROUP_A).one(fixture.db.conn()).await.unwrap().unwrap();
    use sway_groups_core::db::entities::workspace_group;
    workspace_group::ActiveModel {
        workspace_id: sea_orm::Set(ws.id),
        group_id: sea_orm::Set(group_a.id),
        created_at: sea_orm::Set(Some(now)),
        ..Default::default()
    }.insert(fixture.db.conn()).await.unwrap();

    use sway_groups_core::db::entities::group_state;
    group_state::ActiveModel {
        output: sea_orm::Set(output.clone()),
        group_name: sea_orm::Set(GROUP_A.to_string()),
        last_focused_workspace: sea_orm::Set(Some(WS1.to_string())),
        last_visited: sea_orm::Set(Some(now)),
        ..Default::default()
    }.insert(fixture.db.conn()).await.unwrap();

    // Verify setup
    assert_group_exists(&fixture.db, GROUP_A).await;
    assert_group_exists(&fixture.db, GROUP_B).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;

    // --- Rename GROUP_A → GROUP_RENAMED (success) ---
    fixture.group_service.rename_group(GROUP_A, GROUP_RENAMED).await.expect("rename");

    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_exists(&fixture.db, GROUP_RENAMED).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_RENAMED).await;
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_A).await;

    // output.active_group updated
    let out = OutputEntity::find_by_name(&output).one(fixture.db.conn()).await.unwrap().unwrap();
    assert_eq!(out.active_group, GROUP_RENAMED, "output.active_group should be updated");

    // group_state updated
    let gs = GroupStateEntity::find_by_output_and_group(&output, GROUP_RENAMED)
        .one(fixture.db.conn()).await.unwrap();
    assert!(gs.is_some(), "group_state should be updated to new name");

    let gs_old = GroupStateEntity::find_by_output_and_group(&output, GROUP_A)
        .one(fixture.db.conn()).await.unwrap();
    assert!(gs_old.is_none(), "old group_state entry should be gone");

    // workspace list shows renamed group
    let workspaces = fixture.workspace_service.list_workspaces(None, Some(GROUP_RENAMED)).await.unwrap();
    assert!(workspaces.iter().any(|w| w.name == WS1), "WS1 in renamed group");

    // --- Error: rename to existing name → fails ---
    let result = fixture.group_service.rename_group(GROUP_RENAMED, GROUP_B).await;
    assert!(result.is_err(), "rename to existing name should fail");
    assert_group_exists(&fixture.db, GROUP_RENAMED).await;
    assert_group_exists(&fixture.db, GROUP_B).await;

    // --- Error: rename nonexistent → fails ---
    let result2 = fixture.group_service.rename_group("nonexistent_zz_test", GROUP_RENAMED).await;
    assert!(result2.is_err(), "rename nonexistent should fail");

    // --- Error: rename group "0" → fails ---
    let result3 = fixture.group_service.rename_group("0", "should_not_work_zz").await;
    assert!(result3.is_err(), "rename group '0' should fail");
    assert_group_exists(&fixture.db, "0").await;
    assert_group_not_exists(&fixture.db, "should_not_work_zz").await;

    // --- Cleanup ---
    // Delete test groups and workspace directly (no Sway workspaces involved)
    fixture.group_service.delete_group(GROUP_RENAMED, true).await.ok();
    fixture.group_service.delete_group(GROUP_B, true).await.ok();

    assert_no_test_data(&fixture.db).await;
}
