//! Test: nav go + nav back — navigate to specific workspace, history-based back.

use std::time::Duration;
use sway_groups_tests::common::*;
use sway_groups_core::services::NavigationService;

const GROUP: &str = "zz_test_nav_gb";
const WS_A: &str = "zz_test_one";
const WS_B: &str = "zz_test_two";

#[tokio::test]
async fn test_nav_go_and_back() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    let nav = NavigationService::new(fixture.db.clone(), fixture.ipc.clone());

    // --- Setup ---
    fixture.group_service.get_or_create_group(GROUP).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP).await.unwrap();

    let win_a = DummyWindowHandle::spawn(&fixture, WS_A).expect("spawn A");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_A)).unwrap();

    let win_b = DummyWindowHandle::spawn(&fixture, WS_B).expect("spawn B");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_B)).unwrap();

    // Switch back to orig workspace
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_focused_workspace(&fixture, &orig_ws);

    // --- nav go WS_A ---
    nav.go_workspace(WS_A).await.expect("go A");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_A);

    // --- nav go WS_B ---
    nav.go_workspace(WS_B).await.expect("go B");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_B);

    // --- nav back: WS_B → WS_A ---
    nav.go_back().await.expect("back");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_A);

    // --- nav back: WS_A → WS_B (alternation) ---
    nav.go_back().await.expect("back 2");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_B);

    // --- nav go orig_ws ---
    nav.go_workspace(&orig_ws).await.expect("go orig");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    // --- nav back: orig → WS_B ---
    nav.go_back().await.expect("back from orig");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_B);

    // --- nav back: WS_B → orig ---
    nav.go_back().await.expect("back to orig");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    // --- Cleanup ---
    drop(win_a);
    drop(win_b);
    fixture.wait_until(Duration::from_secs(2), || {
        let ws = fixture.ipc.get_workspaces().unwrap_or_default();
        !ws.iter().any(|w| w.name == WS_A || w.name == WS_B)
    }).expect("windows gone");

    switch_group_and_back(&fixture, GROUP, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP).await;
    assert_no_test_data(&fixture.db).await;
}
