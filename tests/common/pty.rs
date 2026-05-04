//! PTY-driven harness for testing `pswarm attach`.
//!
//! Spawns `pswarm attach <name>` inside a fresh PTY, drains the master
//! into an output buffer on a background OS thread, and exposes a tiny
//! "send keys / wait for substring / wait for exit" API for tests.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, ExitStatus, PtySize, native_pty_system};

use super::TestDaemon;

pub struct PtyClient {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<Vec<u8>>>,
    _reader: JoinHandle<()>,
}

impl PtyClient {
    /// Spawn `pswarm attach <agent_name>` against `daemon` inside a PTY.
    pub fn spawn_attach(daemon: &TestDaemon, agent_name: &str) -> Self {
        let runtime_dir = runtime_dir_from_socket(&daemon.socket_path);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_pswarm"));
        cmd.args(["attach", agent_name]);
        cmd.env("XDG_RUNTIME_DIR", &runtime_dir);
        cmd.env("XDG_STATE_HOME", &runtime_dir);
        // Some terminal libraries probe TERM; give them something benign.
        cmd.env("TERM", "xterm-256color");
        cmd.env("RUST_LOG", "warn");

        let child = pair.slave.spawn_command(cmd).expect("spawn pswarm attach");
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().expect("try_clone_reader");
        let writer = pair.master.take_writer().expect("take_writer");

        let output: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let reader_thread = {
            let output = Arc::clone(&output);
            std::thread::spawn(move || drain_into(reader, output))
        };

        // Drop the master once we've taken its reader and writer; otherwise
        // it would keep the slave end open and prevent the child from
        // seeing EOF when it closes its side.
        drop(pair.master);

        Self {
            child,
            writer,
            output,
            _reader: reader_thread,
        }
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write to PTY");
        self.writer.flush().expect("flush PTY");
    }

    pub fn output_so_far(&self) -> Vec<u8> {
        self.output.lock().unwrap().clone()
    }

    /// Block (up to `timeout`) until `needle` appears in the output buffer.
    pub fn wait_for_substring(&self, needle: &[u8], timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            {
                let buf = self.output.lock().unwrap();
                if find_subslice(&buf, needle).is_some() {
                    return true;
                }
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Block (up to `timeout`) until the child exits. Returns the exit
    /// status if it did, `None` on timeout.
    pub fn wait_for_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for PtyClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn drain_into(mut reader: Box<dyn Read + Send>, sink: Arc<Mutex<Vec<u8>>>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                let mut sink = sink.lock().unwrap();
                sink.extend_from_slice(&buf[..n]);
            }
            Err(_) => return,
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn runtime_dir_from_socket(socket: &std::path::Path) -> PathBuf {
    socket
        .parent()
        .and_then(|p| p.parent())
        .expect("socket path has runtime dir grandparent")
        .to_path_buf()
}
