# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Pezzottify is a music streaming platform with three main components:
- **catalog-server**: Rust backend server that manages music catalog, authentication, and streaming
- **web**: Vue 3 frontend web application
- **android**: Kotlin/Android mobile application

## Development Commands

### Catalog Server (Rust)

The catalog server uses a SQLite database for catalog metadata and a media directory for audio files and images.

**Running the server:**
```bash
cd catalog-server
# Using config file (recommended):
cargo run -- --config ./config.toml

# Using CLI arguments:
cargo run -- --db-dir /path/to/db-dir --media-path /path/to/media --port 3001
```

Example:
```bash
cargo run -- --db-dir ../../pezzottify-catalog --media-path=../../pezzottify-catalog --content-cache-age-sec=60 --logging-level path
```

**Build features:**
- `--features no_search`: Faster builds without search index
- `--features fast`: Alias for no_search (fastest for development)
- `--features slowdown`: Adds slowdown layer for testing

**CLI arguments:**
- `--config <PATH>`: Path to TOML config file (values override CLI arguments)
- `--db-dir <PATH>`: Directory containing database files (catalog.db, user.db, server.db)
- `--media-path <PATH>`: Path to media files (audio/images), defaults to db-dir
- `--port <PORT>`: Server port (default: 3001)
- `--metrics-port <PORT>`: Metrics server port (default: 9091)
- `--logging-level <LEVEL>`: Request logging level (default: path)
- `--content-cache-age-sec <SECONDS>`: HTTP cache duration (default: 3600)
- `--frontend-dir-path <PATH>`: Serve static frontend files
- `--downloader-url <URL>`: URL of the downloader service for fetching missing content (optional)
- `--downloader-timeout-sec <SECONDS>`: Timeout for downloader requests (default: 300)
- `--event-retention-days <DAYS>`: Days to retain sync events before pruning (default: 30, 0 to disable)
- `--prune-interval-hours <HOURS>`: Interval between pruning runs (default: 24)
- `--ssl-cert <PATH>`: Path to SSL certificate file (PEM format). Requires `--ssl-key`.
- `--ssl-key <PATH>`: Path to SSL private key file (PEM format). Requires `--ssl-cert`.

**Running tests:**
```bash
cd catalog-server
cargo test
```

**Running specific test:**
```bash
cd catalog-server
cargo test <test_name>
```

**Additional binaries:**
- `cli-auth`: Authentication CLI tool

**Docker build:**
```bash
./build-docker.sh catalog-server        # Build and start with correct version info
./build-docker.sh -d catalog-server     # Detached mode
```

The wrapper script detects git hash and dirty state on the host and passes them to Docker. This is necessary because Docker builds don't have access to the full git repo.

**SSL/TLS Configuration:**

The server supports HTTPS with TLS using self-signed or CA-signed certificates.

Generate a self-signed certificate (valid for 365 days):
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost"
```

For a certificate with Subject Alternative Names (recommended for production):
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=yourdomain.com" \
  -addext "subjectAltName=DNS:yourdomain.com,DNS:localhost,IP:127.0.0.1"
```

Configure SSL via CLI:
```bash
cargo run -- --db-dir /path/to/db --ssl-cert /path/to/cert.pem --ssl-key /path/to/key.pem
```

Or via config.toml:
```toml
[ssl]
cert_path = "/path/to/cert.pem"
key_path = "/path/to/key.pem"
```

Notes:
- Both `cert_path` and `key_path` are required when enabling SSL
- Certificate and key must be in PEM format
- The metrics endpoint remains HTTP-only (internal use)
- For Android clients: pin the certificate in the app for enhanced security
- For web browsers: users will see a security warning for self-signed certs (click through once)

### Web Frontend (Vue 3)

```bash
cd web
npm install                    # Install dependencies
npm run dev                   # Development server with hot reload
npm run build                 # Production build
npm run preview               # Preview production build
npm run lint                  # Lint with ESLint
npm run format                # Format with Prettier
```

### Android

```bash
cd android
./gradlew build               # Build all modules
./gradlew test                # Run unit tests
./gradlew assembleDebug       # Build debug APK
./run-integration-tests.sh    # Run integration tests (requires Docker)
```

