use std::process::{Command, Stdio};

use sway_groups_tests::common::{
    create_virtual_output, db_count, get_focused_output, get_focused_workspace, orig_active_group,
    swayg_live, swayg_output, unplug_output, TestFixture,
};

const GROUP: &str = "zz_test_oo_fallback";

#[tokio::test]
async fn test_22_optional_output_fallback() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let real_db = dirs::data_dir()
        .unwrap_or_default()
        .join("swayg")
        .join("swayg.db");

    let orig_group = orig_active_group(&fixture.orig_output);
    assert!(!orig_group.is_empty(), "original group must not be empty");

    // Clean up stale outputs from previous failed runs
    let outputs = Command::new("swaymsg")
        .args(["-t", "get_outputs"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("swaymsg failed");
    let all_outputs: Vec<String> = serde_json::from_slice::<serde_json::Value>(&outputs.stdout)
        .expect("parse outputs")
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
        .filter(|n| n != &fixture.orig_output)
        .collect();
    for o in &all_outputs {
        let _ = Command::new("swaymsg")
            .args(["output", o, "unplug"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    if real_db.exists() {
        assert_eq!(
            db_count(
                &real_db,
                &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
            ),
            0,
            "precondition: {} must not exist in production DB",
            GROUP
        );
    }

    // --- Create virtual output ---
    let virtual_output = create_virtual_output().expect("create virtual output");

    // --- Init ---
    fixture.init().success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test group after init"
    );

    // --- Setup: focus virtual output ---
    let _ = Command::new("swaymsg")
        .args(["focus", "output", &virtual_output])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        get_focused_output().expect("get focused output"),
        virtual_output,
        "focused output = '{}'",
        virtual_output
    );

    // --- Setup: create group via CLI (no --output, no prior visit) ---
    fixture.swayg(&["group", "create", GROUP]).success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        1,
        "group '{}' created",
        GROUP
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE group_name = '{}'",
                GROUP
            )
        ),
        0,
        "no group_state entry for '{}' (never visited via select)",
        GROUP
    );

    // --- TEST: group select without --output (fallback to current output) ---
    fixture.swayg(&["group", "select", GROUP]).success();
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &virtual_output]),
        GROUP,
        "active group = '{}' on virtual output (fallback)",
        GROUP
    );
    assert_eq!(
        get_focused_output().expect("get focused output"),
        virtual_output,
        "stayed on virtual output (fallback, no group_state)"
    );

    // --- Verify: group_state now exists for GROUP on virtual output ---
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE group_name = '{}' AND output = '{}'",
                GROUP, virtual_output
            )
        ),
        1,
        "group_state entry created for '{}' on '{}'",
        GROUP,
        virtual_output
    );

    // --- Cleanup: switch back to original group (live DB) ---
    swayg_live(&[
        "group",
        "select",
        &orig_group,
        "--output",
        &fixture.orig_output,
    ])
    .success();
    let _ = Command::new("swaymsg")
        .args(["workspace", &fixture.orig_workspace])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        get_focused_workspace().unwrap(),
        fixture.orig_workspace,
        "focused on original workspace"
    );

    // --- Cleanup: auto-delete test group ---
    fixture.swayg(&["group", "delete", GROUP, "--force"]).success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "test group auto-deleted"
    );

    // --- Cleanup: remove virtual output ---
    unplug_output(&virtual_output);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Post-condition ---
    fixture.init().success();
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{}'", GROUP)
        ),
        0,
        "no test groups remain"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!(
                "SELECT count(*) FROM group_state WHERE group_name = '{}'",
                GROUP
            )
        ),
        0,
        "no test group_state remains"
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        fixture.orig_workspace,
        "focused on original workspace after test"
    );
}
