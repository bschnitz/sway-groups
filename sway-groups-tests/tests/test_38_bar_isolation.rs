//! The binary under test must not reach the developer's waybar.
//!
//! Bar widgets are pushed to a unix socket found by convention under
//! XDG_RUNTIME_DIR. A test run used to redraw the developer's bar with its own
//! throwaway workspaces, which outlived the run because nothing pushed the real
//! state back. The fixture now gives each test process a runtime directory of
//! its own; this test fails if that ever stops being true.

use sway_groups_tests::common::sway_instance::instance_dir;
use sway_groups_tests::common::TestFixture;

const WS: &str = "zz_test_bar_isolation";

#[tokio::test]
async fn test_38_bar_isolation() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is set");
    assert_eq!(
        std::path::PathBuf::from(&runtime_dir),
        instance_dir(),
        "the test process runs in its own runtime directory"
    );

    // Whatever the config names the bar instances, the sockets are looked up
    // here - and nothing is listening in a directory this process just created.
    for entry in std::fs::read_dir(&runtime_dir).expect("read runtime dir") {
        let name = entry.expect("dir entry").file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.starts_with("waybar-dynamic-"),
            "no waybar socket in the test runtime dir, found '{}'",
            name
        );
    }

    // A command that syncs the bars has to succeed all the same: a missing
    // socket is a skipped send, not an error.
    fixture.swayg(&["workspace", "add", WS]).success();
    fixture.swayg(&["sync", "--all", "--init-bars", "--init-bars-retries", "1"]).success();
}
