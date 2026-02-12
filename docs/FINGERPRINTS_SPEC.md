# Fingerprints Feature Specification

**Status**: Draft
**Created**: 2025-12-10
**Last Updated**: 2025-12-10

---

## 1. Overview & Goals

### 1.1 What are Fingerprints?

Fingerprints are multi-dimensional representations of catalog items that capture various characteristics of the content. Each fingerprint is a collection of **feature maps**, where each feature map is a vector of float values representing a specific aspect of the item (e.g., audio characteristics, genre profile, cultural significance).

### 1.2 Goals

1. **Enable similarity search** - Find tracks/albums/artists similar to a given item
2. **Power recommendations** - Suggest content based on user preferences and listening history
3. **Support clustering** - Group content by characteristics for browsing/discovery
4. **Future-proof extensibility** - Easy to add new feature types as analysis capabilities evolve

### 1.3 Non-Goals (Initial Version)

- Real-time feature extraction during playback
- User-generated feature maps (ratings, tags converted to vectors)
- Cross-user collaborative filtering (this uses fingerprints but is a separate system)

---

## 2. Terminology

| Term | Definition |
|------|------------|
| **Fingerprint** | The complete collection of all feature maps for a single entity |
| **Feature Map** | A single vector of float values representing one aspect of an entity |
| **Feature Type** | A registered category of feature maps (e.g., "audio_spectrum", "genre") |
| **Entity** | A catalog item that can have a fingerprint (Track, Album, Artist, Playlist) |
| **Dimension** | The length of a feature map vector (e.g., 128, 256, 512 floats) |
| **Aggregation** | The process of computing derived fingerprints from source fingerprints |

---

## 3. Data Model

### 3.1 Entities That Have Fingerprints

| Entity | Fingerprint Source | Notes |
|--------|-------------------|-------|
| **Track** | Directly computed | Primary source - features extracted from audio/metadata |
| **Album** | Aggregated from tracks | Derived from all tracks in the album |
| **Artist** | Aggregated from tracks | Derived from all tracks by the artist |
| **User Playlist** | Aggregated from tracks | Derived from all tracks in the playlist |

### 3.2 Feature Map Type

Defines metadata about a category of feature maps:

```rust
struct FeatureMapType {
    /// Unique identifier (e.g., "audio_spectrum", "genre_profile")
    id: String,

    /// Human-readable name
    name: String,

    /// Description of what this feature represents
    description: String,

    /// Vector dimension (number of float values)
    dimension: u32,

    /// Current algorithm version
    current_version: u32,

    /// Which entity types this feature applies to
    /// If empty, applies to all entity types
    applies_to: Vec<EntityType>,

    /// How to aggregate track features for albums/artists/playlists
    aggregation_method: AggregationMethod,

    /// Whether this feature type is active
    is_active: bool,

    /// When this type was registered
    created_at: Timestamp,
}

enum EntityType {
    Track,
    Album,
    Artist,
    Playlist,
}

enum AggregationMethod {
    /// Simple arithmetic mean of all vectors
    Average,

    /// Weighted by track duration
    WeightedByDuration,

    /// Weighted by track popularity/play count
    WeightedByPopularity,

    /// Find the centroid that minimizes distance to all tracks
    Centroid,

    /// Element-wise maximum
    Max,

    /// Element-wise minimum
    Min,

    /// No aggregation - only applies to tracks
    None,
}
```

### 3.3 Feature Map

A single feature vector for an entity:

```rust
struct FeatureMap {
    /// The feature type this belongs to
    feature_type_id: String,

    /// Algorithm version used to compute this
    version: u32,

    /// The actual feature vector
    vector: Vec<f32>,

    /// When this was computed
    computed_at: Timestamp,

    /// Optional metadata about computation
    /// (e.g., confidence score, source info)
    metadata: Option<serde_json::Value>,
}
```

### 3.4 Fingerprint

The complete fingerprint for an entity:

```rust
struct Fingerprint {
    /// Entity type
    entity_type: EntityType,

    /// Entity ID (track_id, album_id, artist_id, or playlist_id)
    entity_id: String,

    /// All feature maps for this entity
    feature_maps: HashMap<String, FeatureMap>, // keyed by feature_type_id

    /// When the fingerprint was last updated
    last_updated: Timestamp,
}
```

