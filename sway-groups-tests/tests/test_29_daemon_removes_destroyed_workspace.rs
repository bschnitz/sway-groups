use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use sway_groups_tests::common::{TestFixture, DummyWindowHandle, get_focused_workspace};

const WS_DEL: &str = "zz_test_ws_del";

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

fn swayg_output(db_path: &PathBuf, args: &[&str]) -> String {
    sway_groups_tests::common::swayg_output(db_path, args)
}

struct DaemonHandle {
    child: Child,
}

impl DaemonHandle {
    fn spawn(db_path: &PathBuf) -> Self {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_default();
        let daemon_bin = manifest_dir
            .parent()
            .unwrap_or(&manifest_dir)
            .join("target")
            .join("debug")
            .join("swayg-daemon");
        let child = Command::new(&daemon_bin)
            .arg(db_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn swayg-daemon");
        Self { child }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn test_29_daemon_removes_destroyed_workspace() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = fixture.orig_workspace.clone();
    let orig_output = fixture.orig_output.clone();

    fixture.init().success();

    let _daemon = DaemonHandle::spawn(&fixture.db_path);
    std::thread::sleep(std::time::Duration::from_millis(500));

    swayg_output(&fixture.db_path, &["workspace", "add", WS_DEL, "--groups", "0"]);

    let _ = Command::new("swaymsg")
        .args(["workspace", WS_DEL])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let _win = DummyWindowHandle::spawn(WS_DEL).expect("spawn dummy window");
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_DEL,
        "focused on test workspace"
    );

    let ws_in_db = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_DEL
    ));
    assert_eq!(ws_in_db, "1", "workspace in DB before destroy");

    drop(_win);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let _ = Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let ws_in_db_after = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspaces WHERE name = '{}'", WS_DEL
    ));
    assert_eq!(
        ws_in_db_after, "0",
        "workspace should be removed from DB after sway destroys it"
    );

    let wg_after = db_query(&fixture.db_path, &format!(
        "SELECT count(*) FROM workspace_groups wg \
         JOIN workspaces w ON w.id = wg.workspace_id \
         WHERE w.name = '{}'", WS_DEL
    ));
    assert_eq!(wg_after, "0", "workspace_group entries should be cleaned up");
}
