use crate::catalog_store::CatalogStore;
use crate::server_store::ServerStore;
use crate::user::FullUserStore;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

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
}

impl JobContext {
    /// Create a new job context with the given dependencies.
    pub fn new(
        cancellation_token: CancellationToken,
        catalog_store: Arc<dyn CatalogStore>,
        user_store: Arc<dyn FullUserStore>,
        server_store: Arc<dyn ServerStore>,
    ) -> Self {
        Self {
            cancellation_token,
            catalog_store,
            user_store,
            server_store,
        }
    }

    /// Check if cancellation has been requested.
    ///
    /// Jobs should periodically check this during long-running operations
    /// and return early with `JobError::Cancelled` if true.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}
