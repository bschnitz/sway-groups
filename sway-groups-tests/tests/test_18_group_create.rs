use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, swayg_output, swayg_stderr, TestFixture,
};

const GROUP: &str = "zz_test_create";


#[tokio::test]
async fn test_18_group_create() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // Get original group from REAL db (before init)
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- 1. Precondition: test group does not exist in real DB ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
            ),
            0,
            "precondition: {} must not exist in real DB",
            GROUP
        );
    }

    // --- 2. Setup: init ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test group in DB after init"
    );

    // --- 3. Test: create group (success) ---
    fixture.swayg(&["group", "create", GROUP]).success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' exists in DB",
        GROUP
    );

    let active = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(
        active, "",
        "active group is empty (none set) after init"
    );

    // --- 4. Test: create same group again (error) ---
    fixture
        .swayg(&["group", "create", GROUP])
        .failure();

    let stderr_output = swayg_stderr(&fixture.db_path, &["group", "create", GROUP]);
    assert!(
        stderr_output.contains("already exists"),
        "error contains 'already exists' (stderr: {})",
        stderr_output
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "still exactly 1 group '{}' in DB (no duplicate)",
        GROUP
    );

    let active_still = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(
        active_still, "",
        "active group still empty after failed create"
    );

    // --- 5. Cleanup: switch back to original workspace ---
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- 6. Post-condition ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test data remains in DB"
    );
}
