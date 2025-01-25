use axum::extract::FromRef;

use crate::{catalog::Catalog, search::SearchVault};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::auth::AuthManager;

pub type GuardedCatalog = Arc<Mutex<Catalog>>;
pub type GuardedSearchVault = Arc<Mutex<SearchVault>>;
pub type GuardedAuthManager = Arc<Mutex<AuthManager>>;

#[derive(Clone)]
pub struct ServerState {
    pub start_time: Instant,
    pub catalog: GuardedCatalog,
    pub search_vault: GuardedSearchVault,
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

impl FromRef<ServerState> for GuardedSearchVault {
    fn from_ref(input: &ServerState) -> Self {
        input.search_vault.clone()
    }
}

impl FromRef<ServerState> for GuardedAuthManager {
    fn from_ref(input: &ServerState) -> Self {
        input.auth_manager.clone()
    }
}
