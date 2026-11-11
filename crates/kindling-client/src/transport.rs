//! HTTP/1 transport with auto-spawn.
//!
//! One connection per request (simple and robust; no pooling). Before each
//! request we ensure the daemon is reachable: if the rendezvous (a UDS path on
//! Unix, a published TCP port on Windows/TCP) is missing or a connect is
//! refused, we invoke the spawner ONCE and poll until a connection succeeds or
//! the connect budget elapses. The budget covers the initial connect, spawn,
//! and retry sleeps; each connect attempt and poll sleep is capped by the
//! remaining time.
//!
//! The HTTP/1 exchange itself ([`send_http`]) is transport-agnostic: it wraps
//! any tokio `AsyncRead + AsyncWrite` in [`TokioIo`], so the UDS and TCP paths
//! share it verbatim. The transport is selected per [`ClientConfig::transport`].

use std::future::Future;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use http_body_util::BodyExt;
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::header::HeaderValue;
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
    // Reject permanently invalid local headers before connect/spawn so a
    // malformed request is never mistaken for an outage (and never spooled).
    validate_outgoing(&req)?;
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
            // UDS authenticates by filesystem permissions; no bearer token.
            send_http(stream, req, None).await
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
            // Loopback TCP has no per-user boundary, so the daemon requires a
            // per-daemon bearer secret on every non-health request (KINTEG-010).
            // The secret lives in an owner-only sibling of the port file, written
            // by the daemon before it binds — so it exists once we can connect.
            // Read it best-effort: a missing token yields a 401 from the daemon
            // (surfaced as a normal error) rather than a panic here.
            let token = std::fs::read_to_string(token_path_for(&cfg.port_path))
                .ok()
                .map(|s| s.trim().to_string());
            send_http(stream, req, token.as_deref()).await
        }
    }
}

/// Token-file path for a given port-file path (sibling file). Mirrors the
/// daemon's `token_path_for` so both agree without a shared config field.
fn token_path_for(port_path: &std::path::Path) -> std::path::PathBuf {
    port_path.with_file_name("kindling.token")
}

/// Fail loud on request fields that can never become a valid HTTP/1 exchange.
///
/// Today that is the project-root header: control characters such as a newline
/// are illegal in HTTP header values. Catching them here distinguishes a
/// permanent local validation error from a later transport failure.
fn validate_outgoing(req: &OutgoingRequest<'_>) -> Result<(), ClientError> {
    if let Some(project) = req.project {
        if HeaderValue::try_from(project).is_err() {
            return Err(ClientError::InvalidRequest(format!(
                "invalid {} header value",
                crate::PROJECT_HEADER
            )));
        }
    }
    Ok(())
}

