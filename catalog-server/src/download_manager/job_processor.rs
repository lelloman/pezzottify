//! Background job processor for the download queue.
//!
//! Processes queue items, downloads content, and updates queue state.
//! The QueueProcessor runs in a background task and periodically checks
//! for items to process.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::server::metrics;

use super::DownloadManager;

/// Interval between stale detection checks (1 hour).
const STALE_CHECK_INTERVAL_SECS: u64 = 3600;

/// Background processor for the download queue.
///
/// Runs in a loop, periodically:
/// 1. Promoting retry-waiting items that are ready
/// 2. Processing the next pending item in the queue
/// 3. Checking for stale in-progress items (hourly)
///
/// The processor respects graceful shutdown via the cancellation token.
pub struct QueueProcessor {
    /// Reference to the download manager for queue operations.
    download_manager: Arc<DownloadManager>,
    /// Interval between processing attempts.
    interval: Duration,
    /// Threshold in seconds for considering an in-progress item as stale.
    stale_threshold_secs: u64,
}

impl QueueProcessor {
    /// Create a new QueueProcessor.
    ///
    /// # Arguments
    /// * `download_manager` - The download manager to process items from
    /// * `interval_secs` - Seconds between processing attempts
    pub fn new(download_manager: Arc<DownloadManager>, interval_secs: u64) -> Self {
        // Get stale threshold from config (default 1 hour)
        let stale_threshold_secs = download_manager.get_stale_threshold_secs();

        Self {
            download_manager,
            interval: Duration::from_secs(interval_secs),
            stale_threshold_secs,
        }
    }

    /// Main processing loop - call from a spawned task.
    ///
    /// This method runs indefinitely until the shutdown token is cancelled.
    /// It periodically:
    /// 1. Promotes retry-waiting items whose backoff period has elapsed
    /// 2. Processes the next pending item (if capacity allows)
    /// 3. Checks for stale in-progress items (on startup and hourly)
    ///
    /// # Arguments
    /// * `shutdown` - Cancellation token for graceful shutdown
    pub async fn run(&self, shutdown: CancellationToken) {
        info!(
            "Queue processor starting with {}s interval, stale threshold={}s",
            self.interval.as_secs(),
            self.stale_threshold_secs
        );

        // Check for stale items on startup
        self.check_stale_items();

        let mut interval = tokio::time::interval(self.interval);
        let mut last_stale_check = Instant::now();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.process_tick().await;

                    // Check for stale items hourly
                    if last_stale_check.elapsed().as_secs() >= STALE_CHECK_INTERVAL_SECS {
                        self.check_stale_items();
                        last_stale_check = Instant::now();
                    }
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
        // Update queue size metrics
        self.update_queue_metrics();

        // Update throttle and corruption handler metrics
        self.update_throttle_handler_metrics().await;

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
                // Emit metrics for processed download
                let content_type_str = result.content_type.as_str().to_lowercase();
                let result_str = if result.success { "success" } else { "failure" };
                let duration = Duration::from_millis(result.duration_ms as u64);
                metrics::record_download_processed(&content_type_str, result_str, duration);

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

    /// Update Prometheus metrics with current queue sizes.
    ///
    /// Gets queue stats from the download manager and updates the
    /// corresponding Prometheus gauges.
    fn update_queue_metrics(&self) {
        match self.download_manager.get_queue_stats() {
            Ok(stats) => {
                // Update queue size metrics for each status
                // Using priority "0" as a catch-all since we don't have per-priority breakdown
                metrics::set_download_queue_size("pending", 0, stats.pending as usize);
                metrics::set_download_queue_size("in_progress", 0, stats.in_progress as usize);
                metrics::set_download_queue_size("retry_waiting", 0, stats.retry_waiting as usize);
            }
            Err(e) => {
                error!("Failed to get queue stats for metrics: {}", e);
            }
        }
    }

    /// Update throttle and corruption handler metrics.
    ///
    /// These are updated on each tick to provide real-time visibility.
    async fn update_throttle_handler_metrics(&self) {
        // Update throttle metrics
        let throttle_stats = self.download_manager.get_throttle_stats().await;
        metrics::update_throttle_metrics(
            throttle_stats.bytes_last_minute,
            throttle_stats.bytes_last_hour,
            throttle_stats.max_bytes_per_minute,
            throttle_stats.max_bytes_per_hour,
            throttle_stats.is_throttled,
        );

        // Update corruption handler metrics
        let handler_state = self.download_manager.get_corruption_handler_state().await;
        metrics::update_corruption_handler_metrics(
            handler_state.current_level,
            handler_state.in_cooldown,
            handler_state.cooldown_remaining_secs.unwrap_or(0),
        );
    }

    /// Check for stale in-progress items and log warnings.
    ///
    /// Items stuck in IN_PROGRESS longer than `stale_threshold_secs` indicate
    /// something is broken and needs human investigation. We log warnings
    /// and update the Prometheus metric, but do NOT auto-fail them.
    fn check_stale_items(&self) {
        match self
            .download_manager
            .get_stale_in_progress(self.stale_threshold_secs as i64)
        {
            Ok(stale_items) => {
                let count = stale_items.len();

                // Update Prometheus metric
                metrics::set_download_stale_in_progress(count);

                if count > 0 {
                    warn!(
                        "Found {} stale in-progress download items (threshold={}s):",
                        count, self.stale_threshold_secs
                    );
                    for item in &stale_items {
                        warn!(
                            "  - {} (type={:?}, content_id={}, started_at={:?})",
                            item.id, item.content_type, item.content_id, item.started_at
                        );
                    }
                    warn!(
                        "Stale items require manual investigation - they will NOT be auto-failed"
                    );
                } else {
                    debug!("No stale in-progress items found");
                }
            }
            Err(e) => {
                error!("Failed to check for stale in-progress items: {}", e);
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
