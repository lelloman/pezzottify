# Downloader2 - Torrent-Based Music Downloader

## Overview

Downloader2 is a standalone service that receives download "tickets" from catalog-server and autonomously fetches, converts, and places audio files. It uses Jackett for torrent search, LLM for matching torrents to tickets, qBittorrent for downloading, and ffmpeg for conversion.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            DOWNLOADER2                                  │
│                                                                         │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────────┐│
│  │   HTTP API   │────▶│    Queue     │────▶│      Processor           ││
│  │              │     │   Manager    │     │                          ││
│  │ - tickets    │     │              │     │  ┌────────────────────┐  ││
│  │ - status     │     │ - SQLite     │     │  │  1. Searcher       │  ││
│  │ - admin      │     │ - state      │     │  │     (Jackett)      │  ││
│  │ - websocket  │     │   machine    │     │  ├────────────────────┤  ││
│  └──────────────┘     └──────────────┘     │  │  2. Matcher        │  ││
│                                            │  │     (LLM)          │  ││
│                                            │  ├────────────────────┤  ││
│                                            │  │  3. Downloader     │  ││
│                                            │  │     (qBittorrent)  │  ││
│                                            │  ├────────────────────┤  ││
│                                            │  │  4. Converter      │  ││
│                                            │  │     (ffmpeg)       │  ││
│                                            │  ├────────────────────┤  ││
│                                            │  │  5. Placer         │  ││
│                                            │  │     (file ops)     │  ││
│                                            │  └────────────────────┘  ││
│                                            └──────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘
         ▲                                              │
         │ Tickets                                      │ Files placed at dest_path
         │                                              ▼
┌─────────────────┐                           ┌─────────────────┐
│  catalog-server │                           │   Media Storage │
└─────────────────┘                           └─────────────────┘
```

## Ticket Structure

The ticket is the contract between catalog-server and downloader2:

```json
{
  "ticket_id": "uuid",
  "created_at": "2024-12-24T10:00:00Z",

  "search": {
    "artist": "Radiohead",
    "album": "OK Computer",
    "year": 1997,
    "label": "Parlophone",
    "genres": ["alternative rock", "art rock"]
  },

  "tracks": [
    {
      "catalog_track_id": "t1",
      "disc_number": 1,
      "track_number": 1,
      "name": "Airbag",
      "duration_secs": 284,
      "dest_path": "/media/albums/abc123/d1t01.ogg"
    },
    {
      "catalog_track_id": "t2",
      "disc_number": 1,
      "track_number": 2,
      "name": "Paranoid Android",
      "duration_secs": 383,
      "dest_path": "/media/albums/abc123/d1t02.ogg"
    }
  ],

  "images": [
    {
      "catalog_image_id": "img1",
      "type": "cover_front",
      "dest_path": "/media/albums/abc123/cover.jpg"
    }
  ],

  "constraints": {
    "format": "ogg_vorbis",
    "bitrate_kbps": 320,
    "sample_rate_hz": 44100,
    "embed_metadata": true,
    "embed_cover": true
  },

  "metadata_to_embed": {
    "artist": "Radiohead",
    "album": "OK Computer",
    "year": 1997,
    "genre": "Alternative Rock"
  }
}
```

## State Machine

```
┌─────────────┐
│   PENDING   │ ← Ticket received, queued for processing
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  SEARCHING  │ ← Querying Jackett for torrent candidates
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  MATCHING   │ ← LLM evaluates candidates against ticket
└──────┬──────┘
       │
       ├─────────────────────────────────┐
       │ confidence >= threshold         │ confidence < threshold
       ▼                                 ▼
