//! Retry policy for failed downloads.
//!
//! Implements exponential backoff with configurable parameters.

use crate::config::DownloadManagerSettings;
use crate::download_manager::DownloadError;

/// Retry policy implementing exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries before permanent failure.
    pub max_retries: i32,
    /// Initial backoff duration in seconds.
    pub initial_backoff_secs: u64,
    /// Maximum backoff duration in seconds (cap for exponential growth).
    pub max_backoff_secs: u64,
    /// Multiplier applied to backoff after each retry.
    pub backoff_multiplier: f64,
}

impl RetryPolicy {
    /// Create a new RetryPolicy from configuration settings.
    pub fn new(config: &DownloadManagerSettings) -> Self {
        Self {
            max_retries: config.max_retries as i32,
            initial_backoff_secs: config.initial_backoff_secs,
            max_backoff_secs: config.max_backoff_secs,
            backoff_multiplier: config.backoff_multiplier,
        }
    }

    /// Calculate the next retry timestamp based on current retry count.
    ///
    /// Uses exponential backoff: `initial_backoff * multiplier^retry_count`,
    /// capped at `max_backoff_secs`.
    ///
    /// Returns a Unix timestamp (seconds since epoch).
    pub fn next_retry_at(&self, retry_count: i32) -> i64 {
        let backoff = self.initial_backoff_secs as f64 * self.backoff_multiplier.powi(retry_count);
        let capped_backoff = backoff.min(self.max_backoff_secs as f64) as i64;
        chrono::Utc::now().timestamp() + capped_backoff
    }

    /// Check if an error should be retried given the current retry count.
    ///
    /// Returns true if:
    /// - The error type is retryable (e.g., not NotFound)
    /// - The retry count is less than max_retries
    pub fn should_retry(&self, error: &DownloadError, retry_count: i32) -> bool {
        error.is_retryable() && retry_count < self.max_retries
    }

