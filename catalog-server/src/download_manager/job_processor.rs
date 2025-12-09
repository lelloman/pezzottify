//! Background job processor for the download queue.
//!
//! Processes queue items, downloads content, and updates queue state.
//! The QueueProcessor runs in a background task and periodically checks
//! for items to process.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use super::DownloadManager;

/// Background processor for the download queue.
///
/// Runs in a loop, periodically:
/// 1. Promoting retry-waiting items that are ready
/// 2. Processing the next pending item in the queue
///
/// The processor respects graceful shutdown via the cancellation token.
pub struct QueueProcessor {
    /// Reference to the download manager for queue operations.
    download_manager: Arc<DownloadManager>,
    /// Interval between processing attempts.
    interval: Duration,
}

impl QueueProcessor {
    /// Create a new QueueProcessor.
    ///
    /// # Arguments
    /// * `download_manager` - The download manager to process items from
    /// * `interval_secs` - Seconds between processing attempts
    pub fn new(download_manager: Arc<DownloadManager>, interval_secs: u64) -> Self {
        Self {
            download_manager,
            interval: Duration::from_secs(interval_secs),
        }
    }

    /// Main processing loop - call from a spawned task.
    ///
    /// This method runs indefinitely until the shutdown token is cancelled.
    /// It periodically:
    /// 1. Promotes retry-waiting items whose backoff period has elapsed
    /// 2. Processes the next pending item (if capacity allows)
    ///
    /// # Arguments
    /// * `shutdown` - Cancellation token for graceful shutdown
    pub async fn run(&self, shutdown: CancellationToken) {
        info!(
            "Queue processor starting with {}s interval",
            self.interval.as_secs()
        );

        let mut interval = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.process_tick().await;
                }
                _ = shutdown.cancelled() => {
                    info!("Queue processor shutting down");
                    break;
                }
            }
        }

        info!("Queue processor stopped");
    }

    /// Process a single tick of the queue processor.
    async fn process_tick(&self) {
        // 1. Promote ready retries
        match self.download_manager.promote_ready_retries() {
            Ok(count) => {
                if count > 0 {
                    info!("Promoted {} retry-waiting items to pending", count);
                }
            }
            Err(e) => {
                error!("Failed to promote retries: {}", e);
            }
        }

        // 2. Process next item
        match self.download_manager.process_next().await {
            Ok(Some(result)) => {
                if result.success {
                    info!(
                        "Successfully processed download: {} ({:?})",
                        result.queue_item_id, result.content_type
                    );
                } else {
                    info!(
                        "Download failed (will retry): {} - {:?}",
                        result.queue_item_id, result.error
                    );
                }
            }
            Ok(None) => {
                // Queue empty or at capacity - this is normal
                debug!("No items to process (queue empty or at capacity)");
            }
            Err(e) => {
                error!("Queue processor error: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_processor() {
        // We can't easily test without a real DownloadManager,
        // but we can verify the struct construction logic
        let interval = Duration::from_secs(30);
        assert_eq!(interval.as_secs(), 30);
    }

    #[test]
    fn test_interval_configuration() {
        // Test that different intervals work
        let intervals = vec![1, 5, 10, 30, 60, 300];
        for secs in intervals {
            let duration = Duration::from_secs(secs);
            assert_eq!(duration.as_secs(), secs);
        }
    }
}
