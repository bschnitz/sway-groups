use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use anyhow::{Context, Result};
use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::assert::OutputAssertExt;

pub const TEST_DB_PATH: &str = "/tmp/swayg-integration-test.db";
pub const TEST_PREFIX: &str = "zz_test_";
const DAEMON_STATE_FILE: &str = "/tmp/swayg-daemon-test.state";
const TEST_COUNTER_FILE: &str = "/tmp/swayg-test-counter";
const TEST_PROGRESS_FILE: &str = "/tmp/swayg-test-progress.json";
const SIGUSR1: libc::c_int = 10;
const SIGUSR2: libc::c_int = 12;

static TEST_DAEMON: Mutex<Option<Child>> = Mutex::new(None);
static PROD_DAEMON_REF_COUNT: Mutex<u32> = Mutex::new(0);

// ---------------------------------------------------------------------------
// swayg CLI helper
// ---------------------------------------------------------------------------

pub fn swayg(db_path: &PathBuf, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .arg("--db").arg(db_path)
        .args(args)
        .assert()
}

pub fn swayg_output(db_path: &PathBuf, args: &[&str]) -> String {
    let output = std::process::Command::new(
        std::env::var("CARGO_BIN_EXE_swayg").map(PathBuf::from).unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            manifest_dir
                .parent()
                .unwrap_or(&manifest_dir)
                .join("target")
                .join("debug")
                .join("swayg")
        }),
    )
    .arg("--db").arg(db_path)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .output()
    .expect("swayg command failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn swayg_live(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .args(args)
        .assert()
}

// ---------------------------------------------------------------------------
// Shared test daemon
// ---------------------------------------------------------------------------

fn daemon_binary() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_swayg-daemon")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            manifest_dir
                .parent()
                .unwrap_or(&manifest_dir)
                .join("target")
                .join("debug")
                .join("swayg-daemon")
        })
}

fn read_daemon_state() -> Option<String> {
    std::fs::read_to_string(DAEMON_STATE_FILE)
        .ok()
        .map(|s| s.trim().to_string())
}

