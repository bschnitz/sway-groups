//! The daemon makes the DB follow a focus change swayg did not make.
//!
//! `swaymsg workspace …`, a `[app_id=…] focus` from a launcher, a click on a
//! notification: sway moves the focus and nothing tells the DB. The bars then
//! still show the group that was left -- its workspaces, and its name as the
//! active one. When the newly focused workspace is not in its output's active
//! group, the daemon switches the active group to the group the workspace is in
//! (DB only: sway is already where it should be).
//!
//! swayg's own commands move the focus as well, and the daemon must not undo
//! them: a `group select` with the daemon running has to stay selected.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use sway_groups_tests::common::{
    DummyWindowHandle, TestFixture, db_count, db_query, get_focused_workspace, resume_test_daemon,
    start_test_daemon, swayg_output, workspace_exists_in_sway, workspace_of_window,
    ws_in_group_count,
};

const GROUP_A: &str = "zz_test_fol_ga_41";
const GROUP_B: &str = "zz_test_fol_gb_41";
const WS_A: &str = "zz_test_fol_wa_41";
const WS_B: &str = "zz_test_fol_wb_41";

/// Longer than the daemon's settle time plus the DB work after it.
const FOLLOW: Duration = Duration::from_millis(700);

fn get_active_group(db_path: &PathBuf, output: &str) -> String {
    swayg_output(db_path, &["group", "active", output])
}

fn swaymsg_workspace(name: &str) {
    let status = Command::new("swaymsg")
        .args(["workspace", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace");
    assert!(status.success(), "swaymsg workspace {} succeeded", name);
}

/// Put a fresh window on `ws` in `group`, the way a user builds a group.
fn populate(fixture: &TestFixture, group: &str, ws: &str) -> DummyWindowHandle {
    fixture
        .swayg(&[
            "group",
            "select",
            group,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();
    let win = DummyWindowHandle::spawn(ws).expect("spawn dummy window");
    fixture
        .swayg(&["container", "move", ws, "--switch-to-workspace"])
        .success();
    assert_eq!(
        workspace_of_window(ws).as_deref(),
        Some(ws),
        "the window sits on '{}'",
        ws
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, ws, group),
        1,
        "'{}' is in '{}'",
        ws,
        group
    );
    win
}

#[tokio::test]
async fn test_41_daemon_follows_external_focus() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();

    let win_a = populate(&fixture, GROUP_A, WS_A);
    let win_b = populate(&fixture, GROUP_B, WS_B);
    assert_eq!(get_active_group(&fixture.db_path, &output), GROUP_B);
    assert_eq!(get_focused_workspace().unwrap(), WS_B);

    start_test_daemon();
    resume_test_daemon();
    std::thread::sleep(Duration::from_millis(300));

    // --- Test 1: a focus change outside swayg switches the active group ---
    swaymsg_workspace(WS_A);
    std::thread::sleep(FOLLOW);

    assert_eq!(get_focused_workspace().unwrap(), WS_A, "sway focused WS_A");
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_A,
        "the active group followed the focus into '{}'",
        GROUP_A
    );
    // The group that was left remembers where the user was in it, not the
    // workspace the focus went to: selecting it again returns to WS_B.
    assert_eq!(
        db_query(
            &fixture.db_path,
            &format!(
                "SELECT last_focused_workspace FROM group_state \
                 WHERE output = '{output}' AND group_name = '{GROUP_B}'"
            ),
        ),
        WS_B,
        "'{}' was left on '{}'",
        GROUP_B,
        WS_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_A, GROUP_B),
        0,
        "following the focus did not pull WS_A into the group it left"
    );

    // --- Test 2: swayg's own group switch is not undone by the daemon ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &output])
        .success();
    std::thread::sleep(FOLLOW);

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "the select restored WS_B"
    );
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_B,
        "the select stayed selected with the daemon running"
    );

    // --- Test 3: a focus change inside the active group changes nothing ---
    fixture
        .swayg(&["workspace", "add", WS_A, "--group", GROUP_B])
        .success();
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_A, GROUP_B), 1);
    swaymsg_workspace(WS_A);
    std::thread::sleep(FOLLOW);

    assert_eq!(get_focused_workspace().unwrap(), WS_A, "sway focused WS_A");
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_B,
        "WS_A is in the active group too, so the group stayed"
    );

    // --- Cleanup ---
    drop(win_a);
    drop(win_b);
    fixture
        .swayg(&["group", "select", "0", "--output", &output, "--create"])
        .success();

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !workspace_exists_in_sway(WS_A) && !workspace_exists_in_sway(WS_B) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !workspace_exists_in_sway(WS_A) && !workspace_exists_in_sway(WS_B),
        "both test workspaces are gone from sway"
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                GROUP_A, GROUP_B
            ),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
                WS_A, WS_B
            ),
        ),
        0,
        "no test workspaces remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspace_groups wg \
                 JOIN workspaces w ON w.id = wg.workspace_id \
                 WHERE w.name IN ('{}', '{}')",
                WS_A, WS_B
            ),
        ),
        0,
        "no test memberships remain"
    );
}
