# MCP Server Design for Pezzottify

## Overview

Add an MCP (Model Context Protocol) server to catalog-server for LLM-based administration, debugging, and future automated catalog expansion.

## Goals

1. **Real-time introspection** - Access to live server state, not just database
2. **Admin operations** - User management, catalog editing, job control
3. **Debugging** - Inspect caches, sessions, search index, request flow
4. **Catalog agent** - Automated catalog management via LLM agent:
   - Music discovery and recommendations
   - Playlist generation
   - Catalog expansion (finding missing albums, related artists)
   - Gap filling (detecting incomplete discographies)
   - Metadata enrichment
   - Information retrieval for users

## Non-Goals (for now)

- Public API exposure (MCP is admin/dev only)
- Multi-tenant access control within MCP
- High-performance streaming (this is for occasional admin use)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    catalog-server                        │
│                                                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │  HTTP API   │  │  MCP Server │  │  Background     │  │
│  │  :3001      │  │  :3002      │  │  Jobs           │  │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘  │
│         │                │                   │           │
│         └────────────────┼───────────────────┘           │
│                          ▼                               │
│  ┌───────────────────────────────────────────────────┐  │
│  │              Shared AppState                       │  │
│  │  - CatalogStore      - SearchVault                 │  │
│  │  - UserStore         - SessionManager              │  │
│  │  - ServerStore       - JobScheduler                │  │
│  │  - DownloadManager   - Config                      │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

The MCP server runs in the same process as the main HTTP server, sharing `AppState`.

## Transport

**HTTP/SSE** on a separate port (default: 3002)

- `GET /sse` - Server-sent events stream for responses/notifications
- `POST /message` - Client sends tool calls and requests

### Docker Compose exposure

```yaml
services:
  catalog-server:
    ports:
      - "3001:3001"              # Main API (public)
      - "127.0.0.1:3002:3002"    # MCP (localhost only)
```

### Security

- Bind to localhost only by default
- **Authentication required** - MCP clients must authenticate as a Pezzottify user
- **New permission: `McpAccess`** - Required to connect to MCP at all
- **Existing permissions apply** - Tools are gated by the same permissions as HTTP API
- MCP session = regular user session with same permission checks

### Authentication Flow

1. MCP client connects to SSE endpoint
2. Server sends `auth_required` event with challenge
3. Client responds with credentials (username + password, or existing session token)
4. Server validates credentials, checks `McpAccess` permission
5. On success, server sends `authenticated` event, MCP session begins
6. All subsequent tool calls are executed with that user's permissions

Alternative: Support the existing challenge-response auth flow used by the Android app.

### Session Lifecycle

MCP sessions are tied to the SSE connection. When the connection closes, the session ends. No idle timeout needed.

### Rate Limiting

Per-user rate limits to prevent abuse (buggy LLM loops, malicious users):

| Category | Limit | Window |
|----------|-------|--------|
| Read operations | 120 requests | per minute |
| Write operations | 30 requests | per minute |
| `query_sql` | 10 requests | per minute |

Rate limit responses return standard MCP error with retry-after hint. Limits apply per authenticated user, not per connection (so multiple MCP clients from same user share the limit).

## Configuration

### CLI arguments

```
--mcp-port <PORT>        MCP server port (default: 3002, 0 to disable)
```

### Config file (config.toml)

```toml
[mcp]
enabled = true
port = 3002
```

## Permission Model

MCP tools are gated by the same permissions as the HTTP API. The `McpAccess` permission is required to connect at all, then each tool requires additional permissions.

### Permission → Tool Mapping

| Permission | Tools Available |
|------------|-----------------|
| `McpAccess` | (required to connect) |
| `AccessCatalog` | `search_catalog`, `get_artist`, `get_album`, `get_track`, `list_recent_additions`, `get_catalog_stats` |
| `EditCatalog` | `create_artist`, `update_artist`, `delete_artist`, `create_album`, `update_album`, `delete_album`, `create_track`, `update_track`, `delete_track`, `check_integrity` |
| `ManagePermissions` | `list_users`, `get_user`, `create_user`, `delete_user`, `set_user_role`, `grant_permission`, `revoke_permission` |
| `ViewAnalytics` | `get_listening_stats`, `get_bandwidth_stats`, `get_popular_content`, `get_download_stats`, `get_download_queue` |
| `RequestContent` | `search_external`, `request_download` |
| `IssueContentDownload` | `retry_failed`, `cancel_download` |
| `ServerAdmin` | `get_server_info`, `get_memory_stats`, `list_active_sessions`, `get_recent_requests`, `list_jobs`, `get_job_history`, `trigger_job`, `enable_job`, `disable_job`, `reload_search_index`, `clear_cache`, `get_config`, `query_sql`, `inspect_search_index`, `get_recent_errors` |

### Tool Availability

When a user connects via MCP, they only see tools they have permission to use. The MCP `tools/list` response is dynamically filtered based on the authenticated user's permissions.

This means:
- A regular user with just `AccessCatalog` + `McpAccess` sees only catalog read tools
- An admin sees everything
- Tools can require multiple permissions (e.g., `check_integrity` needs both `EditCatalog` and `ServerAdmin`)

## MCP Tools

### Server Introspection

| Tool | Description | Returns |
|------|-------------|---------|
| `get_server_info` | Server version, uptime, config summary | ServerInfo |
| `get_memory_stats` | Cache sizes, search index stats, connection counts | MemoryStats |
| `list_active_sessions` | Currently authenticated sessions | Session[] |
| `get_recent_requests` | Recent HTTP requests (if logging enabled) | RequestLog[] |

