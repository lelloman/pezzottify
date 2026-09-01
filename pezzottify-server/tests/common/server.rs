//! Test server lifecycle management
//!
//! This module manages spawning and shutting down test HTTP servers.
//! Each test gets an isolated server with its own catalog and database.

use super::constants::*;
use super::fixtures::{create_test_catalog, create_test_db_with_users};
use pezzottify_server::catalog_store::SqliteCatalogStore;
use pezzottify_server::search::{HashedItemType, SearchResult, SearchVault};
use pezzottify_server::server::state::GuardedSearchVault;
use pezzottify_server::server::{server::make_app, RequestsLoggingLevel, ServerConfig};
use pezzottify_server::server_store::{ServerStore, SqliteServerStore};
use pezzottify_server::user::{FullUserStore, SqliteUserStore, UserManager};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;

#[derive(Clone, Default)]
pub struct TestServerBuilder {
    download_manager_enabled: bool,
    ingestion_enabled: bool,
    disable_password_auth: bool,
    available_catalog: bool,
    strict_authorization_header: bool,
    scheduler_enabled: bool,
    scheduler_jobs: Vec<Arc<dyn pezzottify_server::background_jobs::BackgroundJob>>,
}

#[allow(dead_code)] // Each integration-test crate uses a different subset of builder options.
impl TestServerBuilder {
    pub fn with_download_manager(mut self) -> Self {
        self.download_manager_enabled = true;
        self
    }

    pub fn with_ingestion(mut self) -> Self {
        self.ingestion_enabled = true;
        self
    }

    pub fn with_password_auth_disabled(mut self) -> Self {
        self.disable_password_auth = true;
        self
    }

    pub fn with_available_catalog(mut self) -> Self {
        self.available_catalog = true;
        self
    }

    pub fn with_strict_authorization_header(mut self) -> Self {
        self.strict_authorization_header = true;
        self
    }

    pub fn with_scheduler(mut self) -> Self {
        self.scheduler_enabled = true;
        self
    }

    pub fn with_scheduler_job(
        mut self,
        job: Arc<dyn pezzottify_server::background_jobs::BackgroundJob>,
    ) -> Self {
        self.scheduler_enabled = true;
        self.scheduler_jobs.push(job);
        self
    }

    pub async fn spawn(self) -> TestServer {
        TestServer::spawn_with(self).await
    }
}

/// Mock search vault for testing - returns empty results
struct MockSearchVault;

