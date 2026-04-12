use std::process::{Command, Stdio};

use sway_groups_tests::common::{get_focused_workspace, swayg_output, workspace_of_window, DummyWindowHandle, TestFixture};

const GROUP_A: &str = "zz_test_move_a";
const GROUP_B: &str = "zz_test_move_b";
const WS1: &str = "zz_test_ws1_mov";

fn db_count(db_path: &std::path::PathBuf, sql: &str) -> i64 {
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

fn workspace_in_group_count(db_path: &std::path::PathBuf, ws: &str, group: &str) -> i64 {
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
async fn test_04_workspace_move() {
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

    // --- Precondition: no test data in real DB ---
    let real_db =
        dirs::data_dir().unwrap_or_default().join("swayg").join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
            0,
            "{} must not exist in production DB",
            GROUP_A
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)),
            0,
            "{} must not exist in production DB",
            GROUP_B
        );
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
            0,
            "{} must not exist in production DB",
            WS1
        );
    }

    assert!(!workspace_exists_in_sway(WS1), "{} must not exist in sway", WS1);

    // --- Init ---
    fixture.init().success();

    // --- Step 4: Create group A and add workspace ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_A,
            "--create",
            "--output",
            &fixture.orig_output,
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group A was created"
    );

    let _kitty = DummyWindowHandle::spawn(WS1).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS1).is_some(),
        "dummy window '{}' exists in sway tree",
        WS1
    );

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS1,
        "focused on WS1"
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in group {}",
        WS1,
        GROUP_A
    );

    // --- Step 5: Move workspace to group B ---
    fixture
        .swayg(&["workspace", "move", WS1, "--groups", GROUP_B])
        .success();

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_B),
        1,
        "{} is now in group {}",
        WS1,
        GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS1, GROUP_A),
        0,
        "{} is no longer in group {}",
        WS1,
        GROUP_A
    );

    // --- Step 6: Switch to group B (auto-delete group A) ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_B,
            "--create",
            "--output",
            &fixture.orig_output,
        ])
        .success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        0,
        "{} auto-deleted",
        GROUP_A
    );

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
        visible.lines().any(|l| l.contains(WS1)),
        "{} is visible in group {}",
        WS1,
        GROUP_B
    );

    // --- Step 7: Switch to original group ---
    fixture
        .swayg(&[
            "group",
            "select",
            &orig_group,
            "--output",
            &fixture.orig_output,
        ])
        .success();

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        1,
        "{} NOT auto-deleted (still has workspaces)",
        GROUP_B
    );

    // --- Step 8: Kill dummy window, auto-delete group B ---
    drop(_kitty);
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        !workspace_exists_in_sway(WS1),
        "{} is gone from sway",
        WS1
    );

    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output])
        .success();

    fixture
        .swayg(&[
            "group",
            "select",
            &orig_group,
            "--output",
            &fixture.orig_output,
        ])
        .success();

    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(
        get_focused_workspace().unwrap(),
        orig_ws,
        "focused on original workspace after cleanup"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        0,
        "{} auto-deleted",
        GROUP_B
    );

    // --- Post-condition: no test data remains ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                GROUP_A, GROUP_B
            )
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)
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
                 WHERE g.name IN ('{}', '{}')",
                GROUP_A, GROUP_B
            )
        ),
        0,
        "no test workspace_groups remain"
    );
}