┌───────────────┐               ┌────────────────────┐
│ AUTO_APPROVED │               │  NEEDS_APPROVAL    │ ← Admin must review
└───────┬───────┘               └─────────┬──────────┘
        │                                 │
        │                      ┌──────────┴──────────┐
        │                      │                     │
        │               admin approves          admin rejects
        │                      │                     │
        │                      ▼                     ▼
        │               ┌──────────────┐    ┌────────────┐
        │               │   APPROVED   │    │  REJECTED  │ (terminal)
        │               └──────┬───────┘    └────────────┘
        │                      │
        └──────────┬───────────┘
                   │
                   ▼
          ┌───────────────┐
          │  DOWNLOADING  │ ← Torrent downloading via qBittorrent
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │  CONVERTING   │ ← ffmpeg: transcode + embed metadata
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │    PLACING    │ ← Moving files to dest_path
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │   COMPLETED   │ (terminal)
          └───────────────┘

          (any non-terminal state)
                  │
               on error
                  ▼
          ┌───────────────┐
          │    FAILED     │ (terminal, may be retryable)
          └───────────────┘
```

### State Details

```rust
enum TicketState {
    Pending,

    Searching {
        started_at: DateTime<Utc>,
    },

    Matching {
        candidates: Vec<TorrentCandidate>,
        started_at: DateTime<Utc>,
    },

    NeedsApproval {
        candidates: Vec<ScoredCandidate>,
        recommended_idx: usize,
        confidence: f32,
        reason: ApprovalReason,
        waiting_since: DateTime<Utc>,
    },

    AutoApproved {
        selected: ScoredCandidate,
        confidence: f32,
    },

    Approved {
        selected: ScoredCandidate,
        approved_by: String,
        approved_at: DateTime<Utc>,
    },

    Downloading {
        torrent_hash: String,
        progress_pct: f32,
        download_speed_bps: u64,
        eta_secs: Option<u32>,
        started_at: DateTime<Utc>,
    },

    Converting {
        current_track_idx: usize,
        total_tracks: usize,
        current_track_name: String,
        started_at: DateTime<Utc>,
    },

    Placing {
        files_placed: usize,
        total_files: usize,
        started_at: DateTime<Utc>,
    },

    Completed {
        completed_at: DateTime<Utc>,
        stats: CompletionStats,
    },

    Rejected {
        rejected_by: String,
        reason: Option<String>,
        rejected_at: DateTime<Utc>,
    },

    Failed {
        failed_at_state: String,
        error: String,
        retryable: bool,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    },
}

enum ApprovalReason {
    LowConfidence { score: f32, threshold: f32 },
    TrackCountMismatch { expected: usize, found: usize },
    DurationMismatch { expected_secs: u32, found_secs: u32 },
    NameSimilarityLow { similarity: f32 },
    MultipleGoodMatches { top_scores: Vec<f32> },
    NoExactMatch,
}

struct ScoredCandidate {
    torrent: TorrentCandidate,
    score: f32,
    track_mapping: Vec<TrackMapping>,
    reasoning: String,  // LLM explanation
}

struct TrackMapping {
    catalog_track_id: String,
    torrent_file_path: String,
    confidence: f32,
}

struct CompletionStats {
    total_download_bytes: u64,
    download_duration_secs: u32,
    conversion_duration_secs: u32,
    final_size_bytes: u64,
}
```

## API Endpoints

### Ticket Management

```
POST   /api/v1/ticket
       Body: Ticket JSON
       → Creates new ticket, returns ticket_id

GET    /api/v1/ticket/{ticket_id}
       → Returns full ticket state and history

GET    /api/v1/tickets
       Query params: ?state=needs_approval&limit=50&offset=0
       → Lists tickets with filtering

DELETE /api/v1/ticket/{ticket_id}
       → Cancels ticket (if not terminal)
```

### Admin Actions

```
POST   /api/v1/ticket/{ticket_id}/approve
       Body: { "candidate_idx": 0 }  (optional, uses recommended if omitted)
       → Approves ticket with selected candidate

POST   /api/v1/ticket/{ticket_id}/reject
       Body: { "reason": "Wrong album" }
       → Rejects ticket

POST   /api/v1/ticket/{ticket_id}/retry
       → Retries failed ticket from last safe state

POST   /api/v1/ticket/{ticket_id}/force-search
       Body: { "query": "custom search query" }
       → Manual search override
```

### Status & Health

```
GET    /api/v1/health
       → Service health check

GET    /api/v1/stats
       → Queue stats, processing rates, etc.

