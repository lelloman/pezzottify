use std::{sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(super) struct BoundedBlockingPool {
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

impl BoundedBlockingPool {
    pub(super) fn new(
        max_concurrent: usize,
        queue_timeout: Duration,
        execution_timeout: Duration,
    ) -> Self {
        assert!(max_concurrent > 0, "blocking concurrency must be non-zero");
        Self {
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
        let permit = tokio::time::timeout(
            self.queue_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| BlockingWorkError::QueueTimeout)?
        .map_err(|_| BlockingWorkError::ShuttingDown)?;

        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        });

        match tokio::time::timeout(self.execution_timeout, worker).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(BlockingWorkError::WorkerPanicked),
            Err(_) => Err(BlockingWorkError::ExecutionTimeout),
        }
    }
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BoundedBlockingPool>();
};
