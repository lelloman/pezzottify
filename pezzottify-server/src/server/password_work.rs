use std::time::Duration;

use crate::user::{auth::PezzottifyHasher, UserManager, UsernamePasswordCredentials};

use super::blocking_work::{BlockingWorkError, BoundedBlockingPool};

const DEFAULT_MAX_CONCURRENT: usize = 4;
const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(1);
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(super) struct PasswordWorkPool {
    inner: BoundedBlockingPool,
}

pub(super) type PasswordWorkError = BlockingWorkError;

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
            inner: BoundedBlockingPool::new(
                "password",
                max_concurrent,
                queue_timeout,
                execution_timeout,
            ),
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
        self.inner.run(work).await
    }
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PasswordWorkPool>();
};
