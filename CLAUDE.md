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
cargo run -- <catalog-db-path> <user-db-path> --media-path=<media-path> --content-cache-age-sec=60 --logging-level path
```

Example:
```bash
cargo run -- ../../catalog.db ../../test.db --media-path=../../pezzottify-catalog --content-cache-age-sec=60 --logging-level path
```

**Build features:**
- `--features no_search`: Faster builds without search index
- `--features fast`: Alias for no_search (fastest for development)
- `--features slowdown`: Adds slowdown layer for testing

**CLI arguments:**
- `--media-path <PATH>`: Path to media files (audio/images), defaults to parent of catalog-db
- `--port <PORT>`: Server port (default: 3001)
- `--metrics-port <PORT>`: Metrics server port (default: 9091)
- `--logging-level <LEVEL>`: Request logging level (default: path)
- `--content-cache-age-sec <SECONDS>`: HTTP cache duration (default: 3600)
- `--frontend-dir-path <PATH>`: Serve static frontend files
- `--downloader-url <URL>`: URL of the downloader service for fetching missing content (optional)
- `--downloader-timeout-sec <SECONDS>`: Timeout for downloader requests (default: 300)

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
- `server/`: Axum HTTP server
  - `session.rs`: Session management via cookies
  - `stream_track.rs`: Audio streaming with range request support
  - `search.rs`: Search API routes
  - `http_layers/`: Middleware for logging, caching, slowdown
- `search/`: Search functionality
  - `PezzotHashSearchVault`: Full-text search implementation
  - `NoOpSearchVault`: Disabled search (for `no_search` feature)
- `sqlite_persistence/`: Database schema management
  - `versioned_schema.rs`: Schema migrations with version tracking

**Key types:**
- `SqliteCatalogStore`: SQLite-backed catalog with CRUD operations
- `CatalogStore`: Trait for catalog access (read and write)
- `Session`: Request session with user permissions
- `Permission`: Enum for access control (AccessCatalog, LikeContent, OwnPlaylists, EditCatalog, ManagePermissions, IssueContentDownload, RebootServer)
- `UserRole`: Admin (all permissions) or Regular (basic permissions)

**Server routes structure:**
- `/v1/auth/*`: Login, logout, session management
- `/v1/catalog/*`: Artists, albums, tracks, images
- `/v1/search/*`: Search endpoints
- `/v1/user/*`: User playlists, liked content
- `/v1/playback/*`: Track streaming

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

**Store modules (Pinia):**
- `auth.js`: Authentication state
- `player.js`: Audio playback state
- `remote.js`: API communication
- `user.js`: User data (playlists, liked content)
- `statics.js`: Static data caching
- `debug.js`: Debug configuration

**Routing:**
All content routes nest under HomeView with `meta: { requiresAuth: true }`:
- `/search/:query?`: Search results
- `/track/:trackId`: Track details
- `/album/:albumId`: Album details
- `/artist/:artistId`: Artist details
- `/playlist/:playlistId`: Playlist details
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
- Admin role has all permissions; Regular role has AccessCatalog, LikeContent, OwnPlaylists

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
- Catalog change log for users
- Listening stats collection
- Content download functionality
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
