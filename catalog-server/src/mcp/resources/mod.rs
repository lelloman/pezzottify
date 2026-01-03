//! MCP Resources
//!
//! Resource implementations for logs, job output, config, etc.

use super::registry::McpRegistry;

/// Register all resources with the registry
pub fn register_all_resources(registry: &mut McpRegistry) {
    // Future: logs::register_resources(registry);
    // Future: jobs::register_resources(registry);
    // Future: config::register_resources(registry);
    // Future: changelog::register_resources(registry);
    let _ = registry; // Suppress unused warning for now
}
