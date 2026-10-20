//! Server-wide maintenance mode state.
//!
//! The flag lives in `server_state` as JSON so it survives restarts and can be
//! turned off from the admin API even though the player UI is hidden.

use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tracing::{error, warn};

use crate::db_executor::{DbHandle, DbPriority};
use crate::server_store::ServerStore;

pub const MAINTENANCE_STATE_KEY: &str = "server.maintenance.v1";
pub const MAINTENANCE_HEADER: &str = "X-Maintenance-Mode";

/// Persisted maintenance state exposed to clients.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceState {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub message: Option<String>,
    /// RFC 3339 timestamp for the expected end of maintenance.
    #[serde(default)]
    pub estimated_end: Option<String>,
}

impl MaintenanceState {
    pub fn load(store: &dyn ServerStore) -> Self {
        match store.get_state(MAINTENANCE_STATE_KEY) {
            Ok(Some(raw)) => match serde_json::from_str(&raw) {
                Ok(state) => state,
                Err(error) => {
                    warn!(%error, "Ignoring invalid persisted maintenance state");
                    Self::default()
                }
            },
            Ok(None) => Self::default(),
            Err(error) => {
                warn!(%error, "Failed to read persisted maintenance state");
                Self::default()
            }
        }
    }

    pub fn persist(&self, store: &dyn ServerStore) -> Result<()> {
        let raw = serde_json::to_string(self)?;
        store.set_state(MAINTENANCE_STATE_KEY, &raw)
    }

    /// Normalize an administrator-supplied timestamp and reject non-RFC 3339 values.
    pub fn normalize_estimated_end(value: Option<&str>) -> Result<Option<String>, String> {
        let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(None);
        };
        DateTime::parse_from_rfc3339(raw)
            .map(|timestamp| {
                Some(
                    timestamp
                        .with_timezone(&Utc)
                        .to_rfc3339_opts(SecondsFormat::Secs, true),
                )
            })
            .map_err(|_| "estimated_end must be an RFC 3339 timestamp".to_owned())
    }
}

/// Cloneable handle shared by the HTTP layer and handlers.
#[derive(Clone, Default)]
pub struct MaintenanceHandle {
    inner: Arc<RwLock<MaintenanceState>>,
    updates: Arc<tokio::sync::Mutex<()>>,
}

