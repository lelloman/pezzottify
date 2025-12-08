# Pezzottify

A self-hosted music streaming platform with a Rust backend, Vue 3 web frontend, and Android app.

## Components

| Component | Description | Tech Stack |
|-----------|-------------|------------|
| **catalog-server** | Backend API server | Rust, Axum, SQLite |
| **web** | Web frontend | Vue 3, Vite, Pinia |
| **android** | Mobile app | Kotlin, Jetpack Compose |

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Git

### Running with Docker

```bash
# Clone the repository
git clone <repository-url>
cd pezzottify

# Copy and configure environment (for monitoring/alerts)
cp monitoring/.env.example monitoring/.env

# Build and start
./build-docker.sh -d catalog-server
```

The server will be available at http://localhost:3001

### Development Setup

See component-specific READMEs:
- [catalog-server/README.md](catalog-server/README.md) - Backend server
- [web/README.md](web/README.md) - Web frontend
- [android/README.md](android/README.md) - Android app

## Architecture

```
┌─────────────┐     ┌─────────────┐
│   Android   │     │     Web     │
│     App     │     │  Frontend   │
└──────┬──────┘     └──────┬──────┘
       │                   │
       └─────────┬─────────┘
                 │ HTTP/WebSocket
                 ▼
        ┌────────────────┐
        │ Catalog Server │
        │     (Rust)     │
        └───────┬────────┘
                │
       ┌────────┴────────┐
       ▼                 ▼
┌─────────────┐   ┌─────────────┐
│   SQLite    │   │   Media     │
│  Databases  │   │   Files     │
└─────────────┘   └─────────────┘
```

## Features

- Stream music from your personal collection
- User authentication with role-based permissions
- Playlists and liked content
- Full-text search across artists, albums, and tracks
- Real-time sync across devices via WebSocket
- Monitoring with Prometheus and Grafana

## Users & Permissions

### Roles

| Role | Description |
|------|-------------|
| **Admin** | Full system access including catalog editing and user management |
| **Regular** | Standard user with catalog access, playlists, and likes |

### Permissions

| Permission | Admin | Regular | Description |
|------------|:-----:|:-------:|-------------|
| AccessCatalog | ✓ | ✓ | Browse and stream music |
| LikeContent | ✓ | ✓ | Like artists, albums, and tracks |
| OwnPlaylists | ✓ | ✓ | Create and manage playlists |
| EditCatalog | ✓ | | Add, update, delete catalog entries |
| ManagePermissions | ✓ | | Manage user roles and permissions |
| IssueContentDownload | ✓ | | Generate download tokens |
| RebootServer | ✓ | | Restart the server remotely |
| ViewAnalytics | ✓ | | View listening and bandwidth analytics |

### Devices

Users can log in from multiple devices (up to 50). Each device is tracked with:
- Unique device identifier
- Device type (web, android, ios)
- Device name and OS info
- Last used timestamp

## Music Catalog

### Data Model

```
┌────────┐       ┌────────┐       ┌────────┐
│ Artist │◄─────►│ Album  │◄──────│ Track  │
└────────┘  N:M  └────────┘  1:N  └────────┘
     ▲                                 │
     └─────────────────────────────────┘
                    N:M (with roles)
```

**Artists**
- Name and optional sort name
- Genres (e.g., rock, jazz)
- Activity periods (start/end years)
- Related artists
- Images

**Albums**
- Title (original and version titles)
- Multiple artists (for collaborations)
- Release date and label
- Genres
- Cover images

**Tracks**
- Title (original and version titles)
- Belongs to one album
- Multiple artists with roles (performer, composer, featured, etc.)
- Duration, track number, disc number
- Audio file reference
- Tags and languages

### Storage

- **Metadata**: SQLite database (catalog.db)
- **Audio files**: Filesystem organized by album ID
- **Images**: Filesystem with unique image IDs

```
media/
├── albums/
│   └── <album-id>/
│       ├── 01-track.mp3
│       └── 02-track.flac
└── images/
    └── <image-id>.jpg
```

## User Settings

