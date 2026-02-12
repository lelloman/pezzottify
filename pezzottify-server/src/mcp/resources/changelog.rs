//! Changelog Resources
//!
//! Resources for accessing catalog changelog data.
//! NOTE: Disabled for Spotify schema - catalog is read-only.

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::ResourceContent;
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

async fn changelog_recent_handler(_ctx: ToolContext, uri: String) -> ResourceResult {
    // Changelog functionality disabled - Spotify schema is read-only
    let output = serde_json::json!({
        "message": "Changelog not available for Spotify catalog (read-only)",
        "active_batch": null,
        "recent_batches": [],
    });

    let content = ResourceContent::Text {
        uri,
        mime_type: Some("application/json".to_string()),
        text: serde_json::to_string_pretty(&output).unwrap_or_default(),
    };

    Ok(vec![content])
}
