use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    swayg_output, get_focused_workspace, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_repair__";
const GROUP_EMPTY: &str = "zz_test_empty__";
const WS1: &str = "zz_tg_ws1__";
const WS_STALE: &str = "zz_tg_stale__";

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

fn db_exec(db_path: &PathBuf, sql: &str) {
    Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("sqlite3 exec failed");
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

fn output_contains(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|line| line.contains(needle))
}

#[tokio::test]
async fn test_12_repair() {
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
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_EMPTY)),
            0,
            "{} must not exist in production DB",
            GROUP_EMPTY
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "{} must not exist in production DB",
            WS1
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_STALE)),
            0,
            "{} must not exist in production DB",
            WS_STALE
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "{} must not exist in sway", WS1);
    assert!(!workspace_exists_in_sway(WS_STALE), "{} must not exist in sway", WS_STALE);

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init + group + dummy window + move + switch back + DB manipulation ---
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

    let _win = DummyWindowHandle::spawn(WS1).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    fixture
        .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&["group", "create", GROUP_EMPTY])
        .success();

    // Insert stale workspace into DB (exists in DB but not in sway)
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO workspaces (name, is_global, created_at, updated_at) VALUES ('{}', 0, datetime('now'), datetime('now'));",
            WS_STALE
        ),
    );
    // Associate stale workspace with GROUP
    db_exec(
        &fixture.db_path,
        &format!(
            "INSERT INTO workspace_groups (workspace_id, group_id, created_at) \
             SELECT w.id, g.id, datetime('now') FROM workspaces w, groups g \
             WHERE w.name = '{}' AND g.name = '{}';",
            WS_STALE, GROUP
        ),
    );
    // Remove WS1 from DB (exists in sway but not in DB)
    db_exec(
        &fixture.db_path,
        &format!(
            "DELETE FROM workspace_groups WHERE workspace_id IN (SELECT id FROM workspaces WHERE name = '{}');",
            WS1
        ),
    );
    db_exec(
        &fixture.db_path,
        &format!("DELETE FROM workspaces WHERE name = '{}';", WS1),
    );
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        1,
        "group '{}' exists",
        GROUP
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_EMPTY)),
        1,
        "group '{}' exists",
        GROUP_EMPTY
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_STALE)),
        1,
        "'{}' in DB (not in sway)",
        WS_STALE
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        0,
        "'{}' NOT in DB (removed)",
        WS1
    );
    assert!(workspace_exists_in_sway(WS1), "'{}' still in sway", WS1);
    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace"
    );

    // --- Test: repair ---
    fixture.swayg(&["repair"]).success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_STALE)),
        0,
        "'{}' removed from DB (was not in sway)",
        WS_STALE
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        1,
        "'{}' re-added to DB (found in sway)",
        WS1
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id \
                 JOIN workspaces w ON w.id = wg.workspace_id \
                 WHERE w.name = '{}' AND g.name = '0'",
                WS1
            )
        ),
        1,
        "'{}' added to default group '0'",
        WS1
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_EMPTY)),
        0,
        "'{}' pruned (was effectively empty)",
        GROUP_EMPTY
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0,
        "'{}' pruned (WS1 was removed from DB, group effectively empty)",
        GROUP
    );

    // --- Test: workspace list shows repaired workspace ---
    let visible = swayg_output(
        &fixture.db_path,
        &[
            "workspace",
            "list",
            "--visible",
            "--plain",
            "--output",
            &fixture.orig_output,
        ],
    );
    assert!(
        output_contains(&visible, WS1),
        "'{}' visible after repair",
        WS1
    );

    // --- Cleanup ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!workspace_exists_in_sway(WS1), "'{}' is gone from sway", WS1);

    // --- Post-condition: init to sync DB state ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM groups WHERE name IN ('{}', '{}')", GROUP, GROUP_EMPTY),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')", WS1, WS_STALE),
    );
    assert_eq!(group_gone, 0, "no test groups remain");
    assert_eq!(ws_gone, 0, "no test workspaces remain");
}
