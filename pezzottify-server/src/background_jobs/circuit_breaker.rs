use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const CIRCUIT_BREAKER_STATE_KEY: &str = "background_jobs.circuit_breakers.v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CircuitBreakerRegistry {
    #[serde(default)]
    jobs: BTreeMap<String, CircuitBreakerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CircuitBreakerState {
    consecutive_failures: u32,
    open_until_millis: Option<i64>,
}

impl CircuitBreakerRegistry {
    pub(crate) fn is_open(&self, job_id: &str, now_millis: i64) -> bool {
        self.jobs
            .get(job_id)
            .and_then(|state| state.open_until_millis)
            .is_some_and(|open_until| open_until > now_millis)
    }

    pub(crate) fn open_until_millis(&self, job_id: &str) -> Option<i64> {
        self.jobs
            .get(job_id)
            .and_then(|state| state.open_until_millis)
    }

    pub(crate) fn remaining_open_millis(&self, job_id: &str, now_millis: i64) -> Option<u64> {
        let remaining = self.open_until_millis(job_id)?.saturating_sub(now_millis);
        (remaining > 0).then_some(remaining as u64)
    }

    pub(crate) fn record_success(&mut self, job_id: &str) {
        self.jobs.remove(job_id);
    }

    pub(crate) fn record_failure(
        &mut self,
        job_id: &str,
        failure_threshold: u32,
        cooldown_millis: i64,
        now_millis: i64,
    ) -> bool {
        let state = self
            .jobs
            .entry(job_id.to_string())
            .or_insert(CircuitBreakerState {
                consecutive_failures: 0,
                open_until_millis: None,
            });
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= failure_threshold {
            state.open_until_millis = Some(now_millis.saturating_add(cooldown_millis));
            true
        } else {
            false
        }
    }
}
