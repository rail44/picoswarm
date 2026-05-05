//! Run / Ls / Rm / Attach (error paths only) integration tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TestDaemon;
use picoswarm::protocol::{
    self, AgentStatus, ClientToDaemon, DaemonToClient, ErrorCode, RunRequest, TermSize,
};

fn sleep_request(name: &str) -> RunRequest {
    // `/bin/sleep 30` is harmless and stays alive long enough for the
    // assertions below to observe it as Running.
    RunRequest {
        name: name.to_string(),
        cmd: vec!["/bin/sleep".to_string(), "30".to_string()],
        cwd: None,
        env: Vec::new(),
        initial_size: TermSize { rows: 24, cols: 80 },
    }
}

async fn run_agent(daemon: &TestDaemon, name: &str) -> uuid::Uuid {
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Run(sleep_request(name)))
        .await
        .expect("write Run");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read RunResult")
    {
        DaemonToClient::RunResult { id, .. } => id,
        other => panic!("expected RunResult, got {other:?}"),
    }
}

async fn list_agents(daemon: &TestDaemon) -> Vec<picoswarm::protocol::AgentSummary> {
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Ls)
        .await
        .expect("write Ls");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read AgentList")
    {
        DaemonToClient::AgentList(v) => v,
        other => panic!("expected AgentList, got {other:?}"),
    }
}

async fn rm_agent(daemon: &TestDaemon, name: &str, force: bool) -> Result<(), (ErrorCode, String)> {
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Rm {
            name: name.to_string(),
            force,
        },
    )
    .await
    .expect("write Rm");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read Rm response")
    {
        DaemonToClient::Ok => Ok(()),
        DaemonToClient::Error { code, message } => Err((code, message)),
        other => panic!("expected Ok/Error, got {other:?}"),
    }
}

#[tokio::test]
async fn run_then_ls_lists_the_agent_as_running() {
    let daemon = TestDaemon::start();

    let id = run_agent(&daemon, "alpha").await;

    let agents = list_agents(&daemon).await;
    assert_eq!(agents.len(), 1, "expected exactly one agent");
    let agent = &agents[0];
    assert_eq!(agent.id, id);
    assert_eq!(agent.name, "alpha");
    assert_eq!(agent.status, AgentStatus::Running);
}

#[tokio::test]
async fn rm_removes_the_agent_from_ls() {
    let daemon = TestDaemon::start();

    run_agent(&daemon, "beta").await;
    rm_agent(&daemon, "beta", false)
        .await
        .expect("rm should succeed");

    let agents = list_agents(&daemon).await;
    assert!(
        agents.is_empty(),
        "expected empty list after rm, got {agents:?}"
    );
}

#[tokio::test]
async fn duplicate_name_returns_name_taken() {
    let daemon = TestDaemon::start();

    run_agent(&daemon, "gamma").await;

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Run(sleep_request("gamma")))
        .await
        .expect("write second Run");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => assert_eq!(code, ErrorCode::NameTaken),
        other => panic!("expected NameTaken error, got {other:?}"),
    }
}

#[tokio::test]
async fn rm_unknown_returns_not_found() {
    let daemon = TestDaemon::start();

    let err = rm_agent(&daemon, "nope", false)
        .await
        .expect_err("expected error");
    assert_eq!(err.0, ErrorCode::NotFound);
}

#[tokio::test]
async fn clean_removes_dead_agents_only() {
    let daemon = TestDaemon::start();

    // alive: stays running for the duration of the test
    run_agent(&daemon, "alive").await;

    // doomed: exits ~immediately
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "doomed".to_string(),
            cmd: vec!["/bin/sh".to_string(), "-c".to_string(), "true".to_string()],
            cwd: None,
            env: Vec::new(),
            initial_size: TermSize { rows: 24, cols: 80 },
        }),
    )
    .await
    .expect("write Run");
    // Drain the response; the assertions later verify the side-effect.
    #[allow(clippy::let_underscore_must_use)]
    let _ = protocol::read_msg::<DaemonToClient, _>(&mut reader).await;
    drop(stream);

    // Give the daemon a moment to observe the doomed agent's exit.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Issue Clean.
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Clean)
        .await
        .expect("write Clean");
    let removed = match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read Cleaned")
    {
        DaemonToClient::Cleaned { removed } => removed,
        other => panic!("expected Cleaned, got {other:?}"),
    };

    assert_eq!(removed, vec!["doomed".to_string()]);

    // Live agent should still be present.
    let agents = list_agents(&daemon).await;
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "alive");
}

#[tokio::test]
async fn cwd_returns_path_for_running_agent() {
    let daemon = TestDaemon::start();

    // /bin/sleep inherits the daemon's cwd (which is `/` after the
    // daemonize step's working_directory("/")). We don't care about the
    // *value* — just that we get *some* readable path back.
    run_agent(&daemon, "alpha").await;

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::GetCwd {
            name: "alpha".into(),
        },
    )
    .await
    .expect("write GetCwd");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read AgentCwd")
    {
        DaemonToClient::AgentCwd { path: Some(p) } => {
            assert!(p.is_absolute(), "cwd should be absolute, got {p:?}");
        }
        other => panic!("expected AgentCwd with Some path, got {other:?}"),
    }
}

