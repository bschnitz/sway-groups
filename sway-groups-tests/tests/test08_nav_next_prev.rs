//! Test: nav next/prev — navigate visible workspaces, wrap behavior, boundary.

use std::time::Duration;
use sway_groups_tests::common::*;
use sway_groups_core::services::NavigationService;

const GROUP: &str = "zz_test_nav_np";
const WS_A: &str = "zz_test_alpha";
const WS_B: &str = "zz_test_beta";
const WS_C: &str = "zz_test_gamma";

#[tokio::test]
async fn test_nav_next_prev() {
    let fixture = SwayTestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    let nav = NavigationService::new(fixture.db.clone(), fixture.ipc.clone());

    // --- Setup: 3 windows on 3 workspaces in alphabetical order A < B < C ---
    fixture.group_service.get_or_create_group(GROUP).await.unwrap();
    fixture.group_service.set_active_group(&output, GROUP).await.unwrap();

    let win_a = DummyWindowHandle::spawn(&fixture, WS_A).expect("spawn A");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_A)).unwrap();

    let win_b = DummyWindowHandle::spawn(&fixture, WS_B).expect("spawn B");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_B)).unwrap();

    let win_c = DummyWindowHandle::spawn(&fixture, WS_C).expect("spawn C");
    fixture.ipc.run_command(&format!("move container to workspace \"{}\"", WS_C)).unwrap();

    // Focus A as starting point
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS_A)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_focused_workspace(&fixture, WS_A);

    // --- nav next: A → B ---
    nav.next_workspace(&output, false).await.expect("next");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_B);

    // --- nav next: B → C ---
    nav.next_workspace(&output, false).await.expect("next");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_C);

    // --- nav next no-wrap at boundary: C → stays C ---
    nav.next_workspace(&output, false).await.expect("next (boundary)");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_C);

    // --- nav next wrap at boundary: C → A ---
    nav.next_workspace(&output, true).await.expect("next wrap");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_A);

    // Position at B for prev tests
    fixture.ipc.run_command(&format!("workspace \"{}\"", WS_B)).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // --- nav prev: B → A ---
    nav.prev_workspace(&output, false).await.expect("prev");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_A);

    // --- nav prev no-wrap at boundary: A → stays A ---
    nav.prev_workspace(&output, false).await.expect("prev (boundary)");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_A);

    // --- nav prev wrap at boundary: A → C ---
    nav.prev_workspace(&output, true).await.expect("prev wrap");
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, WS_C);

    // --- Cleanup ---
    fixture.group_service.set_active_group(&output, "0").await.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_focused_workspace(&fixture, &orig_ws);

    drop(win_a);
    drop(win_b);
    drop(win_c);
    fixture.wait_until(Duration::from_secs(2), || {
        let ws = fixture.ipc.get_workspaces().unwrap_or_default();
        !ws.iter().any(|w| w.name == WS_A || w.name == WS_B || w.name == WS_C)
    }).expect("windows gone");

    switch_group_and_back(&fixture, GROUP, "0").await.unwrap();
    assert_group_not_exists(&fixture.db, GROUP).await;
    assert_no_test_data(&fixture.db).await;
}
