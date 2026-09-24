//! MCP Rate Limiting
//!
//! Per-user rate limiting for MCP tool calls.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use simple_server::rate_limit::{WindowBoundary, WindowCounters};

use super::registry::ToolCategory;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub read_per_minute: u32,
    pub write_per_minute: u32,
    pub sql_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            read_per_minute: 120,
            write_per_minute: 30,
            sql_per_minute: 10,
        }
    }
}

/// Tracks rate limit state for a single user
#[derive(Debug)]
struct UserRateLimitState {
    window: WindowCounters<3>,
}

impl UserRateLimitState {
    fn new(anchor: Duration) -> Self {
        Self {
            window: WindowCounters::new(Duration::from_secs(60), WindowBoundary::After, anchor),
        }
    }
}

/// Rate limiter for MCP requests
pub struct McpRateLimiter {
    config: RateLimitConfig,
    origin: Instant,
    states: Mutex<HashMap<usize, UserRateLimitState>>,
}

impl McpRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            origin: Instant::now(),
            states: Mutex::new(HashMap::new()),
        }
    }

    /// Check if a request is allowed and record it if so
    /// Returns Ok(()) if allowed, Err(retry_after_secs) if rate limited
    pub fn check_and_record(&self, user_id: usize, category: ToolCategory) -> Result<(), u32> {
        let mut states = self.states.lock().unwrap();
        let now = self.origin.elapsed();
        let state = states
            .entry(user_id)
            .or_insert_with(|| UserRateLimitState::new(now));

        let (index, limit) = match category {
            ToolCategory::Read => (0, self.config.read_per_minute),
            ToolCategory::Write => (1, self.config.write_per_minute),
            ToolCategory::Sql => (2, self.config.sql_per_minute),
        };
        state
            .window
            .admit_at(now, index, u64::from(limit), 1)
            .map_err(|denial| {
                // Legacy code subtracts the floored elapsed whole seconds,
                // which rounds the remaining duration up before applying 1s.
                let retry = denial.retry_after;
                retry
                    .as_secs()
                    .saturating_add(u64::from(retry.subsec_nanos() != 0))
                    .max(1) as u32
            })
    }

    /// Get current usage for a user (for debugging/metrics)
    pub fn get_usage(&self, user_id: usize) -> Option<(u32, u32, u32)> {
        let states = self.states.lock().unwrap();
        states.get(&user_id).map(|s| {
            let counts = s.window.counts();
            (counts[0] as u32, counts[1] as u32, counts[2] as u32)
        })
    }

    /// Clean up old entries (call periodically)
    pub fn cleanup_stale_entries(&self) {
        let mut states = self.states.lock().unwrap();
        let threshold = Duration::from_secs(300); // 5 minutes
        let now = self.origin.elapsed();
        states.retain(|_, state| now.saturating_sub(state.window.anchor()) < threshold);
    }
}

