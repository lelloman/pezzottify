//! SQLite-backed catalog store implementation for Spotify schema.
//!
//! This module provides the `SqliteCatalogStore` which reads catalog metadata
//! from the Spotify metadata database dump.

use super::models::*;
use super::schema::{
    create_artist_enrichment_enqueue_trigger, create_catalog_stats_triggers,
    initialize_empty_catalog_stats, CATALOG_VERSIONED_SCHEMAS,
};
use super::trait_def::{
    AlbumTrackRef, AlbumTracklist, CatalogStore, SearchableContentType, SearchableItem,
    MAX_ALBUM_TRACKLIST_PAGE_SIZE,
};
use super::CatalogMutationError;
use crate::sqlite_persistence::{configure_connection, BASE_DB_VERSION};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::info;

/// Validate the catalog representation of a media path. Catalog paths are URI-like,
/// relative identifiers; accepting platform-specific separators would make a catalog
/// safe on one host and unsafe after moving it to another.
fn normalized_media_identifier(identifier: &str) -> Result<PathBuf> {
    if identifier.is_empty()
        || identifier.starts_with('/')
        || identifier.contains('\\')
        || identifier.contains('\0')
    {
        anyhow::bail!("media identifier must be a non-empty relative path");
    }

    let mut normalized = PathBuf::new();
    for component in identifier.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            anyhow::bail!("media identifier contains a non-normal component");
        }
        normalized.push(component);
    }

    if normalized.is_absolute()
        || normalized
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!("media identifier is not a normalized relative path");
    }
    Ok(normalized)
}

#[cfg(any(not(unix), test))]
fn resolve_existing_media_path(media_root: &Path, identifier: &str) -> Result<PathBuf> {
    let relative = normalized_media_identifier(identifier)?;
    let canonical_root = media_root
        .canonicalize()
        .with_context(|| format!("failed to resolve media root {}", media_root.display()))?;
    let canonical_path = media_root
        .join(relative)
        .canonicalize()
        .with_context(|| format!("failed to resolve media identifier {identifier:?}"))?;
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!("media identifier resolves outside the configured media root");
    }
    if !canonical_path.is_file() {
        anyhow::bail!("media identifier does not resolve to a regular file");
    }
    Ok(canonical_path)
}

#[cfg(unix)]
fn open_media_file_beneath(media_root: &Path, identifier: &str) -> Result<(File, PathBuf)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = normalized_media_identifier(identifier)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut directory = File::open(media_root)
        .with_context(|| format!("failed to open media root {}", media_root.display()))?;

    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(name) = component else {
            anyhow::bail!("media identifier contains a non-normal component");
        };
        let name = CString::new(name.as_bytes()).context("media identifier contains NUL")?;
        let is_last = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if is_last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: directory is a live directory fd, name is NUL-terminated, and a
        // successful fd is immediately owned by File. O_NOFOLLOW is applied at every level.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("failed to safely open media identifier {identifier:?}"));
        }
        // SAFETY: openat returned a new owned descriptor on success.
        let opened = unsafe { File::from_raw_fd(fd) };
        if is_last {
            if !opened.metadata()?.is_file() {
                anyhow::bail!("media identifier does not resolve to a regular file");
            }
            return Ok((opened, media_root.join(relative)));
        }
        directory = opened;
    }

    anyhow::bail!("media identifier must not be empty")
}

#[cfg(not(unix))]
fn open_media_file_beneath(media_root: &Path, identifier: &str) -> Result<(File, PathBuf)> {
    let path = resolve_existing_media_path(media_root, identifier)?;
    Ok((File::open(&path)?, path))
}

/// SQLite-backed catalog store for Spotify metadata.
#[derive(Clone)]
pub struct SqliteCatalogStore {
    read_pool: Vec<Arc<Mutex<Connection>>>,
    write_conn: Arc<Mutex<Connection>>,
    media_base_path: PathBuf,
    read_index: Arc<AtomicUsize>,
}

fn migrate_if_needed(conn: &mut Connection) -> Result<()> {
    let db_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    let latest_version = CATALOG_VERSIONED_SCHEMAS.len() - 1;
    let latest_schema = &CATALOG_VERSIONED_SCHEMAS[latest_version];

    // Check if this is a brand new database (no tables exist)
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if table_count == 0 {
        // Brand new database - create the latest schema directly
        info!("Creating catalog db schema at version {}", latest_version);
        latest_schema.create(conn)?;
        create_artist_enrichment_enqueue_trigger(conn)?;
        initialize_empty_catalog_stats(conn)?;
        create_catalog_stats_triggers(conn)?;
        return Ok(());
    }

    // Handle legacy databases that don't have versioned schema yet (user_version = 0)
    // These should be treated as version 0 and need migration
    let mut current_version = if db_version < BASE_DB_VERSION as i64 {
        // Legacy database - check which columns exist to determine effective version
        let has_album_availability = conn
            .query_row(
                "SELECT 1 FROM pragma_table_info('albums') WHERE name = 'album_availability'",
                [],
                |r| r.get::<_, i32>(0),
            )
            .ok()
            == Some(1);

        if has_album_availability {
            1 // Has v1 columns, treat as v1
        } else {
            0 // Legacy database at v0
        }
    } else {
        (db_version - BASE_DB_VERSION as i64) as usize
    };

    if current_version >= latest_version {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for schema in CATALOG_VERSIONED_SCHEMAS.iter().skip(current_version + 1) {
        if let Some(migration_fn) = schema.migration {
            info!(
                "Migrating catalog db from version {} to {}",
                current_version, schema.version
            );
            migration_fn(&tx)?;
            current_version = schema.version;
        }
    }
    tx.pragma_update(None, "user_version", BASE_DB_VERSION + current_version)?;

    tx.commit()?;
    let _ = conn.query_row(
        "PRAGMA wal_checkpoint(TRUNCATE)",
        [],
        |_: &rusqlite::Row| Ok(()),
    );
    Ok(())
}

const ENRICHMENT_MAX_ATTEMPTS: i64 = 8;
const ENRICHMENT_RETRY_BASE_SECS: i64 = 60 * 60;
const ENRICHMENT_RETRY_MAX_SECS: i64 = 7 * 24 * 60 * 60;
const ENRICHMENT_CLAIM_LEASE_SECS: i64 = 6 * 60 * 60;

include!("sqlite_queries.rs");
include!("sqlite_catalog_adapter.rs");
