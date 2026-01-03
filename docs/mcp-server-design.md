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
- Hosting an LLM - clients bring their own

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         LLM Client                                        │
│  (Claude Desktop, custom app with Anthropic API, etc.)                   │
│                                                                           │
│  ┌──────────┐    ┌──────────────┐    ┌────────────────────────────────┐ │
│  │   User   │───►│     LLM      │───►│  MCP Client (WebSocket)        │ │
│  │  Prompt  │    │ (Claude API) │    │                                │ │
│  └──────────┘    └──────────────┘    └───────────────┬────────────────┘ │
└──────────────────────────────────────────────────────┼──────────────────┘
                                                       │
                                              WebSocket│ /v1/mcp
                                                       ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          catalog-server                                   │
│                                                                           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │    HTTP API     │  │   MCP Server    │  │    Background Jobs      │  │
│  │  /v1/*          │  │   /v1/mcp (WS)  │  │                         │  │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘  │
│           │                    │                        │                │
│           └────────────────────┼────────────────────────┘                │
│                                ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                       Shared AppState                               │  │
│  │  - CatalogStore      - SearchVault        - JobScheduler           │  │
│  │  - UserStore         - SessionManager     - DownloadManager        │  │
│  │  - ServerStore       - Config                                      │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

**Key points:**
- MCP server runs **in-process** with the main HTTP server, sharing `AppState`
- No separate port - MCP is a WebSocket endpoint on the main server
- No LLM on the server - we provide tools only, clients bring their own LLM

---

## Transport

**WebSocket** at `/v1/mcp` on the main server port (3001).

### Why WebSocket over HTTP/SSE?

SSE (Server-Sent Events) requires two channels:
- `GET /sse` for server → client
- `POST /message` for client → server

This is essentially "WebSocket with extra steps." Since we already have WebSocket infrastructure, a single bidirectional WS connection is simpler.

### Why `/v1/mcp` instead of extending `/v1/ws`?

The existing `/v1/ws` serves user sync (likes, settings, playlists, permissions). Mixing MCP traffic with user sync would:
- Complicate message routing
- Mix different audiences (end-users vs LLM agents)
- Create protocol mismatches (sync is push-based, MCP is request/response)

A dedicated endpoint keeps concerns separated while still reusing existing auth infrastructure.

---

## Authentication & Permissions

### Same Auth as HTTP API

MCP uses the **same authentication** as the HTTP API:
- Existing user accounts
- Same session/token mechanism
- No new `McpAccess` permission needed

### Permission-Based Tool Filtering

Tools are **gated by existing permissions**. When a user connects via MCP, they only see tools they have permission to use.

| Permission | Tools Available |
|------------|-----------------|
| `AccessCatalog` | `catalog.search`, `catalog.get` |
| `EditCatalog` | `catalog.mutate` |
| `ManagePermissions` | `users.query`, `users.mutate` |
| `ViewAnalytics` | `analytics.query`, `downloads.query` |
| `RequestContent` | `downloads.action` |
| `ServerAdmin` | `server.query`, `jobs.query`, `jobs.action`, `debug.sql`, `debug.inspect` |

**Examples:**
- A regular user with just `AccessCatalog` sees **2 tools**
- An admin with all permissions sees **13 tools**

---

## Rate Limiting

Per-user limits to prevent abuse (buggy LLM loops, malicious clients):

| Category | Limit | Window |
|----------|-------|--------|
| Read operations | 120 requests | per minute |
| Write operations | 30 requests | per minute |
| `debug.sql` | 10 requests | per minute |

Limits apply per authenticated user, not per connection (multiple MCP clients from same user share the limit).

---

## MCP Features

### v1 Scope

| Feature | Supported | Notes |
|---------|-----------|-------|
| **Tools** | Yes | Core feature - functions the LLM can call |
| **Resources** | Yes | Read-only data access (logs, job output, config) |
| **Prompts** | No | Not needed for our use case |
| **Sampling** | No | Server doesn't call the LLM |
| **Roots** | No | Not applicable |

---

## Tools (Consolidated)

To avoid context bloat, tools are **consolidated** into ~13 tools instead of 40+ granular ones. This keeps the LLM context under control (target: 32K baseline).

### Tool List

| Tool | Permission | Description |
|------|------------|-------------|
| `catalog.search` | AccessCatalog | Search artists, albums, tracks |
| `catalog.get` | AccessCatalog | Get artist/album/track details, recent additions, stats |
| `catalog.mutate` | EditCatalog | Create/update/delete artists, albums, tracks |
| `users.query` | ManagePermissions | List users, get user details |
| `users.mutate` | ManagePermissions | Create/delete users, set role, grant/revoke permissions |
| `analytics.query` | ViewAnalytics | Listening stats, bandwidth stats, popular content |
| `downloads.query` | ViewAnalytics | Download queue stats, queue state |
| `downloads.action` | RequestContent | Request download, retry failed, cancel pending |
| `server.query` | ServerAdmin | Server info, memory stats, active sessions, config |
| `jobs.query` | ServerAdmin | List jobs, job history |
| `jobs.action` | ServerAdmin | Trigger job, enable/disable job |
| `debug.sql` | ServerAdmin | Execute read-only SQL queries |
| `debug.inspect` | ServerAdmin | Search index inspection, recent errors, integrity check |

### Tool Design Principles

1. **Consolidate related operations** - One tool with parameters vs many similar tools
2. **Paginate by default** - Return limited results with `total` and `has_more`
3. **Keep responses concise** - Summarize where possible, let LLM ask for details
4. **Use enums for actions** - `catalog.mutate(action: "create" | "update" | "delete", ...)`

### Example: `catalog.get`

Instead of separate `get_artist`, `get_album`, `get_track`, `get_recent`, `get_stats` tools:

```json
{
  "name": "catalog.get",
  "description": "Get catalog content by type and ID, or get aggregate data",
  "parameters": {
    "type": "object",
    "properties": {
      "query_type": {
        "type": "string",
        "enum": ["artist", "album", "track", "recent", "stats"]
      },
      "id": {
        "type": "string",
        "description": "Required for artist/album/track queries"
      },
      "limit": {
        "type": "integer",
        "description": "For 'recent' query, default 20"
      }
    },
    "required": ["query_type"]
  }
}
```

---

## Resources

Resources provide read-only access to data that changes over time. Unlike tools, resources are accessed on-demand and don't clutter the tool list.

| URI Pattern | Permission | Description |
|-------------|------------|-------------|
| `logs://recent` | ServerAdmin | Recent server log entries |
| `logs://errors` | ServerAdmin | Recent error logs |
| `jobs://{job_id}/output` | ServerAdmin | Latest job run output |
| `config://server` | ServerAdmin | Full server configuration |
| `changelog://recent` | EditCatalog | Recent catalog changes |

---

## Implementation

### Library Choice: Roll Our Own

We evaluated existing Rust MCP libraries:
- **[rmcp](https://github.com/modelcontextprotocol/rust-sdk)** - Official SDK, but focused on stdio transport
- Various community crates - varying quality and transport support

**Decision:** Roll our own implementation because:
- MCP is essentially JSON-RPC with specific message types (not complex)
- We need WebSocket transport (not stdio)
- Tight integration with our existing Axum auth is cleaner
- No external dependencies to manage

### Tool Definition Architecture

**Function-based with explicit registration:**

```rust
// Tools are async functions
async fn catalog_search(
    ctx: &ToolContext,
    query: String,
    limit: Option<u32>,
) -> Result<SearchResults, ToolError> {
    let results = ctx.search_vault.search(&query, limit.unwrap_or(20)).await?;
    Ok(results)
}

// Registered with metadata
registry.register(ToolDef {
    name: "catalog.search",
    description: "Search the music catalog for artists, albums, and tracks",
    permissions: &[Permission::AccessCatalog],
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "Search query" },
            "limit": { "type": "integer", "description": "Max results (default 20)" }
        },
        "required": ["query"]
    }),
    handler: catalog_search,
});
```

**Why this approach:**
- No macro magic - easy to understand and debug
- Explicit registration makes the tool list discoverable
- Parameters are strongly typed in the handler
- Permissions are declared alongside the tool

### Module Structure

```
catalog-server/src/
├── mcp/
│   ├── mod.rs              # Public API, McpServer struct
│   ├── protocol.rs         # MCP message types (JSON-RPC based)
│   ├── transport.rs        # WebSocket handling
│   ├── registry.rs         # Tool/resource registration
│   ├── context.rs          # ToolContext with shared state
│   ├── rate_limit.rs       # Per-user rate limiting
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── catalog.rs      # catalog.search, catalog.get, catalog.mutate
│   │   ├── users.rs        # users.query, users.mutate
│   │   ├── analytics.rs    # analytics.query
│   │   ├── downloads.rs    # downloads.query, downloads.action
│   │   ├── server.rs       # server.query
│   │   ├── jobs.rs         # jobs.query, jobs.action
│   │   └── debug.rs        # debug.sql, debug.inspect
│   └── resources/
│       ├── mod.rs
│       ├── logs.rs         # logs://recent, logs://errors
│       ├── jobs.rs         # jobs://{id}/output
│       └── config.rs       # config://server
```

---

## Implementation Phases

### Phase 1: Foundation

1. Create `mcp/` module structure
2. Define MCP protocol message types
3. Implement WebSocket transport at `/v1/mcp`
4. Add authentication (reuse existing session middleware)
5. Implement tool/resource registry
6. Add rate limiting

### Phase 2: Read-only Tools

1. `catalog.search` - Search catalog
2. `catalog.get` - Get content by type/ID
3. `server.query` - Server info, memory, sessions
4. `jobs.query` - Job list and history
5. `analytics.query` - Listening/bandwidth stats

### Phase 3: Resources

1. `logs://recent` and `logs://errors`
2. `jobs://{job_id}/output`
3. `config://server`
4. `changelog://recent`

### Phase 4: Write Tools

1. `catalog.mutate` - CRUD for catalog content
2. `users.query` and `users.mutate` - User management
3. `jobs.action` - Trigger/enable/disable jobs
4. `downloads.query` and `downloads.action` - Download management

### Phase 5: Debug Tools

1. `debug.sql` - Read-only SQL queries
2. `debug.inspect` - Search index, errors, integrity

### Phase 6: Future - Catalog Agent

1. Define agent role/permissions
2. Add rate limiting / quotas for automated access
3. Audit logging for agent actions
4. Agent-specific tools if needed

---

## Design Decisions

### No Separate Port

Originally planned for port 3002, but `/v1/mcp` on the main port is simpler:
- One port to manage/expose
- Reuses existing TLS/proxy configuration
- Auth middleware is already there

### No `McpAccess` Permission

Originally planned a separate permission to connect to MCP. Removed because:
- Existing permissions already gate all tools
- A user with no permissions sees no tools anyway
- Simpler permission model

### Tool Confirmation

No server-level confirmation for destructive operations. MCP is a protocol, not a UI. The LLM client (Claude, etc.) is responsible for confirmation UX.

### Audit Logging

Comprehensive audit logging for all MCP interactions:
- Stored in `server.db` (or separate `mcp_audit.db` if volume is high)
- Logs tool calls, parameters, results
- Critical for debugging catalog agent behavior

### Context Budget

Tools designed for **32K context baseline**:
- Consolidated tools (~13 instead of 40+)
- Paginated responses by default
- Concise result formats
- Clients with larger context can request more detail

---

## Configuration

### CLI Arguments

```
--mcp-enabled            Enable MCP server (default: true if feature enabled)
```

### Config File (config.toml)

```toml
[mcp]
enabled = true

[mcp.rate_limit]
read_per_minute = 120
write_per_minute = 30
sql_per_minute = 10
```

---

## Web Client Integration

The web frontend includes an AI chat assistant that uses both MCP tools and local UI tools:

**Location:** `web/src/components/chat/`, `web/src/services/llm/`, `web/src/services/mcp.js`, `web/src/services/uiTools.js`

**Architecture:**
- LLM calls are made directly from the browser (no backend proxy)
- Supports multiple providers: Anthropic, OpenAI, Google, Ollama, OpenRouter
- MCP client connects via WebSocket to `/v1/mcp` for catalog operations
- UI tools execute locally for playback, navigation, likes, playlists

**Tool Sources:**
1. **MCP tools** (from server): `catalog.search`, `catalog.get`, etc.
2. **UI tools** (local): `ui.play`, `ui.pause`, `ui.navigate`, `ui.likeAlbum`, etc.

**Configuration:**
- Provider settings stored in localStorage (`ai_chat_config`)
- For Ollama, users must set `OLLAMA_ORIGINS` to allow cross-origin requests

---

## References

- [MCP Specification](https://modelcontextprotocol.io/specification)
- [Official Rust SDK (rmcp)](https://github.com/modelcontextprotocol/rust-sdk) - Reference only, we roll our own
