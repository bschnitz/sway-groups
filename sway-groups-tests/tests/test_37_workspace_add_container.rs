//! `workspace add` and `container move` must never move the user's view.
//!
//! A workspace sway does not know yet cannot be conjured up by focusing it:
//! sway destroys an empty workspace as soon as it loses focus, so `workspace
//! <name>` would drag the user to an empty workspace and strand them there.
//! `workspace add` therefore records the membership in the database alone, and
//! `--container` is the way to materialise the workspace properly — by moving a
//! window that already exists into it.

use sway_groups_tests::common::{
    DummyWindowHandle, TestFixture, con_id_of_window, get_focused_workspace, swayg_stderr,
    workspace_exists_in_sway, workspace_of_window, ws_in_group_count,
};

const GROUP: &str = "zz_test_add_ct";
const WS_RECORDED: &str = "zz_tg_ct_recorded";
const WS_MATERIALISED: &str = "zz_tg_ct_material";
const WS_MOVED: &str = "zz_tg_ct_moved";
const WIN_A: &str = "zz_ct_win_a";
const WIN_B: &str = "zz_ct_win_b";

#[tokio::test]
async fn test_37_workspace_add_container() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    let home_ws = fixture.orig_workspace.clone();

    fixture.swayg(&["group", "create", GROUP]).success();

    // --- workspace add without --container: database only ---
    let stderr = swayg_stderr(
        &fixture.db_path,
        &["workspace", "add", WS_RECORDED, "--group", GROUP],
    );

    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_RECORDED, GROUP),
        1,
        "'{}' is recorded in group '{}'",
        WS_RECORDED,
        GROUP
    );
    assert!(
        !workspace_exists_in_sway(WS_RECORDED),
        "'{}' is not created in sway",
        WS_RECORDED
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        home_ws,
        "the view did not move"
    );
    assert!(
        stderr.contains("does not exist in sway yet"),
        "the note says the membership is not backed by a sway workspace, got: {}",
        stderr
    );

    // --- workspace add --container: materialised, view still put ---
    let win_a = DummyWindowHandle::spawn(WIN_A).expect("spawn WIN_A");
    let con_a = con_id_of_window(WIN_A).expect("con_id of WIN_A");

    fixture
        .swayg(&[
            "workspace",
            "add",
            WS_MATERIALISED,
            "--group",
            GROUP,
            "--container",
            &con_a.to_string(),
        ])
        .success();

    assert!(
        workspace_exists_in_sway(WS_MATERIALISED),
        "'{}' exists in sway after --container",
        WS_MATERIALISED
    );
    assert_eq!(
        workspace_of_window(WIN_A).as_deref(),
        Some(WS_MATERIALISED),
        "the container was moved onto '{}'",
        WS_MATERIALISED
    );
    assert_eq!(
        ws_in_group_count(&fixture.db_path, WS_MATERIALISED, GROUP),
        1,
        "'{}' is in group '{}'",
        WS_MATERIALISED,
        GROUP
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        home_ws,
        "the view did not follow the container"
    );

    // --- container move --con-id: moves that container, not the focused one ---
    let win_b = DummyWindowHandle::spawn(WIN_B).expect("spawn WIN_B");
    let con_b = con_id_of_window(WIN_B).expect("con_id of WIN_B");

    fixture
        .swayg(&[
            "container",
            "move",
            WS_MOVED,
            "--con-id",
            &con_b.to_string(),
        ])
        .success();

    assert_eq!(
        workspace_of_window(WIN_B).as_deref(),
        Some(WS_MOVED),
        "the named container moved to '{}'",
        WS_MOVED
    );
    assert_eq!(
        workspace_of_window(WIN_A).as_deref(),
        Some(WS_MATERIALISED),
        "the other container stayed where it was"
    );
    assert_eq!(
        get_focused_workspace().unwrap(),
        home_ws,
        "the view did not follow the container"
    );

    drop(win_a);
    drop(win_b);
}
