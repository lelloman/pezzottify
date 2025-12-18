# Plan: Swappable Search Mechanisms

## Status Summary

### Completed ✅
- **Step 1-6**: Runtime-configurable search engine via CLI (`--search-engine`) and config (`[search] engine`)
- **Step 8**: Live index sync (add/update/remove items without restart)
- **Step 9a**: Typo-tolerant search via FTS5 + Levenshtein (implemented as `fts5-levenshtein` engine)

### Available Search Engines
| Engine | CLI Value | Description |
|--------|-----------|-------------|
| PezzotHash | `pezzothash` | SimHash-based fuzzy search (default) |
| FTS5 | `fts5` | SQLite FTS5 with trigram tokenizer |
| FTS5+Levenshtein | `fts5-levenshtein` | FTS5 with typo correction |
| NoOp | `noop` | Disabled (fastest startup) |

### Not Yet Implemented
- **Step 10**: Parallel category search (optional, for 100k+ item catalogs)

### Next Up: Step 9b - Popularity Weighting (FTS5-Levenshtein only)
See detailed implementation plan below.

---

## Current State

- `SearchVault` trait in `catalog-server/src/search/search_vault.rs:173` already abstracts search
- Two implementations exist: `PezzotHashSearchVault` (SimHash) and `NoOpSearchVault`
- Selection is **compile-time** via `#[cfg(feature = "no_search")]` in `main.rs:285-290`
- Search vault stored as `Arc<Mutex<Box<dyn SearchVault>>>` in `ServerState`

## Goal

Runtime-configurable search mechanism selection via config file/CLI, with ability to add new implementations (FTS5, etc.).

## Implementation Steps

### 1. Add search engine enum to config ✅ DONE

**File: `catalog-server/src/config/file_config.rs`**

Add a new section to `FileConfig`:

```rust
#[derive(Debug, Deserialize, Default, Clone)]
pub struct SearchConfig {
    pub engine: Option<String>,  // "pezzothash", "fts5", "noop"
}
```

Add to `FileConfig`:
```rust
pub search: Option<SearchConfig>,
```

**File: `catalog-server/src/config/mod.rs`**

Add enum and settings to `AppConfig`:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SearchEngine {
    #[default]
    PezzotHash,
    Fts5,
    NoOp,
}

#[derive(Debug, Clone)]
pub struct SearchSettings {
    pub engine: SearchEngine,
}
```

Add to `AppConfig`:
```rust
pub search: SearchSettings,
```

Parse in `AppConfig::resolve()`:
```rust
let search_engine = file.search
    .and_then(|s| s.engine)
    .map(|e| match e.to_lowercase().as_str() {
        "fts5" => SearchEngine::Fts5,
        "noop" | "none" => SearchEngine::NoOp,
        _ => SearchEngine::PezzotHash,  // default
    })
    .unwrap_or_default();
```

### 2. Add CLI argument ✅ DONE

**File: `catalog-server/src/main.rs`**

Add to CLI args:
```rust
#[arg(long, default_value = "pezzothash")]
search_engine: String,
```

Add to `CliConfig`:
```rust
pub search_engine: String,
```

### 3. Create FTS5 search implementation ✅ DONE

**New file: `catalog-server/src/search/fts5_search.rs`**

```rust
use super::{HashedItemType, SearchResult, SearchVault};
use crate::catalog_store::CatalogStore;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct Fts5SearchVault {
    conn: Mutex<Connection>,
}

impl Fts5SearchVault {
    pub fn new(catalog_store: Arc<dyn CatalogStore>, db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Create FTS5 virtual table
        conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
                item_id,
                item_type,
                name,
                tokenize='trigram'
            );
        "#)?;

        // Index content from catalog
        let searchable = catalog_store.get_searchable_content()?;
        let mut stmt = conn.prepare(
            "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)"
        )?;

        for item in searchable {
            let type_str = match item.content_type {
                SearchableContentType::Artist => "artist",
                SearchableContentType::Album => "album",
                SearchableContentType::Track => "track",
            };
            stmt.execute([&item.id, type_str, &item.name])?;
        }

        Ok(Self { conn: Mutex::new(conn) })
    }
}

