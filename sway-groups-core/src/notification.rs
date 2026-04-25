//! Notification records — ephemeral file-based storage shared between daemon and CLI.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

const MAX_RECORDS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationRecord {
    pub workspace_name: String,
    pub app_name: String,
    pub summary: String,
    pub sender_pid: u32,
    pub timestamp: NaiveDateTime,
}

/// Path to the notifications file: `$XDG_RUNTIME_DIR/swayg-notifications.json`.
pub fn notifications_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(runtime_dir).join("swayg-notifications.json")
}

/// Read all stored notification records.  Returns empty vec on any error.
pub fn read_notifications() -> Vec<NotificationRecord> {
    read_notifications_from(&notifications_path())
}

/// Append a notification record, keeping at most [`MAX_RECORDS`] entries.
/// Uses atomic write (temp file + rename).
pub fn append_notification(record: NotificationRecord) {
    append_notification_to(&notifications_path(), record);
}

/// Remove and return the last notification record.
pub fn pop_last() -> Option<NotificationRecord> {
    pop_last_from(&notifications_path())
}

// --- Internal path-parameterised functions (also used by tests) ---

fn read_notifications_from(path: &Path) -> Vec<NotificationRecord> {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn append_notification_to(path: &Path, record: NotificationRecord) {
    let mut records = read_notifications_from(path);
    records.push(record);
    if records.len() > MAX_RECORDS {
        records.drain(..records.len() - MAX_RECORDS);
    }
    write_notifications(path, &records);
}

fn pop_last_from(path: &Path) -> Option<NotificationRecord> {
    let mut records = read_notifications_from(path);
    let record = records.pop();
    if record.is_some() {
        write_notifications(path, &records);
    }
    record
}

fn write_notifications(path: &Path, records: &[NotificationRecord]) {
    let tmp = path.with_extension("json.tmp");
    if let Ok(data) = serde_json::to_string_pretty(records) {
        if std::fs::write(&tmp, data.as_bytes()).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

    fn test_record(ws: &str, app: &str, summary: &str) -> NotificationRecord {
        NotificationRecord {
            workspace_name: ws.to_string(),
            app_name: app.to_string(),
            summary: summary.to_string(),
            sender_pid: 1234,
            timestamp: NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
        }
    }

    fn temp_path() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join("swayg-test-notif");
        fs::create_dir_all(&dir).unwrap();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        dir.join(format!("test-{}-{}.json", std::process::id(), id))
    }

    #[test]
    fn read_missing_file_returns_empty() {
        let path = std::env::temp_dir().join("swayg-nonexistent-12345.json");
        assert!(read_notifications_from(&path).is_empty());
    }

    #[test]
    fn append_and_read() {
        let path = temp_path();
        let _ = fs::remove_file(&path);

        append_notification_to(&path, test_record("ws1", "app1", "hello"));
        append_notification_to(&path, test_record("ws2", "app2", "world"));

        let records = read_notifications_from(&path);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].workspace_name, "ws1");
        assert_eq!(records[1].workspace_name, "ws2");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn max_records_enforced() {
        let path = temp_path();
        let _ = fs::remove_file(&path);

        for i in 0..25 {
            append_notification_to(&path, test_record(&format!("ws{i}"), "app", "s"));
        }

        let records = read_notifications_from(&path);
        assert_eq!(records.len(), MAX_RECORDS);
        // oldest 5 should be dropped
        assert_eq!(records[0].workspace_name, "ws5");
        assert_eq!(records[MAX_RECORDS - 1].workspace_name, "ws24");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn pop_last_returns_and_removes() {
        let path = temp_path();
        let _ = fs::remove_file(&path);

        append_notification_to(&path, test_record("ws1", "app1", "a"));
        append_notification_to(&path, test_record("ws2", "app2", "b"));

        let popped = pop_last_from(&path).unwrap();
        assert_eq!(popped.workspace_name, "ws2");

        let remaining = read_notifications_from(&path);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].workspace_name, "ws1");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn pop_last_empty_returns_none() {
        let path = temp_path();
        let _ = fs::remove_file(&path);

        assert!(pop_last_from(&path).is_none());
    }

    #[test]
    fn serialization_roundtrip() {
        let path = temp_path();
        let _ = fs::remove_file(&path);

        let record = test_record("35_conf", "kitty", "Claude Code");
        append_notification_to(&path, record.clone());

        let records = read_notifications_from(&path);
        assert_eq!(records[0], record);

        let _ = fs::remove_file(&path);
    }
}
