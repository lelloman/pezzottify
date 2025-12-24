# Pezzottify + Quentin Torrentino Integration Plan

This document describes the integration between Pezzottify (catalog-server, web, android) and Quentin Torrentino.

**Related docs:**
- [Interface Contract](./QUENTIN_TORRENTINO_INTERFACE.md) - API endpoints, ticket format, WebSocket messages
- Quentin Torrentino repo (separate) - Full implementation details

## Pre-requisites (Track B - Pezzottify Prep)

These changes prepare Pezzottify for integration, independent of Quentin Torrentino development.

### 1. Track Availability State (catalog-server)

Tracks have an availability state that indicates whether audio is playable and why/why not.

**Schema change:**
```sql
-- tracks table
audio_uri TEXT,  -- NULL if not available
availability TEXT NOT NULL DEFAULT 'available'  -- enum: available, unavailable, fetching, fetch_error
```

**Track availability states:**
```rust
enum TrackAvailability {
    Available,    // Has audio_uri, can be played
    Unavailable,  // No audio, not requested for download
    Fetching,     // Download in progress
    FetchError,   // Download failed (may be retryable)
}
```

**API behavior:**
- Track endpoints return `availability` field alongside `audio_uri`
- `GET /stream/{id}` returns 404 if not `Available`
- State transitions:
  - `Unavailable` → `Fetching` (when download requested)
  - `Fetching` → `Available` (download succeeded, audio_uri set)
  - `Fetching` → `FetchError` (download failed)
  - `FetchError` → `Fetching` (retry requested)

**Immediate use case (before integration):**
- Delete existing corrupted audio files from the catalog
- Set those tracks to `Unavailable` or `FetchError`
- Validates the feature works end-to-end

### 2. Web Client Changes

- Show visual indicator per availability state:
  - `Available`: Normal playback
  - `Unavailable`: Grayed out, "Not available" label
  - `Fetching`: Loading spinner or progress indicator
  - `FetchError`: Error icon, "Download failed" with retry option
- Player: Skip non-Available tracks in queue
- Request download button for Unavailable tracks

### 3. Android Client Changes

Same as web:
- Visual indicators for each state
- Player skips non-Available tracks
- Request download action

## Integration (Track C - After Quentin Torrentino is ready)

### 1. Catalog-Server as Ticket Issuer

Catalog-server creates tickets and sends them to Quentin Torrentino.

**New module: `torrentino_client/`**

```rust
struct TorrentinoClient {
    base_url: String,
    auth_token: String,
    http_client: reqwest::Client,
}

impl TorrentinoClient {
    async fn create_ticket(&self, ticket: MusicTicket) -> Result<TicketId>;
    async fn get_ticket_status(&self, id: &TicketId) -> Result<TicketState>;
    async fn approve(&self, id: &TicketId, candidate_idx: Option<usize>) -> Result<()>;
    async fn reject(&self, id: &TicketId, reason: &str) -> Result<()>;
    async fn retry(&self, id: &TicketId) -> Result<()>;
}
```

**Ticket creation flow:**
1. User requests album/track download
2. Catalog-server creates track entries with `availability = Fetching`
3. Catalog-server builds MusicTicket with track metadata + dest_paths
4. Catalog-server sends ticket to Quentin Torrentino
5. Catalog-server stores ticket_id for tracking

### 2. WebSocket Connection for Real-time Updates

Catalog-server maintains WebSocket connection to Quentin Torrentino:

```rust
async fn handle_torrentino_event(event: TorrentinoEvent) {
    match event {
        TorrentinoEvent::Progress { ticket_id, progress_pct, .. } => {
            // Update internal state, broadcast to connected web clients
        }
        TorrentinoEvent::Completed { ticket_id, .. } => {
            // Update track availability to Available
            // Set audio_uri for each track
        }
        TorrentinoEvent::Failed { ticket_id, error, retryable } => {
            // Update track availability to FetchError
        }
        TorrentinoEvent::NeedsApproval { ticket_id, candidates } => {
            // Store candidates, notify admin
        }
    }
}
```

### 3. Admin UI for Approvals

New admin section for managing download tickets:

**Endpoints:**
```
GET    /v1/admin/downloads
       → List pending/active downloads

GET    /v1/admin/downloads/{ticket_id}
       → Get ticket details + candidates

POST   /v1/admin/downloads/{ticket_id}/approve
       Body: { "candidate_idx": 0 }

POST   /v1/admin/downloads/{ticket_id}/reject
       Body: { "reason": "..." }
```

**Web UI:**
- List of tickets needing approval
- Show candidates with scores, seeders, sizes
- Approve/reject buttons
- Force-search input for failed searches

### 4. User Download Request UI

Users can request downloads for unavailable content:

**Endpoints:**
```
POST   /v1/user/request-download
       Body: { "album_id": "..." } or { "track_ids": ["...", "..."] }
       → Creates ticket, returns request_id

GET    /v1/user/download-requests
       → List user's pending requests with status
```

**Rate limiting:**
- Max N requests per user per day
- Admin can adjust limits per user

### 5. Callback Handler (Optional)

If using webhooks instead of/alongside WebSocket:

```
POST   /v1/internal/torrentino-callback
       Body: TorrentinoEvent
       → Update track availability, notify users
```

## Configuration

```toml
[torrentino]
enabled = true
url = "http://quentin-torrentino:8080"
auth_token = "shared-secret"
websocket_reconnect_secs = 5

[torrentino.rate_limits]
requests_per_user_per_day = 10
max_pending_per_user = 5
```

## Migration from Old Downloader

1. Disable old downloader
2. Enable Quentin Torrentino integration
3. Set all tracks without audio_uri to `Unavailable`
4. Old download queue items are abandoned (users re-request if needed)

## Implementation Checklist

### Pre-requisites (can start now)
- [ ] Track availability state schema migration
- [ ] Update track API responses with availability
- [ ] Stream endpoint 404 for non-Available
- [ ] Web: availability indicators
- [ ] Web: player skips non-Available
- [ ] Android: availability indicators
- [ ] Android: player skips non-Available
- [ ] Delete corrupted files, test the feature

### Integration (after Quentin Torrentino ready)
- [ ] TorrentinoClient module
- [ ] Ticket creation flow
- [ ] WebSocket event handling
- [ ] Track availability updates on events
- [ ] Admin approval UI
- [ ] User request download UI
- [ ] Rate limiting
- [ ] Configuration
- [ ] Migration script
