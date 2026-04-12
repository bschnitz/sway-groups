use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{get_focused_workspace, DummyWindowHandle, TestFixture};

const GROUP_A: &str = "zz_test_pa";
const GROUP_B: &str = "zz_test_pb";
const GROUP_C: &str = "zz_test_pc";
const GROUP_D: &str = "zz_test_pd";
const GROUP_E: &str = "zz_test_pe";
const GROUP_F: &str = "zz_test_pf";
const WS1: &str = "zz_tg_ws1_prune";

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

fn window_count_in_tree(app_id: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse tree");
    let mut count = 0i64;
    fn find(node: &serde_json::Value, app_id: &str, count: &mut i64) {
        if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
            *count += 1;
        }
        for key in &["nodes", "floating_nodes"] {
            if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
                for child in children {
                    find(child, app_id, count);
                }
            }
        }
    }
    find(&tree, app_id, &mut count);
    count
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
async fn test_16_group_prune() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    // Get original group from REAL db (before init)
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

    // --- 1. Precondition checks (BEFORE init) ---
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    for g in [GROUP_A, GROUP_B, GROUP_C, GROUP_D, GROUP_E, GROUP_F] {
        if real_db.exists() {
            assert_eq!(
                db_count(
                    &real_db,
                    &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)
                ),
                0,
                "precondition: {} must not exist in real DB",
                g
            );
        }
    }

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
            ),
            0,
            "precondition: {} must not exist in real DB",
            WS1
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);

    // --- 2. Setup: init + create 4 groups + dummy window in group A + switch back ---
    fixture.init().success();

    fixture
        .swayg(&["group", "create", GROUP_A])
        .success();
    fixture
        .swayg(&["group", "create", GROUP_B])
        .success();
    fixture
        .swayg(&["group", "create", GROUP_C])
        .success();
    fixture
        .swayg(&["group", "create", GROUP_D])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_A,
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
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- 3. Verify setup ---
    for g in [GROUP_A, GROUP_B, GROUP_C, GROUP_D] {
        assert_eq!(
            db_count(
                &fixture.db_path,
                &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)
            ),
            1,
            "group '{}' exists",
            g
        );
    }

    assert!(
        window_count_in_tree(WS1) >= 1,
        "dummy window '{}' is running",
        WS1
    );

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "'{}' is in group '{}'",
        WS1,
        GROUP_A
    );

    for g in [GROUP_B, GROUP_C, GROUP_D] {
        assert_eq!(
            workspace_in_group_count(&fixture.db_path, WS1, g),
            0,
            "'{}' NOT in group '{}'",
            WS1,
            g
        );
    }

    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name = '0'"
        ),
        1,
        "group '0' exists"
    );

    // --- 4. Test: group prune (no --keep) ---
    fixture.swayg(&["group", "prune"]).success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group '{}' still exists (has {})",
        GROUP_A,
        WS1
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        0,
        "group '{}' pruned",
        GROUP_B
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_C)
        ),
        0,
        "group '{}' pruned",
        GROUP_C
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_D)
        ),
        0,
        "group '{}' pruned",
        GROUP_D
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name = '0'"
        ),
        1,
        "group '0' NOT pruned (default group)"
    );

    // --- 5. Test: group prune with --keep ---
    // Create groups E and F via sqlite3
    let _ = Command::new("sqlite3")
        .arg(&fixture.db_path)
        .arg(&format!(
            "INSERT INTO groups (name, created_at, updated_at) VALUES ('{}', datetime('now'), datetime('now'))",
            GROUP_E
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("sqlite3")
        .arg(&fixture.db_path)
        .arg(&format!(
            "INSERT INTO groups (name, created_at, updated_at) VALUES ('{}', datetime('now'), datetime('now'))",
            GROUP_F
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_E)
        ),
        1,
        "group '{}' exists",
        GROUP_E
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_F)
        ),
        1,
        "group '{}' exists",
        GROUP_F
    );

    fixture
        .swayg(&["group", "prune", "--keep", GROUP_E])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_E)
        ),
        1,
        "group '{}' kept (--keep)",
        GROUP_E
    );

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_F)
        ),
        0,
        "group '{}' pruned (not in --keep)",
        GROUP_F
    );

    // --- 6. Cleanup: kill dummy window, auto-delete group A ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        window_count_in_tree(WS1),
        0,
        "dummy window '{}' is gone",
        WS1
    );

    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();
    fixture
        .swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        0,
        "'{}' auto-deleted",
        GROUP_A
    );

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace '{}'",
        orig_ws
    );

    // --- 7. Post-condition ---
    fixture.init().success();

    let groups_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}', '{}', '{}', '{}')",
            GROUP_A, GROUP_B, GROUP_C, GROUP_D, GROUP_E, GROUP_F
        ),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1),
    );
    assert_eq!(
        (groups_gone, ws_gone),
        (0, 0),
        "no test data remains in DB"
    );
}
