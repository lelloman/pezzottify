//! Bandwidth throttling for download operations.
//!
//! Provides rate limiting to prevent overwhelming the downloader service.

use async_trait::async_trait;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Trait for download bandwidth throttling.
#[async_trait]
pub trait DownloadThrottler: Send + Sync {
    /// Check if we can download right now based on historical usage.
    /// Returns Ok(()) if allowed, Err(wait_duration) if throttled.
    async fn check_bandwidth(&self) -> Result<(), Duration>;

    /// Record that we downloaded `bytes`. Called after successful download.
    async fn record_download(&self, bytes: u64);

    /// Reset state (for testing or manual override).
    async fn reset(&self);

    /// Get current usage stats for monitoring.
    async fn get_stats(&self) -> ThrottleStats;
}

/// Current throttle statistics.
#[derive(Debug, Clone, Default)]
pub struct ThrottleStats {
    /// Bytes downloaded in the last minute
    pub bytes_last_minute: u64,
    /// Bytes downloaded in the last hour
    pub bytes_last_hour: u64,
    /// Maximum bytes per minute
    pub max_bytes_per_minute: u64,
    /// Maximum bytes per hour
    pub max_bytes_per_hour: u64,
    /// Whether currently throttled
    pub is_throttled: bool,
}

/// Configuration for the sliding window throttler.
#[derive(Debug, Clone)]
pub struct ThrottlerConfig {
    /// Maximum bytes allowed per minute
    pub max_bytes_per_minute: u64,
    /// Maximum bytes allowed per hour
    pub max_bytes_per_hour: u64,
    /// Whether throttling is enabled
    pub enabled: bool,
}

impl Default for ThrottlerConfig {
    fn default() -> Self {
        Self {
            max_bytes_per_minute: 20 * 1024 * 1024, // 20 MB/min
            max_bytes_per_hour: 1500 * 1024 * 1024, // 1500 MB/hour
            enabled: true,
        }
    }
}

/// Timestamped download record.
#[derive(Debug, Clone)]
struct DownloadRecord {
    timestamp: Instant,
    bytes: u64,
}

/// Sliding window bandwidth throttler.
///
/// Tracks bytes downloaded in sliding time windows and enforces
/// configurable per-minute and per-hour limits.
pub struct SlidingWindowThrottler {
    /// Timestamped download records
    downloads: Mutex<VecDeque<DownloadRecord>>,
    /// Configuration
    config: ThrottlerConfig,
}

impl SlidingWindowThrottler {
    /// Create a new throttler with the given configuration.
    pub fn new(config: ThrottlerConfig) -> Self {
        Self {
            downloads: Mutex::new(VecDeque::new()),
            config,
        }
    }

