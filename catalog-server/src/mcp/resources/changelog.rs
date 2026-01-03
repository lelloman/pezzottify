//! Changelog Resources
//!
//! Resources for accessing catalog changelog data.

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ResourceContent};
use crate::mcp::registry::{McpRegistry, ResourceBuilder, ResourceResult};
use crate::user::Permission;

/// Register changelog resources with the registry
pub fn register_resources(registry: &mut McpRegistry) {
    registry.register_resource(changelog_recent_resource());
}

// ============================================================================
// changelog://recent
// ============================================================================

fn changelog_recent_resource() -> super::super::registry::RegisteredResource {
    ResourceBuilder::new("changelog://recent", "Recent Catalog Changes")
        .description(
            "Recent catalog change batches showing what content was added, updated, or removed",
        )
        .mime_type("application/json")
        .permission(Permission::EditCatalog)
        .build(changelog_recent_handler)
}

async fn changelog_recent_handler(ctx: ToolContext, uri: String) -> ResourceResult {
    // Get recent closed batches with their summaries
    let batches = ctx
        .catalog_store
        .get_whats_new_batches(10)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    // Also get the active batch if any
    let active_batch = ctx
        .catalog_store
        .get_active_changelog_batch()
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let output = serde_json::json!({
        "active_batch": active_batch.map(|b| serde_json::json!({
            "id": b.id,
            "name": b.name,
            "description": b.description,
            "created_at": b.created_at,
            "last_activity_at": b.last_activity_at,
        })),
        "recent_batches": batches.iter().map(|b| serde_json::json!({
            "id": b.id,
            "name": b.name,
            "description": b.description,
            "closed_at": b.closed_at,
            "summary": {
                "artists": {
                    "added": b.summary.artists.added.len(),
                    "updated": b.summary.artists.updated_count,
                    "deleted": b.summary.artists.deleted.len(),
                },
                "albums": {
                    "added": b.summary.albums.added.len(),
                    "updated": b.summary.albums.updated_count,
                    "deleted": b.summary.albums.deleted.len(),
                },
                "tracks": {
                    "added": b.summary.tracks.added_count,
                    "updated": b.summary.tracks.updated_count,
                    "deleted": b.summary.tracks.deleted_count,
                },
            }
        })).collect::<Vec<_>>(),
    });

    let content = ResourceContent::Text {
        uri,
        mime_type: Some("application/json".to_string()),
        text: serde_json::to_string_pretty(&output).unwrap_or_default(),
    };

    Ok(vec![content])
}
