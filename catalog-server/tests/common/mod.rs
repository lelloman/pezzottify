//! Common test infrastructure
//!
//! This module provides all the infrastructure needed for end-to-end tests.
//! Tests should only import from this module, not from internal submodules.
//!
//! # Example
//!
//! ```no_run
//! mod common;
//! use common::{TestServer, TestClient, ARTIST_1_ID};
//! use reqwest::StatusCode;
//!
//! #[tokio::test]
//! async fn test_get_artist() {
//!     let server = TestServer::spawn().await;
//!     let client = TestClient::authenticated(server.base_url.clone()).await;
//!
//!     let response = client.get_artist(ARTIST_1_ID).await;
//!     assert_eq!(response.status(), StatusCode::OK);
//! }
//! ```

mod client;
mod constants;
mod fixtures;
mod server;

// Public API - this is what tests import
pub use client::TestClient;
pub use constants::*;
pub use server::TestServer;

// Keep fixtures internal - only accessed via TestServer::spawn()
#[allow(unused_imports)]
pub(crate) use fixtures::{create_test_catalog, create_test_db_with_users};
