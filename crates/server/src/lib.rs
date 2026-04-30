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

    let shutdown_signal = signal::wait_shutdown_signal(signal);

    tokio::select! {
        result = axum_server::bind_rustls(addr, config).serve(router.into_make_service()) => {
            result?;
        }
        _ = shutdown_signal => {
            tracing::info!("Shutdown signal received");
        }
    }

    Ok(())
}