---

## 4. Storage Schema

### 4.1 Database Choice

Fingerprints will be stored in the **catalog database** (`catalog.db`) for catalog items (Track, Album, Artist) and in the **user database** (`user.db`) for user playlists.

### 4.2 Schema Design

```sql
-- ============================================
-- Feature Map Type Registry
-- ============================================
CREATE TABLE feature_map_types (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    dimension INTEGER NOT NULL,
    current_version INTEGER NOT NULL DEFAULT 1,
    applies_to TEXT,  -- JSON array: ["track", "album", "artist", "playlist"] or null for all
    aggregation_method TEXT NOT NULL DEFAULT 'average',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================
-- Feature Maps Storage
-- ============================================
CREATE TABLE feature_maps (
    -- Composite primary key
    entity_type TEXT NOT NULL,      -- 'track', 'album', 'artist'
    entity_id TEXT NOT NULL,
    feature_type_id TEXT NOT NULL,

    -- Feature data
    version INTEGER NOT NULL,
    vector BLOB NOT NULL,           -- Binary: packed f32 array (little-endian)
    computed_at INTEGER NOT NULL,
    metadata TEXT,                  -- Optional JSON

    PRIMARY KEY (entity_type, entity_id, feature_type_id),
    FOREIGN KEY (feature_type_id) REFERENCES feature_map_types(id)
);

-- Index for querying all features of an entity
CREATE INDEX idx_feature_maps_entity
    ON feature_maps(entity_type, entity_id);

-- Index for querying all entities with a specific feature type
CREATE INDEX idx_feature_maps_type
    ON feature_maps(feature_type_id);

-- Index for finding outdated features (version < current)
CREATE INDEX idx_feature_maps_version
    ON feature_maps(feature_type_id, version);
```

### 4.3 User Database Addition (for Playlists)

```sql
-- In user.db
CREATE TABLE playlist_feature_maps (
    playlist_id TEXT NOT NULL,
    feature_type_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    vector BLOB NOT NULL,
    computed_at INTEGER NOT NULL,
    metadata TEXT,

    PRIMARY KEY (playlist_id, feature_type_id),
    FOREIGN KEY (playlist_id) REFERENCES user_playlist(id) ON DELETE CASCADE
);
```

### 4.4 Vector Storage Format

Feature vectors are stored as binary blobs for efficiency:

```rust
// Encoding
fn encode_vector(v: &[f32]) -> Vec<u8> {
    v.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

// Decoding
fn decode_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}
```

---

## 5. Computation Pipeline

### 5.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     External Feature Extractors                      │
│  (Audio analyzer, ML models, metadata enrichment services, etc.)    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ HTTP POST feature data
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Catalog Server                                  │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              Feature Ingestion API                           │   │
│  │   POST /v1/admin/fingerprints/ingest                        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                    │                                 │
│                                    ▼                                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              Feature Map Storage                             │   │
│  │   SQLite tables: feature_map_types, feature_maps            │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                    │                                 │
│                                    ▼                                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │           Aggregation Background Job                         │   │
│  │   Computes Album/Artist fingerprints from Track data        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                    │                                 │
│                                    ▼                                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │            Similarity Index (In-Memory)                      │   │
│  │   Built at startup, updated incrementally                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Feature Ingestion Flow

1. External service analyzes content (audio file, metadata, etc.)
2. Service calls ingestion API with entity ID and feature vector
3. Server validates feature type exists and vector dimension matches
4. Server stores/updates feature map in database
5. Server marks affected aggregates as stale (albums/artists containing this track)

### 5.3 Aggregation Flow

Background job runs periodically (or on-demand):

1. Find entities with stale aggregate fingerprints
2. For each entity:
   a. Fetch all track feature maps
   b. Apply aggregation method per feature type
   c. Store computed aggregate feature maps
3. Update similarity index

### 5.4 Similarity Index

Built at startup, similar to the search index:

```rust
struct SimilarityIndex {
    /// Per feature type, per entity type
    /// Maps entity_id -> normalized vector
    indices: HashMap<(FeatureTypeId, EntityType), VectorIndex>,
}

trait VectorIndex {
    /// Find k nearest neighbors to the query vector
    fn search(&self, query: &[f32], k: usize) -> Vec<(EntityId, f32)>;

    /// Add or update a vector
    fn upsert(&mut self, id: &str, vector: &[f32]);

    /// Remove a vector
    fn remove(&mut self, id: &str);
}
```

