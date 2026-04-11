//! Test: group select with auto-creation and switching back.
//!
//! Covers:
//! - `get_or_create_group` + `set_active_group` creates the group and switches
//! - Switching back to group "0" restores the original workspace
//! - An empty group is auto-deleted when switching away from it

use std::time::Duration;

use sway_groups_tests::common::{
    assert_active_group, assert_focused_workspace, assert_group_exists,
    assert_group_not_exists, assert_no_test_data, SwayTestFixture,
};

const TEST_GROUP: &str = "zz_test_group_select";

#[tokio::test]
async fn test_group_select_create_and_switch_back() {
    let fixture = SwayTestFixture::new()
        .await
        .expect("Failed to set up test fixture");

    let output = fixture.orig_output.clone();
    let orig_ws = fixture.orig_workspace.clone();

    // Precondition: fresh DB, test group cannot exist yet.
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;

    // --- Action: create and select the test group ---

    fixture
        .group_service
        .get_or_create_group(TEST_GROUP)
        .await
        .expect("get_or_create_group failed");

    fixture
        .group_service
        .set_active_group(&output, TEST_GROUP)
        .await
        .expect("set_active_group failed");

    // --- Assertions: group was created and is now active ---

    assert_group_exists(&fixture.db, TEST_GROUP).await;
    assert_active_group(&fixture, &output, TEST_GROUP).await;

    // --- Action: switch back to group "0" ---

    fixture
        .group_service
        .set_active_group(&output, "0")
        .await
        .expect("set_active_group back to '0' failed");

    // Brief settle time for Sway.
    std::thread::sleep(Duration::from_millis(100));

    // --- Assertions: original workspace restored, test group gone ---

    assert_focused_workspace(&fixture, &orig_ws);

    // The test group had no workspaces, so it should have been auto-deleted.
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;

    assert_no_test_data(&fixture.db).await;
}
