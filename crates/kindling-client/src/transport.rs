//! HTTP/1-over-UDS transport with auto-spawn.
//!
//! One connection per request (simple and robust; no pooling). Before each
//! request we ensure the daemon is reachable: if the socket is missing or a
//! connect is refused, we invoke the spawner ONCE and poll the socket until a
//! connection succeeds or the connect budget elapses.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use http_body_util::BodyExt;
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

use crate::config::{ClientConfig, Spawner};
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

/// Connect to the daemon socket (spawning + polling per `cfg` if necessary),
/// then send one request and collect the response.
pub(crate) async fn request(
    cfg: &ClientConfig,
    req: OutgoingRequest<'_>,
) -> Result<RawResponse, ClientError> {
    let stream = ensure_connected(
        &cfg.socket_path,
        &cfg.spawn,
        cfg.connect_timeout,
        cfg.poll_interval,
    )
    .await?;
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

/// Ensure the daemon is reachable, auto-spawning once if needed.
///
/// 1. Try to connect. On success, return the stream.
/// 2. If the socket is missing OR the connect is refused/not-found, invoke the
///    spawner ONCE, then poll-connect every `poll_interval` until success or
///    the `connect_timeout` budget elapses.
/// 3. If still failing, return [`ClientError::Unavailable`].
async fn ensure_connected(
    socket_path: &Path,
    spawner: &Spawner,
    connect_timeout: Duration,
    poll_interval: Duration,
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
        return Err(ClientError::Unavailable(format!(
            "failed to spawn kindling daemon: {e}"
        )));
    }

    let deadline = Instant::now() + connect_timeout;
    loop {
        let last_err = match UnixStream::connect(socket_path).await {
            Ok(stream) => return Ok(stream),
            Err(e) if is_absent(&e) => e.to_string(),
            Err(e) => {
                return Err(ClientError::Unavailable(format!(
                    "connecting to {} after spawn: {e}",
                    socket_path.display()
                )));
            }
        };
        if Instant::now() >= deadline {
            return Err(ClientError::Unavailable(format!(
                "daemon socket {} did not become reachable within {:?} after spawn ({last_err})",
                socket_path.display(),
                connect_timeout
            )));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// Whether a connect error means "daemon not (yet) listening": a missing socket
/// file or a refused connection.
fn is_absent(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}