fn poll_daemon_state(expected: &str, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if read_daemon_state().as_deref() == Some(expected) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

fn send_signal(pid: u32, sig: libc::c_int) {
    unsafe {
        libc::kill(pid as libc::pid_t, sig);
    }
}

pub fn start_test_daemon() {
    let mut guard = TEST_DAEMON.lock().unwrap();
    if guard.is_some() {
        return;
    }

    let _ = std::fs::remove_file(DAEMON_STATE_FILE);

    let child = Command::new(daemon_binary())
        .arg(TEST_DB_PATH)
        .arg(DAEMON_STATE_FILE)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn swayg-daemon for tests");

    std::thread::sleep(std::time::Duration::from_millis(300));

    if !poll_daemon_state("running", std::time::Duration::from_secs(2)) {
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        panic!("Test daemon did not start (state file not written)");
    }

    *guard = Some(child);
}

pub fn resume_test_daemon() {
    let guard = TEST_DAEMON.lock().unwrap();
    let child = guard.as_ref().expect("Test daemon not started");
    send_signal(child.id(), SIGUSR2);
    drop(guard);

    if !poll_daemon_state("running", std::time::Duration::from_secs(2)) {
        panic!("Test daemon did not resume (state file not updated to 'running')");
    }
}

pub fn pause_test_daemon() {
    let guard = TEST_DAEMON.lock().unwrap();
    let child = guard.as_ref().expect("Test daemon not started");
    send_signal(child.id(), SIGUSR1);
    drop(guard);

    if !poll_daemon_state("paused", std::time::Duration::from_secs(2)) {
        panic!("Test daemon did not pause (state file not updated to 'paused')");
    }
}

pub fn stop_test_daemon() {
    let mut guard = TEST_DAEMON.lock().unwrap();
    if let Some(ref mut child) = *guard {
        let _ = child.kill();
        let _ = child.wait();
        *guard = None;
    }
    let _ = std::fs::remove_file(DAEMON_STATE_FILE);
}

fn stop_prod_daemon() {
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "swayg-daemon.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let output = Command::new("systemctl")
            .args(["--user", "is-active", "swayg-daemon.service"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok();
        if let Some(o) = output
            && String::from_utf8_lossy(&o.stdout).trim() == "inactive" {
                break;
            }
    }
}

fn start_prod_daemon() {
    let _ = Command::new("systemctl")
        .args(["--user", "start", "swayg-daemon.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn acquire_prod_daemon_lock() {
    let mut count = PROD_DAEMON_REF_COUNT.lock().unwrap();
    *count += 1;
    if *count == 1 {
        stop_prod_daemon();
    }
}

fn release_prod_daemon_lock() {
    let mut count = PROD_DAEMON_REF_COUNT.lock().unwrap();
    if *count == 1 {
        start_prod_daemon();
    }
    *count -= 1;
}

pub fn daemon_state() -> Option<String> {
    read_daemon_state()
}

// ---------------------------------------------------------------------------
// TestFixture
// ---------------------------------------------------------------------------

pub struct TestFixture {
    pub db_path: PathBuf,
    pub orig_workspace: String,
    pub orig_output: String,
    test_name: String,
}

impl TestFixture {
    pub async fn new() -> Result<Self> {
        acquire_prod_daemon_lock();

        let db_path = PathBuf::from(TEST_DB_PATH);
        if db_path.exists() {
            std::fs::remove_file(&db_path).context("Failed to remove stale test DB")?;
        }

        // Clean up stale HEADLESS outputs from previous failed test runs
        let outputs = Command::new("swaymsg")
            .args(["-t", "get_outputs"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok();
        if let Some(outputs) = outputs
            && let Ok(all) = serde_json::from_slice::<serde_json::Value>(&outputs.stdout)
                && let Some(arr) = all.as_array() {
                    for o in arr.iter().filter_map(|o| o.get("name").and_then(|n| n.as_str())) {
                        if o.starts_with("HEADLESS") {
                            let _ = Command::new("swaymsg")
                                .args(["output", o, "unplug"])
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .status();
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }

        let orig_output = get_primary_output()?;
        let orig_workspace = get_focused_workspace()?;

        // Derive test name from the current binary name
        let test_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            // cargo test binary names have a hash suffix: test_01_group_select-abc123
            .map(|n| n.split('-').next().unwrap_or(&n).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        waybar_test_started(&test_name);

        Ok(Self {
            db_path,
            orig_workspace,
            orig_output,
            test_name,
        })
    }

    pub fn init(&self) -> assert_cmd::assert::Assert {
        swayg(&self.db_path, &["init"])
    }

    pub fn swayg(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        swayg(&self.db_path, args)
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        stop_test_daemon();

        let _ = Command::new("swaymsg")
            .args(["workspace", &self.orig_workspace])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // Re-sync the live DB to waybar so the bar reflects the real state
        // after tests that may have changed outputs or workspace focus.
        let _ = swayg_live(&["sync"]);

        waybar_test_finished(&self.test_name);

        release_prod_daemon_lock();
    }
}

// ---------------------------------------------------------------------------
// DummyWindowHandle
// ---------------------------------------------------------------------------

pub struct DummyWindowHandle {
    child: Child,
    pub app_id: String,
}

impl DummyWindowHandle {
    pub fn spawn(app_id: &str) -> Result<Self> {
        let binary = dummy_window_binary();
        let child = Command::new(&binary)
            .arg(app_id)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn '{}'", binary.display()))?;

        let handle = Self {
            child,
            app_id: app_id.to_string(),
        };

        let id = app_id.to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if window_exists_in_tree(&id) {
                return Ok(handle);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::bail!("Dummy window '{}' never appeared in Sway tree", app_id)
    }

    pub fn exists_in_tree(&self) -> bool {
        window_exists_in_tree(&self.app_id)
    }
}

impl Drop for DummyWindowHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn dummy_window_binary() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sway-dummy-window") {
        return PathBuf::from(path);
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            manifest_dir
                .parent()
                .unwrap_or(&manifest_dir)
                .join("target")
        });

    let candidate = target_dir.join("debug").join("sway-dummy-window");
    if candidate.exists() {
        return candidate;
    }

    if let Ok(mut exe) = std::env::current_exe() {
        exe.pop();
        let candidate = exe.join("sway-dummy-window");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("sway-dummy-window")
}

// ---------------------------------------------------------------------------
// SQLite helpers
// ---------------------------------------------------------------------------

/// Execute a raw SQL query and return the trimmed stdout as a String.
pub fn db_query(db_path: &PathBuf, sql: &str) -> String {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sqlite3 failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Execute a `SELECT count(*)` query and parse the result as i64.
pub fn db_count(db_path: &PathBuf, sql: &str) -> i64 {
    db_query(db_path, sql).parse().unwrap_or(0)
}

/// Execute a SQL statement that returns no output (INSERT / UPDATE / DELETE).
pub fn db_exec(db_path: &PathBuf, sql: &str) {
    let _ = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Count rows in `workspace_groups` that match a workspace name and group name.
pub fn ws_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
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

// ---------------------------------------------------------------------------
// Sway state query helpers
// ---------------------------------------------------------------------------

/// Check whether a workspace with the given name exists in Sway.
pub fn workspace_exists_in_sway(name: &str) -> bool {
    workspace_count_in_sway(name) > 0
}

/// Count how many workspaces with the given name exist in Sway.
pub fn workspace_count_in_sway(name: &str) -> i64 {
    let Some(workspaces) = swaymsg_json(&["-t", "get_workspaces"]) else {
        return 0;
    };
    workspaces
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|w| w.get("name").and_then(|n| n.as_str()) == Some(name))
                .count() as i64
        })
        .unwrap_or(0)
}

/// Count how many windows with the given app_id exist anywhere in the Sway tree.
pub fn window_count_in_tree(app_id: &str) -> i64 {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else { return 0 };
    let Ok(tree) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return 0;
    };
    count_app_id_in_tree(&tree, app_id)
}

fn count_app_id_in_tree(node: &serde_json::Value, app_id: &str) -> i64 {
    let mut count = 0i64;
    if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
        count += 1;
    }
    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
            for child in children {
                count += count_app_id_in_tree(child, app_id);
            }
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Production swayg helpers
// ---------------------------------------------------------------------------

/// Read the active group for an output from the **production** database.
/// Does NOT pass `--db`, so it reads the live user database.
pub fn orig_active_group(output_name: &str) -> String {
    Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .args(["group", "active", output_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// String assertion helpers
// ---------------------------------------------------------------------------

/// Return true if `haystack` contains any line that includes `needle`.
pub fn output_contains(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|l| l.contains(needle))
}

/// Return true if `haystack` contains any line that starts with `needle`.
pub fn line_starts_with(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|l| l.trim_start().starts_with(needle))
}

// ---------------------------------------------------------------------------
// swayg stderr capture
// ---------------------------------------------------------------------------

/// Run `swayg --db <path> <args>` and return stderr as a String.
pub fn swayg_stderr(db_path: &PathBuf, args: &[&str]) -> String {
    Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .arg("--db")
        .arg(db_path)
        .args(args)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Sway state helpers
// ---------------------------------------------------------------------------

fn swaymsg_json(args: &[&str]) -> Option<serde_json::Value> {
    let output = Command::new("swaymsg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    serde_json::from_slice(&output.stdout).ok()
}

pub fn get_primary_output() -> Result<String> {
    let workspaces = swaymsg_json(&["-t", "get_workspaces"])
        .context("Failed to get workspaces from sway")?;
    let arr = workspaces.as_array().context("workspaces not an array")?;
    let focused = arr
        .iter()
        .find(|w| w.get("focused").and_then(|f| f.as_bool()) == Some(true))
        .context("No focused workspace found")?;
    Ok(focused
        .get("output")
        .and_then(|o| o.as_str())
        .unwrap_or_default()
        .to_string())
}

pub fn get_focused_output() -> Result<String> {
    let workspaces = swaymsg_json(&["-t", "get_workspaces"])
        .context("Failed to get workspaces from sway")?;
    let arr = workspaces.as_array().context("workspaces not an array")?;
    let focused = arr
        .iter()
        .find(|w| w.get("focused").and_then(|f| f.as_bool()) == Some(true))
        .context("No focused workspace found")?;
    Ok(focused
        .get("output")
        .and_then(|o| o.as_str())
        .unwrap_or_default()
        .to_string())
}

pub fn get_focused_workspace() -> Result<String> {
    let workspaces = swaymsg_json(&["-t", "get_workspaces"])
        .context("Failed to get workspaces from sway")?;
    let arr = workspaces.as_array().context("workspaces not an array")?;
    let focused = arr
        .iter()
        .find(|w| w.get("focused").and_then(|f| f.as_bool()) == Some(true))
        .context("No focused workspace found")?;
    Ok(focused
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or_default()
        .to_string())
}

fn window_exists_in_tree(app_id: &str) -> bool {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    let Ok(tree) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return false;
    };
    find_app_id(&tree, app_id)
}

fn find_app_id(node: &serde_json::Value, app_id: &str) -> bool {
    if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
        return true;
    }
    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v| v.as_array())
            && children.iter().any(|c| find_app_id(c, app_id)) {
                return true;
            }
    }
    false
}

pub fn workspace_of_window(app_id: &str) -> Option<String> {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    find_workspace_of_app_id(&tree, app_id)
}

fn find_workspace_of_app_id(node: &serde_json::Value, app_id: &str) -> Option<String> {
    find_workspace_of_app_id_inner(node, app_id, None)
}

fn find_workspace_of_app_id_inner(
    node: &serde_json::Value,
    app_id: &str,
    current_ws: Option<&str>,
) -> Option<String> {
    let node_type = node.get("type").and_then(|v| v.as_str());
    let node_name = node.get("name").and_then(|v| v.as_str());
    let ws = if node_type == Some("workspace") {
        node_name
    } else {
        current_ws
    };

    if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
        return ws.map(|s| s.to_string());
    }

    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
            for child in children {
                if let Some(result) = find_workspace_of_app_id_inner(child, app_id, ws) {
                    return Some(result);
                }
            }
        }
    }
    None
}

pub fn create_virtual_output() -> Result<String> {
    let before: Vec<String> = swaymsg_json(&["-t", "get_outputs"])
        .context("Failed to get outputs from sway")?
        .as_array()
        .context("outputs not an array")?
        .iter()
        .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    let _ = Command::new("swaymsg")
        .args(["create_output", "HEADLESS-1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let after: Vec<String> = swaymsg_json(&["-t", "get_outputs"])
            .context("Failed to get outputs from sway")?
            .as_array()
            .context("outputs not an array")?
            .iter()
            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect();
        if let Some(new_name) = after.into_iter().find(|n| !before.contains(n)) {
            return Ok(new_name);
        }
        if std::time::Instant::now() > deadline {
            anyhow::bail!("Virtual output was not created");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn unplug_output(name: &str) {
    let _ = Command::new("swaymsg")
        .args(["output", name, "unplug"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

// ---------------------------------------------------------------------------
// Waybar test progress display
// ---------------------------------------------------------------------------

/// Count total integration test files.
fn count_test_files() -> u32 {
    let test_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("tests");
    std::fs::read_dir(test_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    name.starts_with("test_") && name.ends_with(".rs")
                })
                .count() as u32
        })
        .unwrap_or(0)
}

/// Read the current counter from the progress file, increment it, and write back.
/// Auto-resets when the previous run completed (counter >= total).
/// Returns (new_current, total).
fn increment_test_counter() -> (u32, u32) {
    let total = count_test_files();
    let prev = std::fs::read_to_string(TEST_COUNTER_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let current = if prev >= total { 1 } else { prev + 1 };
    let _ = std::fs::write(TEST_COUNTER_FILE, current.to_string());
    (current, total)
}

/// Reset the test counter (call before a test run or at the end).
pub fn reset_test_counter() {
    let _ = std::fs::remove_file(TEST_COUNTER_FILE);
}

/// Write test progress as waybar-compatible JSON to a file.
/// The waybar custom module reads this file every second.
fn write_test_progress(text: &str, class: &str, tooltip: &str) {
    let json = format!(
        r#"{{"text": "{}", "class": "{}", "tooltip": "{}"}}"#,
        text.replace('"', "\\\""),
        class,
        tooltip.replace('"', "\\\""),
    );
    let tmp = format!("{}.tmp", TEST_PROGRESS_FILE);
    if std::fs::write(&tmp, &json).is_ok() {
        let _ = std::fs::rename(&tmp, TEST_PROGRESS_FILE);
    }
}

/// Notify waybar that a test is starting. Called from TestFixture::new().
pub fn waybar_test_started(test_name: &str) {
    let (current, total) = increment_test_counter();
    let text = format!(" {} ({}/{})", test_name, current, total);
    write_test_progress(&text, "running", test_name);
}

/// Notify waybar that the last test finished. Called from TestFixture::drop().
pub fn waybar_test_finished(_test_name: &str) {
    let current = std::fs::read_to_string(TEST_COUNTER_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let total = count_test_files();
    if current >= total {
        let text = format!(" done ({}/{})", total, total);
        write_test_progress(&text, "done", "All tests completed");
    }
}
