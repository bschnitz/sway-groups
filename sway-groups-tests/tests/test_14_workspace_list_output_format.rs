use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    swayg_output, get_focused_workspace, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_vis__";
const WS_A: &str = "zz_tg_vis__";
const WS_B: &str = "zz_tg_hid__";

fn db_count(db_path: &PathBuf, sql: &str) -> i64 {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn db_query(db_path: &PathBuf, sql: &str) -> String {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn workspace_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
    db_count(
        db_path,
        &format!(
            "SELECT count(*) FROM workspace_groups wg \
             JOIN groups g ON g.id = wg.group_id \
             JOIN workspaces w ON w.id = wg.workspace_id \
             WHERE w.name = '{}' AND g.name = '{}'",
            ws, group
        ),
    )
}

fn workspace_exists_in_sway(ws: &str) -> bool {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let workspaces: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse workspaces");
    workspaces
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.get("name").and_then(|n| n.as_str()) == Some(ws))
}

fn orig_active_group(output_name: &str) -> String {
    let out = Command::new("swayg")
        .args(["group", "active", output_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swayg group active failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn output_contains(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|line| line.contains(needle))
}

fn line_starts_with(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|line| line.trim_start().starts_with(needle))
}

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
        .swayg(&["group", "select", &fixture.orig_output, GROUP, "--create"])
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
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
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
            workspace_in_group_count(&fixture.db_path, ws, GROUP),
            1,
            "'{}' in group '{}'",
            ws, GROUP
        );
    }
    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace"
    );

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
        .swayg(&["group", "select", &fixture.orig_output, GROUP])
        .success();
    fixture
        .swayg(&["group", "select", &fixture.orig_output, &orig_group])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0,
        "'{}' auto-deleted",
        GROUP
    );

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace"
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
}
