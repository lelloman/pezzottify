//! Catalog Tools
//!
//! Tools for searching and reading catalog data.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::search::HashedItemType;
use crate::user::Permission;

/// Register catalog tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(catalog_search_tool());
    registry.register_tool(catalog_get_tool());
    registry.register_tool(catalog_mutate_tool());
}

// ============================================================================
// catalog.search
// ============================================================================

#[derive(Debug, Deserialize)]
struct CatalogSearchParams {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Serialize)]
struct CatalogSearchResult {
    artists: Vec<SearchResultItem>,
    albums: Vec<SearchResultItem>,
    tracks: Vec<SearchResultItem>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct SearchResultItem {
    id: String,
    name: String,
}

fn catalog_search_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("catalog.search")
        .description("Search the music catalog for artists, albums, and tracks")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results per category (default 20)",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["query"]
        }))
        .permission(Permission::AccessCatalog)
        .category(ToolCategory::Read)
        .build(catalog_search_handler)
}

async fn catalog_search_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: CatalogSearchParams =
        serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let limit = params.limit.min(100);

    // Perform search using the search vault
    let results = ctx.search_vault.search(&params.query, limit * 3, None);

    // Convert results to our format, grouped by type
    let mut artists = Vec::new();
    let mut albums = Vec::new();
    let mut tracks = Vec::new();

    for result in results {
        let item = SearchResultItem {
            id: result.item_id.clone(),
            name: result.matchable_text.clone(),
        };

        match result.item_type {
            HashedItemType::Artist => {
                if artists.len() < limit {
                    artists.push(item)
                }
            }
            HashedItemType::Album => {
                if albums.len() < limit {
                    albums.push(item)
                }
            }
            HashedItemType::Track => {
                if tracks.len() < limit {
                    tracks.push(item)
                }
            }
        }
    }

    let total = artists.len() + albums.len() + tracks.len();

    let result = CatalogSearchResult {
        artists,
        albums,
        tracks,
        total,
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

// ============================================================================
// catalog.get
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CatalogGetParams {
    query_type: CatalogQueryType,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CatalogQueryType {
    Artist,
    Album,
    Track,
    Stats,
}

fn catalog_get_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("catalog.get")
        .description("Get catalog content by type and ID, or get statistics")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query_type": {
                    "type": "string",
                    "enum": ["artist", "album", "track", "stats"],
                    "description": "Type of query to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Content ID (required for artist/album/track queries)"
                }
            },
            "required": ["query_type"]
        }))
        .permission(Permission::AccessCatalog)
        .category(ToolCategory::Read)
        .build(catalog_get_handler)
}

async fn catalog_get_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: CatalogGetParams =
        serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.query_type {
        CatalogQueryType::Artist => {
            let id = params
                .id
                .ok_or_else(|| McpError::InvalidParams("id is required for artist query".into()))?;
            get_artist(&ctx, &id).await
        }
        CatalogQueryType::Album => {
            let id = params
                .id
                .ok_or_else(|| McpError::InvalidParams("id is required for album query".into()))?;
            get_album(&ctx, &id).await
        }
        CatalogQueryType::Track => {
            let id = params
                .id
                .ok_or_else(|| McpError::InvalidParams("id is required for track query".into()))?;
            get_track(&ctx, &id).await
        }
        CatalogQueryType::Stats => get_stats(&ctx).await,
    }
}

