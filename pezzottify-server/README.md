# Pezzottify Server

A high-performance Rust backend server for the Pezzottify music streaming platform. Handles music catalog management, user authentication, audio streaming, and search functionality.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Catalog Directory Structure](#catalog-directory-structure)
- [Building](#building)
- [Docker Build](#docker-build)
- [Running the Server](#running-the-server)
- [Command-line Arguments](#command-line-arguments)
- [Build Features](#build-features)
- [Configuration](#configuration)
  - [Rate Limits](#rate-limits)
  - [HTTP Caching](#http-caching)
  - [Metadata Enrichment](#metadata-enrichment)
- [API Endpoints](#api-endpoints)
- [Download Manager](#download-manager)
- [Authentication & Authorization](#authentication--authorization)
- [CLI Auth Tool](#cli-auth-tool)
- [Testing](#testing)
- [Development Tips](#development-tips)
- [Monitoring & Alerting](#monitoring--alerting)

## Overview

The pezzottify server is the backend component of Pezzottify that provides:

- **Music Catalog Management**: SQLite-backed catalog with CRUD operations
- **Audio Streaming**: HTTP range request support for efficient audio playback
- **User Authentication**: Token-based authentication with Argon2 password hashing
- **Authorization**: Role-based permissions system (Admin/Regular users)
- **Search**: Full-text search across artists, albums, and tracks
- **User Content**: Playlists and liked content management
- **Rate Limiting**: Per-endpoint rate limiting to prevent abuse
- **Download Manager**: Queue-based content acquisition from external music providers
- **Metadata Enrichment**: Queue-backed LLM enrichment for artist, album, and track facts

## Architecture

### Core Modules

- **`catalog_store/`**: SQLite-backed catalog management

  - `SqliteCatalogStore`: Main store implementation with CRUD operations
  - `CatalogStore` trait: Abstract interface for catalog access
  - Validation for write operations (foreign keys, duplicates)
  - Transactional writes with `BEGIN IMMEDIATE`

- **`user/`**: Authentication and authorization

  - `SqliteUserStore`: User persistence in SQLite database
  - `UserManager`: Authentication with Argon2 password hashing and RSA token signing
  - `permissions.rs`: Role-based permissions (Admin, Regular)
  - `auth.rs`: Token-based authentication with RSA signing
  - `device.rs`: Device tracking and management

- **`server/`**: Axum-based HTTP server

  - `session.rs`: Session management via HTTP-only cookies
  - `stream_track.rs`: Audio streaming with range request support
  - `search.rs`: Search API routes
  - `http_layers/`: Middleware for logging, caching, rate limiting, optional slowdown

- **`search/`**: Search functionality

  - `Fts5LevenshteinSearchVault`: SQLite FTS5 with trigram tokenizer and Levenshtein typo correction
  - `streaming/`: Streaming search pipeline with target identification and enrichment

- **`download_manager/`**: Passive download request queue

  - `DownloadManager`: Main facade for download operations
  - `DownloadQueueStore`: SQLite-backed queue persistence (download_queue.db)
  - `AuditLogger`: Comprehensive audit trail for all operations

- **`background_jobs/`**: Scheduled background task system

  - `JobScheduler`: Manages job scheduling and execution
  - `PopularContentJob`: Computes popular content metrics
  - `MetadataEnrichmentJob`: Processes queued artist, album, and track metadata enrichment
  - `IntegrityWatchdogJob`: Periodic integrity scans
  - `AuditLogCleanupJob`: Cleans old audit log entries

- **`enrichment_store/`**: SQLite-backed enrichment database

  - `artist_enrichment_v1`, `album_enrichment_v1`, and `track_enrichment_v1`: typed canonical metadata tables
  - `enrichment_queue_v1`: pending/running/completed enrichment queue state
  - Child tables for tags, contributors, relations, aliases, sources, external IDs, and evidence

- **`agent/`**: LLM provider abstraction used by AI-assisted features

  - Ollama and OpenAI-compatible chat providers
  - Shared `[agent.llm]` configuration for generated show scripts and metadata enrichment

- **`sqlite_persistence/`**: Database schema management
  - `versioned_schema.rs`: Schema migrations with version tracking

### Key Types

- **`SqliteCatalogStore`**: SQLite-backed catalog with CRUD operations
- **`CatalogStore`**: Trait for catalog access (read and write operations)
- **`Session`**: Request session containing user ID, token, and permissions
- **`Permission`**: Enum for access control:
  - `AccessCatalog`: View catalog content
  - `LikeContent`: Like/unlike content
  - `OwnPlaylists`: Create and manage playlists
  - `EditCatalog`: Create, update, delete catalog entries
  - `ManagePermissions`: Manage user permissions
  - `IssueContentDownload`: Issue download tokens
  - `ServerAdmin`: Server administration (reboot, etc.)
  - `ViewAnalytics`: View listening analytics and statistics
  - `RequestContent`: Request content downloads from external provider
- **`UserRole`**: Admin or Regular with different permission sets

## Prerequisites

- **Rust**: Latest stable toolchain (install via [rustup](https://rustup.rs/))
- **ffprobe** (optional): Required for `--check-all` catalog validation
- **SQLite**: Bundled via rusqlite (no separate installation needed)

## Installation

1. Clone the repository:

   ```bash
   git clone https://github.com/lelloman/pezzottify
   cd pezzottify/pezzottify-server
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

## Media Directory Structure

The server expects a media directory (specified via `--media-path`) with audio files and images:

```
<media-root>/
├── albums/
│   └── <album-id>/          # Album audio directories
│       ├── <track-file>.mp3
│       ├── <track-file>.flac
│       └── ...
└── images/
    └── <image-id>           # Image files (jpg, png, etc.)
```

The catalog metadata (artists, albums, tracks) is stored in the SQLite catalog database.

## Building

### Standard Build

```bash
cargo build --release
```

### Development Builds with Features

For faster development iteration, use feature flags to skip expensive operations:

```bash
# Skip catalog integrity checks (faster startup)
cargo build --features no_checks

# Fastest for development (skips integrity checks)
cargo build --features fast

# Add artificial slowdown for testing (useful for frontend development)
cargo build --features slowdown
```

## Docker Build

The Docker image includes both the pezzottify server and web frontend. A wrapper script handles git version detection since Docker builds don't have access to the full git repository.

### Using the Build Script (Recommended)

```bash
# From repository root
./build-docker.sh pezzottify-server        # Build and start
./build-docker.sh -d pezzottify-server     # Detached mode
```

The script:

1. Detects git commit hash on the host
2. Detects dirty state (uncommitted changes)
3. Passes these as build args to Docker
4. Runs `docker-compose up --build`

### Manual Build

If you need to build manually:

```bash
GIT_HASH=$(git rev-parse --short HEAD) \
GIT_DIRTY=$(git status --porcelain | grep -q . && echo 1 || echo 0) \
docker-compose up --build pezzottify-server
```

### Version in Docker Image

The server version will show:

- `v0.5.0-abc1234` for clean builds (commit hash)
- `v0.5.0-abc1234-dirty` for builds with uncommitted changes

## Running the Server

### Using Config File (Recommended)

```bash
# Create config from example
cp config.example.toml config.toml

# Edit config.toml to set your paths
# Then run:
cargo run --release -- --config ./config.toml
```

### Using CLI Arguments

```bash
cargo run --release -- --db-dir /path/to/db-dir --media-path /path/to/media
```

### Example with CLI Arguments

```bash
cargo run --release -- \
  --db-dir /path/to/db-dir \
  --media-path /path/to/media \
  --port 3001 \
  --content-cache-age-sec 60 \
  --logging-level path
```

### Development Example (Fast Build)

```bash
cargo run --features fast -- \
  --db-dir ../../pezzottify-catalog \
  --media-path ../../pezzottify-catalog \
  --content-cache-age-sec 60 \
  --logging-level path
```

### Serving Static Frontend

To serve the web frontend from the server:

```bash
cargo run --release -- \
  --db-dir /path/to/db-dir \
  --frontend-dir-path /path/to/web/dist
```

## Command-line Arguments

### Configuration Options

| Argument                             | Default       | Description                                                        |
| ------------------------------------ | ------------- | ------------------------------------------------------------------ |
| `--config <PATH>`                    | None          | Path to TOML configuration file. Values override CLI arguments.    |
| `--db-dir <PATH>`                    | None          | Directory containing database files (catalog.db, user.db)          |
| `--media-path <PATH>`                | Same as db-dir| Path to media files (audio/images)                                 |
| `--port <PORT>`                      | `3001`        | Server port to bind to                                             |
| `--metrics-port <PORT>`              | `9091`        | Metrics server port (Prometheus scraping)                          |
| `--logging-level <LEVEL>`            | `path`        | Request logging level (`none`, `path`, `headers`, `body`)          |
| `--content-cache-age-sec <SECONDS>`  | `3600`        | HTTP cache duration in seconds                                     |
| `--frontend-dir-path <PATH>`         | None          | Serve static frontend files from this path                         |
| `--downloader-url <URL>`             | None          | URL of the downloader service for fetching missing content         |
| `--downloader-timeout-sec <SECONDS>` | `300`         | Timeout in seconds for downloader requests                         |
| `--event-retention-days <DAYS>`      | `30`          | Number of days to retain sync events before pruning (0 to disable) |
| `--prune-interval-hours <HOURS>`     | `24`          | Interval in hours between pruning runs                             |

### TOML Configuration

The server can be configured via a TOML file. TOML values override CLI arguments. See `config.example.toml` for all available options.

Example `config.toml`:

```toml
db_dir = "/data/db"
media_path = "/data/media"
port = 3001
logging_level = "path"

# Enable download request queue
[download_manager]
max_albums_per_hour = 10
max_albums_per_day = 60
```

### Configuration Precedence

1. TOML config file values (highest priority)
2. CLI arguments
3. Default values (lowest priority)

### Environment Variables

- `LOG_LEVEL`: Set log level (default: `INFO`). Options: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`

### Metadata Enrichment

Metadata enrichment stores queryable, generated facts in `enrichment.db`. The background job `metadata_enrichment_v1` claims manual/admin queue rows first; if none are claimable, it seeds missing or stale artist, album, and track work from all-time listening counts. Page impressions and listening events keep recording analytics and plays, but they do not enqueue enrichment work directly. The job reuses the existing `[agent]` / `[agent.llm]` configuration to produce strict JSON for the typed tables.

There is no metadata-specific model/provider configuration. Enable the existing agent LLM before expecting queued work to complete:

```toml
[agent]
enabled = true

[agent.llm]
# Use "openai" for OpenAI-compatible APIs, including Simple-AI deployments that expose /v1/chat/completions.
provider = "openai"
base_url = "http://simple-ai:8000/v1"
model = "your-chat-model"
temperature = 0.2
timeout_secs = 120

[background_jobs.metadata_enrichment]
interval_hours = 6
batch_size = 2000
retry_after_secs = 21600
```

`[background_jobs.metadata_enrichment]` only controls scheduling, batch size, and retry delay. Manual admin triggers accept optional run parameters: `batch_size` and `entity_types` (`artist`, `album`, `track`); `entity_types` limits both listening backfill seeding and processing for that run. If `agent.enabled` is false, the job leaves claimed items queued with a retry delay and the last error `agent LLM is disabled`.

See [docs/metadata-enrichment-v1.md](../docs/metadata-enrichment-v1.md) for the table model, queue behavior, and operational notes.

## Build Features

Configure build-time behavior with Cargo features:

| Feature     | Description                                                        |
| ----------- | ------------------------------------------------------------------ |
| `no_checks` | Skip expensive catalog integrity checks during load                |
| `fast`      | Alias for `no_checks` (fastest for development builds)             |
| `slowdown`  | Adds artificial request delay for frontend development testing     |

## Configuration

### Rate Limits

The server implements per-endpoint rate limiting (configured in `server/http_layers/rate_limit.rs`):

**Per Minute:**

- **Login**: 10 requests/minute per IP
- **Stream**: 100 requests/minute per user
- **Content Read**: 500 requests/minute per user
- **Write Operations**: 60 requests/minute per user
- **Search**: 100 requests/minute per user
- **Global**: 1000 requests/minute per user

**Per Hour:**

- **Login**: 100 requests/hour per IP
- **Stream**: 5000 requests/hour per user
- **Content Read**: 25000 requests/hour per user
- **Write Operations**: 2000 requests/hour per user
- **Search**: 5000 requests/hour per user
- **Global**: 50000 requests/hour per user

### HTTP Caching

Static content (catalog data, images, audio) is cached using HTTP `Cache-Control` headers:

- Configurable via `--content-cache-age-sec`
- Default: 1 hour (3600 seconds)
- Useful for development: `--content-cache-age-sec 60` (1 minute)

## API Endpoints

### Authentication (`/v1/auth`)

| Method | Endpoint     | Auth | Description                                   |
| ------ | ------------ | ---- | --------------------------------------------- |
| POST   | `/login`     | No   | Login with credentials, returns session token |
| GET    | `/logout`    | Yes  | Logout and invalidate session token           |
| GET    | `/session`   | Yes  | Get current session info                      |
| GET    | `/challenge` | No   | Get authentication challenge                  |
| POST   | `/challenge` | No   | Submit authentication challenge response      |

#### Login Request Body

```json
{
  "user_handle": "username",
  "password": "password",
  "device_uuid": "unique-device-identifier",
  "device_type": "web|android|ios",
  "device_name": "Chrome Browser", // optional
  "os_info": "Windows 11" // optional
}
```

**Device fields:**

- `device_uuid`: Unique identifier for the device (8-64 characters). Should be generated once and persisted on the client.
- `device_type`: One of `web`, `android`, or `unknown`
- `device_name`: Human-readable device name (max 100 characters)
- `os_info`: Operating system information (max 200 characters)

Devices are tracked per-user with a limit of 50 devices. Oldest devices are automatically pruned when the limit is exceeded.

### Catalog Content (`/v1/content`)

All content endpoints require `AccessCatalog` permission.

| Method | Endpoint                   | Description                                         |
| ------ | -------------------------- | --------------------------------------------------- |
| GET    | `/artist/{id}`             | Get artist by ID, with `enrichment_status` when available |
| GET    | `/artist/{id}/discography` | Get artist's album IDs                              |
| GET    | `/album/{id}`              | Get album by ID, with `enrichment_status` when available |
| GET    | `/album/{id}/resolved`     | Get album with resolved artist references and `enrichment_status` |
| GET    | `/track/{id}`              | Get track by ID, with `enrichment_status` when available |
| GET    | `/track/{id}/resolved`     | Get track with resolved album/artist references and `enrichment_status` |
| GET    | `/image/{id}`              | Get image file                                      |
| GET    | `/stream/{id}`             | Stream audio file (supports range requests)         |
| GET    | `/whatsnew`                | Get recently added content                          |
| GET    | `/popular`                 | Get popular albums and artists based on listening   |
| POST   | `/search`                  | Search catalog (requires search feature enabled)    |

### User Content (`/v1/user`)

#### Liked Content

Requires `LikeContent` permission.

| Method | Endpoint                             | Description                                                  |
| ------ | ------------------------------------ | ------------------------------------------------------------ |
| GET    | `/liked/{content_type}`              | Get liked content (content_type: `album`, `artist`, `track`) |
| POST   | `/liked/{content_type}/{content_id}` | Like content                                                 |
| DELETE | `/liked/{content_type}/{content_id}` | Unlike content                                               |

#### Playlists

Requires `OwnPlaylists` permission.

| Method | Endpoint                | Description                        |
| ------ | ----------------------- | ---------------------------------- |
| GET    | `/playlists`            | Get user's playlists               |
| GET    | `/playlist/{id}`        | Get playlist by ID                 |
| POST   | `/playlist`             | Create new playlist                |
| PUT    | `/playlist/{id}`        | Update playlist name and/or tracks |
| DELETE | `/playlist/{id}`        | Delete playlist                    |
| PUT    | `/playlist/{id}/add`    | Add tracks to playlist             |
| PUT    | `/playlist/{id}/remove` | Remove tracks from playlist        |

#### Listening Stats

Requires `AccessCatalog` permission.

| Method | Endpoint             | Description                  |
| ------ | -------------------- | ---------------------------- |
| POST   | `/listening`         | Record a listening event; completed plays enqueue track plus adjacent album/artists for metadata enrichment |
| POST   | `/impression`        | Record an artist/album/track page view and enqueue that entity for metadata enrichment |
| GET    | `/listening/summary` | Get user's listening summary |
| GET    | `/listening/history` | Get user's listening history |
| GET    | `/listening/events`  | Get user's listening events  |

#### Settings

Requires authentication.

| Method | Endpoint    | Description          |
| ------ | ----------- | -------------------- |
| GET    | `/settings` | Get user settings    |
| PUT    | `/settings` | Update user settings |

### Sync (`/v1/sync`)

Requires authentication.

| Method | Endpoint  | Description                     |
| ------ | --------- | ------------------------------- |
| GET    | `/state`  | Get sync state                  |
| GET    | `/events` | Get sync events since last sync |

### Catalog Edit (`/v1/content`)

Requires `EditCatalog` permission.

| Method | Endpoint       | Description       |
| ------ | -------------- | ----------------- |
| POST   | `/artist`      | Create new artist |
| PUT    | `/artist/{id}` | Update artist     |
| DELETE | `/artist/{id}` | Delete artist     |
| POST   | `/album`       | Create new album  |
| PUT    | `/album/{id}`  | Update album      |
| DELETE | `/album/{id}`  | Delete album      |
| POST   | `/track`       | Create new track  |
| PUT    | `/track/{id}`  | Update track      |
| DELETE | `/track/{id}`  | Delete track      |
| POST   | `/image`       | Create new image  |
| PUT    | `/image/{id}`  | Update image      |
| DELETE | `/image/{id}`  | Delete image      |

### Admin (`/v1/admin`)

#### Server Management

Requires `ServerAdmin` permission.

| Method | Endpoint  | Description       |
| ------ | --------- | ----------------- |
| POST   | `/reboot` | Reboot the server |

#### User Management

Requires `ManagePermissions` permission.

| Method | Endpoint                            | Description                  |
| ------ | ----------------------------------- | ---------------------------- |
| GET    | `/users`                            | List all users               |
| GET    | `/users/{user_handle}/roles`        | Get user's roles             |
| POST   | `/users/{user_handle}/roles`        | Add role to user             |
| DELETE | `/users/{user_handle}/roles/{role}` | Remove role from user        |
| GET    | `/users/{user_handle}/permissions`  | Get user's permissions       |
| POST   | `/users/{user_handle}/permissions`  | Add extra permission to user |
| DELETE | `/permissions/{permission_id}`      | Remove extra permission      |

#### Bandwidth Analytics

Requires `ViewAnalytics` permission.

| Method | Endpoint                                 | Description                  |
| ------ | ---------------------------------------- | ---------------------------- |
| GET    | `/bandwidth/summary`                     | Get bandwidth summary        |
| GET    | `/bandwidth/usage`                       | Get bandwidth usage details  |
| GET    | `/bandwidth/users/{user_handle}/summary` | Get user's bandwidth summary |
| GET    | `/bandwidth/users/{user_handle}/usage`   | Get user's bandwidth usage   |

#### Listening Analytics

Requires `ViewAnalytics` permission.

| Method | Endpoint                                 | Description                    |
| ------ | ---------------------------------------- | ------------------------------ |
| GET    | `/listening/daily`                       | Get daily listening stats      |
| GET    | `/listening/top-tracks`                  | Get top tracks                 |
| GET    | `/listening/track/{track_id}`            | Get track listening stats      |
| GET    | `/listening/users/{user_handle}/summary` | Get user's listening summary   |
| GET    | `/online-users`                          | Get currently connected users  |

#### Background Jobs

Requires `ServerAdmin` permission.

| Method | Endpoint                         | Description                                   |
| ------ | -------------------------------- | --------------------------------------------- |
| GET    | `/jobs`                          | List registered background jobs               |
| GET    | `/jobs/{job_id}`                 | Get job status and schedule details           |
| POST   | `/jobs/{job_id}/trigger`         | Trigger a job manually, optionally with JSON params |
| GET    | `/jobs/{job_id}/history`         | Get recent job audit history                  |

`metadata_enrichment_v1` supports trigger params such as `{"batch_size": 10, "entity_types": ["artist", "track"]}`.

#### Changelog Management

Requires `EditCatalog` permission.

| Method | Endpoint                                      | Description                 |
| ------ | --------------------------------------------- | --------------------------- |
| POST   | `/changelog/batch`                            | Create changelog batch      |
| GET    | `/changelog/batches`                          | List changelog batches      |
| GET    | `/changelog/batch/{batch_id}`                 | Get changelog batch details |
| GET    | `/changelog/batch/{batch_id}/changes`         | Get changelog batch changes |
| POST   | `/changelog/batch/{batch_id}/close`           | Close changelog batch       |
| DELETE | `/changelog/batch/{batch_id}`                 | Delete changelog batch      |
| GET    | `/changelog/entity/{entity_type}/{entity_id}` | Get entity change history   |

### WebSocket (`/v1/ws`)

| Method | Endpoint | Description                                |
| ------ | -------- | ------------------------------------------ |
| GET    | `/ws`    | WebSocket connection for real-time updates |

## Download Manager

The download manager enables users to request content from external music providers. It provides a queue-based system with rate limiting, retry logic, and comprehensive audit logging.

### Overview

- **Queue-based Downloads**: Album and discography requests are queued for background processing
- **Rate Limiting**: Per-user hourly/daily limits prevent abuse
- **Retry Logic**: Failed downloads are automatically retried with exponential backoff
- **Integrity Watchdog**: Periodic scans detect and repair missing content
- **Audit Trail**: All operations are logged for compliance and debugging

### Architecture

```
User Request → Rate Check → Queue → Background Processor → Downloader Service → Catalog
                                           ↓
                                      Audit Logger
```

The download manager uses a priority-based queue:
1. **Watchdog (Priority 1)**: Repair items from integrity scans
2. **User (Priority 2)**: User-requested downloads
3. **Expansion (Priority 3)**: Child items (tracks, images) spawned from album downloads

### Configuration

Enable the download request queue in `config.toml`:

```toml
[download_manager]
# Rate limiting
max_albums_per_hour = 10
max_albums_per_day = 60
user_max_requests_per_day = 100
user_max_queue_size = 200

# Stale detection
stale_in_progress_threshold_secs = 3600

# Retry settings
max_retries = 8
initial_backoff_secs = 60
max_backoff_secs = 86400
backoff_multiplier = 2.5

# Audit log
audit_log_retention_days = 90
```

| Option | Default | Description |
| ------ | ------- | ----------- |
| `enabled` | `true` | Enable/disable the download request queue |
| `max_albums_per_hour` | `10` | Maximum albums a user can request per hour |
| `max_albums_per_day` | `60` | Maximum albums a user can request per day |
| `user_max_requests_per_day` | `100` | Maximum requests a user can make per day |
| `user_max_queue_size` | `200` | Maximum items a user can have in the queue |
| `max_retries` | `8` | Maximum retry attempts for failed downloads |
| `initial_backoff_secs` | `60` | Initial delay before first retry |
| `max_backoff_secs` | `86400` | Maximum delay between retries (24 hours) |
| `backoff_multiplier` | `2.5` | Multiplier for exponential backoff |
| `stale_in_progress_threshold_secs` | `3600` | Time before in-progress items are flagged as stale |
| `audit_log_retention_days` | `90` | Days to retain audit log entries |

### User Endpoints (`/v1/download`)

Require `RequestContent` permission.

| Method | Endpoint | Description |
| ------ | -------- | ----------- |
| GET | `/limits` | Get user's rate limit status |
| GET | `/my-requests` | Get user's queued/recent requests |
| POST | `/request/album` | Request an album download |
| POST | `/request/discography` | Request an artist's discography |

#### Album Request

```bash
POST /v1/download/request/album
Content-Type: application/json

{
  "album_id": "external-album-id",
  "album_name": "The Dark Side of the Moon",
  "artist_name": "Pink Floyd"
}
```

### Admin Endpoints (`/v1/download/admin`)

Require `ViewAnalytics` or `EditCatalog` permission.

| Method | Endpoint | Permission | Description |
| ------ | -------- | ---------- | ----------- |
| GET | `/stats` | ViewAnalytics | Queue statistics and activity summary |
| GET | `/failed` | ViewAnalytics | List failed download items |
| GET | `/activity` | ViewAnalytics | Recent download activity log |
| GET | `/requests` | ViewAnalytics | All queued/recent requests |
| POST | `/retry/{id}` | EditCatalog | Retry a failed download |
| GET | `/audit` | ViewAnalytics | Query audit log |
| GET | `/audit/item/{id}` | ViewAnalytics | Audit history for a queue item |
| GET | `/audit/user/{user_id}` | ViewAnalytics | Audit history for a user |

### Error Handling

Downloads may fail with different error types:

| Error Type | Retryable | Description |
| ---------- | --------- | ----------- |
| `not_found` | No | Content doesn't exist on provider |
| `connection` | Yes | Network connectivity issues |
| `timeout` | Yes | Request timed out |
| `rate_limited` | Yes | Provider rate limit hit |
| `parse_error` | No | Invalid response from provider |
| `internal` | Yes | Internal server error |

Non-retryable errors immediately mark the item as `FAILED`. Retryable errors trigger exponential backoff retries up to `max_retries`.

### Prometheus Metrics

| Metric | Type | Description |
| ------ | ---- | ----------- |
| `pezzottify_download_queue_size` | Gauge | Queue size by status and priority |
| `pezzottify_download_processed_total` | Counter | Processed downloads by type and result |
| `pezzottify_download_processing_duration_seconds` | Histogram | Download processing duration |
| `pezzottify_download_capacity_used` | Gauge | Capacity usage by period |
| `pezzottify_download_user_requests_total` | Counter | User requests by type |
| `pezzottify_download_audit_events_total` | Counter | Audit events by type |
| `pezzottify_download_queue_stale_in_progress` | Gauge | Stale in-progress items |

## Authentication & Authorization

### Authentication Flow

1. Client sends credentials and device info to `POST /v1/auth/login`
2. Server validates device info (UUID format, field lengths)
3. Server validates password using Argon2
4. Server registers/updates device and associates it with the user
5. Server enforces per-user device limit (50 devices max)
6. Server generates auth token linked to the device
7. Token returned in response body and set as HTTP-only cookie
8. Client includes cookie in subsequent requests
9. Session middleware validates token and extracts user permissions + device info
10. Permission middleware checks required permissions for each route

### User Roles

- **Admin**: Administrative access (does not include user features like liking content or playlists)

  - AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin, ViewAnalytics, RequestContent

- **Regular**: Standard user access
  - AccessCatalog, LikeContent, OwnPlaylists

### Permission System

Permissions can be granted via:

- **Role-based**: Permissions inherited from user role
- **Extra grants**: Temporary or counted permission grants (future use)

## CLI Auth Tool

The `cli-auth` binary provides user and authentication management.

### Build and Run

```bash
cargo build --release --bin cli-auth

# Using config file
./target/release/cli-auth --config /path/to/config.toml

# Using db-dir
./target/release/cli-auth --db-dir /path/to/db-dir

# Legacy: direct path to user.db (backward compatible)
./target/release/cli-auth /path/to/user.db
```

### Available Commands

#### User Management

```bash
# Create a new user
add-user <user_handle>

# Show user information
show <user_handle>

# List all user handles
user-handles
```

#### Password Management

```bash
# Set initial password (fails if password already exists)
add-login <user_handle> <password>

# Change existing password
update-login <user_handle> <password>

# Remove password authentication
delete-login <user_handle>

# Verify password without creating token
check-password <user_handle> <password>
```

#### Role Management

```bash
# List available roles and permissions
list-roles

# Add role to user
add-role <user_handle> <role>

# Remove role from user
remove-role <user_handle> <role>
```

#### Utility Commands

```bash
# Show the path of the current auth database
where

# Show available commands
help

# Exit the CLI
exit
```

### Example Workflow

```bash
# Start the CLI tool (using db-dir)
cargo run --bin cli-auth -- --db-dir /path/to/db-dir

# Create a new admin user
> add-user admin
> add-login admin secure_password123
> add-role admin Admin

# Create a regular user
> add-user john
> add-login john password456
> add-role john Regular

# Verify setup
> show admin
> show john
```

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test

```bash
cargo test <test_name>
```

### Test Coverage Areas

- Route authentication/authorization
- User store operations
- Catalog loading and validation
- Permission system
- Session management

## Development Tips

### Faster Development Iteration

1. **Use the `fast` feature** for quick rebuilds:

   ```bash
   cargo run --features fast -- --db-dir /path/to/db
   ```

2. **Use shorter cache times** for frontend development:

   ```bash
   cargo run -- --db-dir /path/to/db --content-cache-age-sec 60
   ```

3. **Use `slowdown` feature** to test loading states in frontend:
   ```bash
   cargo run --features slowdown -- --db-dir /path/to/db
   ```

### Debugging

Enable detailed logging:

```bash
LOG_LEVEL=DEBUG cargo run -- --db-dir /path/to/db --logging-level body
```

Log levels:

- `TRACE`: Very detailed logging
- `DEBUG`: Debug information
- `INFO`: General information (default)
- `WARN`: Warnings only
- `ERROR`: Errors only

### Database Schema

The SQLite database is automatically initialized with the required schema on first run. Schema migrations are handled via the `VersionedSchema` system in `sqlite_persistence/`.

To inspect the database:

```bash
sqlite3 /path/to/user.db
.schema
```

### Project Structure

```text
pezzottify-server/
├── src/
│   ├── main.rs              # Server startup, store wiring, background job registration
│   ├── lib.rs               # Library exports for tests and companion binaries
│   ├── cli_auth.rs          # User/auth management CLI
│   ├── agent/               # LLM providers and workflow helpers
│   ├── background_jobs/     # Scheduler, job audit log, scheduled/manual jobs
│   ├── catalog_store/       # Catalog models, schema, validation, SQLite store
│   ├── config/              # CLI/TOML config parsing and defaults
│   ├── download_manager/    # User request queue, audit log, downloader integration
│   ├── enrichment_store/    # Audio features, metadata enrichment v1 tables, queue state
│   ├── related_artists/     # MusicBrainz/Last.fm related artist lookup
│   ├── search/              # Search vaults, streaming search, organic index helpers
│   ├── server/              # Axum routes, state, middleware, metrics, WebSocket support
│   ├── server_store/        # Operational state such as job history and server metadata
│   ├── shows/               # AI show scripts and generated show media
│   ├── sqlite_persistence/  # Versioned schema migration helpers
│   └── user/                # Users, permissions, auth tokens, settings, sync events
├── tests/                   # E2E HTTP tests and shared fixtures
├── Cargo.toml
├── config.example.toml
└── README.md
```

## Monitoring & Alerting

The server exposes Prometheus metrics on a separate metrics port, default `9091`. In the local Docker setup this is intended for internal scraping rather than direct browser use.

All custom metrics use the `pezzottify_` prefix for easy filtering. Important metric families include:

| Metric | Type | Description |
| ------ | ---- | ----------- |
| `pezzottify_http_requests_total` | Counter | Total HTTP requests by method, path, and status |
| `pezzottify_http_request_duration_seconds` | Histogram | Request duration by method and path |
| `pezzottify_auth_login_attempts_total` | Counter | Login attempts by status |
| `pezzottify_rate_limit_hits_total` | Counter | Rate limit violations by endpoint |
| `pezzottify_db_query_duration_seconds` | Histogram | Database query duration by operation |
| `pezzottify_catalog_items_total` | Gauge | Catalog items by entity type |
| `pezzottify_errors_total` | Counter | Server errors by type and endpoint |
| `pezzottify_bandwidth_bytes_total` | Counter | Bytes transferred by user and endpoint category |
| `pezzottify_listening_events_total` | Counter | Listening events by client type and completion |
| `pezzottify_downloader_requests_total` | Counter | Downloader requests by operation and status |
| `pezzottify_downloader_errors_total` | Counter | Downloader errors by operation and type |

The production monitoring stack, including Prometheus, Grafana, Alertmanager, and notification wiring, is maintained in the [homelab](https://github.com/lelloman/homelab) deployment repository.
