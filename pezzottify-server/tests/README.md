# End-to-End HTTP Tests

This directory contains end-to-end integration tests for the pezzottify-server. These tests spawn a real HTTP server, make actual HTTP requests, and verify the complete request/response cycle.

## Overview

The test suite uses:
- **Real HTTP server**: Spawns `axum` server on a random port
- **reqwest**: HTTP client with cookie support for session management
- **Temporary resources**: Each test gets isolated catalog and database
- **No mocking**: Tests the actual production code paths

## Running Tests

```bash
# Run all e2e tests
cargo test --test '*'

# Run specific test file
cargo test --test e2e_auth_tests

# Run specific test
cargo test --test e2e_catalog_tests test_get_artist_returns_correct_data

# Run with output
cargo test --test e2e_auth_tests -- --nocapture

# Run tests in parallel (default)
cargo test --test '*' -- --test-threads=4

# Run tests sequentially (for debugging)
cargo test --test '*' -- --test-threads=1
```

## Architecture

### Directory Structure

```
tests/
├── README.md                     # This file
├── common/                       # Shared test infrastructure (DO NOT MODIFY TESTS DIRECTLY)
│   ├── mod.rs                   # Public API, re-exports
│   ├── constants.rs             # Test user credentials, catalog IDs, timeouts
│   ├── fixtures.rs              # Catalog and database creation
│   ├── server.rs                # TestServer - server lifecycle management
│   └── client.rs                # TestClient - HTTP request abstractions
├── fixtures/                     # Static test data
│   ├── test-audio.mp3           # 8KB silent MP3 (2 seconds)
│   └── test-image.jpg           # 224B gray 32x32 JPEG
├── e2e_auth_tests.rs            # Authentication: login, logout, sessions
├── e2e_catalog_tests.rs         # Catalog endpoints: artists, albums, tracks
├── e2e_streaming_tests.rs       # Audio streaming, range requests
├── e2e_search_tests.rs          # Search functionality
├── e2e_user_content_tests.rs    # User playlists, liked content
└── e2e_permissions_tests.rs     # Permission-based access control
```

### Test Catalog Structure

Each test gets a temporary catalog with:
- **2 artists**: "The Test Band" (artist-1), "Jazz Ensemble" (artist-2)
- **2 albums**: "First Album" (album-1), "Jazz Collection" (album-2)
- **5 tracks**: track-1 through track-5 (all same MP3 content, different metadata)
- **3 images**: image-1, image-2, image-3 (all same JPEG content)

### Test Users

Each test database includes:
- **testuser** / **testpass123** - Regular user (AccessCatalog, LikeContent, OwnPlaylists)
- **admin** / **adminpass123** - Admin user (all permissions)

## Writing Tests

### Basic Pattern

```rust
mod common;
use common::{TestServer, TestClient, ARTIST_1_ID};
use reqwest::StatusCode;

#[tokio::test]
async fn test_your_feature() {
    // 1. Spawn isolated test server
    let server = TestServer::spawn().await;

    // 2. Create authenticated client
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // 3. Make requests
    let response = client.get_artist(ARTIST_1_ID).await;

    // 4. Assert results
    assert_eq!(response.status(), StatusCode::OK);
    let artist: serde_json::Value = response.json().await.unwrap();
    assert_eq!(artist["name"], "The Test Band");
}
```

### Testing Authentication

```rust
#[tokio::test]
async fn test_login_flow() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Test login explicitly
    let response = client.login(TEST_USER, TEST_PASS).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify session works
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Test logout
    let response = client.logout().await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify session is gone
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

### Testing Permissions

```rust
#[tokio::test]
async fn test_admin_only_endpoint() {
    let server = TestServer::spawn().await;

    // Regular user denied
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client.some_admin_endpoint().await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Admin allowed
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin.some_admin_endpoint().await;
    assert_eq!(response.status(), StatusCode::OK);
}
```

### Testing Streaming with Range Requests

```rust
#[tokio::test]
async fn test_partial_content() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.stream_track_with_range(TRACK_1_ID, "bytes=0-1023").await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert!(response.headers().get("content-range").is_some());
}
```

## Common Module API

### TestServer

```rust
// Spawns server on random port, creates temp catalog and DB
let server = TestServer::spawn().await;

// Access base URL
println!("Server at: {}", server.base_url);

// Server automatically shuts down when dropped
```

### TestClient

```rust
// Unauthenticated client (for testing auth flows)
let client = TestClient::new(server.base_url.clone());

