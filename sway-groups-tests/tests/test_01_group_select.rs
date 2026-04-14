use sway_groups_tests::common::{TestFixture, get_focused_workspace, swayg_output};

const TEST_GROUP: &str = "zz_test_group_select";

fn db_count(db_path: &std::path::PathBuf, table: &str, column: &str, value: &str) -> i64 {
    let output = std::process::Command::new("sqlite3")
        .arg(db_path)
        .arg(&format!("SELECT count(*) FROM {} WHERE {} = '{}'", table, column, value))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0)
}

fn db_query(db_path: &std::path::PathBuf, sql: &str) -> String {
    let output = std::process::Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[tokio::test]
async fn test_01_group_select() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // Get original group from REAL db (before init)
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    let orig_group = {
        let output = std::process::Command::new("swayg")
            .args(["group", "active", &fixture.orig_output])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .expect("swayg group active failed");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // --- Precondition: test group does not exist in real DB ---
    if real_db.exists() {
        let real_count: i64 = {
            let output = std::process::Command::new("sqlite3")
                .arg(&real_db)
                .arg(&format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
                .expect("sqlite3 failed");
            String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0)
        };
        assert_eq!(real_count, 0, "test group must not exist in production DB");
    }

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init ---
    fixture.init().success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        0,
        "no test group after init"
    );

    // --- Test: group select --create ---
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        1,
        "group was created"
    );

    let active = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert_eq!(active, TEST_GROUP, "active group changed to test group");

    // --- Test: switch back to default group (auto-delete) ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        0,
        "test group auto-deleted"
    );

    // --- Cleanup: restore original group on live DB ---
    use sway_groups_tests::common::swayg_live;
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(
        db_count(&fixture.db_path, "groups", "name", TEST_GROUP),
        0,
        "test group was auto-deleted"
    );

    // --- Post-condition: no test data ---
    assert_eq!(db_count(&fixture.db_path, "groups", "name", TEST_GROUP), 0);
    let wsgrp_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '{}'",
            TEST_GROUP
        ),
    );
    assert_eq!(wsgrp_gone, "0", "no test workspace_groups remain");
}
