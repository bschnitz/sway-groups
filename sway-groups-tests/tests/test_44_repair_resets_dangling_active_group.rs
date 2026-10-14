use sway_groups_tests::common::{TestFixture, db_count, db_exec, swayg_output};

const GROUP: &str = "zz_test_dangling_44";

#[tokio::test]
async fn test_44_repair_resets_dangling_active_group() {
    let fixture = TestFixture::new().await.expect("fixture setup");
    fixture.init().success();

    // An empty group left active on the output, as after a reboot: repair
    // prunes it, so the output must not keep pointing at it.
    fixture.swayg(&["group", "create", GROUP]).success();
    db_exec(
        &fixture.db_path,
        &format!(
            "UPDATE outputs SET active_group = '{GROUP}' WHERE name = '{}'",
            fixture.orig_output
        ),
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        GROUP,
        "precondition: the empty group is active"
    );

    fixture.swayg(&["sync", "--repair"]).success();

    assert_eq!(
        db_count(
            &fixture.db_path,
            &format!("SELECT count(*) FROM groups WHERE name = '{GROUP}'")
        ),
        0,
        "repair pruned the empty group"
    );
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        "0",
        "the output fell back to the default group"
    );
    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name = '0'"
        ),
        1,
        "the default group exists"
    );

    // Post-condition: no test entity left.
    assert_eq!(
        db_count(
            &fixture.db_path,
            "SELECT count(*) FROM groups WHERE name LIKE 'zz_test_%'"
        ),
        0
    );
}
