//! Collapse rules fold matching workspaces into one widget of the workspaces
//! bar, and the `bar_expanded` flag (set by the daemon while the bar modifier
//! is held) shows them again — also when a swayg command redraws the bar.
//!
//! The test stands in for waybar-dynamic: it listens on the socket swayg
//! pushes the widgets to and reads what arrives.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::time::Duration;

use sway_groups_tests::common::{DummyWindowHandle, TestFixture, db_exec};

const WS_A: &str = "zz_test_fold_46_a";
const WS_B: &str = "zz_test_fold_46_b";
const LABEL: &str = "zz_fold_46";

/// Labels of the widgets in the next `set_all` message on the socket.
fn next_labels(messages: &mpsc::Receiver<String>) -> Vec<String> {
    let line = messages
        .recv_timeout(Duration::from_secs(5))
        .expect("swayg pushes the workspaces bar");
    let message: serde_json::Value = serde_json::from_str(&line).expect("message is JSON");
    message["widgets"]
        .as_array()
        .expect("set_all carries widgets")
        .iter()
        .map(|w| w["label"].as_str().expect("widget has a label").to_string())
        .collect()
}

#[tokio::test]
async fn test_46_bar_collapse() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let config_path = std::env::temp_dir().join("swayg-test-46-config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[[bar.workspaces.collapse]]\npattern = \"^zz_test_fold_46_\"\nlabel = \"{}\"\n",
            LABEL
        ),
    )
    .expect("write temp config");
    let config = config_path.to_str().expect("config path is valid UTF-8");

    // Two workspaces the rule matches, each held open by a window.
    let _win_a = DummyWindowHandle::spawn(WS_A).expect("spawn dummy window A");
    std::thread::sleep(Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS_A]).success();
    let _win_b = DummyWindowHandle::spawn(WS_B).expect("spawn dummy window B");
    std::thread::sleep(Duration::from_millis(500));
    fixture.swayg(&["container", "move", WS_B]).success();

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is set");
    let socket = std::path::Path::new(&runtime_dir).join("waybar-dynamic-swayg_workspaces.sock");
    let listener = UnixListener::bind(&socket).expect("bind the bar socket");
    let (tx, messages) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        }
    });

    // --- Folded: one widget with the count instead of the two workspaces ---
    fixture
        .swayg(&[
            "--config",
            config,
            "sync",
            "--init-bars",
            "--init-bars-retries",
            "1",
        ])
        .success();
    let labels = next_labels(&messages);
    let folded = format!("{} (2)", LABEL);
    assert!(
        labels.contains(&folded),
        "the bar shows '{}' (labels: {:?})",
        folded,
        labels
    );
    assert!(
        !labels.iter().any(|l| l == WS_A || l == WS_B),
        "the matching workspaces are not shown on their own (labels: {:?})",
        labels
    );

    // --- Expanded: every workspace again, also on a redraw by a command ---
    db_exec(
        &fixture.db_path,
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('bar_expanded', 'true')",
    );
    fixture
        .swayg(&[
            "--config",
            config,
            "sync",
            "--init-bars",
            "--init-bars-retries",
            "1",
        ])
        .success();
    let labels = next_labels(&messages);
    assert!(
        labels.iter().any(|l| l == WS_A) && labels.iter().any(|l| l == WS_B),
        "with bar_expanded both workspaces are shown (labels: {:?})",
        labels
    );
    assert!(
        !labels.contains(&folded),
        "with bar_expanded the fold is gone (labels: {:?})",
        labels
    );

    let _ = std::fs::remove_file(&config_path);
}
