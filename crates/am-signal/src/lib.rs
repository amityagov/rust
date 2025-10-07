use async_channel::{Receiver, Sender};
use tracing::info;

pub type ShutdownReceiver = Receiver<()>;

pub fn shutdown_signal() -> (Sender<()>, Receiver<()>) {
    async_channel::unbounded::<()>()
}

pub async fn wait_shutdown_signal(rx: Receiver<()>) {
    _ = rx.recv().await;
}

pub async fn subscribe_shutdown_signal(tx: Sender<()>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown initiated");

    drop(tx);
}
