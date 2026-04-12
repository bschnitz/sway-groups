use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};
use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::assert::OutputAssertExt;

pub const TEST_DB_PATH: &str = "/tmp/swayg-integration-test.db";
pub const TEST_PREFIX: &str = "zz_test_";

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

// ---------------------------------------------------------------------------
// TestFixture
// ---------------------------------------------------------------------------

pub struct TestFixture {
    pub db_path: PathBuf,
    pub orig_workspace: String,
    pub orig_output: String,
}

impl TestFixture {
    pub async fn new() -> Result<Self> {
        let db_path = PathBuf::from(TEST_DB_PATH);
        if db_path.exists() {
            std::fs::remove_file(&db_path).context("Failed to remove stale test DB")?;
        }

        let orig_output = get_primary_output()?;
        let orig_workspace = get_focused_workspace()?;

        Ok(Self {
            db_path,
            orig_workspace,
            orig_output,
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
        let _ = Command::new("swaymsg")
            .args(["workspace", &self.orig_workspace])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
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
        if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
            if children.iter().any(|c| find_app_id(c, app_id)) {
                return true;
            }
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
