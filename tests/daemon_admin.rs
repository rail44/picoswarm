//! Daemon shutdown / restart admin commands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::process::Command;
use std::time::{Duration, Instant};

use common::TestDaemon;
use picoswarm::protocol::{self, ClientToDaemon, DaemonToClient, ErrorCode, RunRequest, TermSize};

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
    protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown { force: false })
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
async fn shutdown_with_active_attachment_is_refused_unless_forced() {
    let daemon = TestDaemon::start();

    // Spawn an agent.
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "stayer".to_string(),
            cmd: vec!["/bin/sleep".to_string(), "30".to_string()],
            cwd: None,
            env: Vec::new(),
            initial_size: TermSize { rows: 24, cols: 80 },
        }),
    )
    .await
    .expect("write Run");
    #[allow(clippy::let_underscore_must_use)]
    let _ = protocol::read_msg::<DaemonToClient, _>(&mut reader).await;
    drop(stream);

    // Open an attach connection that stays open.
    let mut attach_stream = daemon.connect().await;
    {
        let (mut areader, mut awriter) = attach_stream.split();
        protocol::write_msg(
            &mut awriter,
            &ClientToDaemon::Attach {
                name: "stayer".to_string(),
                initial_size: TermSize { rows: 24, cols: 80 },
            },
        )
        .await
        .expect("write Attach");
        match protocol::read_msg::<DaemonToClient, _>(&mut areader)
            .await
            .expect("read Attach ack")
        {
            DaemonToClient::Ok => {}
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    // Non-forced Shutdown should be refused.
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown { force: false })
        .await
        .expect("write Shutdown");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read shutdown response")
    {
        DaemonToClient::Error { code, message } => {
            assert_eq!(code, ErrorCode::ActiveAttachments);
            assert!(
                message.contains("stayer"),
                "expected 'stayer' in message, got: {message}"
            );
        }
        other => panic!("expected Error/ActiveAttachments, got {other:?}"),
    }
    drop(stream);

    // Forced Shutdown should succeed.
    let socket_path = daemon.socket_path.clone();
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown { force: true })
        .await
        .expect("write Shutdown force");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read shutdown response")
    {
        DaemonToClient::Ok => {}
        other => panic!("expected Ok on forced shutdown, got {other:?}"),
    }
    drop(stream);
    drop(attach_stream);
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
