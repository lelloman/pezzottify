//! MCP Tool Execution Context
//!
//! Provides access to server state for tool implementations.

use std::sync::Arc;

use crate::background_jobs::SchedulerHandle;
use crate::catalog_store::CatalogStore;
use crate::search::SearchVault;
use crate::server::session::Session;
use crate::server::ServerConfig;
use crate::server_store::ServerStore;
use crate::user::Permission;
use crate::user::UserManager;
use std::sync::Mutex;

/// Context provided to tool and resource handlers during execution
#[derive(Clone)]
pub struct ToolContext {
    /// The authenticated session (user, permissions)
    pub session: Session,

    /// Access to catalog data
    pub catalog_store: Arc<dyn CatalogStore>,

    /// Access to search functionality
    pub search_vault: Arc<Mutex<Box<dyn SearchVault>>>,

    /// Access to user management
    pub user_manager: Arc<Mutex<UserManager>>,

    /// Access to server store (jobs, etc.)
    pub server_store: Arc<dyn ServerStore>,

    /// Access to background job scheduler
    pub scheduler_handle: Option<SchedulerHandle>,

    /// Server configuration
    pub config: ServerConfig,

    /// Server version info
    pub server_version: String,

    /// Server start time (for uptime calculation)
    pub start_time: std::time::Instant,
}

impl ToolContext {
    /// Check if the session has a specific permission
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.session.has_permission(permission)
    }

    /// Get the user ID from the session
    pub fn user_id(&self) -> usize {
        self.session.user_id
    }
}
