use crate::catalog_store::CatalogStore;
use crate::download_manager::DownloadSyncNotifier;
use crate::enrichment_store::EnrichmentStore;
use crate::search::SearchVault;
use crate::server_store::ServerStore;
use crate::user::{FullUserStore, UserManager};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// Type alias for thread-safe UserManager access.
pub type GuardedUserManager = Arc<Mutex<UserManager>>;

/// Type alias for thread-safe SearchVault access.
/// SearchVault is internally thread-safe (uses separate read/write connections with internal Mutex).
/// No external Mutex needed - the implementation handles concurrent access.
pub type GuardedSearchVault = Arc<dyn SearchVault>;

/// Type alias for thread-safe DownloadSyncNotifier access.
pub type GuardedSyncNotifier = Arc<DownloadSyncNotifier>;

/// Context provided to jobs during execution.
///
/// Contains references to shared resources and a cancellation token
/// for graceful shutdown handling.
#[derive(Clone)]
pub struct JobContext {
    /// Token to check for cancellation/shutdown requests.
    pub cancellation_token: CancellationToken,

    /// Access to the music catalog database.
    pub catalog_store: Arc<dyn CatalogStore>,

    /// Access to user data (playlists, liked content, etc.).
    pub user_store: Arc<dyn FullUserStore>,

    /// Access to server-side state (job history, schedules).
    pub server_store: Arc<dyn ServerStore>,

    /// Access to user manager (for caching, etc.).
    pub user_manager: GuardedUserManager,

    /// Access to search vault for updating popularity scores.
    pub search_vault: Option<GuardedSearchVault>,

    /// Access to sync notifier for emitting catalog events.
    pub sync_notifier: Option<GuardedSyncNotifier>,

    /// Access to enrichment store for audio features and metadata enrichment.
    pub enrichment_store: Option<Arc<dyn EnrichmentStore>>,
}

impl JobContext {
    /// Create a new job context with the given dependencies.
    pub fn new(
        cancellation_token: CancellationToken,
        catalog_store: Arc<dyn CatalogStore>,
        user_store: Arc<dyn FullUserStore>,
        server_store: Arc<dyn ServerStore>,
        user_manager: GuardedUserManager,
    ) -> Self {
        Self {
            cancellation_token,
            catalog_store,
            user_store,
            server_store,
            user_manager,
            search_vault: None,
            sync_notifier: None,
            enrichment_store: None,
        }
    }

    /// Create a new job context with search vault.
    pub fn with_search_vault(
        cancellation_token: CancellationToken,
        catalog_store: Arc<dyn CatalogStore>,
        user_store: Arc<dyn FullUserStore>,
        server_store: Arc<dyn ServerStore>,
        user_manager: GuardedUserManager,
        search_vault: GuardedSearchVault,
    ) -> Self {
        Self {
            cancellation_token,
            catalog_store,
            user_store,
            server_store,
            user_manager,
            search_vault: Some(search_vault),
            sync_notifier: None,
            enrichment_store: None,
        }
    }

    /// Create a new job context with sync notifier.
    pub fn with_sync_notifier(
        cancellation_token: CancellationToken,
        catalog_store: Arc<dyn CatalogStore>,
        user_store: Arc<dyn FullUserStore>,
        server_store: Arc<dyn ServerStore>,
        user_manager: GuardedUserManager,
        sync_notifier: GuardedSyncNotifier,
    ) -> Self {
        Self {
            cancellation_token,
            catalog_store,
            user_store,
            server_store,
            user_manager,
            search_vault: None,
            sync_notifier: Some(sync_notifier),
            enrichment_store: None,
        }
    }

    /// Create a new job context with search vault and sync notifier.
    pub fn with_search_vault_and_sync_notifier(
        cancellation_token: CancellationToken,
        catalog_store: Arc<dyn CatalogStore>,
        user_store: Arc<dyn FullUserStore>,
        server_store: Arc<dyn ServerStore>,
        user_manager: GuardedUserManager,
        search_vault: GuardedSearchVault,
        sync_notifier: GuardedSyncNotifier,
    ) -> Self {
        Self {
            cancellation_token,
            catalog_store,
            user_store,
            server_store,
            user_manager,
            search_vault: Some(search_vault),
            sync_notifier: Some(sync_notifier),
            enrichment_store: None,
        }
    }

    /// Set the enrichment store on this context.
    pub fn with_enrichment_store(mut self, store: Arc<dyn EnrichmentStore>) -> Self {
        self.enrichment_store = Some(store);
        self
    }

    /// Check if cancellation has been requested.
    ///
    /// Jobs should periodically check this during long-running operations
    /// and return early with `JobError::Cancelled` if true.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}
