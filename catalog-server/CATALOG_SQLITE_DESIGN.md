# Catalog SQLite Migration Design

This document outlines the design for migrating the catalog from filesystem-based JSON storage to SQLite.

## Overview

### Goals
- Replace filesystem-based catalog (JSON files in `albums/`, `artists/` directories) with SQLite
- SQLite becomes the source of truth for catalog metadata
- Audio and image files remain on filesystem, referenced by relative URIs
- Enable catalog editing via API endpoints
- Clean break from filesystem catalog (import tool for migration)

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    catalog.db                           │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│  │ artists │ │ albums  │ │ tracks  │ │ images  │       │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘       │
│  + join tables for many-to-many relationships          │
└─────────────────────────────────────────────────────────┘
                           │
                           │ metadata queries
                           ▼
┌─────────────────────────────────────────────────────────┐
│              Filesystem (media_base_path)               │
│  ┌─────────────┐  ┌─────────────────────────────┐      │
│  │   images/   │  │   albums/<album_id>/        │      │
│  │   *.jpg     │  │   track_<track_id>.<ext>    │      │
│  └─────────────┘  └─────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```toml
[catalog]
db_path = "/path/to/catalog.db"
media_base_path = "/mnt/music"  # Base path for resolving relative URIs
```

URIs in the database are stored as relative paths. At runtime, they're resolved against `media_base_path`:
```rust
fn resolve_uri(&self, relative_uri: &str) -> PathBuf {
    self.media_base_path.join(relative_uri)
}
```

This makes the database portable and allows easy migration to different storage backends (local, NFS, S3).

---

## Database Schema

### Core Tables

```sql
-- Artists
CREATE TABLE artists (
    id TEXT PRIMARY KEY,  -- 'R...' prefix
    name TEXT NOT NULL,
    genres TEXT,  -- JSON array: ["rock", "metal"]
    activity_periods TEXT  -- JSON array: [{"Timespan": {"start_year": 1990, "end_year": null}}]
);

-- Albums
CREATE TABLE albums (
    id TEXT PRIMARY KEY,  -- 'A...' prefix
    name TEXT NOT NULL,
    album_type TEXT NOT NULL,  -- 'ALBUM', 'SINGLE', 'EP', etc.
    label TEXT,
    release_date INTEGER,  -- Unix timestamp
    genres TEXT,  -- JSON array
    original_title TEXT,
    version_title TEXT
);

-- Tracks
CREATE TABLE tracks (
    id TEXT PRIMARY KEY,  -- 'T...' prefix
    name TEXT NOT NULL,
    album_id TEXT NOT NULL,
    disc_number INTEGER NOT NULL DEFAULT 1,
    track_number INTEGER NOT NULL,
    duration_secs INTEGER,
    is_explicit INTEGER NOT NULL DEFAULT 0,
    audio_uri TEXT NOT NULL,  -- Relative path: 'albums/A123/track_T456.mp3'
    format TEXT NOT NULL,  -- 'MP3_320', 'FLAC_FLAC', etc.
    tags TEXT,  -- JSON array
    has_lyrics INTEGER NOT NULL DEFAULT 0,
    languages TEXT,  -- JSON array
    original_title TEXT,
    version_title TEXT,
    FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE
);

-- Images
CREATE TABLE images (
    id TEXT PRIMARY KEY,
    uri TEXT NOT NULL,  -- Relative path: 'images/abc123.jpg'
    size TEXT NOT NULL,  -- 'DEFAULT', 'SMALL', 'LARGE', 'XLARGE'
    width INTEGER NOT NULL,
    height INTEGER NOT NULL
);
```

### Relationship Tables

```sql
-- Album <-> Artist (many-to-many)
CREATE TABLE album_artists (
    album_id TEXT NOT NULL,
    artist_id TEXT NOT NULL,
    position INTEGER NOT NULL,  -- Ordering of artists
    PRIMARY KEY (album_id, artist_id),
    FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE,
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

-- Track <-> Artist (many-to-many with role)
CREATE TABLE track_artists (
    track_id TEXT NOT NULL,
    artist_id TEXT NOT NULL,
    role TEXT NOT NULL,  -- 'MAIN_ARTIST', 'FEATURED_ARTIST', 'COMPOSER', etc.
    position INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist_id, role),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

-- Artist <-> Artist (related artists, many-to-many)
CREATE TABLE related_artists (
    artist_id TEXT NOT NULL,
    related_artist_id TEXT NOT NULL,
    PRIMARY KEY (artist_id, related_artist_id),
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE,
    FOREIGN KEY (related_artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

-- Artist <-> Image (many-to-many with type)
CREATE TABLE artist_images (
    artist_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    image_type TEXT NOT NULL,  -- 'portrait', 'portrait_group'
    position INTEGER NOT NULL,
    PRIMARY KEY (artist_id, image_id, image_type),
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE,
    FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
);

-- Album <-> Image (many-to-many with type)
CREATE TABLE album_images (
    album_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    image_type TEXT NOT NULL,  -- 'cover', 'cover_group'
    position INTEGER NOT NULL,
    PRIMARY KEY (album_id, image_id, image_type),
    FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE,
    FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
);
```

