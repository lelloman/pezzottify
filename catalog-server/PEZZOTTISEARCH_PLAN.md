# Pezzottisearch - Search Engine Design

## Implementation Status

| Stage | Status | Location | Notes |
|-------|--------|----------|-------|
| **Stage 1**: SimHash | ✅ Done | `search/pezzott_hash.rs`, `search/search_vault.rs` | `PezzotHashSearchVault` |
| **Stage 1**: Popularity | ⚠️ Partial | `search/fts5_levenshtein_search.rs` | Only in FTS5+Levenshtein, not in PezzotHash |
| **Stage 2**: Trigram Boost | ⚠️ Partial | `search/search_vault.rs` | Trigram **similarity** exists, not **containment** boost |
| **Stage 3**: Levenshtein | ✅ Done | `search/fts5_levenshtein_search.rs` | Only in FTS5+Levenshtein engine |
| **Stage 4**: Target ID | ✅ Done | `search/streaming/target_identifier.rs` | `ScoreGapStrategy` implementation |
| **Stage 4**: Enrichment | ✅ Done | `search/streaming/pipeline.rs` | Popular tracks, albums, related artists |

### Current Architecture

The planned unified 4-stage pipeline was **not** implemented as a single engine. Instead:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Available Search Engines (choose ONE via config)                   │
│                                                                     │
│  ┌─────────────────────┐  ┌─────────────────────────────────────┐  │
│  │ PezzotHashSearchVault│  │ Fts5LevenshteinSearchVault          │  │
│  │                     │  │                                     │  │
│  │ - SimHash matching  │  │ - FTS5 trigram search               │  │
│  │ - Trigram re-sort   │  │ - Levenshtein typo correction       │  │
│  │ - NO popularity     │  │ - Popularity scoring ✅              │  │
│  └──────────┬──────────┘  └──────────────────┬──────────────────┘  │
│             │                                │                      │
│             └────────────────┬───────────────┘                      │
│                              ▼                                      │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Streaming Search Pipeline (Stage 4)                                │
│  Location: src/search/streaming/                                    │
│                                                                     │
│  1. Takes results from whichever engine is configured               │
│  2. Identifies targets per content type (artist, album, track)      │
│  3. Enriches with related content                                   │
│  4. Returns structured SSE response                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### Streaming Search Endpoint

`GET /v1/content/search/stream?q=<query>` (see `server/search.rs:333-376`)

1. Calls `search_vault.search()` using configured engine
2. Passes results to `StreamingSearchPipeline.execute()`
3. Returns SSE events with structured sections

### What's Missing for Full Pezzottisearch

To implement the original unified Stages 1-3 pipeline:

1. **Add popularity to `PezzotHashSearchVault`**
   - `update_popularity()` is currently a no-op (line 200 of `search_vault.rs`)
   - Need to store and apply popularity scores in search ranking

2. **Stage 2: Trigram Containment Boost** (not implemented)
   - Current `CharsTrigrams::similarity()` checks overlap ratio
   - Need: containment check for prefix matching ("ABC" → "ABCDEFG")
   - Should activate for short queries (≤10 chars)

3. **Add Levenshtein to SimHash pipeline**
   - Currently Levenshtein only exists in `Fts5LevenshteinSearchVault`
   - For unified pipeline: add as re-ranking step after SimHash + Trigram

---

## Original Plan

### Overview

A search engine combining multiple techniques for optimal typo tolerance, prefix matching, and structured results.

