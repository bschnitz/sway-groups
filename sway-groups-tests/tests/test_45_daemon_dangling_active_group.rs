use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    TestFixture, db_count, db_exec, resume_test_daemon, start_test_daemon, swayg_output,
    workspace_count_in_sway, ws_in_group_count,
};

const GONE: &str = "zz_test_gone_45";
const WS_EXT: &str = "zz_test_ws_45";

fn swaymsg_workspace(name: &str) {
    let _ = Command::new("swaymsg")
        .args(["workspace", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[tokio::test]
async fn test_45_daemon_dangling_active_group() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    fixture.init().success();

    // The output names a group that no longer exists, and the default group is
    // gone too — the state a boot-time repair used to leave behind.
    db_exec(
        &fixture.db_path,
        "DELETE FROM workspace_groups WHERE group_id = (SELECT id FROM groups WHERE name = '0')",
    );
    db_exec(&fixture.db_path, "DELETE FROM groups WHERE name = '0'");
    db_exec(
        &fixture.db_path,
        &format!(
            "UPDATE outputs SET active_group = '{GONE}' WHERE name = '{}'",
            fixture.orig_output
        ),
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name = '0'"
        ),
        0,
        "precondition: no default group"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GONE,
        "precondition: the active group is dangling"
    );

    start_test_daemon();
    resume_test_daemon();

    swaymsg_workspace(WS_EXT);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(workspace_count_in_sway(WS_EXT), 1, "WS_EXT exists in sway");
    std::thread::sleep(std::time::Duration::from_secs(2));

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_EXT, "0"),
        1,
        "the daemon put the new workspace into the default group"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        "0",
        "the output now points at the default group"
    );

    // Post-condition: leaving the empty workspace destroys it, and the daemon
    // removes it from the DB.
    swaymsg_workspace(&fixture.orig_workspace);
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert_eq!(
        workspace_count_in_sway(WS_EXT),
        0,
        "WS_EXT is gone from sway"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM workspaces WHERE name LIKE 'zz_test_%'"
        ),
        0
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name LIKE 'zz_test_%'"
        ),
        0
    );
}
