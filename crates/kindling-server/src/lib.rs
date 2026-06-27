//! kindling daemon — HTTP API over a Unix domain socket.
//!
//! A long-running per-user process that serves the kindling v1 HTTP API,
//! wrapping the in-process [`kindling_service::KindlingService`]. This is the
//! `kindling serve` backend; the CLI wiring lives in a later task (PORT-013).
//! This crate exposes a library surface so it can be both unit/integration
//! tested and driven by the future CLI.
//!
//! # v1 HTTP API
//!
//! ```text
//! GET    /v1/health                  → 200 { version, schemaVersion, supportedKinds, storagePath, kindRegistry, projects: [...] }
//! POST   /v1/capsules                → 201 Capsule
//! GET    /v1/capsules/open?sessionId → 200 Capsule | null
//! PATCH  /v1/capsules/:id/close      → 200 Capsule
//! POST   /v1/observations            → 201 Observation
//! POST   /v1/observations/list       → 200 ListObservationsResult (paginated)
//! POST   /v1/observations/:id/forget  → 204 (redact an observation)
//! POST   /v1/retrieve                → 200 RetrieveResult
//! POST   /v1/pins                    → 201 Pin
//! DELETE /v1/pins/:id                → 204
//! POST   /v1/context/session-start   → 200 { additionalContext: string | null }
//! POST   /v1/context/pre-compact     → 200 { additionalContext: string | null }
//! ```
//!
//! Request bodies are camelCase JSON; response bodies serialize the domain
//! types (already camelCase). See [`dto`] for the request shapes. The
//! `/v1/context/*` endpoints assemble AND format the injected-context markdown
//! server-side (the byte-for-byte date/markdown logic lives in [`inject`]).
//!
//! # Per-project routing
//!
//! Every data endpoint requires the `X-Kindling-Project` header. Its value is
//! the **project root string**; the daemon derives the SQLite DB via
//! [`kindling_store::project_db_path`] under [`ServerConfig::kindling_home`]
//! and caches one service per project. `/v1/health` needs no header; any other
//! endpoint without it returns `400`.
//!
//! # Lifecycle
//!
//! [`serve`] acquires a PID lock (cleaning up a stale file — see [`pid`]), binds
//! the UDS at mode `0600`, builds the router, and runs until idle. The daemon
//! shuts down after [`ServerConfig::idle_timeout`] of no in-flight and no
//! recent requests, then removes the socket and PID file.

mod config;
mod dto;
mod error;
mod handlers;
pub mod inject;
mod pid;
mod state;

pub use config::{ServerConfig, Transport, DEFAULT_IDLE_TIMEOUT};
pub use error::{ApiError, ServerError};
pub use handlers::{PROJECT_HEADER, SESSION_HEADER};
pub use pid::{acquire_pid_lock, PidGuard};
pub use state::AppState;

use std::sync::Arc;
use std::time::Duration;

use axum::response::IntoResponse;
use axum::routing::{delete, patch, post};
use axum::Router;

/// Side-channel file holding the per-daemon TCP auth secret. Lives next to the
/// port file ([`ServerConfig::port_path`]); written owner-only on the TCP path.
const TOKEN_FILE: &str = "kindling.token";

/// Generate a 256-bit per-daemon secret, hex-encoded (64 chars). Used to
/// authenticate non-health requests over the loopback-TCP transport, which —
/// unlike the UDS path's filesystem permissions — has no per-user boundary
/// (KINTEG-010).
fn generate_tcp_secret() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS CSPRNG available for daemon auth secret");
    let mut hex = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Constant-time byte comparison so a bearer-token check does not leak the
/// secret through timing. Unequal lengths fail, but the loop still runs over
/// the presented value to keep the work data-independent.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Token-file path for a given port-file path (sibling file). Both the daemon
/// and the client derive it from the shared port-file rendezvous so they agree
/// without a separate config field.
fn token_path_for(port_path: &std::path::Path) -> std::path::PathBuf {
    port_path.with_file_name(TOKEN_FILE)
}

