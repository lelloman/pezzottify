//! Organic search index growth.
//!
//! This module provides organic, on-demand indexing of catalog items.
//! Items are indexed when they are "touched" (accessed by users), and their
//! related items are expanded into the index automatically.
//!
//! ## Expansion Rules
//!
//! - **Artist touched**: Index artist + related artists + all albums in discography
//! - **Album touched**: Index album + all artists on it + all tracks
//! - **Track touched**: Index track + its album + all artists on track
//!
//! ## Architecture
//!
//! The indexer maintains:
//! - An in-memory `HashSet` for O(1) "already indexed?" checks
//! - An async queue for non-blocking indexing
//! - A background worker that processes items in batches

use crate::catalog_store::CatalogStore;
use crate::search::{HashedItemType, SearchIndexItem, SearchVault};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Maximum queue capacity before dropping new items
const QUEUE_CAPACITY: usize = 10_000;

/// Batch size for processing multiple items at once
const BATCH_SIZE: usize = 100;

/// Type of item to index with expansion
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IndexTask {
    Artist(String),
    Album(String),
    Track(String),
}

impl IndexTask {
    /// Get the item ID for logging/tracing
    pub fn id(&self) -> &str {
        match self {
            IndexTask::Artist(id) => id,
            IndexTask::Album(id) => id,
            IndexTask::Track(id) => id,
        }
    }

    /// Get a display string for logging
    pub fn type_name(&self) -> &'static str {
        match self {
            IndexTask::Artist(_) => "artist",
            IndexTask::Album(_) => "album",
            IndexTask::Track(_) => "track",
        }
    }
}

/// Combined ID for tracking indexed items
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IndexedItemId {
    id: String,
    item_type: HashedItemType,
}

/// Organic search indexer that grows the index based on user activity.
pub struct OrganicIndexer {
    /// Set of already-indexed item IDs for fast lookup
    indexed_ids: RwLock<HashSet<IndexedItemId>>,
    /// Channel sender for queueing index tasks
    queue_tx: mpsc::Sender<IndexTask>,
    /// Whether the background worker is running
    is_running: RwLock<bool>,
}

impl OrganicIndexer {
    /// Create a new organic indexer and start the background worker.
    ///
    /// # Arguments
    /// * `search_vault` - The search vault to add items to
    /// * `catalog_store` - The catalog store to fetch related items from
    pub fn new(
        search_vault: Arc<dyn SearchVault>,
        catalog_store: Arc<dyn CatalogStore>,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);

        let indexer = Arc::new(Self {
            indexed_ids: RwLock::new(HashSet::new()),
            queue_tx: tx,
            is_running: RwLock::new(true),
        });

        // Start background worker
        let indexer_clone = Arc::clone(&indexer);
        tokio::spawn(async move {
            indexer_clone
                .background_worker(rx, search_vault, catalog_store)
                .await;
        });

