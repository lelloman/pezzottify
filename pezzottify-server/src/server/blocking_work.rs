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
    admitted: Option<Arc<Semaphore>>,
    queue_timeout: Duration,
    execution_timeout: Duration,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum BlockingWorkError {
    #[error("blocking work queue is full")]
    QueueFull,
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
            admitted: None,
            queue_timeout,
            execution_timeout,
        }
    }

    /// Bound queued plus executing work. Running closures retain admission even
    /// when their caller is cancelled or its execution deadline expires.
    pub(super) fn with_admission_limit(mut self, max_admitted: usize) -> Self {
        assert!(max_admitted > 0, "blocking admission must be non-zero");
        self.admitted = Some(Arc::new(Semaphore::new(max_admitted)));
        self
    }

    pub(super) async fn run<T, F>(&self, work: F) -> Result<T, BlockingWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let admission = match &self.admitted {
            Some(slots) => match Arc::clone(slots).try_acquire_owned() {
                Ok(permit) => Some(permit),
                Err(_) => {
                    super::metrics::record_blocking_work_outcome(
                        self.name,
                        super::metrics::ExecutorOutcome::QueueFull,
                    );
                    return Err(BlockingWorkError::QueueFull);
                }
            },
            None => None,
        };
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
            let _admission = admission;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn admission_stays_bounded_after_execution_timeout() {
        let pool = BoundedBlockingPool::new(
            "test_admission_timeout",
            1,
            Duration::from_millis(20),
            Duration::from_millis(50),
        )
        .with_admission_limit(1);
        let (release, wait) = std::sync::mpsc::channel();
        let result = pool
            .run(move || {
                let _ = wait.recv_timeout(Duration::from_secs(5));
            })
            .await;
        assert_eq!(result, Err(BlockingWorkError::ExecutionTimeout));
        assert_eq!(pool.run(|| ()).await, Err(BlockingWorkError::QueueFull));
        assert_eq!(
            super::super::metrics::BLOCKING_WORK_OPERATIONS_TOTAL
                .with_label_values(&["test_admission_timeout", "queue_full"])
                .get(),
            1.0
        );
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while pool.admitted.as_ref().unwrap().available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(pool.run(|| 42).await, Ok(42));
    }

    #[tokio::test]
    async fn bounded_queue_releases_cancelled_waiters_but_keeps_running_work() {
        let pool = BoundedBlockingPool::new(
            "test_admission_cancel",
            1,
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .with_admission_limit(2);
        let (release, wait) = std::sync::mpsc::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let active_pool = pool.clone();
        let active = tokio::spawn(async move {
            active_pool
                .run(move || {
                    let _ = started.send(());
                    let _ = wait.recv_timeout(Duration::from_secs(5));
                })
                .await
        });
        ready.await.unwrap();
        let waiting_pool = pool.clone();
        let waiting = tokio::spawn(async move { waiting_pool.run(|| ()).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while pool.admitted.as_ref().unwrap().available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(pool.run(|| ()).await, Err(BlockingWorkError::QueueFull));
        waiting.abort();
        assert!(waiting.await.unwrap_err().is_cancelled());
        active.abort();
        assert!(active.await.unwrap_err().is_cancelled());
        assert_eq!(pool.admitted.as_ref().unwrap().available_permits(), 1);
        assert_eq!(pool.run(|| ()).await, Err(BlockingWorkError::QueueTimeout));
        assert_eq!(pool.admitted.as_ref().unwrap().available_permits(), 1);
        release.send(()).unwrap();
        assert_eq!(pool.run(|| 42).await, Ok(42));
    }
}
