//! Shared test support: spin up a real `kindling-server` daemon on a temp
//! socket + temp kindling home in a background task, and build clients pointed
//! at it. Mirrors the `kindling-client` test harness.

#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use kindling_client::{Client, ClientConfig, Spawner, Transport};
use kindling_server::{serve, ServerConfig};
use tempfile::TempDir;
use tokio::task::JoinHandle;

/// The store's canonical schema version, as `u32` (the client's type).
pub fn schema_version_u32() -> u32 {
    kindling_store::schema_version().version as u32
}

/// A running in-process daemon on a temp socket.
pub struct TestDaemon {
    pub socket_path: PathBuf,
    pub kindling_home: PathBuf,
    _home: TempDir,
    handle: JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl TestDaemon {
    /// Start a daemon and wait for its socket to appear.
    pub async fn start() -> Self {
        let (config, home, socket_path) = temp_server_config();
        let handle = tokio::spawn(async move { serve(config).await });
        wait_for_socket(&socket_path).await;
        Self {
            socket_path,
            kindling_home: home.path().to_path_buf(),
            _home: home,
            handle,
        }
    }

    /// Build a client for `project_root` against this daemon, with a spawner
    /// that must never be invoked (the daemon is already up).
    pub fn client(&self, project_root: &str) -> Client {
        live_client(self.socket_path.clone(), project_root)
    }
}

/// A client pointed at `socket_path`, with a spawner that panics if invoked
/// (the daemon is expected to be running).
pub fn live_client(socket_path: PathBuf, project_root: &str) -> Client {
    let cfg = ClientConfig {
        socket_path,
        port_path: PathBuf::from("unused.port"),
        project_root: project_root.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_secs(2),
        poll_interval: Duration::from_millis(10),
        spawn: Spawner::custom(|| panic!("spawner must not be called when daemon is running")),
        transport: Transport::default(),
    };
    Client::with_config(cfg)
}

/// A client pointed at a socket with NO daemon and a spawner that fails like a
/// missing binary would — every call resolves to `ClientError::Unavailable`
/// within a short budget. This is how we simulate "daemon down".
pub fn down_client(socket_path: PathBuf, project_root: &str) -> Client {
    let cfg = ClientConfig {
        socket_path,
        port_path: PathBuf::from("unused.port"),
        project_root: project_root.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_millis(150),
        poll_interval: Duration::from_millis(10),
        spawn: Spawner::custom(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "kindling binary not found (simulated daemon-down)",
            ))
        }),
        transport: Transport::default(),
    };
    Client::with_config(cfg)
}

/// Build a `ServerConfig` over a fresh temp home with a short socket path and a
/// long idle timeout. Returns the config, the (owned) temp dir, and the socket.
pub fn temp_server_config() -> (ServerConfig, TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let home_path = home.path().to_path_buf();
    let socket_path = home_path.join("k.sock");
    let config = ServerConfig {
        socket_path: socket_path.clone(),
        kindling_home: home_path.clone(),
        pid_path: home_path.join("k.pid"),
        port_path: home_path.join("k.port"),
        idle_timeout: Duration::from_secs(3600),
        transport: kindling_server::Transport::default(),
    };
    (config, home, socket_path)
}

/// Poll until the socket file exists (or panic after a generous bound).
pub async fn wait_for_socket(socket_path: &std::path::Path) {
    for _ in 0..400 {
        if socket_path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("daemon socket never appeared: {}", socket_path.display());
}
