use axum::extract::FromRef;

use crate::{catalog::Catalog, search::SearchVault};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::server::auth::AuthManager;

use super::ServerConfig;

pub type GuardedCatalog = Arc<Mutex<Catalog>>;
pub type GuardedAuthManager = Arc<Mutex<AuthManager>>;

#[derive(Clone)]
pub struct ServerState {
    pub config: ServerConfig,
    pub start_time: Instant,
    pub catalog: GuardedCatalog,
    pub search_vault: Arc<Mutex<Box<dyn SearchVault>>>,
    pub auth_manager: GuardedAuthManager,
    pub hash: String,
}

unsafe impl Send for ServerState {}
unsafe impl Sync for ServerState {}

impl FromRef<ServerState> for GuardedCatalog {
    fn from_ref(input: &ServerState) -> Self {
        input.catalog.clone()
    }
}

impl FromRef<ServerState> for Arc<Mutex<Box<dyn SearchVault>>> {
    fn from_ref(input: &ServerState) -> Self {
        input.search_vault.clone()
    }
}

impl FromRef<ServerState> for GuardedAuthManager {
    fn from_ref(input: &ServerState) -> Self {
        input.auth_manager.clone()
    }
}

impl FromRef<ServerState> for ServerConfig {
    fn from_ref(input: &ServerState) -> Self {
        input.config.clone()
    }
}
