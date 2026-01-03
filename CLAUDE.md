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
- `--features fast`: Faster builds (skips expensive integrity checks)
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
  - `Fts5LevenshteinSearchVault`: SQLite FTS5 with trigram tokenizer and Levenshtein typo correction
  - `streaming/`: Streaming search pipeline with target identification and enrichment
- `sqlite_persistence/`: Database schema management
  - `versioned_schema.rs`: Schema migrations with version tracking
- `config/`: Configuration management
  - Combines CLI arguments and TOML file configuration
  - TOML values override CLI defaults
- `background_jobs/`: Scheduled background tasks
  - Job scheduler with configurable intervals
  - `PopularContentJob`: Computes popular content metrics
  - `IntegrityWatchdogJob`: Periodic catalog integrity scans
  - `AuditLogCleanupJob`: Cleans old download audit entries
- `server_store/`: Server-level data persistence
  - `SqliteServerStore`: Stores server state (job history, etc.)
- `downloader/`: External content fetching (optional)
  - Integration with external downloader service for missing content
- `download_manager/`: Queue-based content acquisition (optional)
  - `DownloadManager`: Main facade for download operations
  - `DownloadQueueStore`: SQLite queue persistence
  - `QueueProcessor`: Background download processing
  - `AuditLogger`: Comprehensive audit trail
  - `IntegrityWatchdog`: Scans for missing content
  - `SearchProxy`: External provider search

**Key types:**
- `SqliteCatalogStore`: SQLite-backed catalog with CRUD operations
- `CatalogStore`: Trait for catalog access (read and write)
- `Session`: Request session with user permissions
- `Permission`: Enum for access control (AccessCatalog, LikeContent, OwnPlaylists, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin, ViewAnalytics, RequestContent)
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
- `/v1/download/*`: Download manager (requires RequestContent permission)
  - User endpoints:
    - `GET /limits`: Get user's rate limit status
    - `GET /my-requests`: Get user's queued requests
    - `POST /request/album`: Request album download
    - `POST /request/discography`: Request artist discography
  - Admin endpoints (require ViewAnalytics/EditCatalog):
    - `GET /admin/stats`: Queue statistics
    - `GET /admin/failed`: Failed download items
    - `GET /admin/activity`: Recent activity log
    - `GET /admin/requests`: All queued requests
    - `POST /admin/retry/{id}`: Retry failed download
    - `GET /admin/audit`: Query audit log
    - `GET /admin/audit/item/{id}`: Audit for queue item
    - `GET /admin/audit/user/{user_id}`: Audit for user
- `/v1/ws`: WebSocket for real-time updates
- `/v1/mcp`: WebSocket for MCP (Model Context Protocol) - LLM tool access
  - Tools: `catalog.search`, `catalog.get`, `catalog.mutate`, `users.query`, `users.mutate`, `analytics.query`, `downloads.query`, `downloads.action`, `server.query`, `jobs.query`, `jobs.action`, `debug.sql`, `debug.inspect`
  - Resources: `logs://`, `jobs://`, `config://`, `changelog://`
  - See `docs/mcp-server-design.md` for full documentation

**Authentication flow (OIDC):**
Both web and Android use client-side OIDC authentication:
1. Client initiates OIDC flow with authorization code + PKCE
2. User authenticates with OIDC provider (e.g., LelloAuth)
3. Client receives authorization code, exchanges for tokens (ID token + refresh token)
4. Client stores tokens locally (localStorage for web, encrypted prefs for Android)
5. Client sends ID token in `Authorization` header (or cookie for WebSocket)
6. Server validates ID token against OIDC provider's JWKS
7. On 401 response, client refreshes tokens using refresh token and retries

### Web Frontend Architecture

**Technology stack:**
- Vue 3 Composition API
- Vue Router for navigation
- Pinia for state management
- Axios for HTTP requests
- Howler.js for audio playback
- Vite build tool
- oidc-client-ts for OIDC authentication

**Configuration:**
Copy `.env.example` to `.env.local` and configure:
- `VITE_OIDC_AUTHORITY`: OIDC provider URL (required)
- `VITE_OIDC_CLIENT_ID`: Client ID registered with provider (required)
- `VITE_OIDC_REDIRECT_URI`: Callback URL (defaults to `{origin}/auth/callback`)
- `VITE_OIDC_SCOPE`: OAuth scopes (defaults to `openid profile email offline_access`)

**Store modules (Pinia):** Located in `web/src/store/`
- `auth.js`: Authentication state and OIDC integration
- `player.js`: Audio playback state
- `remote.js`: API communication
- `user.js`: User data (playlists, liked content)
- `statics.js`: Static data caching
- `debug.js`: Debug configuration
- `sync.js`: Multi-device sync state management (WebSocket connection, sync events)
- `chat.js`: AI chat state, LLM config, tool execution

**Services:** Located in `web/src/services/`
- `oidc.js`: Client-side OIDC authentication (login, callback, token refresh)
- `api.js`: Axios interceptors for auth headers and automatic token refresh on 401
- `websocket.js`: WebSocket connection for real-time updates
- `llm/`: Multi-provider LLM adapters (Anthropic, OpenAI, Google, Ollama, OpenRouter)
- `mcp.js`: WebSocket client for MCP server (catalog queries via tools)
- `uiTools.js`: Local UI control tools (playback, navigation, likes, playlists)

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
- `/auth/callback`: OIDC callback handler
- `/logout`: Logout handler

**Component structure:**
- `views/`: Page components (HomeView, LoginView, AuthCallbackView, AdminView)
- `components/content/`: Content display components
- `components/common/`: Reusable UI components
- `components/common/contextmenu/`: Right-click context menus
- `components/search/`: Search-related components
- `components/icons/`: Icon components
- `components/chat/`: AI chat interface (ChatButton, ChatPanel, ChatMessage, ChatSettings)

**Key features:**
- Authentication guard on router (redirects to `/login` if not authenticated)
- Global debug config accessible via `window.config`
- HTTP cache blocking for development
- Right-click context menu system
- AI chat assistant with multi-provider LLM support

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
- Admin role has: AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin, ViewAnalytics, RequestContent
- Regular role has: AccessCatalog, LikeContent, OwnPlaylists

**Database operations:**
- SQLite used for user storage
- Schema managed via `VersionedSchema` in `sqlite_persistence/`
- Migrations tracked with version numbers
- Foreign keys enforced (cascade deletes for user content)
- All multi-step operations use transactions (role management, permission countdown, auth credentials, migrations)
- Single operations rely on SQLite's default atomicity

**Search indexing:**
- Built at startup from catalog database using FTS5 with Levenshtein typo correction
- Supports organic search and streaming search with target identification/enrichment

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