/// Build the v1 API router over the given [`AppState`].
///
/// Exposed so integration tests (and the future CLI) can drive routes either
/// through the full [`serve`] over a temp socket or by serving this router
/// directly. An activity-tracking middleware updates the idle clock on every
/// request.
///
/// `tcp_secret` carries the per-daemon bearer secret when serving over loopback
/// TCP (KINTEG-010): when `Some`, every request except `GET /v1/health` must
/// present `Authorization: Bearer <secret>` or it is rejected `401`. When `None`
/// (the UDS path, where filesystem permissions are the per-user boundary), no
/// bearer check is applied. The `X-Kindling-Project` header is routing only and
/// is never treated as authorization.
pub fn build_router(state: AppState, tcp_secret: Option<Arc<str>>) -> Router {
    let activity = Arc::clone(state.activity());
    Router::new()
        .route("/v1/health", axum::routing::get(handlers::health))
        .route("/v1/capsules", post(handlers::open_capsule))
        .route(
            "/v1/capsules/open",
            axum::routing::get(handlers::get_open_capsule),
        )
        .route("/v1/capsules/{id}/close", patch(handlers::close_capsule))
        .route("/v1/observations", post(handlers::append_observation))
        .route("/v1/observations/list", post(handlers::list_observations))
        .route(
            "/v1/observations/{id}/forget",
            post(handlers::forget_observation),
        )
        .route("/v1/retrieve", post(handlers::retrieve))
        .route("/v1/pins", post(handlers::create_pin))
        .route("/v1/pins/{id}", delete(handlers::unpin))
        .route(
            "/v1/context/session-start",
            post(handlers::session_start_context),
        )
        .route(
            "/v1/context/pre-compact",
            post(handlers::pre_compact_context),
        )
        .layer(axum::middleware::from_fn(
            move |req, next: axum::middleware::Next| {
                let activity = Arc::clone(&activity);
                async move {
                    activity.enter();
                    let response = next.run(req).await;
                    activity.leave();
                    response
                }
            },
        ))
        // TCP bearer-auth gate (KINTEG-010). Added after the activity layer so
        // it runs OUTERMOST — an unauthenticated request is rejected before it
        // touches a handler. A no-op when `tcp_secret` is None (UDS path).
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let tcp_secret = tcp_secret.clone();
                async move {
                    if let Some(secret) = tcp_secret.as_deref() {
                        // Health is the only unauthenticated route: it carries no
                        // project data and is the client's contract-drift probe.
                        if req.uri().path() != "/v1/health" {
                            let presented = req
                                .headers()
                                .get(axum::http::header::AUTHORIZATION)
                                .and_then(|v| v.to_str().ok())
                                .and_then(|v| v.strip_prefix("Bearer "));
                            let ok = presented.is_some_and(|tok| {
                                constant_time_eq(tok.as_bytes(), secret.as_bytes())
                            });
                            if !ok {
                                return axum::http::StatusCode::UNAUTHORIZED.into_response();
                            }
                        }
                    }
                    next.run(req).await
                }
            },
        ))
        .with_state(state)
}

/// Run the daemon to completion: acquire the PID lock, bind the UDS at mode
/// `0600`, serve the v1 API, and shut down on idle — cleaning up the socket and
/// PID file on exit.
///
/// Resolves `Ok(())` on a clean idle shutdown, so callers (and tests) can wrap
/// it in a `tokio::time::timeout`.
pub async fn serve(config: ServerConfig) -> Result<(), ServerError> {
    let _pid_guard = acquire_pid_lock(&config.pid_path)?;
    let state = AppState::new(config.kindling_home.clone());

    // The loopback-TCP transport has no per-user boundary (any local process can
    // connect to 127.0.0.1:<port>), so it requires a per-daemon bearer secret;
    // the UDS path relies on filesystem permissions and needs none (KINTEG-010).
    let tcp_secret: Option<Arc<str>> = match config.transport {
        Transport::Tcp => Some(Arc::from(generate_tcp_secret().as_str())),
        #[cfg(unix)]
        Transport::Uds => None,
    };
    let app = build_router(state.clone(), tcp_secret.clone());

    match config.transport {
        #[cfg(unix)]
        Transport::Uds => {
            serve_on_uds(&config, app, state.activity().clone()).await?;
            // Best-effort socket cleanup; the PID guard removes the PID file on
            // drop.
            let _ = remove_socket(&config.socket_path);
        }
        Transport::Tcp => {
            let secret = tcp_secret.expect("TCP transport always has an auth secret");
            serve_on_tcp(&config, app, state.activity().clone(), &secret).await?;
        }
    }
    Ok(())
}

