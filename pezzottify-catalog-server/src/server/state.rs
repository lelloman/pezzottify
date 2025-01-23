use axum::extract::FromRef;

use crate::{catalog::Catalog, search::SearchVault};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct ServerState {
    pub start_time: Instant,
    pub catalog: Arc<Mutex<Catalog>>,
    pub search_vault: Arc<Mutex<SearchVault>>,
    pub hash: String,
}

unsafe impl Send for ServerState {}
unsafe impl Sync for ServerState {}


impl FromRef<ServerState> for Arc<Mutex<Catalog>> {
    fn from_ref(input: &ServerState) -> Self {
        input.catalog.clone()
    }
}

impl FromRef<ServerState> for Arc<Mutex<SearchVault>> {
    fn from_ref(input: &ServerState) -> Self {
        input.search_vault.clone()
    }
}