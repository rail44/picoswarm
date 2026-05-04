//! Daemon shutdown / restart admin commands.

mod common;

use std::process::Command;
use std::time::{Duration, Instant};

use common::TestDaemon;
use picoswarm::protocol::{self, ClientToDaemon, DaemonToClient};

/// Wait until `path` no longer represents a reachable Unix socket.
fn wait_for_socket_gone(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !path.exists() {
            return;
        }
        if std::os::unix::net::UnixStream::connect(path).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "socket should be unreachable after Shutdown but is still up: {}",
        path.display()
    );
}

#[tokio::test]
async fn shutdown_via_protocol_makes_socket_disappear() {
    let daemon = TestDaemon::start();
    let socket_path = daemon.socket_path.clone();

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown)
        .await
        .expect("write Shutdown");

    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read Shutdown ack")
    {
        DaemonToClient::Ok => {}
        other => panic!("expected Ok, got {other:?}"),
    }
    drop(stream);

    wait_for_socket_gone(&socket_path);
}

#[tokio::test]
async fn pswarm_daemon_stop_cli_terminates_daemon() {
    let daemon = TestDaemon::start();
    let socket_path = daemon.socket_path.clone();
    let runtime_dir = socket_path
        .parent()
        .and_then(|p| p.parent())
        .expect("expected runtime dir parent of socket")
        .to_path_buf();

    // Run `pswarm daemon stop` against the test daemon by routing it
    // through the same XDG dirs.
    let status = Command::new(env!("CARGO_BIN_EXE_pswarm"))
        .args(["daemon", "stop"])
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .env("XDG_STATE_HOME", &runtime_dir)
        .status()
        .expect("spawn pswarm daemon stop");

    assert!(
        status.success(),
        "`pswarm daemon stop` should exit 0, got {status:?}"
    );

    wait_for_socket_gone(&socket_path);
}
