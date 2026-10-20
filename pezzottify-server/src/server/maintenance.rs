//! Server-wide maintenance mode state.
//!
//! The flag lives in `server_state` as JSON so it survives restarts and can be
//! turned off from the admin API even though the player UI is hidden.

use anyhow::{anyhow, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
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
}

impl MaintenanceHandle {
    pub fn new(state: MaintenanceState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
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

    /// Persist `next` before publishing it so a failed write never leaves the
    /// in-memory flag out of sync with the database.
    pub fn persist_and_replace(
        &self,
        store: &dyn ServerStore,
        next: MaintenanceState,
    ) -> Result<MaintenanceState> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| anyhow!("maintenance state lock poisoned"))?;
        next.persist(store)?;
        *guard = next.clone();
        Ok(next)
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
}