    /// Create a new throttler with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ThrottlerConfig::default())
    }

    /// Prune entries older than 1 hour.
    fn prune_old_entries(downloads: &mut VecDeque<DownloadRecord>, now: Instant) {
        let hour_ago = now - Duration::from_secs(3600);
        while let Some(front) = downloads.front() {
            if front.timestamp < hour_ago {
                downloads.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate bytes in a given time window.
    fn bytes_in_window(
        downloads: &VecDeque<DownloadRecord>,
        now: Instant,
        window: Duration,
    ) -> u64 {
        let cutoff = now - window;
        downloads
            .iter()
            .filter(|r| r.timestamp >= cutoff)
            .map(|r| r.bytes)
            .sum()
    }
}

#[async_trait]
impl DownloadThrottler for SlidingWindowThrottler {
    async fn check_bandwidth(&self) -> Result<(), Duration> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut downloads = self.downloads.lock().await;
        let now = Instant::now();

        // Prune old entries
        Self::prune_old_entries(&mut downloads, now);

        // Check per-minute limit
        let minute_window = Duration::from_secs(60);
        let bytes_last_minute = Self::bytes_in_window(&downloads, now, minute_window);
        if bytes_last_minute >= self.config.max_bytes_per_minute {
            // Find when the oldest record in the minute window will expire
            let minute_ago = now - minute_window;
            if let Some(oldest_in_minute) = downloads.iter().find(|r| r.timestamp >= minute_ago) {
                let wait = oldest_in_minute.timestamp + minute_window - now;
                return Err(wait);
            }
            return Err(Duration::from_secs(60));
        }

        // Check per-hour limit
        let hour_window = Duration::from_secs(3600);
        let bytes_last_hour = Self::bytes_in_window(&downloads, now, hour_window);
        if bytes_last_hour >= self.config.max_bytes_per_hour {
            // Find when the oldest record will expire
            if let Some(oldest) = downloads.front() {
                let wait = oldest.timestamp + hour_window - now;
                return Err(wait);
            }
            return Err(Duration::from_secs(3600));
        }

        Ok(())
    }

    async fn record_download(&self, bytes: u64) {
        let mut downloads = self.downloads.lock().await;
        let now = Instant::now();

        // Prune old entries first
        Self::prune_old_entries(&mut downloads, now);

        // Record new download
        downloads.push_back(DownloadRecord {
            timestamp: now,
            bytes,
        });
    }

    async fn reset(&self) {
        let mut downloads = self.downloads.lock().await;
        downloads.clear();
    }

    async fn get_stats(&self) -> ThrottleStats {
        let downloads = self.downloads.lock().await;
        let now = Instant::now();

        let bytes_last_minute = Self::bytes_in_window(&downloads, now, Duration::from_secs(60));
        let bytes_last_hour = Self::bytes_in_window(&downloads, now, Duration::from_secs(3600));

        let is_throttled = bytes_last_minute >= self.config.max_bytes_per_minute
            || bytes_last_hour >= self.config.max_bytes_per_hour;

        ThrottleStats {
            bytes_last_minute,
            bytes_last_hour,
            max_bytes_per_minute: self.config.max_bytes_per_minute,
            max_bytes_per_hour: self.config.max_bytes_per_hour,
            is_throttled,
        }
    }
}

/// No-op throttler that always allows downloads.
/// Used when throttling is disabled.
pub struct NoOpThrottler;

#[async_trait]
impl DownloadThrottler for NoOpThrottler {
    async fn check_bandwidth(&self) -> Result<(), Duration> {
        Ok(())
    }

    async fn record_download(&self, _bytes: u64) {}

    async fn reset(&self) {}

    async fn get_stats(&self) -> ThrottleStats {
        ThrottleStats::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_op_throttler_always_allows() {
        let throttler = NoOpThrottler;
        assert!(throttler.check_bandwidth().await.is_ok());
        throttler.record_download(1_000_000_000).await;
        assert!(throttler.check_bandwidth().await.is_ok());
    }

    #[tokio::test]
    async fn test_sliding_window_allows_under_limit() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 100 * 1024 * 1024,
            max_bytes_per_hour: 1000 * 1024 * 1024,
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Record some downloads under the limit
        throttler.record_download(10 * 1024 * 1024).await;
        throttler.record_download(10 * 1024 * 1024).await;

        // Should still be allowed
        assert!(throttler.check_bandwidth().await.is_ok());
    }

    #[tokio::test]
    async fn test_sliding_window_blocks_at_minute_limit() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 50 * 1024 * 1024, // 50 MB/min
            max_bytes_per_hour: 1000 * 1024 * 1024,
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Record downloads that hit the minute limit
        throttler.record_download(50 * 1024 * 1024).await;

        // Should be throttled
        let result = throttler.check_bandwidth().await;
        assert!(result.is_err());

        // Wait duration should be positive
        if let Err(wait) = result {
            assert!(wait > Duration::ZERO);
            assert!(wait <= Duration::from_secs(60));
        }
    }

    #[tokio::test]
    async fn test_sliding_window_blocks_at_hour_limit() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 1000 * 1024 * 1024, // High minute limit
            max_bytes_per_hour: 100 * 1024 * 1024,    // Low hour limit
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Record downloads that hit the hour limit
        throttler.record_download(100 * 1024 * 1024).await;

        // Should be throttled
        let result = throttler.check_bandwidth().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_disabled_throttler_always_allows() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 1, // Tiny limit
            max_bytes_per_hour: 1,
            enabled: false, // But disabled
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Record way over limit
        throttler.record_download(1_000_000_000).await;

        // Should still be allowed because disabled
        assert!(throttler.check_bandwidth().await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_clears_history() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 50 * 1024 * 1024,
            max_bytes_per_hour: 100 * 1024 * 1024,
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Fill up to limit
        throttler.record_download(100 * 1024 * 1024).await;
        assert!(throttler.check_bandwidth().await.is_err());

        // Reset
        throttler.reset().await;

        // Should be allowed now
        assert!(throttler.check_bandwidth().await.is_ok());
    }

    #[tokio::test]
    async fn test_get_stats() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 100 * 1024 * 1024,
            max_bytes_per_hour: 1000 * 1024 * 1024,
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Record some downloads
        throttler.record_download(10 * 1024 * 1024).await;
        throttler.record_download(20 * 1024 * 1024).await;

        let stats = throttler.get_stats().await;
        assert_eq!(stats.bytes_last_minute, 30 * 1024 * 1024);
        assert_eq!(stats.bytes_last_hour, 30 * 1024 * 1024);
        assert_eq!(stats.max_bytes_per_minute, 100 * 1024 * 1024);
        assert_eq!(stats.max_bytes_per_hour, 1000 * 1024 * 1024);
        assert!(!stats.is_throttled);
    }

    #[tokio::test]
    async fn test_stats_shows_throttled() {
        let config = ThrottlerConfig {
            max_bytes_per_minute: 50 * 1024 * 1024,
            max_bytes_per_hour: 1000 * 1024 * 1024,
            enabled: true,
        };
        let throttler = SlidingWindowThrottler::new(config);

        // Hit the limit
        throttler.record_download(50 * 1024 * 1024).await;

        let stats = throttler.get_stats().await;
        assert!(stats.is_throttled);
    }
}
