use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    swayg_output, swayg_live, get_focused_workspace, DummyWindowHandle, TestFixture,
};

const GROUP_A: &str = "zz_test_ga__";
const GROUP_B: &str = "zz_test_gb__";
const GROUP_C: &str = "zz_test_gc__";
const WS_A: &str = "zz_tg_a__";
const WS_B: &str = "zz_tg_b__";
const WS_C: &str = "zz_tg_c__";

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

fn orig_active_group(output_name: &str) -> String {
    let out = Command::new("swayg")
        .args(["group", "active", output_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swayg group active failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn get_active_group(db_path: &PathBuf, output: &str) -> String {
    swayg_output(db_path, &["group", "active", output])
}

#[tokio::test]
async fn test_13_group_next_prev() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    // --- Precondition: no test data in production DB ---
    if real_db.exists() {
        for g in [GROUP_A, GROUP_B, GROUP_C] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
                0,
                "{} must not exist in production DB",
                g
            );
        }
        for ws in [WS_A, WS_B, WS_C] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0,
                "{} must not exist in production DB",
                ws
            );
        }
    }

    for ws in [WS_A, WS_B, WS_C] {
        assert!(!workspace_exists_in_sway(ws), "{} must not exist in sway", ws);
    }

    // --- Remember original state ---
    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");
    let orig_ws = get_focused_workspace().expect("get focused workspace");

    // --- Setup: init + create 3 groups + launch 3 dummy windows + move containers ---
    fixture.init().success();

    // Group A + WS_A
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();
    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn WS_A");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_A, "--switch-to-workspace"])
        .success();

    // Group B + WS_B
    fixture
        .swayg(&["group", "select", GROUP_B, "--output", &fixture.orig_output, "--create"])
        .success();
    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn WS_B");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_B, "--switch-to-workspace"])
        .success();

    // Group C + WS_C
    fixture
        .swayg(&["group", "select", GROUP_C, "--output", &fixture.orig_output, "--create"])
        .success();
    let _win_c = DummyWindowHandle::spawn(WS_C).expect("spawn WS_C");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture
        .swayg(&["container", "move", WS_C, "--switch-to-workspace"])
        .success();

    // Switch back to group A
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output])
        .success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify setup ---
    for g in [GROUP_A, GROUP_B, GROUP_C] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", g)),
            1,
            "group '{}' exists",
            g
        );
    }

    assert!(_win_a.exists_in_tree(), "dummy window '{}' is running", WS_A);
    assert!(_win_b.exists_in_tree(), "dummy window '{}' is running", WS_B);
    assert!(_win_c.exists_in_tree(), "dummy window '{}' is running", WS_C);

    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_A, GROUP_A),
        1,
        "'{}' in group '{}'",
        WS_A, GROUP_A
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_B, GROUP_B),
        1,
        "'{}' in group '{}'",
        WS_B, GROUP_B
    );
    assert_eq!(
        workspace_in_group_count(&fixture.db_path, WS_C, GROUP_C),
        1,
        "'{}' in group '{}'",
        WS_C, GROUP_C
    );

    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_A,
        "active group = '{}'",
        GROUP_A
    );

    // --- Test: group next (A → B) ---
    fixture
        .swayg(&["group", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_B,
        "active group = '{}'",
        GROUP_B
    );

    // --- Test: group next (B → C) ---
    fixture
        .swayg(&["group", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_C,
        "active group = '{}'",
        GROUP_C
    );

    // --- Test: group next without wrap at boundary (C → stays) ---
    fixture
        .swayg(&["group", "next", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_C,
        "still '{}' (no wrap, at boundary)",
        GROUP_C
    );

    // --- Test: group next with wrap (C → "0") ---
    fixture
        .swayg(&["group", "next", "--output", &fixture.orig_output, "--wrap"])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        "0",
        "active group = '0' after wrap (past end to first)"
    );

    // --- Test: group prev from "0" without wrap (boundary → stays) ---
    fixture
        .swayg(&["group", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        "0",
        "still '0' (no wrap prev, at start)"
    );

    // --- Test: group prev with wrap ("0" → C) ---
    fixture
        .swayg(&["group", "prev", "--output", &fixture.orig_output, "--wrap"])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_C,
        "active group = '{}' after wrap prev",
        GROUP_C
    );

    // --- Test: group prev (C → B) ---
    fixture
        .swayg(&["group", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_B,
        "active group = '{}' after prev",
        GROUP_B
    );

    // --- Test: group prev (B → A) ---
    fixture
        .swayg(&["group", "prev", "--output", &fixture.orig_output])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_A,
        "active group = '{}' after prev",
        GROUP_A
    );

    // --- Test: group next-on-output --wrap (A → next non-empty group on output) ---
    fixture
        .swayg(&["group", "next-on-output", "--wrap"])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_B,
        "active group = '{}' after next-on-output",
        GROUP_B
    );

    // --- Test: group prev-on-output --wrap (B → A) ---
    fixture
        .swayg(&["group", "prev-on-output", "--wrap"])
        .success();
    assert_eq!(
        get_active_group(&fixture.db_path, &fixture.orig_output),
        GROUP_A,
        "active group = '{}' after prev-on-output",
        GROUP_A
    );

    // --- Cleanup: switch to original workspace FIRST (so sway can remove empty workspaces) ---
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    drop(_win_a);
    drop(_win_b);
    drop(_win_c);

    // Wait for sway to remove empty workspaces
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !workspace_exists_in_sway(WS_A) && !workspace_exists_in_sway(WS_B) && !workspace_exists_in_sway(WS_C) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    assert!(!workspace_exists_in_sway(WS_A), "'{}' is gone from sway", WS_A);
    assert!(!workspace_exists_in_sway(WS_B), "'{}' is gone from sway", WS_B);
    assert!(!workspace_exists_in_sway(WS_C), "'{}' is gone from sway", WS_C);

    // --- Auto-delete empty groups ---
    for g in [GROUP_A, GROUP_B, GROUP_C] {
        fixture
            .swayg(&["group", "select", g, "--output", &fixture.orig_output])
            .success();
        fixture
            .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
            .success();
    }

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}')",
                GROUP_A, GROUP_B, GROUP_C
            ),
        ),
        0,
        "all test groups auto-deleted"
    );

    // --- Post-condition: init to sync DB state ---
    fixture.init().success();

    let group_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM groups WHERE name IN ('{}', '{}', '{}')",
            GROUP_A, GROUP_B, GROUP_C
        ),
    );
    let ws_gone = db_count(
        &fixture.db_path,
        &format!(
            "SELECT count(*) FROM workspaces WHERE name IN ('{}', '{}', '{}')",
            WS_A, WS_B, WS_C
        ),
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