impl SearchVault for Fts5SearchVault {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        let conn = self.conn.lock().unwrap();

        // Build query with optional type filter
        let type_filter = filter.map(|types| {
            let type_strs: Vec<&str> = types.iter().map(|t| match t {
                HashedItemType::Artist => "artist",
                HashedItemType::Album => "album",
                HashedItemType::Track => "track",
            }).collect();
            format!("AND item_type IN ({})", type_strs.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(","))
        }).unwrap_or_default();

        let sql = format!(
            r#"SELECT item_id, item_type, name, bm25(search_index) as score
               FROM search_index
               WHERE search_index MATCH ?
               {}
               ORDER BY score
               LIMIT ?"#,
            type_filter
        );

        // Execute and map results
        // ... (error handling, result mapping)
    }
}
```

### 4. Create search vault factory ✅ DONE

**New file: `catalog-server/src/search/factory.rs`**

```rust
use super::{NoOpSearchVault, PezzotHashSearchVault, SearchVault};
use crate::catalog_store::CatalogStore;
use crate::config::SearchEngine;
use std::path::Path;
use std::sync::Arc;

pub fn create_search_vault(
    engine: &SearchEngine,
    catalog_store: Arc<dyn CatalogStore>,
    db_dir: &Path,
) -> Box<dyn SearchVault> {
    match engine {
        SearchEngine::PezzotHash => {
            Box::new(PezzotHashSearchVault::new(catalog_store))
        }
        SearchEngine::Fts5 => {
            let db_path = db_dir.join("search.db");
            Box::new(Fts5SearchVault::new(catalog_store, &db_path).unwrap())
        }
        SearchEngine::NoOp => {
            Box::new(NoOpSearchVault {})
        }
    }
}
```

### 5. Update main.rs to use factory ✅ DONE

**File: `catalog-server/src/main.rs`**

Replace the `#[cfg]` blocks with:

```rust
use pezzottify_catalog_server::search::create_search_vault;

// ...

info!("Indexing content for search using {:?} engine...", app_config.search.engine);
let search_vault = create_search_vault(
    &app_config.search.engine,
    catalog_store.clone(),
    &app_config.db_dir,
);
```

### 6. Update mod.rs exports ✅ DONE

**File: `catalog-server/src/search/mod.rs`**

```rust
mod pezzott_hash;
mod search_vault;
mod fts5_search;
mod factory;

pub use search_vault::*;
pub use fts5_search::Fts5SearchVault;
pub use factory::create_search_vault;
```

### 7. Remove compile-time feature gating (optional)

The `no_search` feature can remain for faster dev builds, but runtime selection takes precedence when not using the feature.

## Config Examples

**config.toml:**
```toml
[search]
engine = "fts5"  # or "pezzothash", "noop"
```

**CLI:**
```bash
cargo run -- --db-dir /path/to/db --search-engine fts5
```

### 8. Live index sync (no server reboot) ✅ DONE

Since catalog updates are now a requirement, all search engines must support live index updates.

**Implementation completed:**
- Added `add_item`, `update_item`, `remove_item` methods to `SearchVault` trait
- `PezzotHashSearchVault` now uses `RwLock<Vec<HashedItem>>` for interior mutability
- `NoOpSearchVault` has empty implementations
- Admin CRUD handlers (artists, albums, tracks) now call search vault mutations after successful catalog operations

**Modify `SearchVault` trait to support mutations:**

```rust
pub trait SearchVault: Send + Sync {
    fn search(&self, query: &str, options: SearchOptions) -> Vec<SearchResult>;

    // Index mutation methods
    fn add_item(&self, id: &str, item_type: HashedItemType, name: &str);
    fn update_item(&self, id: &str, item_type: HashedItemType, name: &str);
    fn remove_item(&self, id: &str, item_type: HashedItemType);
}
```

**Implementation for each engine:**

#### PezzotHash