impl Default for McpRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allows_under_limit() {
        let limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 10,
            write_per_minute: 5,
            sql_per_minute: 3,
        });

        // Should allow up to limit
        for _ in 0..10 {
            assert!(limiter.check_and_record(1, ToolCategory::Read).is_ok());
        }
    }

    #[test]
    fn test_rate_limit_blocks_over_limit() {
        let limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 5,
            write_per_minute: 3,
            sql_per_minute: 2,
        });

        // Use up the limit
        for _ in 0..5 {
            assert!(limiter.check_and_record(1, ToolCategory::Read).is_ok());
        }

        // Should be blocked
        let result = limiter.check_and_record(1, ToolCategory::Read);
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_limit_separate_categories() {
        let limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 3,
            write_per_minute: 2,
            sql_per_minute: 1,
        });

        // Each category has its own limit
        for _ in 0..3 {
            assert!(limiter.check_and_record(1, ToolCategory::Read).is_ok());
        }
        for _ in 0..2 {
            assert!(limiter.check_and_record(1, ToolCategory::Write).is_ok());
        }
        assert!(limiter.check_and_record(1, ToolCategory::Sql).is_ok());

        // All should now be at limit
        assert!(limiter.check_and_record(1, ToolCategory::Read).is_err());
        assert!(limiter.check_and_record(1, ToolCategory::Write).is_err());
        assert!(limiter.check_and_record(1, ToolCategory::Sql).is_err());
    }

    #[test]
    fn test_rate_limit_separate_users() {
        let limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 2,
            write_per_minute: 2,
            sql_per_minute: 2,
        });

        // User 1 uses their limit
        for _ in 0..2 {
            assert!(limiter.check_and_record(1, ToolCategory::Read).is_ok());
        }
        assert!(limiter.check_and_record(1, ToolCategory::Read).is_err());

        // User 2 should still have their own limit
        for _ in 0..2 {
            assert!(limiter.check_and_record(2, ToolCategory::Read).is_ok());
        }
    }

    #[test]
    fn test_get_usage() {
        let limiter = McpRateLimiter::new(RateLimitConfig::default());

        // No usage initially
        assert!(limiter.get_usage(1).is_none());

        // Record some usage
        limiter.check_and_record(1, ToolCategory::Read).unwrap();
        limiter.check_and_record(1, ToolCategory::Read).unwrap();
        limiter.check_and_record(1, ToolCategory::Write).unwrap();

        let usage = limiter.get_usage(1).unwrap();
        assert_eq!(usage, (2, 1, 0));
    }

    #[test]
    fn zero_limit_rejects_without_recording_and_keeps_shared_anchor() {
        let limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 0,
            write_per_minute: 1,
            sql_per_minute: 1,
        });
        assert_eq!(limiter.check_and_record(7, ToolCategory::Read), Err(60));
        assert_eq!(limiter.get_usage(7), Some((0, 0, 0)));
        assert_eq!(limiter.check_and_record(7, ToolCategory::Write), Ok(()));
        assert_eq!(limiter.get_usage(7), Some((0, 1, 0)));
    }

    #[test]
    fn retry_rounds_up_remaining_window_like_legacy_elapsed_seconds() {
        let mut limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 0,
            write_per_minute: 1,
            sql_per_minute: 1,
        });
        assert_eq!(limiter.check_and_record(3, ToolCategory::Read), Err(60));
        limiter.origin = Instant::now() - Duration::from_millis(500);
        assert_eq!(limiter.check_and_record(3, ToolCategory::Read), Err(60));
        limiter.origin = Instant::now() - Duration::from_millis(1500);
        assert_eq!(limiter.check_and_record(3, ToolCategory::Read), Err(59));
    }

    #[test]
    fn expired_window_resets_all_categories_together() {
        let mut limiter = McpRateLimiter::new(RateLimitConfig {
            read_per_minute: 1,
            write_per_minute: 1,
            sql_per_minute: 1,
        });
        limiter.check_and_record(9, ToolCategory::Read).unwrap();
        limiter.check_and_record(9, ToolCategory::Write).unwrap();
        limiter.origin = Instant::now() - Duration::from_secs(61);
        assert_eq!(limiter.get_usage(9), Some((1, 1, 0)));
        assert_eq!(limiter.check_and_record(9, ToolCategory::Sql), Ok(()));
        assert_eq!(limiter.get_usage(9), Some((0, 0, 1)));
    }

    #[test]
    fn cleanup_uses_window_anchor_without_touching_recent_user() {
        let mut limiter = McpRateLimiter::default();
        limiter.check_and_record(1, ToolCategory::Read).unwrap();
        limiter.origin = Instant::now() - Duration::from_secs(301);
        limiter.check_and_record(2, ToolCategory::Read).unwrap();
        limiter.cleanup_stale_entries();
        assert_eq!(limiter.get_usage(1), None);
        assert_eq!(limiter.get_usage(2), Some((1, 0, 0)));
    }
}
