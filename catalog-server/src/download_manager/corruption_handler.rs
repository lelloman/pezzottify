//! Corruption detection and downloader restart management.
//!
//! Detects when the downloader is producing corrupted files (ffprobe failures)
//! and manages restart with escalating cooldown.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Action to take after recording a download result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerAction {
    /// Continue normal processing
    Continue,
    /// Restart the downloader service
    RestartNeeded,
}

/// Configuration for the corruption handler.
#[derive(Debug, Clone)]
pub struct CorruptionHandlerConfig {
    /// Size of the sliding window for tracking results
    pub window_size: usize,
    /// Number of failures in window that triggers restart
    pub failure_threshold: usize,
    /// Base cooldown duration in seconds (level 0)
    pub base_cooldown_secs: u64,
    /// Maximum cooldown duration in seconds
    pub max_cooldown_secs: u64,
    /// Multiplier for cooldown escalation (2.0 = double each level)
    pub cooldown_multiplier: f64,
    /// Number of successful downloads to de-escalate one level
    pub successes_to_deescalate: u32,
}

impl Default for CorruptionHandlerConfig {
    fn default() -> Self {
        Self {
            window_size: 4,
            failure_threshold: 2,
            base_cooldown_secs: 600,       // 10 minutes
            max_cooldown_secs: 7200,       // 2 hours
            cooldown_multiplier: 2.0,
            successes_to_deescalate: 10,
        }
    }
}

/// Current state of the corruption handler for diagnostics/persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerState {
    /// Current escalation level (0 = base)
    pub current_level: u32,
    /// Successful downloads since last level change
    pub successes_since_last_level_change: u32,
    /// Whether currently in cooldown
    pub in_cooldown: bool,
    /// Remaining cooldown time
    pub cooldown_remaining_secs: Option<u64>,
    /// Current cooldown duration based on level
    pub current_cooldown_duration_secs: u64,
    /// Recent results (true = success, false = corruption)
    pub recent_results: Vec<bool>,
}

/// Persistent state that can be saved/loaded across restarts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedState {
    pub current_level: u32,
    pub successes_since_last_level_change: u32,
    /// Unix timestamp of last restart (if any)
    pub last_restart_at_unix: Option<i64>,
}

/// Internal cooldown state.
struct CooldownState {
    current_level: u32,
    last_restart_at: Option<Instant>,
    successes_since_last_level_change: u32,
}

/// Handles corruption detection and downloader restart management.
///
/// Tracks a sliding window of recent download results and triggers
/// restarts when corruption rate exceeds threshold. Manages escalating
/// cooldown periods between restarts.
pub struct CorruptionHandler {
    /// Sliding window of recent results (true = success, false = corruption)
    recent_results: Mutex<VecDeque<bool>>,
    /// Cooldown state
    state: Mutex<CooldownState>,
    /// Flag to prevent concurrent restart attempts
    restart_in_progress: AtomicBool,
    /// Configuration
    config: CorruptionHandlerConfig,
}

impl CorruptionHandler {
    /// Create a new handler with the given configuration.
    pub fn new(config: CorruptionHandlerConfig) -> Self {
        Self {
            recent_results: Mutex::new(VecDeque::with_capacity(config.window_size)),
            state: Mutex::new(CooldownState {
                current_level: 0,
                last_restart_at: None,
                successes_since_last_level_change: 0,
            }),
            restart_in_progress: AtomicBool::new(false),
            config,
        }
    }

