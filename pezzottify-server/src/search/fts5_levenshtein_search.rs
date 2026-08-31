//! FTS5 search with Levenshtein-based typo tolerance
//!
//! This search vault combines SQLite FTS5 full-text search with
//! Levenshtein distance-based query correction for typo tolerance.
//!
//! ## Background Indexing
//!
//! The vault supports non-blocking background indexing:
//! 1. Call `new_lazy()` for instant startup (returns empty vault)
//! 2. Call `start_background_build()` to begin indexing
//! 3. Items are indexed in batches by popularity (most popular first)
//! 4. Search works during indexing (returns partial results)
//! 5. Progress is available via `get_stats()`

use super::levenshtein::Vocabulary;
use super::{
    CandidateCoordinator, CandidateDelivery, HashedItemType, ImpressionSource, IndexState,
    ProviderEvidence, RankedCandidate, SearchIndexItem, SearchResult, SearchVault,
    SearchVaultStats,
};
use crate::catalog_store::{CatalogStore, SearchableContentType};
use anyhow::{bail, Result};
use rayon::prelude::*;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

const SEARCH_INDEX_SCHEMA_VERSION: i64 = 2;
const ACTIVE_INDEX: &str = "search_index";
const BUILD_INDEX: &str = "search_index_building";
const BUILD_INDEX_CONTENT: &str = "search_index_building_content";
const PREVIOUS_INDEX: &str = "search_index_previous";
const AVAILABLE_INDEX: &str = "search_index_available";
const AVAILABLE_BUILD_INDEX: &str = "search_index_available_building";
const AVAILABLE_PREVIOUS_INDEX: &str = "search_index_available_previous";
const AVAILABLE_INDEX_SCHEMA_VERSION: i64 = 1;
const MAX_VOCABULARY_SOURCE_ROWS: usize = 500_000;
const BUILD_CHECKPOINT_RETRIES: usize = 3;
const BUILD_CHECKPOINT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Clone, Copy)]
struct SearchIndexCounts {
    artists: usize,
    albums: usize,
    tracks: usize,
}

impl SearchIndexCounts {
    fn total(self) -> usize {
        self.artists
            .saturating_add(self.albums)
            .saturating_add(self.tracks)
    }

    fn from_catalog(catalog_store: &dyn CatalogStore) -> Result<Self> {
        Ok(catalog_store
            .get_catalog_cardinality_stats()?
            .map(|stats| Self {
                artists: stats.artists,
                albums: stats.albums,
                tracks: stats.tracks,
            })
            .unwrap_or_else(|| Self {
                artists: catalog_store.get_artists_count(),
                albums: catalog_store.get_albums_count(),
                tracks: catalog_store.get_tracks_count(),
            }))
    }
}

const DEFAULT_INDEX_BATCH_SIZE: usize = 50_000;

#[derive(Clone, Copy, Debug)]
pub struct SearchBuildOptions {
    pub batch_size: usize,
    pub preparation_threads: usize,
    /// Store only available entities in `item_availability`. Absence is
    /// equivalent to `is_available = 0` for every availability-filtered query.
    /// This is intended for offline builds where no legacy search traffic is
    /// using the table while a fresh build resets it.
    pub sparse_availability: bool,
    /// Replay mutations captured while a live catalog build was running.
    /// Offline builds from a static catalog snapshot can disable this because
    /// the snapshot already contains those mutations.
    pub replay_mutations: bool,
    /// Run FTS5's exhaustive structural integrity command before activation.
    /// Offline recovery can disable this after exact counts and bounded smoke
    /// tests because the command has no progress reporting on a full catalog.
    pub verify_fts_integrity: bool,
}

impl Default for SearchBuildOptions {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_INDEX_BATCH_SIZE,
            preparation_threads: 1,
            sparse_availability: false,
            replay_mutations: true,
            verify_fts_integrity: true,
        }
    }
}

struct PreparedSearchDocument {
    item_id: String,
    display_name: String,
    primary_name: String,
    artist_text: String,
    album_text: String,
    extra_text: String,
    is_available: bool,
}

const IMPRESSION_USER_DAILY_BUDGET: i64 = 500;
const IMPRESSION_DEVICE_DAILY_BUDGET: i64 = 200;

/// FTS5 search vault with Levenshtein-based typo correction.
///
/// This implementation builds a vocabulary from all indexed content and
/// uses Levenshtein distance to correct typos in search queries before
/// passing them to FTS5.
///
/// ## Usage
///
/// For blocking initialization (old behavior):
/// ```ignore
/// let vault = Fts5LevenshteinSearchVault::new(catalog_store, db_path, &db_registry)?;
/// ```
///
/// For non-blocking background indexing:
/// ```ignore
/// let vault = Fts5LevenshteinSearchVault::new_lazy(db_path, &db_registry)?;
/// vault.start_background_build(catalog_store);
/// // Search works immediately, returns partial results during build
/// ```
pub struct Fts5LevenshteinSearchVault {
    /// Read connection for search queries (separate from write to avoid blocking)
    read_conn: Mutex<Connection>,
    /// Write connection for indexing operations
    write_conn: Mutex<Connection>,
    vocabulary: RwLock<Vocabulary>,
    /// Maximum edit distance for typo correction (default: 2)
    max_edit_distance: usize,
    build_options: SearchBuildOptions,
    /// Current indexing state
    state: RwLock<IndexState>,
}

impl Fts5LevenshteinSearchVault {
    /// Bound build WAL growth at a batch boundary. A failed checkpoint stops
    /// the resumable build instead of allowing the WAL to grow without limit.
    fn checkpoint_build_wal(conn: &Connection) -> Result<()> {
        for attempt in 1..=BUILD_CHECKPOINT_RETRIES {
            let (busy, total, checkpointed): (i64, i64, i64) =
                conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
            if busy == 0 {
                info!(total, checkpointed, "Search build WAL checkpoint complete");
                return Ok(());
            }
            warn!(
                attempt,
                total, checkpointed, "Search build WAL checkpoint blocked"
            );
            if attempt < BUILD_CHECKPOINT_RETRIES {
                std::thread::sleep(BUILD_CHECKPOINT_RETRY_DELAY);
            }
        }
        bail!(
            "search build stopped because the WAL could not be truncated after {} attempts",
            BUILD_CHECKPOINT_RETRIES
        )
    }

    /// Create a new FTS5 + Levenshtein search vault (blocking).
    ///
    /// This constructor blocks until the index is fully built.
    /// For non-blocking initialization, use `new_lazy()` + `start_background_build()`.
    ///
    /// # Arguments
    /// * `catalog_store` - The catalog store to index content from
    /// * `db_path` - Path to the search database file
    pub fn new(
        catalog_store: Arc<dyn CatalogStore>,
        db_path: &Path,
        db_registry: &crate::backup::DbRegistry,
    ) -> Result<Self> {
        Self::with_max_distance(catalog_store, db_path, 2, db_registry)
    }

    /// Create a lazy vault that doesn't index on construction.
    ///
    /// The vault is immediately usable but returns empty results until
    /// `start_background_build()` is called and completes.
    ///
    /// If a valid index already exists on disk, it will be loaded.
    /// If a partial build was interrupted, it will be detected and can be resumed.
    pub fn new_lazy(db_path: &Path, db_registry: &crate::backup::DbRegistry) -> Result<Self> {
        Self::new_lazy_with_build_options(db_path, db_registry, SearchBuildOptions::default())
    }

    pub fn new_lazy_with_build_options(
        db_path: &Path,
        db_registry: &crate::backup::DbRegistry,
        build_options: SearchBuildOptions,
    ) -> Result<Self> {
        if build_options.batch_size == 0 {
            bail!("search build batch size must be greater than zero");
        }
        if build_options.preparation_threads == 0 {
            bail!("search build preparation thread count must be greater than zero");
        }
        // Create write connection first (handles table creation)
        let write_conn = Connection::open(db_path)?;
        crate::sqlite_persistence::configure_connection(&write_conn)?;
        db_registry.register(db_path.to_path_buf(), &write_conn)?;
        Self::create_tables(&write_conn)?;

        // Create separate read connection for search queries
        // This allows searches to proceed while writes are happening
        let read_conn = Connection::open(db_path)?;
        crate::sqlite_persistence::configure_connection(&read_conn)?;

        // Check build state
        let build_in_progress = Self::get_metadata(&write_conn, "build_in_progress")
            .map(|v| v == "true")
            .unwrap_or(false);
        let index_count = Self::get_index_item_count(&write_conn).unwrap_or(0);
        let active_schema_version = Self::get_metadata(&write_conn, "active_search_schema_version")
            .and_then(|value| value.parse::<i64>().ok());

        let (vocabulary, state) = if build_in_progress {
            // Partial build detected - load what we have and prepare to resume
            let build_offset = Self::get_metadata(&write_conn, "build_offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let build_total = Self::get_metadata(&write_conn, "build_total")
                .and_then(|v| v.parse::<usize>().ok());

            info!(
                "Detected partial build: {} items indexed, offset {}, total {:?}",
                index_count, build_offset, build_total
            );

            let vocab = if index_count > 0 {
                Self::load_vocabulary_from_index(&write_conn)?
            } else {
                Vocabulary::new()
            };

            (
                vocab,
                IndexState::Building {
                    processed: build_offset,
                    total: build_total,
                },
            )
        } else if index_count > 0
            && active_schema_version == Some(SEARCH_INDEX_SCHEMA_VERSION)
            && Self::is_enriched_index(&write_conn, ACTIVE_INDEX)?
        {
            info!(
                schema_version = SEARCH_INDEX_SCHEMA_VERSION,
                "Loading existing search index with {} items", index_count
            );
            let vocab = Self::load_vocabulary_from_index(&write_conn)?;
            (vocab, IndexState::Ready)
        } else {
            if index_count > 0 {
                info!(active_schema_version = ?active_schema_version, "Serving legacy search index while waiting for enriched background build");
                (
                    Self::load_vocabulary_from_index(&write_conn)?,
                    IndexState::Empty,
                )
            } else {
                info!("Search index is empty, waiting for background build");
                (Vocabulary::new(), IndexState::Empty)
            }
        };

        Ok(Self {
            read_conn: Mutex::new(read_conn),
            write_conn: Mutex::new(write_conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance: 2,
            build_options,
            state: RwLock::new(state),
        })
    }

    /// Create a new vault with custom max edit distance (blocking).
    pub fn with_max_distance(
        catalog_store: Arc<dyn CatalogStore>,
        db_path: &Path,
        max_edit_distance: usize,
        db_registry: &crate::backup::DbRegistry,
    ) -> Result<Self> {
        let write_conn = Connection::open(db_path)?;
        crate::sqlite_persistence::configure_connection(&write_conn)?;
        db_registry.register(db_path.to_path_buf(), &write_conn)?;
        Self::create_tables(&write_conn)?;

        // Create separate read connection
        let read_conn = Connection::open(db_path)?;
        crate::sqlite_persistence::configure_connection(&read_conn)?;

        // For Spotify catalog (static), version is always 0
        let current_catalog_version: i64 = 0;
        let expected_item_count = catalog_store
            .get_searchable_content()
            .map(|items| items.len())
            .unwrap_or(0);

        let needs_rebuild =
            Self::check_needs_rebuild(&write_conn, current_catalog_version, expected_item_count);

        let vocabulary = if needs_rebuild {
            Self::rebuild_index_internal(&write_conn, &catalog_store, current_catalog_version)?
        } else {
            Self::load_vocabulary_from_index(&write_conn)?
        };

        Ok(Self {
            read_conn: Mutex::new(read_conn),
            write_conn: Mutex::new(write_conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance,
            build_options: SearchBuildOptions::default(),
            state: RwLock::new(IndexState::Ready),
        })
    }

    /// Start building the index in the background.
    ///
    /// This method returns immediately. Progress can be monitored via `get_stats()`.
    /// Items are indexed in batches by popularity (most popular first), so common
    /// searches work quickly even before indexing completes.
    ///
    /// If a partial build was interrupted, this will resume from where it left off.
    /// If the index is already complete and ready, this is a no-op.
    pub fn start_background_build(self: &Arc<Self>, catalog_store: Arc<dyn CatalogStore>) {
        // Check current state and determine resume offset
        let resume_offset: Option<usize>;
        {
            let state = self.state.read().unwrap();
            match &*state {
                IndexState::Building { processed, .. } => {
                    // Resume from partial build
                    resume_offset = Some(*processed);
                    info!(
                        "Resuming partial build from offset {}",
                        resume_offset.unwrap()
                    );
                }
                IndexState::Ready => {
                    // Check if index is actually populated
                    let conn = self.write_conn.lock().unwrap();
                    let count = Self::get_index_item_count(&conn).unwrap_or(0);
                    if count > 0 {
                        let available_ready = Self::available_index_is_ready(&conn);
                        drop(conn);
                        if !available_ready {
                            info!(
                                "Full index already has {} items; building compact available index",
                                count
                            );
                            drop(state);
                            *self.state.write().unwrap() = IndexState::Building {
                                processed: 0,
                                total: None,
                            };
                            let vault = Arc::clone(self);
                            std::thread::spawn(move || {
                                if let Err(error) = vault.build_available_index(catalog_store) {
                                    error!(%error, "Compact available search index build failed");
                                    *vault.state.write().unwrap() = IndexState::Failed {
                                        error: error.to_string(),
                                    };
                                }
                            });
                            return;
                        }
                        info!("Index already has {} items, skipping build", count);
                        return;
                    }
                    resume_offset = None;
                }
                _ => {
                    resume_offset = None;
                }
            }
        }

        // Set state to building (if not already)
        if resume_offset.is_none() {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: 0,
                total: None,
            };
        }

        let vault = Arc::clone(self);

        std::thread::spawn(move || {
            if let Err(e) = vault.build_index_progressively(catalog_store, resume_offset) {
                error!("Background index build failed: {}", e);
                let mut state = vault.state.write().unwrap();
                *state = IndexState::Failed {
                    error: e.to_string(),
                };
            }
        });
    }

    /// Build the index progressively in batches.
    ///
    /// Items are fetched with a stable per-type rowid cursor. At most one batch
    /// is resident in memory, including across interrupted/resumed builds.
    fn build_index_progressively(
        &self,
        catalog_store: Arc<dyn CatalogStore>,
        resume_offset: Option<usize>,
    ) -> Result<()> {
        let mut start_offset = resume_offset.unwrap_or(0);
        if start_offset > 0 {
            let conn = self.write_conn.lock().unwrap();
            if !Self::table_exists(&conn, BUILD_INDEX)?
                || !Self::is_enriched_index(&conn, BUILD_INDEX)?
                || !Self::table_exists(&conn, AVAILABLE_BUILD_INDEX)?
            {
                warn!(start_offset, "Interrupted legacy build has no enriched side table; restarting migration build");
                start_offset = 0;
            }
        }
        if start_offset > 0 {
            info!(
                "Resuming progressive index build from offset {}...",
                start_offset
            );
        } else {
            info!("Starting progressive index build...");
        }

        let counts = SearchIndexCounts::from_catalog(catalog_store.as_ref())?;
        let total = counts.total();

        info!("Found {} items to index", total);

        // Update state with total
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: start_offset,
                total: Some(total),
            };
        }

        // Mark build as in progress and store total in metadata
        {
            let conn = self.write_conn.lock().unwrap();
            Self::set_metadata(&conn, "build_in_progress", "true")?;
            Self::set_metadata(&conn, "build_total", &total.to_string())?;

            // Build beside the active table so legacy/enriched queries continue
            // to be served until validation and activation complete.
            if start_offset == 0 {
                conn.execute_batch(&format!("DROP TABLE IF EXISTS {BUILD_INDEX};"))?;
                Self::create_enriched_index(&conn, BUILD_INDEX)?;
                conn.execute_batch(&format!("DROP TABLE IF EXISTS {AVAILABLE_BUILD_INDEX};"))?;
                Self::create_enriched_index(&conn, AVAILABLE_BUILD_INDEX)?;
                conn.execute("DELETE FROM search_index_mutations", [])?;
                if self.build_options.sparse_availability {
                    conn.execute("DELETE FROM item_availability", [])?;
                }
                Self::set_metadata(&conn, "build_offset", "0")?;
                Self::set_metadata(&conn, "build_entity_type", "artist")?;
                Self::set_metadata(&conn, "build_after_rowid", "0")?;
                Self::set_metadata(
                    &conn,
                    "building_search_schema_version",
                    &SEARCH_INDEX_SCHEMA_VERSION.to_string(),
                )?;
            }
        }

        let mut vocabulary = if start_offset > 0 {
            let conn = self.write_conn.lock().unwrap();
            Self::load_vocabulary_from_table(&conn, BUILD_INDEX)?
        } else {
            Vocabulary::new()
        };
        let preparation_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.build_options.preparation_threads)
            .thread_name(|index| format!("search-prepare-{index}"))
            .build()?;