impl MaintenanceHandle {
    pub fn new(state: MaintenanceState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
            updates: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn snapshot(&self) -> MaintenanceState {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                error!("Maintenance state lock poisoned while reading");
                poisoned.into_inner().clone()
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.snapshot().enabled
    }

    /// Persist the supplied state on the database lane and publish it.
    ///
    /// Both the write and the live-state swap run inside the executor job, so a
    /// request that gives up waiting (execution timeout or cancellation) cannot
    /// leave the persisted flag and the maintenance gate disagreeing. The update
    /// lock travels with that job too: a later update cannot persist or publish
    /// before this one has finished, even if the caller already returned.
    ///
    /// The snapshot lock is only taken for the final swap, so status probes
    /// never wait for database work. A failed write leaves the live state
    /// untouched.
    pub async fn persist_and_replace<S, F>(
        &self,
        database: DbHandle<S>,
        persist: F,
    ) -> Result<MaintenanceState>
    where
        S: ?Sized + Send + Sync + 'static,
        F: FnOnce(&S) -> Result<MaintenanceState> + Send + 'static,
    {
        let updates = self.updates.clone().lock_owned().await;
        let handle = self.clone();
        database
            .run(DbPriority::Interactive, move |store| {
                let _updates = updates;
                let next = persist(store)?;
                Ok(handle.replace(next))
            })
            .await
            .map_err(anyhow::Error::from)
    }

    fn replace(&self, next: MaintenanceState) -> MaintenanceState {
        match self.inner.write() {
            Ok(mut guard) => *guard = next.clone(),
            Err(poisoned) => {
                error!("Maintenance state lock poisoned while writing; replacing it anyway");
                *poisoned.into_inner() = next.clone();
            }
        }
        next
    }
}

/// Paths that must stay reachable while maintenance mode is on: clients read
/// `/v1/status`, users log in and out, and server admins turn the flag back off.
pub fn path_is_exempt(path: &str) -> bool {
    path == "/v1/status"
        || path == "/v1/auth"
        || path.starts_with("/v1/auth/")
        || path == "/v1/admin"
        || path.starts_with("/v1/admin/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_executor::{DbExecutor, DbExecutorConfig, DbLane};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    #[test]
    fn exempts_status_auth_and_admin_only() {
        assert!(path_is_exempt("/v1/status"));
        assert!(path_is_exempt("/v1/auth/login"));
        assert!(path_is_exempt("/v1/auth/session"));
        assert!(path_is_exempt("/v1/admin/maintenance"));
        assert!(!path_is_exempt("/v1/content/album/1"));
        assert!(!path_is_exempt("/v1/user/settings"));
        assert!(!path_is_exempt("/v1/administrator"));
    }

    #[test]
    fn normalizes_estimated_end_to_rfc3339_utc() {
        assert_eq!(
            MaintenanceState::normalize_estimated_end(Some("2026-10-21T10:00:00+02:00")).unwrap(),
            Some("2026-10-21T08:00:00Z".to_owned())
        );
        assert_eq!(
            MaintenanceState::normalize_estimated_end(Some("   ")).unwrap(),
            None
        );
        assert!(MaintenanceState::normalize_estimated_end(Some("tomorrow")).is_err());
    }

    struct TestStore {
        persisted: StdMutex<Vec<MaintenanceState>>,
        slow_enabled_write: Duration,
        fail: AtomicBool,
        started: AtomicUsize,
    }

    impl TestStore {
        fn new(slow_enabled_write: Duration) -> Arc<Self> {
            Arc::new(Self {
                persisted: StdMutex::new(Vec::new()),
                slow_enabled_write,
                fail: AtomicBool::new(false),
                started: AtomicUsize::new(0),
            })
        }

        fn persist(&self, next: &MaintenanceState) -> Result<()> {
            self.started.fetch_add(1, Ordering::SeqCst);
            if next.enabled {
                std::thread::sleep(self.slow_enabled_write);
            }
            if self.fail.load(Ordering::SeqCst) {
                anyhow::bail!("simulated write failure");
            }
            self.persisted.lock().unwrap().push(next.clone());
            Ok(())
        }

        fn last_persisted(&self) -> Option<MaintenanceState> {
            self.persisted.lock().unwrap().last().cloned()
        }

        fn write_count(&self) -> usize {
            self.persisted.lock().unwrap().len()
        }
    }

    fn database(store: Arc<TestStore>, execution_timeout: Duration) -> DbHandle<TestStore> {
        let mut config = DbExecutorConfig::default();
        config.worker_threads = 1;
        config.interactive.queue_timeout = Duration::from_secs(2);
        config.interactive.execution_timeout = execution_timeout;
        DbHandle::new(store, DbExecutor::new(config), DbLane::Server)
    }

    async fn wait_until(mut predicate: impl FnMut() -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            if predicate() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("condition was not met before the timeout");
    }

    fn enabled(message: &str) -> MaintenanceState {
        MaintenanceState {
            enabled: true,
            message: Some(message.to_owned()),
            estimated_end: None,
        }
    }

    fn disabled(message: &str) -> MaintenanceState {
        MaintenanceState {
            enabled: false,
            message: Some(message.to_owned()),
            estimated_end: None,
        }
    }

    #[tokio::test]
    async fn delayed_write_does_not_block_status_reads() {
        let store = TestStore::new(Duration::from_millis(150));
        let database = database(store.clone(), Duration::from_secs(2));
        let handle = MaintenanceHandle::new(MaintenanceState::default());

        let writer = {
            let handle = handle.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(database, |store| {
                        let next = enabled("delayed");
                        store.persist(&next)?;
                        Ok(next)
                    })
                    .await
            })
        };
        wait_until(|| store.started.load(Ordering::SeqCst) > 0).await;

        let read_handle = handle.clone();
        let snapshot = tokio::time::timeout(
            Duration::from_secs(1),
            tokio::task::spawn_blocking(move || read_handle.snapshot()),
        )
        .await
        .expect("status read must not wait for the delayed write")
        .expect("status read should complete");
        assert!(!snapshot.enabled);

        let state = writer.await.expect("writer task").expect("write");
        assert!(state.enabled);
        assert!(handle.is_enabled());
        assert_eq!(store.last_persisted(), Some(enabled("delayed")));
    }