/// Idle-shutdown future: resolves once the daemon has been idle for
/// `idle_timeout`. Polled at a fraction of the timeout (min 25ms) so short
/// test timeouts still fire promptly.
async fn wait_until_idle(activity: Arc<state::Activity>, idle_timeout: Duration) {
    let poll = idle_timeout
        .checked_div(4)
        .unwrap_or(idle_timeout)
        .max(Duration::from_millis(25));
    loop {
        tokio::time::sleep(poll).await;
        if activity.is_idle_for(idle_timeout) {
            return;
        }
    }
}

#[cfg(unix)]
async fn serve_on_uds(
    config: &ServerConfig,
    app: Router,
    activity: Arc<state::Activity>,
) -> Result<(), ServerError> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::UnixListener;

    // A leftover socket from an unclean shutdown would make bind fail with
    // EADDRINUSE. Remove it first (the PID lock already guarantees no live
    // daemon is using it).
    let _ = remove_socket(&config.socket_path);
    if let Some(parent) = config.socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
            // Defence in depth: the socket is chmod'd to 0600 only after bind,
            // so for a brief window it carries the process umask. Lock the
            // containing directory to the owner (0700) so no other local user
            // can reach the socket during that window — filesystem permissions
            // are the daemon's only authn (per the design spec).
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }

    let listener = UnixListener::bind(&config.socket_path)?;
    // Restrict the socket to the owning user (0600) after bind, before serving.
    std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o600))?;

    let idle_timeout = config.idle_timeout;
    axum::serve(listener, app)
        .with_graceful_shutdown(wait_until_idle(activity, idle_timeout))
        .await?;
    Ok(())
}

/// Serve over loopback TCP on an ephemeral `127.0.0.1` port.
///
/// Compiled on all platforms (it is the Windows default, and is exercised by
/// the Linux test suite). Binds `127.0.0.1:0`, reads back the OS-assigned port,
/// and publishes it as decimal text to [`ServerConfig::port_path`] so the
/// client can discover where to connect — TCP has no filesystem rendezvous like
/// a UDS path. The port file is removed (best-effort) on shutdown.
async fn serve_on_tcp(
    config: &ServerConfig,
    app: Router,
    activity: Arc<state::Activity>,
    secret: &str,
) -> Result<(), ServerError> {
    use tokio::net::TcpListener;

    if let Some(parent) = config.port_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Publish the auth secret to an owner-only token file BEFORE binding, so by
    // the time a client can discover the port (written after bind) the token it
    // must present already exists. The client reads this same sibling file.
    let token_path = token_path_for(&config.port_path);
    write_owner_only(&token_path, secret.as_bytes())?;

    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    std::fs::write(&config.port_path, port.to_string())?;

    let idle_timeout = config.idle_timeout;
    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(wait_until_idle(activity, idle_timeout))
        .await;

    // Best-effort cleanup of both side-channel files; mirrors the UDS socket
    // cleanup.
    let _ = remove_socket(&config.port_path);
    let _ = remove_socket(&token_path);
    serve_result?;
    Ok(())
}

/// Write `bytes` to `path`, restricted to the owning user. On Unix the file is
/// created (or truncated) and chmod'd to `0600`; on other platforms it inherits
/// the default ACLs of the per-user `~/.kindling` directory (best effort — the
/// secret, not just the port, is what gates access).
fn write_owner_only(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn remove_socket(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
