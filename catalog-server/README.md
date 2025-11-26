# Pezzottify Catalog Server

A high-performance Rust backend server for the Pezzottify music streaming platform. Handles music catalog management, user authentication, audio streaming, and search functionality.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Catalog Directory Structure](#catalog-directory-structure)
- [Building](#building)
- [Running the Server](#running-the-server)
- [Command-line Arguments](#command-line-arguments)
- [Build Features](#build-features)
- [Configuration](#configuration)
- [API Endpoints](#api-endpoints)
- [Authentication & Authorization](#authentication--authorization)
- [CLI Auth Tool](#cli-auth-tool)
- [Testing](#testing)
- [Development Tips](#development-tips)

## Overview

The catalog server is the backend component of Pezzottify that provides:

- **Music Catalog Management**: Loads and serves music metadata from JSON files
- **Audio Streaming**: HTTP range request support for efficient audio playback
- **User Authentication**: Token-based authentication with Argon2 password hashing
- **Authorization**: Role-based permissions system (Admin/Regular users)
- **Search**: Full-text search across artists, albums, and tracks
- **User Content**: Playlists and liked content management
- **Rate Limiting**: Per-endpoint rate limiting to prevent abuse

## Architecture

### Core Modules

- **`catalog/`**: Music catalog management
  - Loads artists, albums, and tracks from JSON files
  - Validates entity references (artist IDs, album IDs, image IDs)
  - Resolves relationships between tracks, albums, and artists
  - In-memory storage with HashMap-based lookups

- **`user/`**: Authentication and authorization
  - `SqliteUserStore`: User persistence in SQLite database
  - `UserManager`: Authentication with Argon2 password hashing and RSA token signing
  - `permissions.rs`: Role-based permissions (Admin, Regular)
  - `auth.rs`: Token-based authentication with RSA signing

- **`server/`**: Axum-based HTTP server
  - `session.rs`: Session management via HTTP-only cookies
  - `stream_track.rs`: Audio streaming with range request support
  - `search.rs`: Search API routes
  - `http_layers/`: Middleware for logging, caching, rate limiting, optional slowdown

- **`search/`**: Search functionality
  - `PezzotHashSearchVault`: Custom full-text search implementation
  - `NoOpSearchVault`: Disabled search stub (for `no_search` feature)

- **`sqlite_persistence/`**: Database schema management
  - `versioned_schema.rs`: Schema migrations with version tracking

### Key Types

- **`Catalog`**: In-memory catalog with HashMaps for artists, albums, tracks
- **`Dirs`**: Catalog directory paths (root, albums, artists, images)
- **`Session`**: Request session containing user ID, token, and permissions
- **`Permission`**: Enum for access control:
  - `AccessCatalog`: View catalog content
  - `LikeContent`: Like/unlike content
  - `OwnPlaylists`: Create and manage playlists
  - `EditCatalog`: Modify catalog (not yet implemented)
  - `ManagePermissions`: Manage user permissions (not yet implemented)
  - `IssueContentDownload`: Issue download tokens (not yet implemented)
  - `RebootServer`: Reboot server (not yet implemented)
- **`UserRole`**: Admin (all permissions) or Regular (basic permissions)

## Prerequisites

- **Rust**: Latest stable toolchain (install via [rustup](https://rustup.rs/))
- **ffprobe** (optional): Required for `--check-all` catalog validation
- **SQLite**: Bundled via rusqlite (no separate installation needed)

## Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd pezzottify/catalog-server
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

## Catalog Directory Structure

The server expects a catalog directory with the following structure:

```
<catalog-root>/
├── albums/
│   ├── album_<id>.json      # Album metadata files
│   └── album_<id>/          # Album audio directories
│       ├── <track-file>.mp3
│       ├── <track-file>.flac
│       └── ...
├── artists/
│   └── artist_<id>.json     # Artist metadata files
└── images/
    └── <image-id>           # Image files (jpg, png, etc.)
```

### Metadata File Formats

**Artist JSON** (`artist_<id>.json`):
```json
{
  "id": "a123",
  "name": "Artist Name",
  "images": [
    {"id": "image123", "width": 500, "height": 500}
  ]
}
```

**Album JSON** (`album_<id>.json`):
```json
{
  "id": "b123",
  "name": "Album Name",
  "artist_id": "a123",
  "release_date": "2024-01-01",
  "images": [
    {"id": "image456", "width": 1000, "height": 1000}
  ],
  "discs": [
    {
      "number": 1,
      "tracks": [
        {
          "id": "t123",
          "name": "Track Name",
          "track_number": 1,
          "disc_number": 1,
          "duration_ms": 180000,
          "artists": [
            {"id": "a123", "role": "Main"}
          ],
          "file": "01-track-name.mp3"
        }
      ]
    }
  ]
}
```

## Building

### Standard Build

```bash
cargo build --release
```

### Development Builds with Features

For faster development iteration, use feature flags to skip expensive operations:

```bash
# Skip search index building (faster startup)
cargo build --features no_search

# Skip catalog integrity checks (faster startup)
cargo build --features no_checks

# Skip both (fastest for development)
cargo build --features fast

# Add artificial slowdown for testing (useful for frontend development)
cargo build --features slowdown
```

## Running the Server

### Basic Usage

```bash
cargo run --release -- <catalog-path> <user-db-path>
```

### Example

```bash
cargo run --release -- \
  /path/to/pezzottify-catalog \
  /path/to/user.db \
  --port 3001 \
  --content-cache-age-sec 60 \
  --logging-level path
```

### Development Example (Fast Build)

```bash
cargo run --features fast -- \
  ../../pezzottify-catalog \
  ../../test.db \
  --content-cache-age-sec 60 \
  --logging-level path
```

### Serving Static Frontend

To serve the web frontend from the server:

```bash
cargo run --release -- \
  /path/to/catalog \
  /path/to/user.db \
  --frontend-dir-path /path/to/web/dist
```

## Command-line Arguments

### Required Arguments

- `<catalog-path>`: Path to the catalog directory
- `<user-db-path>`: Path to the SQLite database file for user storage

### Optional Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--port <PORT>` | `3001` | Server port to bind to |
| `--logging-level <LEVEL>` | `path` | Request logging level (`path`, `full`, `none`) |
| `--content-cache-age-sec <SECONDS>` | `3600` | HTTP cache duration in seconds |
| `--frontend-dir-path <PATH>` | None | Serve static frontend files from this path |
| `--check-only` | false | Validate catalog without starting the server |
| `--check-all` | false | Perform full catalog validation including ffprobe checks |

### Environment Variables

- `LOG_LEVEL`: Set log level (default: `INFO`). Options: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`

## Build Features

Configure build-time behavior with Cargo features:

| Feature | Description |
|---------|-------------|
| `no_search` | Disable search index building (faster builds, no search functionality) |
| `no_checks` | Skip expensive catalog integrity checks during load |
| `fast` | Combines `no_search` and `no_checks` for fastest development builds |
| `slowdown` | Adds artificial request delay for frontend development testing |

## Configuration

### Rate Limits

The server implements per-endpoint rate limiting (configured in `server/mod.rs`):

- **Login**: 5 requests/minute per IP
- **Stream**: 60 requests/minute per user/IP
- **Content Read**: 120 requests/minute per user/IP
- **Write Operations**: 30 requests/minute per user/IP
- **Search**: 30 requests/minute per user/IP
- **Global**: 200 requests/minute per user/IP

### HTTP Caching

Static content (catalog data, images, audio) is cached using HTTP `Cache-Control` headers:
- Configurable via `--content-cache-age-sec`
- Default: 1 hour (3600 seconds)
- Useful for development: `--content-cache-age-sec 60` (1 minute)

## API Endpoints

### Authentication (`/v1/auth`)

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/login` | No | Login with credentials, returns session token |
| GET | `/logout` | Yes | Logout and invalidate session token |

### Catalog Content (`/v1/content`)

All content endpoints require `AccessCatalog` permission.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/artist/{id}` | Get artist by ID |
| GET | `/artist/{id}/discography` | Get artist's album IDs |
| GET | `/album/{id}` | Get album by ID |
| GET | `/album/{id}/resolved` | Get album with resolved artist references |
| GET | `/track/{id}` | Get track by ID |
| GET | `/track/{id}/resolved` | Get track with resolved album and artist references |
| GET | `/image/{id}` | Get image file |
| GET | `/stream/{id}` | Stream audio file (supports range requests) |
| POST | `/search` | Search catalog (requires search feature enabled) |

### User Content (`/v1/user`)

#### Liked Content

Requires `LikeContent` permission.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/liked/{content_type}` | Get liked content (content_type: `album`, `artist`, `track`) |
| POST | `/liked/{content_id}` | Like content |
| DELETE | `/liked/{content_id}` | Unlike content |

#### Playlists

Requires `OwnPlaylists` permission.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/playlists` | Get user's playlists |
| GET | `/playlist/{id}` | Get playlist by ID |
| POST | `/playlist` | Create new playlist |
| PUT | `/playlist/{id}` | Update playlist name and/or tracks |
| DELETE | `/playlist/{id}` | Delete playlist |
| PUT | `/playlist/{id}/add` | Add tracks to playlist |
| PUT | `/playlist/{id}/remove` | Remove tracks from playlist |

## Authentication & Authorization

### Authentication Flow

1. Client sends credentials to `POST /v1/auth/login`
2. Server validates password using Argon2
3. Server generates signed auth token using RSA
4. Token returned in response body and set as HTTP-only cookie
5. Client includes cookie in subsequent requests
6. Session middleware validates token and extracts user permissions
7. Permission middleware checks required permissions for each route

### User Roles

- **Admin**: Full system access
  - AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, RebootServer

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
./target/release/cli-auth <user-db-path>
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

### Example Workflow

```bash
# Start the CLI tool
cargo run --bin cli-auth -- /path/to/user.db

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
   cargo run --features fast -- <catalog> <db>
   ```

2. **Use shorter cache times** for frontend development:
   ```bash
   cargo run -- <catalog> <db> --content-cache-age-sec 60
   ```

3. **Use `slowdown` feature** to test loading states in frontend:
   ```bash
   cargo run --features slowdown -- <catalog> <db>
   ```

### Validating Catalog Changes

Before starting the server, validate catalog integrity:

```bash
# Quick validation (structure and references only)
cargo run -- <catalog> <db> --check-only

# Full validation (includes ffprobe on all audio files)
cargo run -- <catalog> <db> --check-only --check-all
```

### Debugging

Enable detailed logging:

```bash
LOG_LEVEL=DEBUG cargo run -- <catalog> <db> --logging-level full
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

```
catalog-server/
├── src/
│   ├── main.rs              # Main server entry point
│   ├── cli_auth.rs          # CLI auth tool entry point
│   ├── catalog/             # Catalog models and loading
│   │   ├── mod.rs
│   │   ├── catalog.rs       # Core catalog type
│   │   ├── load.rs          # Catalog loading logic
│   │   ├── artist.rs
│   │   ├── album.rs
│   │   ├── track.rs
│   │   └── image.rs
│   ├── server/              # HTTP server
│   │   ├── mod.rs
│   │   ├── server.rs        # Route handlers
│   │   ├── config.rs
│   │   ├── state.rs
│   │   ├── session.rs       # Session middleware
│   │   ├── stream_track.rs  # Audio streaming
│   │   ├── search.rs        # Search routes
│   │   └── http_layers/     # Middleware
│   ├── user/                # Authentication & authorization
│   │   ├── mod.rs
│   │   ├── user_manager.rs
│   │   ├── user_store.rs
│   │   ├── sqlite_user_store.rs
│   │   ├── auth.rs
│   │   ├── permissions.rs
│   │   └── user_models.rs
│   ├── search/              # Search functionality
│   │   ├── mod.rs
│   │   ├── search_vault.rs
│   │   └── pezzott_hash.rs
│   └── sqlite_persistence/  # Database schema
│       ├── mod.rs
│       └── versioned_schema.rs
├── Cargo.toml
└── README.md
```

## Monitoring & Alerting

The catalog server includes a full monitoring stack with Prometheus metrics, Grafana dashboards, and Alertmanager for notifications.

### Quick Start

```bash
# From the repository root
docker-compose up -d
```

This starts:
- **catalog-server** on port 3001
- **Prometheus** on port 9090
- **Grafana** on port 3000
- **Alertmanager** on port 9093

### Accessing Grafana

1. Open http://localhost:3000
2. Login with username `admin` and password `admin`
3. Navigate to Dashboards to view the Pezzottify dashboard

### Prometheus Metrics

The server exposes metrics at `/metrics` endpoint. Available metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `http_requests_total` | Counter | Total HTTP requests by method, path, status |
| `http_request_duration_seconds` | Histogram | Request duration by method and path |
| `auth_login_attempts_total` | Counter | Login attempts by status (success/failure) |
| `auth_login_duration_seconds` | Histogram | Login request duration |
| `auth_active_sessions` | Gauge | Number of active sessions |
| `rate_limit_hits_total` | Counter | Rate limit violations by endpoint |
| `db_query_duration_seconds` | Histogram | Database query duration by operation |
| `db_connection_errors_total` | Counter | Database connection errors |
| `catalog_items_total` | Gauge | Catalog items by type (artist/album/track) |
| `process_memory_bytes` | Gauge | Process memory usage |

### Alert Rules

The following alerts are configured in `monitoring/alerts.yml`:

**Critical:**
- `ServiceDown` - Catalog server unreachable
- `LoginBruteForceAttempt` - Possible brute force attack on login
- `HighErrorRate` - High HTTP 5xx error rate
- `DatabaseErrors` - Database connection failures

**Warning:**
- `HighRateLimitViolations` - Excessive rate limiting
- `HighLoginFailureRate` - Many failed login attempts
- `SlowLoginPerformance` - Login latency above threshold
- `SlowDatabaseQueries` - Database queries taking too long
- `HighMemoryUsage` - Memory usage above 1GB

### Configuring Alertmanager

Edit `monitoring/alertmanager.yml` to configure notifications:

**Email notifications:**
```yaml
global:
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'your-email@gmail.com'
  smtp_auth_username: 'your-email@gmail.com'
  smtp_auth_password: 'your-app-password'  # Use Gmail app password
```

**Telegram notifications:**
Update the environment variables in `docker-compose.yml`:
```yaml
telegram-webhook:
  environment:
    - TELEGRAM_TOKEN=your_bot_token
    - TELEGRAM_ADMIN=your_chat_id
```

### Running Individual Services

```bash
# Start only the catalog server
docker-compose up -d catalog-server

# Start server with monitoring (no alerting)
docker-compose up -d catalog-server prometheus grafana

# Start the full stack
docker-compose up -d
```

### Viewing Metrics Directly

```bash
# Raw Prometheus metrics
curl http://localhost:3001/metrics

# Prometheus query interface
open http://localhost:9090

# Example PromQL queries
# Request rate: rate(http_requests_total[5m])
# Login failures: auth_login_attempts_total{status="failure"}
# P95 latency: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
```

## License

See the root project LICENSE file.

## Contributing

See the root project CONTRIBUTING file.
