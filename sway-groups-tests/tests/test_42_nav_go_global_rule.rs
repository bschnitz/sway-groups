//! `nav go` must restore the global flag an `[[assign]]` rule asks for.
//!
//! sway destroys a workspace together with its last window, and the database
//! forgets it with it -- the global flag included. The daemon marks a workspace
//! global again when sway recreates it, but it leaves a workspace that `nav go`
//! creates to the CLI (pending event). So the jump itself has to read the rule,
//! and it must not file the workspace into the active group on top of that.

use std::path::PathBuf;

use sway_groups_tests::common::sway_instance::instance_dir;
use sway_groups_tests::common::{
    TestFixture, db_count, get_focused_workspace, swayg_output, workspace_exists_in_sway,
    ws_in_group_count,
};

const WS_GLOBAL: &str = "zz_test_gog_ws_42";
/// Global and filed in a group, like the README's `music` example.
const WS_BOTH: &str = "zz_test_gog_wsb_42";
const GROUP_BOTH: &str = "zz_test_gog_grp_42";

fn is_global(db_path: &PathBuf, name: &str) -> i64 {
    db_count(
        db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name = '{name}' AND is_global = 1"),
    )
}

fn membership_count(db_path: &PathBuf, name: &str) -> i64 {
    db_count(
        db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '{name}'"
        ),
    )
}

#[tokio::test]
async fn test_42_nav_go_global_rule() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();

    let config_path = instance_dir().join("config-42.toml");
    std::fs::write(
        &config_path,
        format!(
            "[[assign]]\nmatch = \"{WS_GLOBAL}\"\nglobal = true\n\n\
             [[assign]]\nmatch = \"{WS_BOTH}\"\nglobal = true\ngroups = [\"{GROUP_BOTH}\"]\n"
        ),
    )
    .expect("write test config");
    let config_arg = config_path.to_string_lossy().to_string();

    // --- Precondition: the workspace is unknown, as after sway destroyed it ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{WS_GLOBAL}'"),
        ),
        0,
        "'{WS_GLOBAL}' is not on record yet"
    );
    assert!(
        !workspace_exists_in_sway(WS_GLOBAL),
        "'{WS_GLOBAL}' is not in sway yet"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &output]),
        "0",
        "the fixture starts in the default group"
    );

    // --- Test 1: the jump creates the workspace as global ---
    fixture
        .swayg(&["--config", &config_arg, "nav", "go", WS_GLOBAL])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_GLOBAL,
        "focus landed on the workspace"
    );
    assert_eq!(
        is_global(&fixture.db_path, WS_GLOBAL),
        1,
        "the rule's global flag was restored"
    );
    assert_eq!(
        membership_count(&fixture.db_path, WS_GLOBAL),
        0,
        "a global workspace was not filed into the active group"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &output]),
        "0",
        "the active group was left alone"
    );

    // --- Test 2: jumping to it again keeps it global and unfiled ---
    fixture
        .swayg(&["--config", &config_arg, "nav", "go", WS_GLOBAL])
        .success();

    assert_eq!(is_global(&fixture.db_path, WS_GLOBAL), 1, "still global");
    assert_eq!(
        membership_count(&fixture.db_path, WS_GLOBAL),
        0,
        "still in no group"
    );

    // --- Test 3: global and groups together, as the daemon does it ---
    // The flag has to come first: marking a workspace global drops its
    // memberships, so the other order would lose the group again.
    fixture
        .swayg(&["--config", &config_arg, "nav", "go", WS_BOTH])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_BOTH,
        "focus landed on the workspace"
    );
    assert_eq!(
        is_global(&fixture.db_path, WS_BOTH),
        1,
        "global flag restored"
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_BOTH, GROUP_BOTH),
        1,
        "group membership restored alongside the flag"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &output]),
        GROUP_BOTH,
        "the active group followed the workspace"
    );

    // --- Cleanup: leave the empty workspaces so sway destroys them ---
    fixture
        .swayg(&["group", "select", "0", "--output", &output, "--create"])
        .success();
    fixture
        .swayg(&["nav", "go", &fixture.orig_workspace])
        .success();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline
        && (workspace_exists_in_sway(WS_GLOBAL) || workspace_exists_in_sway(WS_BOTH))
    {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !workspace_exists_in_sway(WS_GLOBAL) && !workspace_exists_in_sway(WS_BOTH),
        "both test workspaces are gone from sway"
    );

    let _ = std::fs::remove_file(&config_path);

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name IN ('{WS_GLOBAL}', '{WS_BOTH}')"),
        ),
        0,
        "no test workspaces remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{GROUP_BOTH}'"),
        ),
        0,
        "no test group remains"
    );
    assert_eq!(
        membership_count(&fixture.db_path, WS_GLOBAL) + membership_count(&fixture.db_path, WS_BOTH),
        0,
        "no test memberships remain"
    );
}