#[tokio::test]
async fn cwd_unknown_returns_not_found() {
    let daemon = TestDaemon::start();

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::GetCwd {
            name: "nope".into(),
        },
    )
    .await
    .expect("write GetCwd");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => assert_eq!(code, ErrorCode::NotFound),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn send_to_running_agent_returns_ok() {
    let daemon = TestDaemon::start();

    // /bin/cat sits on its stdin forever; the daemon's writer can push
    // bytes into it without the process exiting on us.
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "echoer".to_string(),
            cmd: vec!["/bin/cat".to_string()],
            cwd: None,
            env: Vec::new(),
            initial_size: TermSize { rows: 24, cols: 80 },
        }),
    )
    .await
    .expect("write Run");
    // Drain the response; the assertions later verify the side-effect.
    #[allow(clippy::let_underscore_must_use)]
    let _ = protocol::read_msg::<DaemonToClient, _>(&mut reader).await;
    drop(stream);

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Send {
            name: "echoer".to_string(),
            payload: b"hello\n".to_vec(),
        },
    )
    .await
    .expect("write Send");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read Send response")
    {
        DaemonToClient::Ok => {}
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[tokio::test]
async fn send_unknown_returns_not_found() {
    let daemon = TestDaemon::start();

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Send {
            name: "nope".to_string(),
            payload: b"hi\n".to_vec(),
        },
    )
    .await
    .expect("write Send");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => assert_eq!(code, ErrorCode::NotFound),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn run_propagates_request_env_to_agent() {
    // Verifies the daemon-side half of "client env reaches the agent":
    // any (k, v) in RunRequest.env should land in the spawned process's
    // environment. The client::run::run() side that populates env from
    // std::env::vars() is verified separately by inspection.
    let daemon = TestDaemon::start();
    let dir = tempfile::tempdir().expect("tempdir");
    let outpath = dir.path().join("envdump");

    let key = "PSWARM_INTEGRATION_TEST_KEY";
    let value = "propagated-value";

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "envprop".to_string(),
            cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("printenv {} > {} ; sleep 5", key, outpath.display()),
            ],
            cwd: None,
            env: vec![(key.to_string(), value.to_string())],
            initial_size: TermSize { rows: 24, cols: 80 },
        }),
    )
    .await
    .expect("write Run");
    // Drain the response; the assertions later verify the side-effect.
    #[allow(clippy::let_underscore_must_use)]
    let _ = protocol::read_msg::<DaemonToClient, _>(&mut reader).await;
    drop(stream);

    // Wait for non-empty content; `outpath.exists()` alone races with
    // the shell's `>` redirect, which truncates before `printenv` writes.
    for _ in 0..40 {
        if outpath.metadata().is_ok_and(|m| m.len() > 0) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let dump = std::fs::read_to_string(&outpath).expect("read envdump");
    assert_eq!(dump.trim(), value, "expected {key}={value} in dump");
}

#[tokio::test]
async fn pswarm_daemon_env_cannot_be_overridden_by_request() {
    // The daemon must always set PSWARM_DAEMON=1 last, so the client
    // cannot accidentally (or intentionally) flip it to 0 / unset.
    let daemon = TestDaemon::start();
    let dir = tempfile::tempdir().expect("tempdir");
    let outpath = dir.path().join("envdump");

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "daemonenv".to_string(),
            cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("printenv PSWARM_DAEMON > {} ; sleep 5", outpath.display()),
            ],
            cwd: None,
            env: vec![("PSWARM_DAEMON".to_string(), "0".to_string())],
            initial_size: TermSize { rows: 24, cols: 80 },
        }),
    )
    .await
    .expect("write Run");
    // Drain the response; the assertions later verify the side-effect.
    #[allow(clippy::let_underscore_must_use)]
    let _ = protocol::read_msg::<DaemonToClient, _>(&mut reader).await;
    drop(stream);

    // Wait for non-empty content; `outpath.exists()` alone races with
    // the shell's `>` redirect, which truncates before `printenv` writes.
    for _ in 0..40 {
        if outpath.metadata().is_ok_and(|m| m.len() > 0) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let dump = std::fs::read_to_string(&outpath).expect("read envdump");
    assert_eq!(
        dump.trim(),
        "1",
        "PSWARM_DAEMON should remain '1' even when request tries to override"
    );
}

#[tokio::test]
async fn view_returns_recent_output() {
    let daemon = TestDaemon::start();

    // Spawn an agent that prints a known marker, then sleeps.
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Run(RunRequest {
            name: "viewable".to_string(),
            cmd: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'PSWARM-VIEW-MARKER\\n'; sleep 5".to_string(),
            ],
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

    // Wait for the marker to land in the ring buffer.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let mut stream = daemon.connect().await;
        let (mut reader, mut writer) = stream.split();
        protocol::write_msg(
            &mut writer,
            &ClientToDaemon::View {
                name: "viewable".to_string(),
            },
        )
        .await
        .expect("write View");
        let bytes = match protocol::read_msg::<DaemonToClient, _>(&mut reader)
            .await
            .expect("read View response")
        {
            DaemonToClient::Stdout(b) => b,
            other => panic!("expected Stdout, got {other:?}"),
        };
        if bytes
            .windows(b"PSWARM-VIEW-MARKER".len())
            .any(|w| w == b"PSWARM-VIEW-MARKER")
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "marker not found in view output after 2s: {}",
            String::from_utf8_lossy(&bytes)
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn view_unknown_returns_not_found() {
    let daemon = TestDaemon::start();

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::View {
            name: "nope".to_string(),
        },
    )
    .await
    .expect("write View");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => assert_eq!(code, ErrorCode::NotFound),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn attach_unknown_returns_not_found() {
    let daemon = TestDaemon::start();

    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Attach {
            name: "nope".to_string(),
            initial_size: TermSize { rows: 24, cols: 80 },
        },
    )
    .await
    .expect("write Attach");
    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => assert_eq!(code, ErrorCode::NotFound),
        other => panic!("expected NotFound, got {other:?}"),
    }
}