/// Send one HTTP/1 request over an established byte stream and collect the
/// response. Transport-agnostic: works over any `AsyncRead + AsyncWrite`.
async fn send_http<S>(
    stream: S,
    req: OutgoingRequest<'_>,
    auth_token: Option<&str>,
) -> Result<RawResponse, ClientError>
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
    // TCP transport only: present the per-daemon bearer secret. Harmless on the
    // health route (the daemon ignores it there) and never sent over UDS.
    if let Some(token) = auth_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let req = builder
        .body(body)
        .map_err(|e| ClientError::InvalidRequest(format!("building request: {e}")))?;

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
/// 1. Try to connect, bounded by `connect_timeout`. On success, return the
///    stream.
/// 2. If the socket is missing OR the connect is refused/not-found, invoke the
///    spawner ONCE (still inside the same budget), then poll-connect every
///    `poll_interval` until success or the remaining budget elapses. Each
///    connect and sleep is capped by the time left.
/// 3. If still failing, return [`ClientError::Unavailable`].
#[cfg(unix)]
async fn ensure_connected(
    socket_path: &Path,
    spawner: &Spawner,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn_log_path: Option<&Path>,
) -> Result<UnixStream, ClientError> {
    let deadline = Instant::now() + connect_timeout;
    let timeout_target = format!("daemon socket {}", socket_path.display());

    // Fast path: daemon already up.
    match within_budget(deadline, UnixStream::connect(socket_path)).await {
        Ok(Ok(stream)) => return Ok(stream),
        Ok(Err(e)) if is_absent(&e) => { /* fall through to spawn + poll */ }
        Ok(Err(e)) => {
            return Err(ClientError::Unavailable(format!(
                "connecting to {}: {e}",
                socket_path.display()
            )));
        }
        Err(BudgetExhausted) => {
            return timeout_unavailable(
                &timeout_target,
                connect_timeout,
                "connect timed out",
                spawn_log_path,
            );
        }
    }

    // Spawn exactly once, then poll within the remaining budget.
    if let Err(e) = spawner.spawn() {
        let msg = format!("failed to spawn kindling daemon: {e}");
        log_spawn_failure(spawn_log_path, &msg);
        return Err(ClientError::Unavailable(msg));
    }

    if remaining_budget(deadline).is_none() {
        return timeout_unavailable(
            &timeout_target,
            connect_timeout,
            "spawn exhausted the connect budget",
            spawn_log_path,
        );
    }

    let mut last_err = String::from("not yet reachable");
    loop {
        match within_budget(deadline, UnixStream::connect(socket_path)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(e)) if is_absent(&e) => last_err = e.to_string(),
            Ok(Err(e)) => {
                let msg = format!("connecting to {} after spawn: {e}", socket_path.display());
                log_spawn_failure(spawn_log_path, &msg);
                return Err(ClientError::Unavailable(msg));
            }
            Err(BudgetExhausted) => {
                return timeout_unavailable(
                    &timeout_target,
                    connect_timeout,
                    &last_err,
                    spawn_log_path,
                );
            }
        }
        if let Err(BudgetExhausted) = sleep_poll_interval(deadline, poll_interval).await {
            return timeout_unavailable(
                &timeout_target,
                connect_timeout,
                &last_err,
                spawn_log_path,
            );
        }
    }
}

/// Ensure the daemon is reachable over loopback TCP, auto-spawning once if
/// needed. This is the Windows default transport, and is exercised on Linux.
///
/// Unlike UDS, TCP has no filesystem rendezvous: the daemon binds an ephemeral
/// port and publishes it to `port_path`. So discovery is two-step — read the
/// port file, then connect to `127.0.0.1:<port>`.
///
/// 1. Read the port file and try to connect, bounded by `connect_timeout`. On
///    success, return the stream.
/// 2. If the port file is missing/unparseable OR the connect is refused, invoke
///    the spawner ONCE (still inside the same budget), then poll (re-reading
///    the port file each iteration, as the daemon writes it only after binding)
///    until success or the remaining budget elapses. Each connect and sleep is
///    capped by the time left.
/// 3. If still failing, return [`ClientError::Unavailable`].
async fn ensure_connected_tcp(
    port_path: &Path,
    spawner: &Spawner,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn_log_path: Option<&Path>,
) -> Result<TcpStream, ClientError> {
    let deadline = Instant::now() + connect_timeout;
    let timeout_target = format!("daemon TCP port (via {})", port_path.display());

    // Fast path: port file present and daemon already up.
    if let Some(port) = read_port(port_path) {
        match within_budget(deadline, TcpStream::connect(("127.0.0.1", port))).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(e)) if is_absent(&e) => { /* stale port file; spawn + poll */ }
            Ok(Err(e)) => {
                return Err(ClientError::Unavailable(format!(
                    "connecting to 127.0.0.1:{port}: {e}"
                )));
            }
            Err(BudgetExhausted) => {
                return timeout_unavailable(
                    &timeout_target,
                    connect_timeout,
                    "connect timed out",
                    spawn_log_path,
                );
            }
        }
    }

    // Spawn exactly once, then poll within the remaining budget.
    if let Err(e) = spawner.spawn() {
        let msg = format!("failed to spawn kindling daemon: {e}");
        log_spawn_failure(spawn_log_path, &msg);
        return Err(ClientError::Unavailable(msg));
    }

    if remaining_budget(deadline).is_none() {
        return timeout_unavailable(
            &timeout_target,
            connect_timeout,
            "spawn exhausted the connect budget",
            spawn_log_path,
        );
    }

    let mut last_err = format!("port file {} not yet present", port_path.display());
    loop {
        match read_port(port_path) {
            Some(port) => {
                match within_budget(deadline, TcpStream::connect(("127.0.0.1", port))).await {
                    Ok(Ok(stream)) => return Ok(stream),
                    Ok(Err(e)) if is_absent(&e) => {
                        last_err = format!("connecting to 127.0.0.1:{port}: {e}");
                    }
                    Ok(Err(e)) => {
                        let msg = format!("connecting to 127.0.0.1:{port} after spawn: {e}");
                        log_spawn_failure(spawn_log_path, &msg);
                        return Err(ClientError::Unavailable(msg));
                    }
                    Err(BudgetExhausted) => {
                        return timeout_unavailable(
                            &timeout_target,
                            connect_timeout,
                            &last_err,
                            spawn_log_path,
                        );
                    }
                }
            }
            None => {
                last_err = format!("port file {} not yet present", port_path.display());
            }
        }
        if let Err(BudgetExhausted) = sleep_poll_interval(deadline, poll_interval).await {
            return timeout_unavailable(
                &timeout_target,
                connect_timeout,
                &last_err,
                spawn_log_path,
            );
        }
    }
}

