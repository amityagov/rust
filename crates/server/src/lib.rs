pub mod auth;
pub mod error;

use axum::Router;
use tokio::net::{TcpListener, ToSocketAddrs};

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
