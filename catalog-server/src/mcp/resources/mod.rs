//! MCP Resources
//!
//! Resource implementations for logs, job output, config, etc.

pub mod changelog;
pub mod config;
pub mod jobs;

use super::registry::McpRegistry;

/// Register all resources with the registry
pub fn register_all_resources(registry: &mut McpRegistry) {
    jobs::register_resources(registry);
    config::register_resources(registry);
    changelog::register_resources(registry);
    // Future: logs::register_resources(registry);
}