    #[tokio::test]
    async fn failed_write_keeps_previous_state_and_later_updates_still_apply() {
        let store = TestStore::new(Duration::ZERO);
        store.fail.store(true, Ordering::SeqCst);
        let database = database(store.clone(), Duration::from_secs(2));
        let handle = MaintenanceHandle::new(MaintenanceState {
            enabled: true,
            message: Some("still up".to_owned()),
            estimated_end: None,
        });

        let result = handle
            .persist_and_replace(database.clone(), |store| {
                let next = disabled("failed");
                store.persist(&next)?;
                Ok(next)
            })
            .await;
        assert!(result.is_err());
        assert!(handle.is_enabled());
        assert_eq!(handle.snapshot().message.as_deref(), Some("still up"));
        assert_eq!(store.write_count(), 0);

        store.fail.store(false, Ordering::SeqCst);
        let state = handle
            .persist_and_replace(database, |store| {
                let next = disabled("recovered");
                store.persist(&next)?;
                Ok(next)
            })
            .await
            .expect("later write should succeed");
        assert!(!state.enabled);
        assert!(!handle.is_enabled());
        assert_eq!(store.last_persisted(), Some(disabled("recovered")));
    }

    #[tokio::test]
    async fn updates_are_applied_in_order() {
        let store = TestStore::new(Duration::from_millis(100));
        let database = database(store.clone(), Duration::from_secs(2));
        let handle = MaintenanceHandle::new(MaintenanceState::default());

        let first = {
            let handle = handle.clone();
            let database = database.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(database, |store| {
                        let next = enabled("first");
                        store.persist(&next)?;
                        Ok(next)
                    })
                    .await
            })
        };
        wait_until(|| store.started.load(Ordering::SeqCst) > 0).await;

        let second = {
            let handle = handle.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(database, |store| {
                        let next = disabled("second");
                        store.persist(&next)?;
                        Ok(next)
                    })
                    .await
            })
        };

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            store.write_count(),
            0,
            "second update must wait for the first"
        );

        let first_state = first.await.expect("first task").expect("first write");
        assert_eq!(first_state.message.as_deref(), Some("first"));
        let second_state = second.await.expect("second task").expect("second write");
        assert_eq!(second_state.message.as_deref(), Some("second"));
        assert_eq!(handle.snapshot().message.as_deref(), Some("second"));
        assert_eq!(store.write_count(), 2);
    }

    #[tokio::test]
    async fn timed_out_write_still_publishes_and_orders_the_next_update() {
        let store = TestStore::new(Duration::from_millis(150));
        let database = database(store.clone(), Duration::from_millis(20));
        let handle = MaintenanceHandle::new(MaintenanceState::default());

        let error = handle
            .persist_and_replace(database.clone(), |store| {
                let next = enabled("timed out");
                store.persist(&next)?;
                Ok(next)
            })
            .await
            .expect_err("the executor should report its execution timeout");
        assert!(
            error.to_string().contains("timed out"),
            "unexpected error: {error}"
        );

        // The executor job keeps running after the request stops waiting, so
        // the persisted flag and the live gate still converge on enabled.
        wait_until(|| store.write_count() == 1).await;
        wait_until(|| handle.is_enabled()).await;

        // The next update must publish only after the timed-out write, even
        // though the previous caller already received an error.
        let state = handle
            .persist_and_replace(database, |store| {
                let next = disabled("after timeout");
                store.persist(&next)?;
                Ok(next)
            })
            .await
            .expect("later update should apply");
        assert!(!state.enabled);
        assert!(!handle.is_enabled());
        assert_eq!(store.last_persisted(), Some(disabled("after timeout")));
        assert_eq!(store.write_count(), 2);
    }

    #[tokio::test]
    async fn cancelled_write_still_publishes_and_orders_the_next_update() {
        let store = TestStore::new(Duration::from_millis(150));
        let database = database(store.clone(), Duration::from_secs(2));
        let handle = MaintenanceHandle::new(MaintenanceState::default());

        let writer = {
            let handle = handle.clone();
            let database = database.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(database, |store| {
                        let next = enabled("cancelled");
                        store.persist(&next)?;
                        Ok(next)
                    })
                    .await
            })
        };
        wait_until(|| store.started.load(Ordering::SeqCst) > 0).await;
        writer.abort();
        let _ = writer.await;

        wait_until(|| store.write_count() == 1).await;
        wait_until(|| handle.is_enabled()).await;

        let state = handle
            .persist_and_replace(database, |store| {
                let next = disabled("after cancellation");
                store.persist(&next)?;
                Ok(next)
            })
            .await
            .expect("later update should apply");
        assert!(!state.enabled);
        assert!(!handle.is_enabled());
        assert_eq!(store.last_persisted(), Some(disabled("after cancellation")));
        assert_eq!(store.write_count(), 2);
    }
}
