//! `nav go` must know where a forgotten workspace belongs before it jumps.
//!
//! sway destroys a workspace together with its last window. The destruction
//! takes the workspace's group memberships with it, and the emptied group is
//! pruned on top of that -- so the next jump to that workspace found nothing on
//! record, left the active group alone and recreated the workspace wherever the
//! output happened to stand. The window appeared, but the bar and every
//! group-relative binding still belonged to the group the jump was meant to
//! leave.
//!
//! Where the workspace belongs is written down twice over: in the `[[assign]]`
//! rules the daemon already uses to file a new workspace, and in `--group` for
//! callers that know it at runtime but have no rule. Both are a fallback, not
//! an override: a workspace that is still filed somewhere is not moved.

use std::path::PathBuf;

use sway_groups_tests::common::sway_instance::instance_dir;
use sway_groups_tests::common::{
    TestFixture, db_count, db_exec, get_focused_workspace, swayg_output, workspace_exists_in_sway,
    ws_in_group_count,
};

/// Group named by an `[[assign]]` rule.
const GROUP_CONFIG: &str = "zz_test_gof_cfg_40";
/// Group named by `--group` on the command line.
const GROUP_FLAG: &str = "zz_test_gof_flg_40";
const WS_CONFIG: &str = "zz_test_gof_ws_40";
const WS_FLAG: &str = "zz_test_gof_wsf_40";

fn get_active_group(db_path: &PathBuf, output: &str) -> String {
    swayg_output(db_path, &["group", "active", output])
}

fn group_count(db_path: &PathBuf, name: &str) -> i64 {
    db_count(
        db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", name),
    )
}

#[tokio::test]
async fn test_40_nav_go_group_fallback() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let output = fixture.orig_output.clone();

    let config_path = instance_dir().join("config-40.toml");
    std::fs::write(
        &config_path,
        format!("[[assign]]\nmatch = \"{WS_CONFIG}\"\ngroups = [\"{GROUP_CONFIG}\"]\n"),
    )
    .expect("write test config");
    let config_arg = config_path.to_string_lossy().to_string();

    // --- Precondition: nothing on record, which is the state a pruned
    // workspace leaves behind. The fixture has already run init/repair/select,
    // so this is its database as handed over -- another `init` here would drop
    // the group selection it just made. ---
    assert_eq!(
        group_count(&fixture.db_path, GROUP_CONFIG),
        0,
        "'{}' does not exist yet",
        GROUP_CONFIG
    );
    assert_eq!(
        group_count(&fixture.db_path, GROUP_FLAG),
        0,
        "'{}' does not exist yet",
        GROUP_FLAG
    );
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        "0",
        "the fixture starts in the default group"
    );

    // --- Test 1: the `[[assign]]` rule answers where the workspace belongs ---
    fixture
        .swayg(&["--config", &config_arg, "nav", "go", WS_CONFIG])
        .success();

    assert_eq!(
        group_count(&fixture.db_path, GROUP_CONFIG),
        1,
        "the pruned group named by the rule was recreated"
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_CONFIG, GROUP_CONFIG),
        1,
        "membership restored from the config rule"
    );
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_CONFIG,
        "the active group followed the workspace"
    );
    // The membership has to be restored *before* the group is selected:
    // selecting an empty group focuses its default workspace, and the jump
    // would end up stranded there.
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_CONFIG,
        "focus landed on the workspace, not on the group's default workspace"
    );

    // --- Test 2: a workspace that is still filed is not refiled ---
    // Straight after test 1, so WS_CONFIG still exists and the DB still knows
    // its group: --group then has nothing to restore and must keep its hands
    // off. (Navigating away first would not do -- that destroys the empty
    // workspace and prunes the group, which is the very state test 1 covers.)
    fixture
        .swayg(&[
            "--config",
            &config_arg,
            "nav",
            "go",
            WS_CONFIG,
            "--group",
            GROUP_FLAG,
        ])
        .success();

    assert_eq!(
        group_count(&fixture.db_path, GROUP_FLAG),
        0,
        "--group did not even create its group for a workspace that is filed"
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_CONFIG, GROUP_CONFIG),
        1,
        "'{}' stayed in the group it was filed in",
        WS_CONFIG
    );
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_CONFIG,
        "the active group followed the recorded membership, not the flag"
    );

    // --- Test 3: `--group` answers it for a workspace with no rule ---
    fixture
        .swayg(&[
            "--config",
            &config_arg,
            "nav",
            "go",
            WS_FLAG,
            "--group",
            GROUP_FLAG,
        ])
        .success();

    assert_eq!(
        group_count(&fixture.db_path, GROUP_FLAG),
        1,
        "--group created the group it names"
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_FLAG, GROUP_FLAG),
        1,
        "membership restored from --group"
    );
    assert_eq!(
        get_active_group(&fixture.db_path, &output),
        GROUP_FLAG,
        "the active group followed the workspace"
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_FLAG,
        "focus landed on the workspace named on the command line"
    );

    // --- Test 4: a membership cannot be written twice ---
    // The daemon files a workspace from the same `[[assign]]` rule the moment
    // sway creates it, while the jump is filing it too -- either can overtake
    // the other between its check and its insert, so "already there" cannot be
    // a check in either of them. Writing the row directly is what a lost race
    // amounts to; the unique index is what has to refuse it.
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO workspace_groups (workspace_id, group_id, created_at) \
             SELECT w.id, g.id, '2000-01-01 00:00:00' FROM workspaces w, groups g \
             WHERE w.name = '{WS_FLAG}' AND g.name = '{GROUP_FLAG}'"
        ),
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_FLAG, GROUP_FLAG),
        1,
        "the unique index refused the duplicate membership"
    );

    // --- Cleanup ---
    fixture
        .swayg(&["group", "select", "0", "--output", &output, "--create"])
        .success();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !workspace_exists_in_sway(WS_CONFIG) && !workspace_exists_in_sway(WS_FLAG) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !workspace_exists_in_sway(WS_CONFIG) && !workspace_exists_in_sway(WS_FLAG),
        "both test workspaces are gone from sway"
    );

    let _ = std::fs::remove_file(&config_path);

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                GROUP_CONFIG, GROUP_FLAG
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
                WS_CONFIG, WS_FLAG
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
                WS_CONFIG, WS_FLAG
            ),
        ),
        0,
        "no test memberships remain"
    );
}
