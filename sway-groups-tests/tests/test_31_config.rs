use std::path::PathBuf;
use std::process::{Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;
use sway_groups_tests::common::{get_focused_workspace, swayg_live, DummyWindowHandle, TestFixture};

const GROUP_FALLBACK: &str = "zz_test_fallback_31";
const GROUP_A: &str = "zz_test_grp_cfg_a_31";
const WS1: &str = "zz_test_ws_cfg_31";

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

fn ws_in_group_count(db_path: &PathBuf, ws: &str, group: &str) -> i64 {
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

fn workspace_exists_in_sway(ws: &str) -> bool {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let workspaces: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse workspaces");
    workspaces
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.get("name").and_then(|n| n.as_str()) == Some(ws))
}

#[tokio::test]
async fn test_31_config() {
    // --- Part 1: config dump (no DB needed, tests the default config output) ---
    let dump_stdout = Command::cargo_bin("swayg")
        .expect("swayg binary not found")
        .args(["config", "dump"])
        .output()
        .expect("swayg config dump failed");

    assert!(dump_stdout.status.success(), "config dump must succeed");

    let dump_text = String::from_utf8_lossy(&dump_stdout.stdout);
    assert!(
        dump_text.contains("[defaults]"),
        "config dump contains [defaults] section (output: {})",
        dump_text
    );
    assert!(
        dump_text.contains("default_group"),
        "config dump contains default_group key (output: {})",
        dump_text
    );
    assert!(
        dump_text.contains("[bar.workspaces]"),
        "config dump contains [bar.workspaces] section (output: {})",
        dump_text
    );

    // --- Part 2: --config flag loads custom config and changes default_group behavior ---
    let fixture = TestFixture::new().await.expect("fixture setup");
    let orig_ws = get_focused_workspace().expect("get focused workspace");
    let orig_group = {
        let out = Command::new("swayg")
            .args(["group", "active", &fixture.orig_output])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .expect("swayg group active failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // Precondition: no test data in real DB
    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");
    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!(
                    "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                    GROUP_FALLBACK, GROUP_A
                ),
            ),
            0,
            "precondition: test groups must not exist in production DB"
        );
    }
    assert!(!workspace_exists_in_sway(WS1), "precondition: {} must not exist in sway", WS1);

    // Write custom config TOML to a temp file
    let config_path = std::env::temp_dir().join("swayg-test-31-config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[defaults]\ndefault_group = \"{}\"\ndefault_workspace = \"0\"\n",
            GROUP_FALLBACK
        ),
    )
    .expect("write temp config");

    // Init
    fixture.init().success();

    // Create the custom fallback group so delete can move workspaces into it
    fixture
        .swayg(&["group", "create", GROUP_FALLBACK])
        .success();

    // Create GROUP_A and put WS1 into it
    fixture
        .swayg(&["group", "select", GROUP_A, "--output", &fixture.orig_output, "--create"])
        .success();

    let _win1 = DummyWindowHandle::spawn(WS1).expect("spawn dummy window WS1");
    std::thread::sleep(std::time::Duration::from_millis(500));

    fixture
        .swayg(&["container", "move", WS1, "--switch-to-workspace"])
        .success();

    assert!(workspace_exists_in_sway(WS1), "{} must exist in sway", WS1);
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_A),
        1,
        "{} is in '{}'",
        WS1, GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_FALLBACK),
        0,
        "{} is NOT yet in '{}' (fallback)",
        WS1, GROUP_FALLBACK
    );

    // Switch to neutral group before delete
    fixture
        .swayg(&["group", "select", "0", "--output", &fixture.orig_output, "--create"])
        .success();

    // Delete GROUP_A with --force and custom --config.
    // The custom config sets default_group = GROUP_FALLBACK, so WS1 should move
    // to GROUP_FALLBACK instead of the compiled-in default "0".
    fixture
        .swayg(&[
            "--config",
            config_path.to_str().expect("config path is valid UTF-8"),
            "group",
            "delete",
            GROUP_A,
            "--force",
        ])
        .success();

    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP_A)),
        0,
        "{} was deleted",
        GROUP_A
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, GROUP_FALLBACK),
        1,
        "{} moved to custom fallback group '{}' (not default '0')",
        WS1, GROUP_FALLBACK
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS1, "0"),
        0,
        "{} was NOT moved to hard-coded '0' (config overrides default)",
        WS1
    );

    // --- Cleanup: kill dummy window ---
    drop(_win1);
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = std::fs::remove_file(&config_path);

    // --- Post-condition: no test data remains ---
    fixture.init().success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM groups WHERE name IN ('{}', '{}')",
                GROUP_FALLBACK, GROUP_A
            ),
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(&fixture.db_path, &format!("SELECT count(*) FROM workspaces WHERE name = '{}'", WS1)),
        0,
        "no test workspace remains"
    );

    // --- Restore original state ---
    swayg_live(&["group", "select", &orig_group, "--output", &fixture.orig_output])
        .success();
    let _ = std::process::Command::new("swaymsg")
        .args(["workspace", &orig_ws])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}