```rust
impl SearchVault for PezzotHashSearchVault {
    fn add_item(&self, id: &str, item_type: HashedItemType, name: &str) {
        let mut items = self.items.write().unwrap(); // Change to RwLock
        items.push(HashedItem {
            item_id: id.to_string(),
            item_type,
            hash: PezzottHash::calc(name),
        });
    }

    fn update_item(&self, id: &str, item_type: HashedItemType, name: &str) {
        let mut items = self.items.write().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.item_id == id && i.item_type == item_type) {
            item.hash = PezzottHash::calc(name);
        }
    }

    fn remove_item(&self, id: &str, item_type: HashedItemType) {
        let mut items = self.items.write().unwrap();
        items.retain(|i| !(i.item_id == id && i.item_type == item_type));
    }
}
```

Note: Change `Vec<HashedItem>` to be behind `RwLock` instead of just holding it directly, to allow concurrent reads during writes.

#### FTS5 / Fts5Spellfix

```rust
impl SearchVault for Fts5SearchVault {
    fn add_item(&self, id: &str, item_type: HashedItemType, name: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)",
            [id, item_type.as_str(), name],
        ).unwrap();
    }

    fn update_item(&self, id: &str, item_type: HashedItemType, name: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE search_index SET name = ? WHERE item_id = ? AND item_type = ?",
            [name, id, item_type.as_str()],
        ).unwrap();
    }

    fn remove_item(&self, id: &str, item_type: HashedItemType) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM search_index WHERE item_id = ? AND item_type = ?",
            [id, item_type.as_str()],
        ).unwrap();
    }
}
```

For `Fts5Spellfix`, also update the spelling vocabulary when items change.

#### NoOp

```rust
impl SearchVault for NoOpSearchVault {
    fn add_item(&self, _: &str, _: HashedItemType, _: &str) {}
    fn update_item(&self, _: &str, _: HashedItemType, _: &str) {}
    fn remove_item(&self, _: &str, _: HashedItemType) {}
}
```

**Hook into catalog CRUD operations:**

Locate where catalog mutations happen (likely in `catalog_store` or admin routes in `server.rs`) and call search vault mutations.

**Option A: Decorator pattern**

Wrap `CatalogStore` with a decorator that also updates search:

```
┌─────────────────────────────────────┐
│     SearchAwareCatalogStore         │  ← Decorator (implements CatalogStore)
│  ┌───────────────────────────────┐  │
│  │    SqliteCatalogStore         │  │  ← Wrapped component
│  │    (actual storage)           │  │
│  └───────────────────────────────┘  │
│  + search_vault                     │  ← Added behavior
└─────────────────────────────────────┘
```

```rust
pub struct SearchAwareCatalogStore {
    inner: Arc<dyn CatalogStore>,
    search_vault: Arc<dyn SearchVault>,
}

impl CatalogStore for SearchAwareCatalogStore {
    fn add_track(&self, track: &Track) -> Result<()> {
        self.inner.add_track(track)?;
        self.search_vault.add_item(&track.id, HashedItemType::Track, &track.name);
        Ok(())
    }

    fn update_track(&self, track: &Track) -> Result<()> {
        self.inner.update_track(track)?;
        self.search_vault.update_item(&track.id, HashedItemType::Track, &track.name);
        Ok(())
    }

    fn delete_track(&self, id: &str) -> Result<()> {
        self.inner.delete_track(id)?;
        self.search_vault.remove_item(id, HashedItemType::Track);
        Ok(())
    }

    // Same for artists, albums...
    // Read operations just delegate to inner
}
```

**Option B: Event-driven via channel**

Catalog store emits events, search indexer consumes them:

```rust
pub enum CatalogEvent {
    ItemAdded { id: String, item_type: HashedItemType, name: String },
    ItemUpdated { id: String, item_type: HashedItemType, name: String },
    ItemRemoved { id: String, item_type: HashedItemType },
}

// In catalog store
self.event_tx.send(CatalogEvent::ItemAdded { ... })?;

// Background task
async fn index_updater(rx: Receiver<CatalogEvent>, search_vault: Arc<dyn SearchVault>) {
    while let Some(event) = rx.recv().await {
        match event {
            CatalogEvent::ItemAdded { id, item_type, name } => {
                search_vault.add_item(&id, item_type, &name);
            }
            // ...
        }
    }
}
```

**Option C: SQLite triggers (FTS5 only)**

For FTS5 variants, let SQLite handle it automatically via triggers on the catalog tables. This is the most reliable for FTS5 but doesn't work for PezzotHash.

