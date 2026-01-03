//! MCP Tools
//!
//! Tool implementations for catalog, users, analytics, etc.

pub mod analytics;
pub mod catalog;
pub mod jobs;
pub mod server;

use super::registry::McpRegistry;

/// Register all tools with the registry
pub fn register_all_tools(registry: &mut McpRegistry) {
    catalog::register_tools(registry);
    server::register_tools(registry);
    jobs::register_tools(registry);
    analytics::register_tools(registry);
    // Future: users::register_tools(registry);
    // Future: downloads::register_tools(registry);
    // Future: debug::register_tools(registry);
}