The Android project uses a multi-module Gradle setup with modules: `app`, `ui`, `domain`, `localdata`, `logger`, `player`, `remoteapi`, `debuginterface`.

**Integration tests:**
- Located in `remoteapi/src/integrationTest/`
- Require Docker to run (spins up test catalog-server instance)
- Run via `./run-integration-tests.sh` script
- Not included in `./gradlew test` to keep unit tests fast

## Architecture

### Catalog Server Architecture

**Core modules:**
- `catalog_store/`: SQLite-backed catalog management
  - `SqliteCatalogStore`: Main store with CRUD operations
  - `CatalogStore` trait: Abstract interface for catalog access
  - Validation for write operations (foreign keys, duplicates)
  - Transactional writes with `BEGIN IMMEDIATE`
- `user/`: Authentication and authorization
  - `SqliteUserStore`: User persistence in SQLite
  - `UserManager`: Authentication with Argon2 password hashing
  - `permissions.rs`: Role-based permissions (Admin, Regular)
  - `auth.rs`: Token-based authentication with RSA signing
  - `sync_events.rs`: Event types for multi-device sync
- `server/`: Axum HTTP server
  - `session.rs`: Session management via cookies
  - `stream_track.rs`: Audio streaming with range request support
  - `search.rs`: Search API routes
  - `websocket/`: WebSocket support for real-time updates
  - `http_layers/`: Middleware for logging, caching, slowdown
- `search/`: Search functionality
  - `PezzotHashSearchVault`: Full-text search implementation
  - `NoOpSearchVault`: Disabled search (for `no_search` feature)
- `sqlite_persistence/`: Database schema management
  - `versioned_schema.rs`: Schema migrations with version tracking
- `config/`: Configuration management
  - Combines CLI arguments and TOML file configuration
  - TOML values override CLI defaults
- `background_jobs/`: Scheduled background tasks
  - Job scheduler with configurable intervals
  - `PopularContentJob`: Computes popular content metrics
- `server_store/`: Server-level data persistence
  - `SqliteServerStore`: Stores server state (job history, etc.)
- `downloader/`: External content fetching (optional)
  - Integration with external downloader service for missing content

**Key types:**
- `SqliteCatalogStore`: SQLite-backed catalog with CRUD operations
- `CatalogStore`: Trait for catalog access (read and write)
- `Session`: Request session with user permissions
- `Permission`: Enum for access control (AccessCatalog, LikeContent, OwnPlaylists, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin, ViewAnalytics)
- `UserRole`: Admin or Regular with different permission sets

**Server routes structure:**
- `/v1/auth/*`: Login, logout, session management, challenge-response auth
  - `POST /login`, `GET /logout`, `GET /session`, `GET|POST /challenge`
- `/v1/content/*`: Catalog content and streaming
  - `GET /album/{id}`, `GET /album/{id}/resolved`
  - `GET /artist/{id}`, `GET /artist/{id}/discography`
  - `GET /track/{id}`, `GET /track/{id}/resolved`
  - `GET /image/{id}`, `GET /stream/{id}`
  - `GET /whatsnew`, `GET /popular`
  - `POST /search`: Full-text search (request body: `{ "query": "..." }`)
- `/v1/user/*`: User data and preferences
  - `GET /liked/{content_type}`, `PUT|DELETE /liked/{content_type}/{id}`
  - `GET /playlists`, `GET /playlist/{id}`, `POST /playlist`
  - `PUT|DELETE /playlist/{id}`, `PUT /playlist/{id}/add|remove`
  - `GET|PUT /settings`
  - `POST /listening`, `GET /listening/summary|history|events`
- `/v1/admin/*`: Administration (requires admin permissions)
  - User management: `GET|POST /users`, `DELETE /users/{handle}`, roles/permissions
  - Jobs: `GET /jobs`, `GET|POST /jobs/{id}`, `GET /jobs/{id}/history`
  - Changelog: `POST /changelog/batch`, `GET /changelog/batches`, etc.
  - Analytics: `GET /bandwidth/*`, `GET /listening/*`, `GET /online-users`
  - Catalog CRUD: `POST|PUT|DELETE /artist|album|track|image`
  - Server control: `POST /reboot`
- `/v1/sync/*`: Multi-device sync
  - `GET /state`, `GET /events`
- `/v1/ws`: WebSocket for real-time updates