### Indices

```sql
CREATE INDEX idx_tracks_album ON tracks(album_id);
CREATE INDEX idx_tracks_disc_number ON tracks(album_id, disc_number, track_number);
CREATE INDEX idx_album_artists_artist ON album_artists(artist_id);
CREATE INDEX idx_track_artists_artist ON track_artists(artist_id);
CREATE INDEX idx_artist_images_artist ON artist_images(artist_id);
CREATE INDEX idx_album_images_album ON album_images(album_id);
```

### Full-Text Search (Optional)

```sql
-- FTS5 virtual table for search
CREATE VIRTUAL TABLE catalog_fts USING fts5(
    entity_type,  -- 'artist', 'album', 'track'
    entity_id UNINDEXED,
    name,
    extra_text,  -- genres, tags, etc.
    tokenize='unicode61'
);

-- Populate from main tables
INSERT INTO catalog_fts (entity_type, entity_id, name, extra_text)
SELECT 'artist', id, name, genres FROM artists;

INSERT INTO catalog_fts (entity_type, entity_id, name, extra_text)
SELECT 'album', id, name, genres FROM albums;

INSERT INTO catalog_fts (entity_type, entity_id, name, extra_text)
SELECT 'track', id, name, tags FROM tracks;
```

---

## Rust Models

### Core Entities

```rust
/// Audio format enumeration
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Format {
    OggVorbis96,
    OggVorbis160,
    OggVorbis320,
    Mp3_96,
    Mp3_160,
    Mp3_256,
    Mp3_320,
    Aac24,
    Aac48,
    Aac160,
    Aac320,
    Flac,
    Unknown,
}

/// Artist role on a track
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ArtistRole {
    MainArtist,
    FeaturedArtist,
    Remixer,
    Composer,
    Conductor,
    Orchestra,
    Actor,
    Unknown,
}

/// Activity period for an artist
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActivityPeriod {
    Timespan {
        start_year: u16,
        end_year: Option<u16>,
    },
    Decade(u16),
}

/// Album type classification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlbumType {
    Album,
    Single,
    Ep,
    Compilation,
    Audiobook,
    Podcast,
}

/// Image size classification
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageSize {
    Small,
    Default,
    Large,
    XLarge,
}

/// Image metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub uri: String,
    pub size: ImageSize,
    pub width: u16,
    pub height: u16,
}

/// Artist entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub genres: Vec<String>,
    pub activity_periods: Vec<ActivityPeriod>,
}

/// Album entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub album_type: AlbumType,
    pub label: Option<String>,
    pub release_date: Option<i64>,
    pub genres: Vec<String>,
    pub original_title: Option<String>,
    pub version_title: Option<String>,
}

/// Track entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub album_id: String,
    pub disc_number: i32,
    pub track_number: i32,
    pub duration_secs: i32,
    pub is_explicit: bool,
    pub audio_uri: String,
    pub format: Format,
    pub tags: Vec<String>,
    pub has_lyrics: bool,
    pub languages: Vec<String>,
    pub original_title: Option<String>,
    pub version_title: Option<String>,
}
```

### Relationship Types

```rust
/// Artist with their role on a track
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackArtist {
    pub artist: Artist,
    pub role: ArtistRole,
}

/// Disc grouping for album tracks
#[derive(Clone, Debug, Serialize)]
pub struct Disc {
    pub number: i32,
    pub name: Option<String>,
    pub tracks: Vec<Track>,
}
```

### Resolved/Composite Types (API Responses)

```rust
/// Full artist with all related data
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedArtist {
    pub artist: Artist,
    pub images: Vec<Image>,
    pub related_artists: Vec<Artist>,
}

/// Full album with tracks and artists
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedAlbum {
    pub album: Album,
    pub artists: Vec<Artist>,
    pub discs: Vec<Disc>,
    pub images: Vec<Image>,
}

/// Track with its artists
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedTrack {
    pub track: Track,
    pub album: Album,
    pub artists: Vec<TrackArtist>,
}

/// Artist's complete discography
#[derive(Clone, Debug, Serialize)]
pub struct ArtistDiscography {
    pub albums: Vec<Album>,    // Albums where artist is primary
    pub features: Vec<Album>,  // Albums where artist is featured on tracks
}
```

---

## CatalogStore Trait

