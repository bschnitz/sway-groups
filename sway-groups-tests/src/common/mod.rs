pub mod binaries;
pub mod sway_instance;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use anyhow::{Context, Result};
use assert_cmd::assert::OutputAssertExt;

use binaries::binaries;
use sway_instance::{SwayInstance, instance_dir};

pub const TEST_PREFIX: &str = "zz_test_";
/// The window that keeps the fixture's starting workspace from evaporating.
pub const ANCHOR_APP_ID: &str = "zz_fixture_anchor";
const TEST_COUNTER_FILE: &str = "/tmp/swayg-test-counter";
const TEST_PROGRESS_FILE: &str = "/tmp/swayg-test-progress.json";
const SIGUSR1: libc::c_int = 10;
const SIGUSR2: libc::c_int = 12;

static TEST_DAEMON: Mutex<Option<Child>> = Mutex::new(None);

/// The test database, next to the compositor it belongs to.
pub fn test_db_path() -> PathBuf {
    instance_dir().join("swayg-test.db")
}

/// Where the test daemon reports its state.
fn daemon_state_file() -> PathBuf {
    instance_dir().join("daemon.state")
}

// ---------------------------------------------------------------------------
// swayg CLI helper
// ---------------------------------------------------------------------------

pub fn swayg(db_path: &PathBuf, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::new(&binaries().swayg)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .assert()
}

