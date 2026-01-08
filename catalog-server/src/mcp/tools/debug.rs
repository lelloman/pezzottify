//! Debug Tools
//!
//! Tools for inspecting server internals and debugging.

use serde::Deserialize;
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::user::Permission;

/// Register debug tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(debug_inspect_tool());
}

// ============================================================================
// debug.inspect
// ============================================================================

#[derive(Debug, Deserialize)]
struct DebugInspectParams {
    target: InspectTarget,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum InspectTarget {
    /// Server overview: uptime, version, catalog counts
    Overview,
    /// Catalog statistics: entity counts, integrity info
    Catalog,
    /// Search index statistics
    Search,
    /// Download manager statistics (if enabled)
    Downloads,
    /// All available debug info
    All,
}

fn debug_inspect_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("debug.inspect")
        .description("Inspect server internals for debugging: overview, catalog stats, search index, download manager")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["overview", "catalog", "search", "downloads", "all"],
                    "description": "What to inspect: 'overview' for server status, 'catalog' for entity counts, 'search' for index stats, 'downloads' for download manager, 'all' for everything"
                }
            },
            "required": ["target"]
        }))
        .permission(Permission::ServerAdmin)
        .category(ToolCategory::Read)
        .build(debug_inspect_handler)
}

async fn debug_inspect_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: DebugInspectParams =
        serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.target {
        InspectTarget::Overview => inspect_overview(&ctx).await,
        InspectTarget::Catalog => inspect_catalog(&ctx).await,
        InspectTarget::Search => inspect_search(&ctx).await,
        InspectTarget::Downloads => inspect_downloads(&ctx).await,
        InspectTarget::All => inspect_all(&ctx).await,
    }
}

async fn inspect_overview(ctx: &ToolContext) -> ToolResult {
    let uptime_secs = ctx.start_time.elapsed().as_secs();
    let uptime_formatted = format_uptime(uptime_secs);

    let result = serde_json::json!({
        "server": {
            "version": ctx.server_version,
            "uptime_seconds": uptime_secs,
            "uptime_formatted": uptime_formatted,
        },
        "user": {
            "id": ctx.user_id(),
            "session_permissions": ctx.session.permissions.iter()
                .map(|p| format!("{:?}", p))
                .collect::<Vec<_>>(),
        },
        "features": {
            "scheduler_enabled": ctx.scheduler_handle.is_some(),
        },
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn inspect_catalog(ctx: &ToolContext) -> ToolResult {
    let artists_count = ctx.catalog_store.get_artists_count();
    let albums_count = ctx.catalog_store.get_albums_count();
    let tracks_count = ctx.catalog_store.get_tracks_count();

    let result = serde_json::json!({
        "counts": {
            "artists": artists_count,
            "albums": albums_count,
            "tracks": tracks_count,
        },
        "note": "Spotify catalog is read-only - integrity checks not available",
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn inspect_search(ctx: &ToolContext) -> ToolResult {
    let stats = ctx.search_vault.get_stats();

    let result = serde_json::json!({
        "search_index": {
            "indexed_items": stats.indexed_items,
            "index_type": stats.index_type,
        },
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn inspect_downloads(_ctx: &ToolContext) -> ToolResult {
    // Download manager disabled for Spotify schema
    let result = serde_json::json!({
        "enabled": false,
        "message": "Download manager not available for Spotify catalog",
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn inspect_all(ctx: &ToolContext) -> ToolResult {
    // Gather all sections
    let uptime_secs = ctx.start_time.elapsed().as_secs();
    let uptime_formatted = format_uptime(uptime_secs);

    let artists_count = ctx.catalog_store.get_artists_count();
    let albums_count = ctx.catalog_store.get_albums_count();
    let tracks_count = ctx.catalog_store.get_tracks_count();

    let search_stats = ctx.search_vault.get_stats();

    let result = serde_json::json!({
        "server": {
            "version": ctx.server_version,
            "uptime_seconds": uptime_secs,
            "uptime_formatted": uptime_formatted,
        },
        "features": {
            "scheduler_enabled": ctx.scheduler_handle.is_some(),
        },
        "catalog": {
            "counts": {
                "artists": artists_count,
                "albums": albums_count,
                "tracks": tracks_count,
            },
            "note": "Spotify catalog is read-only",
        },
        "search": {
            "indexed_items": search_stats.indexed_items,
            "index_type": search_stats.index_type,
        },
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}
