pub mod auth;
pub mod error;

use axum::Router;
use tokio::net::{TcpListener, ToSocketAddrs};

#[cfg(feature = "tls-server")]
pub mod tls;

pub async fn run_server<T: ToSocketAddrs + Send + Sync + std::fmt::Debug + 'static>(
    name: &'static str,
    router: Router,
    addr: T,
    signal: signal::ShutdownReceiver,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("Server {name} listening on {:?}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(signal::wait_shutdown_signal(signal))
        .await?;

    Ok(())
}

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
        shutdown_handle.graceful_shutdown(None);
    });

    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}
