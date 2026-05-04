//! Integration test harness: spawn a fresh `pswarm daemon` in a tempdir,
//! hand it back to the test as a `TestDaemon`, and tear it down on drop.

// Each test binary that does `mod common;` gets its own compilation of
// this file. A helper used by only some of the test binaries shows up as
// dead_code in the others — silence that.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::net::UnixStream;

use picoswarm::protocol::{
    self, ClientToDaemon, DaemonToClient, PROTOCOL_VERSION,
};

/// A daemon instance scoped to one test, with its own runtime/state dirs
/// and Unix socket. Killed and cleaned up on drop.
pub struct TestDaemon {
    pub socket_path: PathBuf,
    _runtime_dir: TempDir,
    daemon: Child,
}

impl TestDaemon {
    pub fn start() -> Self {
        let runtime_dir = TempDir::new().expect("failed to create tempdir");
        let runtime_path = runtime_dir.path().to_path_buf();
        let socket_path = runtime_path.join("picoswarm").join("sock");

        let daemon = Command::new(env!("CARGO_BIN_EXE_pswarm"))
            .args(["daemon", "start"])
            .env("PSWARM_DAEMON_FOREGROUND", "1")
            .env("XDG_RUNTIME_DIR", &runtime_path)
            .env("XDG_STATE_HOME", &runtime_path)
            .env("RUST_LOG", "warn")
            // Drop stdout/stderr; tests don't inspect daemon logs.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn pswarm daemon");

        wait_for_socket(&socket_path);

        Self {
            socket_path,
            _runtime_dir: runtime_dir,
            daemon,
        }
    }

    /// Open a fresh connection and complete the Hello handshake. Returns
    /// the open stream ready for a request.
    pub async fn connect(&self) -> UnixStream {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .expect("failed to connect to test daemon socket");

        let (mut reader, mut writer) = stream.split();
        protocol::write_msg(
            &mut writer,
            &ClientToDaemon::Hello {
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await
        .expect("failed to send Hello");
        let resp: DaemonToClient = protocol::read_msg(&mut reader)
            .await
            .expect("failed to read Hello response");
        match resp {
            DaemonToClient::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => {}
            other => panic!("unexpected handshake response: {other:?}"),
        }
        drop((reader, writer));
        stream
    }

    /// Open a connection without doing the Hello handshake. Use for tests
    /// that exercise the handshake itself.
    pub async fn connect_raw(&self) -> UnixStream {
        UnixStream::connect(&self.socket_path)
            .await
            .expect("failed to connect to test daemon socket")
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn wait_for_socket(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if path.exists() {
            // Confirm a daemon is actually listening, not just that the
            // file exists from a half-completed bind.
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "test daemon socket did not become reachable within 2s: {}",
        path.display()
    );
}
