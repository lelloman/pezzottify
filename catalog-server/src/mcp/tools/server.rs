//! Server Tools
//!
//! Tools for querying server status and information.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::user::Permission;

/// Register server tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(server_query_tool());
}

// ============================================================================
// server.query
// ============================================================================

#[derive(Debug, Deserialize)]
struct ServerQueryParams {
    query_type: ServerQueryType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ServerQueryType {
    Info,
    Stats,
}

#[derive(Debug, Serialize)]
struct ServerInfoResult {
    version: String,
    uptime_secs: u64,
    catalog: CatalogStats,
}

#[derive(Debug, Serialize)]
struct CatalogStats {
    artists: usize,
    albums: usize,
    tracks: usize,
}

#[derive(Debug, Serialize)]
struct ServerStatsResult {
    version: String,
    uptime_secs: u64,
    catalog: CatalogStats,
    users: UserStats,
}

#[derive(Debug, Serialize)]
struct UserStats {
    total_users: usize,
}

fn server_query_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("server.query")
        .description("Get server information, status, and statistics")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query_type": {
                    "type": "string",
                    "enum": ["info", "stats"],
                    "description": "Type of query: 'info' for basic info, 'stats' for detailed statistics"
                }
            },
            "required": ["query_type"]
        }))
        .permission(Permission::ServerAdmin)
        .category(ToolCategory::Read)
        .build(server_query_handler)
}

async fn server_query_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: ServerQueryParams =
        serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.query_type {
        ServerQueryType::Info => get_server_info(&ctx).await,
        ServerQueryType::Stats => get_server_stats(&ctx).await,
    }
}

async fn get_server_info(ctx: &ToolContext) -> ToolResult {
    let uptime_secs = ctx.start_time.elapsed().as_secs();

    let catalog_stats = CatalogStats {
        artists: ctx.catalog_store.get_artists_count(),
        albums: ctx.catalog_store.get_albums_count(),
        tracks: ctx.catalog_store.get_tracks_count(),
    };

    let result = ServerInfoResult {
        version: ctx.server_version.clone(),
        uptime_secs,
        catalog: catalog_stats,
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_server_stats(ctx: &ToolContext) -> ToolResult {
    let uptime_secs = ctx.start_time.elapsed().as_secs();

    let catalog_stats = CatalogStats {
        artists: ctx.catalog_store.get_artists_count(),
        albums: ctx.catalog_store.get_albums_count(),
        tracks: ctx.catalog_store.get_tracks_count(),
    };

    let user_manager = ctx.user_manager.lock().unwrap();
    let total_users = user_manager
        .get_all_user_handles()
        .map(|h| h.len())
        .unwrap_or(0);

    let result = ServerStatsResult {
        version: ctx.server_version.clone(),
        uptime_secs,
        catalog: catalog_stats,
        users: UserStats { total_users },
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}
