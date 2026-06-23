//! HTTP/1 transport with auto-spawn.
//!
//! One connection per request (simple and robust; no pooling). Before each
//! request we ensure the daemon is reachable: if the rendezvous (a UDS path on
//! Unix, a published TCP port on Windows/TCP) is missing or a connect is
//! refused, we invoke the spawner ONCE and poll until a connection succeeds or
//! the connect budget elapses.
//!
//! The HTTP/1 exchange itself ([`send_http`]) is transport-agnostic: it wraps
//! any tokio `AsyncRead + AsyncWrite` in [`TokioIo`], so the UDS and TCP paths
//! share it verbatim. The transport is selected per [`ClientConfig::transport`].

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use http_body_util::BodyExt;
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::Request;
use hyper::StatusCode;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

use crate::config::{append_spawn_log, ClientConfig, Spawner, Transport};
use crate::error::ClientError;

/// A decoded HTTP response: status plus raw body bytes.
pub(crate) struct RawResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

/// One request to send: verb, URI, optional project header, and a
/// pre-serialized JSON body (empty for bodyless requests).
pub(crate) struct OutgoingRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    /// `X-Kindling-Project` header value; `None` for `/v1/health`.
    pub project: Option<&'a str>,
    pub body: String,
}

/// Connect to the daemon (spawning + polling per `cfg` if necessary) over the
/// configured transport, then send one request and collect the response.
pub(crate) async fn request(
    cfg: &ClientConfig,
    req: OutgoingRequest<'_>,
) -> Result<RawResponse, ClientError> {
    match cfg.transport {
        #[cfg(unix)]
        Transport::Uds => {
            let stream = ensure_connected(
                &cfg.socket_path,
                &cfg.spawn,
                cfg.connect_timeout,
                cfg.poll_interval,
                cfg.effective_spawn_log_path().as_deref(),
            )
            .await?;
            send_http(stream, req).await
        }
        Transport::Tcp => {
            let stream = ensure_connected_tcp(
                &cfg.port_path,
                &cfg.spawn,
                cfg.connect_timeout,
                cfg.poll_interval,
                cfg.effective_spawn_log_path().as_deref(),
            )
            .await?;
            send_http(stream, req).await
        }
    }
}

/// Send one HTTP/1 request over an established byte stream and collect the
/// response. Transport-agnostic: works over any `AsyncRead + AsyncWrite`.
async fn send_http<S>(stream: S, req: OutgoingRequest<'_>) -> Result<RawResponse, ClientError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let io = TokioIo::new(stream);
    let OutgoingRequest {
        method,
        path,
        project,
        body,
    } = req;

    let (mut sender, conn) = http1::handshake::<_, String>(io)
        .await
        .map_err(|e| ClientError::Http(format!("handshake failed: {e}")))?;

    // Drive the connection in the background for the lifetime of this request.
    let conn_task = tokio::spawn(async move {
        let _ = conn.await;
    });

    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "kindling.local")
        .header("content-type", "application/json");
    if let Some(p) = project {
        builder = builder.header(crate::PROJECT_HEADER, p);
    }
    let req = builder
        .body(body)
        .map_err(|e| ClientError::Http(format!("building request: {e}")))?;

    let resp = sender
        .send_request(req)
        .await
        .map_err(|e| ClientError::Http(format!("send_request: {e}")))?;
    let status = resp.status();
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| ClientError::Http(format!("reading body: {e}")))?
        .to_bytes();

    conn_task.abort();
    Ok(RawResponse { status, body })
}

