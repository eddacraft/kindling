//! Shared test support: spin up a real `kindling-server` daemon on a temp
//! socket + temp kindling home in a background task, and build clients pointed
//! at it.

#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use kindling_client::{ClientConfig, Spawner};
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
    /// Start a daemon and wait for its socket to appear (long idle timeout so
    /// it won't shut down mid-test).
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
    pub fn client(&self, project_root: &str) -> kindling_client::Client {
        let cfg = ClientConfig {
            socket_path: self.socket_path.clone(),
            project_root: project_root.to_string(),
            expected_schema_version: schema_version_u32(),
            connect_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(10),
            spawn: Spawner::custom(|| panic!("spawner must not be called when daemon is running")),
        };
        kindling_client::Client::with_config(cfg)
    }

    /// Build a client with an explicit expected schema version (for the
    /// schema-mismatch test).
    pub fn client_with_schema(&self, project_root: &str, expected: u32) -> kindling_client::Client {
        let cfg = ClientConfig {
            socket_path: self.socket_path.clone(),
            project_root: project_root.to_string(),
            expected_schema_version: expected,
            connect_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(10),
            spawn: Spawner::custom(|| panic!("spawner must not be called when daemon is running")),
        };
        kindling_client::Client::with_config(cfg)
    }
}

/// Build a `ServerConfig` over a fresh temp home with a short socket path and a
/// long idle timeout. Returns the config, the (owned) temp dir, and the socket.
pub fn temp_server_config() -> (ServerConfig, TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let home_path = home.path().to_path_buf();
    // Keep the socket name short — UDS paths cap at ~108 bytes.
    let socket_path = home_path.join("k.sock");
    let config = ServerConfig {
        socket_path: socket_path.clone(),
        kindling_home: home_path.clone(),
        pid_path: home_path.join("k.pid"),
        idle_timeout: Duration::from_secs(3600),
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
