//! Socket-path cleanup must not destroy a regular file that occupies the
//! daemon's socket path, either at startup or because the path was replaced
//! after bind.

use std::os::unix::fs::FileTypeExt;
use std::time::Duration;

use kindling_server::{serve, ServerConfig, ServerError, Transport};

fn config(home: &std::path::Path, idle: Duration) -> ServerConfig {
    ServerConfig {
        socket_path: home.join("k.sock"),
        kindling_home: home.to_path_buf(),
        pid_path: home.join("k.pid"),
        port_path: home.join("k.port"),
        idle_timeout: idle,
        transport: Transport::Uds,
    }
}

#[tokio::test]
async fn startup_leaves_a_regular_file_at_the_socket_path() {
    let home = tempfile::tempdir().unwrap();
    let cfg = config(home.path(), Duration::from_millis(200));
    let marker = b"not-a-socket";
    std::fs::write(&cfg.socket_path, marker).unwrap();

    let err = serve(cfg.clone())
        .await
        .expect_err("bind should fail when a regular file occupies the socket path");
    assert!(matches!(err, ServerError::Io(_)));
    assert_eq!(std::fs::read(&cfg.socket_path).unwrap(), marker);
    assert!(cfg
        .socket_path
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_file());
}

#[tokio::test]
async fn shutdown_keeps_a_regular_file_that_replaced_the_socket() {
    let home = tempfile::tempdir().unwrap();
    let cfg = config(home.path(), Duration::from_millis(800));
    let serve_cfg = cfg.clone();
    let handle = tokio::spawn(async move { serve(serve_cfg).await });
    for _ in 0..200 {
        if cfg
            .socket_path
            .symlink_metadata()
            .map(|meta| meta.file_type().is_socket())
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        cfg.socket_path
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_socket(),
        "daemon should have bound a socket before the path is replaced"
    );
    assert!(
        cfg.pid_path.exists(),
        "daemon exited before the socket path was replaced"
    );

    std::fs::remove_file(&cfg.socket_path).unwrap();
    let marker = b"replaced-socket-path";
    std::fs::write(&cfg.socket_path, marker).unwrap();
    assert!(
        cfg.pid_path.exists(),
        "daemon exited before shutdown cleanup"
    );

    let joined = tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .expect("daemon should shut down on idle");
    assert!(joined.expect("serve task").is_ok());
    // The temp directory is still alive, so a missing file means cleanup unlinked it.
    assert_eq!(std::fs::read(&cfg.socket_path).unwrap(), marker);
}

#[tokio::test]
async fn shutdown_removes_the_bound_socket() {
    let home = tempfile::tempdir().unwrap();
    let cfg = config(home.path(), Duration::from_millis(200));
    let serve_cfg = cfg.clone();
    let handle = tokio::spawn(async move { serve(serve_cfg).await });
    let joined = tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .expect("daemon should shut down on idle");
    assert!(joined.expect("serve task").is_ok());
    assert!(
        home.path().exists(),
        "the home directory should still exist when the socket is checked"
    );
    assert!(
        !cfg.socket_path.exists(),
        "daemon should unlink the socket it bound"
    );
}

#[tokio::test]
async fn startup_replaces_a_stale_socket() {
    let home = tempfile::tempdir().unwrap();
    let cfg = config(home.path(), Duration::from_millis(400));
    {
        let _listener = tokio::net::UnixListener::bind(&cfg.socket_path).unwrap();
    }
    assert!(cfg
        .socket_path
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_socket());

    let serve_cfg = cfg.clone();
    let handle = tokio::spawn(async move { serve(serve_cfg).await });
    for _ in 0..200 {
        if cfg.pid_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        cfg.pid_path.exists(),
        "daemon should bind over a stale socket"
    );
    assert!(cfg
        .socket_path
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_socket());

    let joined = tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .expect("idle shutdown");
    assert!(joined.expect("serve task").is_ok());
    assert!(!cfg.socket_path.exists());
}