GET    /api/v1/config
       → Current configuration (admin only)
```

### Real-time Updates

```
WS     /api/v1/ws
       → WebSocket for state change notifications

       Messages:
       - { "type": "state_change", "ticket_id": "...", "old_state": "...", "new_state": "...", "details": {...} }
       - { "type": "progress", "ticket_id": "...", "progress_pct": 45.2 }
       - { "type": "needs_approval", "ticket_id": "...", "candidates": [...] }
```

## Components

### 1. HTTP API (`api/`)

Axum-based HTTP server:
- Ticket CRUD endpoints
- Admin action endpoints
- WebSocket for real-time updates
- Authentication (shared secret or JWT)

### 2. Queue Manager (`queue/`)

- SQLite-backed ticket persistence
- State machine enforcement
- Retry logic with exponential backoff
- Priority handling

### 3. Searcher (`searcher/`)

Jackett integration:
- Search by artist + album
- Filter by category (music)
- Parse results into `TorrentCandidate` structs
- Handle rate limiting

```rust
struct TorrentCandidate {
    title: String,
    indexer: String,
    magnet_uri: String,
    info_hash: String,
    size_bytes: u64,
    seeders: u32,
    leechers: u32,
    files: Option<Vec<TorrentFile>>,  // If available from indexer
    publish_date: DateTime<Utc>,
}
```

### 4. Matcher (`matcher/`)

LLM-based matching:
- Takes ticket + candidates
- Scores each candidate
- Maps torrent files to catalog tracks
- Returns confidence score + reasoning

```rust
#[async_trait]
trait Matcher {
    async fn score_candidates(
        &self,
        ticket: &Ticket,
        candidates: Vec<TorrentCandidate>,
    ) -> Result<Vec<ScoredCandidate>>;
}
```

LLM prompt considerations:
- Track count matching
- Track name similarity
- Duration matching (if available)
- Quality indicators in torrent name (FLAC, 320, etc.)
- Release year matching
- Avoid compilations, remasters unless specified

### 5. Torrent Client (`torrent_client/`)

qBittorrent Web API integration:
- Add torrent by magnet URI
- Monitor download progress
- Get file list when complete
- Remove torrent after processing
- Handle seeding requirements (configurable ratio)

```rust
#[async_trait]
trait TorrentClient {
    async fn add_torrent(&self, magnet: &str, save_path: &Path) -> Result<String>;  // Returns hash
    async fn get_progress(&self, hash: &str) -> Result<TorrentProgress>;
    async fn get_files(&self, hash: &str) -> Result<Vec<TorrentFile>>;
    async fn remove_torrent(&self, hash: &str, delete_files: bool) -> Result<()>;
}
```

### 6. Converter (`converter/`)

ffmpeg wrapper:
- Transcode to target format
- Normalize bitrate/sample rate
- Embed metadata tags
- Embed cover art
- Validate output with ffprobe

```rust
struct ConversionJob {
    input_path: PathBuf,
    output_path: PathBuf,
    format: AudioFormat,
    bitrate_kbps: u32,
    sample_rate_hz: u32,
    metadata: HashMap<String, String>,
    cover_art: Option<PathBuf>,
}

#[async_trait]
trait Converter {
    async fn convert(&self, job: ConversionJob) -> Result<ConversionResult>;
    async fn validate(&self, path: &Path) -> Result<AudioInfo>;
}
```

### 7. Placer (`placer/`)

File operations:
- Move converted files to dest_path
- Create directories as needed
- Verify file integrity after move
- Cleanup temp files

## Configuration

```toml
[server]
host = "0.0.0.0"
port = 8080
auth_token = "secret"  # For catalog-server authentication

[database]
path = "/data/downloader2.db"

[jackett]
url = "http://localhost:9117"
api_key = "your-jackett-api-key"
timeout_secs = 30

[qbittorrent]
url = "http://localhost:8081"
username = "admin"
password = "adminadmin"
download_path = "/downloads/incomplete"
seed_ratio_limit = 0.0  # 0 = no seeding
seed_time_limit_mins = 0