        let mut processed = start_offset;
        let resume_type = {
            let conn = self.write_conn.lock().unwrap();
            Self::get_metadata(&conn, "build_entity_type").unwrap_or_else(|| "artist".into())
        };
        let phases = [
            ("artist", SearchableContentType::Artist),
            ("album", SearchableContentType::Album),
            ("track", SearchableContentType::Track),
        ];
        let start_phase = phases
            .iter()
            .position(|(name, _)| *name == resume_type)
            .unwrap_or(0);

        for (phase_index, (phase_name, content_type)) in phases.iter().enumerate().skip(start_phase)
        {
            let mut after_rowid = if phase_index == start_phase {
                let conn = self.write_conn.lock().unwrap();
                Self::get_metadata(&conn, "build_after_rowid")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            } else {
                0
            };

            loop {
                let page = catalog_store.get_searchable_content_page(
                    *content_type,
                    after_rowid,
                    self.build_options.batch_size,
                )?;
                if page.is_empty() {
                    break;
                }
                let next_rowid = page.last().map(|(rowid, _)| *rowid).unwrap_or(after_rowid);
                let batch_len = page.len();
                let prepared = preparation_pool.install(|| {
                    page.par_iter()
                        .map(|(_, item)| Self::prepare_searchable_item(item))
                        .collect::<Vec<_>>()
                });

                if vocabulary.len() < MAX_VOCABULARY_SOURCE_ROWS {
                    for item in &prepared {
                        vocabulary.add_text(&format!(
                            "{} {} {} {}",
                            item.primary_name, item.artist_text, item.album_text, item.extra_text
                        ));
                    }
                }

                let conn = self.write_conn.lock().unwrap();
                conn.execute("BEGIN IMMEDIATE", [])?;
                let insert_result = (|| -> Result<()> {
                    let mut stmt = conn.prepare(
                        "INSERT INTO search_index_building
                         (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                         VALUES(?,?,?,?,?,?,?)",
                    )?;
                    let mut avail_stmt = conn.prepare(
                        "INSERT OR REPLACE INTO item_availability
                         (item_id,item_type,is_available) VALUES(?,?,?)",
                    )?;
                    let mut available_index_stmt = conn.prepare(&format!(
                        "INSERT INTO {AVAILABLE_BUILD_INDEX}
                         (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                         VALUES(?,?,?,?,?,?,?)"
                    ))?;
                    for item in &prepared {
                        stmt.execute(rusqlite::params![
                            &item.item_id,
                            phase_name,
                            &item.display_name,
                            &item.primary_name,
                            &item.artist_text,
                            &item.album_text,
                            &item.extra_text
                        ])?;
                        if !self.build_options.sparse_availability || item.is_available {
                            avail_stmt.execute(rusqlite::params![
                                &item.item_id,
                                phase_name,
                                if item.is_available { 1 } else { 0 }
                            ])?;
                        }
                        if item.is_available {
                            available_index_stmt.execute(rusqlite::params![
                                &item.item_id,
                                phase_name,
                                &item.display_name,
                                &item.primary_name,
                                &item.artist_text,
                                &item.album_text,
                                &item.extra_text
                            ])?;
                        }
                    }
                    Self::set_metadata(&conn, "build_entity_type", phase_name)?;
                    Self::set_metadata(&conn, "build_after_rowid", &next_rowid.to_string())?;
                    Self::set_metadata(
                        &conn,
                        "build_offset",
                        &(processed + batch_len).to_string(),
                    )?;
                    Ok(())
                })();
                match insert_result {
                    Ok(()) => {
                        conn.execute("COMMIT", [])?;
                        Self::checkpoint_build_wal(&conn)?;
                    }
                    Err(error) => {
                        let _ = conn.execute("ROLLBACK", []);
                        return Err(error);
                    }
                }
                drop(conn);

                processed += batch_len;
                after_rowid = next_rowid;
                *self.state.write().unwrap() = IndexState::Building {
                    processed,
                    total: Some(total),
                };
                info!(
                    entity_type = phase_name,
                    after_rowid,
                    processed,
                    total,
                    progress_percent = if total == 0 {
                        100.0
                    } else {
                        processed as f64 / total as f64 * 100.0
                    },
                    "Search index build progress"
                );
            }

            if let Some((next_name, _)) = phases.get(phase_index + 1) {
                let conn = self.write_conn.lock().unwrap();
                Self::set_metadata(&conn, "build_entity_type", next_name)?;
                Self::set_metadata(&conn, "build_after_rowid", "0")?;
            }
        }

        // Validate and atomically activate. Active queries keep using the old
        // table until this transaction commits.
        {
            // The catalog can change during a full build. Re-read its counts
            // immediately before replaying the mutation journal and swapping.
            let final_counts = SearchIndexCounts::from_catalog(catalog_store.as_ref())?;
            let conn = self.write_conn.lock().unwrap();
            if !self.build_options.replay_mutations {
                let discarded = conn.execute("DELETE FROM search_index_mutations", [])?;
                info!(
                    discarded,
                    "Discarded mutation journal for static-catalog offline build"
                );
            }
            Self::activate_built_index_counts(
                &conn,
                final_counts,
                self.build_options.verify_fts_integrity,
            )?;
        }

        *self.vocabulary.write().unwrap() = vocabulary.clone();

        // Mark as ready
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Ready;
        }

        info!(
            schema_version = SEARCH_INDEX_SCHEMA_VERSION,
            "Index build complete and activated: {} items, vocabulary has {} words",
            total,
            vocabulary.len()
        );

        Ok(())
    }