**Implementation options:**
- Simple brute-force (fine for < 100k items)
- HNSW (Hierarchical Navigable Small World) for larger catalogs
- External library: `instant-distance`, `hnsw`, or FFI to `faiss`

---

## 6. API Specification

### 6.1 Admin APIs - Feature Type Management

#### Register Feature Type
```http
POST /v1/admin/fingerprints/types
Content-Type: application/json

{
    "id": "audio_spectrum",
    "name": "Audio Spectrum Analysis",
    "description": "128-dimensional representation of audio frequency characteristics",
    "dimension": 128,
    "applies_to": ["track"],
    "aggregation_method": "weighted_by_duration"
}
```

#### List Feature Types
```http
GET /v1/admin/fingerprints/types

Response:
{
    "types": [
        {
            "id": "audio_spectrum",
            "name": "Audio Spectrum Analysis",
            "dimension": 128,
            "current_version": 1,
            "applies_to": ["track"],
            "aggregation_method": "weighted_by_duration",
            "is_active": true,
            "stats": {
                "track_count": 15000,
                "album_count": 1200,
                "artist_count": 400,
                "coverage_percent": 95.5
            }
        }
    ]
}
```

#### Update Feature Type
```http
PUT /v1/admin/fingerprints/types/{type_id}
Content-Type: application/json

{
    "name": "Updated Name",
    "description": "Updated description",
    "aggregation_method": "average",
    "is_active": true
}
```

#### Bump Feature Type Version
```http
POST /v1/admin/fingerprints/types/{type_id}/bump-version

Response:
{
    "type_id": "audio_spectrum",
    "previous_version": 1,
    "new_version": 2,
    "outdated_count": 15000
}
```

### 6.2 Admin APIs - Feature Data Management

#### Ingest Feature Maps (Batch)
```http
POST /v1/admin/fingerprints/ingest
Content-Type: application/json

{
    "feature_type_id": "audio_spectrum",
    "version": 1,
    "features": [
        {
            "entity_type": "track",
            "entity_id": "track_abc123",
            "vector": [0.1, 0.2, 0.3, ...],  // 128 floats
            "metadata": {"confidence": 0.95}
        },
        {
            "entity_type": "track",
            "entity_id": "track_def456",
            "vector": [0.4, 0.5, 0.6, ...]
        }
    ]
}

Response:
{
    "ingested": 2,
    "errors": []
}
```

#### Trigger Aggregation
```http
POST /v1/admin/fingerprints/aggregate
Content-Type: application/json

{
    "feature_type_id": "audio_spectrum",  // optional, null = all types
    "entity_types": ["album", "artist"],  // which aggregates to compute
    "force": false                        // recompute even if not stale
}

Response:
{
    "job_id": "agg_12345",
    "status": "queued",
    "estimated_entities": 1600
}
```

#### Get Aggregation Job Status
```http
GET /v1/admin/fingerprints/aggregate/{job_id}

Response:
{
    "job_id": "agg_12345",
    "status": "running",
    "progress": {
        "processed": 800,
        "total": 1600,
        "percent": 50.0
    },
    "started_at": 1702200000,
    "errors": []
}
```

#### Get Statistics
```http
GET /v1/admin/fingerprints/stats

Response:
{
    "feature_types": {
        "audio_spectrum": {
            "track": {"total": 20000, "with_feature": 15000, "outdated": 500},
            "album": {"total": 2000, "with_feature": 1200, "outdated": 100},
            "artist": {"total": 500, "with_feature": 400, "outdated": 50}
        },
        "genre_profile": {
            "track": {"total": 20000, "with_feature": 18000, "outdated": 0},
            ...
        }
    },
    "storage_bytes": 125000000,
    "index_status": "ready",
    "last_aggregation": 1702195000
}
```

#### Delete Feature Maps
```http
DELETE /v1/admin/fingerprints/features
Content-Type: application/json

{
    "feature_type_id": "deprecated_feature",  // delete all of this type
    // OR
    "entities": [
        {"entity_type": "track", "entity_id": "track_abc123"}
    ]
}
```

