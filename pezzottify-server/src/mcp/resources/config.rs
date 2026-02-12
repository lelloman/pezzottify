//! Config Resources
//!
//! Resources for accessing server configuration.

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::ResourceContent;
use crate::mcp::registry::{McpRegistry, ResourceBuilder, ResourceResult};
use crate::user::Permission;

/// Register config resources with the registry
pub fn register_resources(registry: &mut McpRegistry) {
    registry.register_resource(server_config_resource());
}

// ============================================================================
// config://server
// ============================================================================

fn server_config_resource() -> super::super::registry::RegisteredResource {
    ResourceBuilder::new("config://server", "Server Configuration")
        .description("Current server configuration settings (read-only view)")
        .mime_type("application/json")
        .permission(Permission::ServerAdmin)
        .build(server_config_handler)
}

async fn server_config_handler(ctx: ToolContext, uri: String) -> ResourceResult {
    let config = &ctx.config;

    // Build a sanitized view of the config (no sensitive data)
    let streaming = &config.streaming_search;
    let config_view = serde_json::json!({
        "server": {
            "port": config.port,
            "requests_logging_level": format!("{:?}", config.requests_logging_level),
            "content_cache_age_sec": config.content_cache_age_sec,
            "has_frontend": config.frontend_dir_path.is_some(),
            "disable_password_auth": config.disable_password_auth,
        },
        "streaming_search": {
            "strategy": format!("{:?}", streaming.strategy),
            "min_absolute_score": streaming.min_absolute_score,
            "min_score_gap_ratio": streaming.min_score_gap_ratio,
            "exact_match_boost": streaming.exact_match_boost,
            "popular_tracks_limit": streaming.popular_tracks_limit,
        },
        "runtime": {
            "version": ctx.server_version.clone(),
            "uptime_secs": ctx.start_time.elapsed().as_secs(),
        }
    });

    let content = ResourceContent::Text {
        uri,
        mime_type: Some("application/json".to_string()),
        text: serde_json::to_string_pretty(&config_view).unwrap_or_default(),
    };

    Ok(vec![content])
}
