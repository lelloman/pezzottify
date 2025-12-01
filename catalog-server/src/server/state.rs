use axum::extract::FromRef;

use crate::catalog_store::CatalogStore;
use crate::downloader::DownloaderClient;
use crate::search::SearchVault;
use crate::user::UserManager;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::proxy::CatalogProxy;
use super::ServerConfig;

pub type GuardedCatalogStore = Arc<dyn CatalogStore>;
pub type GuardedUserManager = Arc<Mutex<UserManager>>;
pub type OptionalDownloader = Option<Arc<DownloaderClient>>;
pub type OptionalProxy = Option<Arc<CatalogProxy>>;

#[derive(Clone)]
pub struct ServerState {
    pub config: ServerConfig,
    pub start_time: Instant,
    pub catalog_store: GuardedCatalogStore,
    pub search_vault: Arc<Mutex<Box<dyn SearchVault>>>,
    pub user_manager: GuardedUserManager,
    pub downloader: OptionalDownloader,
    pub proxy: OptionalProxy,
    pub hash: String,
}

unsafe impl Send for ServerState {}
unsafe impl Sync for ServerState {}

impl FromRef<ServerState> for GuardedCatalogStore {
    fn from_ref(input: &ServerState) -> Self {
        input.catalog_store.clone()
    }
}

impl FromRef<ServerState> for Arc<Mutex<Box<dyn SearchVault>>> {
    fn from_ref(input: &ServerState) -> Self {
        input.search_vault.clone()
    }
}

impl FromRef<ServerState> for GuardedUserManager {
    fn from_ref(input: &ServerState) -> Self {
        input.user_manager.clone()
    }
}

impl FromRef<ServerState> for ServerConfig {
    fn from_ref(input: &ServerState) -> Self {
        input.config.clone()
    }
}

impl FromRef<ServerState> for OptionalDownloader {
    fn from_ref(input: &ServerState) -> Self {
        input.downloader.clone()
    }
}

impl FromRef<ServerState> for OptionalProxy {
    fn from_ref(input: &ServerState) -> Self {
        input.proxy.clone()
    }
}
