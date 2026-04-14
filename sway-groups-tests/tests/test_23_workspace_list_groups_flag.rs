use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    swayg_output, get_focused_workspace, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_lgf_a";
const GROUP_B: &str = "zz_test_lgf_b";
const WS_MULTI: &str = "zz_tg_lgf_multi";
const WS_SINGLE: &str = "zz_tg_lgf_single";
const WS_NONE: &str = "zz_tg_lgf_none";

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

#[tokio::test]
async fn test_23_workspace_list_groups_flag() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        for g in [GROUP_A, GROUP_B] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0,
                "{} must not exist in production DB",
                g
            );
        }
        for ws in [WS_MULTI, WS_SINGLE, WS_NONE] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0,
                "{} must not exist in production DB",
                ws
            );
        }
    }

    for ws in [WS_MULTI, WS_SINGLE, WS_NONE] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Remember original state ---
    let orig_group = {
        let output = Command::new("swayg")
            .args(["group", "active", &fixture.orig_output])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .expect("swayg group active failed");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init + groups ---
    fixture.init().success();

    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();

    // Create WS_MULTI: container move (adds to GROUP_A automatically), then add to GROUP_B
    let _win_multi = DummyWindowHandle::spawn(WS_MULTI).expect("spawn WS_MULTI");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_MULTI])
        .success();
    fixture
        .swayg(&["workspace", "add", WS_MULTI, "--group", GROUP_B])
        .success();

    // Create WS_SINGLE: container move (adds to GROUP_A automatically)
    let _win_single = DummyWindowHandle::spawn(WS_SINGLE).expect("spawn WS_SINGLE");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_SINGLE])
        .success();

    // Create WS_NONE: container move, sync to DB, then remove from group
    let _win_none = DummyWindowHandle::spawn(WS_NONE).expect("spawn WS_NONE");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_NONE])
        .success();
    fixture
        .swayg(&["sync", "--workspaces"])
        .success();
    fixture
        .swayg(&["workspace", "remove", WS_NONE, "--group", GROUP_A])
        .success();

    // Switch back to original group
    fixture
        .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output, "--create"])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_MULTI, GROUP_A),
        1,
        "'{}' in {}",
        WS_MULTI, GROUP_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_MULTI, GROUP_B),
        1,
        "'{}' in {}",
        WS_MULTI, GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_SINGLE, GROUP_A),
        1,
        "'{}' in {}",
        WS_SINGLE, GROUP_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_SINGLE, GROUP_B),
        0,
        "'{}' NOT in {}",
        WS_SINGLE, GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_NONE, GROUP_A),
        0,
        "'{}' NOT in {}",
        WS_NONE, GROUP_A
    );

    // --- Test: workspace list --plain (no groups column) ---
    let plain_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain"],
    );
    assert!(
        plain_out.lines().any(|l| l.trim() == WS_MULTI || l.trim() == WS_SINGLE || l.trim() == WS_NONE),
        "workspaces appear in plain output without groups"
    );
    assert!(
        !plain_out.contains("│"),
        "no │ separator in plain output without --groups"
    );

    // --- Test: workspace list --plain --groups ---
    let groups_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain", "--groups"],
    );

    // WS_MULTI should be in both groups
    let multi_line = groups_out.lines().find(|l| l.starts_with(&format!("{}│", WS_MULTI)));
    assert!(
        multi_line.is_some(),
        "'{}' appears in --groups output",
        WS_MULTI
    );
    let multi_line = multi_line.unwrap();
    assert!(
        multi_line.contains(GROUP_A) && multi_line.contains(GROUP_B),
        "'{}' line contains both groups: {}",
        WS_MULTI,
        multi_line
    );

    // WS_SINGLE should be in only GROUP_A
    let single_line = groups_out.lines().find(|l| l.starts_with(&format!("{}│", WS_SINGLE)));
    assert!(
        single_line.is_some(),
        "'{}' appears in --groups output",
        WS_SINGLE
    );
    let single_line = single_line.unwrap();
    assert!(
        single_line.contains(GROUP_A) && !single_line.contains(GROUP_B),
        "'{}' line contains only {}: {}",
        WS_SINGLE,
        GROUP_A,
        single_line
    );

    // WS_NONE should have empty groups (just "name│")
    let none_line = groups_out.lines().find(|l| l.starts_with(&format!("{}│", WS_NONE)));
    assert!(
        none_line.is_some(),
        "'{}' appears in --groups output",
        WS_NONE
    );
    let none_line = none_line.unwrap();
    let after_pipe = none_line.split('│').nth(1).unwrap_or("");
    assert!(
        after_pipe.is_empty(),
        "'{}' has no groups (empty after │): {}",
        WS_NONE,
        none_line
    );

    // --- Test: workspace list --plain --groups --flatten ---
    // Switch to GROUP_B to test active-group-first sorting
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    let flatten_out = swayg_output(
        &fixture.db_path,
        &["workspace", "list", "--plain", "--groups", "--flatten"],
    );

    // WS_MULTI: two lines (one per group), GROUP_B first (active), then GROUP_A
    let mut multi_lines: Vec<&str> = flatten_out.lines()
        .filter(|l| l.starts_with(&format!("{}│", WS_MULTI)))
        .collect();
    assert_eq!(
        multi_lines.len(), 2,
        "'{}' has 2 lines in --flatten output",
        WS_MULTI
    );
    // Active group (GROUP_B) first
    assert_eq!(
        multi_lines[0],
        format!("{}│{}", WS_MULTI, GROUP_B),
        "'{}' first line has active group {}",
        WS_MULTI, GROUP_B
    );
    assert_eq!(
        multi_lines[1],
        format!("{}│{}", WS_MULTI, GROUP_A),
        "'{}' second line has other group {}",
        WS_MULTI, GROUP_A
    );

    // WS_SINGLE: one line (only GROUP_A)
    let single_lines: Vec<&str> = flatten_out.lines()
        .filter(|l| l.starts_with(&format!("{}│", WS_SINGLE)))
        .collect();
    assert_eq!(
        single_lines.len(), 1,
        "'{}' has 1 line in --flatten output",
        WS_SINGLE
    );
    assert_eq!(
        single_lines[0],
        format!("{}│{}", WS_SINGLE, GROUP_A),
        "'{}' line has group {}",
        WS_SINGLE, GROUP_A
    );

    // WS_NONE: no lines (empty groups, nothing to flatten)
    let none_lines: Vec<&str> = flatten_out.lines()
        .filter(|l| l.starts_with(&format!("{}│", WS_NONE)))
        .collect();
    assert_eq!(
        none_lines.len(), 0,
        "'{}' has 0 lines in --flatten output (no groups)",
        WS_NONE
    );

    // Switch back to orig_group
    fixture
        .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output, "--create"])
        .success();

    // --- Cleanup ---
    fixture.swayg(&["nav", "go", &orig_ws]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    drop(_win_multi);
    drop(_win_single);
    drop(_win_none);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS_MULTI), "'{}' is gone from sway", WS_MULTI);
    assert!(!workspace_exists_in_sway(WS_SINGLE), "'{}' is gone from sway", WS_SINGLE);
    assert!(!workspace_exists_in_sway(WS_NONE), "'{}' is gone from sway", WS_NONE);

    // --- Auto-delete empty groups ---
    for g in [GROUP_A, GROUP_B] {
        fixture
            .swayg(&["group", "select", g, "--output", &fixture.orig_output])
            .success();
        fixture
            .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output, "--create"])
            .success();
    }

    assert_eq!(
        db_count(&fixture.db_path, &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
            GROUP_A, GROUP_B
        )),
        0,
        "test groups auto-deleted"
    );

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace"
    );

    // --- Post-condition: init to sync DB state ---
    fixture.init().success();

    for g in [GROUP_A, GROUP_B] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
            0,
            "no test groups remain"
        );
    }
    for ws in [WS_MULTI, WS_SINGLE, WS_NONE] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
            0,
            "no test workspaces remain"
        );
    }
}
