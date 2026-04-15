use sway_groups_tests::common::{
    db_count, output_contains, swayg_output, window_count_in_tree, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_status";
const WS1: &str = "zz_tg_ws1_status";

#[tokio::test]
async fn test_17_status() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // --- 1. Setup: init ---
    fixture.init().success();

    // --- 2. Test: status (clean state) ---
    let status_out = swayg_output(&fixture.db_path, &["status"]);
    eprintln!("Status output:\n{}", status_out);

    assert!(
        output_contains(&status_out, &fixture.orig_output),
        "output contains output name '{}'",
        fixture.orig_output
    );
    assert!(
        output_contains(&status_out, "active group"),
        "output contains 'active group'"
    );
    assert!(
        output_contains(&status_out, "Visible:"),
        "output contains 'Visible:'"
    );
    assert!(
        output_contains(&status_out, "Hidden:"),
        "output contains 'Hidden:'"
    );

    // --- 3. Test: status with dummy window in test group (non-default active group) ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();

    let _win = DummyWindowHandle::spawn(WS1).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    let status_out2 = swayg_output(&fixture.db_path, &["status"]);
    eprintln!("Status output (with test group):\n{}", status_out2);

    assert!(
        output_contains(&status_out2, "active group = \"0\""),
        "active group = '0' (not test group)"
    );
    assert!(
        output_contains(&status_out2, WS1),
        "output mentions '{}' (hidden workspace)",
        WS1
    );

    // --- 4. Test: status with global workspace ---
    fixture.swayg(&["workspace", "global", WS1]).success();

    let status_global = swayg_output(&fixture.db_path, &["status"]);
    eprintln!("Status output (global):\n{}", status_global);

    assert!(
        output_contains(&status_global, "Global:"),
        "output contains 'Global:' section"
    );
    assert!(
        output_contains(&status_global, WS1),
        "'{}' listed in Global section",
        WS1
    );

    // --- 5. Cleanup ---
    fixture.swayg(&["workspace", "unglobal", WS1]).success();

    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1),
        0,
        "dummy window '{}' is gone",
        WS1
    );

    // Group may already be auto-deleted (if WS1 was the only non-global workspace)
    // Verify it's gone via init sync
    fixture.init().success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0,
        "'{}' gone after cleanup",
        GROUP
    );

    // --- 6. Post-condition ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(
        (group_gone, ws_gone),
        (0, 0),
        "no test data remains"
    );
}
