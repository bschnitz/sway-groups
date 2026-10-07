//! When sway goes away (a restart, or the session ending), the daemon's event
//! socket is closed for good. The daemon must exit with an error then, so that
//! systemd restarts it against the new sway, instead of retrying the dead
//! socket forever.
//!
//! The daemon talks to a stand-in socket here, not to a sway: one that accepts
//! the subscription and then hangs up, exactly what a vanishing sway looks
//! like from the daemon's side.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sway_groups_tests::common::binaries::binaries;

/// IPC message type of sway's `SUBSCRIBE`.
const SUBSCRIBE: u32 = 2;

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("swayg-test-43-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    dir
}

/// Accept the daemon's subscription, confirm it, then close the connection.
fn serve_one_subscription_then_hang_up(listener: UnixListener) {
    let (mut stream, _) = listener.accept().expect("daemon connects");

    let mut header = [0u8; 14];
    stream.read_exact(&mut header).expect("read request header");
    assert_eq!(&header[0..6], b"i3-ipc", "daemon speaks sway IPC");
    let payload_size = u32::from_ne_bytes(header[6..10].try_into().unwrap());
    let message_type = u32::from_ne_bytes(header[10..14].try_into().unwrap());
    assert_eq!(message_type, SUBSCRIBE, "daemon subscribes first");
    let mut payload = vec![0u8; payload_size as usize];
    stream.read_exact(&mut payload).expect("read request payload");

    let reply = br#"{"success": true}"#;
    let mut frame = Vec::new();
    frame.extend_from_slice(b"i3-ipc");
    frame.extend_from_slice(&(reply.len() as u32).to_ne_bytes());
    frame.extend_from_slice(&SUBSCRIBE.to_ne_bytes());
    frame.extend_from_slice(reply);
    stream.write_all(&frame).expect("write subscribe reply");
    // Dropping the stream closes it: sway is gone.
}

#[test]
fn test_43_daemon_exits_when_sway_goes_away() {
    let dir = test_dir();
    let socket = dir.join("sway.sock");
    let listener = UnixListener::bind(&socket).expect("bind stand-in sway socket");
    let server = std::thread::spawn(move || serve_one_subscription_then_hang_up(listener));

    let mut daemon = Command::new(&binaries().daemon)
        .arg(dir.join("swayg-test.db"))
        .arg(dir.join("daemon.state"))
        .env("SWAYSOCK", &socket)
        .env("XDG_RUNTIME_DIR", &dir)
        .env("XDG_DATA_HOME", &dir)
        .env("SWAYG_CONFIG", dir.join("config.toml"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn swayg-daemon");

    server.join().expect("stand-in sway served the subscription");

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = daemon.try_wait().expect("poll daemon") {
            break Some(status);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    if status.is_none() {
        let _ = daemon.kill();
        let _ = daemon.wait();
    }
    let _ = std::fs::remove_dir_all(&dir);

    let status = status.expect("daemon still runs 5 s after sway closed its event socket");
    assert!(
        !status.success(),
        "daemon must exit with an error so systemd restarts it, got {status}"
    );
}
