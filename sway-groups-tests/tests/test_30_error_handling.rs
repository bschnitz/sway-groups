
use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, swayg_live, swayg_stderr,
    workspace_exists_in_sway, ws_in_group_count, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_grp_err_a_30";
const GROUP_B: &str = "zz_test_grp_err_b_30";
const WS1: &str = "zz_test_ws_err_30";

#[tokio::test]
async fn test_30_error_handling() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = get_focused_workspace().expect("get focused workspace");
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP_A, GROUP_B)),
            0,
            "precondition: test groups must not exist in production DB"
        );
    }
    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);

    // --- Init ---
    fixture.init().success();

    // --- Test: group create "" → validation error (new validation added during refactoring) ---
    fixture.swayg(&["group", "create", ""]).failure();

    let stderr = swayg_stderr(&fixture.db_path, &["group", "create", ""]);
    assert!(
        stderr.contains("must not be empty"),
        "error message contains 'must not be empty' (stderr: {})",
        stderr
    );

    assert_eq!(
        db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = ''"),
        0,
        "no empty-name group created in DB"
    );

    // --- Setup for remaining tests: create GROUP_A, spawn WS1, add to group ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert!(workspace_exists_in_sway(WS1), "{} must exist in sway", WS1);
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group '{}'",
        WS1, GROUP_A
    );

    // --- Test: workspace add WS --group <explicit> ---
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();

    fixture
        .swayg(&["workspace", "add", WS1, "--group", GROUP_B])
        .success();

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in '{}' (membership preserved)",
        WS1, GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} now also in '{}' via explicit --group",
        WS1, GROUP_B
    );

    // --- Test: workspace remove WS --group <explicit> ---
    fixture
        .swayg(&["workspace", "remove", WS1, "--group", GROUP_B])
        .success();

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_B),
        0,
        "{} removed from '{}' via explicit --group",
        WS1, GROUP_B
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} still in '{}' (other membership untouched)",
        WS1, GROUP_A
    );

    // --- Test: group delete without --force on non-empty group ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture.swayg(&["group", "delete", GROUP_A]).failure();

    let stderr_delete = swayg_stderr(&fixture.db_path, &["group", "delete", GROUP_A]);
    assert!(
        stderr_delete.to_lowercase().contains("force"),
        "error message hints at --force (stderr: {})",
        stderr_delete
    );

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        1,
        "{} NOT deleted (no --force)",
        GROUP_A
    );

    // --- Cleanup: kill dummy window, auto-delete GROUP_A ---
    // GROUP_B was auto-deleted already when we switched to GROUP_A above.
    // Switching to "0" first keeps GROUP_A alive (WS1 still in sway).
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Switch to GROUP_A then away: GROUP_A becomes effectively empty → auto-delete.
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP_A, GROUP_B),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        0,
        "no test workspace remains"
    );

    // --- Restore original state ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
