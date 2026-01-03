//! MCP Rate Limiting
//!
//! Per-user rate limiting for MCP tool calls.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    read_count: u32,
    write_count: u32,
    sql_count: u32,
    window_start: Instant,
}

impl UserRateLimitState {
    fn new() -> Self {
        Self {
            read_count: 0,
            write_count: 0,
            sql_count: 0,
            window_start: Instant::now(),
        }
    }

    fn reset_if_expired(&mut self) {
        if self.window_start.elapsed() > Duration::from_secs(60) {
            self.read_count = 0;
            self.write_count = 0;
            self.sql_count = 0;
            self.window_start = Instant::now();
        }
    }
}

/// Rate limiter for MCP requests
pub struct McpRateLimiter {
    config: RateLimitConfig,
    states: Mutex<HashMap<usize, UserRateLimitState>>,
}

impl McpRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            states: Mutex::new(HashMap::new()),
        }
    }

    /// Check if a request is allowed and record it if so
    /// Returns Ok(()) if allowed, Err(retry_after_secs) if rate limited
    pub fn check_and_record(
        &self,
        user_id: usize,
        category: ToolCategory,
    ) -> Result<(), u32> {
        let mut states = self.states.lock().unwrap();
        let state = states
            .entry(user_id)
            .or_insert_with(UserRateLimitState::new);

        // Reset window if expired
        state.reset_if_expired();

        // Check and increment based on category
        let (current, limit) = match category {
            ToolCategory::Read => (&mut state.read_count, self.config.read_per_minute),
            ToolCategory::Write => (&mut state.write_count, self.config.write_per_minute),
            ToolCategory::Sql => (&mut state.sql_count, self.config.sql_per_minute),
        };

        if *current >= limit {
            // Calculate retry-after based on window expiry
            let elapsed = state.window_start.elapsed().as_secs();
            let retry_after = 60u64.saturating_sub(elapsed) as u32;
            return Err(retry_after.max(1));
        }

        *current += 1;
        Ok(())
    }

    /// Get current usage for a user (for debugging/metrics)
    pub fn get_usage(&self, user_id: usize) -> Option<(u32, u32, u32)> {
        let states = self.states.lock().unwrap();
        states
            .get(&user_id)
            .map(|s| (s.read_count, s.write_count, s.sql_count))
    }

    /// Clean up old entries (call periodically)
    pub fn cleanup_stale_entries(&self) {
        let mut states = self.states.lock().unwrap();
        let threshold = Duration::from_secs(300); // 5 minutes
        states.retain(|_, state| state.window_start.elapsed() < threshold);
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
}
