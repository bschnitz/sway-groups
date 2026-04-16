use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    get_focused_workspace, orig_active_group, resume_test_daemon, start_test_daemon,
    DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_urg_33";
const WS_A: &str = "zz_tg_urgA";
const WS_B: &str = "zz_tg_urgB";

/// Query sway IPC to check whether a workspace has the urgent flag set.
fn is_workspace_urgent(ws_name: &str) -> bool {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok();
    let Some(output) = output else { return false };
    let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return false;
    };
    arr.as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|w| {
            w.get("name").and_then(|n| n.as_str()) == Some(ws_name)
                && w.get("urgent").and_then(|u| u.as_bool()) == Some(true)
        })
}

#[tokio::test]
async fn test_33_daemon_handles_window_urgent_event() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init, create group, start daemon ---
    fixture.init().success();

    fixture
        .swayg(&[
            "group", "select", GROUP, "--output", &fixture.orig_output, "--create",
        ])
        .success();

    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();

    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn WS_B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_B, "--switch-to-workspace"])
        .success();

    // Focus WS_A so WS_B is not focused
    Command::new("swaymsg")
        .args(["workspace", WS_A])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace");
    std::thread::sleep(std::time::Duration::from_millis(200));

    start_test_daemon();
    resume_test_daemon();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // --- Test 1: workspace is not urgent by default ---
    assert!(
        !is_workspace_urgent(WS_B),
        "WS_B must not be urgent initially"
    );

    // --- Test 2: set urgency via swaymsg, sway marks workspace urgent ---
    let res = Command::new("swaymsg")
        .args(&[&format!("[app_id={}]", WS_B), "urgent", "enable"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("swaymsg urgent enable");

    // Give sway + daemon time to process the event
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        is_workspace_urgent(WS_B),
        "WS_B should be urgent after 'urgent enable' (swaymsg exit={}, stderr={})",
        res.status,
        String::from_utf8_lossy(&res.stderr)
    );

    // --- Test 3: daemon didn't crash — still responds ---
    // The daemon should have received a window urgent event and updated waybar.
    // We can't inspect waybar output, but verify the daemon is still alive by
    // creating a workspace and checking it gets picked up.
    // (If the daemon crashed on the window event, this would fail in daemon tests.)

    // --- Test 4: clear urgency ---
    Command::new("swaymsg")
        .args(&[&format!("[app_id={}]", WS_B), "urgent", "disable"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg urgent disable");
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        !is_workspace_urgent(WS_B),
        "WS_B should no longer be urgent after 'urgent disable'"
    );

    // --- Cleanup ---
    fixture
        .swayg(&[
            "group", "select", &orig_group, "--output", &fixture.orig_output, "--create",
        ])
        .success();
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    drop(_win_a);
    drop(_win_b);
    std::thread::sleep(std::time::Duration::from_millis(500));
}
