use sway_groups_tests::common::{
    db_count, db_query, get_focused_workspace, line_starts_with, orig_active_group,
    output_contains, swayg_live, swayg_output, workspace_exists_in_sway, ws_in_group_count,
    DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_vis__";
const WS_A: &str = "zz_tg_vis__";
const WS_B: &str = "zz_tg_hid__";

#[tokio::test]
async fn test_14_workspace_list_output_format() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
            0,
            "{} must not exist in production DB",
            GROUP
        );
        for ws in [WS_A, WS_B] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0,
                "{} must not exist in production DB",
                ws
            );
        }
    }

    for ws in [WS_A, WS_B] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init + group + 2 dummy windows + move + switch back ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();

    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn WS_B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_B, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        1,
        "group '{}' exists",
        GROUP
    );
    assert!(_win_a.exists_in_tree(), "dummy window '{}' is running", WS_A);
    assert!(_win_b.exists_in_tree(), "dummy window '{}' is running", WS_B);
    for ws in [WS_A, WS_B] {
        assert_eq!(
            ws_in_group_count(&fixture.db_path, ws, GROUP),
            1,
            "'{}' in group '{}'",
            ws, GROUP
        );
    }

    // --- Test: workspace list --visible (only active group's workspaces) ---
    let vis_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--visible", "--output", &fixture.orig_output],
    );
    assert!(
        !output_contains(&vis_out, WS_A),
        "'{}' NOT in visible list (different active group)",
        WS_A
    );
    assert!(
        !output_contains(&vis_out, WS_B),
        "'{}' NOT in visible list",
        WS_B
    );

    // --- Test: workspace list --output (all with status markers) ---
    let all_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&all_out, WS_A),
        "'{}' in full list",
        WS_A
    );
    assert!(
        output_contains(&all_out, WS_B),
        "'{}' in full list",
        WS_B
    );
    assert!(
        output_contains(&all_out, "hidden") && output_contains(&all_out, WS_A) && all_out.lines().any(|l| l.contains(WS_A) && l.contains("hidden")),
        "'{}' marked as (hidden)",
        WS_A
    );
    assert!(
        output_contains(&all_out, "hidden") && output_contains(&all_out, WS_B) && all_out.lines().any(|l| l.contains(WS_B) && l.contains("hidden")),
        "'{}' marked as (hidden)",
        WS_B
    );
    assert!(
        all_out.lines().any(|l| l.contains(&orig_ws) && l.contains("visible")),
        "'{}' marked as (visible)",
        orig_ws
    );

    // --- Test: make WS_A global, check (global) marker ---
    fixture.swayg(&["workspace", "global", WS_A]).success();

    let is_global = db_query(
        &fixture.db_path,
        &format!("SELECT is_global FROM workspaces WHERE name = '{}'", WS_A),
    );
    assert_eq!(is_global, "1", "'{}' is global in DB", WS_A);

    let global_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--output", &fixture.orig_output],
    );
    assert!(
        global_out.lines().any(|l| l.contains(WS_A) && l.contains("global")),
        "'{}' marked as (global)",
        WS_A
    );

    // --- Test: workspace list --plain (no status markers) ---
    let plain_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain", "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&plain_out, WS_A),
        "'{}' in plain list",
        WS_A
    );
    assert!(
        !output_contains(&plain_out, "(global)"),
        "no (global) markers in plain output"
    );
    assert!(
        !output_contains(&plain_out, "(hidden)"),
        "no (hidden) markers in plain output"
    );
    assert!(
        !output_contains(&plain_out, "(visible)"),
        "no (visible) markers in plain output"
    );

    // --- Test: workspace list --group (filtered by group) ---
    let grp_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--group", GROUP, "--output", &fixture.orig_output],
    );
    assert!(
        output_contains(&grp_out, WS_B),
        "'{}' in group list",
        WS_B
    );
    assert!(
        !line_starts_with(&grp_out, &orig_ws),
        "'{}' NOT in group list (different group)",
        orig_ws
    );

    // --- Cleanup ---
    fixture.swayg(&["workspace", "unglobal", WS_A]).success();

    drop(_win_a);
    drop(_win_b);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS_A), "'{}' is gone from sway", WS_A);
    assert!(!workspace_exists_in_sway(WS_B), "'{}' is gone from sway", WS_B);

    // --- Auto-delete empty group ---
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output])
        .success();
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0,
        "'{}' auto-deleted",
        GROUP
    );

    // --- Post-condition: init to sync DB state ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')", WS_A, WS_B),
    );
    assert_eq!(group_gone, 0, "no test groups remain");
    assert_eq!(ws_gone, 0, "no test workspaces remain");

    // --- Cleanup: restore original group on live DB ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
