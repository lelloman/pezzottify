pub mod config;
mod download_routes;
mod http_layers;
mod ingestion_routes;
pub mod metrics;
// TODO: Re-enable after updating for Spotify schema (depends on downloader)
// pub mod proxy;
mod search;
#[allow(clippy::module_inception)]
pub mod server;
pub mod session;
// Skeleton sync module removed - Android now uses on-demand discography API
// mod skeleton;
pub mod state;
mod stream_track;
pub mod websocket;

pub use config::ServerConfig;
pub use download_routes::download_routes;
pub use http_layers::*;
pub use ingestion_routes::ingestion_routes;
use search::{make_search_admin_routes, make_search_routes};
#[allow(unused_imports)] // Used by main.rs
pub use server::run_server;
