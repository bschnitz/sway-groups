//! A collapse rule without `step_through` makes `nav next`/`nav prev` stop
//! once on its fold, at the fold's place in the bar, and go to the workspace
//! of it focused last (or its first one when none is remembered).

use std::process::{Command, Stdio};
use std::time::Duration;

use sway_groups_tests::common::{DummyWindowHandle, TestFixture, db_exec, get_focused_workspace};

const GROUP: &str = "zz_test_nav_47";
const WS_A: &str = "zz_tn47_a";
const WS_X: &str = "zz_tn47_fold_x";
const WS_Y: &str = "zz_tn47_fold_y";
const WS_Z: &str = "zz_tn47_z";
const LABEL: &str = "zz_tn47_fold";

fn focus(ws: &str) {
    Command::new("swaymsg")
        .args(["workspace", ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("swaymsg workspace failed");
    std::thread::sleep(Duration::from_millis(100));
}

#[tokio::test]
async fn test_47_nav_collapsed() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let config_path = std::env::temp_dir().join("swayg-test-47-config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[[bar.workspaces.collapse]]\npattern = \"^zz_tn47_fold_\"\nlabel = \"{}\"\nstep_through = false\n",
            LABEL
        ),
    )
    .expect("write temp config");
    let config = config_path.to_str().expect("config path is valid UTF-8");
    let nav = |direction: &str| {
        fixture
            .swayg(&[
                "--config",
                config,
                "nav",
                direction,
                "--output",
                &fixture.orig_output,
            ])
            .success();
    };
    let remember = |ws: &str| {
        db_exec(
            &fixture.db_path,
            &format!(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('collapse_last:{}', '{}')",
                LABEL, ws
            ),
        )
    };

    fixture.init().success();
    fixture
        .swayg(&[
            "group",
            "select",
            GROUP,
            "--output",
            &fixture.orig_output,
            "--create",
        ])
        .success();
    let mut windows = Vec::new();
    for ws in [WS_A, WS_X, WS_Y, WS_Z] {
        windows.push(DummyWindowHandle::spawn(ws).expect("spawn dummy window"));
        std::thread::sleep(Duration::from_millis(500));
        fixture
            .swayg(&["container", "move", ws, "--switch-to-workspace"])
            .success();
    }

    // --- Into the fold: the workspace of it focused last ---
    remember(WS_Y);
    focus(WS_A);
    nav("next");
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_Y,
        "nav next enters the fold on the workspace focused last"
    );

    // --- Out of the fold: past its other workspace ---
    nav("prev");
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_A,
        "nav prev leaves the fold without stopping on '{}'",
        WS_X
    );

    // --- Nothing remembered: the fold's first workspace ---
    db_exec(
        &fixture.db_path,
        &format!("DELETE FROM settings WHERE key = 'collapse_last:{}'", LABEL),
    );
    focus(WS_Z);
    nav("prev");
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_X,
        "without a remembered workspace nav prev enters the fold on its first one"
    );

    // --- A remembered workspace that is gone: the first one too ---
    remember("zz_tn47_fold_gone");
    focus(WS_A);
    nav("next");
    assert_eq!(
        get_focused_workspace().unwrap(),
        WS_X,
        "a remembered workspace that no longer exists is skipped"
    );

    drop(windows);
    let _ = std::fs::remove_file(&config_path);
}
