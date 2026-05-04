//! Headless attach tests using a PTY-driven client harness.

mod common;

use std::time::Duration;

use common::pty::PtyClient;
use common::{TestDaemon, spawn_agent};

/// Wait briefly so the attach handshake can complete and the backlog
/// (if any) can flow through to the PTY before the test starts asserting.
fn settle() {
    std::thread::sleep(Duration::from_millis(200));
}

#[tokio::test]
async fn attach_detaches_on_raw_ctrl_backslash() {
    let daemon = TestDaemon::start();
    spawn_agent(
        &daemon,
        "agent",
        vec!["/bin/sleep".to_string(), "30".to_string()],
    )
    .await;

    let mut client = PtyClient::spawn_attach(&daemon, "agent");
    settle();

    // Raw Ctrl-\ (FS, 0x1c) — the encoding sent when the agent has not
    // enabled the kitty keyboard protocol.
    client.send_bytes(b"\x1c");

    assert!(
        client.wait_for_substring(b"[detached:", Duration::from_secs(2)),
        "expected `[detached:` in output, got {:?}",
        String::from_utf8_lossy(&client.output_so_far())
    );

    let exit = client
        .wait_for_exit(Duration::from_secs(2))
        .expect("attach client should exit after detach");
    assert!(exit.success(), "attach should exit cleanly: {exit:?}");
}

#[tokio::test]
async fn attach_detaches_on_csi_u_ctrl_backslash() {
    let daemon = TestDaemon::start();
    spawn_agent(
        &daemon,
        "agent",
        vec!["/bin/sleep".to_string(), "30".to_string()],
    )
    .await;

    let mut client = PtyClient::spawn_attach(&daemon, "agent");
    settle();

    // CSI-u encoded Ctrl-\: codepoint 92 ('\\') with modifier 5 (Ctrl).
    // This is the form arriving when the agent has enabled kitty kbd
    // protocol level 1 (disambiguate escape codes).
    client.send_bytes(b"\x1b[92;5u");

    assert!(
        client.wait_for_substring(b"[detached:", Duration::from_secs(2)),
        "expected `[detached:` in output for CSI-u trigger, got {:?}",
        String::from_utf8_lossy(&client.output_so_far())
    );

    let exit = client
        .wait_for_exit(Duration::from_secs(2))
        .expect("attach client should exit after detach");
    assert!(exit.success(), "attach should exit cleanly: {exit:?}");
}

#[tokio::test]
async fn attach_forwards_user_input_to_agent() {
    let daemon = TestDaemon::start();
    // `cat` echoes whatever it reads from stdin back to stdout.
    spawn_agent(&daemon, "agent", vec!["/bin/cat".to_string()]).await;

    let mut client = PtyClient::spawn_attach(&daemon, "agent");
    settle();

    // Send "hello\n" and expect cat to echo it.
    client.send_bytes(b"hello\n");

    assert!(
        client.wait_for_substring(b"hello", Duration::from_secs(2)),
        "expected `hello` in echoed output, got {:?}",
        String::from_utf8_lossy(&client.output_so_far())
    );

    // Detach cleanly so the harness can verify exit.
    client.send_bytes(b"\x1c");
    let exit = client
        .wait_for_exit(Duration::from_secs(2))
        .expect("attach client should exit after detach");
    assert!(exit.success(), "attach should exit cleanly: {exit:?}");
}

#[tokio::test]
async fn session_ended_propagates_when_agent_exits() {
    let daemon = TestDaemon::start();
    // Sleep briefly, then exit. Long enough that the attach is active
    // before exit (so the subscription is in place to receive
    // SessionEnded), short enough to keep the test fast.
    spawn_agent(
        &daemon,
        "agent",
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 0.5".to_string(),
        ],
    )
    .await;

    let mut client = PtyClient::spawn_attach(&daemon, "agent");

    assert!(
        client.wait_for_substring(b"[session ended", Duration::from_secs(3)),
        "expected `[session ended` in output, got {:?}",
        String::from_utf8_lossy(&client.output_so_far())
    );

    let exit = client
        .wait_for_exit(Duration::from_secs(2))
        .expect("attach client should exit after session ends");
    assert!(exit.success(), "attach should exit cleanly: {exit:?}");
}

#[tokio::test]
async fn attach_replays_backlog_on_reconnect() {
    let daemon = TestDaemon::start();
    // Use `sh -c 'echo X; sleep 30'` so the agent emits output AND stays
    // alive: the session task only exposes a backlog while it is still
    // running. Once the agent exits the session task tears itself down
    // and a fresh attach gets ErrorCode::Internal "session task is gone"
    // — which is correct, but not what this test exercises.
    spawn_agent(
        &daemon,
        "agent",
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo marker-from-backlog; sleep 30".to_string(),
        ],
    )
    .await;

    // Give the daemon time to drain the echo's output into the ring buffer
    // before we attach.
    std::thread::sleep(Duration::from_millis(150));

    let mut client = PtyClient::spawn_attach(&daemon, "agent");

    assert!(
        client.wait_for_substring(b"marker-from-backlog", Duration::from_secs(2)),
        "expected `marker-from-backlog` in replayed backlog, got {:?}",
        String::from_utf8_lossy(&client.output_so_far())
    );

    // Detach cleanly so the harness can verify exit.
    client.send_bytes(b"\x1c");
    let exit = client
        .wait_for_exit(Duration::from_secs(2))
        .expect("attach client should exit after detach");
    assert!(exit.success(), "attach should exit cleanly: {exit:?}");
}
