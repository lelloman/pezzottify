# SQLite Catalog Migration Progress

This file tracks the progress of migrating the catalog from filesystem to SQLite.
Based on the design doc: `CATALOG_SQLITE_DESIGN.md`

## Completed Tasks

### Phase 1: Foundation
- [x] **1.1** Define SQLite schema with migrations (`src/catalog_store/schema.rs`)
- [x] **1.2** Implement new Rust model structs (`src/catalog_store/models.rs`)
- [x] **1.3** Implement SqliteCatalogStore with read operations (`src/catalog_store/store.rs`)

### Phase 2: Import Tool
- [x] **2** Create catalog-import binary (`src/catalog_import.rs`) - *Removed in Phase 6*

### Phase 3: Server Integration
- [x] **3.8** Replace GuardedCatalog with Arc<dyn CatalogStore> (`src/server/state.rs`)
- [x] **3.9** Update server handlers to use new store (`src/server/server.rs`, `src/server/search.rs`, etc.)
- [x] **3.10** Add --catalog-db CLI argument to switch between legacy and SQLite catalog stores

### Phase 4: Write Operations
- [x] **4.11** Implement write operations in SqliteCatalogStore (insert_*, add_* methods)
- [x] **4.12** Add catalog editing API endpoints
  - Added CRUD endpoints for artists, albums, tracks, and images
  - Endpoints: POST/PUT/DELETE for `/v1/content/artist`, `/v1/content/album`, `/v1/content/track`, `/v1/content/image`
  - Protected by `EditCatalog` permission
- [x] **4.13** Add validation for write operations
  - Created validation module (`src/catalog_store/validation.rs`)
  - Field validation: required fields, positive values, non-empty strings
  - Foreign key validation: track.album_id must reference existing album
  - Duplicate ID detection before insert
  - All write operations wrapped in `BEGIN IMMEDIATE` transactions

### Phase 5: Search
- [x] **5.14-5.16** Search evaluation deferred
  - PezzotHashSearchVault already works with SqliteCatalogStore
  - FTS5 migration is optional future optimization

### Phase 6: Cleanup
- [x] **6.17** Remove filesystem catalog loading code
  - Removed `src/catalog/` directory
  - Removed `catalog-import` binary
- [x] **6.18** Remove old model definitions
  - Removed `LegacyCatalogAdapter`
  - Added `NullCatalogStore` for CLI tools that don't need catalog
- [x] **6.19** Update tests
  - Updated test fixtures to create SQLite catalog database
  - Updated e2e tests for new JSON response shapes
- [x] **6.20** Update documentation

## Migration Complete

The SQLite catalog migration is now complete. The server requires a SQLite catalog database.

### Current CLI Usage

```bash
# Run server (SQLite catalog is now required)
cargo run -- <catalog-db-path> <user-db-path> [options]

# Options:
#   --media-path <path>     Path to media files (audio/images), defaults to parent of catalog-db
#   --port <port>           Server port (default: 3001)
#   --metrics-port <port>   Metrics port (default: 9091)
#   --logging-level <level> Request logging level (default: path)
#   --content-cache-age-sec <seconds>  Cache duration (default: 3600)
#   --frontend-dir-path <path>  Serve static frontend files
```

### API Response Shapes

The new API returns different JSON structures than the old filesystem-based catalog:

- **Artist endpoint** (`/v1/content/artist/{id}`): Returns `ResolvedArtist` with nested `artist` field
  ```json
  { "artist": { "id": "...", "name": "..." }, "images": [...], "related_artists": [...] }
  ```

- **Album endpoint** (`/v1/content/album/{id}`): Returns `Album` directly
  ```json
  { "id": "...", "name": "...", "album_type": "Album", ... }
  ```

- **Track endpoint** (`/v1/content/track/{id}`): Returns `Track` directly
  ```json
  { "id": "...", "name": "...", "album_id": "...", ... }
  ```

- **Resolved Track** (`/v1/content/track/{id}/resolved`): Returns `ResolvedTrack`
  ```json
  { "track": {...}, "album": {...}, "artists": [...] }
  ```

- **Discography** (`/v1/content/artist/{id}/discography`): Returns `ArtistDiscography`
  ```json
  { "albums": [{ "id": "...", "name": "..." }, ...], "features": [...] }
  ```

## Key Files

### Current Structure
- `src/catalog_store/mod.rs` - Module root
- `src/catalog_store/schema.rs` - SQLite schema definitions
- `src/catalog_store/models.rs` - Model structs (Artist, Album, Track, etc.)
- `src/catalog_store/store.rs` - SqliteCatalogStore implementation
- `src/catalog_store/trait_def.rs` - CatalogStore trait
- `src/catalog_store/null_store.rs` - NullCatalogStore for CLI tools
- `src/catalog_store/validation.rs` - Validation for write operations

### Removed Files
- `src/catalog/` - Old filesystem-based catalog (removed)
- `src/catalog_import.rs` - Import binary (removed)
- `src/catalog_store/legacy_adapter.rs` - LegacyCatalogAdapter (removed)
