//! Empty groups must stay reachable via `group next-on-output` / `prev-on-output`.
//!
//! A group with no workspaces used to be filtered out of the output's group
//! list, so the bar showed it but navigation skipped it. Switching to such a
//! group is well defined (the switch lands on the default workspace) and the
//! group is pruned again as soon as it is left. Only groups whose workspaces
//! live on *other* outputs stay skipped.

use std::path::PathBuf;
use std::process::Stdio;

use sway_groups_tests::common::{
    db_count, db_exec, get_focused_workspace, orig_active_group, swayg_fixture_db, swayg_output,
    workspace_exists_in_sway, DummyWindowHandle, TestFixture,
};

/// Non-empty group, sorts first of the three.
const GROUP_FILLED: &str = "zz_test_ne1__";
/// Empty group, sorts second — the one navigation used to skip.
const GROUP_EMPTY: &str = "zz_test_ne2__";
/// Empty group bound to a different output, sorts last — must stay skipped.
const GROUP_ELSEWHERE: &str = "zz_test_ne3__";
const WS_FILLED: &str = "zz_tg_ne_a__";
const OTHER_OUTPUT: &str = "zz_no_such_output__";

fn get_active_group(db_path: &PathBuf, output: &str) -> String {
    swayg_output(db_path, &["group", "active", output])
}

#[tokio::test]
async fn test_36_group_nav_empty_group() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        for g in [GROUP_FILLED, GROUP_EMPTY, GROUP_ELSEWHERE] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0,
                "{} must not exist in production DB",
                g
            );
        }
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_FILLED)
            ),
            0,
            "{} must not exist in production DB",
            WS_FILLED
        );
    }

    assert!(
        !workspace_exists_in_sway(WS_FILLED),
        "{} must not exist in sway",
        WS_FILLED
    );

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init + one filled group + two empty groups ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", GROUP_FILLED, "--output", &fixture.orig_output, "--create"])
        .success();
    let _win = DummyWindowHandle::spawn(WS_FILLED).expect("spawn WS_FILLED");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_FILLED, "--switch-to-workspace"])
        .success();

    fixture.swayg(&["group", "create", GROUP_EMPTY]).success();
    fixture.swayg(&["group", "create", GROUP_ELSEWHERE]).success();

    // Pin the third group to an output that is not the one under test.
    db_exec(
        &fixture.db_path,
        &format!(
            "UPDATE groups SET last_active_output = '{}' WHERE name = '{}'",
            OTHER_OUTPUT, GROUP_ELSEWHERE
        ),
    );

    fixture
        .swayg(&["group", "select", GROUP_FILLED, "--output", &fixture.orig_output])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    for g in [GROUP_FILLED, GROUP_EMPTY, GROUP_ELSEWHERE] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
            1,
            "group '{}' exists",
            g
        );
    }
    assert!(_win.exists_in_tree(), "dummy window '{}' is running", WS_FILLED);
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_FILLED,
        "active group = '{}'",
        GROUP_FILLED
    );

    // --- Test: next-on-output reaches the empty group (used to skip to the end) ---
    fixture.swayg(&["group", "next-on-output"]).success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_EMPTY,
        "empty group '{}' is reachable via next-on-output",
        GROUP_EMPTY
    );

    // The switch lands on the default workspace, not on some other group's one.
    assert_eq!(
        get_focused_workspace().unwrap(),
        "0",
        "switch into the empty group focuses the default workspace"
    );

    // --- Test: a group bound to another output stays skipped ---
    // '{GROUP_EMPTY}' is the last reachable group, so without --wrap nothing moves.
    fixture.swayg(&["group", "next-on-output"]).success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_EMPTY,
        "'{}' is bound to another output and is not navigated to",
        GROUP_ELSEWHERE
    );

    // --- Test: leaving the empty group prunes it again ---
    fixture.swayg(&["group", "prev-on-output"]).success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_FILLED,
        "prev-on-output returns to '{}'",
        GROUP_FILLED
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_EMPTY)
        ),
        0,
        "'{}' auto-deleted after being left",
        GROUP_EMPTY
    );

    // --- Cleanup ---
    fixture
        .swayg(&["group", "delete", GROUP_ELSEWHERE, "--force"])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();
    drop(_win);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !workspace_exists_in_sway(WS_FILLED) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !workspace_exists_in_sway(WS_FILLED),
        "'{}' is gone from sway",
        WS_FILLED
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}')",
                GROUP_FILLED, GROUP_EMPTY, GROUP_ELSEWHERE
            ),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_FILLED),
        ),
        0,
        "no test workspaces remain"
    );

    // --- Cleanup: restore original group on live DB ---
    swayg_fixture_db(&["group", "select", &orig_group, "--output", &fixture.orig_output]).success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
