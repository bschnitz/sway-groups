//! Locating the binaries under test.
//!
//! `swayg`, `swayg-daemon` and `sway-dummy-window` live in sibling packages, so
//! cargo does not build them for this package and does not export a
//! `CARGO_BIN_EXE_*` path for them. Guessing `target/debug/swayg` instead is how
//! a test run silently ends up exercising a binary from an earlier build.
//!
//! So the harness builds them itself, once per test process, and takes the paths
//! straight out of cargo's JSON output. `cargo test` releases the build-directory
//! lock before it runs the test binaries, so building from inside a test does not
//! deadlock against the cargo that started it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The three binaries a test drives.
pub struct Binaries {
    pub swayg: PathBuf,
    pub daemon: PathBuf,
    pub dummy_window: PathBuf,
}

static BINARIES: OnceLock<Binaries> = OnceLock::new();

/// Freshly built binaries, built on first use and reused afterwards.
///
/// Panics rather than falling back to a stale binary: a test that runs against
/// the wrong build reports a result about code nobody is looking at.
pub fn binaries() -> &'static Binaries {
    BINARIES.get_or_init(build_binaries)
}

/// Whether the test binary itself was built with `--release`.
fn is_release() -> bool {
    std::env::current_exe()
        .ok()
        .map(|p| p.components().any(|c| c.as_os_str() == "release"))
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests package has a parent directory")
        .to_path_buf()
}

fn build_binaries() -> Binaries {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace_root())
        .arg("build")
        .arg("--message-format=json-render-diagnostics")
        .args([
            "-p",
            "sway-groups-cli",
            "-p",
            "sway-groups-daemon",
            "-p",
            "sway-groups-dummy-window",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    if is_release() {
        cmd.arg("--release");
    }

    let output = cmd
        .output()
        .expect("failed to run cargo to build the binaries under test");

    assert!(
        output.status.success(),
        "cargo build for the binaries under test failed"
    );

    let mut swayg = None;
    let mut daemon = None;
    let mut dummy_window = None;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|r| r.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(executable) = msg.get("executable").and_then(|e| e.as_str()) else {
            continue;
        };
        let name = msg
            .get("target")
            .and_then(|t| t.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or_default();

        match name {
            "swayg" => swayg = Some(PathBuf::from(executable)),
            "swayg-daemon" => daemon = Some(PathBuf::from(executable)),
            "sway-dummy-window" => dummy_window = Some(PathBuf::from(executable)),
            _ => {}
        }
    }

    Binaries {
        swayg: swayg.expect("cargo did not report an executable for 'swayg'"),
        daemon: daemon.expect("cargo did not report an executable for 'swayg-daemon'"),
        dummy_window: dummy_window
            .expect("cargo did not report an executable for 'sway-dummy-window'"),
    }
}
