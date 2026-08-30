use sway_groups_tests::common::{
    TestFixture, swayg_output,
    db_count, db_query, orig_active_group,
};

const TEST_GROUP: &str = "zz_test_group_select";

#[tokio::test]
async fn test_01_group_select() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // The group the fixture seeded on this output.
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // --- Setup: init ---
    fixture.init().success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        0,
        "no test group after init"
    );

    // --- Test: group select --create ---
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
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
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", TEST_GROUP)),
        0,
        "test group auto-deleted"
    );

    // --- Post-condition: no test data ---
    let wsgrp_gone = db_query(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '{}'",
            TEST_GROUP
        ),
    );
    assert_eq!(wsgrp_gone, "0", "no test workspace_groups remain");
}