    /// Create a new handler with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CorruptionHandlerConfig::default())
    }

    /// Create a handler and restore state from persistence.
    pub fn with_persisted_state(config: CorruptionHandlerConfig, persisted: PersistedState) -> Self {
        let last_restart_at = persisted.last_restart_at_unix.and_then(|unix_ts| {
            let now_unix = chrono::Utc::now().timestamp();
            let elapsed_secs = (now_unix - unix_ts).max(0) as u64;
            // Convert to Instant by subtracting elapsed time from now
            Instant::now().checked_sub(Duration::from_secs(elapsed_secs))
        });

        Self {
            recent_results: Mutex::new(VecDeque::with_capacity(config.window_size)),
            state: Mutex::new(CooldownState {
                current_level: persisted.current_level,
                last_restart_at,
                successes_since_last_level_change: persisted.successes_since_last_level_change,
            }),
            restart_in_progress: AtomicBool::new(false),
            config,
        }
    }

    /// Record a download result.
    ///
    /// - `success = true`: Download succeeded (or failed for non-corruption reasons)
    /// - `success = false`: File was corrupted (ffprobe failure)
    ///
    /// Returns `RestartNeeded` if corruption threshold is exceeded.
    pub async fn record_result(&self, success: bool) -> HandlerAction {
        let mut results = self.recent_results.lock().await;
        let mut state = self.state.lock().await;

        // Add to sliding window
        if results.len() >= self.config.window_size {
            results.pop_front();
        }
        results.push_back(success);

        if success {
            // Track successes for de-escalation
            state.successes_since_last_level_change += 1;

            // Check for de-escalation
            if state.successes_since_last_level_change >= self.config.successes_to_deescalate {
                if state.current_level > 0 {
                    state.current_level -= 1;
                    info!(
                        "Corruption handler de-escalated to level {} after {} successes",
                        state.current_level,
                        self.config.successes_to_deescalate
                    );
                }
                state.successes_since_last_level_change = 0;
            }

            HandlerAction::Continue
        } else {
            // Count failures in window
            let failures = results.iter().filter(|&&r| !r).count();

            if failures >= self.config.failure_threshold {
                HandlerAction::RestartNeeded
            } else {
                HandlerAction::Continue
            }
        }
    }

    /// Called after a restart has been triggered.
    /// Escalates the cooldown level and resets counters.
    pub async fn record_restart(&self) {
        let mut state = self.state.lock().await;
        let mut results = self.recent_results.lock().await;

        state.current_level += 1;
        state.successes_since_last_level_change = 0;
        state.last_restart_at = Some(Instant::now());

        // Clear sliding window for fresh start
        results.clear();

        let cooldown = self.calculate_cooldown_duration(state.current_level);
        warn!(
            "Corruption handler escalated to level {}, cooldown: {} seconds",
            state.current_level,
            cooldown.as_secs()
        );
    }

    /// Check if currently in cooldown period.
    pub async fn is_in_cooldown(&self) -> bool {
        let state = self.state.lock().await;

        if let Some(last_restart) = state.last_restart_at {
            let cooldown = self.calculate_cooldown_duration(state.current_level);
            last_restart.elapsed() < cooldown
        } else {
            false
        }
    }

    /// Try to acquire the restart lock.
    /// Returns true if lock was acquired, false if restart already in progress.
    pub fn try_acquire_restart_lock(&self) -> bool {
        self.restart_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Release the restart lock.
    pub fn release_restart_lock(&self) {
        self.restart_in_progress.store(false, Ordering::SeqCst);
    }

    /// Admin reset: immediately resets level to 0 and clears cooldown.
    pub async fn admin_reset(&self) {
        let mut state = self.state.lock().await;
        let mut results = self.recent_results.lock().await;

        state.current_level = 0;
        state.successes_since_last_level_change = 0;
        state.last_restart_at = None;
        results.clear();

        info!("Corruption handler reset by admin");
    }

    /// Get current state for diagnostics/admin API.
    pub async fn get_state(&self) -> HandlerState {
        let state = self.state.lock().await;
        let results = self.recent_results.lock().await;

        let cooldown_duration = self.calculate_cooldown_duration(state.current_level);
        let (in_cooldown, cooldown_remaining_secs) = if let Some(last_restart) = state.last_restart_at
        {
            let elapsed = last_restart.elapsed();
            if elapsed < cooldown_duration {
                (true, Some((cooldown_duration - elapsed).as_secs()))
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };

        HandlerState {
            current_level: state.current_level,
            successes_since_last_level_change: state.successes_since_last_level_change,
            in_cooldown,
            cooldown_remaining_secs,
            current_cooldown_duration_secs: cooldown_duration.as_secs(),
            recent_results: results.iter().copied().collect(),
        }
    }

    /// Get state for persistence.
    pub async fn get_persisted_state(&self) -> PersistedState {
        let state = self.state.lock().await;

        let last_restart_at_unix = state.last_restart_at.map(|instant| {
            let elapsed = instant.elapsed();
            chrono::Utc::now().timestamp() - elapsed.as_secs() as i64
        });

        PersistedState {
            current_level: state.current_level,
            successes_since_last_level_change: state.successes_since_last_level_change,
            last_restart_at_unix,
        }
    }

    /// Calculate cooldown duration for a given level.
    fn calculate_cooldown_duration(&self, level: u32) -> Duration {
        let base = self.config.base_cooldown_secs as f64;
        let multiplier = self.config.cooldown_multiplier.powi(level as i32);
        let duration_secs = (base * multiplier) as u64;
        let capped = duration_secs.min(self.config.max_cooldown_secs);
        Duration::from_secs(capped)
    }

    /// Get current cooldown duration based on level.
    pub async fn current_cooldown_duration(&self) -> Duration {
        let state = self.state.lock().await;
        self.calculate_cooldown_duration(state.current_level)
    }

    /// Get the configuration.
    pub fn config(&self) -> &CorruptionHandlerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_config() -> CorruptionHandlerConfig {
        CorruptionHandlerConfig {
            window_size: 4,
            failure_threshold: 2,
            base_cooldown_secs: 10,
            max_cooldown_secs: 100,
            cooldown_multiplier: 2.0,
            successes_to_deescalate: 3,
        }
    }

    #[tokio::test]
    async fn test_success_does_not_trigger_restart() {
        let handler = CorruptionHandler::new(make_test_config());

        for _ in 0..10 {
            let action = handler.record_result(true).await;
            assert_eq!(action, HandlerAction::Continue);
        }
    }

    #[tokio::test]
    async fn test_single_failure_does_not_trigger_restart() {
        let handler = CorruptionHandler::new(make_test_config());

        let action = handler.record_result(false).await;
        assert_eq!(action, HandlerAction::Continue);
    }

    #[tokio::test]
    async fn test_threshold_triggers_restart() {
        let handler = CorruptionHandler::new(make_test_config());

        // First failure
        let action = handler.record_result(false).await;
        assert_eq!(action, HandlerAction::Continue);

        // Second failure reaches threshold (2 of 2 in window)
        let action = handler.record_result(false).await;
        assert_eq!(action, HandlerAction::RestartNeeded);
    }

    #[tokio::test]
    async fn test_successes_dilute_failures() {
        let handler = CorruptionHandler::new(make_test_config());

        // One failure
        handler.record_result(false).await;

        // Three successes
        handler.record_result(true).await;
        handler.record_result(true).await;
        handler.record_result(true).await;

        // Another failure - window is now [success, success, success, failure]
        // Only 1 failure, below threshold
        let action = handler.record_result(false).await;
        assert_eq!(action, HandlerAction::Continue);
    }

    #[tokio::test]
    async fn test_record_restart_escalates_level() {
        let handler = CorruptionHandler::new(make_test_config());

        let state_before = handler.get_state().await;
        assert_eq!(state_before.current_level, 0);

        handler.record_restart().await;

        let state_after = handler.get_state().await;
        assert_eq!(state_after.current_level, 1);
        assert!(state_after.in_cooldown);
    }

    #[tokio::test]
    async fn test_cooldown_duration_escalates() {
        let handler = CorruptionHandler::new(make_test_config());

        // Level 0: base (10 secs)
        let d0 = handler.calculate_cooldown_duration(0);
        assert_eq!(d0, Duration::from_secs(10));

        // Level 1: 10 * 2 = 20 secs
        let d1 = handler.calculate_cooldown_duration(1);
        assert_eq!(d1, Duration::from_secs(20));

        // Level 2: 10 * 4 = 40 secs
        let d2 = handler.calculate_cooldown_duration(2);
        assert_eq!(d2, Duration::from_secs(40));

        // Level 3: 10 * 8 = 80 secs
        let d3 = handler.calculate_cooldown_duration(3);
        assert_eq!(d3, Duration::from_secs(80));

        // Level 4: 10 * 16 = 160, but capped at 100
        let d4 = handler.calculate_cooldown_duration(4);
        assert_eq!(d4, Duration::from_secs(100));
    }

    #[tokio::test]
    async fn test_de_escalation_after_successes() {
        let config = make_test_config(); // successes_to_deescalate = 3
        let handler = CorruptionHandler::new(config);

        // Escalate to level 2
        handler.record_restart().await;
        handler.record_restart().await;

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 2);

        // 3 successes should de-escalate to level 1
        handler.record_result(true).await;
        handler.record_result(true).await;
        handler.record_result(true).await;

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 1);
        assert_eq!(state.successes_since_last_level_change, 0);

        // 3 more successes should de-escalate to level 0
        handler.record_result(true).await;
        handler.record_result(true).await;
        handler.record_result(true).await;

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 0);
    }

    #[tokio::test]
    async fn test_de_escalation_stops_at_zero() {
        let config = make_test_config();
        let handler = CorruptionHandler::new(config);

        // Already at level 0, successes should not go negative
        for _ in 0..10 {
            handler.record_result(true).await;
        }

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 0);
    }

    #[tokio::test]
    async fn test_admin_reset() {
        let handler = CorruptionHandler::new(make_test_config());

        // Escalate
        handler.record_restart().await;
        handler.record_restart().await;

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 2);
        assert!(state.in_cooldown);

        // Admin reset
        handler.admin_reset().await;

        let state = handler.get_state().await;
        assert_eq!(state.current_level, 0);
        assert!(!state.in_cooldown);
        assert_eq!(state.successes_since_last_level_change, 0);
    }

    #[tokio::test]
    async fn test_restart_lock() {
        let handler = CorruptionHandler::new(make_test_config());

        // First acquire should succeed
        assert!(handler.try_acquire_restart_lock());

        // Second acquire should fail
        assert!(!handler.try_acquire_restart_lock());

        // Release
        handler.release_restart_lock();

        // Now should succeed again
        assert!(handler.try_acquire_restart_lock());
    }

    #[tokio::test]
    async fn test_persisted_state_round_trip() {
        let config = make_test_config();
        let handler = CorruptionHandler::new(config.clone());

        // Escalate and record some successes
        handler.record_restart().await;
        handler.record_result(true).await;
        handler.record_result(true).await;

        let persisted = handler.get_persisted_state().await;
        assert_eq!(persisted.current_level, 1);
        assert_eq!(persisted.successes_since_last_level_change, 2);

        // Create new handler from persisted state
        let handler2 = CorruptionHandler::with_persisted_state(config, persisted);
        let state = handler2.get_state().await;

        assert_eq!(state.current_level, 1);
        assert_eq!(state.successes_since_last_level_change, 2);
    }

    #[tokio::test]
    async fn test_window_slides() {
        let handler = CorruptionHandler::new(make_test_config()); // window_size = 4

        // Fill window with failures
        handler.record_result(false).await;
        handler.record_result(false).await;

        // At this point we have 2 failures, threshold reached
        let state = handler.get_state().await;
        assert_eq!(state.recent_results, vec![false, false]);

        // Add successes to push failures out
        handler.record_result(true).await;
        handler.record_result(true).await;
        handler.record_result(true).await;
        handler.record_result(true).await;

        // Window should now be all successes (oldest failures pushed out)
        let state = handler.get_state().await;
        assert_eq!(state.recent_results, vec![true, true, true, true]);
    }
}
