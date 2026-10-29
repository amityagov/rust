pub mod auth;
pub mod error;

use std::time::Duration;

use axum::Router;
use tokio::net::{TcpListener, ToSocketAddrs};

#[cfg(feature = "tls-server")]
pub mod tls;

/// How long a graceful shutdown waits for open connections before the server stops anyway.
pub const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(30);

/// Binds `addr`, serves `router` and, once `signal` fires, stops accepting connections and waits up
/// to [`DEFAULT_SHUTDOWN_GRACE`] for open ones to finish.
pub async fn run_server<T: ToSocketAddrs + Send + Sync + std::fmt::Debug + 'static>(
    name: &'static str,
    router: Router,
    addr: T,
    signal: signal::ShutdownReceiver,
) -> anyhow::Result<()> {
    run_server_with_grace(name, router, addr, signal, DEFAULT_SHUTDOWN_GRACE).await
}

/// Like [`run_server`], with an explicit `grace` period.
///
/// A graceful shutdown alone waits for every connection to close, so one stuck request or idle
/// keep-alive connection would keep the process alive indefinitely. After `grace` the server stops
/// and drops what is still open.
pub async fn run_server_with_grace<T: ToSocketAddrs + Send + Sync + std::fmt::Debug + 'static>(
    name: &'static str,
    router: Router,
    addr: T,
    signal: signal::ShutdownReceiver,
    grace: Duration,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("Server {name} listening on {:?}", addr);

    let deadline_signal = signal.clone();
    let serve =
        axum::serve(listener, router).with_graceful_shutdown(signal::wait_shutdown_signal(signal));
    let deadline = async move {
        signal::wait_shutdown_signal(deadline_signal).await;
        tokio::time::sleep(grace).await;
    };

    tokio::select! {
        result = serve => result?,
        _ = deadline => tracing::warn!(
            "Server {name} still has open connections {grace:?} after shutdown began; stopping anyway"
        ),
    }

    Ok(())
}

/// Serves `router` over mutual TLS. Shutdown waits up to [`DEFAULT_SHUTDOWN_GRACE`] for open
/// connections, then closes them.
#[cfg(feature = "tls-server")]
pub async fn run_tls_server(
    name: &'static str,
    router: Router,
    addr: std::net::SocketAddr,
    config: tls::RustlsConfig,
    signal: signal::ShutdownReceiver,
) -> anyhow::Result<()> {
    tracing::info!("Server {name} listening with mTLS on {addr:?}");

    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();

    tokio::spawn(async move {
        signal::wait_shutdown_signal(signal).await;
        tracing::info!("Shutdown signal received");
        shutdown_handle.graceful_shutdown(Some(DEFAULT_SHUTDOWN_GRACE));
    });

    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn shutdown_does_not_wait_for_a_stuck_request() {
        // Reserve a free port, then hand it to the server.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let router = Router::new().route(
            "/",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                "late"
            }),
        );
        let (tx, rx) = signal::shutdown_signal();
        let server = tokio::spawn(run_server_with_grace(
            "test",
            router,
            ("127.0.0.1", port),
            rx,
            Duration::from_millis(200),
        ));
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut stuck = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stuck
            .write_all(b"GET / HTTP/1.1\r\nHost: test\r\n\r\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        drop(tx);
        let finished = tokio::time::timeout(Duration::from_secs(5), server).await;

        assert!(
            finished.is_ok(),
            "the server must stop after the grace period"
        );
    }
}