/// Ensure the daemon is reachable over UDS, auto-spawning once if needed.
///
/// 1. Try to connect. On success, return the stream.
/// 2. If the socket is missing OR the connect is refused/not-found, invoke the
///    spawner ONCE, then poll-connect every `poll_interval` until success or
///    the `connect_timeout` budget elapses.
/// 3. If still failing, return [`ClientError::Unavailable`].
#[cfg(unix)]
async fn ensure_connected(
    socket_path: &Path,
    spawner: &Spawner,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn_log_path: Option<&Path>,
) -> Result<UnixStream, ClientError> {
    // Fast path: daemon already up.
    match UnixStream::connect(socket_path).await {
        Ok(stream) => return Ok(stream),
        Err(e) if is_absent(&e) => { /* fall through to spawn + poll */ }
        Err(e) => {
            return Err(ClientError::Unavailable(format!(
                "connecting to {}: {e}",
                socket_path.display()
            )));
        }
    }

    // Spawn exactly once, then poll within the budget.
    if let Err(e) = spawner.spawn() {
        let msg = format!("failed to spawn kindling daemon: {e}");
        log_spawn_failure(spawn_log_path, &msg);
        return Err(ClientError::Unavailable(msg));
    }

    let deadline = Instant::now() + connect_timeout;
    loop {
        let last_err = match UnixStream::connect(socket_path).await {
            Ok(stream) => return Ok(stream),
            Err(e) if is_absent(&e) => e.to_string(),
            Err(e) => {
                let msg = format!("connecting to {} after spawn: {e}", socket_path.display());
                log_spawn_failure(spawn_log_path, &msg);
                return Err(ClientError::Unavailable(msg));
            }
        };
        if Instant::now() >= deadline {
            let msg = format!(
                "daemon socket {} did not become reachable within {:?} after spawn ({last_err})",
                socket_path.display(),
                connect_timeout
            );
            log_spawn_failure(spawn_log_path, &msg);
            return Err(ClientError::Unavailable(msg));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// Ensure the daemon is reachable over loopback TCP, auto-spawning once if
/// needed. This is the Windows default transport, and is exercised on Linux.
///
/// Unlike UDS, TCP has no filesystem rendezvous: the daemon binds an ephemeral
/// port and publishes it to `port_path`. So discovery is two-step — read the
/// port file, then connect to `127.0.0.1:<port>`.
///
/// 1. Read the port file and try to connect. On success, return the stream.
/// 2. If the port file is missing/unparseable OR the connect is refused, invoke
///    the spawner ONCE, then poll (re-reading the port file each iteration, as
///    the daemon writes it only after binding) until success or the
///    `connect_timeout` budget elapses.
/// 3. If still failing, return [`ClientError::Unavailable`].
async fn ensure_connected_tcp(
    port_path: &Path,
    spawner: &Spawner,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn_log_path: Option<&Path>,
) -> Result<TcpStream, ClientError> {
    // Fast path: port file present and daemon already up.
    if let Some(port) = read_port(port_path) {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(stream) => return Ok(stream),
            Err(e) if is_absent(&e) => { /* stale port file; spawn + poll */ }
            Err(e) => {
                return Err(ClientError::Unavailable(format!(
                    "connecting to 127.0.0.1:{port}: {e}"
                )));
            }
        }
    }

    // Spawn exactly once, then poll within the budget.
    if let Err(e) = spawner.spawn() {
        let msg = format!("failed to spawn kindling daemon: {e}");
        log_spawn_failure(spawn_log_path, &msg);
        return Err(ClientError::Unavailable(msg));
    }

    let deadline = Instant::now() + connect_timeout;
    loop {
        let last_err = match read_port(port_path) {
            Some(port) => match TcpStream::connect(("127.0.0.1", port)).await {
                Ok(stream) => return Ok(stream),
                Err(e) if is_absent(&e) => format!("connecting to 127.0.0.1:{port}: {e}"),
                Err(e) => {
                    let msg = format!("connecting to 127.0.0.1:{port} after spawn: {e}");
                    log_spawn_failure(spawn_log_path, &msg);
                    return Err(ClientError::Unavailable(msg));
                }
            },
            None => format!("port file {} not yet present", port_path.display()),
        };
        if Instant::now() >= deadline {
            let msg = format!(
                "daemon TCP port (via {}) did not become reachable within {:?} after spawn ({last_err})",
                port_path.display(),
                connect_timeout
            );
            log_spawn_failure(spawn_log_path, &msg);
            return Err(ClientError::Unavailable(msg));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn log_spawn_failure(spawn_log_path: Option<&Path>, detail: &str) {
    if let Some(path) = spawn_log_path {
        append_spawn_log(path, detail);
    }
}

/// Read and parse the daemon's published TCP port. Returns `None` when the file
/// is missing, empty, or does not contain a valid `u16` (treated identically to
/// "daemon not listening yet").
fn read_port(port_path: &Path) -> Option<u16> {
    std::fs::read_to_string(port_path)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
}

/// Whether a connect error means "daemon not (yet) listening": a missing socket
/// file or a refused connection.
fn is_absent(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}
