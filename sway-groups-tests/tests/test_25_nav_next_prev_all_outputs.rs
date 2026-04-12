use std::path::PathBuf;
use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    swayg_output, get_focused_workspace, workspace_of_window, DummyWindowHandle, TestFixture,
    create_virtual_output, unplug_output,
};

const GROUP: &str = "zz_test_nav_ao";
const WS1: &str = "zz_tg_ao_ws1";
const WS2: &str = "zz_tg_ao_ws2";

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

#[tokio::test]
async fn test_25_nav_next_prev_all_outputs() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    if real_db.exists() {
        assert_eq!(
            db_count(&real_db, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
            0
        );
        for ws in [WS1, WS2] {
            assert_eq!(
                db_count(&real_db, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
                0
            );
        }
    }

    let orig_group = swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]);
    assert!(!orig_group.is_empty(), "original group not empty");
    let orig_ws = get_focused_workspace().expect("focused ws");

    let virt_output = create_virtual_output().expect("virtual output");
    assert_ne!(virt_output, fixture.orig_output, "different outputs");

    fixture.init().success();

    // Set GROUP as active on BOTH outputs
    fixture
        .swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
        .success();
    fixture
        .swayg(&["group", "select", GROUP, "--output", &virt_output])
        .success();

    // WS1 on orig_output, WS2 on virt_output, both in GROUP
    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn ws1");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS1, "--switch-to-workspace"]).success();

    // Switch to virt_output, create WS2 there
    fixture.swayg(&["group", "select", GROUP, "--output", &virt_output]).success();

    let _win2 = DummyWindowHandle::spawn(WS2).expect("spawn ws2");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS2, "--switch-to-workspace"]).success();

    // Switch back to orig_output, focus WS1
    fixture.swayg(&["group", "select", GROUP, "--output", &fixture.orig_output]).success();
    fixture.swayg(&["nav", "go", WS1]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(get_focused_workspace().unwrap(), WS1, "on WS1");

    // --- nav next (single output) only sees WS1 ---
    // WS2 is on virt_output, not visible here
    fixture.swayg(&["nav", "next"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS1, "next single-output wraps to WS1 (only WS1 visible)");

    // --- nav next --all-outputs sees WS2 on virt_output ---
    fixture.swayg(&["nav", "next", "--all-outputs"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS2, "next --all-outputs → WS2 (cross-output)");

    // --- nav prev --all-outputs goes back to WS1 ---
    fixture.swayg(&["nav", "prev", "--all-outputs"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS1, "prev --all-outputs → WS1");

    // --- nav next --all-outputs --wrap from WS2 wraps to WS1 ---
    fixture.swayg(&["nav", "next", "--all-outputs"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS2, "next --all-outputs → WS2");

    fixture.swayg(&["nav", "next", "--all-outputs", "--wrap"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS1, "next --all-outputs --wrap → WS1");

    // --- nav prev --all-outputs --wrap from WS1 wraps to WS2 ---
    fixture.swayg(&["nav", "prev", "--all-outputs", "--wrap"]).success();
    assert_eq!(get_focused_workspace().unwrap(), WS2, "prev --all-outputs --wrap → WS2");

    // --- Cleanup ---
    fixture.swayg(&["nav", "go", &orig_ws]).success();
    std::thread::sleep(std::time::Duration::from_millis(100));

    drop(_win1);
    drop(_win2);
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Auto-delete GROUP (empty after window kill)
    for output in [&fixture.orig_output, &virt_output] {
        fixture.swayg(&["group", "select", GROUP, "--output", output]).success();
        fixture.swayg(&["group", "select", &orig_group, "--output", output]).success();
    }

    unplug_output(&virt_output);

    fixture.init().success();
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)),
        0
    );
    for ws in [WS1, WS2] {
        assert_eq!(
            db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", ws)),
            0
        );
    }
}
