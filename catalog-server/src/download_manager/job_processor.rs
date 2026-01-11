//! Background processor for download queue via Quentin Torrentino WebSocket.
//!
//! Maintains a persistent WebSocket connection to QT and dispatches events
//! to the DownloadManager. Handles reconnection on disconnects.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::torrent_client::TorrentClient;
use super::DownloadManager;

/// Background processor that connects to Quentin Torrentino via WebSocket.
///
/// Runs in a loop:
/// 1. Connect to QT WebSocket
/// 2. On connect: submit any pending tickets
/// 3. Listen for events and dispatch to DownloadManager
/// 4. On disconnect: wait and reconnect
pub struct QueueProcessor {
    /// Reference to the download manager for queue operations.
    download_manager: Arc<DownloadManager>,
    /// Reference to the torrent client.
    torrent_client: Arc<TorrentClient>,
    /// Delay between reconnection attempts.
    reconnect_delay: Duration,
}

impl QueueProcessor {
    /// Create a new QueueProcessor.
    pub fn new(
        download_manager: Arc<DownloadManager>,
        torrent_client: Arc<TorrentClient>,
        reconnect_delay_secs: u64,
    ) -> Self {
        Self {
            download_manager,
            torrent_client,
            reconnect_delay: Duration::from_secs(reconnect_delay_secs),
        }
    }

    /// Main processing loop - call from a spawned task.
    ///
    /// Maintains persistent WebSocket connection to QT with automatic
    /// reconnection on disconnect.
    pub async fn run(&self, shutdown: CancellationToken) {
        info!(
            "Queue processor starting (reconnect_delay={}s)",
            self.reconnect_delay.as_secs()
        );

        loop {
            tokio::select! {
                result = self.run_ws_loop() => {
                    if let Err(e) = result {
                        warn!(
                            "WebSocket disconnected: {}, reconnecting in {}s",
                            e, self.reconnect_delay.as_secs()
                        );
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("Queue processor shutting down");
                    break;
                }
            }

            // Wait before reconnecting
            tokio::select! {
                _ = tokio::time::sleep(self.reconnect_delay) => {}
                _ = shutdown.cancelled() => {
                    info!("Queue processor shutting down during reconnect wait");
                    break;
                }
            }
        }

        info!("Queue processor stopped");
    }

    /// Run the WebSocket event loop.
    ///
    /// Connects to QT, submits pending tickets, then processes events
    /// until the connection closes.
    async fn run_ws_loop(&self) -> anyhow::Result<()> {
        // Subscribe to events before connecting
        let mut event_rx = self.torrent_client.subscribe();

        // Connect WebSocket (runs until disconnected)
        let mut ws_task = tokio::spawn({
            let client = self.torrent_client.clone();
            async move { client.run_websocket().await }
        });

        // Wait a moment for connection to establish
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Submit any pending tickets
        if self.torrent_client.is_connected() {
            match self.download_manager.submit_pending_tickets().await {
                Ok(count) => {
                    if count > 0 {
                        info!("Submitted {} pending tickets to QT", count);
                    }
                }
                Err(e) => {
                    error!("Failed to submit pending tickets: {}", e);
                }
            }
        }

        // Process events until disconnected
        loop {
            tokio::select! {
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            debug!("Processing event: {:?}", event);
                            if let Err(e) = self.download_manager.handle_ticket_event(event).await {
                                error!("Error handling ticket event: {}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Event receiver lagged by {} messages", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            debug!("Event channel closed");
                            break;
                        }
                    }
                }
                ws_result = &mut ws_task => {
                    return ws_result?.map_err(Into::into);
                }
            }
        }

        // Wait for WebSocket task to finish
        ws_task.await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_delay() {
        let delay = Duration::from_secs(5);
        assert_eq!(delay.as_secs(), 5);
    }
}