        indexer
    }

    /// Queue an artist to be indexed (with expansion to related artists and albums).
    pub fn touch_artist(&self, artist_id: &str) {
        if self.is_indexed(artist_id, HashedItemType::Artist) {
            return;
        }

        if let Err(e) = self
            .queue_tx
            .try_send(IndexTask::Artist(artist_id.to_string()))
        {
            debug!("Failed to queue artist {} for indexing: {}", artist_id, e);
        }
    }

    /// Queue an album to be indexed (with expansion to artists and tracks).
    pub fn touch_album(&self, album_id: &str) {
        if self.is_indexed(album_id, HashedItemType::Album) {
            return;
        }

        if let Err(e) = self
            .queue_tx
            .try_send(IndexTask::Album(album_id.to_string()))
        {
            debug!("Failed to queue album {} for indexing: {}", album_id, e);
        }
    }

    /// Queue a track to be indexed (with expansion to album and artists).
    pub fn touch_track(&self, track_id: &str) {
        if self.is_indexed(track_id, HashedItemType::Track) {
            return;
        }

        if let Err(e) = self
            .queue_tx
            .try_send(IndexTask::Track(track_id.to_string()))
        {
            debug!("Failed to queue track {} for indexing: {}", track_id, e);
        }
    }

    /// Check if an item is already indexed.
    pub fn is_indexed(&self, id: &str, item_type: HashedItemType) -> bool {
        let ids = self.indexed_ids.read().unwrap();
        ids.contains(&IndexedItemId {
            id: id.to_string(),
            item_type,
        })
    }

    /// Mark an item as indexed.
    fn mark_indexed(&self, id: &str, item_type: HashedItemType) {
        let mut ids = self.indexed_ids.write().unwrap();
        ids.insert(IndexedItemId {
            id: id.to_string(),
            item_type,
        });
    }

    /// Get the number of items marked as indexed.
    pub fn indexed_count(&self) -> usize {
        self.indexed_ids.read().unwrap().len()
    }

    /// Get queue backlog size (approximate).
    pub fn queue_size(&self) -> usize {
        // Note: This is an approximation based on channel capacity
        QUEUE_CAPACITY.saturating_sub(self.queue_tx.capacity())
    }

    /// Background worker that processes index tasks.
    async fn background_worker(
        self: &Arc<Self>,
        mut rx: mpsc::Receiver<IndexTask>,
        search_vault: Arc<dyn SearchVault>,
        catalog_store: Arc<dyn CatalogStore>,
    ) {
        info!("Organic indexer background worker started");

        let batch: Arc<Mutex<Vec<SearchIndexItem>>> =
            Arc::new(Mutex::new(Vec::with_capacity(BATCH_SIZE)));
        let indexer = Arc::clone(self);

        loop {
            // Check if we should stop
            if !*self.is_running.read().unwrap() {
                break;
            }

            // Try to receive a task with timeout
            let task =
                match tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await {
                    Ok(Some(task)) => task,
                    Ok(None) => {
                        // Channel closed
                        break;
                    }
                    Err(_) => {
                        // Timeout - flush any pending batch and continue
                        let mut batch_guard = batch.lock().unwrap();
                        if !batch_guard.is_empty() {
                            self.flush_batch(&search_vault, &mut batch_guard);
                        }
                        continue;
                    }
                };

            // Process the task and collect items to index
            let task_clone = task.clone();
            let catalog_store_clone = Arc::clone(&catalog_store);
            let indexer_clone = Arc::clone(&indexer);
            let batch_clone = Arc::clone(&batch);

            let task_id = format!("{}:{}", task.type_name(), task.id());

            let result = tokio::task::spawn_blocking(move || {
                let mut batch = batch_clone.lock().unwrap();
                indexer_clone.process_task(&task_clone, &catalog_store_clone, &mut batch);
            })
            .await;

            if let Err(e) = result {
                error!("Task {} processing failed: {}", task_id, e);
            }

            // Flush batch when full
            {
                let batch_guard = batch.lock().unwrap();
                if batch_guard.len() >= BATCH_SIZE {
                    drop(batch_guard);
                    let mut batch_guard = batch.lock().unwrap();
                    self.flush_batch(&search_vault, &mut batch_guard);
                }
            }
        }

        // Flush remaining items
        {
            let mut batch_guard = batch.lock().unwrap();
            if !batch_guard.is_empty() {
                self.flush_batch(&search_vault, &mut batch_guard);
            }
        }

        info!("Organic indexer background worker stopped");
    }

    /// Process a single index task and collect items to add to the batch.
    fn process_task(
        &self,
        task: &IndexTask,
        catalog_store: &Arc<dyn CatalogStore>,
        batch: &mut Vec<SearchIndexItem>,
    ) {
        match task {
            IndexTask::Artist(id) => self.expand_artist(id, catalog_store, batch),
            IndexTask::Album(id) => self.expand_album(id, catalog_store, batch),
            IndexTask::Track(id) => self.expand_track(id, catalog_store, batch),
        }
    }

    /// Expand an artist: index artist + related artists + all albums.
    fn expand_artist(
        &self,
        artist_id: &str,
        catalog_store: &Arc<dyn CatalogStore>,
        batch: &mut Vec<SearchIndexItem>,
    ) {
        // Skip if already indexed
        if self.is_indexed(artist_id, HashedItemType::Artist) {
            return;
        }

        // Get resolved artist (includes related artists)
        let resolved_artist = match catalog_store.get_resolved_artist(artist_id) {
            Ok(Some(a)) => a,
            Ok(None) => {
                debug!("Artist {} not found in catalog", artist_id);
                return;
            }
            Err(e) => {
                warn!("Failed to get artist {}: {}", artist_id, e);
                return;
            }
        };

        // Index the artist itself
        self.add_to_batch(
            batch,
            &resolved_artist.artist.id,
            &resolved_artist.artist.name,
            HashedItemType::Artist,
        );

        // Index related artists
        for related in &resolved_artist.related_artists {
            if !self.is_indexed(&related.id, HashedItemType::Artist) {
                self.add_to_batch(batch, &related.id, &related.name, HashedItemType::Artist);
            }
        }

        // Get and index discography (albums) - fetch all for indexing purposes
        if let Ok(Some(discography)) = catalog_store.get_discography(
            artist_id,
            1000, // Large limit for indexing
            0,
            crate::catalog_store::DiscographySort::Popularity,
            false,
        ) {
            for album in discography.albums.iter() {
                if !self.is_indexed(&album.id, HashedItemType::Album) {
                    self.add_to_batch(batch, &album.id, &album.name, HashedItemType::Album);
                }
            }
        }
    }

    /// Expand an album: index album + all artists + all tracks.
    fn expand_album(
        &self,
        album_id: &str,
        catalog_store: &Arc<dyn CatalogStore>,
        batch: &mut Vec<SearchIndexItem>,
    ) {
        // Skip if already indexed
        if self.is_indexed(album_id, HashedItemType::Album) {
            return;
        }

        // Get resolved album (includes artists and tracks)
        let resolved_album = match catalog_store.get_resolved_album(album_id) {
            Ok(Some(a)) => a,
            Ok(None) => {
                debug!("Album {} not found in catalog", album_id);
                return;
            }
            Err(e) => {
                warn!("Failed to get album {}: {}", album_id, e);
                return;
            }
        };

        // Index the album itself
        self.add_to_batch(
            batch,
            &resolved_album.album.id,
            &resolved_album.album.name,
            HashedItemType::Album,
        );

        // Index all artists on the album
        for artist in &resolved_album.artists {
            if !self.is_indexed(&artist.id, HashedItemType::Artist) {
                self.add_to_batch(batch, &artist.id, &artist.name, HashedItemType::Artist);
            }
        }

        // Index all tracks on the album
        for disc in &resolved_album.discs {
            for track in &disc.tracks {
                if !self.is_indexed(&track.id, HashedItemType::Track) {
                    self.add_to_batch(batch, &track.id, &track.name, HashedItemType::Track);
                }
            }
        }
    }

    /// Expand a track: index track + album + all artists.
    fn expand_track(
        &self,
        track_id: &str,
        catalog_store: &Arc<dyn CatalogStore>,
        batch: &mut Vec<SearchIndexItem>,
    ) {
        // Skip if already indexed
        if self.is_indexed(track_id, HashedItemType::Track) {
            return;
        }

        // Get resolved track (includes album and artists)
        let resolved_track = match catalog_store.get_resolved_track(track_id) {
            Ok(Some(t)) => t,
            Ok(None) => {
                debug!("Track {} not found in catalog", track_id);
                return;
            }
            Err(e) => {
                warn!("Failed to get track {}: {}", track_id, e);
                return;
            }
        };

        // Index the track itself
        self.add_to_batch(
            batch,
            &resolved_track.track.id,
            &resolved_track.track.name,
            HashedItemType::Track,
        );

        // Index the album
        if !self.is_indexed(&resolved_track.album.id, HashedItemType::Album) {
            self.add_to_batch(
                batch,
                &resolved_track.album.id,
                &resolved_track.album.name,
                HashedItemType::Album,
            );
        }

        // Index all artists on the track
        for track_artist in &resolved_track.artists {
            if !self.is_indexed(&track_artist.artist.id, HashedItemType::Artist) {
                self.add_to_batch(
                    batch,
                    &track_artist.artist.id,
                    &track_artist.artist.name,
                    HashedItemType::Artist,
                );
            }
        }
    }

    /// Add an item to the batch and mark it as indexed.
    fn add_to_batch(
        &self,
        batch: &mut Vec<SearchIndexItem>,
        id: &str,
        name: &str,
        item_type: HashedItemType,
    ) {
        // Double-check to avoid duplicates in the same batch
        if self.is_indexed(id, item_type) {
            return;
        }

        batch.push(SearchIndexItem {
            id: id.to_string(),
            name: name.to_string(),
            item_type,
        });

        // Mark as indexed immediately to prevent duplicates
        self.mark_indexed(id, item_type);
    }

    /// Flush the batch to the search vault.
    fn flush_batch(&self, search_vault: &Arc<dyn SearchVault>, batch: &mut Vec<SearchIndexItem>) {
        if batch.is_empty() {
            return;
        }

        let count = batch.len();

        if let Err(e) = search_vault.upsert_items(batch) {
            error!("Failed to upsert {} items to search index: {}", count, e);
            // Note: Items are already marked as indexed, so we won't retry
            // This is acceptable for organic growth
        } else {
            debug!("Flushed {} items to search index", count);
        }

        batch.clear();
    }

    /// Shutdown the background worker gracefully.
    pub fn shutdown(&self) {
        let mut running = self.is_running.write().unwrap();
        *running = false;
        info!("Organic indexer shutdown requested");
    }

    /// Load indexed item IDs from the search vault (for persistence across restarts).
    ///
    /// This should be called during initialization to restore the indexed state
    /// from the existing search database.
    pub fn load_indexed_from_vault(&self, _search_vault: &Arc<dyn SearchVault>) {
        // The search vault already tracks what's indexed in its FTS5 table
        // We could query it to populate our in-memory set, but for simplicity
        // we start fresh each time - the vault will de-dup on upsert anyway
        info!("Organic indexer initialized (starting with fresh tracking set)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_item_id_hash() {
        let id1 = IndexedItemId {
            id: "abc123".to_string(),
            item_type: HashedItemType::Artist,
        };
        let id2 = IndexedItemId {
            id: "abc123".to_string(),
            item_type: HashedItemType::Album,
        };
        let id3 = IndexedItemId {
            id: "abc123".to_string(),
            item_type: HashedItemType::Artist,
        };

        // Same ID but different type = different
        assert_ne!(id1, id2);
        // Same ID and type = equal
        assert_eq!(id1, id3);

        let mut set = HashSet::new();
        set.insert(id1.clone());
        assert!(set.contains(&id1));
        assert!(!set.contains(&id2));
        assert!(set.contains(&id3));
    }
}
