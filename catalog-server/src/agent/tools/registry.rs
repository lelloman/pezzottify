//! Tool registry for agent capabilities.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Definition of a tool that an agent can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool (must be unique within a registry).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's parameters.
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    /// Create a tool definition with no parameters.
    pub fn no_params(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

/// Context provided to tools when they are executed.
///
/// This is intentionally generic - specific workflows will create
/// their own context types that implement the necessary traits.
pub struct ToolContext {
    /// Arbitrary context data, keyed by type name.
    data: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>,
}

impl ToolContext {
    /// Create an empty tool context.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a value into the context.
    pub fn insert<T: Send + Sync + 'static>(&mut self, key: impl Into<String>, value: T) {
        self.data.insert(key.into(), Arc::new(value));
    }

    /// Get a value from the context.
    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.data
            .get(key)
            .and_then(|v| Arc::clone(v).downcast::<T>().ok())
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when executing a tool.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Missing context: {0}")]
    MissingContext(String),
}

/// Trait for tools that agents can use.
///
/// Implement this trait to create custom tools for your agent workflows.
#[async_trait]
pub trait AgentTool: Send + Sync {
    /// Get the tool's definition (name, description, parameters).
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments.
    ///
    /// # Arguments
    /// * `args` - The arguments passed to the tool (as JSON).
    /// * `ctx` - The execution context containing shared resources.
    ///
    /// # Returns
    /// The tool's output as JSON.
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<serde_json::Value, ToolError>;
}

/// Registry for managing agent tools.
pub struct AgentToolRegistry {
    tools: HashMap<String, Arc<dyn AgentTool>>,
}

impl AgentToolRegistry {
    /// Create an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: impl AgentTool + 'static) {
        let def = tool.definition();
        self.tools.insert(def.name.clone(), Arc::new(tool));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn AgentTool>> {
        self.tools.get(name).cloned()
    }

    /// Get all tool definitions.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<serde_json::Value, ToolError> {
        let tool = self.get(name).ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(args, ctx).await
    }

    /// Check if a tool exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for AgentToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    #[async_trait]
    impl AgentTool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new(
                "echo",
                "Echoes the input",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"]
                }),
            )
        }

        async fn execute(
            &self,
            args: serde_json::Value,
            _ctx: &ToolContext,
        ) -> Result<serde_json::Value, ToolError> {
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidArguments("missing message".to_string()))?;
            Ok(serde_json::json!({"echo": message}))
        }
    }

    #[tokio::test]
    async fn test_tool_registry() {
        let mut registry = AgentToolRegistry::new();
        registry.register(EchoTool);

        assert!(registry.contains("echo"));
        assert_eq!(registry.len(), 1);

        let ctx = ToolContext::new();
        let result = registry
            .execute("echo", serde_json::json!({"message": "hello"}), &ctx)
            .await
            .unwrap();

        assert_eq!(result, serde_json::json!({"echo": "hello"}));
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let registry = AgentToolRegistry::new();
        let ctx = ToolContext::new();
        let result = registry.execute("nonexistent", serde_json::json!({}), &ctx).await;

        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }

    #[test]
    fn test_tool_context() {
        let mut ctx = ToolContext::new();
        ctx.insert("user_id", "user123".to_string());

        let user_id: Arc<String> = ctx.get("user_id").unwrap();
        assert_eq!(*user_id, "user123");

        let missing: Option<Arc<String>> = ctx.get("nonexistent");
        assert!(missing.is_none());
    }
}
