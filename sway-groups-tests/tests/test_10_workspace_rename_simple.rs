use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, swayg_live, swayg_output,
    workspace_exists_in_sway, ws_in_group_count, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_rn";
const WS_SRC: &str = "zz_test_rn_src";
const WS_DST: &str = "zz_test_rn_dst";

#[tokio::test]
async fn test_10_workspace_rename_simple() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir().unwrap_or_default().join("swayg").join("swayg.db");
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Precondition: no test data in real DB ---
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
            0,
            "{} must not exist in production DB",
            GROUP
        );
    }

    for ws in [WS_SRC, WS_DST] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Setup: init + create group + launch dummy + move to WS_SRC ---
    fixture.init().success();

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

    let _win = DummyWindowHandle::spawn(WS_SRC).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_SRC, "--switch-to-workspace"])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' exists",
        GROUP
    );

    assert!(_win.exists_in_tree(), "dummy window '{}' is running", WS_SRC);

    assert!(workspace_exists_in_sway(WS_SRC), "'{}' exists in sway", WS_SRC);

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_SRC, GROUP),
        1,
        "'{}' in group '{}'",
        WS_SRC,
        GROUP
    );

    // --- Test: rename WS_SRC → WS_DST (simple, no merge) ---
    fixture
        .swayg(&["workspace", "rename", WS_SRC, WS_DST])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_SRC)
        ),
        0,
        "'{}' gone from DB",
        WS_SRC
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_DST)
        ),
        1,
        "'{}' in DB",
        WS_DST
    );

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_DST, GROUP),
        1,
        "'{}' in group '{}'",
        WS_DST,
        GROUP
    );

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_DST,
        "focused on '{}' after rename",
        WS_DST
    );

    let ws_of_win = sway_groups_tests::common::workspace_of_window(WS_SRC);
    assert_eq!(
        ws_of_win.as_deref(),
        Some(WS_DST),
        "dummy window '{}' still on workspace '{}'",
        WS_SRC,
        WS_DST
    );

    // --- Test: workspace list shows renamed workspace ---
    let list_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain", "--group", GROUP],
    );
    assert!(
        list_out.contains(WS_DST),
        "'{}' listed in group via workspace list",
        WS_DST
    );
    assert!(
        !list_out.contains(WS_SRC),
        "'{}' NOT listed",
        WS_SRC
    );

    // --- Cleanup: kill dummy window, auto-delete group, restore live DB ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS_SRC), "dummy window '{}' is gone", WS_SRC);

    fixture
        .swayg(&[
            "group",
            "select",
            "0",
            "--output",
            &fixture.orig_output,
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "'{}' auto-deleted",
        GROUP
    );

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
                WS_SRC, WS_DST
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
                 JOIN groups g ON g.id = wg.group_id \
                 WHERE g.name = '{}'",
                GROUP
            ),
        ),
        0,
        "no test workspace_groups remain"
    );

    // --- Restore original group on live DB ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
