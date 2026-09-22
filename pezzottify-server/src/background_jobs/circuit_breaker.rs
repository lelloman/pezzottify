use super::JobCircuitBreakerPolicy;
use serde::{Deserialize, Serialize};
use simple_server::task_policies::{
    CircuitBreaker, CircuitOutcome, CircuitPermit, CircuitPolicy, CircuitSnapshot,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    time::{Duration, SystemTime},
};

pub(crate) const CIRCUIT_BREAKER_STATE_KEY: &str = "background_jobs.circuit_breakers.v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CircuitBreakerRegistry {
    #[serde(default)]
    jobs: BTreeMap<String, CircuitBreakerState>,
    #[serde(skip)]
    active: BTreeMap<String, CircuitBreaker>,
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

    /// Per-job overlap exclusion in the scheduler serializes admission and
    /// completion. Only the serializable snapshot is written to ServerStore.
    pub(crate) fn begin(&mut self, job_id: &str, policy: JobCircuitBreakerPolicy) -> CircuitPermit {
        let mut snapshot = self
            .jobs
            .get(job_id)
            .map(|state| CircuitSnapshot {
                consecutive_failures: state.consecutive_failures,
                open_until: state.open_until_millis.map(millis_to_time),
            })
            .unwrap_or_default();
        // A new application version can lower the configured threshold below
        // a previously persisted closed failure count. Keep the next run allowed,
        // as before migration; its success clears history and its failure opens.
        if snapshot.open_until.is_none()
            && snapshot.consecutive_failures >= policy.failure_threshold
        {
            snapshot.open_until = Some(SystemTime::now());
        }
        let mut breaker = CircuitBreaker::restore(
            CircuitPolicy {
                failure_threshold: NonZeroU32::new(policy.failure_threshold)
                    .expect("nonzero threshold"),
                cooldown: policy.cooldown,
            },
            snapshot,
        )
        .expect("valid persisted circuit state");
        // Admission was already checked before writing the execution history.
        // Use the persisted boundary if the wall clock moved backwards meanwhile.
        let now = SystemTime::now();
        let admission_time = breaker
            .snapshot()
            .open_until
            .map_or(now, |until| now.max(until));
        let permit = breaker
            .admit(admission_time)
            .expect("one execution per job");
        self.active.insert(job_id.to_owned(), breaker);
        permit
    }

    pub(crate) fn finish(
        &mut self,
        job_id: &str,
        permit: CircuitPermit,
        outcome: CircuitOutcome,
    ) -> bool {
        let mut breaker = self
            .active
            .remove(job_id)
            .expect("admitted circuit execution");
        breaker.record(permit, outcome, SystemTime::now());
        let snapshot = breaker.snapshot();
        let opened = outcome == CircuitOutcome::Failure && snapshot.open_until.is_some();
        if outcome == CircuitOutcome::Success {
            self.jobs.remove(job_id);
        } else if outcome == CircuitOutcome::Failure {
            self.jobs.insert(
                job_id.to_owned(),
                CircuitBreakerState {
                    consecutive_failures: snapshot.consecutive_failures,
                    open_until_millis: snapshot
                        .open_until
                        .map(|date| chrono::DateTime::<chrono::Utc>::from(date).timestamp_millis()),
                },
            );
        }
        opened
    }
}

fn millis_to_time(millis: i64) -> SystemTime {
    if millis >= 0 {
        SystemTime::UNIX_EPOCH + Duration::from_millis(millis as u64)
    } else {
        SystemTime::UNIX_EPOCH - Duration::from_millis(millis.unsigned_abs())
    }
}
