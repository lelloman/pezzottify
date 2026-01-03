//! MCP Tool and Resource Registry
//!
//! Manages registration and lookup of tools and resources.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use super::context::ToolContext;
use super::protocol::{
    McpError, ResourceContent, ResourceDefinition, ToolDefinition, ToolsCallResult,
};
use crate::user::Permission;

// ============================================================================
// Tool Types
// ============================================================================

/// Result type for tool execution
pub type ToolResult = Result<ToolsCallResult, McpError>;

/// Boxed future for async tool execution
pub type ToolFuture = Pin<Box<dyn Future<Output = ToolResult> + Send>>;

/// Tool handler function type
pub type ToolHandler = Arc<dyn Fn(ToolContext, Value) -> ToolFuture + Send + Sync>;

/// A registered tool with metadata and handler
pub struct RegisteredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub permissions: Vec<Permission>,
    pub handler: ToolHandler,
    pub category: ToolCategory,
}

/// Tool category for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    Read,
    Write,
    Sql,
}

// ============================================================================
// Resource Types
// ============================================================================

/// Result type for resource read
pub type ResourceResult = Result<Vec<ResourceContent>, McpError>;

/// Boxed future for async resource read
pub type ResourceFuture = Pin<Box<dyn Future<Output = ResourceResult> + Send>>;

/// Resource handler function type
pub type ResourceHandler = Arc<dyn Fn(ToolContext, String) -> ResourceFuture + Send + Sync>;

/// A registered resource with metadata and handler
pub struct RegisteredResource {
    pub uri_pattern: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub permissions: Vec<Permission>,
    pub handler: ResourceHandler,
}

// ============================================================================
// Registry
// ============================================================================

/// Registry for MCP tools and resources
pub struct McpRegistry {
    tools: HashMap<String, RegisteredTool>,
    resources: Vec<RegisteredResource>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            resources: Vec::new(),
        }
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: RegisteredTool) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Register a resource
    pub fn register_resource(&mut self, resource: RegisteredResource) {
        self.resources.push(resource);
    }

    /// Get tools available to a user based on their permissions
    pub fn get_available_tools(&self, permissions: &[Permission]) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| tool.permissions.iter().all(|p| permissions.contains(p)))
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
            })
            .collect()
    }

    /// Get a tool by name, checking permissions
    pub fn get_tool(&self, name: &str, permissions: &[Permission]) -> Option<&RegisteredTool> {
        self.tools.get(name).filter(|tool| {
            tool.permissions.iter().all(|p| permissions.contains(p))
        })
    }

    /// Get resources available to a user based on their permissions
    pub fn get_available_resources(&self, permissions: &[Permission]) -> Vec<ResourceDefinition> {
        self.resources
            .iter()
            .filter(|resource| {
                resource
                    .permissions
                    .iter()
                    .all(|p| permissions.contains(p))
            })
            .map(|resource| ResourceDefinition {
                uri: resource.uri_pattern.clone(),
                name: resource.name.clone(),
                description: resource.description.clone(),
                mime_type: resource.mime_type.clone(),
            })
            .collect()
    }

    /// Find a resource handler for a URI
    pub fn find_resource(
        &self,
        uri: &str,
        permissions: &[Permission],
    ) -> Option<&RegisteredResource> {
        self.resources.iter().find(|resource| {
            // Check permissions first
            if !resource
                .permissions
                .iter()
                .all(|p| permissions.contains(p))
            {
                return false;
            }

            // Simple pattern matching - supports {param} style patterns
            matches_uri_pattern(&resource.uri_pattern, uri)
        })
    }

    /// Get the number of registered tools
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Get the number of registered resources
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a URI matches a pattern with {param} placeholders
fn matches_uri_pattern(pattern: &str, uri: &str) -> bool {
    // Simple implementation: convert pattern to regex-like matching
    // Pattern: jobs://{job_id}/output
    // URI: jobs://popular_content/output

    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let uri_parts: Vec<&str> = uri.split('/').collect();

    if pattern_parts.len() != uri_parts.len() {
        return false;
    }

    for (pattern_part, uri_part) in pattern_parts.iter().zip(uri_parts.iter()) {
        if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
            // Parameter placeholder - matches anything
            continue;
        }
        if pattern_part != uri_part {
            return false;
        }
    }

    true
}

// ============================================================================
// Builder helpers
// ============================================================================

/// Builder for registering a tool
pub struct ToolBuilder {
    name: String,
    description: String,
    input_schema: Value,
    permissions: Vec<Permission>,
    category: ToolCategory,
}

impl ToolBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            permissions: Vec::new(),
            category: ToolCategory::Read,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn permission(mut self, perm: Permission) -> Self {
        self.permissions.push(perm);
        self
    }

    pub fn permissions(mut self, perms: &[Permission]) -> Self {
        self.permissions.extend_from_slice(perms);
        self
    }

    pub fn category(mut self, cat: ToolCategory) -> Self {
        self.category = cat;
        self
    }

    pub fn build<F, Fut>(self, handler: F) -> RegisteredTool
    where
        F: Fn(ToolContext, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolResult> + Send + 'static,
    {
        RegisteredTool {
            name: self.name,
            description: self.description,
            input_schema: self.input_schema,
            permissions: self.permissions,
            category: self.category,
            handler: Arc::new(move |ctx, params| Box::pin(handler(ctx, params))),
        }
    }
}

/// Builder for registering a resource
pub struct ResourceBuilder {
    uri_pattern: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    permissions: Vec<Permission>,
}

impl ResourceBuilder {
    pub fn new(uri_pattern: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri_pattern: uri_pattern.into(),
            name: name.into(),
            description: None,
            mime_type: None,
            permissions: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    pub fn permission(mut self, perm: Permission) -> Self {
        self.permissions.push(perm);
        self
    }

    pub fn build<F, Fut>(self, handler: F) -> RegisteredResource
    where
        F: Fn(ToolContext, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ResourceResult> + Send + 'static,
    {
        RegisteredResource {
            uri_pattern: self.uri_pattern,
            name: self.name,
            description: self.description,
            mime_type: self.mime_type,
            permissions: self.permissions,
            handler: Arc::new(move |ctx, uri| Box::pin(handler(ctx, uri))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_pattern_matching_exact() {
        assert!(matches_uri_pattern("logs://recent", "logs://recent"));
        assert!(!matches_uri_pattern("logs://recent", "logs://errors"));
    }

    #[test]
    fn test_uri_pattern_matching_with_param() {
        assert!(matches_uri_pattern(
            "jobs://{job_id}/output",
            "jobs://popular_content/output"
        ));
        assert!(matches_uri_pattern(
            "jobs://{job_id}/output",
            "jobs://integrity_watchdog/output"
        ));
        assert!(!matches_uri_pattern(
            "jobs://{job_id}/output",
            "jobs://popular_content/history"
        ));
    }

    #[test]
    fn test_uri_pattern_matching_different_lengths() {
        assert!(!matches_uri_pattern(
            "jobs://{job_id}/output",
            "jobs://popular_content"
        ));
        assert!(!matches_uri_pattern(
            "jobs://{job_id}",
            "jobs://popular_content/output"
        ));
    }

    #[test]
    fn test_registry_tool_count() {
        let registry = McpRegistry::new();
        assert_eq!(registry.tool_count(), 0);
    }
}
