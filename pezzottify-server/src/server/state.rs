use axum::extract::FromRef;

use crate::background_jobs::SchedulerHandle;
use crate::backup::DbRegistry;
use crate::catalog_store::CatalogStore;
use crate::db_executor::{DbExecutor, DbExecutorConfig, DbHandle, DbLane};
use crate::download_manager::DownloadManager;
use crate::enrichment_store::EnrichmentStore;
use crate::ingestion::IngestionManager;
use crate::mcp::handler::McpState;
use crate::oidc::{AuthStateStore, OidcClient};
use crate::search::{OrganicIndexer, SearchVault};
use crate::server_store::ServerStore;
use crate::shows::ShowStore;
use crate::user::{FullUserStore, UserManager};
use std::sync::Arc;
use std::time::Instant;

use super::websocket::{ConnectionManager, PlaybackSessionManager};
use super::ServerConfig;

pub type GuardedCatalogStore = Arc<dyn CatalogStore>;
/// SearchVault is internally thread-safe (uses separate read/write connections with internal Mutex).
/// No external Mutex needed - the implementation handles concurrent access.
pub type GuardedSearchVault = Arc<dyn SearchVault>;
pub type GuardedUserManager = Arc<UserManager>;
pub type GuardedConnectionManager = Arc<ConnectionManager>;
pub type OptionalSchedulerHandle = Option<SchedulerHandle>;
pub type GuardedServerStore = Arc<dyn ServerStore>;
pub type GuardedShowStore = Arc<dyn ShowStore>;
pub type OptionalOidcClient = Option<Arc<OidcClient>>;
pub type GuardedAuthStateStore = Arc<AuthStateStore>;
pub type GuardedMcpState = Arc<McpState>;
pub type OptionalOrganicIndexer = Option<Arc<OrganicIndexer>>;
pub type HttpClient = reqwest::Client;
pub type OptionalDownloadManager = Option<Arc<DownloadManager>>;
pub type OptionalIngestionManager = Option<Arc<IngestionManager>>;
pub type OptionalEnrichmentStore = Option<Arc<dyn EnrichmentStore>>;
pub type GuardedPlaybackSessionManager = Arc<PlaybackSessionManager>;
pub type GuardedDbRegistry = Arc<DbRegistry>;

/// Typed executor handles for stores that are present for the server lifetime.
///
/// Existing code continues to use the legacy fields during the incremental
/// migration. Keeping both views backed by the same `Arc`s makes this wiring
/// behavior-neutral while giving every later migration an explicit lane.
#[derive(Clone)]
pub struct DatabaseHandles {
    pub executor: DbExecutor,
    pub catalog_read: DbHandle<dyn CatalogStore>,
    pub catalog_write: DbHandle<dyn CatalogStore>,
    pub search_read: DbHandle<dyn SearchVault>,
    pub search_write: DbHandle<dyn SearchVault>,
    pub user_store: DbHandle<dyn FullUserStore>,
    pub user_manager: DbHandle<UserManager>,
    pub server: DbHandle<dyn ServerStore>,
    pub shows: DbHandle<dyn ShowStore>,
    pub backup: DbHandle<DbRegistry>,
    pub enrichment_read: Option<DbHandle<dyn EnrichmentStore>>,
    pub enrichment_write: Option<DbHandle<dyn EnrichmentStore>>,
}

impl DatabaseHandles {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        catalog_store: Arc<dyn CatalogStore>,
        search_vault: Arc<dyn SearchVault>,
        user_store: Arc<dyn FullUserStore>,
        user_manager: Arc<UserManager>,
        server_store: Arc<dyn ServerStore>,
        show_store: Arc<dyn ShowStore>,
        db_registry: Arc<DbRegistry>,
        enrichment_store: Option<Arc<dyn EnrichmentStore>>,
    ) -> Self {
        let executor = DbExecutor::new(DbExecutorConfig::default());
        Self {
            catalog_read: DbHandle::new(
                catalog_store.clone(),
                executor.clone(),
                DbLane::CatalogRead,
            ),
            catalog_write: DbHandle::new(catalog_store, executor.clone(), DbLane::CatalogWrite),
            search_read: DbHandle::new(search_vault.clone(), executor.clone(), DbLane::SearchRead),
            search_write: DbHandle::new(search_vault, executor.clone(), DbLane::SearchWrite),
            user_store: DbHandle::new(user_store, executor.clone(), DbLane::User),
            user_manager: DbHandle::new(user_manager, executor.clone(), DbLane::User),
            server: DbHandle::new(server_store, executor.clone(), DbLane::Server),
            shows: DbHandle::new(show_store, executor.clone(), DbLane::Shows),
            backup: DbHandle::new(db_registry, executor.clone(), DbLane::Server),
            enrichment_read: enrichment_store
                .clone()
                .map(|store| DbHandle::new(store, executor.clone(), DbLane::EnrichmentRead)),
            enrichment_write: enrichment_store
                .map(|store| DbHandle::new(store, executor.clone(), DbLane::EnrichmentWrite)),
            executor,
        }
    }
}

