# Pezzottisearch - New Search Engine Design

## Overview

A new search engine combining multiple techniques for optimal typo tolerance, prefix matching, and structured results.

## Architecture

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

## Stage Details

### Stage 1: SimHash + Popularity

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

### Stage 2: Trigram Containment Boost

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

### Stage 3: Precise Re-rank (Levenshtein)

On the ~200 remaining candidates:
- Compute Levenshtein distance between query and item name
- Normalize: `normalized_distance = distance / max(query_len, name_len)`
- Re-sort by normalized distance

This handles cases where SimHash/trigrams aren't precise enough.

### Stage 4: Target Identification & Enrichment

**Target identification:**
- Look at top result after re-ranking
- If confidence is high (score significantly better than #2), mark as "target"
- Target types: Artist, Album, Track

**Enrichment based on target type:**
- Artist target → include top N popular albums by this artist
- Album target → include tracks from this album
- Track target → include other tracks from same album

**Response structure:**
```rust
struct SearchResponse {
    target: Option<TargetResult>,
    other_matches: Vec<SearchResult>,
}

struct TargetResult {
    item: SearchResult,
    confidence: f64,
    related: Vec<SearchResult>,  // Albums by artist, tracks from album, etc.
}
```

## Configuration

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

# Stage 4
target_confidence_threshold = 0.8
related_items_count = 5
```

## Index Structure

```rust
pub struct PezzottisearchVault {
    /// All indexed items with their SimHashes
    items: Vec<IndexedItem>,

    /// Inverted index: trigram → item indices (for stage 2)
    trigram_index: HashMap<String, Vec<usize>>,

    /// Popularity scores (updated by background job)
    popularity: HashMap<(String, HashedItemType), f64>,

    /// Reference to catalog store (for enrichment in stage 4)
    catalog_store: Arc<dyn CatalogStore>,
}
```

## Files to Create

| File | Purpose |
|------|---------|
| `src/search/pezzottisearch.rs` | Main implementation |
| `src/search/trigram.rs` | Trigram extraction and indexing |

## Files to Modify

| File | Changes |
|------|---------|
| `src/search/mod.rs` | Export new module |
| `src/search/factory.rs` | Add `pezzottisearch` engine option |
| `src/config/mod.rs` | Add config options |
| `src/config/file_config.rs` | Add TOML parsing |

## Implementation Order

1. [ ] Create basic `PezzottisearchVault` struct with SimHash indexing
2. [ ] Implement Stage 1: SimHash search + popularity
3. [ ] Add trigram extraction and indexing
4. [ ] Implement Stage 2: Trigram boost for short queries
5. [ ] Implement Stage 3: Levenshtein re-ranking
6. [ ] Implement Stage 4: Target identification
7. [ ] Implement Stage 4: Related items enrichment
8. [ ] Add configuration options
9. [ ] Add to factory and CLI
10. [ ] Tests

## Open Questions

1. Should trigram index be in-memory or SQLite?
   → Start with in-memory, optimize later if needed

2. How to define "confidence" for target identification?
   → Score gap between #1 and #2? Absolute score threshold?

3. Should enrichment (related items) be optional/configurable?
   → Yes, some clients might want flat results only