```rust
#[async_trait]
pub trait CatalogStore: Send + Sync {
    // === Read Operations ===

    async fn get_artist(&self, id: &str) -> Result<Option<Artist>>;
    async fn get_resolved_artist(&self, id: &str) -> Result<Option<ResolvedArtist>>;
    async fn get_artist_discography(&self, id: &str) -> Result<Option<ArtistDiscography>>;

    async fn get_album(&self, id: &str) -> Result<Option<Album>>;
    async fn get_resolved_album(&self, id: &str) -> Result<Option<ResolvedAlbum>>;

    async fn get_track(&self, id: &str) -> Result<Option<Track>>;
    async fn get_resolved_track(&self, id: &str) -> Result<Option<ResolvedTrack>>;

    async fn get_image(&self, id: &str) -> Result<Option<Image>>;

    // URI resolution
    fn resolve_audio_uri(&self, track: &Track) -> PathBuf;
    fn resolve_image_uri(&self, image: &Image) -> PathBuf;

    // === Write Operations ===

    async fn create_artist(&self, artist: &Artist) -> Result<()>;
    async fn update_artist(&self, artist: &Artist) -> Result<()>;
    async fn delete_artist(&self, id: &str) -> Result<()>;

    async fn create_album(&self, album: &Album, artist_ids: &[String]) -> Result<()>;
    async fn update_album(&self, album: &Album) -> Result<()>;
    async fn delete_album(&self, id: &str) -> Result<()>;

    async fn create_track(&self, track: &Track, artists: &[(String, ArtistRole)]) -> Result<()>;
    async fn update_track(&self, track: &Track) -> Result<()>;
    async fn delete_track(&self, id: &str) -> Result<()>;

    async fn create_image(&self, image: &Image) -> Result<()>;
    async fn delete_image(&self, id: &str) -> Result<()>;

    // === Relationship Operations ===

    async fn set_album_artists(&self, album_id: &str, artist_ids: &[String]) -> Result<()>;
    async fn set_track_artists(&self, track_id: &str, artists: &[(String, ArtistRole)]) -> Result<()>;
    async fn set_related_artists(&self, artist_id: &str, related_ids: &[String]) -> Result<()>;
    async fn set_artist_images(&self, artist_id: &str, images: &[(String, &str)]) -> Result<()>;  // (image_id, type)
    async fn set_album_images(&self, album_id: &str, images: &[(String, &str)]) -> Result<()>;

    // === Search ===

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults>;
}
```

---

## Implementation Plan

### Phase 1: Foundation
1. Define SQLite schema with migrations (extend `sqlite_persistence`)
2. Implement new Rust model structs
3. Implement `SqliteCatalogStore` with read operations

### Phase 2: Import Tool
4. Create `catalog-import` binary
5. Read filesystem catalog (existing JSON parsing code)
6. Transform to new models and insert into SQLite
7. Validate imported data

### Phase 3: Server Integration
8. Replace `GuardedCatalog` with `Arc<dyn CatalogStore>`
9. Update server handlers to use new store
10. Update response serialization for new model shapes

### Phase 4: Write Operations
11. Implement write operations in `SqliteCatalogStore`
12. Add catalog editing API endpoints
13. Add validation for write operations

### Phase 5: Search
14. Evaluate: keep PezzotHash (rebuild from SQLite) vs migrate to FTS5
15. Implement chosen search solution
16. Update search endpoints

### Phase 6: Cleanup
17. Remove filesystem catalog loading code
18. Remove old model definitions
19. Update tests
20. Update documentation

---

## Migration Path

1. Build new system alongside existing
2. Create import tool to populate SQLite from filesystem catalog
3. Run both systems in parallel for testing
4. Switch server to use SQLite backend
5. Remove filesystem catalog code

---

## Changes from Current System

| Aspect | Current (Filesystem) | New (SQLite) |
|--------|---------------------|--------------|
| Storage | JSON files in directories | SQLite database |
| Loading | Full parse at startup | On-demand queries |
| Memory | Everything in RAM | Query results only |
| Editing | Edit JSON files | API endpoints |
| Search | PezzotHash in-memory | FTS5 or rebuilt PezzotHash |
| URIs | Derived from IDs | Explicit relative paths |
| Portability | Copy directory tree | Copy DB + configure base path |

## Model Changes

| Field | Current | New |
|-------|---------|-----|
| `Track.files` | `HashMap<Format, String>` | `audio_uri: String` + `format: Format` |
| `Track.artists_ids` + `artists_with_role` | Both exist (redundant) | Single join table with role |
| `Artist.genre` | Singular name | `genres` (consistent) |
| `Image` | No URI | Has `uri` field |
| `ResolvedTrack` | Alias for `ResolvedAlbum` | Proper distinct struct |
| Optional fields | Empty string defaults | `Option<T>` |

## Removed Fields

These fields from the current models are dropped (unused or redundant):
- `Track.alternatives` - not used
- `Track.earliest_live_timestamp` - not used
- `Album.type_str` - redundant with `album_type`
- `Album.related` - not populated
- `Album.covers` vs `cover_group` - consolidated to single image relationship with type
- `Artist.portraits` vs `portrait_group` - consolidated to single image relationship with type
