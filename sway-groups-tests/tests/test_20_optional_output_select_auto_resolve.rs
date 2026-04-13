use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    create_virtual_output, get_focused_output, get_focused_workspace, swayg_output,
    unplug_output, workspace_of_window, DummyWindowHandle, TestFixture,
};

const GROUP: &str = "zz_test_oo_sel";
const WS: &str = "zz_tg_oo_sel_ws";

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
async fn test_20_optional_output_select_auto_resolve() {
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

    // Clean up stale workspaces/outputs from previous failed runs
    if workspace_exists_in_sway(WS) {
        let _ = Command::new("swaymsg")
            .args(["workspace", WS])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = Command::new("swaymsg")
            .args(["workspace", "back_and_forth"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let outputs = Command::new("swaymsg")
        .args(["-t", "get_outputs"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let all_outputs: Vec<String> = serde_json::from_slice::<serde_json::Value>(&outputs.stdout)
        .expect("parse outputs")
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
        .filter(|n| n != &fixture.orig_output)
        .collect();
    for o in &all_outputs {
        let _ = Command::new("swaymsg")
            .args(["output", o, "unplug"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
            ),
            0,
            "precondition: {} must not exist in production DB",
            GROUP
        );
    }
    assert!(
        !workspace_exists_in_sway(WS),
        "precondition: {} must not exist in sway",
        WS
    );

    // --- Create virtual output ---
    let virtual_output = create_virtual_output().expect("create virtual output");

    // --- Init ---
    fixture.init().success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test group after init"
    );

    // --- Setup: group on orig_output ---
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
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' created",
        GROUP
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP,
        "active group = '{}'",
        GROUP
    );

    // --- Setup: dummy window + move to workspace ---
    let _win = DummyWindowHandle::spawn(WS).expect("spawn");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS).is_some(),
        "dummy window '{}' in sway",
        WS
    );

    fixture
        .swayg(&["container", "move", WS, "--switch-to-workspace"])
        .success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS)
        ),
        1,
        "'{}' exists in DB",
        WS
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS, GROUP),
        1,
        "'{}' in group '{}'",
        WS,
        GROUP
    );
    assert!(workspace_exists_in_sway(WS), "'{}' exists in sway", WS);

    // --- Focus virtual output ---
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

    // --- TEST: group select without --output (auto-resolve from group_state) ---
    fixture.swayg(&["group", "select", GROUP]).success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP,
        "active group = '{}' after auto-resolve",
        GROUP
    );
    assert_eq!(
        get_focused_output().expect("get focused output"),
        fixture.orig_output,
        "output switched back to '{}'",
        fixture.orig_output
    );

    // --- Cleanup: switch back to default group on test DB ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &fixture.orig_workspace])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // --- Cleanup: kill window ---
    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        workspace_of_window(WS).is_none(),
        "dummy window '{}' gone from sway",
        WS
    );

    // --- Cleanup: auto-delete test group on test DB ---
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output])
        .success();
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "test group auto-deleted"
    );

    // --- Cleanup: remove virtual output ---
    unplug_output(&virtual_output);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Post-condition ---
    fixture.init().success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS)
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
                 WHERE g.name = '{}'",
                GROUP
            )
        ),
        0,
        "no test workspace_groups remain"
    );
    // --- Cleanup: restore original group on live DB ---
    use sway_groups_tests::common::swayg_live;
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &fixture.orig_workspace])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(
        get_focused_workspace().unwrap(),
        fixture.orig_workspace,
        "focused on original workspace after test"
    );
}
