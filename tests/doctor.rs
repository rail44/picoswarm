//! Hello handshake + Ping/Pong round-trip tests.

mod common;

use common::TestDaemon;
use picoswarm::protocol::{
    self, ClientToDaemon, DaemonToClient, ErrorCode, PROTOCOL_VERSION,
};

#[tokio::test]
async fn ping_pong() {
    let daemon = TestDaemon::start();
    let mut stream = daemon.connect().await;
    let (mut reader, mut writer) = stream.split();

    protocol::write_msg(&mut writer, &ClientToDaemon::Ping)
        .await
        .expect("write Ping");

    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read Pong")
    {
        DaemonToClient::Pong => {}
        other => panic!("expected Pong, got {other:?}"),
    }
}

#[tokio::test]
async fn protocol_version_mismatch_is_rejected() {
    let daemon = TestDaemon::start();
    let mut stream = daemon.connect_raw().await;
    let (mut reader, mut writer) = stream.split();

    // Pretend to speak a future protocol version.
    let bogus_version = PROTOCOL_VERSION + 999;
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Hello {
            protocol_version: bogus_version,
        },
    )
    .await
    .expect("write Hello");

    match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .expect("read response")
    {
        DaemonToClient::Error { code, .. } => {
            assert_eq!(code, ErrorCode::ProtocolMismatch);
        }
        other => panic!("expected ProtocolMismatch error, got {other:?}"),
    }
}
