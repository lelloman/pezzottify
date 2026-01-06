//! Test server lifecycle management
//!
//! This module manages spawning and shutting down test HTTP servers.
//! Each test gets an isolated server with its own catalog and database.

use super::constants::*;
use super::fixtures::{create_test_catalog, create_test_db_with_users};
use pezzottify_catalog_server::catalog_store::{CatalogStore, SqliteCatalogStore};
use pezzottify_catalog_server::search::{HashedItemType, SearchResult, SearchVault};
use pezzottify_catalog_server::server::state::GuardedSearchVault;
use pezzottify_catalog_server::server::{server::make_app, RequestsLoggingLevel, ServerConfig};
use pezzottify_catalog_server::server_store::SqliteServerStore;
use pezzottify_catalog_server::user::{FullUserStore, SqliteUserStore, UserManager};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;

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

    fn upsert_items(&self, _items: &[pezzottify_catalog_server::search::SearchIndexItem]) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_items(&self, _items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_stats(&self) -> pezzottify_catalog_server::search::SearchVaultStats {
        pezzottify_catalog_server::search::SearchVaultStats {
            indexed_items: 0,
            index_type: "Mock".to_string(),
            state: pezzottify_catalog_server::search::IndexState::Ready,
        }
    }
}

/// Test server instance with isolated catalog and database
///
/// When dropped, the server gracefully shuts down and temp resources are cleaned up.
pub struct TestServer {
    /// Base URL for making requests (e.g., "http://127.0.0.1:12345")
    pub base_url: String,

    /// The port the server is listening on
    pub port: u16,

    /// User store for direct database access in tests
    pub user_store: Arc<dyn FullUserStore>,

    // Private fields - keep resources alive until drop
    _temp_catalog_dir: TempDir,
    _temp_db_dir: TempDir,
    _shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestServer {
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
        // Create temporary test resources
        let (temp_catalog_dir, catalog_db_path, media_path) =
            create_test_catalog().expect("Failed to create test catalog");
        let (temp_db_dir, db_path) =
            create_test_db_with_users().expect("Failed to create test database");

        // Open SQLite catalog store
        let catalog_store = Arc::new(
            SqliteCatalogStore::new(&catalog_db_path, &media_path)
                .expect("Failed to open catalog store"),
        );

        // Create user store
        let user_store: Arc<dyn FullUserStore> =
            Arc::new(SqliteUserStore::new(&db_path).expect("Failed to open user store"));
        let user_store_for_test = user_store.clone();

        // Create search vault (use mock for speed in tests)
        let search_vault: GuardedSearchVault = Arc::new(Mutex::new(Box::new(MockSearchVault)));

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
        let config = ServerConfig {
            port,
            requests_logging_level: RequestsLoggingLevel::None,
            content_cache_age_sec: 0, // Disable caching in tests
            frontend_dir_path: None,
            disable_password_auth: false,
            streaming_search: pezzottify_catalog_server::config::StreamingSearchSettings::default(),
        };

        // Create user manager
        let user_manager = Arc::new(Mutex::new(UserManager::new(
            catalog_store.clone() as Arc<dyn CatalogStore>,
            user_store.clone(),
        )));

        // Create server store for testing
        let server_db_path = temp_db_dir.path().join("server.db");
        let server_store = Arc::new(
            SqliteServerStore::new(&server_db_path).expect("Failed to create server store"),
        );

        let app = make_app(
            config,
            catalog_store,
            search_vault,
            user_store,
            user_manager,
            None, // scheduler_handle
            server_store,
            None, // oidc_config
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
            _temp_catalog_dir: temp_catalog_dir,
            _temp_db_dir: temp_db_dir,
            _shutdown_tx: Some(shutdown_tx),
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
        // Send shutdown signal
        if let Some(tx) = self._shutdown_tx.take() {
            let _ = tx.send(());
        }
        // TempDir and NamedTempFile will be cleaned up automatically
    }
}
