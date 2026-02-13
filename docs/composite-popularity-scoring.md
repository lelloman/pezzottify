# Composite Popularity Scoring

**Status**: ✅ **IMPLEMENTED**
**Implemented**: 2025-12-xx (exact date TBD - see git history)
**Last Updated**: 2026-02-13 (marked as implemented)

## Overview

This document describes the **implemented** composite popularity scoring system that combines three data sources:

1. **Listening data** (70% weight) - Based on actual play counts from Pezzottify users
2. **Impression data** (25% weight) - Based on page views (artist/album/track screens)
3. **Spotify popularity** (5% weight) - Static fallback from imported Spotify metadata

The goal is to prioritize content that Pezzottify users actually engage with, rather than relying solely on Spotify's global popularity metrics.

## Implementation Status ✅

**This feature has been fully implemented and is in production use.**

### What's Implemented:
- ✅ `item_impressions` table for tracking page views
- ✅ Extended `item_popularity` table with `listening_score`, `impression_score`, `spotify_score` columns
- ✅ `POST /v1/user/impression` endpoint for recording page views
- ✅ Composite score calculation with weight redistribution (70/25/5 weights)
- ✅ Impression aggregation in `PopularContentJob`
- ✅ Impression pruning based on retention period

### What's NOT Implemented (from original plan):
- ❌ Web client impression tracking (clients don't call the endpoint yet)
- ❌ Android client impression tracking
- ⚠️  Consider adding time decay to impressions (future consideration)
- ⚠️  Consider user deduplication per day (future consideration)

## Current State (as of implementation)

### Listening Data
- Stored in `listening_events` table (user database)
- Aggregated by `PopularContentJob` every 6 hours
- Last 30 days lookback window (configurable)
- Normalized to 0.0-1.0 range for composite scoring

### Impression Data
- Stored in `item_impressions` table (search database)
- Tracked via `POST /v1/user/impression` endpoint
- 365-day lookback window for aggregation
- 365-day retention period for pruning

### Spotify Popularity
- Static `popularity` field (0-100) on `artists`, `albums`, `tracks` tables
- Normalized to 0.0-1.0 range for composite scoring
- Used as fallback when listening/impression data unavailable

### Search Ranking
- Uses composite score with weight redistribution:
  - All sources available: 70% listening / 25% impression / 5% Spotify
  - Missing sources trigger weight redistribution to remaining sources
- Applied via `POPULARITY_WEIGHT = 0.5` factor in search ranking

## Proposed Changes (original design - now implemented)

### 1. New Table: `item_impressions`

Location: Search database (alongside `item_popularity`)

```sql
CREATE TABLE IF NOT EXISTS item_impressions (
    item_id TEXT NOT NULL,
    item_type TEXT NOT NULL,  -- "track", "album", "artist"
    date INTEGER NOT NULL,     -- YYYYMMDD format
    impression_count INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (item_id, item_type, date)
);

CREATE INDEX IF NOT EXISTS idx_impressions_date ON item_impressions(date);
CREATE INDEX IF NOT EXISTS idx_impressions_item ON item_impressions(item_id, item_type);
```

Data model: One row per item per day. When a user views a page:
- If row exists for (item_id, item_type, today): increment `impression_count`
- Otherwise: insert new row with `impression_count = 1`

### 2. Extended `item_popularity` Table

Add columns to track component scores:

```sql
ALTER TABLE item_popularity ADD COLUMN listening_score REAL NOT NULL DEFAULT 0.0;
ALTER TABLE item_popularity ADD COLUMN impression_score REAL NOT NULL DEFAULT 0.0;
ALTER TABLE item_popularity ADD COLUMN spotify_score REAL NOT NULL DEFAULT 0.0;
```

The existing `score` column becomes the composite score used by search ranking.

### 3. New API Endpoint

**`POST /v1/user/impression`**

Records a content view/impression.

Request body:
```json
{
  "item_id": "spotify_id_here",
  "item_type": "track" | "album" | "artist"
}
```

Response: `200 OK` (fire-and-forget, no response body needed)

Authentication: Requires `AccessCatalog` permission (same as other user endpoints).

Implementation notes:
- Use `INSERT OR REPLACE` with increment pattern for efficiency
- No rate limiting needed (one impression per page view is fine)
- Could batch client-side if needed, but single calls are fine initially

### 4. SearchVault Changes

Add method to `SearchVault` trait:

```rust
/// Record an impression for an item (page view).
/// Increments today's impression count for the given item.
fn record_impression(&self, item_id: &str, item_type: HashedItemType);
```

Implementation in `Fts5LevenshteinSearchVault`:
- Use write connection
- `INSERT INTO item_impressions (item_id, item_type, date, impression_count)
   VALUES (?, ?, ?, 1)
   ON CONFLICT(item_id, item_type, date)
   DO UPDATE SET impression_count = impression_count + 1`

### 5. CatalogStore Changes

Add method to query Spotify popularity:

```rust
/// Get Spotify popularity scores for multiple items.
/// Returns a map of (item_id, item_type) -> popularity (0-100).
fn get_items_popularity(&self, items: &[(String, HashedItemType)]) -> HashMap<(String, HashedItemType), i32>;
```

This allows batch lookup of Spotify popularity during job execution.

### 6. PopularContentJob Changes

#### Configuration

Extend `PopularContentJob` config:

```rust
pub struct PopularContentJob {
    albums_limit: usize,           // existing
    artists_limit: usize,          // existing
    lookback_days: u32,            // existing (for listening data)
    impression_lookback_days: u32, // NEW: default 365
    impression_retention_days: u32, // NEW: default 365
}
```

#### Execution Flow

1. **Reset Phase**
   ```sql
   UPDATE item_popularity SET listening_score = 0.0, impression_score = 0.0;
   ```
   Note: `spotify_score` is not reset (computed fresh each run anyway).

2. **Listening Data Aggregation** (existing logic, modified)
   - Query `listening_events` for last `lookback_days` (default: 30)
   - Aggregate play counts by track → album → artist
   - Normalize within each type (0.0-1.0)
   - Update `listening_score` column

3. **Impression Data Aggregation** (NEW)
   - Query `item_impressions` for last `impression_lookback_days` (default: 365)
   - Aggregate impression counts by item
   - Normalize within each type (0.0-1.0)
   - Update `impression_score` column

4. **Spotify Popularity Lookup** (NEW)
   - For all items in `item_popularity`, batch query catalog for `popularity`
   - Normalize 0-100 → 0.0-1.0
   - Update `spotify_score` column

5. **Composite Score Calculation** (NEW)
   - For each item, compute weighted score with redistribution:
   ```
   weights = (listening: 0.70, impression: 0.25, spotify: 0.05)

   if listening_score > 0:
       composite = 0.70 * listening + 0.25 * impression + 0.05 * spotify
   elif impression_score > 0:
       # Redistribute listening weight to impression and spotify
       composite = (0.25 / 0.30) * impression + (0.05 / 0.30) * spotify
       # = 0.833 * impression + 0.167 * spotify
   else:
       # Only spotify available
       composite = spotify_score
   ```
   - Update `score` column with composite value

6. **Prune Old Impressions** (NEW)
   ```sql
   DELETE FROM item_impressions WHERE date < ?;
   ```
   Where threshold = today - `impression_retention_days`

#### Optimization Notes

- Batch all updates in transactions
- Use prepared statements for repeated queries
- Consider running impression aggregation in parallel with listening aggregation
- The reset + recompute approach ensures consistency and handles items that fall off

### 7. Client Changes (Web & Android)

Clients should call `POST /v1/user/impression` when:
- User navigates to an artist page
- User navigates to an album page
- User navigates to a track detail page (if exists)

Fire-and-forget pattern - don't block UI on response.

## Weight Redistribution Logic

The 70/25/5 weights apply when all data sources have values. When sources are missing:

| Listening | Impression | Spotify | Effective Weights |
|-----------|------------|---------|-------------------|
| Yes       | Yes        | Yes     | 70% / 25% / 5%    |
| Yes       | Yes        | No      | 70% / 25% / 0% (renormalize to 73.7% / 26.3%) |
| Yes       | No         | Yes     | 70% / 0% / 5% (renormalize to 93.3% / 6.7%) |
| No        | Yes        | Yes     | 0% / 83.3% / 16.7% |
| No        | Yes        | No      | 0% / 100% / 0%    |
| No        | No         | Yes     | 0% / 0% / 100%    |
| No        | No         | No      | 0 (no boost)      |

Note: Spotify popularity should exist for all imported items, so the "No Spotify" cases are edge cases.

## Migration

### Database Migration

1. Add new columns to `item_popularity`:
   ```sql
   ALTER TABLE item_popularity ADD COLUMN listening_score REAL NOT NULL DEFAULT 0.0;
   ALTER TABLE item_popularity ADD COLUMN impression_score REAL NOT NULL DEFAULT 0.0;
   ALTER TABLE item_popularity ADD COLUMN spotify_score REAL NOT NULL DEFAULT 0.0;
   ```

2. Create `item_impressions` table (see schema above)

3. Backfill `spotify_score` for existing items (one-time):
   - Query all items in `item_popularity`
   - Look up Spotify popularity from catalog
   - Update `spotify_score`

4. Copy existing `score` to `listening_score` (existing scores are listening-based):
   ```sql
   UPDATE item_popularity SET listening_score = score;
   ```

### Rollout

1. Deploy database migration
2. Deploy server with new endpoint and job changes
3. Deploy web client with impression tracking
4. Deploy Android client with impression tracking
5. Wait for first job run to populate composite scores

## Files to Modify

### pezzottify-server

1. **`src/search/fts5_levenshtein_search.rs`**
   - Add `item_impressions` table creation in `create_tables()`
   - Add `record_impression()` method
   - Modify schema for `item_popularity` (add columns)

2. **`src/search/search_vault.rs`**
   - Add `record_impression()` to `SearchVault` trait
   - Add `get_impression_totals()` method for job aggregation

3. **`src/catalog_store/store.rs`** (or `mod.rs`)
   - Add `get_items_popularity()` method to `CatalogStore` trait

4. **`src/catalog_store/sqlite_catalog_store.rs`**
   - Implement `get_items_popularity()`

5. **`src/background_jobs/jobs/popular_content.rs`**
   - Add impression aggregation
   - Add Spotify lookup
   - Add composite score calculation
   - Add impression pruning
   - Add new config fields

6. **`src/server/user.rs`** (or new file `src/server/impression.rs`)
   - Add `POST /v1/user/impression` endpoint

7. **`src/server/server.rs`**
   - Wire up impression endpoint

### web

1. **`src/services/api.js`** (or similar)
   - Add `recordImpression(itemId, itemType)` API call

2. **`src/views/ArtistView.vue`** (or component)
   - Call `recordImpression` on mount

3. **`src/views/AlbumView.vue`** (or component)
   - Call `recordImpression` on mount

4. **`src/views/TrackView.vue`** (if exists)
   - Call `recordImpression` on mount

### android

Similar changes to track page views and call the impression endpoint.

## Testing

1. **Unit tests** for composite score calculation with various input combinations
2. **Integration test** for impression recording endpoint
3. **Job test** verifying correct aggregation and score computation
4. **Manual testing** of search ranking changes

## Future Considerations

- **Decay function**: Could apply time decay to impressions (recent views worth more)
- **User deduplication**: Currently counts all views; could dedupe per user per day
- **Batch impression endpoint**: If client-side batching is needed for performance
- **Analytics dashboard**: Surface impression data in admin panel