    /// Calculate backoff duration in seconds for a given retry count.
    ///
    /// This is useful for testing or displaying estimated wait times.
    pub fn backoff_secs(&self, retry_count: i32) -> u64 {
        let backoff = self.initial_backoff_secs as f64 * self.backoff_multiplier.powi(retry_count);
        (backoff.min(self.max_backoff_secs as f64)) as u64
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 8,
            initial_backoff_secs: 60,
            max_backoff_secs: 86400, // 24 hours
            backoff_multiplier: 2.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download_manager::DownloadErrorType;

    fn make_default_config() -> DownloadManagerSettings {
        DownloadManagerSettings {
            enabled: true,
            max_albums_per_hour: 10,
            max_albums_per_day: 60,
            user_max_requests_per_day: 100,
            user_max_queue_size: 200,
            stale_in_progress_threshold_secs: 3600,
            max_retries: 8,
            initial_backoff_secs: 60,
            max_backoff_secs: 86400,
            backoff_multiplier: 2.5,
            audit_log_retention_days: 90,
        }
    }

    #[test]
    fn test_new_from_config() {
        let config = make_default_config();
        let policy = RetryPolicy::new(&config);

        assert_eq!(policy.max_retries, 8);
        assert_eq!(policy.initial_backoff_secs, 60);
        assert_eq!(policy.max_backoff_secs, 86400);
        assert_eq!(policy.backoff_multiplier, 2.5);
    }

    #[test]
    fn test_default() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.max_retries, 8);
        assert_eq!(policy.initial_backoff_secs, 60);
        assert_eq!(policy.max_backoff_secs, 86400);
        assert_eq!(policy.backoff_multiplier, 2.5);
    }

    #[test]
    fn test_backoff_calculation() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_secs: 60,
            max_backoff_secs: 3600,
            backoff_multiplier: 2.0,
        };

        // retry_count=0: 60 * 2^0 = 60
        assert_eq!(policy.backoff_secs(0), 60);

        // retry_count=1: 60 * 2^1 = 120
        assert_eq!(policy.backoff_secs(1), 120);

        // retry_count=2: 60 * 2^2 = 240
        assert_eq!(policy.backoff_secs(2), 240);

        // retry_count=3: 60 * 2^3 = 480
        assert_eq!(policy.backoff_secs(3), 480);

        // retry_count=4: 60 * 2^4 = 960
        assert_eq!(policy.backoff_secs(4), 960);
    }

    #[test]
    fn test_backoff_capping() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_backoff_secs: 60,
            max_backoff_secs: 300, // 5 minutes cap
            backoff_multiplier: 2.0,
        };

        // retry_count=2: 60 * 2^2 = 240 (under cap)
        assert_eq!(policy.backoff_secs(2), 240);

        // retry_count=3: 60 * 2^3 = 480 -> capped at 300
        assert_eq!(policy.backoff_secs(3), 300);

        // retry_count=5: 60 * 2^5 = 1920 -> capped at 300
        assert_eq!(policy.backoff_secs(5), 300);
    }

    #[test]
    fn test_next_retry_at() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_secs: 60,
            max_backoff_secs: 3600,
            backoff_multiplier: 2.0,
        };

        let now = chrono::Utc::now().timestamp();

        // retry_count=0: should be ~60 seconds from now
        let retry_at = policy.next_retry_at(0);
        assert!(retry_at >= now + 59 && retry_at <= now + 61);

        // retry_count=1: should be ~120 seconds from now
        let retry_at = policy.next_retry_at(1);
        assert!(retry_at >= now + 119 && retry_at <= now + 121);
    }

    #[test]
    fn test_should_retry_retryable_errors() {
        let policy = RetryPolicy::default();

        let connection_error =
            DownloadError::new(DownloadErrorType::Connection, "Connection refused");
        let timeout_error = DownloadError::new(DownloadErrorType::Timeout, "Request timed out");
        let parse_error = DownloadError::new(DownloadErrorType::Parse, "Invalid response format");
        let storage_error = DownloadError::new(DownloadErrorType::Storage, "Disk full");
        let unknown_error = DownloadError::new(DownloadErrorType::Unknown, "Unknown error");

        // All retryable errors should return true when under max_retries
        assert!(policy.should_retry(&connection_error, 0));
        assert!(policy.should_retry(&timeout_error, 1));
        assert!(policy.should_retry(&parse_error, 2));
        assert!(policy.should_retry(&storage_error, 3));
        assert!(policy.should_retry(&unknown_error, 4));
    }

    #[test]
    fn test_should_retry_not_found_never_retries() {
        let policy = RetryPolicy::default();

        let not_found_error = DownloadError::new(DownloadErrorType::NotFound, "Album not found");

        // NotFound errors should never be retried, regardless of retry count
        assert!(!policy.should_retry(&not_found_error, 0));
        assert!(!policy.should_retry(&not_found_error, 1));
        assert!(!policy.should_retry(&not_found_error, 2));
    }

    #[test]
    fn test_should_retry_max_retries_exceeded() {
        let policy = RetryPolicy {
            max_retries: 3,
            ..Default::default()
        };

        let connection_error =
            DownloadError::new(DownloadErrorType::Connection, "Connection refused");

        // Under max_retries: should retry
        assert!(policy.should_retry(&connection_error, 0));
        assert!(policy.should_retry(&connection_error, 1));
        assert!(policy.should_retry(&connection_error, 2));

        // At or above max_retries: should not retry
        assert!(!policy.should_retry(&connection_error, 3));
        assert!(!policy.should_retry(&connection_error, 4));
        assert!(!policy.should_retry(&connection_error, 10));
    }

    #[test]
    fn test_custom_backoff_multiplier() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_secs: 10,
            max_backoff_secs: 1000,
            backoff_multiplier: 3.0,
        };

        // 10 * 3^0 = 10
        assert_eq!(policy.backoff_secs(0), 10);

        // 10 * 3^1 = 30
        assert_eq!(policy.backoff_secs(1), 30);

        // 10 * 3^2 = 90
        assert_eq!(policy.backoff_secs(2), 90);

        // 10 * 3^3 = 270
        assert_eq!(policy.backoff_secs(3), 270);
    }

    #[test]
    fn test_zero_initial_backoff() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_secs: 0,
            max_backoff_secs: 100,
            backoff_multiplier: 2.0,
        };

        // 0 * anything = 0
        assert_eq!(policy.backoff_secs(0), 0);
        assert_eq!(policy.backoff_secs(5), 0);
    }

    #[test]
    fn test_multiplier_of_one() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_secs: 100,
            max_backoff_secs: 1000,
            backoff_multiplier: 1.0,
        };

        // 100 * 1^n = 100 for all n
        assert_eq!(policy.backoff_secs(0), 100);
        assert_eq!(policy.backoff_secs(5), 100);
        assert_eq!(policy.backoff_secs(10), 100);
    }
}