pub fn swayg_output(db_path: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(&binaries().swayg)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swayg command failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

// ---------------------------------------------------------------------------
// Shared test daemon
// ---------------------------------------------------------------------------

fn read_daemon_state() -> Option<String> {
    std::fs::read_to_string(daemon_state_file())
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
    start_test_daemon_inner(None);
}

/// Start the test daemon with a custom config file.
pub fn start_test_daemon_with_config(config_path: &std::path::Path) {
    start_test_daemon_inner(Some(config_path));
}

fn start_test_daemon_inner(config_path: Option<&std::path::Path>) {
    let mut guard = TEST_DAEMON.lock().unwrap();
    if guard.is_some() {
        return;
    }

    let _ = std::fs::remove_file(daemon_state_file());

    let mut cmd = Command::new(&binaries().daemon);
    cmd.arg(test_db_path())
        .arg(daemon_state_file())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(path) = config_path {
        cmd.env("SWAYG_CONFIG", path);
    }

    let child = cmd.spawn().expect("Failed to spawn swayg-daemon for tests");

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
    let _ = std::fs::remove_file(daemon_state_file());
}

pub fn daemon_state() -> Option<String> {
    read_daemon_state()
}

// ---------------------------------------------------------------------------
// TestFixture
// ---------------------------------------------------------------------------

/// One test, one compositor, one database.
///
/// Starting a private headless sway is what makes a test independent: it begins
/// from a compositor that has nothing on it but sway's own workspace `1`, and
/// whatever it does to that compositor dies with the fixture. Nothing needs to
/// be put back, and nothing outside the test can be broken by getting the
/// putting-back wrong.
pub struct TestFixture {
    pub db_path: PathBuf,
    pub orig_workspace: String,
    pub orig_output: String,
    /// Kept alive for the duration of the test; dropping it stops sway.
    pub sway: SwayInstance,
    /// Keeps the starting workspace alive; see [`TestFixture::with_sway_config`].
    _anchor: DummyWindowHandle,
    test_name: String,
}

impl TestFixture {
    pub async fn new() -> Result<Self> {
        Self::with_sway_config("").await
    }

    /// A fixture whose compositor is configured with extra sway config lines,
    /// for tests that need sway itself to react to a rule.
    pub async fn with_sway_config(extra_config: &str) -> Result<Self> {
        let sway = SwayInstance::start_with_config(extra_config)
            .context("start headless sway for this test")?;

        let db_path = test_db_path();
        if db_path.exists() {
            std::fs::remove_file(&db_path).context("Failed to remove stale test DB")?;
        }

        // sway destroys a workspace the moment it is empty and unfocused, so a
        // fresh compositor's workspace `1` would evaporate as soon as a test
        // navigated away from it — and "go back to where you started" would
        // have nowhere to go. One window pins it, which is also what a real
        // session looks like: you start a test on a workspace with something
        // on it.
        let anchor = DummyWindowHandle::spawn(ANCHOR_APP_ID).context("spawn fixture anchor")?;

        let orig_output = get_primary_output()?;
        let orig_workspace = get_focused_workspace()?;

        // Seed the state a test expects to find, which is the state a session
        // someone has actually used is in: the default group `0` exists, holds
        // the workspace sway started on, and is the active group on the output.
        // `repair` before `select` is what keeps the focus where it is - the
        // group already owns the focused workspace, so selecting it is a no-op
        // for the user's view instead of a jump to an empty default workspace.
        swayg(&db_path, &["init"]).success();
        swayg(&db_path, &["repair"]).success();
        swayg(
            &db_path,
            &["group", "select", "0", "--output", &orig_output, "--create"],
        )
        .success();

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
            sway,
            _anchor: anchor,
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
        waybar_test_finished(&self.test_name);
        // `self.sway` shuts the compositor down from here.
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
    binaries().dummy_window.clone()
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
// Fixture state helpers
// ---------------------------------------------------------------------------

/// The active group for an output, as the fixture database sees it.
///
/// Named for what tests use it for: the group that was active before the test
/// started messing with them.
pub fn orig_active_group(output_name: &str) -> String {
    swayg_output(&test_db_path(), &["group", "active", output_name])
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
    Command::new(&binaries().swayg)
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
    let workspaces =
        swaymsg_json(&["-t", "get_workspaces"]).context("Failed to get workspaces from sway")?;
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
    let workspaces =
        swaymsg_json(&["-t", "get_workspaces"]).context("Failed to get workspaces from sway")?;
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
    let workspaces =
        swaymsg_json(&["-t", "get_workspaces"]).context("Failed to get workspaces from sway")?;
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
            && children.iter().any(|c| find_app_id(c, app_id))
        {
            return true;
        }
    }
    false
}

/// The sway container id of a window, for commands that address one directly.
pub fn con_id_of_window(app_id: &str) -> Option<i64> {
    let output = Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    find_con_id(&tree, app_id)
}

fn find_con_id(node: &serde_json::Value, app_id: &str) -> Option<i64> {
    if node.get("app_id").and_then(|v| v.as_str()) == Some(app_id) {
        return node.get("id").and_then(|v| v.as_i64());
    }
    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
            for child in children {
                if let Some(id) = find_con_id(child, app_id) {
                    return Some(id);
                }
            }
        }
    }
    None
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

/// How long the counter file may go untouched before the next test is treated
/// as the start of a new run. One test takes seconds, so a minute is generous.
const COUNTER_STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

/// Whether the counter file is old enough to belong to a previous run.
fn counter_is_stale() -> bool {
    std::fs::metadata(TEST_COUNTER_FILE)
        .and_then(|m| m.modified())
        .and_then(|t| t.elapsed().map_err(std::io::Error::other))
        .map(|age| age > COUNTER_STALE_AFTER)
        .unwrap_or(true)
}

/// Read the current counter from the progress file, increment it, and write back.
/// Restarts at 1 when the previous run completed or was abandoned - otherwise a
/// run that was cut short would leave the next one counting from its number.
/// Returns (new_current, total).
fn increment_test_counter() -> (u32, u32) {
    let total = count_test_files();
    let prev = if counter_is_stale() {
        0
    } else {
        std::fs::read_to_string(TEST_COUNTER_FILE)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0)
    };
    let current = if prev >= total { 1 } else { prev + 1 };
    let _ = std::fs::write(TEST_COUNTER_FILE, current.to_string());
    (current, total)
}

/// Reset the test counter (call before a test run or at the end).
pub fn reset_test_counter() {
    let _ = std::fs::remove_file(TEST_COUNTER_FILE);
}

/// Write test progress as waybar-compatible JSON to a file.
///
/// The waybar module ignores the file once it stops being touched, so the badge
/// disappears on its own when a run ends - however it ends. Nothing here has to
/// know that the suite is over, which is good, because nothing can: cargo runs
/// one process per test binary and none of them is the last by construction.
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