// Pre-authenticated regular user (most tests)
let client = TestClient::authenticated(server.base_url.clone()).await;

// Pre-authenticated admin user
let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
```

### Available Methods

#### Authentication
- `client.login(handle, password)` - POST /v1/auth/login
- `client.logout()` - POST /v1/auth/logout
- `client.get_session()` - GET /v1/auth/session

#### Catalog Content
- `client.get_artist(id)` - GET /v1/content/artist/{id}
- `client.get_album(id)` - GET /v1/content/album/{id}
- `client.get_track(id)` - GET /v1/content/track/{id}
- `client.get_resolved_track(id)` - GET /v1/content/track/{id}/resolved
- `client.get_artist_discography(id)` - GET /v1/content/artist/{id}/discography
- `client.get_image(id)` - GET /v1/content/image/{id} (id is album or artist ID)
- `client.get_whats_new()` - GET /v1/content/whatsnew

#### Streaming
- `client.stream_track(id)` - GET /v1/content/stream/{id}
- `client.stream_track_with_range(id, range)` - GET /v1/content/stream/{id} with Range header

#### User Content
- `client.add_liked_content(content_type, content_id)` - POST /v1/user/liked/{content_type}/{content_id}
- `client.remove_liked_content(content_type, content_id)` - DELETE /v1/user/liked/{content_type}/{content_id}
- `client.get_liked_content(content_type)` - GET /v1/user/liked/{content_type}
- `client.get_liked_status(content_id)` - GET /v1/user/liked/{content_id}/status

#### Playlists
- `client.create_playlist(name)` - POST /v1/user/playlist
- `client.get_playlists()` - GET /v1/user/playlists
- `client.get_playlist(id)` - GET /v1/user/playlist/{id}
- `client.update_playlist(id, name, track_ids)` - PUT /v1/user/playlist/{id}
- `client.delete_playlist(id)` - DELETE /v1/user/playlist/{id}
- `client.add_tracks_to_playlist(id, track_ids)` - PUT /v1/user/playlist/{id}/add
- `client.remove_tracks_from_playlist(id, track_ids)` - PUT /v1/user/playlist/{id}/remove

#### Search
- `client.search(query)` - POST /v1/content/search
- `client.search_resolved(query)` - POST /v1/content/search with resolve=true
- `client.search_with_filters(query, filters)` - POST /v1/content/search with filters

#### User Settings
- `client.get_user_settings()` - GET /v1/user/settings
- `client.update_user_settings_json(body)` - PUT /v1/user/settings

#### Listening Stats
- `client.post_listening_event(...)` - POST /v1/user/listening
- `client.get_listening_summary(...)` - GET /v1/user/listening/summary
- `client.get_listening_history(...)` - GET /v1/user/listening/history
- `client.get_listening_events(...)` - GET /v1/user/listening/events

#### Sync
- `client.get_sync_state()` - GET /v1/sync/state
- `client.get_sync_events(since)` - GET /v1/sync/events?since={since}

#### Admin (requires admin user)
- `client.get_jobs()` - GET /v1/admin/jobs
- `client.get_job(job_id)` - GET /v1/admin/jobs/{job_id}
- `client.trigger_job(job_id)` - POST /v1/admin/jobs/{job_id}/trigger
- `client.get_job_history(job_id, limit)` - GET /v1/admin/jobs/{job_id}/history
- `client.create_changelog_batch(...)` - POST /v1/admin/changelog/batch
- `client.list_changelog_batches(...)` - GET /v1/admin/changelog/batches
- `client.get_daily_listening_stats(...)` - GET /v1/admin/listening/daily
- `client.get_top_tracks(...)` - GET /v1/admin/listening/top-tracks

#### Health Check
- `client.get_statics()` - GET /v1/statics (server readiness check)

### Constants

```rust
use common::{
    TEST_USER, TEST_PASS,           // Regular user credentials
    ADMIN_USER, ADMIN_PASS,         // Admin credentials
    ARTIST_1_ID, ARTIST_2_ID,       // Artist IDs
    ALBUM_1_ID, ALBUM_2_ID,         // Album IDs
    TRACK_1_ID, TRACK_2_ID, ...,    // Track IDs (1-5)
    // Note: Images are now fetched by item ID (album/artist ID), not separate image IDs
};
```

## Modifying the Test Infrastructure

### ⚠️ Single Point of Change Principle

When the server changes, update **only the relevant file in common/**:

| If you need to change... | Update only... | Example |
|-------------------------|----------------|---------|
| API route paths | `common/client.rs` | `/v1/content/artist` → `/v2/artist` |
| Request/response JSON format | `common/client.rs` | Add new field to login request |
| Auth mechanism | `common/client.rs` | Switch from cookie to bearer token |
| Server startup logic | `common/server.rs` | Change `make_app()` signature |
| Catalog JSON schema | `common/fixtures.rs` | Add `year` field to albums |
| Test user roles | `common/fixtures.rs` | Add new test user type |
| Catalog IDs or names | `common/constants.rs` | Rename test artist |

**DO NOT** modify test files for infrastructure changes - only update `common/` modules.

### Adding New Endpoints

To add a new endpoint to the test suite:

1. Add method to `common/client.rs`:
```rust
pub async fn your_new_endpoint(&self, param: &str) -> Response {
    self.client
        .get(format!("{}/v1/your/endpoint/{}", self.base_url, param))
        .send()
        .await
        .expect("Your endpoint request failed")
}
```

2. (Optional) Add constant to `common/constants.rs` if needed

3. Write tests using the new method

### Extending Test Catalog

To add more test data, modify `common/fixtures.rs`:

```rust
pub fn create_test_catalog() -> Result<TempDir> {
    // ...existing code...

    // Add new artist
    write_artist_json(&dir, "artist-3", "New Artist", "image-3")?;

    // ...
}
```

Then add constant to `common/constants.rs`:
```rust
pub const ARTIST_3_ID: &str = "artist-3";
```

## Troubleshooting

### Tests Hang or Timeout

- Check server logs: `cargo test -- --nocapture`
- Verify server startup: Look for "Server ready" messages
- Reduce parallelism: `cargo test -- --test-threads=1`

### Port Already in Use

Tests use random ports (bind to `0.0.0.0:0`), so this shouldn't happen. If it does:
- Check for orphaned test processes: `ps aux | grep pezzottify-server`
- Kill them: `pkill -f pezzottify-server`

### Tests Fail After Server Changes

1. Check if routes changed → Update `common/client.rs`
2. Check if auth changed → Update `common/client.rs` auth methods
3. Check if startup changed → Update `common/server.rs`
4. Check if catalog format changed → Update `common/fixtures.rs`

### Fixture Files Missing

Regenerate fixtures:
```bash
cd pezzottify-server/tests/fixtures

