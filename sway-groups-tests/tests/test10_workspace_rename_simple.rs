//! Test: workspace rename — simple rename (target does not yet exist, no merge).

use std::time::Duration;
use sway_groups_tests::common::*;

const GROUP: &str = "zz_test_rns";
const WS_SRC: &str = "zz_test_src";
const WS_DST: &str = "zz_test_dst";

#[tokio::test]
async fn test_workspace_rename_simple() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // --- Setup ---
    fixture.group_service.get_or_create_group(GROUP).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP).await.unwrap();

    let win = DummyWindowHandle::spawn(&fixture, WS_SRC).expect("spawn");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_SRC)).unwrap();
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS_SRC)).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    assert_workspace_exists(&fixture.db, WS_SRC).await;
    assert_workspace_in_group(&fixture.db, WS_SRC, GROUP).await;

    // --- Rename SRC → DST (simple, no merge) ---
    let merged = fixture.workspace_service.rename_workspace(WS_SRC, WS_DST).await.expect("rename");
    assert!(!merged, "Should be simple rename, not merge");
    std::thread::sleep(Duration::from_millis(100));

    // SRC gone, DST exists
    assert_workspace_not_exists(&fixture.db, WS_SRC).await;
    assert_workspace_exists(&fixture.db, WS_DST).await;

    // DST in same group
    assert_workspace_in_group(&fixture.db, WS_DST, GROUP).await;

    // Focused on DST after rename
    assert_focused_workspace(&fixture, WS_DST);

    // Window still on DST workspace
    assert_window_on_workspace(&fixture, WS_SRC, WS_DST);

    // workspace list shows DST
    let workspaces = fixture.workspace_service.list_workspaces(None, Some(GROUP)).await.unwrap();
    assert!(workspaces.iter().any(|w| w.name == WS_DST), "DST in group");
    assert!(!workspaces.iter().any(|w| w.name == WS_SRC), "SRC not in group");

    // --- Cleanup ---
    drop(win);
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));

    fixture.wait_until(Duration::from_secs(2), || {
        fixture.ipc.get_workspaces().map(|ws| !ws.iter().any(|w| w.name == WS_DST)).unwrap_or(false)
    }).expect("DST gone");

    switch_group_and_back(&fixture, GROUP, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP).await;
    assert_no_test_data(&fixture.db).await;
}
