# SQLite Catalog Migration Progress

This file tracks the progress of migrating the catalog from filesystem to SQLite.
Based on the design doc: `CATALOG_SQLITE_DESIGN.md`

## Completed Tasks

### Phase 1: Foundation
- [x] **1.1** Define SQLite schema with migrations (`src/catalog_store/schema.rs`)
- [x] **1.2** Implement new Rust model structs (`src/catalog_store/models.rs`)
- [x] **1.3** Implement SqliteCatalogStore with read operations (`src/catalog_store/store.rs`)

### Phase 2: Import Tool
- [x] **2** Create catalog-import binary (`src/catalog_import.rs`)

### Phase 3: Server Integration
- [x] **3.8** Replace GuardedCatalog with Arc<dyn CatalogStore> (`src/server/state.rs`)
- [x] **3.9** Update server handlers to use new store (`src/server/server.rs`, `src/server/search.rs`, etc.)
- [x] **3.10** Add --catalog-db CLI argument to switch between legacy and SQLite catalog stores
  - Added `--catalog-db` flag to main.rs for specifying SQLite catalog database path
  - When provided, server uses SqliteCatalogStore with new model shapes (breaking API change)
  - When not provided, falls back to LegacyCatalogAdapter for backward compatibility

### Phase 4: Write Operations
- [x] **4.11** Implement write operations in SqliteCatalogStore (insert_*, add_* methods)

## Pending Tasks

### Phase 4 (continued)
- [ ] **4.12** Add catalog editing API endpoints
- [ ] **4.13** Add validation for write operations

### Phase 5: Search
- [ ] **5.14** Evaluate search approach (PezzotHash vs FTS5)
- [ ] **5.15** Implement chosen search solution
- [ ] **5.16** Update search endpoints

### Phase 6: Cleanup
- [ ] **6.17** Remove filesystem catalog loading code
- [ ] **6.18** Remove old model definitions
- [ ] **6.19** Update tests
- [ ] **6.20** Update documentation

## Key Files Created/Modified

### New Files
- `src/catalog_store/mod.rs` - Module root
- `src/catalog_store/schema.rs` - SQLite schema definitions
- `src/catalog_store/models.rs` - New model structs
- `src/catalog_store/store.rs` - SqliteCatalogStore implementation
- `src/catalog_store/trait_def.rs` - CatalogStore trait
- `src/catalog_store/legacy_adapter.rs` - LegacyCatalogAdapter for backward compatibility
- `src/catalog_import.rs` - Import binary

### Modified Files
- `src/main.rs` - Uses LegacyCatalogAdapter
- `src/server/state.rs` - Changed GuardedCatalog to GuardedCatalogStore
- `src/server/server.rs` - Updated handlers to use CatalogStore
- `src/server/stream_track.rs` - Updated to use CatalogStore
- `src/server/search.rs` - Updated resolve functions for JSON-based queries
- `src/search/search_vault.rs` - Updated PezzotHashSearchVault to use CatalogStore
- `src/user/user_manager.rs` - Changed to accept Arc<dyn CatalogStore>
- `src/cli_auth.rs` - Updated to use LegacyCatalogAdapter
- `tests/common/server.rs` - Updated e2e tests to use LegacyCatalogAdapter
- `Cargo.toml` - Added catalog-import binary

## Git Commits (in order)
1. `[catalog-server] Add SQLite schema for catalog database`
2. `[catalog-server] Add new catalog model structs for SQLite storage`
3. `[catalog-server] Implement SqliteCatalogStore with read operations`
4. `[catalog-server] Add catalog-import binary and write operations`
5. `[catalog-server] Add CatalogStore trait and implementations`
6. `[catalog-server] Update server handlers to use CatalogStore trait`
7. `[catalog-server] Add --catalog-db CLI flag to enable SQLite catalog backend`

## Notes
- The server now supports both catalog backends via CLI flags:
  - Default (no --catalog-db): Uses `LegacyCatalogAdapter` with filesystem-based catalog
  - With `--catalog-db <path>`: Uses `SqliteCatalogStore` with new model shapes
- To use SQLite catalog:
  1. First import filesystem catalog: `cargo run --bin catalog-import -- <catalog-path> <output-db-path>`
  2. Run server with: `cargo run -- <catalog-path> <user-db-path> --catalog-db <catalog-db-path>`
- **API Breaking Change**: SqliteCatalogStore returns different JSON response shapes (e.g., `genres` instead of `genre`, nested structures for resolved entities)
- All tests pass with the current implementation