# Generate test audio (requires ffmpeg)
ffmpeg -f lavfi -i anullsrc=r=44100:cl=stereo -t 2 -q:a 9 \
       -acodec libmp3lame test-audio.mp3 -y

# Generate test image (requires ffmpeg)
ffmpeg -f lavfi -i color=c=gray:s=32x32:d=1 -frames:v 1 test-image.jpg -y
```

## Design Decisions

### Why Real HTTP Tests?

- Tests the complete request/response cycle
- Catches HTTP-specific bugs (headers, status codes, cookies)
- Validates streaming, range requests, rate limiting
- Tests production code paths (no mocking)

### Why Random Ports?

- Allows parallel test execution
- No port conflicts
- Safe for CI/CD

### Why Temporary Resources?

- Complete test isolation
- No shared state between tests
- Clean teardown (automatic via RAII)

### Why One Audio File for All Tracks?

- Streaming tests care about bytes/ranges, not audio content
- Minimal git footprint (~8KB)
- Fast test execution
- Can extend to multiple formats later if needed

### Why Panic on Auth Failure?

- Auth is test infrastructure, not the thing being tested
- Loud failures prevent confusing downstream errors
- Most tests should use `TestClient::authenticated()`, not test auth

## Performance

- **Test server startup**: ~50-200ms (catalog loading, DB init)
- **Single request**: ~1-5ms
- **Typical test**: ~100-300ms total
- **Full suite**: ~5-30s (depends on test count)

To optimize:
- Use `#[cfg(feature = "no_checks")]` to skip catalog validation
- Use `#[cfg(feature = "fast")]` for faster builds
- Run in parallel: `cargo test -- --test-threads=8`

## Contributing

When adding tests:
1. Use existing `common/` infrastructure - don't reinvent
2. Follow existing test patterns
3. Use constants from `common/constants.rs`
4. Test one thing per test function
5. Use descriptive test names: `test_verb_expected_outcome`
6. Add comments for non-obvious assertions

When modifying infrastructure:
1. Update `common/` modules, not test files
2. Keep changes backward compatible if possible
3. Update this README if adding new patterns
4. Run full test suite after changes: `cargo test --test '*'`
