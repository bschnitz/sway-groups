//! Resolve D-Bus notification senders to sway workspaces.
//!
//! Chain: D-Bus sender → PID → parent PID walk → sway tree → workspace name.

use std::collections::HashMap;
use std::process::Command;

use crate::sway::SwayIpcClient;

/// Resolve a D-Bus sender (e.g. `:1.129`) to its owning process ID via
/// `busctl --user call org.freedesktop.DBus`.
pub fn resolve_dbus_sender_to_pid(sender: &str) -> Option<u32> {
    let output = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionUnixProcessID",
            "s",
            sender,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // Output looks like: "u 76272\n"
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .last()
        .and_then(|s| s.parse::<u32>().ok())
}

/// Resolve a PID to the sway workspace it (or an ancestor process) belongs to.
///
/// Walks the parent PID chain from `/proc/<pid>/status` until a PID is found
/// in the sway tree.
pub fn resolve_pid_to_workspace(ipc: &SwayIpcClient, pid: u32) -> Option<String> {
    let tree_bytes = ipc.get_tree().ok()?;
    let tree: serde_json::Value = serde_json::from_slice(&tree_bytes).ok()?;

    let mut pid_to_workspace: HashMap<u32, String> = HashMap::new();
    collect_pids(&tree, &mut None, &mut pid_to_workspace);

    let mut current = pid;
    for _ in 0..64 {
        if current <= 1 {
            break;
        }
        if let Some(ws) = pid_to_workspace.get(&current) {
            return Some(ws.clone());
        }
        current = get_parent_pid(current)?;
    }

    None
}

/// Recursively walk the sway tree, collecting `pid → workspace_name` mappings.
fn collect_pids(
    node: &serde_json::Value,
    current_workspace: &mut Option<String>,
    map: &mut HashMap<u32, String>,
) {
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Track which workspace we're inside.
    let mut ws = current_workspace.clone();
    if node_type == "workspace" {
        if let Some(name) = node.get("name").and_then(|v| v.as_str()) {
            ws = Some(name.to_string());
        }
    }

    // If this node has a pid, record it.
    if let Some(pid) = node.get("pid").and_then(|v| v.as_u64()) {
        if pid > 0 {
            if let Some(ref ws_name) = ws {
                map.insert(pid as u32, ws_name.clone());
            }
        }
    }

    // Recurse into children (nodes, floating_nodes).
    for key in &["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
            for child in children {
                collect_pids(child, &mut ws, map);
            }
        }
    }
}

/// Read the parent PID from `/proc/<pid>/status`.
fn get_parent_pid(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse::<u32>().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collect_pids_from_tree() {
        let tree = json!({
            "type": "root",
            "nodes": [{
                "type": "output",
                "name": "eDP-1",
                "nodes": [{
                    "type": "workspace",
                    "name": "35_conf",
                    "nodes": [{
                        "type": "con",
                        "pid": 1001,
                        "name": "kitty",
                        "nodes": [],
                        "floating_nodes": []
                    }, {
                        "type": "con",
                        "pid": 1002,
                        "name": "firefox",
                        "nodes": [],
                        "floating_nodes": []
                    }],
                    "floating_nodes": [{
                        "type": "floating_con",
                        "pid": 1003,
                        "name": "floating",
                        "nodes": [],
                        "floating_nodes": []
                    }]
                }, {
                    "type": "workspace",
                    "name": "35_chat",
                    "nodes": [{
                        "type": "con",
                        "pid": 2001,
                        "name": "telegram",
                        "nodes": [],
                        "floating_nodes": []
                    }],
                    "floating_nodes": []
                }]
            }]
        });

        let mut map = HashMap::new();
        collect_pids(&tree, &mut None, &mut map);

        assert_eq!(map.get(&1001), Some(&"35_conf".to_string()));
        assert_eq!(map.get(&1002), Some(&"35_conf".to_string()));
        assert_eq!(map.get(&1003), Some(&"35_conf".to_string())); // floating
        assert_eq!(map.get(&2001), Some(&"35_chat".to_string()));
        assert_eq!(map.get(&9999), None);
    }

    #[test]
    fn collect_pids_skips_zero_pid() {
        let tree = json!({
            "type": "workspace",
            "name": "ws1",
            "pid": 0,
            "nodes": [],
            "floating_nodes": []
        });

        let mut map = HashMap::new();
        collect_pids(&tree, &mut None, &mut map);
        assert!(map.is_empty());
    }

    #[test]
    fn get_parent_pid_of_self() {
        // Our own process should have a valid parent PID.
        let my_pid = std::process::id();
        let ppid = get_parent_pid(my_pid);
        assert!(ppid.is_some());
        assert!(ppid.unwrap() > 0);
    }

    #[test]
    fn get_parent_pid_nonexistent() {
        assert!(get_parent_pid(999_999_999).is_none());
    }
}