### Planned Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Stage 1: SimHash + Popularity (always, fast)           │
│                                                         │
│  - Brute force SimHash comparison (~2ms for 1M items)   │
│  - Popularity weighting applied here                    │
│  - Output: top ~500 candidates                          │
└─────────────────┬───────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 2: Trigram Containment Boost                     │
│                                                         │
│  - For short queries (≤ configurable X chars)           │
│  - Boost items that contain query trigrams              │
│  - Fixes prefix matching ("ABC" → "ABCDEFG")            │
│  - Output: top ~200 candidates                          │
└─────────────────┬───────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 3: Precise Re-rank (Levenshtein)                 │
│                                                         │
│  - Levenshtein distance on actual names                 │
│  - Normalize by length for fair comparison              │
│  - Output: top ~50 candidates                           │
└─────────────────┬───────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 4: Target Identification & Enrichment            │
│                                                         │
│  - Identify likely "target" (what user is looking for)  │
│  - Fetch related items (albums by artist, etc.)         │
│  - Structure the response                               │
└─────────────────────────────────────────────────────────┘
```

### Stage Details

#### Stage 1: SimHash + Popularity

Reuse existing SimHash implementation from `pezzott_hash.rs`:
- Compute SimHash of query
- Compare against all indexed items
- Score = hamming_distance × (1 + popularity × weight)
- Keep top 500

**Data structure:**
```rust
struct IndexedItem {
    id: String,
    item_type: HashedItemType,  // Artist, Album, Track
    name: String,
    hash: PezzottHash,
    popularity: f64,  // 0.0 - 1.0
}
```

#### Stage 2: Trigram Containment Boost

For queries ≤ X characters (configurable, default 10):
- Extract trigrams from query
- For each candidate, check what % of query trigrams are contained
- Boost score: `score += containment_ratio × boost_factor`

This fixes the prefix matching problem:
- Query "ABC" → trigrams ["ABC"]
- Item "ABCDEFG" → contains "ABC" → 100% containment → big boost

**Implementation options:**
1. Store trigrams per item (HashSet)
2. Build inverted index (trigram → item_ids) for faster lookup

#### Stage 3: Precise Re-rank (Levenshtein)

On the ~200 remaining candidates:
- Compute Levenshtein distance between query and item name
- Normalize: `normalized_distance = distance / max(query_len, name_len)`
- Re-sort by normalized distance

This handles cases where SimHash/trigrams aren't precise enough.

#### Stage 4: Target Identification & Enrichment ✅ IMPLEMENTED

**Location:** `src/search/streaming/`

**Target identification** (`target_identifier.rs`):
- `ScoreGapStrategy` looks at score gap between #1 and #2
- Configurable thresholds: `min_absolute_score`, `min_score_gap_ratio`, `exact_match_boost`
- Identifies best target per content type (artist, album, track)

**Enrichment** (`pipeline.rs`):
- Artist target → popular tracks by artist, albums, related artists
- Album target → tracks from album, related artists
- Track target → (no additional enrichment currently)

**Response structure** (`sections.rs`):
```rust
enum SearchSection {
    PrimaryArtist { item, confidence },
    PrimaryAlbum { item, confidence },
    PrimaryTrack { item, confidence },
    PopularBy { target_id, target_type, items },
    AlbumsBy { target_id, items },
    RelatedArtists { target_id, items },
    TracksFrom { target_id, items },
    MoreResults { items },
    Results { items },
    Done { total_time_ms },
}
```

### Configuration

Current streaming search config (`config/mod.rs`):
```rust
pub struct StreamingSearchSettings {
    pub min_absolute_score: f64,      // Target ID threshold
    pub min_score_gap_ratio: f64,     // Gap between #1 and #2
    pub exact_match_boost: f64,       // Boost for exact name matches
    pub popular_tracks_limit: usize,  // Enrichment limits
    pub albums_limit: usize,
    pub related_artists_limit: usize,
    pub top_results_limit: usize,
    pub other_results_limit: usize,
}
```

Planned (not yet implemented):
```toml
[search]
engine = "pezzottisearch"

[search.pezzottisearch]
# Stage 1
popularity_weight = 0.5

# Stage 2
short_query_threshold = 10  # chars
trigram_boost_factor = 2.0

# Stage 3
levenshtein_candidates = 200
```

### Existing Code Reference

| File | What It Does |
|------|--------------|
| `search/pezzott_hash.rs` | SimHash calculation (Stage 1 core) |
| `search/search_vault.rs` | `PezzotHashSearchVault` - SimHash + trigram similarity |
| `search/fts5_search.rs` | `Fts5SearchVault` - FTS5 trigram search |
| `search/fts5_levenshtein_search.rs` | `Fts5LevenshteinSearchVault` - FTS5 + Levenshtein + popularity |
| `search/levenshtein.rs` | Levenshtein distance + vocabulary |
| `search/streaming/pipeline.rs` | Stage 4: orchestrates target ID + enrichment |
| `search/streaming/target_identifier.rs` | Stage 4: `ScoreGapStrategy` implementation |
| `search/streaming/enrichment.rs` | Stage 4: helper functions for enrichment |
| `search/streaming/sections.rs` | Stage 4: response section types |
| `search/factory.rs` | Creates search vault based on config |
| `search/relevance_filter.rs` | Post-processing relevance filtering |
| `background_jobs/jobs/popular_content.rs` | Computes popularity, calls `update_popularity()` |

### Remaining Implementation Tasks

If building the unified Pezzottisearch engine:

1. [ ] Add popularity storage to `PezzotHashSearchVault`
2. [ ] Implement `update_popularity()` in `PezzotHashSearchVault`
3. [ ] Apply popularity weighting in SimHash search
4. [ ] Add trigram containment check (not just similarity)
5. [ ] Add short query detection for Stage 2 activation
6. [ ] Add Levenshtein re-ranking step to SimHash pipeline
7. [ ] Create unified `PezzottisearchVault` or extend `PezzotHashSearchVault`
8. [ ] Add `pezzottisearch` engine option to factory
9. [ ] Add TOML configuration options
10. [ ] Tests

### Open Questions

1. Should trigram index be in-memory or SQLite?
   → Start with in-memory, optimize later if needed

2. ~~How to define "confidence" for target identification?~~
   → **Resolved**: Score gap strategy implemented in `ScoreGapStrategy`

3. ~~Should enrichment (related items) be optional/configurable?~~
   → **Resolved**: Yes, configurable via `StreamingSearchSettings`

4. Should we build unified Pezzottisearch or keep separate engines?
   → Current approach: separate engines + streaming pipeline on top
   → Trade-off: less optimal but more flexible/maintainable