impl SearchVault for MockSearchVault {
    fn search(
        &self,
        _query: &str,
        _max_results: usize,
        _filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        Vec::new()
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_popularity(&self, _items: &[(String, HashedItemType, u64, f64)]) {}

    fn upsert_items(
        &self,
        _items: &[pezzottify_server::search::SearchIndexItem],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_items(&self, _items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_stats(&self) -> pezzottify_server::search::SearchVaultStats {
        pezzottify_server::search::SearchVaultStats {
            indexed_items: 0,
            index_type: "Mock".to_string(),
            state: pezzottify_server::search::IndexState::Ready,
        }
    }

    fn record_impression(
        &self,
        _item_id: &str,
        _item_type: HashedItemType,
        _source: pezzottify_server::search::ImpressionSource,
    ) -> bool {
        false
    }

    fn get_impression_totals(
        &self,
        _min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        std::collections::HashMap::new()
    }

    fn prune_impressions(&self, _before_date: i64) -> usize {
        0
    }

    fn update_availability(&self, _items: &[(String, HashedItemType, bool)]) {}
}

/// Test server instance with isolated catalog and database
///
/// When dropped, the server gracefully shuts down and temp resources are cleaned up.
pub struct TestServer {
    /// Base URL for making requests (e.g., "http://127.0.0.1:12345")
    pub base_url: String,

    /// The port the server is listening on
    #[allow(dead_code)]
    pub port: u16,

    /// User store for direct database access in tests
    #[allow(dead_code)]
    pub user_store: Arc<dyn FullUserStore>,

    /// Server store for direct database access in tests (jobs, catalog events, whatsnew)
    #[allow(dead_code)]
    pub server_store: Arc<dyn ServerStore>,

    // Private fields - keep resources alive until drop
    _temp_catalog_dir: TempDir,
    _temp_db_dir: TempDir,
    _shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    _scheduler_shutdown: Option<tokio_util::sync::CancellationToken>,
    _scheduler_hook_sender:
        Option<tokio::sync::mpsc::Sender<pezzottify_server::background_jobs::HookEvent>>,
}

impl TestServer {
    #[allow(dead_code)] // Used by configurable integration-test crates.
    pub fn builder() -> TestServerBuilder {
        TestServerBuilder::default()
    }

    /// Spawns a new test server on a random port
    ///
    /// This function:
    /// 1. Creates a temporary catalog with test data
    /// 2. Creates a temporary database with test users
    /// 3. Loads the catalog (with no_checks for speed)
    /// 4. Binds to a random port (127.0.0.1:0)
    /// 5. Spawns the server in a background task
    /// 6. Waits for the server to be ready
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Catalog or database creation fails
    /// - Port binding fails
    /// - Server fails to start
    /// - Server doesn't become ready within timeout
    pub async fn spawn() -> Self {
        Self::builder().spawn().await
    }

    async fn spawn_with(options: TestServerBuilder) -> Self {
        // Create temporary test resources
        let (temp_catalog_dir, catalog_db_path, media_path) =
            create_test_catalog().expect("Failed to create test catalog");
        if options.available_catalog {
            let connection = rusqlite::Connection::open(&catalog_db_path)
                .expect("Failed to reopen test catalog");
            connection
                .execute("UPDATE tracks SET track_available = 1", [])
                .expect("Failed to mark test catalog tracks available");
        }
        if options.download_manager_enabled {
            let connection = rusqlite::Connection::open(&catalog_db_path)
                .expect("Failed to reopen test catalog");
            connection
                .execute("UPDATE tracks SET audio_uri = NULL", [])
                .expect("Failed to make test catalog downloadable");
        }
        let (temp_db_dir, db_path) =
            create_test_db_with_users().expect("Failed to create test database");

        // Create database registry for backup checkpoint control
        let db_registry = Arc::new(pezzottify_server::backup::DbRegistry::new());

        // Open SQLite catalog store
        let catalog_store = Arc::new(
            SqliteCatalogStore::new(&catalog_db_path, &media_path, 4, &db_registry)
                .expect("Failed to open catalog store"),
        );

        // Create user store
        let user_store: Arc<dyn FullUserStore> = Arc::new(
            SqliteUserStore::new(&db_path, &db_registry).expect("Failed to open user store"),
        );
        let user_store_for_test = user_store.clone();

        // Create search vault (use mock for speed in tests)
        let search_vault: GuardedSearchVault = Arc::new(MockSearchVault);

        // Bind to random port
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to random port");

        let port = listener
            .local_addr()
            .expect("Failed to get local address")
            .port();

        let base_url = format!("http://127.0.0.1:{}", port);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Build the app
        let download_manager = pezzottify_server::config::DownloadManagerSettings {
            enabled: options.download_manager_enabled,
            ..Default::default()
        };
        let ingestion = pezzottify_server::config::IngestionSettings {
            enabled: options.ingestion_enabled,
            ..Default::default()
        };

        let config = ServerConfig {
            port,
            requests_logging_level: RequestsLoggingLevel::None,
            content_cache_age_sec: 0, // Disable caching in tests
            frontend_dir_path: None,
            disable_password_auth: options.disable_password_auth,
            secure_session_cookies: false,
            session_cookie_max_age_secs: 7 * 24 * 60 * 60,
            allow_legacy_raw_authorization: !options.strict_authorization_header,
            login_rate_limit_per_minute: 1_000,
            login_rate_limit_per_hour: 1_000,
            streaming_search: pezzottify_server::config::StreamingSearchSettings::default(),
            download_manager,
            proxy_mode: pezzottify_server::config::ProxyModeSettings::default(),
            downloader_url: None,
            downloader_timeout_sec: 300,
            db_dir: temp_db_dir.path().to_path_buf(),
            media_path: media_path.clone(),
            agent: pezzottify_server::config::AgentSettings::default(),
            ingestion,
            audio_embeddings: None,
        };

        // Create user manager
        let user_manager = Arc::new(UserManager::new(user_store.clone()));

        // Create server store for testing
        let server_db_path = temp_db_dir.path().join("server.db");
        let server_store: Arc<dyn ServerStore> = Arc::new(
            SqliteServerStore::new(&server_db_path, &db_registry)
                .expect("Failed to create server store"),
        );
        let server_store_for_test = server_store.clone();

        let (scheduler_handle, scheduler_shutdown, scheduler_hook_sender) =
            if options.scheduler_enabled {
                let scheduler_shutdown = tokio_util::sync::CancellationToken::new();
                let (hook_sender, hook_receiver) = tokio::sync::mpsc::channel(16);
                let context = pezzottify_server::background_jobs::JobContext::with_search_vault(
                    scheduler_shutdown.child_token(),
                    catalog_store.clone(),
                    user_store.clone(),
                    server_store.clone(),
                    user_manager.clone(),
                    search_vault.clone(),
                );
                let (mut scheduler, handle) = pezzottify_server::background_jobs::create_scheduler(
                    server_store.clone(),
                    hook_receiver,
                    scheduler_shutdown.clone(),
                    context,
                );
                for job in options.scheduler_jobs {
                    scheduler.register_job(job).await;
                }
                tokio::spawn(async move { scheduler.run().await });
                (Some(handle), Some(scheduler_shutdown), Some(hook_sender))
            } else {
                (None, None, None)
            };

        let app = make_app(
            config,
            catalog_store,
            search_vault,
            user_store,
            user_manager,
            scheduler_handle,
            server_store,
            None, // oidc_config
            db_registry,
            None,
        )
        .await
        .expect("Failed to build app");

        // Spawn server in background task with graceful shutdown
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .expect("Server failed");
        });

        // Wait for server to be ready
        let server = Self {
            base_url: base_url.clone(),
            port,
            user_store: user_store_for_test,
            server_store: server_store_for_test,
            _temp_catalog_dir: temp_catalog_dir,
            _temp_db_dir: temp_db_dir,
            _shutdown_tx: Some(shutdown_tx),
            _scheduler_shutdown: scheduler_shutdown,
            _scheduler_hook_sender: scheduler_hook_sender,
        };

        server.wait_for_ready().await;

        server
    }

    /// Waits for the server to become ready by polling the /v1/statics endpoint
    async fn wait_for_ready(&self) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .expect("Failed to build reqwest client");

        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(SERVER_READY_TIMEOUT_MS);

        loop {
            if start.elapsed() > timeout {
                panic!(
                    "Server did not become ready within {}ms",
                    SERVER_READY_TIMEOUT_MS
                );
            }

            match client.get(format!("{}/", self.base_url)).send().await {
                Ok(response) if response.status().is_success() => {
                    // Server is ready
                    return;
                }
                _ => {
                    // Server not ready yet, wait and retry
                    tokio::time::sleep(Duration::from_millis(SERVER_READY_POLL_INTERVAL_MS)).await;
                }
            }
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(token) = self._scheduler_shutdown.take() {
            token.cancel();
        }
        // Send shutdown signal
        if let Some(tx) = self._shutdown_tx.take() {
            let _ = tx.send(());
        }
        // TempDir and NamedTempFile will be cleaned up automatically
    }
}
