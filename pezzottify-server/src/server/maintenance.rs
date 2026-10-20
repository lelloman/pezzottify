//! Server-wide maintenance mode state.
//!
//! The flag lives in `server_state` as JSON so it survives restarts and can be
//! turned off from the admin API even though the player UI is hidden.

use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::{Arc, RwLock};
use tracing::{error, warn};

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

    /// Serialize updates, persist the supplied state, then publish it. The
    /// snapshot lock is only taken for the final swap, so status probes never
    /// wait for database work. A failed write leaves the live state untouched.
    pub async fn persist_and_replace<F>(&self, persist: F) -> Result<MaintenanceState>
    where
        F: Future<Output = Result<MaintenanceState>> + Send,
    {
        let _updates = self.updates.lock().await;
        let next = persist.await?;
        Ok(self.replace(next))
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

    #[tokio::test]
    async fn delayed_write_does_not_block_status_reads() {
        let handle = MaintenanceHandle::new(MaintenanceState::default());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let writer = {
            let handle = handle.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(async move {
                        let _ = started_tx.send(());
                        let _ = release_rx.await;
                        Ok(MaintenanceState {
                            enabled: true,
                            ..MaintenanceState::default()
                        })
                    })
                    .await
            })
        };
        started_rx.await.expect("writer should start");

        let read_handle = handle.clone();
        let snapshot = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            tokio::task::spawn_blocking(move || read_handle.snapshot()),
        )
        .await
        .expect("status read must not wait for the delayed write")
        .expect("status read should complete");
        assert!(!snapshot.enabled);

        release_tx.send(()).expect("release writer");
        let state = writer.await.expect("writer task").expect("write");
        assert!(state.enabled);
        assert!(handle.is_enabled());
    }

    #[tokio::test]
    async fn failed_write_keeps_previous_state_and_later_updates_still_apply() {
        let handle = MaintenanceHandle::new(MaintenanceState {
            enabled: true,
            message: Some("still up".to_owned()),
            estimated_end: None,
        });

        let result = handle
            .persist_and_replace(async { anyhow::bail!("simulated write failure") })
            .await;
        assert!(result.is_err());
        assert!(handle.is_enabled());
        assert_eq!(handle.snapshot().message.as_deref(), Some("still up"));

        let state = handle
            .persist_and_replace(async { Ok(MaintenanceState::default()) })
            .await
            .expect("later write should succeed");
        assert!(!state.enabled);
        assert!(!handle.is_enabled());
    }

    #[tokio::test]
    async fn updates_are_applied_in_order() {
        let handle = MaintenanceHandle::new(MaintenanceState::default());
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (release_first_tx, release_first_rx) = tokio::sync::oneshot::channel();
        let (second_started_tx, mut second_started_rx) = tokio::sync::mpsc::unbounded_channel();

        let first = {
            let handle = handle.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(async move {
                        let _ = first_started_tx.send(());
                        let _ = release_first_rx.await;
                        Ok(MaintenanceState {
                            enabled: true,
                            message: Some("first".to_owned()),
                            estimated_end: None,
                        })
                    })
                    .await
            })
        };
        first_started_rx.await.expect("first writer should start");

        let second = {
            let handle = handle.clone();
            tokio::spawn(async move {
                handle
                    .persist_and_replace(async move {
                        let _ = second_started_tx.send(());
                        Ok(MaintenanceState {
                            enabled: false,
                            message: Some("second".to_owned()),
                            estimated_end: None,
                        })
                    })
                    .await
            })
        };

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            second_started_rx.try_recv().is_err(),
            "second update must wait for the first"
        );
        release_first_tx.send(()).expect("release first writer");

        let first_state = first.await.expect("first task").expect("first write");
        assert_eq!(first_state.message.as_deref(), Some("first"));
        second_started_rx
            .recv()
            .await
            .expect("second writer should start after the first");
        let second_state = second.await.expect("second task").expect("second write");
        assert_eq!(second_state.message.as_deref(), Some("second"));
        assert_eq!(handle.snapshot().message.as_deref(), Some("second"));
    }
}
