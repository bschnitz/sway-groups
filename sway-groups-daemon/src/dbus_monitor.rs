//! D-Bus notification monitor — spawns `dbus-monitor` and resolves notification
//! senders to sway workspaces.

use sway_groups_core::notification;
use sway_groups_core::sway::pid_resolver;
use sway_groups_core::sway::SwayIpcClient;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Run the D-Bus notification monitor in a loop, restarting on failure.
pub async fn run(ipc: SwayIpcClient) {
    loop {
        info!("Starting dbus-monitor for notifications");
        if let Err(e) = monitor_notifications(&ipc).await {
            error!("dbus-monitor failed: {e}");
        }
        warn!("dbus-monitor exited, restarting in 5s");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn monitor_notifications(ipc: &SwayIpcClient) -> anyhow::Result<()> {
    let mut child = Command::new("dbus-monitor")
        .args([
            "--session",
            "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout piped");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    // State machine to accumulate Notify call arguments.
    let mut in_notify = false;
    let mut sender: Option<String> = None;
    let mut string_args: Vec<String> = Vec::new();

    while let Some(line) = lines.next_line().await? {
        if line.contains("member=Notify") && line.contains("sender=") {
            // Start of a new Notify method call.
            in_notify = true;
            sender = extract_sender(&line);
            string_args.clear();
            continue;
        }

        if in_notify {
            // Collect string arguments from subsequent lines.
            // dbus-monitor prints them as:  `   string "value"`
            if let Some(val) = extract_string_arg(&line) {
                string_args.push(val);
            }

            // After the int32 timeout argument (last in Notify signature), process.
            if line.trim_start().starts_with("int32") {
                in_notify = false;
                if let Some(ref s) = sender {
                    let app_name = string_args.first().cloned().unwrap_or_default();
                    // args: app_name(0), replaces_id skipped, icon(1?), summary(2), body(3)
                    // Actually dbus-monitor only prints string args, so:
                    // string 0 = app_name, string 1 = icon, string 2 = summary, string 3 = body
                    let summary = string_args.get(2).cloned().unwrap_or_default();
                    handle_notification(ipc, s, &app_name, &summary).await;
                }
            }
        }
    }

    Ok(())
}

fn extract_sender(line: &str) -> Option<String> {
    let idx = line.find("sender=")?;
    let rest = &line[idx + 7..];
    let end = rest.find(' ').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn extract_string_arg(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("string \"")?;
    // Remove trailing quote.
    Some(rest.strip_suffix('"').unwrap_or(rest).to_string())
}

async fn handle_notification(
    ipc: &SwayIpcClient,
    sender: &str,
    app_name: &str,
    summary: &str,
) {
    let pid = match pid_resolver::resolve_dbus_sender_to_pid(sender) {
        Some(p) => p,
        None => {
            debug!("Could not resolve sender {sender} to PID");
            return;
        }
    };

    let workspace = match pid_resolver::resolve_pid_to_workspace(ipc, pid) {
        Some(ws) => ws,
        None => {
            debug!("Could not resolve PID {pid} to workspace");
            return;
        }
    };

    info!(
        "Notification: sender={sender}, pid={pid}, workspace={workspace}, app={app_name}, summary={summary}"
    );

    let record = notification::NotificationRecord {
        workspace_name: workspace,
        app_name: app_name.to_string(),
        summary: summary.to_string(),
        sender_pid: pid,
        timestamp: chrono::Utc::now().naive_utc(),
    };

    notification::append_notification(record);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_sender_from_method_call() {
        let line = "method call time=1777116552.398238 sender=:1.129 -> destination=:1.5 serial=42 path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify";
        assert_eq!(extract_sender(line), Some(":1.129".to_string()));
    }

    #[test]
    fn extract_sender_missing() {
        assert_eq!(extract_sender("no sender here"), None);
    }

    #[test]
    fn extract_sender_at_end() {
        let line = "something sender=:1.42";
        assert_eq!(extract_sender(line), Some(":1.42".to_string()));
    }

    #[test]
    fn extract_string_arg_normal() {
        assert_eq!(
            extract_string_arg("   string \"kitty\""),
            Some("kitty".to_string()),
        );
    }

    #[test]
    fn extract_string_arg_with_spaces() {
        assert_eq!(
            extract_string_arg("   string \"Claude Code\""),
            Some("Claude Code".to_string()),
        );
    }

    #[test]
    fn extract_string_arg_empty() {
        assert_eq!(
            extract_string_arg("   string \"\""),
            Some("".to_string()),
        );
    }

    #[test]
    fn extract_string_arg_not_a_string() {
        assert_eq!(extract_string_arg("   uint32 0"), None);
        assert_eq!(extract_string_arg("   int32 -1"), None);
    }
}