### 6.3 Content APIs - Query

#### Get Fingerprint for Entity
```http
GET /v1/content/fingerprint/{entity_type}/{entity_id}
GET /v1/content/fingerprint/{entity_type}/{entity_id}?types=audio_spectrum,genre_profile

Response:
{
    "entity_type": "track",
    "entity_id": "track_abc123",
    "fingerprint": {
        "audio_spectrum": {
            "version": 1,
            "vector": [0.1, 0.2, 0.3, ...],
            "computed_at": 1702200000
        },
        "genre_profile": {
            "version": 2,
            "vector": [0.8, 0.1, 0.05, ...],
            "computed_at": 1702180000
        }
    }
}
```

#### Find Similar Entities
```http
GET /v1/content/similar/{entity_type}/{entity_id}
    ?feature_type=audio_spectrum
    &limit=20
    &min_score=0.7

Response:
{
    "query": {
        "entity_type": "track",
        "entity_id": "track_abc123",
        "feature_type": "audio_spectrum"
    },
    "results": [
        {"entity_id": "track_xyz789", "score": 0.95},
        {"entity_id": "track_def456", "score": 0.89},
        ...
    ]
}
```

#### Find Similar by Vector (Advanced)
```http
POST /v1/content/similar
Content-Type: application/json

{
    "feature_type": "audio_spectrum",
    "target_entity_type": "track",
    "query": {
        // Option 1: By entity
        "entity_type": "track",
        "entity_id": "track_abc123"

        // Option 2: By raw vector
        // "vector": [0.1, 0.2, 0.3, ...]

        // Option 3: By multiple entities (centroid)
        // "entities": [
        //     {"entity_type": "track", "entity_id": "track_1"},
        //     {"entity_type": "track", "entity_id": "track_2"}
        // ]
    },
    "limit": 20,
    "min_score": 0.5,
    "exclude_ids": ["track_abc123"]  // exclude query entity
}
```

#### Batch Similarity (for recommendations)
```http
POST /v1/content/similar/batch
Content-Type: application/json

{
    "feature_type": "audio_spectrum",
    "queries": [
        {"entity_type": "track", "entity_id": "track_1"},
        {"entity_type": "track", "entity_id": "track_2"},
        {"entity_type": "track", "entity_id": "track_3"}
    ],
    "target_entity_type": "track",
    "limit_per_query": 5,
    "deduplicate": true
}
```

---

## 7. Similarity Search

### 7.1 Distance Metrics

```rust
enum DistanceMetric {
    /// Cosine similarity: dot(a, b) / (||a|| * ||b||)
    /// Best for: normalized vectors, semantic similarity
    /// Range: -1 to 1 (1 = identical)
    Cosine,

    /// Euclidean distance: sqrt(sum((a[i] - b[i])^2))
    /// Best for: absolute differences matter
    /// Range: 0 to infinity (0 = identical)
    Euclidean,

    /// Dot product: sum(a[i] * b[i])
    /// Best for: pre-normalized vectors
    DotProduct,
}
```

**Default**: Cosine similarity (most common for feature vectors)

### 7.2 Multi-Feature Similarity

When using multiple feature types for similarity:

```rust
struct MultiFeatureQuery {
    features: Vec<(FeatureTypeId, f32)>,  // (type, weight)
}

// Example: 70% audio similarity, 30% genre similarity
let query = MultiFeatureQuery {
    features: vec![
        ("audio_spectrum".into(), 0.7),
        ("genre_profile".into(), 0.3),
    ],
};
```

Combined score: `sum(weight[i] * similarity[i]) / sum(weight[i])`

### 7.3 Index Implementation Options

| Option | Pros | Cons | Best For |
|--------|------|------|----------|
| Brute Force | Simple, exact results | O(n) per query | < 50k items |
| KD-Tree | Fast for low dimensions | Poor for d > 20 | Low-dim features |
| HNSW | Fast, good accuracy | More memory | 50k - 10M items |
| IVF (Inverted File) | Scalable | Approximate | > 1M items |
| External (Faiss/Milvus) | Very scalable | Operational complexity | > 10M items |

**Recommendation**: Start with brute force, add HNSW when needed.

---

## 8. Client Integration

### 8.1 Web Frontend

