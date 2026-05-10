//! Catalog store module for Spotify metadata.
//!
//! This module provides the `SqliteCatalogStore` for reading catalog data
//! from the Spotify metadata database.

mod models;
mod null_store;
mod schema;
mod store;
mod trait_def;
mod validation;

pub use models::*;
pub use null_store::NullCatalogStore;
pub use schema::CATALOG_VERSIONED_SCHEMAS;
pub use store::SqliteCatalogStore;
pub use trait_def::{
    AlbumEmbeddingCoverage, AlbumEmbeddingNamespaceCoverage, AlbumTrackRef, AlbumTracklist,
    CatalogStore, SearchableContentType, SearchableItem, TrackEmbeddingCoverage,
    TrackEmbeddingNamespaceCoverage,
};
pub use validation::{validate_album, validate_artist, validate_track, ValidationError};