[matcher]
llm_provider = "anthropic"  # or "openai", "local"
llm_model = "claude-3-haiku"
api_key = "your-api-key"
auto_approve_threshold = 0.85  # Confidence threshold for auto-approval

[converter]
ffmpeg_path = "/usr/bin/ffmpeg"
ffprobe_path = "/usr/bin/ffprobe"
temp_dir = "/tmp/downloader2"
max_parallel_conversions = 4

[processing]
max_parallel_downloads = 2
check_interval_secs = 10
retry_max_attempts = 3
retry_initial_delay_secs = 60
retry_max_delay_secs = 3600
```

## Directory Structure

```
downloader2/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   ├── handlers.rs
│   │   ├── websocket.rs
│   │   └── auth.rs
│   ├── queue/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── state.rs
│   │   └── store.rs          # SQLite persistence
│   ├── searcher/
│   │   ├── mod.rs
│   │   └── jackett.rs
│   ├── matcher/
│   │   ├── mod.rs
│   │   ├── llm.rs
│   │   └── scoring.rs
│   ├── torrent_client/
│   │   ├── mod.rs
│   │   └── qbittorrent.rs
│   ├── converter/
│   │   ├── mod.rs
│   │   └── ffmpeg.rs
│   ├── placer/
│   │   ├── mod.rs
│   │   └── file_ops.rs
│   └── models/
│       ├── mod.rs
│       ├── ticket.rs
│       ├── state.rs
│       └── torrent.rs
├── tests/
│   ├── integration/
│   │   ├── api_tests.rs
│   │   ├── queue_tests.rs
│   │   └── end_to_end.rs
│   └── mocks/
│       ├── jackett_mock.rs
│       ├── qbittorrent_mock.rs
│       └── llm_mock.rs
└── docker/
    ├── Dockerfile
    └── docker-compose.yml    # Full stack: downloader2 + jackett + qbittorrent
```

## Testing Strategy

### Unit Tests
- State machine transitions
- Ticket validation
- Score calculation logic

### Integration Tests (with mocks)
- API endpoint tests
- Queue processing with mocked external services
- Converter with real ffmpeg but test files

### End-to-End Tests
- Docker compose with real Jackett + qBittorrent
- Test with legal/free torrents (e.g., creative commons music)

## Implementation Phases

### Phase 1: Core Infrastructure
- [ ] Project setup (Cargo, dependencies)
- [ ] Configuration loading
- [ ] SQLite schema + migrations
- [ ] Basic HTTP API (health, ticket CRUD)
- [ ] State machine implementation
- [ ] Queue manager

### Phase 2: External Integrations
- [ ] Jackett client
- [ ] qBittorrent client
- [ ] Basic searcher (no LLM yet, simple text matching)

### Phase 3: Processing Pipeline
- [ ] Converter (ffmpeg wrapper)
- [ ] Placer (file operations)
- [ ] End-to-end processing without LLM matching

### Phase 4: Smart Matching
- [ ] LLM integration (Anthropic/OpenAI)
- [ ] Scoring algorithm
- [ ] Track-to-file mapping
- [ ] Approval workflow

### Phase 5: Production Readiness
- [ ] WebSocket real-time updates
- [ ] Comprehensive error handling
- [ ] Retry logic
- [ ] Metrics/observability
- [ ] Docker packaging

### Phase 6: Catalog-Server Integration
- [ ] Update catalog-server to use new ticket format
- [ ] Admin UI for approval workflow
- [ ] Migration from old downloader

## Open Questions

1. **Seeding policy**: How long to seed after download? Ratio-based or time-based?

2. **Failed matching**: If no good torrents found, should we:
   - Fail immediately?
   - Retry periodically?
   - Notify admin to manually search?

3. **Partial success**: If 10/12 tracks convert successfully, should we:
   - Fail the whole ticket?
   - Complete with partial results and flag?

4. **Cover art source**: Download from torrent or fetch separately (e.g., MusicBrainz, Discogs)?

5. **Duplicate detection**: What if same album is requested while one is already processing?