```sql
-- Assuming catalog tables: artists, albums, tracks

CREATE TRIGGER artist_ai AFTER INSERT ON artists BEGIN
    INSERT INTO search_index (item_id, item_type, name) VALUES (NEW.id, 'artist', NEW.name);
END;

CREATE TRIGGER artist_au AFTER UPDATE OF name ON artists BEGIN
    UPDATE search_index SET name = NEW.name WHERE item_id = NEW.id AND item_type = 'artist';
END;

CREATE TRIGGER artist_ad AFTER DELETE ON artists BEGIN
    DELETE FROM search_index WHERE item_id = OLD.id AND item_type = 'artist';
END;

-- Repeat for albums, tracks
```

**Recommendation:**

Use **Option A (Decorator pattern)** because:

1. **Single Responsibility**: `SqliteCatalogStore` handles storage only, decorator adds search indexing. Neither knows about the other's internals.

2. **Composable**: Can stack more decorators later without modifying existing code:
   ```rust
   let store = SqliteCatalogStore::new(...);
   let store = SearchAwareCatalogStore::new(store, search_vault);
   let store = AuditLoggingCatalogStore::new(store, audit_log);  // future
   let store = CachingCatalogStore::new(store, cache);           // future
   ```

3. **Testable**:
   - Test `SqliteCatalogStore` in isolation (no search concerns)
   - Test `SearchAwareCatalogStore` with a mock inner store
   - Test search engines independently

4. **No modification to existing code**: `SqliteCatalogStore` stays unchanged.

5. **Loose coupling**: Decorator depends only on `CatalogStore` trait (not concrete impl) and `SearchVault` trait.

6. **Works uniformly for all search engines**: Same decorator works whether using PezzotHash, FTS5, or any future engine.

Create the decorator in `main.rs` when setting up the catalog store:

```rust
let raw_catalog_store = Arc::new(SqliteCatalogStore::new(...));
let search_vault = create_search_vault(&app_config.search.engine, ...);
let catalog_store: Arc<dyn CatalogStore> = Arc::new(
    SearchAwareCatalogStore::new(raw_catalog_store, search_vault.clone())
);
```

For tests or CLI tools that don't need search:
```rust
let catalog_store: Arc<dyn CatalogStore> = Arc::new(SqliteCatalogStore::new(...));
```

### 9. Add FTS5 + Spellfix1 variant (typo tolerance) ✅ DONE (implemented with Levenshtein instead)

**New file: `catalog-server/src/search/fts5_spellfix_search.rs`**

```rust
pub struct Fts5SpellfixSearchVault {
    conn: Mutex<Connection>,
}

impl Fts5SpellfixSearchVault {
    pub fn new(catalog_store: Arc<dyn CatalogStore>, db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Load spellfix1 extension (must be compiled/available)
        conn.load_extension("spellfix1")?;

        // Create FTS5 index
        conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
                item_id UNINDEXED,
                item_type UNINDEXED,
                name,
                content=''
            );
        "#)?;

        // Create spellfix vocabulary table
        conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS spelling USING spellfix1;
        "#)?;

        // Index content and build vocabulary
        let searchable = catalog_store.get_searchable_content()?;
        for item in &searchable {
            // Add to FTS5
            conn.execute(
                "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)",
                [&item.id, &item.content_type.as_str(), &item.name],
            )?;

            // Add words to spellfix vocabulary
            for word in item.name.split_whitespace() {
                conn.execute(
                    "INSERT OR IGNORE INTO spelling (word) VALUES (lower(?))",
                    [word],
                )?;
            }
        }

        Ok(Self { conn: Mutex::new(conn) })
    }

    fn expand_query(&self, query: &str) -> String {
        // For each word, find spelling corrections within edit distance 2
        let conn = self.conn.lock().unwrap();
        let words: Vec<String> = query
            .split_whitespace()
            .map(|word| {
                let mut stmt = conn
                    .prepare("SELECT word FROM spelling WHERE word MATCH ? AND top=1")
                    .unwrap();
                stmt.query_row([word], |row| row.get(0))
                    .unwrap_or_else(|_| word.to_string())
            })
            .collect();
        words.join(" ")
    }
}

impl SearchVault for Fts5SpellfixSearchVault {
    fn search(&self, query: &str, max_results: usize, filter: Option<Vec<HashedItemType>>) -> Vec<SearchResult> {
        let corrected_query = self.expand_query(query);
        // Then search FTS5 with corrected query...
    }
}
```

