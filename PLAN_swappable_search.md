# Plan: Swappable Search Mechanisms

## Current State

- `SearchVault` trait in `catalog-server/src/search/search_vault.rs:173` already abstracts search
- Two implementations exist: `PezzotHashSearchVault` (SimHash) and `NoOpSearchVault`
- Selection is **compile-time** via `#[cfg(feature = "no_search")]` in `main.rs:285-290`
- Search vault stored as `Arc<Mutex<Box<dyn SearchVault>>>` in `ServerState`

## Goal

Runtime-configurable search mechanism selection via config file/CLI, with ability to add new implementations (FTS5, etc.).

## Implementation Steps

### 1. Add search engine enum to config

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

### 2. Add CLI argument

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

### 3. Create FTS5 search implementation

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

### 4. Create search vault factory

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

### 5. Update main.rs to use factory

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

### 6. Update mod.rs exports

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

### 8. Live index sync (no server reboot)

Since catalog updates are now a requirement, all search engines must support live index updates.

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

### 9. Add FTS5 + Spellfix1 variant (typo tolerance)

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

### 9. Add popularity and category weighting

**Modify `SearchVault` trait to support weighted search:**

```rust
pub struct SearchOptions {
    pub max_results: usize,
    pub filter: Option<Vec<HashedItemType>>,
    pub category_weights: Option<CategoryWeights>,
    pub use_popularity: bool,
}

pub struct CategoryWeights {
    pub artist: f64,  // e.g., 2.0
    pub album: f64,   // e.g., 1.5
    pub track: f64,   // e.g., 1.0
}

pub trait SearchVault {
    fn search(&self, query: &str, options: SearchOptions) -> Vec<SearchResult>;
}
```

**For FTS5 variants, apply in SQL:**

```sql
SELECT
    item_id,
    item_type,
    name,
    bm25(search_index)
        * CASE item_type
            WHEN 'artist' THEN :artist_weight
            WHEN 'album' THEN :album_weight
            ELSE :track_weight
          END
        * COALESCE(popularity_score, 1.0) as final_score
FROM search_index
LEFT JOIN item_popularity ON search_index.item_id = item_popularity.id
WHERE search_index MATCH :query
ORDER BY final_score DESC
LIMIT :max_results
```

**Popularity source:** Could come from:
- Play counts (already tracked in user listening history)
- A background job that computes popularity scores periodically
- External data (e.g., Spotify popularity)

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
| `PezzotHash` | SimHash | Yes (built-in) | No | Unbounded |
| `Fts5` | Trigram | Partial | Configurable | Bounded |
| `Fts5Spellfix` | Trigram + Edit Distance | Yes | Configurable | Bounded |
| `NoOp` | - | - | - | Zero |

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
