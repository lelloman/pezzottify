# Downloader Service Integration Plan

## Overview

Integrate an external HTTP "downloader" service with catalog-server. The catalog-server acts as a proxy: when serving content, it detects missing related data (e.g., artist with no albums) and fetches it from the downloader, blocking until complete.

## Configuration

- **Optional** at startup - server functions without downloader, proxy features disabled
- New CLI arg: `--downloader-url <URL>` (e.g., `http://localhost:8080`)
- Optional timeout: `--downloader-timeout-sec <SECONDS>` (default: 300)

## Architecture

```
┌─────────────┐     ┌─────────────────┐     ┌────────────────┐
│   Client    │────▶│  Catalog Server │────▶│   Downloader   │
└─────────────┘     └─────────────────┘     └────────────────┘
                           │                        │
                           ▼                        ▼
                    ┌─────────────┐          External Source
                    │  SQLite DB  │          (metadata + files)
                    │  + Media    │
                    └─────────────┘
```

## Implementation Steps

### Step 1: Add Dependencies

**File:** `catalog-server/Cargo.toml`

Move `reqwest` from dev-dependencies to dependencies with streaming support:
```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
```

### Step 2: Add CLI Configuration

**File:** `catalog-server/src/main.rs`

Add to `CliArgs`:
```rust
#[clap(long)]
pub downloader_url: Option<String>,

#[clap(long, default_value_t = 300)]
pub downloader_timeout_sec: u64,
```

### Step 3: Create Downloader Client Module

**New file:** `catalog-server/src/downloader/mod.rs`

```rust
pub mod client;
pub mod models;
```

**New file:** `catalog-server/src/downloader/models.rs`

Define response types matching downloader API:
- `DownloaderArtist`
- `DownloaderAlbum`
- `DownloaderTrack`
- `DownloaderImage`

**New file:** `catalog-server/src/downloader/client.rs`

```rust
pub struct DownloaderClient {
    client: reqwest::Client,
    base_url: String,
}

impl DownloaderClient {
    pub fn new(base_url: String, timeout_sec: u64) -> Self;

    // API methods matching downloader endpoints
    pub async fn health_check(&self) -> Result<()>;
    pub async fn get_artist(&self, id: &str) -> Result<DownloaderArtist>;
    pub async fn get_album(&self, id: &str) -> Result<DownloaderAlbum>;
    pub async fn get_album_tracks(&self, id: &str) -> Result<Vec<DownloaderTrack>>;
    pub async fn get_track(&self, id: &str) -> Result<DownloaderTrack>;
    pub async fn download_track_audio(&self, id: &str, dest: &Path) -> Result<()>;
    pub async fn download_image(&self, id: &str, dest: &Path) -> Result<()>;
}
```

### Step 4: Add to ServerState

**File:** `catalog-server/src/server/state.rs`

```rust
pub struct ServerState {
    // ... existing fields
    pub downloader: Option<Arc<DownloaderClient>>,
}
```

### Step 5: Create Proxy Logic Module

**New file:** `catalog-server/src/server/proxy.rs`

Encapsulates the "detect missing, fetch, store" logic:

```rust
pub struct CatalogProxy {
    downloader: Arc<DownloaderClient>,
    catalog_store: GuardedCatalogStore,
}

impl CatalogProxy {
    /// Check if artist needs enrichment, fetch if so
    pub async fn ensure_artist_complete(&self, id: &str) -> Result<()>;

    /// Fetch album + tracks + images from downloader, store in catalog
    pub async fn fetch_and_store_album(&self, id: &str) -> Result<()>;

    /// Fetch artist metadata + related artists from downloader
    pub async fn fetch_and_store_artist(&self, id: &str) -> Result<()>;

    // Internal helpers
    async fn store_image(&self, image: DownloaderImage) -> Result<()>;
    async fn store_track(&self, track: DownloaderTrack) -> Result<()>;
}
```

### Step 6: Integrate into Route Handlers

**File:** `catalog-server/src/server/server.rs`

Modify `get_artist()`, `get_album()`, etc. to use proxy when available:

```rust
async fn get_artist(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Response {
    // If downloader configured, check and fetch missing content
    if let Some(proxy) = &state.proxy {
        if let Err(e) = proxy.ensure_artist_complete(&id).await {
            warn!("Proxy fetch failed for artist {}: {}", id, e);
            // Continue serving what we have
        }
    }

    // Existing logic
    match catalog_store.get_artist_json(&id) { ... }
}
```

### Step 7: Add Metrics

**File:** `catalog-server/src/server/metrics.rs`

```rust
pub static ref DOWNLOADER_REQUESTS_TOTAL: CounterVec = ...
pub static ref DOWNLOADER_REQUEST_DURATION: HistogramVec = ...
pub static ref DOWNLOADER_ERRORS_TOTAL: CounterVec = ...
pub static ref DOWNLOADER_BYTES_DOWNLOADED: Counter = ...
```

### Step 8: Wire Up in Main

**File:** `catalog-server/src/main.rs`

```rust
let downloader = args.downloader_url.map(|url| {
    Arc::new(DownloaderClient::new(url, args.downloader_timeout_sec))
});

let proxy = downloader.as_ref().map(|d| {
    Arc::new(CatalogProxy::new(d.clone(), catalog_store.clone()))
});

let state = ServerState {
    // ... existing
    proxy,
};
```

## File Structure

```
catalog-server/src/
├── downloader/
│   ├── mod.rs
│   ├── client.rs      # HTTP client for downloader API
│   └── models.rs      # Response types
├── server/
│   ├── proxy.rs       # Detection + fetch + store logic (NEW)
│   └── server.rs      # Modified route handlers
└── main.rs            # CLI args + wiring
```

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Add reqwest to dependencies |
| `main.rs` | Add CLI args, wire downloader |
| `server/state.rs` | Add `proxy` field to ServerState |
| `server/server.rs` | Integrate proxy in route handlers |
| `server/metrics.rs` | Add downloader metrics |
| `lib.rs` | Export downloader module |