Add to `SearchEngine` enum:
```rust
pub enum SearchEngine {
    PezzotHash,
    Fts5,
    Fts5Spellfix,  // NEW
    NoOp,
}
```

### 9b. Add popularity weighting (FTS5-Levenshtein only)

Add popularity weighting to search results in `Fts5LevenshteinSearchVault`. Popular items (based on listening history) will be boosted in search rankings.

**Architecture:**
```
┌─────────────────────┐     writes to      ┌─────────────────────┐
│ PopularContentJob   │ ──────────────────→│ item_popularity     │
│ (extended)          │                    │ table (search.db)   │
└─────────────────────┘                    └─────────────────────┘
                                                    │
                                                    │ LEFT JOIN
                                                    ▼
┌─────────────────────┐     queries        ┌─────────────────────┐
│ Fts5Levenshtein     │ ←─────────────────│ search_index FTS5   │
│ SearchVault         │                    │ + item_popularity   │
└─────────────────────┘                    └─────────────────────┘
```

#### 9b.1: Create `item_popularity` table in search database

**File:** `src/search/fts5_levenshtein_search.rs`

Add table creation in `new()` method after FTS5 table creation:

```sql
CREATE TABLE IF NOT EXISTS item_popularity (
    item_id TEXT NOT NULL,
    item_type TEXT NOT NULL,  -- 'track', 'album', 'artist'
    play_count INTEGER NOT NULL DEFAULT 0,
    score REAL NOT NULL DEFAULT 0.0,  -- normalized 0.0-1.0
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (item_id, item_type)
);
CREATE INDEX IF NOT EXISTS idx_popularity_type ON item_popularity(item_type);
```

#### 9b.2: Add method to update popularity data

**File:** `src/search/fts5_levenshtein_search.rs`

Add new method to `Fts5LevenshteinSearchVault`:

```rust
/// Update popularity scores for items.
/// Scores should be normalized 0.0-1.0 within each type.
pub fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
    let conn = self.conn.lock().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO item_popularity (item_id, item_type, play_count, score, updated_at)
         VALUES (?, ?, ?, ?, ?)"
    ).unwrap();

    for (id, item_type, play_count, score) in items {
        stmt.execute(params![id, item_type_to_str(item_type), play_count, score, now]).ok();
    }
}
```

#### 9b.3: Modify search query to include popularity

**File:** `src/search/fts5_levenshtein_search.rs`

Update the SQL query in `search()` to LEFT JOIN with popularity and combine scores:

```sql
SELECT
    s.item_id,
    s.item_type,
    s.name,
    bm25(search_index) as text_score,
    COALESCE(p.score, 0.0) as popularity_score
FROM search_index s
LEFT JOIN item_popularity p
    ON s.item_id = p.item_id AND s.item_type = p.item_type
WHERE search_index MATCH ?
AND s.item_type IN (?)
ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * 0.5))
LIMIT ?
```

Scoring formula: `bm25_score * (1.0 + popularity * weight)`
- BM25 scores are negative (more negative = better match)
- Multiplying by `(1 + popularity * 0.5)` boosts popular items
- Weight of 0.5 means max 50% boost for most popular items

#### 9b.4: Expose popularity updater via SearchVault trait

**File:** `src/search/search_vault.rs`

Add optional method to trait with default no-op implementation:

```rust
pub trait SearchVault: Send + Sync {
    // ... existing methods ...

    /// Update popularity scores. Default implementation is no-op.
    fn update_popularity(&self, _items: &[(String, HashedItemType, u64, f64)]) {}
}
```

#### 9b.5: Extend PopularContentJob to write popularity data

**File:** `src/background_jobs/jobs/popular_content.rs`

After computing popular content, also write to search vault. Normalize scores within each content type (tracks, albums, artists).

#### 9b.6: Add search_vault to JobContext

**File:** `src/background_jobs/context.rs`