async fn get_artist(ctx: &ToolContext, id: &str) -> ToolResult {
    let resolved = ctx
        .catalog_store
        .get_resolved_artist(id)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Artist not found: {}", id)))?;

    // Get discography (first page, sorted by popularity)
    let discography = ctx
        .catalog_store
        .get_discography(id, 20, 0, crate::catalog_store::DiscographySort::Popularity)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "artist": {
            "id": resolved.artist.id,
            "name": resolved.artist.name,
            "genres": resolved.artist.genres,
            "followers_total": resolved.artist.followers_total,
            "popularity": resolved.artist.popularity,
        },
        "albums": discography.as_ref().map(|d| d.albums.iter().map(|a| serde_json::json!({
            "id": a.id,
            "name": a.name,
            "album_type": format!("{:?}", a.album_type),
            "release_date": a.release_date,
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "album_count": discography.as_ref().map(|d| d.total).unwrap_or(0),
        "related_artists": resolved.related_artists.iter().take(10).map(|a| serde_json::json!({
            "id": a.id,
            "name": a.name,
        })).collect::<Vec<_>>(),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_album(ctx: &ToolContext, id: &str) -> ToolResult {
    let resolved = ctx
        .catalog_store
        .get_resolved_album(id)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Album not found: {}", id)))?;

    let result = serde_json::json!({
        "album": {
            "id": resolved.album.id,
            "name": resolved.album.name,
            "album_type": format!("{:?}", resolved.album.album_type),
            "release_date": resolved.album.release_date,
            "label": resolved.album.label,
            "popularity": resolved.album.popularity,
        },
        "artists": resolved.artists.iter().map(|a| serde_json::json!({
            "id": a.id,
            "name": a.name,
        })).collect::<Vec<_>>(),
        "discs": resolved.discs.iter().map(|disc| serde_json::json!({
            "number": disc.number,
            "tracks": disc.tracks.iter().map(|t| serde_json::json!({
                "id": t.id,
                "name": t.name,
                "track_number": t.track_number,
                "duration_ms": t.duration_ms,
                "explicit": t.explicit,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "track_count": resolved.discs.iter().map(|d| d.tracks.len()).sum::<usize>(),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_track(ctx: &ToolContext, id: &str) -> ToolResult {
    let resolved = ctx
        .catalog_store
        .get_resolved_track(id)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Track not found: {}", id)))?;

    let result = serde_json::json!({
        "track": {
            "id": resolved.track.id,
            "name": resolved.track.name,
            "disc_number": resolved.track.disc_number,
            "track_number": resolved.track.track_number,
            "duration_ms": resolved.track.duration_ms,
            "explicit": resolved.track.explicit,
            "popularity": resolved.track.popularity,
            "language": resolved.track.language,
        },
        "album": {
            "id": resolved.album.id,
            "name": resolved.album.name,
        },
        "artists": resolved.artists.iter().map(|ta| serde_json::json!({
            "id": ta.artist.id,
            "name": ta.artist.name,
            "role": format!("{:?}", ta.role),
        })).collect::<Vec<_>>(),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_stats(ctx: &ToolContext) -> ToolResult {
    let artist_count = ctx.catalog_store.get_artists_count();
    let album_count = ctx.catalog_store.get_albums_count();
    let track_count = ctx.catalog_store.get_tracks_count();

    let result = serde_json::json!({
        "stats": {
            "artists": artist_count,
            "albums": album_count,
            "tracks": track_count,
        }
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

// ============================================================================
// catalog.mutate
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CatalogMutateParams {
    action: CatalogMutateAction,
    entity_type: CatalogEntityType,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    data: Option<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CatalogMutateAction {
    Create,
    Update,
    Delete,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CatalogEntityType {
    Artist,
    Album,
    Track,
    Image,
}

fn catalog_mutate_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("catalog.mutate")
        .description("Create, update, or delete catalog content. CONFIRMATION REQUIRED before executing.")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "delete"],
                    "description": "Action to perform"
                },
                "entity_type": {
                    "type": "string",
                    "enum": ["artist", "album", "track", "image"],
                    "description": "Type of entity to mutate"
                },
                "id": {
                    "type": "string",
                    "description": "Entity ID (required for update/delete)"
                },
                "data": {
                    "type": "object",
                    "description": "Entity data (required for create/update). Structure depends on entity_type."
                }
            },
            "required": ["action", "entity_type"]
        }))
        .permission(Permission::EditCatalog)
        .category(ToolCategory::Write)
        .build(catalog_mutate_handler)
}

async fn catalog_mutate_handler(_ctx: ToolContext, _params: Value) -> ToolResult {
    // Catalog mutations disabled - Spotify schema is read-only
    Err(McpError::ToolExecutionFailed(
        "Catalog mutations not available - Spotify catalog is read-only".to_string(),
    ))
}

// NOTE: create_entity, update_entity, delete_entity removed - Spotify catalog is read-only
