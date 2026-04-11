//! Test: workspace move — removes from ALL groups, adds to specified groups.
//! Supports auto-creating new groups and comma-separated group lists.

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP_A: &str = "zz_test_mv2g_a";
const GROUP_B: &str = "zz_test_mv2g_b";
const GROUP_C: &str = "zz_test_mv2g_c";
const WS1: &str = "zz_test_ws1_mv2g";

#[tokio::test]
async fn test_workspace_move_to_groups() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup: WS1 in GROUP_A + GROUP_B ---
    fixture.group_service.get_or_create_group(GROUP_A).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_A).await.unwrap();

    let win1 = DummyWindowHandle::spawn(&fixture, WS1).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS1)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS1)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    fixture.group_service.get_or_create_group(GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP_B).await.unwrap();
    fixture.workspace_service.add_to_group(WS1, GROUP_B).await.unwrap();
    fixture.group_service.set_active_group(&output, "0").await.unwrap();

    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;

    // --- Move WS1 to GROUP_C (auto-create, removes from A+B) ---
    fixture.workspace_service.move_to_groups(WS1, &[GROUP_C]).await.expect("move to C");

    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_C).await;
    assert_group_exists(&fixture.db, GROUP_C).await;

    // --- Move WS1 to GROUP_A + GROUP_B (comma-separated) ---
    fixture.workspace_service.move_to_groups(WS1, &[GROUP_A, GROUP_B]).await.expect("move to A+B");

    assert_workspace_in_group(&fixture.db, WS1, GROUP_A).await;
    assert_workspace_in_group(&fixture.db, WS1, GROUP_B).await;
    assert_workspace_not_in_group(&fixture.db, WS1, GROUP_C).await;

    // --- Cleanup ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    drop(win1);
    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS1)).unwrap_or(false)
    }).expect("WS1 gone");

    for g in [GROUP_C, GROUP_A, GROUP_B] {
        switch_group_and_back(&fixture, g, "0").await.unwrap();
    }

    assert_group_not_exists(&fixture.db, GROUP_A).await;
    assert_group_not_exists(&fixture.db, GROUP_B).await;
    assert_group_not_exists(&fixture.db, GROUP_C).await;
    assert_no_test_data(&fixture.db).await;
}
