use std::{sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::Semaphore;

use crate::user::{auth::PezzottifyHasher, UserManager, UsernamePasswordCredentials};

const DEFAULT_MAX_CONCURRENT: usize = 4;
const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(1);
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(super) struct PasswordWorkPool {
    permits: Arc<Semaphore>,
    queue_timeout: Duration,
    execution_timeout: Duration,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum PasswordWorkError {
    #[error("password verification queue timed out")]
    QueueTimeout,
    #[error("password verification timed out")]
    ExecutionTimeout,
    #[error("password verification pool is shutting down")]
    ShuttingDown,
    #[error("password verification worker panicked")]
    WorkerPanicked,
}

impl Default for PasswordWorkPool {
    fn default() -> Self {
        Self::with_limits(
            DEFAULT_MAX_CONCURRENT,
            DEFAULT_QUEUE_TIMEOUT,
            DEFAULT_EXECUTION_TIMEOUT,
        )
    }
}

impl PasswordWorkPool {
    pub(super) fn with_limits(
        max_concurrent: usize,
        queue_timeout: Duration,
        execution_timeout: Duration,
    ) -> Self {
        assert!(
            max_concurrent > 0,
            "password verification concurrency must be non-zero"
        );
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent)),
            queue_timeout,
            execution_timeout,
        }
    }

    pub(super) async fn verify(
        &self,
        hasher: PezzottifyHasher,
        password: String,
        hash: String,
        salt: String,
    ) -> Result<bool, PasswordWorkError> {
        self.run(move || hasher.verify(password, hash, salt).unwrap_or(false))
            .await
    }

    pub(super) async fn hash(
        &self,
        user_id: usize,
        password: String,
    ) -> Result<anyhow::Result<UsernamePasswordCredentials>, PasswordWorkError> {
        self.run(move || UserManager::create_hashed_password(user_id, password))
            .await
    }

    pub(super) async fn run<T, F>(&self, work: F) -> Result<T, PasswordWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit = tokio::time::timeout(
            self.queue_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| PasswordWorkError::QueueTimeout)?
        .map_err(|_| PasswordWorkError::ShuttingDown)?;

        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        });

        match tokio::time::timeout(self.execution_timeout, worker).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(PasswordWorkError::WorkerPanicked),
            Err(_) => Err(PasswordWorkError::ExecutionTimeout),
        }
    }
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PasswordWorkPool>();
};
