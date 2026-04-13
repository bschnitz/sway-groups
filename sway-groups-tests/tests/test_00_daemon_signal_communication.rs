use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    daemon_state, get_focused_workspace, pause_test_daemon, resume_test_daemon,
    start_test_daemon, swayg_output, TestFixture,
};

const WS_A: &str = "zz_test_d0_ws_a";
const WS_B: &str = "zz_test_d0_ws_b";

fn db_query(db_path: &PathBuf, sql: &str) -> String {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn workspace_count_in_sway(name: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let workspaces: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse workspaces");
    workspaces
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w.get("name").and_then(|n| n.as_str()) == Some(name))
        .count() as i64
}

fn cleanup_workspace(name: &str, orig_ws: &str) {
    let _ = Command::new("swaymsg")
        .args(["workspace", orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(200));
    if workspace_count_in_sway(name) > 0 {
        let _ = Command::new("swaymsg")
            .args(["workspace", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = Command::new("swaymsg")
            .args(["workspace", orig_ws])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

#[tokio::test]
async fn test_00_daemon_signal_communication() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();

    cleanup_workspace(WS_A, &orig_ws);
    cleanup_workspace(WS_B, &orig_ws);

    fixture.init().success();

    start_test_daemon();
    assert_eq!(
        daemon_state().as_deref(),
        Some("running"),
        "daemon starts in running state"
    );

    pause_test_daemon();
    assert_eq!(
        daemon_state().as_deref(),
        Some("paused"),
        "daemon paused after SIGUSR1"
    );

    resume_test_daemon();
    assert_eq!(
        daemon_state().as_deref(),
        Some("running"),
        "daemon running after SIGUSR2"
    );

    let _ = Command::new("swaymsg")
        .args(["workspace", WS_A])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(workspace_count_in_sway(WS_A), 1, "WS_A exists in sway");

    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_a_in_db = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_A
    ));
    assert_eq!(ws_a_in_db, "1", "daemon added WS_A to DB while running");

    pause_test_daemon();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let _ = Command::new("swaymsg")
        .args(["workspace", WS_B])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(workspace_count_in_sway(WS_B), 1, "WS_B exists in sway");

    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_b_in_db = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_B
    ));
    assert_eq!(ws_b_in_db, "0", "daemon did NOT add WS_B while paused");

    pause_test_daemon();

    cleanup_workspace(WS_A, &orig_ws);
    cleanup_workspace(WS_B, &orig_ws);

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "back on original workspace"
    );
}