## New Files

| File | Purpose |
|------|---------|
| `downloader/mod.rs` | Module exports |
| `downloader/client.rs` | reqwest HTTP client |
| `downloader/models.rs` | Downloader API response types |
| `server/proxy.rs` | Proxy logic (detect, fetch, store) |

## Detection Logic (Initial - Tweakable)

For artist endpoint:
- Artist has 0 albums → fetch artist discography
- Artist has 0 related artists → fetch related artists

For album endpoint:
- Album has 0 tracks → fetch album tracks

This logic lives in `proxy.rs` and can be easily modified since it's not user-facing.

## Error Handling

- Downloader unavailable: Log warning, serve existing data
- Partial fetch failure: Store what succeeded, log errors
- Timeout: Configurable via CLI, default 300s for large files
- All errors use `anyhow::Result` consistent with codebase

## Testing Strategy

- Unit tests for `DownloaderClient` with mock server
- Integration tests using existing reqwest test patterns
- Manual testing with real downloader service

---

## Downloader API Response Models

### Artist Response
```json
{
  "id": "5a2EaR3hamoenG9rDuVn8j",
  "name": "Prince",
  "genre": ["funk", "rock"],           // Note: singular "genre"
  "portraits": [],
  "activity_periods": [{"Decade": 2000}, {"Decade": 1990}],
  "related": ["artist_id_1", "artist_id_2", ...],  // IDs only
  "portrait_group": [
    {"id": "abc123", "size": "DEFAULT", "width": 320, "height": 320},
    {"id": "abc124", "size": "SMALL", "width": 160, "height": 160},
    {"id": "abc125", "size": "LARGE", "width": 640, "height": 640}
  ]
}
```

### Album Response
```json
{
  "id": "2umoqwMrmjBBPeaqgYu6J9",
  "name": "Purple Rain",
  "album_type": "ALBUM",               // Uppercase: ALBUM, SINGLE, EP, etc.
  "artists_ids": ["artist_id"],
  "label": "Warner Records",
  "date": 456969600,                   // Unix timestamp
  "genres": [],
  "covers": [...],                     // Same format as portrait_group
  "discs": [
    {
      "number": 1,
      "name": "",
      "tracks": ["track_id_1", "track_id_2", ...]  // IDs only
    }
  ],
  "cover_group": [...],
  "original_title": "Purple Rain",
  "version_title": ""
}
```

### Track Response
```json
{
  "id": "1uvyZBs4IZYRebHIB1747m",
  "name": "Purple Rain",
  "album_id": "album_id",
  "artists_ids": ["artist_id"],
  "number": 9,                         // track_number
  "disc_number": 1,
  "duration": 521866,                  // milliseconds (divide by 1000 for secs)
  "is_explicit": false,
  "files": {                           // format -> file_id mapping
    "OGG_VORBIS_160": "file_hash_1",
    "OGG_VORBIS_320": "file_hash_2",
    "AAC_24": "file_hash_3"
  },
  "tags": [],
  "has_lyrics": true,
  "language_of_performance": ["en"],   // -> languages
  "original_title": "Purple Rain",
  "version_title": "",
  "artists_with_role": [
    {"artist_id": "...", "name": "Prince", "role": "ARTIST_ROLE_MAIN_ARTIST"}
  ]
}
```

## Model Mapping (Downloader → Catalog)

| Downloader Field | Catalog Field | Transformation |
|------------------|---------------|----------------|
| **Artist** |
| `genre` | `genres` | Direct (rename) |
| `activity_periods[].Decade` | `activity_periods[].Decade` | Direct |
| `related` | `related_artists` table | Insert relationships |
| `portrait_group` | `artist_images` table | Fetch images, store files |
| **Album** |
| `album_type` | `album_type` | Lowercase: "ALBUM" → "Album" |
| `artists_ids` | `album_artists` table | Insert relationships |
| `date` | `release_date` | Direct (both unix timestamp) |
| `discs[].tracks` | tracks table | Fetch each track via API |
| `cover_group` | `album_images` table | Fetch images, store files |
| **Track** |
| `number` | `track_number` | Direct |
| `duration` | `duration_secs` | Divide by 1000 |
| `files` | `audio_uri` + `format` | Pick format, download, store path |
| `language_of_performance` | `languages` | Direct |
| `artists_with_role` | `track_artists` table | Map role strings |

## Format Handling

The downloader automatically selects the best format when calling `GET /track/:id/audio`.

To determine the returned format for catalog storage:
1. Check `Content-Type` response header (e.g., `audio/ogg`, `audio/aac`)
2. Fallback: Use first key from track's `files` map as the format

Map downloader format strings to catalog `Format` enum:
- `OGG_VORBIS_320` → `OggVorbis320`
- `OGG_VORBIS_160` → `OggVorbis160`
- `OGG_VORBIS_96` → `OggVorbis96`
- `AAC_24` → `Aac24`
- etc.

## Image & Audio File Handling

**Images:**
- Downloader returns image `id` in metadata, not a URI
- Fetch binary via `GET /image/:id`
- Store to `{media_path}/images/{id}.jpg`
- Set `Image.uri` to relative path `images/{id}.jpg`

**Audio:**
- Fetch binary via `GET /track/:id/audio` (downloader picks best format)
- Store to `{media_path}/tracks/{album_id}/{track_id}.ogg` (or appropriate extension)
- Set `Track.audio_uri` to relative path
- Set `Track.format` based on Content-Type or files map
