//! Run / Ls / Rm / Attach (error paths only) integration tests.

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

async fn rm_agent(
    daemon: &TestDaemon,
    name: &str,
    force: bool,
) -> Result<(), (ErrorCode, String)> {
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
    rm_agent(&daemon, "beta", false).await.expect("rm should succeed");

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

    let err = rm_agent(&daemon, "nope", false).await.expect_err("expected error");
    assert_eq!(err.0, ErrorCode::NotFound);
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
