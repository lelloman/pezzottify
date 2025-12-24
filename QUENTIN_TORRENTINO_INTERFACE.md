# Quentin Torrentino - Interface Contract

This document defines the contract between Quentin Torrentino and its consumers (Pezzottify, Pezzottflix, etc.).

## Ticket Structure (Music)

The ticket is the request format sent to Quentin Torrentino for music downloads.

```json
{
  "ticket_id": "uuid",
  "created_at": "2024-12-24T10:00:00Z",
  "content_type": "music",

  "search": {
    "artist": "Radiohead",
    "album": "OK Computer",
    "year": 1997,
    "label": "Parlophone",
    "genres": ["alternative rock", "art rock"]
  },

  "tracks": [
    {
      "id": "t1",
      "disc_number": 1,
      "track_number": 1,
      "name": "Airbag",
      "duration_secs": 284,
      "dest_path": "/media/albums/abc123/d1t01.ogg",
      "requested": true
    },
    {
      "id": "t2",
      "disc_number": 1,
      "track_number": 2,
      "name": "Paranoid Android",
      "duration_secs": 383,
      "dest_path": "/media/albums/abc123/d1t02.ogg",
      "requested": true
    }
  ],

  "images": [
    {
      "id": "img1",
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

## API Endpoints

### Ticket Management

```
POST   /api/v1/ticket
       Body: Ticket JSON
       Response: { "ticket_id": "uuid", "state": "pending" }

GET    /api/v1/ticket/{ticket_id}
       Response: Full ticket state and history

GET    /api/v1/tickets
       Query: ?state=needs_approval&limit=50&offset=0
       Response: Paginated ticket list

DELETE /api/v1/ticket/{ticket_id}
       Response: { "success": true } (if not terminal)
```

### Admin Actions

```
POST   /api/v1/ticket/{ticket_id}/approve
       Body: { "candidate_idx": 0 }  (optional)
       Response: { "state": "approved" }

POST   /api/v1/ticket/{ticket_id}/reject
       Body: { "reason": "Wrong album" }
       Response: { "state": "rejected" }

POST   /api/v1/ticket/{ticket_id}/retry
       Response: { "state": "pending" }

POST   /api/v1/ticket/{ticket_id}/force-search
       Body: { "query": "custom search query" }
       Response: { "state": "searching" }

POST   /api/v1/ticket/{ticket_id}/force-magnet
       Body: { "magnet_uri": "magnet:?xt=..." }
       Response: { "state": "downloading" }
```

### Status & Health

```
GET    /api/v1/health
       Response: { "status": "ok", "version": "0.1.0" }

GET    /api/v1/stats
       Response: { "pending": 5, "downloading": 2, "completed_today": 15, ... }
```

### WebSocket (Real-time Updates)

```
WS     /api/v1/ws

Messages (server → client):
{
  "type": "state_change",
  "ticket_id": "uuid",
  "old_state": "downloading",
  "new_state": "converting",
  "details": { ... }
}

{
  "type": "progress",
  "ticket_id": "uuid",
  "state": "downloading",
  "progress_pct": 45.2,
  "speed_bps": 1048576,
  "eta_secs": 120
}

{
  "type": "needs_approval",
  "ticket_id": "uuid",
  "candidates": [
    { "title": "...", "score": 0.85, "seeders": 50 },
    ...
  ]
}

{
  "type": "completed",
  "ticket_id": "uuid",
  "items_placed": 12
}

{
  "type": "failed",
  "ticket_id": "uuid",
  "error": "No matching torrents found",
  "retryable": true
}
```

## State Machine

```
PENDING → SEARCHING → MATCHING → AUTO_APPROVED → DOWNLOADING → CONVERTING → PLACING → COMPLETED
                   ↓           ↓
            SEARCH_FAILED   NEEDS_APPROVAL → APPROVED → DOWNLOADING...
                                          ↓
                                       REJECTED

Any state → FAILED (on error, may be retryable)
```

### State Response Format

```json
{
  "ticket_id": "uuid",
  "state": "downloading",
  "state_details": {
    "torrent_hash": "abc123",
    "progress_pct": 45.2,
    "download_speed_bps": 1048576,
    "eta_secs": 120,
    "started_at": "2024-12-24T10:05:00Z"
  },
  "history": [
    { "state": "pending", "entered_at": "2024-12-24T10:00:00Z" },
    { "state": "searching", "entered_at": "2024-12-24T10:00:01Z" },
    ...
  ]
}
```

## Authentication

Requests must include authentication header:

```
Authorization: Bearer <token>
```

Token is configured in Quentin Torrentino and shared with consumers.

## Callbacks (Optional)

Consumers can register webhook callbacks:

```
POST   /api/v1/webhooks
       Body: { "url": "https://catalog-server/callbacks", "events": ["completed", "failed"] }
```

Quentin Torrentino will POST to the callback URL when events occur:

```json
{
  "event": "completed",
  "ticket_id": "uuid",
  "timestamp": "2024-12-24T10:15:00Z",
  "details": { ... }
}
```