#[derive(Clone)]
pub struct ServerState {
    pub config: ServerConfig,
    pub start_time: Instant,
    pub catalog_store: GuardedCatalogStore,
    pub search_vault: GuardedSearchVault,
    pub user_manager: GuardedUserManager,
    pub ws_connection_manager: GuardedConnectionManager,
    pub scheduler_handle: OptionalSchedulerHandle,
    pub server_store: GuardedServerStore,
    pub show_store: GuardedShowStore,
    pub hash: String,
    pub oidc_client: OptionalOidcClient,
    pub auth_state_store: GuardedAuthStateStore,
    pub mcp_state: GuardedMcpState,
    pub organic_indexer: OptionalOrganicIndexer,
    pub http_client: HttpClient,
    pub download_manager: OptionalDownloadManager,
    pub ingestion_manager: OptionalIngestionManager,
    pub enrichment_store: OptionalEnrichmentStore,
    /// Priority-aware database handles used by incrementally migrated call sites.
    pub database: DatabaseHandles,
    /// Playback session manager for multi-device playback sync
    pub playback_session_manager: GuardedPlaybackSessionManager,
    /// Database registry for backup checkpoint operations
    pub db_registry: GuardedDbRegistry,
}

// Keep thread-safety checked by the compiler as fields are added to ServerState.
const _: () = assert_send_sync::<ServerState>();

const fn assert_send_sync<T: Send + Sync>() {}

impl FromRef<ServerState> for GuardedCatalogStore {
    fn from_ref(input: &ServerState) -> Self {
        input.catalog_store.clone()
    }
}

impl FromRef<ServerState> for GuardedShowStore {
    fn from_ref(input: &ServerState) -> Self {
        input.show_store.clone()
    }
}

impl FromRef<ServerState> for GuardedSearchVault {
    fn from_ref(input: &ServerState) -> Self {
        input.search_vault.clone()
    }
}

impl FromRef<ServerState> for GuardedUserManager {
    fn from_ref(input: &ServerState) -> Self {
        input.user_manager.clone()
    }
}

impl FromRef<ServerState> for DatabaseHandles {
    fn from_ref(input: &ServerState) -> Self {
        input.database.clone()
    }
}

impl FromRef<ServerState> for ServerConfig {
    fn from_ref(input: &ServerState) -> Self {
        input.config.clone()
    }
}

impl FromRef<ServerState> for GuardedConnectionManager {
    fn from_ref(input: &ServerState) -> Self {
        input.ws_connection_manager.clone()
    }
}

impl FromRef<ServerState> for OptionalSchedulerHandle {
    fn from_ref(input: &ServerState) -> Self {
        input.scheduler_handle.clone()
    }
}

impl FromRef<ServerState> for GuardedServerStore {
    fn from_ref(input: &ServerState) -> Self {
        input.server_store.clone()
    }
}

impl FromRef<ServerState> for OptionalOidcClient {
    fn from_ref(input: &ServerState) -> Self {
        input.oidc_client.clone()
    }
}

impl FromRef<ServerState> for GuardedAuthStateStore {
    fn from_ref(input: &ServerState) -> Self {
        input.auth_state_store.clone()
    }
}

impl FromRef<ServerState> for GuardedMcpState {
    fn from_ref(input: &ServerState) -> Self {
        input.mcp_state.clone()
    }
}

impl FromRef<ServerState> for OptionalOrganicIndexer {
    fn from_ref(input: &ServerState) -> Self {
        input.organic_indexer.clone()
    }
}

impl FromRef<ServerState> for HttpClient {
    fn from_ref(input: &ServerState) -> Self {
        input.http_client.clone()
    }
}

impl FromRef<ServerState> for OptionalDownloadManager {
    fn from_ref(input: &ServerState) -> Self {
        input.download_manager.clone()
    }
}

impl FromRef<ServerState> for OptionalIngestionManager {
    fn from_ref(input: &ServerState) -> Self {
        input.ingestion_manager.clone()
    }
}

impl FromRef<ServerState> for GuardedPlaybackSessionManager {
    fn from_ref(input: &ServerState) -> Self {
        input.playback_session_manager.clone()
    }
}

impl FromRef<ServerState> for GuardedDbRegistry {
    fn from_ref(input: &ServerState) -> Self {
        input.db_registry.clone()
    }
}

impl FromRef<ServerState> for OptionalEnrichmentStore {
    fn from_ref(input: &ServerState) -> Self {
        input.enrichment_store.clone()
    }
}
