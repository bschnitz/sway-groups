use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, db_query, get_focused_workspace, orig_active_group, pause_test_daemon,
    resume_test_daemon, start_test_daemon_with_config, stop_test_daemon,
    ws_in_group_count, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_cfga_34";
const GROUP_B: &str = "zz_test_cfgb_34";
const APP_ID: &str = "assignment-test-id";
const ASSIGNED_WS: &str = "test_workspace_1";
const CONFIG_PATH: &str = "/tmp/swayg-test-config-34.toml";

fn write_test_config(content: &str) {
    std::fs::write(CONFIG_PATH, content).expect("write test config");
}

fn check_assignment_rule() {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".config/sway/assignments.conf");

    if !config_path.exists()
        || !std::fs::read_to_string(&config_path)
            .unwrap_or_default()
            .contains("assignment-test-id")
    {
        panic!(
            "ASSIGNMENT RULE NOT FOUND.\n\
             Please add to {}:\n\
             \n\
             for_window [app_id=\"assignment-test-id\"] workspace test_workspace_1\n",
            config_path.display()
        );
    }
}

#[tokio::test]
async fn test_34_daemon_config_assign() {
    check_assignment_rule();

    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();
    let orig_output = fixture.orig_output.clone();
    let orig_group = orig_active_group(&orig_output);

    // --- Setup: init, create groups ---
    fixture.init().success();
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &orig_output, "--create"])
        .success();
    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();

    // --- Test 1: config rule assigns workspace to specific groups ---
    write_test_config(&format!(
        "[[assign]]\nmatch = \"{ASSIGNED_WS}\"\ngroups = [\"{GROUP_A}\", \"{GROUP_B}\"]\n"
    ));

    let config_path = std::path::Path::new(CONFIG_PATH);
    start_test_daemon_with_config(config_path);
    resume_test_daemon();

    let _win = DummyWindowHandle::spawn(APP_ID).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(2500));

    let focused = get_focused_workspace().expect("get focused workspace");
    assert_eq!(focused, ASSIGNED_WS, "sway assignment rule moved window to workspace");

    assert_eq!(
        ws_in_group_count(&fixture.db_path, ASSIGNED_WS, GROUP_A),
        1,
        "workspace assigned to GROUP_A by config rule"
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, ASSIGNED_WS, GROUP_B),
        1,
        "workspace assigned to GROUP_B by config rule"
    );

    let total_memberships = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{ASSIGNED_WS}'"
        ),
    );
    assert_eq!(total_memberships, 2, "workspace in exactly 2 groups");

    // --- Cleanup test 1 ---
    pause_test_daemon();
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    // Make sure the assigned workspace is gone from sway before test 2.
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));
    stop_test_daemon();

    // --- Test 2: global flag (no groups → falls back to active group + sets global) ---
    fixture.init().success();
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &orig_output, "--create"])
        .success();
    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();

    write_test_config(&format!(
        "[[assign]]\nmatch = \"{ASSIGNED_WS}\"\nglobal = true\n"
    ));

    start_test_daemon_with_config(config_path);
    resume_test_daemon();

    let _win2 = DummyWindowHandle::spawn(APP_ID).expect("spawn dummy window 2");
    std::thread::sleep(std::time::Duration::from_millis(2500));

    let is_global = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{ASSIGNED_WS}'"),
    );
    assert_eq!(is_global, "1", "workspace marked global by config rule");

    // global=true without groups → should still be in active group.
    assert_eq!(
        ws_in_group_count(&fixture.db_path, ASSIGNED_WS, GROUP_A),
        1,
        "global workspace still added to active group when rule has no groups"
    );

    // --- Cleanup ---
    pause_test_daemon();
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = std::fs::remove_file(CONFIG_PATH);

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    fixture
        .swayg(&[
            "group", "select", &orig_group, "--output", &orig_output, "--create",
        ])
        .success();
}