**New Store**: `fingerprints.js`
```javascript
// Pinia store for fingerprint-related state
export const useFingerprintStore = defineStore('fingerprints', {
    state: () => ({
        similarCache: new Map(),  // entity_id -> similar items
    }),

    actions: {
        async getSimilar(entityType, entityId, featureType, limit = 20) {
            // Fetch and cache similar items
        },
    },
});
```

**UI Components**:
- `SimilarItems.vue` - Display similar tracks/albums/artists
- Add "Similar" section to Track, Album, Artist detail views
- Optionally: fingerprint visualization (radar chart, etc.)

### 8.2 Android App

**New Domain Layer**:
```kotlin
// domain/src/main/java/fingerprints/
interface FingerprintRepository {
    suspend fun getSimilar(
        entityType: EntityType,
        entityId: String,
        featureType: String,
        limit: Int = 20
    ): List<SimilarItem>
}

data class SimilarItem(
    val entityId: String,
    val entityType: EntityType,
    val score: Float
)
```

**UI Integration**:
- Add "Similar Tracks" to track details
- Add "Similar Artists" to artist details
- Use fingerprints for "Radio" feature (auto-playlist from seed)

---

## 9. Open Questions

### 9.1 Storage & Architecture

> **Q1: Should fingerprints be stored in catalog.db or a separate database?**
>
> Options:
> - A) catalog.db (consistent, simpler)
> - B) fingerprints.db (isolation, independent scaling)
>
> Current proposal: A) catalog.db

> **Q2: Should we use an external vector database?**
>
> Options:
> - A) SQLite + in-memory index (simpler, sufficient for <1M items)
> - B) External vector DB like Milvus/Qdrant (more scalable, operational overhead)
>
> Current proposal: A) Start simple

> **Q3: Should playlist fingerprints be real-time or cached?**
>
> Options:
> - A) Cached in user.db, updated on playlist change
> - B) Computed on-demand from track fingerprints
>
> Considerations: Playlist edits are infrequent, caching seems fine

### 9.2 Feature Extraction

> **Q4: Where does feature extraction happen?**
>
> Options:
> - A) External service (Python with librosa, ML models, etc.)
> - B) Built into pezzottify-server (Rust libraries)
> - C) Separate microservice
>
> Current proposal: A) External service, pezzottify-server just stores/indexes

> **Q5: How to handle missing fingerprints?**
>
> Options:
> - A) Graceful degradation (similarity search excludes items without fingerprints)
> - B) Queue for background extraction
> - C) Block content without fingerprints from similarity results
>
> Current proposal: A) Graceful degradation

> **Q6: Should fingerprint extraction be triggered by catalog changes?**
>
> When a new track is added:
> - A) Admin manually triggers extraction
> - B) Automatic webhook to extraction service
> - C) Extraction service polls for new content

### 9.3 API Design

> **Q7: Should fingerprints be included in standard content responses?**
>
> e.g., Should `GET /v1/content/track/{id}` include fingerprint data?
>
> Options:
> - A) Never (separate endpoint only)
> - B) Optional via query param `?include=fingerprint`
> - C) Always include (may be large)
>
> Current proposal: A) Separate endpoint

> **Q8: Should similarity search support cross-entity queries?**
>
> e.g., "Find tracks similar to this album's fingerprint"
>
> This is useful for "play something like this album" but adds complexity.

### 9.4 Data Model

> **Q9: Should images have fingerprints?**
>
> Could enable visual similarity (album art style, color palette).
>
> Options:
> - A) Yes, include from the start
> - B) No, defer to future version
> - C) Design schema to support it, implement later

> **Q10: How to handle feature type versioning?**
>
> When algorithm improves, need to recompute all features.
>
> Options:
> - A) Mark old features as outdated, recompute in background
> - B) Keep multiple versions, deprecate old over time
> - C) Hard cutover (risky)
>
> Current proposal: A) Mark outdated, background recompute

> **Q11: What aggregation methods do we actually need?**
>
> Current proposal includes: Average, WeightedByDuration, WeightedByPopularity, Centroid, Max, Min
>
> Are all of these needed? Are there others?

### 9.5 Performance

> **Q12: What's the expected scale?**
>
> - Number of tracks: ___
> - Number of feature types: ___
> - Vector dimensions: ___
> - Query latency requirements: ___

