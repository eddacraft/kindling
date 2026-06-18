//! Shared test support: a minimal hyper-over-UDS HTTP/1 client and helpers to
//! spin up a daemon on a temp socket.

#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use http_body_util::BodyExt;
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use kindling_server::{serve, ServerConfig, PROJECT_HEADER};
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::UnixStream;
use tokio::task::JoinHandle;

/// A running daemon on a temp socket with a temp kindling home.
pub struct TestDaemon {
    pub socket_path: PathBuf,
    pub kindling_home: PathBuf,
    _home: TempDir,
    handle: JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl TestDaemon {
    /// Start a daemon with a long idle timeout (won't shut down mid-test).
    pub async fn start() -> Self {
        Self::start_with_idle(Duration::from_secs(3600)).await
    }

    /// Start a daemon with a specific idle timeout.
    pub async fn start_with_idle(idle: Duration) -> Self {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        // Keep the socket path short — UDS paths have a ~108 byte limit.
        let socket_path = home_path.join("k.sock");
        let config = ServerConfig {
            socket_path: socket_path.clone(),
            kindling_home: home_path.clone(),
            pid_path: home_path.join("k.pid"),
            idle_timeout: idle,
        };
        let handle = tokio::spawn(async move { serve(config).await });

        // Wait for the socket to appear.
        for _ in 0..200 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(socket_path.exists(), "daemon socket never appeared");

        Self {
            socket_path,
            kindling_home: home_path,
            _home: home,
            handle,
        }
    }

    /// Open a fresh HTTP/1 connection to the daemon's socket and return a
    /// single-shot request sender.
    pub async fn connect(&self) -> Client {
        Client::connect(&self.socket_path).await
    }

    /// Await daemon shutdown (used by idle tests).
    pub async fn join(self) -> Result<(), kindling_server::ServerError> {
        self.handle.await.expect("serve task panicked")
    }
}

/// One HTTP/1 connection over a Unix socket.
pub struct Client {
    sender: http1::SendRequest<String>,
    socket_path: PathBuf,
}

/// A decoded HTTP response.
pub struct TestResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

impl TestResponse {
    pub fn json(&self) -> Value {
        if self.body.is_empty() {
            return Value::Null;
        }
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("response body was not JSON ({e}): {:?}", self.body))
    }
}

impl Client {
    pub async fn connect(socket_path: &std::path::Path) -> Self {
        let stream = UnixStream::connect(socket_path).await.expect("connect uds");
        let io = TokioIo::new(stream);
        let (sender, conn) = http1::handshake::<_, String>(io)
            .await
            .expect("http1 handshake");
        tokio::spawn(async move {
            let _ = conn.await;
        });
        Self {
            sender,
            socket_path: socket_path.to_path_buf(),
        }
    }

    /// Send a request with optional project header and JSON body.
    pub async fn send(
        &mut self,
        method: &str,
        path: &str,
        project: Option<&str>,
        body: Option<Value>,
    ) -> TestResponse {
        let body_str = body.map(|v| v.to_string()).unwrap_or_default();
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "kindling.local")
            .header("content-type", "application/json");
        if let Some(p) = project {
            builder = builder.header(PROJECT_HEADER, p);
        }
        let req = builder.body(body_str).unwrap();

        let resp: Response<_> = self.sender.send_request(req).await.expect("send_request");
        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        TestResponse { status, body }
    }

    /// Send a request with optional project header, arbitrary extra headers,
    /// and an optional JSON body.
    pub async fn send_with_headers(
        &mut self,
        method: &str,
        path: &str,
        project: Option<&str>,
        extra_headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> TestResponse {
        let body_str = body.map(|v| v.to_string()).unwrap_or_default();
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "kindling.local")
            .header("content-type", "application/json");
        if let Some(p) = project {
            builder = builder.header(PROJECT_HEADER, p);
        }
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        let req = builder.body(body_str).unwrap();

        let resp: Response<_> = self.sender.send_request(req).await.expect("send_request");
        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        TestResponse { status, body }
    }

    /// Path of the socket this client is connected to (for fresh connections).
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }
}
