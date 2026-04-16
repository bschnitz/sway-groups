use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    db_count, get_focused_workspace, orig_active_group, output_contains, swayg_output,
    swayg_stderr, workspace_exists_in_sway, workspace_of_window, ws_in_group_count,
    DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_hid_a_32";
const GROUP_B: &str = "zz_test_hid_b_32";
const WS_A: &str = "zz_tg_hidA";
const WS_B: &str = "zz_tg_hidB";
const WS_C: &str = "zz_tg_hidC";
const WS_GLOBAL: &str = "zz_tg_hidGlob";

fn count_hidden(db: &std::path::PathBuf, ws: &str, group: &str) -> i64 {
    db_count(
        db,
        &format!(
            "SELECT count(*) FROM hidden_workspaces hw \
             JOIN workspaces w ON w.id = hw.workspace_id \
             JOIN groups g ON g.id = hw.group_id \
             WHERE w.name = '{}' AND g.name = '{}'",
            ws, group
        ),
    )
}

fn get_show_hidden(db: &std::path::PathBuf) -> String {
    let res = db_count(
        db,
        "SELECT count(*) FROM settings WHERE key = 'show_hidden_workspaces' AND value = 'true'",
    );
    if res > 0 {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

#[tokio::test]
async fn test_32_hidden_workspaces() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Precondition: no test data in real DB ---
    let real_db = dirs::data_dir().unwrap_or_default().join("swayg").join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!(
                    "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                    GROUP_A, GROUP_B
                ),
            ),
            0,
            "precondition: test groups must not exist in production DB"
        );
    }
    for ws in [WS_A, WS_B, WS_C, WS_GLOBAL] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Setup: init + create GROUP_A with 3 workspaces ---
    fixture.init().success();

    fixture
        .swayg(&[
            "group", "select", GROUP_A, "--output", &fixture.orig_output, "--create",
        ])
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

    let _win_c = DummyWindowHandle::spawn(WS_C).expect("spawn WS_C");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_C, "--switch-to-workspace"])
        .success();

    // Position on WS_A
    Command::new("swaymsg")
        .args(["workspace", WS_A])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace");
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(ws_in_group_count(&fixture.db_path, WS_A, GROUP_A), 1);
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_B, GROUP_A), 1);
    assert_eq!(ws_in_group_count(&fixture.db_path, WS_C, GROUP_A), 1);

    // --- Test 1: basic hide / unhide writes rows ---
    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();
    assert_eq!(
        count_hidden(&fixture.db_path, WS_B, GROUP_A),
        1,
        "hide created a hidden row"
    );

    fixture
        .swayg(&["workspace", "unhide", WS_B, "--group", GROUP_A])
        .success();
    assert_eq!(
        count_hidden(&fixture.db_path, WS_B, GROUP_A),
        0,
        "unhide removed the hidden row"
    );

    // --- Test 2: --toggle flips ---
    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A, "--toggle"])
        .success();
    assert_eq!(count_hidden(&fixture.db_path, WS_B, GROUP_A), 1, "toggle hid");

    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A, "--toggle"])
        .success();
    assert_eq!(count_hidden(&fixture.db_path, WS_B, GROUP_A), 0, "toggle unhid");

    // --- Test 3: hide without --workspace (defaults to focused) ---
    // Currently focused on WS_A.
    fixture
        .swayg(&["workspace", "hide", "--group", GROUP_A])
        .success();
    assert_eq!(
        count_hidden(&fixture.db_path, WS_A, GROUP_A),
        1,
        "hide defaulted to focused workspace WS_A"
    );
    // Clean up
    fixture
        .swayg(&["workspace", "unhide", WS_A, "--group", GROUP_A])
        .success();

    // --- Test 4: hide on workspace not in group → error + nothing written ---
    // Create a second group GROUP_B without putting WS_B in it.
    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();
    let stderr = swayg_stderr(
        &fixture.db_path,
        &["workspace", "hide", WS_B, "--group", GROUP_B],
    );
    assert!(
        output_contains(&stderr, "not a member") || output_contains(&stderr, "Cannot hide"),
        "stderr must mention member/cannot hide; got: {}",
        stderr
    );
    assert_eq!(
        count_hidden(&fixture.db_path, WS_B, GROUP_B),
        0,
        "nothing was written for invalid hide"
    );

    // --- Test 5: navigation skips hidden when show_hidden=false (default) ---
    // Hide WS_B; nav next from WS_A should go to WS_C (not WS_B).
    Command::new("swaymsg")
        .args(["workspace", WS_A])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace");
    std::thread::sleep(std::time::Duration::from_millis(100));

    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();

    fixture
        .swayg(&["nav", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_C,
        "nav next skipped hidden WS_B"
    );

    // nav prev from WS_C should land on WS_A, skipping hidden WS_B.
    fixture
        .swayg(&["nav", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "nav prev skipped hidden WS_B"
    );

    // --- Test 6: nav go still works on hidden workspace ---
    fixture.swayg(&["nav", "go", WS_B]).success();
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_B,
        "nav go works even for hidden workspace"
    );

    // Unhide for subsequent tests
    fixture
        .swayg(&["workspace", "unhide", WS_B, "--group", GROUP_A])
        .success();

    // --- Test 6a: hiding the focused workspace auto-focuses away ---
    Command::new("swaymsg")
        .args(["workspace", WS_B])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(get_focused_workspace().unwrap(), WS_B);

    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();
    assert_ne!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focus moved away from newly hidden WS_B"
    );
    // Unhide again
    fixture
        .swayg(&["workspace", "unhide", WS_B, "--group", GROUP_A])
        .success();

    // --- Test 6b: toggling show_hidden to false while on a hidden ws focuses away ---
    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();
    // Enable show_hidden so we can navigate to the hidden workspace
    fixture
        .swayg(&["workspace", "show-hidden"])
        .success();
    fixture.swayg(&["nav", "go", WS_B]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS_B);

    // Now toggle show_hidden off — should auto-focus away
    fixture
        .swayg(&["workspace", "show-hidden", "--toggle"])
        .success();
    assert_ne!(
        get_focused_workspace().unwrap(),
        WS_B,
        "focus moved away when show_hidden toggled off while on hidden WS_B"
    );
    // Unhide WS_B for subsequent tests
    fixture
        .swayg(&["workspace", "unhide", WS_B, "--group", GROUP_A])
        .success();

    // --- Test 7: show-hidden toggle persists in settings table ---
    assert_eq!(get_show_hidden(&fixture.db_path), "false", "default is false");

    fixture
        .swayg(&["workspace", "show-hidden"])
        .success();
    assert_eq!(
        get_show_hidden(&fixture.db_path),
        "true",
        "show-hidden set to true"
    );

    fixture
        .swayg(&["workspace", "show-hidden", "--toggle"])
        .success();
    assert_eq!(
        get_show_hidden(&fixture.db_path),
        "false",
        "toggle flipped to false"
    );

    // --- Test 8: group unhide-all clears all hidden rows in a group ---
    fixture
        .swayg(&["workspace", "hide", WS_A, "--group", GROUP_A])
        .success();
    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();
    fixture
        .swayg(&["workspace", "hide", WS_C, "--group", GROUP_A])
        .success();
    assert_eq!(count_hidden(&fixture.db_path, WS_A, GROUP_A), 1);
    assert_eq!(count_hidden(&fixture.db_path, WS_B, GROUP_A), 1);
    assert_eq!(count_hidden(&fixture.db_path, WS_C, GROUP_A), 1);

    fixture
        .swayg(&["group", "unhide-all", GROUP_A])
        .success();

    assert_eq!(count_hidden(&fixture.db_path, WS_A, GROUP_A), 0);
    assert_eq!(count_hidden(&fixture.db_path, WS_B, GROUP_A), 0);
    assert_eq!(count_hidden(&fixture.db_path, WS_C, GROUP_A), 0);

    // --- Test 9: remove_from_group also removes hidden row ---
    fixture
        .swayg(&["workspace", "hide", WS_B, "--group", GROUP_A])
        .success();
    assert_eq!(count_hidden(&fixture.db_path, WS_B, GROUP_A), 1);

    fixture
        .swayg(&["workspace", "remove", WS_B, "--group", GROUP_A])
        .success();
    assert_eq!(
        count_hidden(&fixture.db_path, WS_B, GROUP_A),
        0,
        "removing from group also clears hidden flag"
    );

    // Re-add for later cleanup
    fixture
        .swayg(&["workspace", "add", WS_B, "--group", GROUP_A])
        .success();

    // --- Test 10: global workspace can be hidden per group ---
    let _win_g = DummyWindowHandle::spawn(WS_GLOBAL).expect("spawn WS_GLOBAL");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_GLOBAL, "--switch-to-workspace"])
        .success();
    fixture
        .swayg(&["workspace", "global", WS_GLOBAL])
        .success();

    // Hide global workspace in GROUP_B (it has no membership there — allowed
    // because the workspace is global, ie. implicitly in all groups).
    fixture
        .swayg(&["workspace", "hide", WS_GLOBAL, "--group", GROUP_B])
        .success();
    assert_eq!(
        count_hidden(&fixture.db_path, WS_GLOBAL, GROUP_B),
        1,
        "global workspace hidden in GROUP_B"
    );

    // Switch to GROUP_B: visible list should NOT contain WS_GLOBAL.
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();
    let vis_b = swayg_output(
        &fixture.db_path,
        &[
            "workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output,
        ],
    );
    assert!(
        !vis_b.contains(WS_GLOBAL),
        "WS_GLOBAL is hidden in GROUP_B; got visible list: {}",
        vis_b
    );

    // Switch to GROUP_A: visible list should contain WS_GLOBAL.
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();
    let vis_a = swayg_output(
        &fixture.db_path,
        &[
            "workspace", "list", "--visible", "--plain", "--output", &fixture.orig_output,
        ],
    );
    assert!(
        vis_a.contains(WS_GLOBAL),
        "WS_GLOBAL is visible in GROUP_A; got: {}",
        vis_a
    );

    // --- Test 11: status command shows "Inactive" label and "Hidden" section ---
    // Hide WS_A in GROUP_A so it appears under the new "Hidden" section.
    fixture
        .swayg(&["workspace", "hide", WS_A, "--group", GROUP_A])
        .success();
    let status = swayg_output(&fixture.db_path, &["status"]);
    assert!(
        status.contains("Inactive"),
        "status output must contain 'Inactive' label; got: {}",
        status
    );
    assert!(
        status.contains("Hidden"),
        "status output must contain 'Hidden' label; got: {}",
        status
    );
    assert!(
        status.contains("show_hidden_workspaces"),
        "status output must show the show_hidden_workspaces flag; got: {}",
        status
    );
    // Unhide again
    fixture
        .swayg(&["workspace", "unhide", WS_A, "--group", GROUP_A])
        .success();

    // --- Cleanup ---
    fixture
        .swayg(&["workspace", "unglobal", WS_GLOBAL])
        .success();
    drop(_win_g);
    std::thread::sleep(std::time::Duration::from_millis(300));

    fixture
        .swayg(&[
            "group", "select", &orig_group, "--output", &fixture.orig_output, "--create",
        ])
        .success();
    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    drop(_win_a);
    drop(_win_b);
    drop(_win_c);
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(workspace_of_window(WS_A).is_none(), "WS_A window is gone");
    assert!(workspace_of_window(WS_B).is_none(), "WS_B window is gone");
    assert!(workspace_of_window(WS_C).is_none(), "WS_C window is gone");
    assert!(workspace_of_window(WS_GLOBAL).is_none(), "WS_GLOBAL window is gone");

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    let groups_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        ),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}', '{}', '{}')",
            WS_A, WS_B, WS_C, WS_GLOBAL
        ),
    );
    let hidden_gone = db_count(&fixture.db_path, "SELECT count(*) FROM hidden_workspaces");
    assert_eq!(groups_gone, 0, "no test groups remain");
    assert_eq!(ws_gone, 0, "no test workspaces remain");
    assert_eq!(hidden_gone, 0, "no hidden entries remain");
}
