use sway_groups_tests::common::{
    db_count, db_exec, get_focused_workspace, orig_active_group,
    ws_in_group_count, DummyWindowHandle, TestFixture,
};

use std::process::{Command, Stdio};

const GROUP: &str = "zz_test_orphan_35";
const WS_A: &str = "zz_tg_orphA";

#[tokio::test]
async fn test_35_orphan_workspace_repair() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_group = orig_active_group(&fixture.orig_output);
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init, create group, create workspace ---
    fixture.init().success();
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();

    assert_eq!(ws_in_group_count(&fixture.db_path, WS_A, GROUP), 1);

    // --- Test 1: orphan a workspace by deleting its group membership ---
    db_exec(
        &fixture.db_path,
        &format!(
            "DELETE FROM workspace_groups WHERE workspace_id = \
             (SELECT id FROM workspaces WHERE name = '{WS_A}')"
        ),
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_A, GROUP),
        0,
        "workspace is now orphaned"
    );

    // Verify the workspace has zero memberships.
    let total = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{WS_A}'"
        ),
    );
    assert_eq!(total, 0, "workspace has no group memberships");

    // --- Test 2: repair adopts orphaned workspace into default group "0" ---
    // Switch active group to "0" before repair so the pruned GROUP doesn't
    // leave a dangling active_group reference.
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();
    fixture.swayg(&["sync", "--repair"]).success();

    let default_group = "0";
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_A, default_group),
        1,
        "repair adopted orphaned workspace into default group"
    );

    // --- Test 3: running repair again does not duplicate ---
    fixture.swayg(&["sync", "--repair"]).success();
    let after = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{WS_A}'"
        ),
    );
    assert_eq!(after, 1, "repair did not duplicate membership");

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
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));
}