Add optional search vault reference:

```rust
pub struct JobContext {
    // ... existing fields ...
    pub search_vault: Option<Arc<Mutex<Box<dyn SearchVault>>>>,
}
```

**File:** `src/main.rs` - Pass search_vault when creating JobContext.

#### Files to Modify for Step 9b

| File | Changes |
|------|---------|
| `src/search/search_vault.rs` | Add `update_popularity()` to trait |
| `src/search/fts5_levenshtein_search.rs` | Create table, implement update, modify query |
| `src/background_jobs/context.rs` | Add `search_vault` field |
| `src/background_jobs/jobs/popular_content.rs` | Write popularity after computing |
| `src/main.rs` | Pass search_vault to JobContext |

#### Notes
- Items not in `item_popularity` table get `score = 0.0` (no boost)
- Popularity scores are relative within each run (max = 1.0)
- The 0.5 weight factor can be tuned later based on user feedback

### 10. Parallel category search (optional optimization)

For large catalogs, search each category in parallel:

```rust
pub struct ParallelSearchVault<V: SearchVault> {
    artist_vault: V,
    album_vault: V,
    track_vault: V,
}

impl<V: SearchVault + Send + Sync> SearchVault for ParallelSearchVault<V> {
    fn search(&self, query: &str, options: SearchOptions) -> Vec<SearchResult> {
        let (artists, albums, tracks) = rayon::join(
            || self.artist_vault.search(query, artist_options),
            || rayon::join(
                || self.album_vault.search(query, album_options),
                || self.track_vault.search(query, track_options),
            ),
        );

        // Interleave results: artist, album, track, artist, album, track...
        interleave_results(artists, albums.0, albums.1, options.max_results)
    }
}
```

This is probably overkill for most catalogs but could help with 100k+ items.

## Search Engine Variants Summary

| Engine | Fuzzy | Typo Tolerance | Popularity | Memory |
|--------|-------|----------------|------------|--------|
| `pezzothash` | SimHash | Yes (built-in) | No | Unbounded |
| `fts5` | Trigram | Partial | No | Bounded |
| `fts5-levenshtein` | Trigram + Levenshtein | Yes | Planned (Step 9b) | Bounded |
| `noop` | - | - | - | Zero |

### Files Created
- `src/search/fts5_search.rs` - FTS5 implementation
- `src/search/fts5_levenshtein_search.rs` - FTS5 + Levenshtein typo correction
- `src/search/levenshtein.rs` - Levenshtein distance algorithm and Vocabulary
- `src/search/factory.rs` - Search vault factory function

## Future Extensions

Once this infrastructure is in place, adding new search engines is straightforward:

1. Create new struct implementing `SearchVault`
2. Add variant to `SearchEngine` enum
3. Add case to factory function
4. Add config parsing for new engine name

Potential future engines:
- `tantivy` - Rust full-text search library (like Lucene)
- `meilisearch` - External search service integration

## Files to Create/Modify

| File | Action |
|------|--------|
| `src/search/fts5_search.rs` | Create |
| `src/search/fts5_spellfix_search.rs` | Create |
| `src/search/factory.rs` | Create |
| `src/search/mod.rs` | Modify |
| `src/search/search_vault.rs` | Modify (add SearchOptions, CategoryWeights, mutation methods) |
| `src/catalog_store/search_aware.rs` | Create (SearchAwareCatalogStore wrapper) |
| `src/catalog_store/mod.rs` | Modify (export wrapper) |
| `src/config/file_config.rs` | Modify |
| `src/config/mod.rs` | Modify |
| `src/main.rs` | Modify (use wrapper, runtime engine selection) |
| `src/lib.rs` | Modify (exports) |

## Notes on spellfix1

The spellfix1 extension is **not** bundled with SQLite by default. Options:
1. Compile SQLite from source with `-DSQLITE_ENABLE_LOAD_EXTENSION`
2. Use `rusqlite` with `bundled` feature and compile spellfix1 separately
3. Ship a pre-compiled `.so`/`.dylib` with the server

This adds deployment complexity. Alternative: implement edit-distance in Rust (Levenshtein) and use it to expand queries before hitting FTS5 - no extension needed.
