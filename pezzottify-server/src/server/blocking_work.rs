use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(super) struct BoundedBlockingPool {
    name: &'static str,
    permits: Arc<Semaphore>,
    queue_timeout: Duration,
    execution_timeout: Duration,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum BlockingWorkError {
    #[error("blocking work queue timed out")]
    QueueTimeout,
    #[error("blocking work execution timed out")]
    ExecutionTimeout,
    #[error("blocking work pool is shutting down")]
    ShuttingDown,
    #[error("blocking worker panicked")]
    WorkerPanicked,
}

struct BlockingExecutionGuard {
    pool: &'static str,
    started: Instant,
}

struct BlockingWaitingGuard {
    pool: &'static str,
}

impl BlockingWaitingGuard {
    fn new(pool: &'static str) -> Self {
        super::metrics::blocking_work_waiting(pool, true);
        Self { pool }
    }
}

impl Drop for BlockingWaitingGuard {
    fn drop(&mut self) {
        super::metrics::blocking_work_waiting(self.pool, false);
    }
}

impl Drop for BlockingExecutionGuard {
    fn drop(&mut self) {
        super::metrics::blocking_work_finished(self.pool, self.started.elapsed());
    }
}

impl BoundedBlockingPool {
    pub(super) fn new(
        name: &'static str,
        max_concurrent: usize,
        queue_timeout: Duration,
        execution_timeout: Duration,
    ) -> Self {
        assert!(max_concurrent > 0, "blocking concurrency must be non-zero");
        Self {
            name,
            permits: Arc::new(Semaphore::new(max_concurrent)),
            queue_timeout,
            execution_timeout,
        }
    }

    pub(super) async fn run<T, F>(&self, work: F) -> Result<T, BlockingWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let queue_started = Instant::now();
        let waiting = BlockingWaitingGuard::new(self.name);
        let permit_result = tokio::time::timeout(
            self.queue_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await;
        drop(waiting);
        let permit = match permit_result {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                super::metrics::record_blocking_work_outcome(
                    self.name,
                    super::metrics::ExecutorOutcome::ShuttingDown,
                );
                return Err(BlockingWorkError::ShuttingDown);
            }
            Err(_) => {
                super::metrics::record_blocking_work_outcome(
                    self.name,
                    super::metrics::ExecutorOutcome::QueueTimeout,
                );
                return Err(BlockingWorkError::QueueTimeout);
            }
        };
        super::metrics::blocking_work_started(self.name, queue_started.elapsed());

        let pool_name = self.name;
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _metrics = BlockingExecutionGuard {
                pool: pool_name,
                started: Instant::now(),
            };
            work()
        });

        let (result, outcome) = match tokio::time::timeout(self.execution_timeout, worker).await {
            Ok(Ok(result)) => (Ok(result), super::metrics::ExecutorOutcome::Success),
            Ok(Err(_)) => (
                Err(BlockingWorkError::WorkerPanicked),
                super::metrics::ExecutorOutcome::Panicked,
            ),
            Err(_) => (
                Err(BlockingWorkError::ExecutionTimeout),
                super::metrics::ExecutorOutcome::ExecutionTimeout,
            ),
        };
        super::metrics::record_blocking_work_outcome(self.name, outcome);
        result
    }
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BoundedBlockingPool>();
};
