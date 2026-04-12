use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    create_virtual_output, get_focused_output, get_focused_workspace, swayg_output,
    unplug_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_onext_a";
const GROUP_B: &str = "zz_test_onext_b";
const WS_A: &str = "zz_tg_onext_a";
const WS_B: &str = "zz_tg_onext_b";

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

#[tokio::test]
async fn test_21_optional_output_next_prev_auto_resolve() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

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

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!(
                    "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                    GROUP_A, GROUP_B
                )
            ),
            0,
            "precondition: test groups must not exist in production DB"
        );
    }
    assert!(
        !workspace_exists_in_sway(WS_A),
        "precondition: {} must not exist in sway",
        WS_A
    );
    assert!(
        !workspace_exists_in_sway(WS_B),
        "precondition: {} must not exist in sway",
        WS_B
    );

    // --- Create virtual output ---
    let virtual_output = create_virtual_output().expect("create virtual output");

    // --- Init ---
    fixture.init().success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                GROUP_A, GROUP_B
            )
        ),
        0,
        "no test groups after init"
    );

    // --- Setup: GROUP_A on orig_output ---
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
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        1,
        "group '{}' created",
        GROUP_A
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP_A,
        "active group = '{}'",
        GROUP_A
    );

    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS_A).is_some(),
        "dummy window '{}' in sway",
        WS_A
    );

    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS_A)
        ),
        1,
        "'{}' exists in DB",
        WS_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_A, GROUP_A),
        1,
        "'{}' in group '{}'",
        WS_A,
        GROUP_A
    );
    assert!(workspace_exists_in_sway(WS_A), "'{}' exists in sway", WS_A);

    // --- Setup: GROUP_B on orig_output ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_B,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        1,
        "group '{}' created",
        GROUP_B
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP_B,
        "active group = '{}'",
        GROUP_B
    );

    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn WS_B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS_B).is_some(),
        "dummy window '{}' in sway",
        WS_B
    );

    fixture
        .swayg(&["container", "move", WS_B, "--switch-to-workspace"])
        .success();
    assert!(workspace_exists_in_sway(WS_B), "'{}' exists in sway", WS_B);

    // --- Setup: visit GROUP_B on virtual output (makes it last_visited) ---
    let _ = Command::new("swaymsg")
        .args(["focus", "output", &virtual_output])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        get_focused_output().expect("get focused output"),
        virtual_output,
        "focused output = '{}'",
        virtual_output
    );

    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &virtual_output])
        .success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &virtual_output]),
        GROUP_B,
        "active group on virtual output = '{}'",
        GROUP_B
    );
    let last_output_b = db_query(
        &fixture.db_path,
        &format!(
            "SELECT output FROM group_state WHERE group_name = '{}' ORDER BY last_visited DESC LIMIT 1",
            GROUP_B
        ),
    );
    assert_eq!(
        last_output_b, virtual_output,
        "GROUP_B last_visited output = '{}'",
        virtual_output
    );

    // --- Setup: return to GROUP_A on orig_output (test starting position) ---
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP_A,
            "--output",
            &fixture.orig_output,
        ])
        .success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP_A,
        "active group = '{}'",
        GROUP_A
    );
    assert_eq!(
        get_focused_output().expect("get focused output"),
        fixture.orig_output,
        "focused output = '{}'",
        fixture.orig_output
    );

    // --- TEST: group next without --output (GROUP_A → GROUP_B, auto-resolve to virtual output) ---
    fixture.swayg(&["group", "next"]).success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &virtual_output]),
        GROUP_B,
        "active group = '{}' after group next",
        GROUP_B
    );
    assert_eq!(
        get_focused_output().expect("get focused output"),
        virtual_output,
        "output switched to virtual output (cross-output)"
    );

    // --- TEST: group prev without --output (GROUP_B → GROUP_A, auto-resolve to orig_output) ---
    fixture.swayg(&["group", "prev"]).success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP_A,
        "active group = '{}' after group prev",
        GROUP_A
    );
    assert_eq!(
        get_focused_output().expect("get focused output"),
        fixture.orig_output,
        "output switched back to '{}'",
        fixture.orig_output
    );

    // --- Cleanup: switch back to original group ---
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
        fixture.orig_workspace,
        "focused on original workspace"
    );

    // --- Cleanup: kill windows ---
    drop(_win_a);
    drop(_win_b);
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS_A).is_none(),
        "dummy window '{}' gone from sway",
        WS_A
    );
    assert!(
        workspace_of_window(WS_B).is_none(),
        "dummy window '{}' gone from sway",
        WS_B
    );

    // --- Cleanup: auto-delete GROUP_A ---
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
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
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)
        ),
        0,
        "GROUP_A auto-deleted"
    );

    // --- Cleanup: auto-delete GROUP_B ---
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
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_B)
        ),
        0,
        "GROUP_B auto-deleted"
    );

    // --- Cleanup: remove virtual output ---
    unplug_output(&virtual_output);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Post-condition ---
    fixture.init().success();
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
            &format!(
                "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}')",
                WS_A, WS_B
            )
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
                 JOIN workspaces w ON w.id = wg.workspace_id \
                 WHERE g.name IN ('{}', '{}')",
                GROUP_A, GROUP_B
            )
        ),
        0,
        "no test workspace_groups remain"
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        fixture.orig_workspace,
        "focused on original workspace after test"
    );
}