> **Q13: Should the similarity index be lazy-loaded?**
>
> Options:
> - A) Build full index at startup (current search behavior)
> - B) Lazy load on first query
> - C) Optional via config flag
>
> Startup time with large catalogs could be significant.

---

## 10. Implementation Phases

### Phase 1: Foundation
- [ ] Database schema (feature_map_types, feature_maps tables)
- [ ] Feature type registry CRUD
- [ ] Basic feature map storage/retrieval
- [ ] Admin API: ingest, get, delete
- [ ] Content API: get fingerprint for entity

### Phase 2: Similarity Search
- [ ] In-memory similarity index (brute force)
- [ ] Similarity query API
- [ ] Index persistence (optional, for faster startup)

### Phase 3: Aggregation
- [ ] Aggregation job infrastructure
- [ ] Track -> Album aggregation
- [ ] Track -> Artist aggregation
- [ ] Stale detection and recomputation

### Phase 4: Client Integration
- [ ] Web: Similar items component
- [ ] Web: Fingerprint admin panel
- [ ] Android: Similar items in detail views

### Phase 5: Advanced Features
- [ ] Multi-feature similarity queries
- [ ] Playlist fingerprints
- [ ] HNSW index for scale
- [ ] Fingerprint visualization

---

## 11. Example Feature Types (Future)

These are examples of feature types we might implement. Each would have its own extraction method:

| Feature Type | Dimension | Source | Notes |
|--------------|-----------|--------|-------|
| `audio_spectrum` | 128 | Audio analysis | MFCC, chroma, spectral features |
| `audio_tempo` | 8 | Audio analysis | BPM, rhythm patterns |
| `audio_energy` | 16 | Audio analysis | Loudness, dynamics |
| `genre_profile` | 64 | ML model / metadata | Genre distribution |
| `mood_profile` | 32 | ML model | Happy, sad, energetic, etc. |
| `era_profile` | 16 | Metadata | Musical era/decade |
| `cultural_context` | 32 | ML model / manual | Regional, political, etc. |
| `lyrical_themes` | 64 | NLP on lyrics | Topic modeling |
| `vocal_style` | 32 | Audio analysis | Male/female, range, style |
| `instrumentation` | 48 | Audio analysis | Instruments present |

---

## 12. References

- [Spotify Audio Features](https://developer.spotify.com/documentation/web-api/reference/get-audio-features)
- [Librosa Audio Analysis](https://librosa.org/)
- [HNSW Paper](https://arxiv.org/abs/1603.09320)
- [Faiss Library](https://github.com/facebookresearch/faiss)

---

## Appendix A: Example Workflows

### A.1 Adding a New Track with Fingerprint

```
1. Admin adds track to catalog
   POST /v1/admin/track { ... }

2. External analyzer processes audio file
   - Reads audio from media path
   - Computes features (MFCC, etc.)
   - Calls ingestion API

3. Ingestion stores feature
   POST /v1/admin/fingerprints/ingest
   {
       "feature_type_id": "audio_spectrum",
       "version": 1,
       "features": [{"entity_type": "track", "entity_id": "new_track_id", "vector": [...]}]
   }

4. Aggregation job detects affected album/artist
   - Marks album fingerprint as stale
   - Marks artist fingerprint as stale

5. Background job recomputes aggregates
   - Fetches all track features for album
   - Computes weighted average
   - Stores album fingerprint

6. Similarity index updated
   - New track added to index
   - Album fingerprint updated in index
```

### A.2 User Searching for Similar Tracks

```
1. User on track detail page clicks "Find Similar"

2. Frontend calls API
   GET /v1/content/similar/track/track_abc123?feature_type=audio_spectrum&limit=10

3. Server looks up track fingerprint
   SELECT vector FROM feature_maps
   WHERE entity_type='track' AND entity_id='track_abc123' AND feature_type_id='audio_spectrum'

4. Server queries similarity index
   index.search(vector, k=10)

5. Server returns ranked results
   {
       "results": [
           {"entity_id": "track_xyz", "score": 0.92},
           ...
       ]
   }

6. Frontend fetches track details for results
   GET /v1/content/track/track_xyz (or batch endpoint)

7. UI displays similar tracks
```
