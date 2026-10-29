//! Graceful shutdown plumbing.
//!
//! [`shutdown_signal`] creates a latch. Whoever owns the [`Sender`] calls
//! [`subscribe_shutdown_signal`], which returns once the process receives Ctrl+C or SIGTERM and
//! then drops the sender. Every holder of a [`ShutdownReceiver`] observes that through
//! [`wait_shutdown_signal`], so a clone of the receiver can be handed to each task that must stop.

use async_channel::{Receiver, Sender};
use tracing::info;

/// Resolves, through [`wait_shutdown_signal`], once shutdown has been requested.
pub type ShutdownReceiver = Receiver<()>;

/// Creates the shutdown latch: the sender is closed to request shutdown.
pub fn shutdown_signal() -> (Sender<()>, ShutdownReceiver) {
    async_channel::unbounded()
}

/// Waits until the matching sender is dropped.
pub async fn wait_shutdown_signal(rx: ShutdownReceiver) {
    // The channel never carries a value; `recv` fails exactly when the sender is gone.
    _ = rx.recv().await;
}

/// Waits for Ctrl+C or SIGTERM (Ctrl+C only off Unix), then drops `tx` to request shutdown.
///
/// # Panics
///
/// Panics if the operating system refuses to install a signal handler.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dropping_the_sender_releases_every_waiter() {
        let (tx, rx) = shutdown_signal();
        let first = tokio::spawn(wait_shutdown_signal(rx.clone()));
        let second = tokio::spawn(wait_shutdown_signal(rx));

        drop(tx);

        first.await.unwrap();
        second.await.unwrap();
    }
}
