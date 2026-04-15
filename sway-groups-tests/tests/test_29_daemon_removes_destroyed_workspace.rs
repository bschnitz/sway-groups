use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_query, get_focused_workspace, pause_test_daemon, resume_test_daemon, start_test_daemon,
    swayg_output, DummyWindowHandle, TestFixture,
};

const WS_DEL: &str = "zz_test_ws_del";

#[tokio::test]
async fn test_29_daemon_removes_destroyed_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();

    fixture.init().success();
    start_test_daemon();
    resume_test_daemon();

    swayg_output(&fixture.db_path, &["workspace", "add", WS_DEL, "--groups", "0"]);

    let _ = Command::new("swaymsg")
        .args(["workspace", WS_DEL])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let _win = DummyWindowHandle::spawn(WS_DEL).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_DEL,
        "focused on test workspace"
    );

    let ws_in_db = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_DEL
    ));
    assert_eq!(ws_in_db, "1", "workspace in DB before destroy");

    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_in_db_after = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_DEL
    ));
    assert_eq!(
        ws_in_db_after, "0",
        "workspace should be removed from DB after sway destroys it"
    );

    let wg_after = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspace_groups wg \
         JOIN workspaces w ON w.id = wg.workspace_id \
         WHERE w.name = '{}'", WS_DEL
    ));
    assert_eq!(wg_after, "0", "workspace_group entries should be cleaned up");

    pause_test_daemon();
}
