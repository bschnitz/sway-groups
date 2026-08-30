//! The binaries under test must not read the developer's configuration.
//!
//! Without `SWAYG_CONFIG` both `swayg` and the daemon fall back to
//! `~/.config/swayg/config.toml`, so bar instance names, assignment rules and
//! defaults would be whatever the machine running the suite happens to have
//! configured. The fixture points the variable into the instance directory
//! instead; this test fails if that ever stops being true.

use sway_groups_tests::common::sway_instance::instance_dir;
use sway_groups_tests::common::TestFixture;

const WS: &str = "zz_test_config_isolation";

#[tokio::test]
async fn test_39_config_isolation() {
    let fixture = TestFixture::new().await.expect("fixture setup");

    let config = std::path::PathBuf::from(
        std::env::var("SWAYG_CONFIG").expect("SWAYG_CONFIG is set for the test process"),
    );
    assert!(
        config.starts_with(instance_dir()),
        "the config path stays inside the instance directory, was '{}'",
        config.display()
    );
    assert!(
        !config.exists(),
        "no config file is written, so the binaries use their built-in defaults"
    );

    // And the binaries still work with that: a missing file is not an error.
    fixture.swayg(&["workspace", "add", WS]).success();
    fixture.swayg(&["sync", "--all"]).success();
}