### Catalog (Read)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `search_catalog` | Search using live search index | `query: string`, `limit?: number` | SearchResult[] |
| `get_artist` | Artist details with albums | `id: string` | Artist |
| `get_album` | Album details with tracks | `id: string` | Album |
| `get_track` | Track details | `id: string` | Track |
| `list_recent_additions` | Recently added content | `days?: number`, `limit?: number` | CatalogItem[] |
| `get_catalog_stats` | Total counts, storage size | | CatalogStats |

### Catalog (Write)

| Tool | Description | Parameters |
|------|-------------|------------|
| `create_artist` | Add new artist | `name: string`, `image_id?: string` |
| `update_artist` | Modify artist | `id: string`, `name?: string`, ... |
| `delete_artist` | Remove artist | `id: string` |
| `create_album` | Add new album | `title: string`, `artist_id: string`, ... |
| `update_album` | Modify album | `id: string`, ... |
| `delete_album` | Remove album | `id: string` |
| `create_track` | Add new track | `title: string`, `album_id: string`, ... |
| `update_track` | Modify track | `id: string`, ... |
| `delete_track` | Remove track | `id: string` |

### Users

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `list_users` | All users with basic info | `include_activity?: bool` | User[] |
| `get_user` | User details, permissions, activity | `handle: string` | UserDetails |
| `create_user` | Create new user | `handle: string`, `password: string`, `role: string` | User |
| `delete_user` | Remove user | `handle: string` | |
| `set_user_role` | Change user role | `handle: string`, `role: string` | |
| `grant_permission` | Add permission to user | `handle: string`, `permission: string`, `count?: number` | |
| `revoke_permission` | Remove permission | `handle: string`, `permission: string` | |

### Background Jobs

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `list_jobs` | All background jobs with status | | Job[] |
| `get_job_history` | Execution history for job | `job_id: string`, `limit?: number` | JobRun[] |
| `trigger_job` | Run a job immediately | `job_id: string` | |
| `enable_job` | Enable a disabled job | `job_id: string` | |
| `disable_job` | Disable a job | `job_id: string` | |

### Download Manager

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_download_queue` | Pending downloads | | QueueItem[] |
| `get_download_stats` | Success/failure rates, recent activity | | DownloadStats |
| `search_external` | Search external provider | `query: string`, `type: string` | ExternalResult[] |
| `request_download` | Queue album/discography | `type: string`, `external_id: string` | |
| `retry_failed` | Retry a failed download | `item_id: string` | |
| `cancel_download` | Remove from queue | `item_id: string` | |

### Analytics

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_listening_stats` | Listening history aggregates | `period?: string`, `user?: string` | ListeningStats |
| `get_bandwidth_stats` | Streaming bandwidth usage | `period?: string` | BandwidthStats |
| `get_popular_content` | Most played content | `type: string`, `limit?: number` | PopularItem[] |

### Debug

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `query_sql` | Execute read-only SQL (uses read-only DB connection) | `db: string`, `query: string` | Row[] |
| `inspect_search_index` | Debug search ranking | `query: string` | SearchDebug |
| `get_recent_errors` | Recent error logs | `limit?: number` | ErrorLog[] |
| `check_integrity` | Verify catalog/file consistency | `fix?: bool` | IntegrityReport |

**Note on `query_sql`**:
- Uses a dedicated read-only SQLite connection (`SQLITE_OPEN_READONLY` flag) to guarantee no writes can occur
- Results capped at 100 rows - use `LIMIT`/`OFFSET` in your SQL for larger datasets

### Server Control

| Tool | Description | Parameters |
|------|-------------|------------|
| `reload_search_index` | Rebuild search index from DB | |
| `clear_cache` | Invalidate specified caches | `cache: string` |
| `get_config` | Current server configuration | |

## MCP Resources

Resources provide read access to data that changes over time:

| Resource | URI Pattern | Description |
|----------|-------------|-------------|
| Server logs | `logs://recent` | Recent log entries |
| Job output | `jobs://{job_id}/output` | Latest job run output |
| Catalog changelog | `changelog://recent` | Recent catalog changes |

## Implementation Plan

### Phase 1: Foundation

1. Add `rmcp` crate dependency
2. Create `mcp/` module structure
3. Implement HTTP/SSE transport
4. Add config options (`--mcp-port`, etc.)
5. Wire up to AppState

### Phase 2: Read-only tools

1. Server introspection tools
2. Catalog read tools
3. User list/get tools
4. Job status tools
5. Analytics tools

### Phase 3: Debug tools

1. SQL query tool (read-only)
2. Search index inspection
3. Error log access
4. Integrity checking

### Phase 4: Write tools

1. Catalog CRUD
2. User management
3. Job control
4. Download manager operations

### Phase 5: Future - Catalog Agent

1. Define agent role/permissions
2. Implement agent-specific tools (or reuse existing)
3. Add rate limiting / quotas for automated access
4. Audit logging for agent actions

## Design Decisions

1. **Tool confirmation** - No server-level confirmation for destructive operations. MCP is a protocol, not a UI. The LLM client (Claude, etc.) is responsible for confirmation UX.

2. **Notifications** - Start without push notifications. Keep it purely request/response. Polling via tools is simpler. Can add `subscribe_to_events` later if needed.

3. **Audit logging** - Yes, comprehensive audit logging for all MCP interactions:
   - Stored in a separate database file (`mcp_audit.db`) to avoid bloating `server.db`
   - Logs full request/response including LLM prompts, responses, and reasoning
   - Critical for debugging catalog agent behavior and tracking automated changes

## References

- [MCP Specification](https://modelcontextprotocol.io/specification)
- [rmcp crate](https://crates.io/crates/rmcp) (Rust MCP implementation)