fn log_spawn_failure(spawn_log_path: Option<&Path>, detail: &str) {
    if let Some(path) = spawn_log_path {
        append_spawn_log(path, detail);
    }
}

fn timeout_unavailable<T>(
    what: &str,
    connect_timeout: Duration,
    last_err: &str,
    spawn_log_path: Option<&Path>,
) -> Result<T, ClientError> {
    let msg = format!("{what} did not become reachable within {connect_timeout:?} ({last_err})");
    log_spawn_failure(spawn_log_path, &msg);
    Err(ClientError::Unavailable(msg))
}

/// Marker returned when the connect budget has no time left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BudgetExhausted;

fn remaining_budget(deadline: Instant) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        None
    } else {
        Some(remaining)
    }
}

/// Run `fut`, aborting it once `deadline` is reached.
async fn within_budget<T>(
    deadline: Instant,
    fut: impl Future<Output = T>,
) -> Result<T, BudgetExhausted> {
    let Some(remaining) = remaining_budget(deadline) else {
        return Err(BudgetExhausted);
    };
    tokio::time::timeout(remaining, fut)
        .await
        .map_err(|_| BudgetExhausted)
}

/// Sleep `poll_interval`, or whatever is left in the budget, whichever is shorter.
async fn sleep_poll_interval(
    deadline: Instant,
    poll_interval: Duration,
) -> Result<(), BudgetExhausted> {
    let Some(remaining) = remaining_budget(deadline) else {
        return Err(BudgetExhausted);
    };
    tokio::time::sleep(remaining.min(poll_interval)).await;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_budget_is_none_once_deadline_passes() {
        let deadline = Instant::now() + Duration::from_millis(1);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(remaining_budget(deadline), None);
    }

    #[tokio::test]
    async fn slow_attempt_is_capped_by_remaining_budget() {
        let deadline = Instant::now() + Duration::from_millis(40);
        let start = Instant::now();
        let result = within_budget(deadline, tokio::time::sleep(Duration::from_secs(5))).await;
        let elapsed = start.elapsed();
        assert_eq!(result, Err(BudgetExhausted));
        assert!(
            elapsed < Duration::from_millis(250),
            "slow attempt overran the budget: {elapsed:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(20),
            "slow attempt returned too quickly: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn poll_sleep_does_not_exceed_remaining_budget() {
        let deadline = Instant::now() + Duration::from_millis(30);
        let start = Instant::now();
        sleep_poll_interval(deadline, Duration::from_secs(2))
            .await
            .expect("sleep within remaining budget");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(250),
            "poll sleep used the full interval past the deadline: {elapsed:?}"
        );
        assert_eq!(remaining_budget(deadline), None);
    }

    #[tokio::test]
    async fn exhausted_budget_skips_poll_sleep() {
        let deadline = Instant::now();
        let start = Instant::now();
        let result = sleep_poll_interval(deadline, Duration::from_secs(2)).await;
        assert_eq!(result, Err(BudgetExhausted));
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