**Authentication flow:**
1. User logs in with credentials (POST `/v1/auth/login`)
2. Server validates with Argon2, returns signed auth token
3. Token stored in HTTP-only cookie
4. Session middleware validates token on each request
5. Permission middleware checks required permissions

### Web Frontend Architecture

**Technology stack:**
- Vue 3 Composition API
- Vue Router for navigation
- Pinia for state management
- Axios for HTTP requests
- Howler.js for audio playback
- Vite build tool

**Store modules (Pinia):** Located in `web/src/store/`
- `auth.js`: Authentication state
- `player.js`: Audio playback state
- `remote.js`: API communication
- `user.js`: User data (playlists, liked content)
- `statics.js`: Static data caching
- `debug.js`: Debug configuration
- `sync.js`: Multi-device sync state management (WebSocket connection, sync events)

**Routing:**
All content routes nest under HomeView with `meta: { requiresAuth: true }`:
- `/search/:query?`: Search results
- `/track/:trackId`: Track details
- `/album/:albumId`: Album details
- `/artist/:artistId`: Artist details
- `/playlist/:playlistId`: Playlist details
- `/settings`: User settings page
- `/admin/*`: Admin panel (users, analytics, server)
- `/login`: Login page
- `/logout`: Logout handler

**Component structure:**
- `views/`: Page components (HomeView, LoginView)
- `components/content/`: Content display components
- `components/common/`: Reusable UI components
- `components/common/contextmenu/`: Right-click context menus
- `components/search/`: Search-related components
- `components/icons/`: Icon components

**Key features:**
- Authentication guard on router (redirects to `/login` if not authenticated)
- Global debug config accessible via `window.config`
- HTTP cache blocking for development
- Right-click context menu system

### Android Architecture

Multi-module Gradle project with clean architecture layers:
- `app`: Main application module
- `domain`: Business logic and use cases
- `ui`: UI components
- `localdata`: Local database and persistence
- `remoteapi`: Server communication
- `player`: Audio playback logic
- `logger`: Logging infrastructure
- `debuginterface`: Debug tools

## Important Implementation Notes

### Catalog Server

**User permissions system:**
- Permissions are checked via middleware functions in server.rs
- Each protected route has a `require_*` middleware (e.g., `require_access_catalog`)
- Permission grants can be role-based or temporary/counted extras
- Admin role has: AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin, ViewAnalytics
- Regular role has: AccessCatalog, LikeContent, OwnPlaylists

**Database operations:**
- SQLite used for user storage
- Schema managed via `VersionedSchema` in `sqlite_persistence/`
- Migrations tracked with version numbers
- Foreign keys enforced (cascade deletes for user content)
- All multi-step operations use transactions (role management, permission countdown, auth credentials, migrations)
- Single operations rely on SQLite's default atomicity

**Search indexing:**
- Built at startup from catalog database
- Can be disabled with `no_search` feature for faster dev builds
- Uses custom "PezzotHash" algorithm

**Catalog storage:**
- SQLite database stores all catalog metadata (artists, albums, tracks, images)
- Media files (audio, images) stored in filesystem at `--media-path`
- CRUD operations available via `CatalogStore` trait
- Write operations validated and transactional

### Web Frontend

**State management:**
- Pinia stores initialized before router
- `remoteStore` handles all API calls
- Player state manages Howler.js instances
- Authentication state persisted across page reloads

**API communication:**
- All requests through axios
- Base URL configurable (defaults to same origin)
- Session token handled via cookies (HTTP-only)
- Cache control via `blockHttpCache` debug flag

**Context menus:**
- Right-click behavior can be disabled via debug config
- Contextual menus implemented for tracks (TODO: albums, artists)

## Known TODOs and Future Work

See TODO.md for comprehensive list. Key items:

**catalog-server:**
- Display image references in artist/album/track models
- FTS5 search optimization (optional)

**web:**
- Toast/Snackbar notification system
- Collapsible side panels
- Text scrolling for overflow
- Logger implementation (replace console.log)
- User profile page
- Admin panel
- Lazy loading for long lists
- Track selection/multi-select
- Playlist reordering

**android:**
- Album and artist image loading
- Track lists in album/artist screens
- Full player screen
- Memory pressure component for cache management
- Offline logout queue
