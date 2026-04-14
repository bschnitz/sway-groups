use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{TestFixture, pause_test_daemon, resume_test_daemon, start_test_daemon};

const WS_EXT: &str = "zz_test_ws_ext";

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

fn cleanup_external_workspace(name: &str, orig_output: &str) {
    let _ = Command::new("swaymsg")
        .args(["workspace", orig_output])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new("swaymsg")
        .args(["workspace", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new("swaymsg")
        .args(["workspace", &format!("\"{}\"", orig_output)])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[tokio::test]
async fn test_27_daemon_catches_external_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    fixture.init().success();
    fixture
        .swayg(&[
            "group",
            "select",
            "0",
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();
    start_test_daemon();
    resume_test_daemon();

    assert_eq!(
        db_query(&fixture.db_path, &format!(
            "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_EXT
        )),
        "0",
        "precondition: WS_EXT not in DB"
    );

    let _ = Command::new("swaymsg")
        .args(["workspace", WS_EXT])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(
        workspace_count_in_sway(WS_EXT), 1,
        "WS_EXT exists in sway"
    );

    std::thread::sleep(std::time::Duration::from_secs(2));

    let ws_count = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_EXT
    ));
    assert_eq!(
        ws_count, "1",
        "external workspace '{}' should appear in swayg DB after daemon processes event", WS_EXT
    );

    let group_count = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspace_groups wg \
         JOIN workspaces w ON w.id = wg.workspace_id \
         JOIN groups g ON g.id = wg.group_id \
         WHERE w.name = '{}'", WS_EXT
    ));
    assert_eq!(
        group_count, "1",
        "external workspace '{}' should be assigned to a group", WS_EXT
    );

    pause_test_daemon();

    cleanup_external_workspace(WS_EXT, &orig_output);
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture.init().success();

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
