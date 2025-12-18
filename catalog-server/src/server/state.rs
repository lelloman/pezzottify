use axum::extract::FromRef;

use crate::background_jobs::SchedulerHandle;
use crate::catalog_store::CatalogStore;
use crate::download_manager::DownloadManager;
use crate::downloader::Downloader;
use crate::search::SearchVault;
use crate::server_store::ServerStore;
use crate::user::UserManager;
use crate::whatsnew::WhatsNewNotifier;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::proxy::CatalogProxy;
use super::websocket::ConnectionManager;
use super::ServerConfig;

pub type GuardedCatalogStore = Arc<dyn CatalogStore>;
pub type GuardedSearchVault = Arc<Mutex<Box<dyn SearchVault>>>;
pub type GuardedUserManager = Arc<Mutex<UserManager>>;
pub type OptionalDownloader = Option<Arc<dyn Downloader>>;
pub type OptionalProxy = Option<Arc<CatalogProxy>>;
pub type GuardedConnectionManager = Arc<ConnectionManager>;
pub type OptionalSchedulerHandle = Option<SchedulerHandle>;
pub type OptionalDownloadManager = Option<Arc<DownloadManager>>;
pub type GuardedWhatsNewNotifier = Arc<WhatsNewNotifier>;
pub type GuardedServerStore = Arc<dyn ServerStore>;

#[derive(Clone)]
pub struct ServerState {
    pub config: ServerConfig,
    pub start_time: Instant,
    pub catalog_store: GuardedCatalogStore,
    pub search_vault: Arc<Mutex<Box<dyn SearchVault>>>,
    pub user_manager: GuardedUserManager,
    pub downloader: OptionalDownloader,
    pub proxy: OptionalProxy,
    pub ws_connection_manager: GuardedConnectionManager,
    pub scheduler_handle: OptionalSchedulerHandle,
    pub download_manager: OptionalDownloadManager,
    pub whatsnew_notifier: GuardedWhatsNewNotifier,
    pub server_store: GuardedServerStore,
    pub hash: String,
}

unsafe impl Send for ServerState {}
unsafe impl Sync for ServerState {}

impl FromRef<ServerState> for GuardedCatalogStore {
    fn from_ref(input: &ServerState) -> Self {
        input.catalog_store.clone()
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

impl FromRef<ServerState> for OptionalDownloadManager {
    fn from_ref(input: &ServerState) -> Self {
        input.download_manager.clone()
    }
}

impl FromRef<ServerState> for GuardedWhatsNewNotifier {
    fn from_ref(input: &ServerState) -> Self {
        input.whatsnew_notifier.clone()
    }
}

impl FromRef<ServerState> for GuardedServerStore {
    fn from_ref(input: &ServerState) -> Self {
        input.server_store.clone()
    }
}
