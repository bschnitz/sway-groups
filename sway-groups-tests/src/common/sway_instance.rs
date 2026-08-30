//! A private, headless sway compositor for one test process.
//!
//! The integration tests used to drive whatever sway session happened to be
//! running — the developer's desktop. That made every test responsible for
//! putting the desktop back the way it found it, and a test that got the
//! restore wrong corrupted the next one instead of failing itself. It also made
//! the starting state whatever the developer's session happened to look like.
//!
//! Each test process now starts its own sway on the headless wlroots backend,
//! points `SWAYSOCK` and `WAYLAND_DISPLAY` at it for the whole process, and
//! kills it again afterwards. Tests start from a known state, cannot disturb
//! anything outside their own compositor, and `create_output` finally makes
//! multi-output behaviour testable.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

/// How long to wait for the compositor to come up.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// A running headless sway, shut down when this value is dropped.
pub struct SwayInstance {
    child: Child,
    dir: PathBuf,
    pub socket: PathBuf,
    pub wayland_display: String,
}

impl SwayInstance {
    /// Start a headless sway and point this process' environment at it.
    pub fn start() -> Result<Self> {
        Self::start_with_config("")
    }

    /// Start with extra config lines appended, e.g. a `for_window` rule a test
    /// needs sway to act on.
    pub fn start_with_config(extra_config: &str) -> Result<Self> {
        let dir = instance_dir();
        std::fs::create_dir_all(&dir).context("create test instance directory")?;

        let socket = dir.join("sway.sock");
        let display_file = dir.join("wayland-display");
        let config = dir.join("sway.conf");
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_file(&display_file);

        std::fs::write(&config, config_contents(&display_file, extra_config))
            .context("write sway config")?;

        let child = Command::new("sway")
            .arg("-c")
            .arg(&config)
            .env("SWAYSOCK", &socket)
            .env("WLR_BACKENDS", "headless")
            .env("WLR_LIBINPUT_NO_DEVICES", "1")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn headless sway (is sway installed?)")?;

        let mut instance = Self {
            child,
            dir,
            socket,
            wayland_display: String::new(),
        };

        instance.wait_until_ready(&display_file)?;

        // Everything below talks to sway through the ambient environment:
        // `swaymsg`, the `swayg` binary under test, the test daemon and the
        // dummy windows all inherit it. One fixture per test process, so this
        // is set once and never contended.
        unsafe {
            std::env::set_var("SWAYSOCK", &instance.socket);
            std::env::set_var("WAYLAND_DISPLAY", &instance.wayland_display);
        }

        Ok(instance)
    }

    /// Add another headless output, e.g. `HEADLESS-2`.
    pub fn create_output(&self) -> Result<String> {
        let before = output_names(&self.socket)?;
        run_swaymsg(&self.socket, &["create_output"])?;

        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let now = output_names(&self.socket)?;
            if let Some(new) = now.iter().find(|n| !before.contains(n)) {
                return Ok(new.clone());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        bail!("sway did not report a new output after create_output")
    }

    /// Remove an output previously added with [`Self::create_output`].
    pub fn remove_output(&self, name: &str) -> Result<()> {
        run_swaymsg(&self.socket, &["output", name, "unplug"])
    }

    fn wait_until_ready(&mut self, display_file: &Path) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                bail!("headless sway exited during startup with {}", status);
            }
            if self.socket.exists()
                && let Ok(display) = std::fs::read_to_string(display_file)
                && !display.trim().is_empty()
                && output_names(&self.socket).map(|o| !o.is_empty()).unwrap_or(false)
            {
                self.wayland_display = display.trim().to_string();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        bail!("headless sway did not become ready within {:?}", STARTUP_TIMEOUT)
    }
}

impl Drop for SwayInstance {
    fn drop(&mut self) {
        let _ = run_swaymsg(&self.socket, &["exit"]);

        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A directory of its own per test process, so parallel test binaries do not
/// share a socket, a config or a database.
pub fn instance_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    base.join(format!("swayg-test-{}", std::process::id()))
}

/// `exec` runs under `sh`, which is the only way to learn the wayland socket
/// name sway picked: sway exports it to its children but does not report it.
fn config_contents(display_file: &Path, extra_config: &str) -> String {
    format!(
        "default_border none\n\
         workspace_layout tabbed\n\
         focus_follows_mouse no\n\
         exec printf '%s' \"$WAYLAND_DISPLAY\" > {}\n\
         {}\n",
        display_file.display(),
        extra_config
    )
}

fn run_swaymsg(socket: &Path, args: &[&str]) -> Result<()> {
    Command::new("swaymsg")
        .env("SWAYSOCK", socket)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("run swaymsg")?;
    Ok(())
}

fn output_names(socket: &Path) -> Result<Vec<String>> {
    let output = Command::new("swaymsg")
        .env("SWAYSOCK", socket)
        .args(["-t", "get_outputs", "-r"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("query outputs")?;

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
    Ok(parsed
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}
