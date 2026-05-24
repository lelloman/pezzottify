# Pezzottify

Pezzottify is a self-hosted music streaming system for a personal catalog. It has a Rust server, a Vue web app, and an Android app, with SQLite databases for catalog, user, queue, and enrichment state.

It is built around owning the full music stack: browsing and streaming local media, syncing user state across devices, tracking listening history, requesting missing content, and enriching catalog metadata in the background.

## What It Does

- Streams local audio with HTTP range support for seeking.
- Serves artist, album, track, image, search, popular, and recently-added catalog APIs.
- Manages users, roles, permissions, sessions, devices, playlists, likes, and settings.
- Syncs user state across web and Android clients through a server-side event log and WebSocket updates.
- Tracks listening events and page impressions for analytics and popularity scoring.
- Queues download requests for missing albums or discographies through an external downloader service.
- Runs scheduled background jobs for popular content, catalog availability, cleanup, and metadata enrichment.
- Stores generated artist, album, and track facts in queryable v1 enrichment tables instead of generic profile JSON.
- Exposes Prometheus metrics for server health, traffic, auth, database, downloader, and listening activity.

## Repository Layout

```text
pezzottify/
├── pezzottify-server/     Rust/Axum backend, SQLite stores, background jobs
├── web/                   Vue 3 frontend
├── android/               Kotlin/Jetpack Compose Android app
├── docs/                  Design notes and feature documentation
├── docker-compose.yml     Local development stack
└── build-docker.sh        Docker build wrapper with git version metadata
```

Start with the component docs when working in a specific area:

- [Server README](pezzottify-server/README.md)
- [Web README](web/README.md)
- [Android README](android/README.md)
- [Metadata Enrichment v1](docs/metadata-enrichment-v1.md)

## Quick Start

Requirements:

- Docker and Docker Compose
- Git

Run the development stack:

```bash
git clone https://github.com/lelloman/pezzottify
cd pezzottify
mkdir -p dev-data
cp pezzottify-server/config.example.toml pezzottify-server/config.toml
# Edit pezzottify-server/config.toml for your paths and optional services.
docker compose up --build
```

The server listens on `http://localhost:3001` by default. If the frontend is served by the server build, open that URL in a browser; otherwise run the web dev server from `web/`.

Production deployment details live in the [homelab](https://github.com/lelloman/homelab) repository.

## Architecture

```text
Android app        Web app
     |               |
     +-------+-------+
             |
      HTTP/WebSocket
             |
     pezzottify-server
             |
   +---------+----------+
   |                    |
SQLite databases     media files
```

The server owns the durable state. Clients keep local caches where useful, but catalog data, sync events, permissions, listening analytics, download queues, background job state, and enrichment data are server-side.

## Core Data

The music catalog is centered on artists, albums, and tracks:

- Artists can appear on tracks and albums, and tracks can carry artist roles such as main artist, featured artist, composer, remixer, conductor, or orchestra.
- Albums group tracks by disc and track number and carry release metadata, label, availability, images, and external IDs where available.
- Tracks point to audio files under the media directory and can include language, ISRC, duration, popularity, explicit flag, and availability.

Media files are stored separately from metadata:

```text
<media-path>/
├── albums/
│   └── <album-id>/
│       ├── 01-track.mp3
│       └── 02-track.flac
└── images/
    └── <image-id>
```

`catalog.db` stores imported catalog metadata. `user.db` stores accounts and user state. `server.db` stores operational state such as background job audits. `enrichment.db` stores audio and metadata enrichment tables.

## Authentication And Permissions

Pezzottify uses server-side sessions backed by signed auth tokens and HTTP-only cookies. Users can register multiple devices, and the server prunes old devices when limits are reached.

There are two built-in roles:

- `Admin`: catalog editing, user and permission management, analytics, server administration, download management, and content requests.
- `Regular`: catalog access, likes, and playlist ownership.

See the [server README](pezzottify-server/README.md#authentication--authorization) for the complete permission list and CLI user-management workflow.

## Sync And Analytics

User-facing state changes are written to an append-only sync event log. Connected clients receive updates over `/v1/ws`; offline clients catch up through `/v1/sync/events`.

Listening events and impressions feed popular content and analytics views. Metadata enrichment is admin/job driven: when the enrichment queue is empty, `metadata_enrichment_v1` seeds missing or stale artist, album, and track work from all-time listening counts, leaving zero-listen catalog items alone.

## Metadata Enrichment

Metadata Enrichment v1 is queue-based and non-blocking. The `metadata_enrichment_v1` background job claims manual/admin queue rows first; if none are claimable, it seeds a backlog from all-time listening data and later fills typed tables for artists, albums, tracks, tags, contributors, aliases, external IDs, relations, sources, and evidence.

The job reuses the existing shared `[agent]` / `[agent.llm]` configuration. It supports Ollama and OpenAI-compatible chat APIs; Simple-AI can be used when it exposes an OpenAI-compatible endpoint.

```toml
[agent]
enabled = true

[agent.llm]
provider = "openai"
base_url = "http://simple-ai:8000/v1"
model = "your-chat-model"
```

For schema details and operational behavior, see [docs/metadata-enrichment-v1.md](docs/metadata-enrichment-v1.md).

## API Surface

The main server endpoints are grouped by responsibility:

| Path | Purpose |
| ---- | ------- |
| `/v1/auth/*` | Login, logout, session, challenge authentication |
| `/v1/content/*` | Catalog reads, streaming, images, search, popular content |
| `/v1/user/*` | Likes, playlists, settings, listening events, impressions |
| `/v1/sync/*` | Sync state and event-log catch-up |
| `/v1/download/*` | Download requests and queue state |
| `/v1/admin/*` | Users, permissions, analytics, changelog, jobs, server control |
| `/v1/ws` | Realtime sync WebSocket |
| `/v1/mcp` | MCP WebSocket for LLM tool access |

For endpoint-level documentation, see [pezzottify-server/README.md#api-endpoints](pezzottify-server/README.md#api-endpoints).

## Development

Server:

```bash
cd pezzottify-server
cargo run --features fast -- --config ./config.toml
cargo test
```

Web:

```bash
cd web
npm install
npm run dev
npm run build
```

Android:

```bash
cd android
./gradlew test
```

End-to-end server tests live in [pezzottify-server/tests](pezzottify-server/tests/README.md).

## Monitoring

The server exports Prometheus metrics on the configured metrics port, default `9091`. Metrics cover request counts and latency, auth attempts, rate limiting, database queries, active sessions, memory, downloader activity, listening events, and errors.

The full production monitoring stack with Prometheus, Grafana, and Alertmanager is maintained in the [homelab](https://github.com/lelloman/homelab) deployment repository.

## License

Pezzottify is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