    /// Upgrade an already-enriched full index by building only the much
    /// smaller playable subset. This deliberately reads filtered catalog pages
    /// and never walks the 330M-row full FTS table.
    fn build_available_index(&self, catalog_store: Arc<dyn CatalogStore>) -> Result<()> {
        {
            let conn = self.write_conn.lock().unwrap();
            conn.execute_batch(&format!("DROP TABLE IF EXISTS {AVAILABLE_BUILD_INDEX};"))?;
            Self::create_enriched_index(&conn, AVAILABLE_BUILD_INDEX)?;
            Self::set_metadata(&conn, "available_index_build_in_progress", "true")?;
        }

        let phases = [
            ("artist", SearchableContentType::Artist),
            ("album", SearchableContentType::Album),
            ("track", SearchableContentType::Track),
        ];
        let mut total = 0usize;
        for (phase_name, content_type) in phases {
            let mut after_rowid = 0i64;
            loop {
                let page = catalog_store.get_available_searchable_content_page(
                    content_type,
                    after_rowid,
                    self.build_options.batch_size,
                )?;
                if page.is_empty() {
                    break;
                }
                after_rowid = page.last().map(|(rowid, _)| *rowid).unwrap_or(after_rowid);
                let prepared = page
                    .iter()
                    .map(|(_, item)| Self::prepare_searchable_item(item))
                    .collect::<Vec<_>>();
                let conn = self.write_conn.lock().unwrap();
                conn.execute("BEGIN IMMEDIATE", [])?;
                let insert_result = (|| -> Result<()> {
                    let mut stmt = conn.prepare(&format!(
                        "INSERT INTO {AVAILABLE_BUILD_INDEX}
                         (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                         VALUES(?,?,?,?,?,?,?)"
                    ))?;
                    for item in &prepared {
                        stmt.execute(rusqlite::params![
                            &item.item_id,
                            phase_name,
                            &item.display_name,
                            &item.primary_name,
                            &item.artist_text,
                            &item.album_text,
                            &item.extra_text
                        ])?;
                    }
                    Ok(())
                })();
                match insert_result {
                    Ok(()) => {
                        conn.execute("COMMIT", [])?;
                    }
                    Err(error) => {
                        let _ = conn.execute("ROLLBACK", []);
                        return Err(error);
                    }
                }
                total += prepared.len();
                *self.state.write().unwrap() = IndexState::Building {
                    processed: total,
                    total: None,
                };
                info!(
                    entity_type = phase_name,
                    total, "Available search index build progress"
                );
            }
        }

        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let swap_result = (|| -> Result<()> {
            if Self::table_exists(&conn, AVAILABLE_PREVIOUS_INDEX)? {
                conn.execute_batch(&format!("DROP TABLE {AVAILABLE_PREVIOUS_INDEX};"))?;
            }
            conn.execute_batch(&format!(
                "ALTER TABLE {AVAILABLE_INDEX} RENAME TO {AVAILABLE_PREVIOUS_INDEX};
                 ALTER TABLE {AVAILABLE_BUILD_INDEX} RENAME TO {AVAILABLE_INDEX};
                 DROP TABLE {AVAILABLE_PREVIOUS_INDEX};"
            ))?;
            Self::set_metadata(
                &conn,
                "available_index_schema_version",
                &AVAILABLE_INDEX_SCHEMA_VERSION.to_string(),
            )?;
            Self::set_metadata(&conn, "available_index_item_count", &total.to_string())?;
            Self::delete_metadata(&conn, "available_index_build_in_progress")?;
            Ok(())
        })();
        match swap_result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
            }
            Err(error) => {
                let _ = conn.execute("ROLLBACK", []);
                return Err(error);
            }
        }
        info!(total, "Compact available search index activated");
        *self.state.write().unwrap() = IndexState::Ready;
        Ok(())
    }

    /// Check if the search index needs to be rebuilt.
    fn check_needs_rebuild(
        conn: &Connection,
        _current_catalog_version: i64,
        _expected_item_count: usize,
    ) -> bool {
        let schema_version = Self::get_metadata(conn, "active_search_schema_version")
            .and_then(|value| value.parse::<i64>().ok());
        let structurally_enriched = Self::is_enriched_index(conn, ACTIVE_INDEX).unwrap_or(false);
        let needs_rebuild =
            schema_version != Some(SEARCH_INDEX_SCHEMA_VERSION) || !structurally_enriched;
        if needs_rebuild {
            info!(
                active_schema_version = ?schema_version,
                target_schema_version = SEARCH_INDEX_SCHEMA_VERSION,
                "Search index schema migration required"
            );
        }
        needs_rebuild
    }

    /// Create all required tables
    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_popularity (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                play_count INTEGER NOT NULL DEFAULT 0,
                score REAL NOT NULL DEFAULT 0.0,
                listening_score REAL NOT NULL DEFAULT 0.0,
                impression_score REAL NOT NULL DEFAULT 0.0,
                spotify_score REAL NOT NULL DEFAULT 0.0,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_popularity_type ON item_popularity(item_type);
        "#,
        )?;

        // Create item_impressions table for tracking page views
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_impressions (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                date INTEGER NOT NULL,
                impression_count INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (item_id, item_type, date)
            );
            CREATE INDEX IF NOT EXISTS idx_impressions_date ON item_impressions(date);
            CREATE INDEX IF NOT EXISTS idx_impressions_item ON item_impressions(item_id, item_type);

            CREATE TABLE IF NOT EXISTS item_impression_events (
                user_id INTEGER NOT NULL,
                device_id INTEGER NOT NULL,
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                date INTEGER NOT NULL,
                PRIMARY KEY (user_id, device_id, item_id, item_type, date)
            );
            CREATE INDEX IF NOT EXISTS idx_impression_events_user_date
                ON item_impression_events(user_id, date);
            CREATE INDEX IF NOT EXISTS idx_impression_events_device_date
                ON item_impression_events(user_id, device_id, date);
        "#,
        )?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS search_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS search_index_mutations (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                operation TEXT NOT NULL,
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                display_name TEXT,
                primary_name TEXT,
                artist_text TEXT,
                album_text TEXT,
                extra_text TEXT
            );
        "#,
        )?;

        // Create item_availability table for availability filtering
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_availability (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                is_available INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_availability_lookup
                ON item_availability(item_id, item_type, is_available);
        "#,
        )?;

        if !Self::table_exists(conn, ACTIVE_INDEX)? {
            Self::create_enriched_index(conn, ACTIVE_INDEX)?;
            info!(
                schema_version = SEARCH_INDEX_SCHEMA_VERSION,
                "Created empty enriched search index"
            );
        }
        if !Self::table_exists(conn, AVAILABLE_INDEX)? {
            Self::create_enriched_index(conn, AVAILABLE_INDEX)?;
            info!("Created empty compact available search index");
        }

        Self::maintain_previous_index(conn)?;

        Ok(())
    }

    fn maintain_previous_index(conn: &Connection) -> Result<()> {
        if !Self::table_exists(conn, PREVIOUS_INDEX)? {
            return Ok(());
        }

        // A structurally unhealthy new index is rolled back before it can be
        // advertised as healthy on restart.
        // Startup validation must be bounded for the full catalog. Exhaustive
        // FTS integrity checking is an explicit build-time option; here we only
        // verify that the enriched virtual table can be read before retaining
        // it through the healthy-restart rollback window.
        let active_is_healthy = Self::is_enriched_index(conn, ACTIVE_INDEX)?
            && conn
                .query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM {ACTIVE_INDEX} LIMIT 1)"),
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .is_ok();
        if !active_is_healthy {
            warn!("Active search index failed restart validation; rolling back to previous table");
            conn.execute("BEGIN IMMEDIATE", [])?;
            let rollback_result = (|| -> Result<()> {
                conn.execute_batch(&format!(
                    "DROP TABLE IF EXISTS search_index_failed;
                     ALTER TABLE {ACTIVE_INDEX} RENAME TO search_index_failed;
                     ALTER TABLE {PREVIOUS_INDEX} RENAME TO {ACTIVE_INDEX};"
                ))?;
                if Self::table_exists(conn, AVAILABLE_PREVIOUS_INDEX)? {
                    conn.execute_batch(&format!(
                        "DROP TABLE IF EXISTS search_index_available_failed;
                         ALTER TABLE {AVAILABLE_INDEX} RENAME TO search_index_available_failed;
                         ALTER TABLE {AVAILABLE_PREVIOUS_INDEX} RENAME TO {AVAILABLE_INDEX};"
                    ))?;
                }
                Self::delete_metadata(conn, "active_search_schema_version")?;
                if let Some(previous_count) = Self::get_metadata(conn, "previous_index_item_count")
                {
                    Self::set_metadata(conn, "active_index_item_count", &previous_count)?;
                } else {
                    Self::delete_metadata(conn, "active_index_item_count")?;
                }
                Self::delete_metadata(conn, "previous_index_item_count")?;
                Self::delete_metadata(conn, "previous_index_retained")?;
                Ok(())
            })();
            return match rollback_result {
                Ok(()) => {
                    conn.execute("COMMIT", [])?;
                    Ok(())
                }
                Err(error) => {
                    let _ = conn.execute("ROLLBACK", []);
                    Err(error)
                }
            };
        }

        match Self::get_metadata(conn, "previous_index_retained").as_deref() {
            Some("pending_restart") => {
                Self::set_metadata(conn, "previous_index_retained", "healthy_restart")?;
                info!("Previous search index retained through healthy restart");
            }
            Some("healthy_restart") => {
                conn.execute_batch(&format!("DROP TABLE {PREVIOUS_INDEX};"))?;
                if Self::table_exists(conn, AVAILABLE_PREVIOUS_INDEX)? {
                    conn.execute_batch(&format!("DROP TABLE {AVAILABLE_PREVIOUS_INDEX};"))?;
                }
                Self::delete_metadata(conn, "previous_index_retained")?;
                Self::delete_metadata(conn, "previous_index_item_count")?;
                info!("Removed previous search index after healthy restart");
            }
            _ => {}
        }
        Ok(())
    }

    fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )?)
    }

    fn is_enriched_index(conn: &Connection, table: &str) -> Result<bool> {
        if !Self::table_exists(conn, table)? {
            return Ok(false);
        }
        let sql: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name = ?1",
            [table],
            |row| row.get(0),
        )?;
        Ok(sql.contains("primary_name") && sql.contains("artist_text"))
    }

    fn available_index_is_ready(conn: &Connection) -> bool {
        Self::get_metadata(conn, "available_index_schema_version")
            .and_then(|value| value.parse::<i64>().ok())
            == Some(AVAILABLE_INDEX_SCHEMA_VERSION)
            && Self::is_enriched_index(conn, AVAILABLE_INDEX).unwrap_or(false)
    }

    fn create_enriched_index(conn: &Connection, table: &str) -> Result<()> {
        // Table names are constants controlled by this module, never user input.
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING fts5(
                item_id UNINDEXED,
                item_type UNINDEXED,
                display_name UNINDEXED,
                primary_name,
                artist_text,
                album_text,
                extra_text,
                tokenize='unicode61 remove_diacritics 2'
            );"
        ))?;
        Ok(())
    }

    /// Get a metadata value by key
    fn get_metadata(conn: &Connection, key: &str) -> Option<String> {
        conn.query_row(
            "SELECT value FROM search_metadata WHERE key = ?",
            [key],
            |row| row.get(0),
        )
        .ok()
    }

    /// Set a metadata value
    fn set_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO search_metadata (key, value) VALUES (?, ?)",
            [key, value],
        )?;
        Ok(())
    }

    /// Delete a metadata key
    fn delete_metadata(conn: &Connection, key: &str) -> Result<()> {
        conn.execute("DELETE FROM search_metadata WHERE key = ?", [key])?;
        Ok(())
    }

    /// Get the number of items in the search index
    fn get_index_item_count(conn: &Connection) -> Option<usize> {
        if let Some(count) =
            Self::get_metadata(conn, "active_index_item_count").and_then(|value| value.parse().ok())
        {
            return Some(count);
        }
        conn.query_row("SELECT COUNT(*) FROM search_index", [], |row| {
            let count: i64 = row.get(0)?;
            Ok(count as usize)
        })
        .ok()
    }

    /// Store the catalog version in metadata
    fn set_stored_catalog_version(conn: &Connection, version: i64) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO search_metadata (key, value) VALUES ('catalog_version', ?)",
            [version.to_string()],
        )?;
        Ok(())
    }

    /// Load vocabulary from existing index (without rebuilding)
    fn load_vocabulary_from_index(conn: &Connection) -> Result<Vocabulary> {
        Self::load_vocabulary_from_table(conn, ACTIVE_INDEX)
    }

    fn load_vocabulary_from_table(conn: &Connection, table: &str) -> Result<Vocabulary> {
        let mut vocabulary = Vocabulary::new();
        let enriched = Self::is_enriched_index(conn, table)?;
        let (source, text_expression) = if enriched {
            // FTS columns are stored in c0..c6 in the content shadow table.
            // Reading 500k UNINDEXED/content columns through the virtual table
            // performs one lookup per row and makes startup take minutes.
            (
                format!("{table}_content"),
                "c3 || ' ' || c4 || ' ' || c5 || ' ' || c6",
            )
        } else {
            (table.to_string(), "name")
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT {text_expression} FROM {source} LIMIT {MAX_VOCABULARY_SOURCE_ROWS}"
        ))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })?;

        for name in rows.flatten() {
            vocabulary.add_text(&name);
        }

        debug!(
            "Loaded vocabulary with {} words from existing index",
            vocabulary.len()
        );
        Ok(vocabulary)
    }

    fn normalize_text(text: &str) -> String {
        fn fold(character: char) -> char {
            match character {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
                'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
                'ď' | 'đ' => 'd',
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
                'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
                'ĥ' | 'ħ' => 'h',
                'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
                'ĵ' => 'j',
                'ķ' => 'k',
                'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
                'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
                'ŕ' | 'ŗ' | 'ř' => 'r',
                'ś' | 'ŝ' | 'ş' | 'š' => 's',
                'ţ' | 'ť' | 'ŧ' => 't',
                'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
                'ŵ' => 'w',
                'ý' | 'ÿ' | 'ŷ' => 'y',
                'ź' | 'ż' | 'ž' => 'z',
                _ => character,
            }
        }

        let mut normalized = String::with_capacity(text.len());
        let mut last_was_space = true;
        for character in text.to_lowercase().chars().map(fold) {
            if character.is_alphanumeric() {
                normalized.push(character);
                last_was_space = false;
            } else if !last_was_space {
                normalized.push(' ');
                last_was_space = true;
            }
        }
        if normalized.ends_with(' ') {
            normalized.pop();
        }
        normalized
    }

    fn document_parts(
        item: &crate::catalog_store::SearchableItem,
    ) -> (String, String, String, String) {
        Self::metadata_document_parts(&item.name, &item.additional_text)
    }

    fn prepare_searchable_item(
        item: &crate::catalog_store::SearchableItem,
    ) -> PreparedSearchDocument {
        let (primary_name, artist_text, album_text, extra_text) = Self::document_parts(item);
        PreparedSearchDocument {
            item_id: item.id.clone(),
            display_name: item.name.clone(),
            primary_name,
            artist_text,
            album_text,
            extra_text,
            is_available: item.is_available,
        }
    }

    fn index_item_document_parts(item: &SearchIndexItem) -> (String, String, String, String) {
        Self::metadata_document_parts(&item.name, &item.additional_text)
    }

    fn metadata_document_parts(
        name: &str,
        additional_text: &[String],
    ) -> (String, String, String, String) {
        let mut artists = Vec::new();
        let mut albums = Vec::new();
        let mut extras = Vec::new();
        for value in additional_text {
            if let Some(value) = value.strip_prefix("artist:") {
                artists.push(value);
            } else if let Some(value) = value.strip_prefix("album:") {
                albums.push(value);
            } else if let Some(value) = value.strip_prefix("extra:") {
                extras.push(value);
            } else {
                extras.push(value);
            }
        }
        (
            Self::normalize_text(name),
            Self::normalize_text(&artists.join(" ")),
            Self::normalize_text(&albums.join(" ")),
            Self::normalize_text(&extras.join(" ")),
        )
    }

    fn insert_searchable_item(
        stmt: &mut rusqlite::Statement<'_>,
        item: &crate::catalog_store::SearchableItem,
    ) -> Result<()> {
        let type_str = match item.content_type {
            SearchableContentType::Artist => "artist",
            SearchableContentType::Album => "album",
            SearchableContentType::Track => "track",
        };
        let (primary, artist, album, extra) = Self::document_parts(item);
        stmt.execute(rusqlite::params![
            &item.id, type_str, &item.name, primary, artist, album, extra
        ])?;
        Ok(())
    }

    /// Internal rebuild that works with a connection reference (blocking).
    fn rebuild_index_internal(
        conn: &Connection,
        catalog_store: &Arc<dyn CatalogStore>,
        catalog_version: i64,
    ) -> Result<Vocabulary> {
        let searchable = catalog_store.get_searchable_content()?;
        let count = searchable.len();

        let mut vocabulary = Vocabulary::new();
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {BUILD_INDEX};"))?;
        Self::create_enriched_index(conn, BUILD_INDEX)?;
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {AVAILABLE_BUILD_INDEX};"))?;
        Self::create_enriched_index(conn, AVAILABLE_BUILD_INDEX)?;
        conn.execute("DELETE FROM search_index_mutations", [])?;

        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            conn.execute("DELETE FROM item_availability", [])?;
            let mut stmt = conn.prepare(
                "INSERT INTO search_index_building
                 (item_id, item_type, display_name, primary_name, artist_text, album_text, extra_text)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )?;
            let mut avail_stmt = conn.prepare(
                "INSERT OR REPLACE INTO item_availability (item_id, item_type, is_available) VALUES (?, ?, ?)",
            )?;
            let mut available_index_stmt = conn.prepare(&format!(
                "INSERT INTO {AVAILABLE_BUILD_INDEX}
                 (item_id, item_type, display_name, primary_name, artist_text, album_text, extra_text)
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            ))?;

            for item in &searchable {
                let (primary, artist, album, extra) = Self::document_parts(item);
                vocabulary.add_text(&format!("{primary} {artist} {album} {extra}"));
                let type_str = match item.content_type {
                    SearchableContentType::Artist => "artist",
                    SearchableContentType::Album => "album",
                    SearchableContentType::Track => "track",
                };
                Self::insert_searchable_item(&mut stmt, item)?;
                if item.is_available {
                    available_index_stmt.execute(rusqlite::params![
                        &item.id, type_str, &item.name, primary, artist, album, extra
                    ])?;
                }
                avail_stmt.execute(rusqlite::params![
                    &item.id,
                    type_str,
                    if item.is_available { 1 } else { 0 }
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Self::activate_built_index(conn, &searchable)?;
                Self::set_stored_catalog_version(conn, catalog_version)?;
                info!(
                    schema_version = SEARCH_INDEX_SCHEMA_VERSION,
                    "Search index built with {} items, vocabulary has {} words",
                    count,
                    vocabulary.len()
                );
                Ok(vocabulary)
            }
            Err(e) => {
                if let Err(rollback_err) = conn.execute("ROLLBACK", []) {
                    warn!("Failed to rollback transaction: {}", rollback_err);
                }
                Err(e)
            }
        }
    }

    fn activate_built_index(
        conn: &Connection,
        expected: &[crate::catalog_store::SearchableItem],
    ) -> Result<()> {
        Self::activate_built_index_counts(
            conn,
            SearchIndexCounts {
                artists: expected
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Artist)
                    .count(),
                albums: expected
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Album)
                    .count(),
                tracks: expected
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Track)
                    .count(),
            },
            true,
        )
    }

    fn activate_built_index_counts(
        conn: &Connection,
        expected: SearchIndexCounts,
        verify_fts_integrity: bool,
    ) -> Result<()> {
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            // Compatibility for an interrupted v2 side-build (and older test
            // fixtures): derive the compact side table once if the dual-index
            // builder had not created it yet.
            if !Self::table_exists(conn, AVAILABLE_BUILD_INDEX)? {
                Self::create_enriched_index(conn, AVAILABLE_BUILD_INDEX)?;
                conn.execute_batch(&format!(
                    "INSERT INTO {AVAILABLE_BUILD_INDEX}
                     (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                     SELECT s.item_id,s.item_type,s.display_name,s.primary_name,
                            s.artist_text,s.album_text,s.extra_text
                     FROM {BUILD_INDEX} s
                     INNER JOIN item_availability a
                       ON a.item_id = s.item_id AND a.item_type = s.item_type
                      AND a.is_available = 1;"
                ))?;
            }
            // Replay writes captured after the build snapshot was read.
            let mutation_count: usize =
                conn.query_row("SELECT COUNT(*) FROM search_index_mutations", [], |row| {
                    row.get(0)
                })?;
            info!(mutation_count, "Replaying search index mutation journal");
            let mut mutations = conn.prepare(
                "SELECT operation, item_id, item_type, display_name, primary_name,
                        artist_text, album_text, extra_text
                 FROM search_index_mutations ORDER BY sequence",
            )?;
            let rows = mutations.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?;
            for row in rows {
                let (operation, id, item_type, display, primary, artist, album, extra) = row?;
                conn.execute(
                    &format!("DELETE FROM {BUILD_INDEX} WHERE item_id = ?1 AND item_type = ?2"),
                    rusqlite::params![id, item_type],
                )?;
                if operation == "upsert" {
                    conn.execute(
                        &format!("INSERT INTO {BUILD_INDEX}
                          (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                          VALUES(?1,?2,?3,?4,?5,?6,?7)"),
                        rusqlite::params![id, item_type, display, primary, artist, album, extra],
                    )?;
                }
                conn.execute(
                    &format!(
                        "DELETE FROM {AVAILABLE_BUILD_INDEX} WHERE item_id = ?1 AND item_type = ?2"
                    ),
                    rusqlite::params![id, item_type],
                )?;
                let is_available = conn
                    .query_row(
                        "SELECT is_available FROM item_availability
                         WHERE item_id = ?1 AND item_type = ?2",
                        rusqlite::params![id, item_type],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                    != 0;
                if operation == "upsert" && is_available {
                    conn.execute(
                        &format!(
                            "INSERT INTO {AVAILABLE_BUILD_INDEX}
                             (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                             VALUES(?1,?2,?3,?4,?5,?6,?7)"
                        ),
                        rusqlite::params![id, item_type, display, primary, artist, album, extra],
                    )?;
                }
            }
            drop(mutations);
            info!(mutation_count, "Search index mutation replay complete");

            // Validate the final candidate after mutation replay. Doing this
            // against the pre-replay snapshot rejects legitimate additions
            // made while a multi-hour build is running.
            // Reading an UNINDEXED column through the FTS virtual table causes
            // one content-table lookup per document. On a full catalog, doing
            // that once per type turns validation into trillions of bytes of
            // logical reads. The content shadow table contains the same values;
            // aggregate all three types and the total in one sequential scan.
            info!("Validating search index document counts");
            let (actual_artists, actual_albums, actual_tracks, actual_total): (
                usize,
                usize,
                usize,
                usize,
            ) = conn.query_row(
                &format!(
                    "SELECT
                         COALESCE(SUM(c1 = 'artist'), 0),
                         COALESCE(SUM(c1 = 'album'), 0),
                         COALESCE(SUM(c1 = 'track'), 0),
                         COUNT(*)
                     FROM {BUILD_INDEX_CONTENT}"
                ),
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            let actual_counts = [
                ("artist", expected.artists, actual_artists),
                ("album", expected.albums, actual_albums),
                ("track", expected.tracks, actual_tracks),
            ];
            for (item_type, expected_count, actual) in actual_counts {
                if actual != expected_count {
                    warn!(
                        item_type,
                        expected_count, actual, "Search index validation failed"
                    );
                    anyhow::bail!("search index {item_type} count mismatch: expected {expected_count}, got {actual}");
                }
            }
            if actual_total != expected.total() {
                warn!(
                    expected_total = expected.total(),
                    actual_total, "Search index total validation failed"
                );
                anyhow::bail!(
                    "search index total count mismatch: expected {}, got {actual_total}",
                    expected.total()
                );
            }
            info!(
                actual_artists,
                actual_albums, actual_tracks, actual_total, "Search index document counts valid"
            );
            if verify_fts_integrity {
                info!("Running FTS structural integrity check");
                conn.execute(
                    &format!(
                        // Exact document counts were validated above. Rank 0
                        // verifies the FTS structure without re-tokenizing all
                        // 330M content rows a second time.
                        "INSERT INTO {BUILD_INDEX}({BUILD_INDEX}, rank) VALUES('integrity-check', 0)"
                    ),
                    [],
                )?;
                info!("FTS structural integrity check complete");
            } else {
                warn!("Skipping exhaustive FTS integrity check for offline recovery build");
            }

            if Self::table_exists(conn, PREVIOUS_INDEX)? {
                conn.execute_batch(&format!("DROP TABLE {PREVIOUS_INDEX};"))?;
            }
            let previous_count = Self::get_index_item_count(conn).unwrap_or(0);
            if !Self::table_exists(conn, AVAILABLE_BUILD_INDEX)? {
                bail!("compact available search index build table is missing");
            }
            if Self::table_exists(conn, AVAILABLE_PREVIOUS_INDEX)? {
                conn.execute_batch(&format!("DROP TABLE {AVAILABLE_PREVIOUS_INDEX};"))?;
            }
            conn.execute_batch(&format!(
                "ALTER TABLE {ACTIVE_INDEX} RENAME TO {PREVIOUS_INDEX};
                 ALTER TABLE {BUILD_INDEX} RENAME TO {ACTIVE_INDEX};
                 ALTER TABLE {AVAILABLE_INDEX} RENAME TO {AVAILABLE_PREVIOUS_INDEX};
                 ALTER TABLE {AVAILABLE_BUILD_INDEX} RENAME TO {AVAILABLE_INDEX};"
            ))?;
            Self::set_metadata(
                conn,
                "active_search_schema_version",
                &SEARCH_INDEX_SCHEMA_VERSION.to_string(),
            )?;
            let available_count: usize = conn.query_row(
                &format!("SELECT COUNT(*) FROM {AVAILABLE_INDEX}"),
                [],
                |row| row.get(0),
            )?;
            Self::set_metadata(
                conn,
                "available_index_schema_version",
                &AVAILABLE_INDEX_SCHEMA_VERSION.to_string(),
            )?;
            Self::set_metadata(
                conn,
                "available_index_item_count",
                &available_count.to_string(),
            )?;
            Self::set_metadata(
                conn,
                "active_index_item_count",
                &expected.total().to_string(),
            )?;
            Self::set_metadata(
                conn,
                "previous_index_item_count",
                &previous_count.to_string(),
            )?;
            Self::set_metadata(conn, "previous_index_retained", "pending_restart")?;
            Self::delete_metadata(conn, "building_search_schema_version")?;
            Self::delete_metadata(conn, "build_in_progress")?;
            Self::delete_metadata(conn, "build_offset")?;
            Self::delete_metadata(conn, "build_total")?;
            Self::delete_metadata(conn, "build_entity_type")?;
            Self::delete_metadata(conn, "build_after_rowid")?;
            conn.execute("DELETE FROM search_index_mutations", [])?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                info!(
                    schema_version = SEARCH_INDEX_SCHEMA_VERSION,
                    "Search index swap complete"
                );
                Ok(())
            }
            Err(error) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }

    /// Rebuild the search index from the catalog (for trait impl).
    pub fn rebuild_index(&self) -> Result<()> {
        // This is the blocking rebuild - mainly for tests
        // In production, use start_background_build()
        warn!("rebuild_index() called - this blocks. Consider using start_background_build()");

        let mut state = self.state.write().unwrap();
        *state = IndexState::Ready;
        Ok(())
    }

    /// Update popularity scores for items.
    pub fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        let conn = self.write_conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut stmt = match conn.prepare(
            "INSERT OR REPLACE INTO item_popularity (item_id, item_type, play_count, score, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to prepare popularity update statement: {}", e);
                return;
            }
        };

        for (id, item_type, play_count, score) in items {
            if let Err(e) = stmt.execute(rusqlite::params![
                id,
                Self::item_type_to_str(item_type),
                *play_count as i64,
                score,
                now
            ]) {
                warn!("Failed to update popularity for {}: {}", id, e);
            }
        }

        debug!("Updated popularity scores for {} items", items.len());
    }

    /// Add or update items in the search index.
    pub fn upsert_items(&self, items: &[super::SearchIndexItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            let enriched = Self::is_enriched_index(&conn, ACTIVE_INDEX)?;
            for item in items {
                let type_str = Self::item_type_to_str(&item.item_type);
                conn.execute(
                    "DELETE FROM search_index WHERE item_id = ?1 AND item_type = ?2",
                    rusqlite::params![item.id, type_str],
                )?;
                let (primary, artist, album, extra) = Self::index_item_document_parts(item);
                if enriched {
                    conn.execute(
                        "INSERT INTO search_index
                         (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                         VALUES(?1,?2,?3,?4,?5,?6,?7)",
                        rusqlite::params![item.id, type_str, item.name, primary, artist, album, extra],
                    )?;
                    conn.execute(
                        &format!(
                            "DELETE FROM {AVAILABLE_INDEX} WHERE item_id = ?1 AND item_type = ?2"
                        ),
                        rusqlite::params![item.id, type_str],
                    )?;
                    let is_available = conn
                        .query_row(
                            "SELECT is_available FROM item_availability
                             WHERE item_id = ?1 AND item_type = ?2",
                            rusqlite::params![item.id, type_str],
                            |row| row.get::<_, i64>(0),
                        )
                        .unwrap_or(0)
                        != 0;
                    if is_available {
                        conn.execute(
                            &format!(
                                "INSERT INTO {AVAILABLE_INDEX}
                                 (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                                 VALUES(?1,?2,?3,?4,?5,?6,?7)"
                            ),
                            rusqlite::params![
                                item.id, type_str, item.name, primary, artist, album, extra
                            ],
                        )?;
                    }
                } else {
                    conn.execute(
                        "INSERT INTO search_index(item_id,item_type,name) VALUES(?1,?2,?3)",
                        rusqlite::params![item.id, type_str, item.name],
                    )?;
                }
                if Self::get_metadata(&conn, "build_in_progress").as_deref() == Some("true") {
                    conn.execute(
                        "INSERT INTO search_index_mutations
                         (operation,item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                         VALUES('upsert',?1,?2,?3,?4,?5,?6,?7)",
                        rusqlite::params![item.id, type_str, item.name, primary, artist, album, extra],
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;

                let mut vocabulary = self.vocabulary.write().unwrap();
                for item in items {
                    vocabulary.add_text(&item.name);
                }

                info!("Upserted {} items in search index", items.len());
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Remove items from the search index.
    pub fn remove_items(&self, items: &[(String, HashedItemType)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            for (id, item_type) in items {
                let type_str = Self::item_type_to_str(item_type);
                conn.execute(
                    "DELETE FROM search_index WHERE item_id = ?1 AND item_type = ?2",
                    rusqlite::params![id, type_str],
                )?;
                if Self::table_exists(&conn, AVAILABLE_INDEX)? {
                    conn.execute(
                        &format!(
                            "DELETE FROM {AVAILABLE_INDEX} WHERE item_id = ?1 AND item_type = ?2"
                        ),
                        rusqlite::params![id, type_str],
                    )?;
                }
                if Self::get_metadata(&conn, "build_in_progress").as_deref() == Some("true") {
                    conn.execute(
                        "INSERT INTO search_index_mutations(operation,item_id,item_type)
                         VALUES('remove',?1,?2)",
                        rusqlite::params![id, type_str],
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                info!("Removed {} items from search index", items.len());
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn item_type_to_str(item_type: &HashedItemType) -> &'static str {
        match item_type {
            HashedItemType::Artist => "artist",
            HashedItemType::Album => "album",
            HashedItemType::Track => "track",
        }
    }

    fn str_to_item_type(s: &str) -> Option<HashedItemType> {
        match s {
            "artist" => Some(HashedItemType::Artist),
            "album" => Some(HashedItemType::Album),
            "track" => Some(HashedItemType::Track),
            _ => None,
        }
    }

    /// Correct a query using the vocabulary
    fn correct_query(&self, query: &str) -> String {
        let vocabulary = self.vocabulary.read().unwrap();
        if vocabulary.is_empty() {
            // No vocabulary yet, return query as-is
            return query.to_string();
        }

        let corrected = vocabulary.correct_query(query, self.max_edit_distance);
        if corrected != query {
            debug!(
                "Query corrected: '{}' -> '{}' (vocabulary size: {})",
                query,
                corrected,
                vocabulary.len()
            );
        }
        corrected
    }

    fn expanded_query_variants(&self, query: &str) -> Vec<String> {
        let tokens: Vec<&str> = query.split_whitespace().collect();
        if tokens.is_empty() || tokens.len() > 5 {
            return Vec::new();
        }

        let vocabulary = self.vocabulary.read().unwrap();
        if vocabulary.is_empty() {
            return Vec::new();
        }

        let mut candidate_lists = Vec::with_capacity(tokens.len());
        for token in tokens {
            let mut candidates: Vec<String> = vocabulary
                .find_best_matches(token, self.max_edit_distance, EXPANDED_TOKEN_CANDIDATES)
                .into_iter()
                .map(str::to_string)
                .collect();

            if candidates.is_empty() {
                candidates.push(token.to_string());
            }

            candidate_lists.push(candidates);
        }

        let mut variants = Vec::new();
        Self::build_query_variants(&candidate_lists, 0, &mut Vec::new(), &mut variants);
        variants
    }

    fn build_query_variants(
        candidate_lists: &[Vec<String>],
        index: usize,
        current: &mut Vec<String>,
        variants: &mut Vec<String>,
    ) {
        if variants.len() >= EXPANDED_MAX_VARIANTS {
            return;
        }

        if index == candidate_lists.len() {
            variants.push(current.join(" "));
            return;
        }

        for candidate in &candidate_lists[index] {
            current.push(candidate.clone());
            Self::build_query_variants(candidate_lists, index + 1, current, variants);
            current.pop();

            if variants.len() >= EXPANDED_MAX_VARIANTS {
                break;
            }
        }
    }

    /// Record a validated impression with daily per-user/per-device budgets and
    /// one contribution per source/entity/day.
    pub fn record_impression(
        &self,
        item_id: &str,
        item_type: HashedItemType,
        source: ImpressionSource,
    ) -> bool {
        let mut conn = self.write_conn.lock().unwrap();
        let today = chrono::Utc::now()
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        let type_str = Self::item_type_to_str(&item_type);
        let device_id = source.device_id.map(|id| id as i64).unwrap_or(-1);
        let transaction = match conn.transaction() {
            Ok(transaction) => transaction,
            Err(error) => {
                warn!("Failed to start impression transaction: {error}");
                return false;
            }
        };
        let result = (|| -> rusqlite::Result<bool> {
            let user_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM item_impression_events WHERE user_id = ?1 AND date = ?2",
                rusqlite::params![source.user_id as i64, today],
                |row| row.get(0),
            )?;
            let device_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM item_impression_events
                 WHERE user_id = ?1 AND device_id = ?2 AND date = ?3",
                rusqlite::params![source.user_id as i64, device_id, today],
                |row| row.get(0),
            )?;
            if user_count >= IMPRESSION_USER_DAILY_BUDGET
                || device_count >= IMPRESSION_DEVICE_DAILY_BUDGET
            {
                return Ok(false);
            }

            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO item_impression_events
                 (user_id, device_id, item_id, item_type, date) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![source.user_id as i64, device_id, item_id, type_str, today],
            )?;
            if inserted == 0 {
                return Ok(false);
            }
            transaction.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(item_id, item_type, date)
                 DO UPDATE SET impression_count = impression_count + 1",
                rusqlite::params![item_id, type_str, today],
            )?;
            Ok(true)
        })();

        match result {
            Ok(recorded) => {
                if let Err(error) = transaction.commit() {
                    warn!("Failed to commit impression transaction: {error}");
                    false
                } else {
                    recorded
                }
            }
            Err(error) => {
                warn!("Failed to record impression: {error}");
                false
            }
        }
    }

    /// Get total impressions for all items within a date range.
    /// Returns a map of (item_id, item_type) -> total impression count.
    pub fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        let conn = self.read_conn.lock().unwrap();
        let mut totals = std::collections::HashMap::new();

        let mut stmt = match conn.prepare(
            "SELECT item_id, item_type, SUM(impression_count) as total
             FROM item_impressions
             WHERE date >= ?
             GROUP BY item_id, item_type",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to prepare impression totals query: {}", e);
                return totals;
            }
        };

        let rows = match stmt.query_map([min_date], |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let total: i64 = row.get(2)?;
            Ok((item_id, item_type_str, total))
        }) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to query impression totals: {}", e);
                return totals;
            }
        };

        for row in rows.flatten() {
            let (item_id, item_type_str, total) = row;
            if let Some(item_type) = Self::str_to_item_type(&item_type_str) {
                totals.insert((item_id, item_type), total as u64);
            }
        }

        totals
    }

    /// Prune old impression records.
    /// Deletes records older than the specified date (in YYYYMMDD format).
    pub fn prune_impressions(&self, before_date: i64) -> usize {
        let conn = self.write_conn.lock().unwrap();
        let event_result = conn.execute(
            "DELETE FROM item_impression_events WHERE date < ?",
            [before_date],
        );
        match conn.execute("DELETE FROM item_impressions WHERE date < ?", [before_date]) {
            Ok(count) => {
                if let Err(error) = event_result {
                    warn!("Failed to prune impression source records: {error}");
                }
                if count > 0 {
                    info!("Pruned {} old impression records", count);
                }
                count
            }
            Err(e) => {
                warn!("Failed to prune impressions: {}", e);
                0
            }
        }
    }

    /// Update availability status for items.
    pub fn update_availability(&self, items: &[(String, HashedItemType, bool)]) {
        if items.is_empty() {
            return;
        }

        let conn = self.write_conn.lock().unwrap();

        for (id, item_type, is_available) in items {
            let type_str = Self::item_type_to_str(item_type);
            let result = (|| -> Result<()> {
                conn.execute(
                    "INSERT OR REPLACE INTO item_availability
                     (item_id, item_type, is_available) VALUES (?, ?, ?)",
                    rusqlite::params![id, type_str, if *is_available { 1 } else { 0 }],
                )?;
                conn.execute(
                    &format!("DELETE FROM {AVAILABLE_INDEX} WHERE item_id = ?1 AND item_type = ?2"),
                    rusqlite::params![id, type_str],
                )?;
                if *is_available && Self::is_enriched_index(&conn, ACTIVE_INDEX)? {
                    conn.execute(
                        &format!(
                            "INSERT INTO {AVAILABLE_INDEX}
                             (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                             SELECT item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text
                             FROM {ACTIVE_INDEX} WHERE item_id = ?1 AND item_type = ?2"
                        ),
                        rusqlite::params![id, type_str],
                    )?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                warn!("Failed to update availability for {}: {}", id, e);
            }
        }

        debug!("Updated availability for {} items", items.len());
    }

    /// Insert documents that are known to have just become available.
    ///
    /// `item_id` and `item_type` are UNINDEXED FTS5 columns, so deleting or
    /// selecting an FTS row by those columns scans the entire virtual table.
    /// Proxy materialization has the document already and guarantees a
    /// missing -> available transition, allowing a direct O(1) insertion.
    pub fn publish_newly_available(&self, items: &[SearchIndexItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            let enriched = Self::is_enriched_index(&conn, ACTIVE_INDEX)?;
            let available_ready = Self::available_index_is_ready(&conn);
            for item in items {
                let type_str = Self::item_type_to_str(&item.item_type);
                conn.execute(
                    "INSERT OR REPLACE INTO item_availability
                     (item_id, item_type, is_available) VALUES (?1, ?2, 1)",
                    rusqlite::params![item.id, type_str],
                )?;

                if enriched && available_ready {
                    let (primary, artist, album, extra) = Self::index_item_document_parts(item);
                    conn.execute(
                        &format!(
                            "INSERT INTO {AVAILABLE_INDEX}
                             (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                             VALUES(?1,?2,?3,?4,?5,?6,?7)"
                        ),
                        rusqlite::params![
                            item.id,
                            type_str,
                            item.name,
                            primary,
                            artist,
                            album,
                            extra
                        ],
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                debug!("Published {} newly available search documents", items.len());
                Ok(())
            }
            Err(error) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }

    /// Search with availability filter in the query itself.
    pub fn search_with_availability(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        let enriched = self
            .read_conn
            .lock()
            .ok()
            .and_then(|conn| Self::is_enriched_index(&conn, ACTIVE_INDEX).ok())
            .unwrap_or(false);
        if enriched {
            return self.search_enriched(query, max_results, filter, available_only);
        }
        if !available_only {
            // Fall back to regular search
            return SearchVault::search(self, query, max_results, filter);
        }

        let corrected_query = self.correct_query(query);
        let conn = self.read_conn.lock().unwrap();
        let escaped_query = corrected_query.replace('"', "\"\"");

        // Build query with availability JOIN
        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(types) = &filter {
            let type_placeholders: Vec<&str> = types.iter().map(Self::item_type_to_str).collect();
            let placeholders = type_placeholders
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                r#"SELECT
                       s.item_id,
                       s.item_type,
                       s.name,
                       bm25(search_index) as text_score,
                       COALESCE(p.score, 0.0) as popularity_score
                   FROM search_index s
                   INNER JOIN item_availability a
                       ON s.item_id = a.item_id AND s.item_type = a.item_type AND a.is_available = 1
                   LEFT JOIN item_popularity p
                       ON s.item_id = p.item_id AND s.item_type = p.item_type
                   WHERE search_index MATCH ?
                   AND s.item_type IN ({})
                   ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                   LIMIT ?"#,
                placeholders
            );

            let mut params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(format!("\"{}\"", escaped_query))];
            for t in type_placeholders {
                params.push(Box::new(t.to_string()));
            }
            params.push(Box::new(POPULARITY_WEIGHT));
            params.push(Box::new(max_results as i64));

            (sql, params)
        } else {
            let sql = r#"SELECT
                             s.item_id,
                             s.item_type,
                             s.name,
                             bm25(search_index) as text_score,
                             COALESCE(p.score, 0.0) as popularity_score
                         FROM search_index s
                         INNER JOIN item_availability a
                             ON s.item_id = a.item_id AND s.item_type = a.item_type AND a.is_available = 1
                         LEFT JOIN item_popularity p
                             ON s.item_id = p.item_id AND s.item_type = p.item_type
                         WHERE search_index MATCH ?
                         ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                         LIMIT ?"#
                .to_string();

            let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(format!("\"{}\"", escaped_query)),
                Box::new(POPULARITY_WEIGHT),
                Box::new(max_results as i64),
            ];

            (sql, params)
        };

        // Execute query (same pattern as existing search())
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("Availability search query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let results = stmt.query_map(param_refs.as_slice(), |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let text_score: f64 = row.get(3)?;
            let popularity_score: f64 = row.get(4)?;

            Ok((item_id, item_type_str, name, text_score, popularity_score))
        });

        match results {
            Ok(rows) => rows
                .filter_map(|r| r.ok())
                .filter_map(
                    |(item_id, item_type_str, name, text_score, popularity_score)| {
                        Self::str_to_item_type(&item_type_str).map(|item_type| {
                            let combined_score =
                                text_score * (1.0 + popularity_score * POPULARITY_WEIGHT);
                            SearchResult {
                                item_id,
                                item_type,
                                score: (-text_score * 1000.0) as u32,
                                adjusted_score: (-combined_score * 1000.0) as i64,
                                matchable_text: name,
                            }
                        })
                    },
                )
                .collect(),
            Err(e) => {
                warn!("Availability search query failed: {}", e);
                Vec::new()
            }
        }
    }

    fn query_enriched_channel(
        &self,
        conn: &Connection,
        index_table: &str,
        match_expression: &str,
        max_results: usize,
        filter: &Option<Vec<HashedItemType>>,
    ) -> Vec<RankedCandidate> {
        let (type_clause, type_values) = if let Some(types) = filter {
            if types.is_empty() {
                return Vec::new();
            }
            (
                format!(
                    " AND item_type IN ({})",
                    std::iter::repeat_n("?", types.len())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                types.iter().map(Self::item_type_to_str).collect::<Vec<_>>(),
            )
        } else {
            (String::new(), Vec::new())
        };
        // `ORDER BY rank` lets FTS5 stop after the bounded candidate window.
        // Popularity is joined only after that window has materialized, so a
        // broad token never produces a 330M-row join/sort before LIMIT.
        let sql = format!(
            "WITH ranked AS MATERIALIZED (
                 SELECT item_id, item_type, display_name, rank AS text_score
                 FROM {index_table}
                 WHERE {index_table} MATCH ?
                   AND rank MATCH 'bm25(0.0, 0.0, 0.0, 10.0, 6.0, 3.0, 1.0)'
                   {type_clause}
                 ORDER BY rank
                 LIMIT ?
             )
             SELECT s.item_id, s.item_type, s.display_name,
                    COALESCE(p.score, 0.0), s.text_score
             FROM ranked s
             LEFT JOIN item_popularity p
               ON s.item_id = p.item_id AND s.item_type = p.item_type
             ORDER BY text_score, s.item_type, s.item_id
             "
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(match_expression.to_string())];
        for item_type in type_values {
            params.push(Box::new(item_type.to_string()));
        }
        params.push(Box::new(max_results as i64));
        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|value| value.as_ref()).collect();
        let mut stmt = match conn.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(error) => {
                warn!(%error, "Enriched search channel preparation failed");
                return Vec::new();
            }
        };
        let rows = match stmt.query_map(refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(error) => {
                warn!(%error, expression = match_expression, "Enriched search channel failed");
                return Vec::new();
            }
        };
        rows.filter_map(Result::ok)
            .filter_map(|(item_id, item_type, display_name, popularity)| {
                Some(RankedCandidate {
                    item_id,
                    item_type: Self::str_to_item_type(&item_type)?,
                    evidence: 1.0,
                    matchable_text: display_name,
                    popularity,
                })
            })
            .collect()
    }

    fn quoted_phrase(query: &str) -> Option<&str> {
        let trimmed = query.trim();
        (trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"'))
            .then(|| &trimmed[1..trimmed.len() - 1])
    }

    fn fts_phrase(text: &str) -> String {
        format!("\"{}\"", text.replace('"', "\"\""))
    }

    fn search_enriched(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        if max_results == 0 {
            return Vec::new();
        }
        let explicit_phrase = Self::quoted_phrase(query);
        let normalized = Self::normalize_text(explicit_phrase.unwrap_or(query));
        if normalized.is_empty() {
            return Vec::new();
        }
        let tokens: Vec<_> = normalized.split_whitespace().collect();
        let candidate_limit = max_results.saturating_mul(4).clamp(max_results, 400);
        let conn = self.read_conn.lock().unwrap();
        let index_table = if available_only {
            AVAILABLE_INDEX
        } else {
            ACTIVE_INDEX
        };
        let mut channels = Vec::new();

        if explicit_phrase.is_some() {
            channels.push(ProviderEvidence {
                provider_id: "lexical.phrase".into(),
                delivery: CandidateDelivery::Immediate,
                weight: 4.0,
                candidates: self.query_enriched_channel(
                    &conn,
                    index_table,
                    &Self::fts_phrase(&normalized),
                    candidate_limit,
                    &filter,
                ),
            });
        } else {
            channels.push(ProviderEvidence {
                provider_id: "lexical.primary_phrase".into(),
                delivery: CandidateDelivery::Immediate,
                weight: 4.0,
                candidates: self.query_enriched_channel(
                    &conn,
                    index_table,
                    &format!("primary_name : {}", Self::fts_phrase(&normalized)),
                    candidate_limit,
                    &filter,
                ),
            });
            channels.push(ProviderEvidence {
                provider_id: "lexical.all_tokens".into(),
                delivery: CandidateDelivery::Immediate,
                weight: 2.0,
                candidates: self.query_enriched_channel(
                    &conn,
                    index_table,
                    &tokens
                        .iter()
                        .map(|token| Self::fts_phrase(token))
                        .collect::<Vec<_>>()
                        .join(" AND "),
                    candidate_limit,
                    &filter,
                ),
            });
            channels.push(ProviderEvidence {
                provider_id: "lexical.any_token".into(),
                delivery: CandidateDelivery::Immediate,
                weight: 1.0,
                candidates: self.query_enriched_channel(
                    &conn,
                    index_table,
                    &tokens
                        .iter()
                        .map(|token| Self::fts_phrase(token))
                        .collect::<Vec<_>>()
                        .join(" OR "),
                    candidate_limit,
                    &filter,
                ),
            });

            let corrected = Self::normalize_text(&self.correct_query(&normalized));
            let typo_expression = self
                .expanded_query_variants(&normalized)
                .into_iter()
                .map(|variant| Self::normalize_text(&variant))
                .chain(std::iter::once(corrected))
                .filter(|variant| !variant.is_empty() && variant != &normalized)
                .take(EXPANDED_MAX_VARIANTS)
                .map(|variant| Self::fts_phrase(&variant))
                .collect::<Vec<_>>()
                .join(" OR ");
            if !typo_expression.is_empty() {
                channels.push(ProviderEvidence {
                    provider_id: "lexical.typo".into(),
                    delivery: CandidateDelivery::Immediate,
                    weight: 0.5,
                    candidates: self.query_enriched_channel(
                        &conn,
                        index_table,
                        &typo_expression,
                        candidate_limit,
                        &filter,
                    ),
                });
            }
        }
        drop(conn);

        CandidateCoordinator
            .fuse(channels, max_results)
            .into_iter()
            .map(|candidate| {
                let scaled = (candidate.score * 1_000_000.0).round().max(1.0) as i64;
                SearchResult {
                    item_id: candidate.item_id,
                    item_type: candidate.item_type,
                    score: scaled.min(u32::MAX as i64) as u32,
                    adjusted_score: scaled,
                    matchable_text: candidate.matchable_text,
                }
            })
            .collect()
    }
}

/// Weight used only by the legacy index while an enriched side-build is active.
const POPULARITY_WEIGHT: f64 = 0.5;
const EXPANDED_TOKEN_CANDIDATES: usize = 3;
const EXPANDED_MAX_VARIANTS: usize = 16;

impl SearchVault for Fts5LevenshteinSearchVault {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        let enriched = self
            .read_conn
            .lock()
            .ok()
            .and_then(|conn| Self::is_enriched_index(&conn, ACTIVE_INDEX).ok())
            .unwrap_or(false);
        if enriched {
            return self.search_enriched(query, max_results, filter, false);
        }
        // Search works even during building (with partial results)
        let corrected_query = self.correct_query(query);

        if corrected_query != query {
            debug!("Query corrected: '{}' -> '{}'", query, corrected_query);
        }

        // Use read connection - this won't block on writes due to WAL mode
        let conn = self.read_conn.lock().unwrap();

        let escaped_query = corrected_query.replace('"', "\"\"");

        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(types) = &filter {
            let type_placeholders: Vec<&str> = types.iter().map(Self::item_type_to_str).collect();
            let placeholders = type_placeholders
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                r#"SELECT
                       s.item_id,
                       s.item_type,
                       s.name,
                       bm25(search_index) as text_score,
                       COALESCE(p.score, 0.0) as popularity_score
                   FROM search_index s
                   LEFT JOIN item_popularity p
                       ON s.item_id = p.item_id AND s.item_type = p.item_type
                   WHERE search_index MATCH ?
                   AND s.item_type IN ({})
                   ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                   LIMIT ?"#,
                placeholders
            );

            let mut params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(format!("\"{}\"", escaped_query))];
            for t in type_placeholders {
                params.push(Box::new(t.to_string()));
            }
            params.push(Box::new(POPULARITY_WEIGHT));
            params.push(Box::new(max_results as i64));

            (sql, params)
        } else {
            let sql = r#"SELECT
                             s.item_id,
                             s.item_type,
                             s.name,
                             bm25(search_index) as text_score,
                             COALESCE(p.score, 0.0) as popularity_score
                         FROM search_index s
                         LEFT JOIN item_popularity p
                             ON s.item_id = p.item_id AND s.item_type = p.item_type
                         WHERE search_index MATCH ?
                         ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                         LIMIT ?"#
                .to_string();

            let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(format!("\"{}\"", escaped_query)),
                Box::new(POPULARITY_WEIGHT),
                Box::new(max_results as i64),
            ];

            (sql, params)
        };

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("FTS5+Levenshtein search query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let results = stmt.query_map(param_refs.as_slice(), |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let text_score: f64 = row.get(3)?;
            let popularity_score: f64 = row.get(4)?;

            Ok((item_id, item_type_str, name, text_score, popularity_score))
        });

        match results {
            Ok(rows) => rows
                .filter_map(|r| r.ok())
                .filter_map(
                    |(item_id, item_type_str, name, text_score, popularity_score)| {
                        Self::str_to_item_type(&item_type_str).map(|item_type| {
                            let combined_score =
                                text_score * (1.0 + popularity_score * POPULARITY_WEIGHT);
                            SearchResult {
                                item_id,
                                item_type,
                                score: (-text_score * 1000.0) as u32,
                                adjusted_score: (-combined_score * 1000.0) as i64,
                                matchable_text: name,
                            }
                        })
                    },
                )
                .collect(),
            Err(e) => {
                warn!("FTS5+Levenshtein search query failed: {}", e);
                Vec::new()
            }
        }
    }

    fn search_expanded(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        if self
            .read_conn
            .lock()
            .ok()
            .and_then(|conn| Self::is_enriched_index(&conn, ACTIVE_INDEX).ok())
            .unwrap_or(false)
        {
            return SearchVault::search(self, query, max_results, filter);
        }
        let mut results = SearchVault::search(self, query, max_results, filter.clone());
        if results.len() >= max_results {
            return results;
        }

        let mut seen_items: HashSet<(String, HashedItemType)> = results
            .iter()
            .map(|result| (result.item_id.clone(), result.item_type))
            .collect();
        let mut tried_queries = HashSet::new();
        tried_queries.insert(self.correct_query(query).to_lowercase());

        for variant in self.expanded_query_variants(query) {
            if results.len() >= max_results {
                break;
            }

            if !tried_queries.insert(variant.to_lowercase()) {
                continue;
            }

            for result in SearchVault::search(self, &variant, max_results, filter.clone()) {
                if seen_items.insert((result.item_id.clone(), result.item_type)) {
                    results.push(result);
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        results
    }

    fn search_expanded_with_availability(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        if self
            .read_conn
            .lock()
            .ok()
            .and_then(|conn| Self::is_enriched_index(&conn, ACTIVE_INDEX).ok())
            .unwrap_or(false)
        {
            return self.search_with_availability(query, max_results, filter, available_only);
        }
        let mut results =
            self.search_with_availability(query, max_results, filter.clone(), available_only);
        if results.len() >= max_results {
            return results;
        }

        let mut seen_items: HashSet<(String, HashedItemType)> = results
            .iter()
            .map(|result| (result.item_id.clone(), result.item_type))
            .collect();
        let mut tried_queries = HashSet::new();
        tried_queries.insert(self.correct_query(query).to_lowercase());

        for variant in self.expanded_query_variants(query) {
            if results.len() >= max_results {
                break;
            }

            if !tried_queries.insert(variant.to_lowercase()) {
                continue;
            }

            for result in
                self.search_with_availability(&variant, max_results, filter.clone(), available_only)
            {
                if seen_items.insert((result.item_id.clone(), result.item_type)) {
                    results.push(result);
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        results
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::rebuild_index(self)
    }

    fn upsert_items(&self, items: &[SearchIndexItem]) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::upsert_items(self, items)
    }

    fn remove_items(&self, items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::remove_items(self, items)
    }

    fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        Fts5LevenshteinSearchVault::update_popularity(self, items)
    }

    fn get_stats(&self) -> SearchVaultStats {
        let conn = self.read_conn.lock().unwrap();
        let count = Self::get_index_item_count(&conn).unwrap_or(0);

        let state = self.state.read().unwrap().clone();

        SearchVaultStats {
            indexed_items: count,
            index_type: "FTS5+Levenshtein".to_string(),
            state,
        }
    }

    fn record_impression(
        &self,
        item_id: &str,
        item_type: HashedItemType,
        source: ImpressionSource,
    ) -> bool {
        Fts5LevenshteinSearchVault::record_impression(self, item_id, item_type, source)
    }

    fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        Fts5LevenshteinSearchVault::get_impression_totals(self, min_date)
    }

    fn prune_impressions(&self, before_date: i64) -> usize {
        Fts5LevenshteinSearchVault::prune_impressions(self, before_date)
    }

    fn update_availability(&self, items: &[(String, HashedItemType, bool)]) {
        Fts5LevenshteinSearchVault::update_availability(self, items)
    }

    fn publish_newly_available(&self, items: &[SearchIndexItem]) -> Result<()> {
        Fts5LevenshteinSearchVault::publish_newly_available(self, items)
    }

    fn search_with_availability(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        Fts5LevenshteinSearchVault::search_with_availability(
            self,
            query,
            max_results,
            filter,
            available_only,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SearchableItem;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn build_checkpoint_truncates_wal() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let conn = Connection::open(&db_path).unwrap();
        crate::backup::DbRegistry::new()
            .register(db_path.clone(), &conn)
            .unwrap();
        conn.execute("CREATE TABLE checkpoint_test(value TEXT)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO checkpoint_test(value) VALUES(zeroblob(1048576))",
            [],
        )
        .unwrap();

        Fts5LevenshteinSearchVault::checkpoint_build_wal(&conn).unwrap();

        let mut wal_path = db_path.into_os_string();
        wal_path.push("-wal");
        let wal_size = std::fs::metadata(std::path::PathBuf::from(wal_path))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        assert_eq!(wal_size, 0);
    }

    mod mock {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::AtomicI64;

        pub struct MockCatalogStore {
            pub items: Vec<SearchableItem>,
            #[allow(dead_code)]
            pub skeleton_version: AtomicI64,
        }

        impl MockCatalogStore {
            pub fn new(items: Vec<SearchableItem>) -> Self {
                Self {
                    items,
                    skeleton_version: AtomicI64::new(0),
                }
            }

            pub fn with_version(items: Vec<SearchableItem>, version: i64) -> Self {
                Self {
                    items,
                    skeleton_version: AtomicI64::new(version),
                }
            }
        }

        impl CatalogStore for MockCatalogStore {
            fn get_artist_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_album_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_track_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_track(&self, _id: &str) -> anyhow::Result<Option<crate::catalog_store::Track>> {
                Ok(None)
            }
            fn get_resolved_artist_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_album_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_track_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_artist(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedArtist>> {
                Ok(None)
            }
            fn get_resolved_album(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedAlbum>> {
                Ok(None)
            }
            fn get_resolved_track(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedTrack>> {
                Ok(None)
            }
            fn get_discography(
                &self,
                _id: &str,
                _limit: usize,
                _offset: usize,
                _sort: crate::catalog_store::DiscographySort,
                _appears_on: bool,
            ) -> anyhow::Result<Option<crate::catalog_store::ArtistDiscography>> {
                Ok(None)
            }
            fn get_album_image_url(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ImageUrl>> {
                Ok(None)
            }
            fn get_artist_image_url(
                &self,
                _artist_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ImageUrl>> {
                Ok(None)
            }
            fn get_image_path(&self, _id: &str) -> PathBuf {
                PathBuf::new()
            }
            fn get_track_audio_path(&self, _track_id: &str) -> Option<PathBuf> {
                None
            }

            fn open_track_audio_file(
                &self,
                _track_id: &str,
            ) -> anyhow::Result<Option<(std::fs::File, PathBuf)>> {
                Ok(None)
            }
            fn get_track_album_id(&self, _track_id: &str) -> Option<String> {
                None
            }
            fn get_artists_count(&self) -> usize {
                self.items
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Artist)
                    .count()
            }
            fn get_albums_count(&self) -> usize {
                self.items
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Album)
                    .count()
            }
            fn get_tracks_count(&self) -> usize {
                self.items
                    .iter()
                    .filter(|item| item.content_type == SearchableContentType::Track)
                    .count()
            }
            fn get_searchable_content(&self) -> anyhow::Result<Vec<SearchableItem>> {
                Ok(self.items.clone())
            }
            fn list_all_track_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn list_available_track_ids_with_audio_uri(
                &self,
                _limit: usize,
                _offset: usize,
            ) -> anyhow::Result<Vec<(String, String)>> {
                Ok(vec![])
            }
            fn create_artist(&self, _artist: &crate::catalog_store::Artist) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_artist(&self, _artist: &crate::catalog_store::Artist) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_artist(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn create_album(
                &self,
                _album: &crate::catalog_store::Album,
                _artist_ids: &[String],
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_album_metadata(
                &self,
                _album_id: &str,
                _metadata: &crate::catalog_store::AlbumMetadataUpdate,
                _artist_ids: Option<&[String]>,
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_album(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn create_track(
                &self,
                _track: &crate::catalog_store::Track,
                _artist_ids: &[String],
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_track_metadata(
                &self,
                _track_id: &str,
                _metadata: &crate::catalog_store::TrackMetadataUpdate,
                _artist_ids: Option<&[String]>,
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_track(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn set_track_audio_uri(&self, _track_id: &str, _audio_uri: &str) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn recompute_album_availability(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<crate::catalog_store::AlbumAvailability> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn recompute_artist_availability(&self, _artist_id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn get_album_artist_ids(&self, _album_id: &str) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn get_items_popularity(
                &self,
                _items: &[(String, SearchableContentType)],
            ) -> anyhow::Result<std::collections::HashMap<(String, SearchableContentType), i32>>
            {
                Ok(std::collections::HashMap::new())
            }
            fn get_genres_with_counts(
                &self,
            ) -> anyhow::Result<Vec<crate::catalog_store::GenreInfo>> {
                Ok(Vec::new())
            }
            fn get_tracks_by_genre(
                &self,
                _genre: &str,
                _limit: usize,
                _offset: usize,
            ) -> anyhow::Result<crate::catalog_store::GenreTracksResult> {
                Ok(crate::catalog_store::GenreTracksResult {
                    track_ids: Vec::new(),
                    total: 0,
                    has_more: false,
                })
            }
            fn get_random_tracks_by_genre(
                &self,
                _genre: &str,
                _limit: usize,
            ) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn get_album_track_durations(&self, _album_id: &str) -> anyhow::Result<Vec<i64>> {
                Ok(vec![])
            }
            fn find_albums_by_fingerprint(
                &self,
                _track_count: i32,
                _total_duration_ms: i64,
            ) -> anyhow::Result<Vec<crate::catalog_store::AlbumFingerprintCandidate>> {
                Ok(Vec::new())
            }
            fn update_album_fingerprint(&self, _album_id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_artists_needing_mbid(
                &self,
                _limit: usize,
            ) -> anyhow::Result<Vec<(String, i64)>> {
                Ok(Vec::new())
            }
            fn get_artists_needing_related(
                &self,
                _limit: usize,
            ) -> anyhow::Result<Vec<(String, String, i64)>> {
                Ok(Vec::new())
            }
            fn get_artist_mbid(&self, _artist_id: &str) -> anyhow::Result<Option<String>> {
                Ok(None)
            }

            fn set_artist_mbid(&self, _artist_id: &str, _mbid: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn mark_artist_mbid_not_found(&self, _artist_id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn record_artist_mbid_failure(
                &self,
                _artist_rowid: i64,
                _error: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn record_artist_related_failure(
                &self,
                _artist_rowid: i64,
                _error: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn release_artist_enrichment_claims(&self) -> anyhow::Result<()> {
                Ok(())
            }
            fn set_related_artists(
                &self,
                _artist_rowid: i64,
                _related: &[(i64, f64)],
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_related_artists(
                &self,
                _artist_id: &str,
            ) -> anyhow::Result<Vec<crate::catalog_store::Artist>> {
                Ok(Vec::new())
            }
            fn get_artist_rowid_by_mbid(&self, _mbid: &str) -> anyhow::Result<Option<i64>> {
                Ok(None)
            }
        }
    }

    use mock::MockCatalogStore;

    #[test]
    fn test_lazy_init_and_background_build() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create lazy vault
        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap(),
        );

        // Initially empty
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 0);
        assert_eq!(stats.state, IndexState::Empty);

        // Search returns empty results
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 0);

        // Start background build
        let catalog = Arc::new(MockCatalogStore::new(vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
        ]));

        vault.start_background_build(catalog);

        // Wait for build to complete without relying on storage timing.
        for _ in 0..500 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Now should have results
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 2);
        assert_eq!(stats.state, IndexState::Ready);

        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn existing_full_index_upgrades_by_building_only_available_subset() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let items = vec![
            SearchableItem {
                id: "playable".into(),
                name: "Chopin Etude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Chopin".into()],
                is_available: true,
            },
            SearchableItem {
                id: "missing".into(),
                name: "Chopin Etude Missing".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Chopin".into()],
                is_available: false,
            },
        ];
        {
            let _vault = Fts5LevenshteinSearchVault::new(
                Arc::new(MockCatalogStore::new(items.clone())),
                &db_path,
                &crate::backup::DbRegistry::new(),
            )
            .unwrap();
        }
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "DROP TABLE search_index_available;
             DELETE FROM search_metadata WHERE key IN
               ('available_index_schema_version', 'available_index_item_count');",
        )
        .unwrap();
        drop(conn);

        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap(),
        );
        vault.start_background_build(Arc::new(MockCatalogStore::new(items)));
        for _ in 0..100 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(vault.get_stats().indexed_items, 2);
        let available = vault.search_with_availability("chopin etude", 10, None, true);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].item_id, "playable");
        assert_eq!(vault.search("chopin etude", 10, None).len(), 2);
    }

    #[test]
    fn test_typo_correction_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::new(vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
        ]));

        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Exact search
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo
        let results = vault.search("Beatels", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo
        let results = vault.search("Metalica", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a2");
    }

    #[test]
    fn test_typo_correction_search_after_large_same_length_bucket() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let mut items = Vec::new();
        for i in 0..6_000 {
            items.push(SearchableItem {
                id: format!("filler_{i}"),
                name: format!("x{i:04}"),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            });
        }
        items.push(SearchableItem {
            id: "lucio_dalla".to_string(),
            name: "Lucio Dalla".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
            is_available: true,
        });

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        let results = vault.search("fucio dalla", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "lucio_dalla");
    }

    #[test]
    fn test_expanded_search_uses_token_alternatives_for_exact_words() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "lucio_dalla".to_string(),
                name: "Lucio Dalla".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "palla".to_string(),
                name: "Palla".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "alla".to_string(),
                name: "Alla".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Schema v2 includes the typo channel in the normal fused search.
        assert!(vault
            .search("fucio palla", 10, None)
            .iter()
            .any(|item| item.item_id == "lucio_dalla"));
        assert!(vault
            .search("fucio alla", 10, None)
            .iter()
            .any(|item| item.item_id == "lucio_dalla"));

        let palla_results = vault.search_expanded("fucio palla", 10, None);
        assert!(palla_results
            .iter()
            .any(|item| item.item_id == "lucio_dalla"));

        let alla_results = vault.search_expanded("fucio alla", 10, None);
        assert!(alla_results
            .iter()
            .any(|item| item.item_id == "lucio_dalla"));
    }

    #[test]
    fn test_search_with_type_filter() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "artist1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "album1".to_string(),
                name: "Beatles For Sale".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "track1".to_string(),
                name: "Beatles Medley".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Search without filter - should find all 3
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 3);

        // Search with Artist filter only
        let results = vault.search("Beatles", 10, Some(vec![HashedItemType::Artist]));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "artist1");
    }

    #[test]
    fn test_resumable_build_detects_partial_state() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // First, create a vault and manually simulate a partial build
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            Fts5LevenshteinSearchVault::create_tables(&conn).unwrap();

            // Insert some items (simulating partial progress)
            conn.execute(
                "INSERT INTO search_index (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                 VALUES ('a1','artist','The Beatles','the beatles','','','')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO search_index (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                 VALUES ('a2','artist','Pink Floyd','pink floyd','','','')",
                [],
            )
            .unwrap();

            // Set partial build metadata
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_in_progress", "true").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_offset", "2").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_total", "5").unwrap();
        }

        // Now create a new lazy vault - it should detect the partial build
        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 2);
        assert!(
            matches!(
                stats.state,
                IndexState::Building {
                    processed: 2,
                    total: Some(5)
                }
            ),
            "Expected Building state with processed=2, total=Some(5), got {:?}",
            stats.state
        );
    }

    #[test]
    fn test_resumable_build_resumes_from_offset() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create 5 items
        let items = vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Pink Floyd".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a3".to_string(),
                name: "Led Zeppelin".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a4".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a5".to_string(),
                name: "Iron Maiden".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
        ];

        // Simulate partial build: insert first 2 items manually
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            Fts5LevenshteinSearchVault::create_tables(&conn).unwrap();

            // Insert the first two items into the side-build table and persist
            // the stable artist cursor, simulating an interrupted process.
            Fts5LevenshteinSearchVault::create_enriched_index(&conn, BUILD_INDEX).unwrap();
            conn.execute(
                "INSERT INTO search_index_building (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                 VALUES ('a1','artist','The Beatles','the beatles','','','')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO search_index_building (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                 VALUES ('a2','artist','Pink Floyd','pink floyd','','','')",
                [],
            )
            .unwrap();

            // Set partial build metadata
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_in_progress", "true").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_offset", "2").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_total", "5").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_entity_type", "artist").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_after_rowid", "2").unwrap();
        }

        // Create vault and resume build
        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap(),
        );
        let catalog = Arc::new(MockCatalogStore::new(items));

        vault.start_background_build(catalog);

        // Wait for build to complete without relying on storage timing.
        for _ in 0..500 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Should now have all 5 items
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 5, "Expected 5 items after resume");
        assert_eq!(stats.state, IndexState::Ready);

        // Verify all items are searchable
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        let results = vault.search("Zeppelin", 10, None);
        assert_eq!(results.len(), 1);
        let results = vault.search("Maiden", 10, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_completed_build_clears_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "a1".to_string(),
            name: "The Beatles".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
            is_available: true,
        }];

        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap(),
        );
        let catalog = Arc::new(MockCatalogStore::new(items));

        vault.start_background_build(catalog);

        // Wait for build to complete without relying on storage timing.
        for _ in 0..500 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Check that metadata was cleared
        let conn = Connection::open(&db_path).unwrap();
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_in_progress").is_none(),
            "build_in_progress should be cleared after completion"
        );
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_offset").is_none(),
            "build_offset should be cleared after completion"
        );
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_total").is_none(),
            "build_total should be cleared after completion"
        );

        // Creating a new vault should show Ready state
        let vault2 =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();
        assert_eq!(vault2.get_stats().state, IndexState::Ready);
    }

    #[test]
    fn sparse_availability_still_indexes_unavailable_content() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let registry = crate::backup::DbRegistry::new();
        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy_with_build_options(
                &db_path,
                &registry,
                SearchBuildOptions {
                    batch_size: 1,
                    preparation_threads: 2,
                    sparse_availability: true,
                    replay_mutations: true,
                    verify_fts_integrity: true,
                },
            )
            .unwrap(),
        );
        let catalog = Arc::new(MockCatalogStore::new(vec![
            SearchableItem {
                id: "available".into(),
                name: "Available Etude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Chopin".into()],
                is_available: true,
            },
            SearchableItem {
                id: "unavailable".into(),
                name: "Unavailable Etude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Chopin".into()],
                is_available: false,
            },
        ]));

        vault.start_background_build(catalog);
        for _ in 0..100 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(vault.get_stats().state, IndexState::Ready);

        let conn = Connection::open(&db_path).unwrap();
        let indexed: i64 = conn
            .query_row("SELECT COUNT(*) FROM search_index", [], |row| row.get(0))
            .unwrap();
        let availability_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_availability", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            indexed, 2,
            "both available and unavailable content is indexed"
        );
        assert_eq!(availability_rows, 1, "only the available ID is stored");
    }

    #[test]
    fn test_search_with_availability_filters_unavailable() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create items with mixed availability
        let items = vec![
            SearchableItem {
                id: "available_artist".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "unavailable_artist".to_string(),
                name: "Beatles Tribute Band".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: false,
            },
            SearchableItem {
                id: "available_album".to_string(),
                name: "Beatles Greatest Hits".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "unavailable_album".to_string(),
                name: "Beatles Live".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: false,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Regular search should return all 4 items
        let all_results = vault.search("Beatles", 10, None);
        assert_eq!(
            all_results.len(),
            4,
            "Regular search should return all items"
        );

        // search_with_availability(available_only=true) should only return available items
        let available_results = vault.search_with_availability("Beatles", 10, None, true);
        assert_eq!(
            available_results.len(),
            2,
            "Availability search should only return available items"
        );

        // Verify we got the right items
        let available_ids: Vec<_> = available_results
            .iter()
            .map(|r| r.item_id.as_str())
            .collect();
        assert!(available_ids.contains(&"available_artist"));
        assert!(available_ids.contains(&"available_album"));
        assert!(!available_ids.contains(&"unavailable_artist"));
        assert!(!available_ids.contains(&"unavailable_album"));

        // search_with_availability(available_only=false) should return all items
        let all_via_availability = vault.search_with_availability("Beatles", 10, None, false);
        assert_eq!(
            all_via_availability.len(),
            4,
            "Availability search with available_only=false should return all items"
        );
    }

    #[test]
    fn test_update_availability_changes_search_results() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Start with one available and one unavailable - both share "day" in the name for searching
        let items = vec![
            SearchableItem {
                id: "track1".to_string(),
                name: "Yesterday Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "track2".to_string(),
                name: "Today Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: false,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Initially, only track1 should be available when searching for "day"
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            1,
            "Initially only 1 track should be available"
        );
        assert_eq!(results[0].item_id, "track1");

        // Update track2 to be available
        vault.update_availability(&[("track2".to_string(), HashedItemType::Track, true)]);

        // Now both should be available
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            2,
            "After update, both tracks should be available"
        );

        // Update track1 to be unavailable
        vault.update_availability(&[("track1".to_string(), HashedItemType::Track, false)]);

        // Now only track2 should be available
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            1,
            "After second update, only 1 track should be available"
        );
        assert_eq!(results[0].item_id, "track2");
    }

    #[test]
    fn test_publish_newly_available_inserts_prepared_document() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let catalog = Arc::new(MockCatalogStore::new(vec![SearchableItem {
            id: "track-new".to_string(),
            name: "Immediate Playback".to_string(),
            content_type: SearchableContentType::Track,
            additional_text: vec!["artist:Fast Artist".to_string()],
            is_available: false,
        }]));
        let vault =
            Fts5LevenshteinSearchVault::new(catalog, &db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        assert!(vault
            .search_with_availability("Immediate", 10, None, true)
            .is_empty());

        vault
            .publish_newly_available(&[SearchIndexItem {
                id: "track-new".to_string(),
                name: "Immediate Playback".to_string(),
                item_type: HashedItemType::Track,
                additional_text: vec!["artist:Fast Artist".to_string()],
            }])
            .unwrap();

        let results = vault.search_with_availability("Immediate", 10, None, true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "track-new");
    }

    #[test]
    fn test_record_impression() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        let first_device = ImpressionSource {
            user_id: 1,
            device_id: Some(10),
        };
        let second_device = ImpressionSource {
            user_id: 1,
            device_id: Some(11),
        };
        assert!(vault.record_impression("artist1", HashedItemType::Artist, first_device));
        assert!(!vault.record_impression("artist1", HashedItemType::Artist, first_device));
        assert!(vault.record_impression("artist1", HashedItemType::Artist, second_device));
        assert!(vault.record_impression("album1", HashedItemType::Album, first_device));

        // Verify they were recorded
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT impression_count FROM item_impressions WHERE item_id = 'artist1' AND item_type = 'artist'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        let count: i64 = conn
            .query_row(
                "SELECT impression_count FROM item_impressions WHERE item_id = 'album1' AND item_type = 'album'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_impression_daily_budgets() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();
        let today = chrono::Utc::now()
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap();

        {
            let mut conn = vault.write_conn.lock().unwrap();
            let transaction = conn.transaction().unwrap();
            for index in 0..IMPRESSION_DEVICE_DAILY_BUDGET {
                transaction
                    .execute(
                        "INSERT INTO item_impression_events
                         (user_id, device_id, item_id, item_type, date)
                         VALUES (1, 10, ?1, 'track', ?2)",
                        rusqlite::params![format!("device-item-{index}"), today],
                    )
                    .unwrap();
            }
            for index in 0..IMPRESSION_USER_DAILY_BUDGET {
                let device_id = 20 + index / IMPRESSION_DEVICE_DAILY_BUDGET;
                transaction
                    .execute(
                        "INSERT INTO item_impression_events
                         (user_id, device_id, item_id, item_type, date)
                         VALUES (2, ?1, ?2, 'track', ?3)",
                        rusqlite::params![device_id, format!("user-item-{index}"), today],
                    )
                    .unwrap();
            }
            transaction.commit().unwrap();
        }

        assert!(!vault.record_impression(
            "over-device-budget",
            HashedItemType::Track,
            ImpressionSource {
                user_id: 1,
                device_id: Some(10),
            },
        ));
        assert!(!vault.record_impression(
            "over-user-budget",
            HashedItemType::Track,
            ImpressionSource {
                user_id: 2,
                device_id: Some(99),
            },
        ));
        assert!(vault.get_impression_totals(today).is_empty());
    }

    #[test]
    fn test_get_impression_totals() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Insert impressions with different dates directly
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20250101, 10)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20250102, 5)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a2', 'album', 20250101, 3)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a3', 'track', 20241201, 100)",
                [],
            ).unwrap();
        }

        // Get totals from 2025-01-01 onwards
        let totals = vault.get_impression_totals(20250101);

        assert_eq!(
            totals.get(&("a1".to_string(), HashedItemType::Artist)),
            Some(&15)
        );
        assert_eq!(
            totals.get(&("a2".to_string(), HashedItemType::Album)),
            Some(&3)
        );
        // a3 should not be included (date is before min_date)
        assert_eq!(totals.get(&("a3".to_string(), HashedItemType::Track)), None);
    }

    #[test]
    fn test_prune_impressions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();

        // Insert impressions with different dates
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20240101, 10)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a2', 'artist', 20250101, 5)",
                [],
            ).unwrap();
        }

        // Prune old impressions (before 2025)
        let pruned = vault.prune_impressions(20250101);
        assert_eq!(pruned, 1);

        // Verify only newer record remains
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_impressions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn enriched_search_normalizes_accents_and_ranks_cross_field_matches() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let items = vec![
            SearchableItem {
                id: "chopin".into(),
                name: "Frédéric Chopin".into(),
                content_type: SearchableContentType::Artist,
                additional_text: vec!["extra:romantic classical".into()],
                is_available: true,
            },
            SearchableItem {
                id: "chopin_etude".into(),
                name: "Étude, Op. 10 No. 3".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec![
                    "artist:Frédéric Chopin".into(),
                    "album:Complete Études".into(),
                ],
                is_available: true,
            },
            SearchableItem {
                id: "rach_etude".into(),
                name: "Étude-tableau".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Sergei Rachmaninoff".into()],
                is_available: true,
            },
        ];
        let vault = Fts5LevenshteinSearchVault::new(
            Arc::new(MockCatalogStore::new(items)),
            &db_path,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();

        assert_eq!(vault.search("etude", 10, None)[0].item_id, "chopin_etude");
        let results = vault.search("chopin etude", 10, None);
        assert_eq!(results[0].item_id, "chopin_etude");
        assert!(
            results
                .iter()
                .position(|item| item.item_id == "chopin_etude")
                < results.iter().position(|item| item.item_id == "rach_etude")
        );
        assert_eq!(
            vault.search("Etude—Op. 10", 10, None)[0].item_id,
            "chopin_etude"
        );
        assert_eq!(
            vault.search("\"etude op 10\"", 10, None)[0].item_id,
            "chopin_etude"
        );
        assert!(vault.search("\"chopin etude\"", 10, None).is_empty());
    }

    #[test]
    fn enriched_search_applies_filters_availability_dedup_and_limits() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let items = vec![
            SearchableItem {
                id: "artist".into(),
                name: "Chopin".into(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "available".into(),
                name: "Chopin Etude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "missing".into(),
                name: "Chopin Etude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: false,
            },
        ];
        let vault = Fts5LevenshteinSearchVault::new(
            Arc::new(MockCatalogStore::new(items)),
            &db_path,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        let results =
            vault.search_with_availability("chopin", 1, Some(vec![HashedItemType::Track]), true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "available");
        let repeated = vault.search("chopin etude", 10, None);
        assert_eq!(
            repeated
                .iter()
                .filter(|item| item.item_id == "available")
                .count(),
            1
        );
        assert_eq!(
            repeated
                .iter()
                .map(|item| (&item.item_type, &item.item_id))
                .collect::<Vec<_>>(),
            vault
                .search("chopin etude", 10, None)
                .iter()
                .map(|item| (&item.item_type, &item.item_id))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn legacy_index_serves_during_side_build_and_swaps_atomically() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE search_index USING fts5(
                item_id UNINDEXED, item_type UNINDEXED, name, tokenize='trigram');
             INSERT INTO search_index(item_id,item_type,name) VALUES('legacy','artist','Legacy Artist');",
        ).unwrap();
        drop(conn);

        let vault = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap(),
        );
        assert_eq!(vault.search("Legacy", 10, None)[0].item_id, "legacy");
        vault.start_background_build(Arc::new(MockCatalogStore::new(vec![SearchableItem {
            id: "new".into(),
            name: "Étude Collection".into(),
            content_type: SearchableContentType::Album,
            additional_text: vec!["artist:Chopin".into()],
            is_available: true,
        }])));
        for _ in 0..100 {
            if vault.get_stats().state == IndexState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(vault.search("chopin etude", 10, None)[0].item_id, "new");
        let conn = Connection::open(&db_path).unwrap();
        assert!(Fts5LevenshteinSearchVault::is_enriched_index(&conn, ACTIVE_INDEX).unwrap());
        assert!(Fts5LevenshteinSearchVault::table_exists(&conn, PREVIOUS_INDEX).unwrap());
        assert_eq!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "active_search_schema_version")
                .as_deref(),
            Some("2")
        );
    }

    #[test]
    fn side_build_replays_incremental_mutations_before_activation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");
        let vault =
            Fts5LevenshteinSearchVault::new_lazy(&db_path, &crate::backup::DbRegistry::new())
                .unwrap();
        let baseline = SearchableItem {
            id: "baseline".into(),
            name: "Baseline".into(),
            content_type: SearchableContentType::Track,
            additional_text: vec![],
            is_available: true,
        };
        {
            let conn = vault.write_conn.lock().unwrap();
            Fts5LevenshteinSearchVault::create_enriched_index(&conn, BUILD_INDEX).unwrap();
            let mut stmt = conn
                .prepare(
                    "INSERT INTO search_index_building
                 (item_id,item_type,display_name,primary_name,artist_text,album_text,extra_text)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                )
                .unwrap();
            Fts5LevenshteinSearchVault::insert_searchable_item(&mut stmt, &baseline).unwrap();
            drop(stmt);
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_in_progress", "true").unwrap();
        }
        vault
            .upsert_items(&[SearchIndexItem {
                id: "during_build".into(),
                name: "Mutation Étude".into(),
                item_type: HashedItemType::Track,
                additional_text: vec!["artist:Chopin".into()],
            }])
            .unwrap();
        {
            let conn = vault.write_conn.lock().unwrap();
            let mutation = SearchableItem {
                id: "during_build".into(),
                name: "Mutation Étude".into(),
                content_type: SearchableContentType::Track,
                additional_text: vec!["artist:Chopin".into()],
                is_available: true,
            };
            Fts5LevenshteinSearchVault::activate_built_index(&conn, &[baseline, mutation]).unwrap();
        }
        assert_eq!(
            vault.search("mutation etude", 10, None)[0].item_id,
            "during_build"
        );
    }
}