Settings are divided into two categories based on whether they should sync across devices.

### Server-side Settings (Synced)

Stored on the server and synchronized to all user devices via the event log.

| Setting | Description |
|---------|-------------|
| `enable_direct_downloads` | Allow on-demand fetching of missing content |

### Client-side Settings (Local)

Stored locally on each device. Users can customize each device independently.

| Setting | Options | Description |
|---------|---------|-------------|
| Theme Mode | System, Light, Dark, Amoled | App appearance |
| Color Palette | Classic, OceanBlue, SunsetCoral, PurpleHaze, RoseGold, Midnight, Forest | Accent colors |
| Font Family | System, SansSerif, Serif, Monospace | Typography |
| Play Behavior | Replace, Add to playlist | What happens when playing a track |
| In-Memory Cache | On/Off | Performance optimization |

## Real-time Sync

Pezzottify keeps user data synchronized across all connected devices using WebSocket connections and an append-only event log.

### How It Works

1. Client connects via WebSocket and receives a sequence number
2. When data changes (on any device), server broadcasts the event
3. Other connected devices receive the update instantly
4. Offline devices catch up by fetching missed events on reconnect

### Synced Data

| Event Type | Description |
|------------|-------------|
| `content_liked` / `content_unliked` | Like/unlike artists, albums, tracks |
| `setting_changed` | User preference changes |
| `playlist_created` | New playlist |
| `playlist_renamed` | Playlist name change |
| `playlist_deleted` | Playlist removal |
| `playlist_tracks_updated` | Tracks added/removed/reordered |
| `permission_granted` / `permission_revoked` | Permission changes (admin actions) |

### Event Log

Events are stored with sequence numbers, allowing clients to:
- Request events since their last known sequence
- Handle offline periods gracefully
- Resolve conflicts with server-authoritative ordering

## API Overview

The server exposes a REST API over HTTP with WebSocket support for real-time sync.

### Endpoint Groups

| Path | Description |
|------|-------------|
| `/v1/auth/*` | Authentication (login, logout, session) |
| `/v1/content/*` | Catalog content (artists, albums, tracks, images, streaming, search) |
| `/v1/user/*` | User content (playlists, liked content, settings, listening stats) |
| `/v1/admin/*` | Admin operations (user management, analytics, server control) |
| `/v1/sync/*` | Event log for multi-device sync |
| `/v1/ws` | WebSocket connection for real-time updates |

### Key Features

- **Token-based auth**: Session tokens in HTTP-only cookies
- **Range requests**: Efficient audio streaming with seek support
- **Rate limiting**: Per-endpoint limits to prevent abuse
- **HTTP caching**: Configurable cache headers for static content

For detailed endpoint documentation, see [catalog-server/README.md](catalog-server/README.md#api-endpoints).

## Monitoring

Pezzottify includes a full observability stack for production deployments.

### Components

| Service | Port | Description |
|---------|------|-------------|
| Prometheus | 9090 | Metrics collection and alerting rules |
| Grafana | 3000 | Dashboards and visualization |
| Alertmanager | 9093 | Alert routing and notifications |

### Metrics

The server exposes Prometheus metrics (internal port 9091) including:
- HTTP request counts and latencies
- Authentication attempts (success/failure)
- Rate limit violations
- Database query performance
- Active sessions and memory usage

### Alerts

Pre-configured alerts for common issues:
- **Critical**: Service down, brute force attempts, high error rate, database errors
- **Warning**: Rate limit violations, slow queries, high memory usage

### Notifications

Supports multiple notification channels:
- Telegram bot (built-in)
- Generic webhook (Slack, Discord, PagerDuty, etc.)

For setup instructions, see [catalog-server/README.md](catalog-server/README.md#monitoring--alerting).

## Project Structure

```
pezzottify/
├── catalog-server/     # Rust backend
├── web/                # Vue 3 frontend
├── android/            # Kotlin/Android app
├── monitoring/         # Prometheus, Grafana, Alertmanager configs
├── docker-compose.yml  # Docker orchestration
└── build-docker.sh     # Docker build script with version detection
```

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.
