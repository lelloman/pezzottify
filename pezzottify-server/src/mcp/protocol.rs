//! MCP Protocol Types
//!
//! Implements the Model Context Protocol message types.
//! MCP is essentially JSON-RPC 2.0 with specific method names and schemas.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC version string
pub const JSONRPC_VERSION: &str = "2.0";

/// MCP protocol version we support
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// ============================================================================
// Core Message Types
// ============================================================================

/// Incoming request from MCP client
#[derive(Debug, Clone, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Response to MCP client
#[derive(Debug, Clone, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpErrorResponse>,
}

impl McpResponse {
    pub fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<RequestId>, error: McpError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error.into()),
        }
    }
}

/// Request ID can be string or number
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Number(i64),
}

/// Error response structure
#[derive(Debug, Clone, Serialize)]
pub struct McpErrorResponse {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ============================================================================
// Error Codes (JSON-RPC + MCP specific)
// ============================================================================

/// MCP error types
#[derive(Debug, Clone)]
pub enum McpError {
    // JSON-RPC standard errors
    ParseError(String),
    InvalidRequest(String),
    MethodNotFound(String),
    InvalidParams(String),
    InternalError(String),

    // MCP specific errors
    Unauthorized,
    PermissionDenied(String),
    RateLimited { retry_after_secs: u32 },
    ResourceNotFound(String),
    ToolExecutionFailed(String),
}

impl McpError {
    pub fn code(&self) -> i32 {
        match self {
            McpError::ParseError(_) => -32700,
            McpError::InvalidRequest(_) => -32600,
            McpError::MethodNotFound(_) => -32601,
            McpError::InvalidParams(_) => -32602,
            McpError::InternalError(_) => -32603,
            McpError::Unauthorized => -32001,
            McpError::PermissionDenied(_) => -32002,
            McpError::RateLimited { .. } => -32003,
            McpError::ResourceNotFound(_) => -32004,
            McpError::ToolExecutionFailed(_) => -32005,
        }
    }

    pub fn message(&self) -> String {
        match self {
            McpError::ParseError(msg) => format!("Parse error: {}", msg),
            McpError::InvalidRequest(msg) => format!("Invalid request: {}", msg),
            McpError::MethodNotFound(method) => format!("Method not found: {}", method),
            McpError::InvalidParams(msg) => format!("Invalid params: {}", msg),
            McpError::InternalError(msg) => format!("Internal error: {}", msg),
            McpError::Unauthorized => "Authentication required".to_string(),
            McpError::PermissionDenied(perm) => format!("Permission denied: {}", perm),
            McpError::RateLimited { retry_after_secs } => {
                format!(
                    "Rate limit exceeded, retry after {} seconds",
                    retry_after_secs
                )
            }
            McpError::ResourceNotFound(uri) => format!("Resource not found: {}", uri),
            McpError::ToolExecutionFailed(msg) => format!("Tool execution failed: {}", msg),
        }
    }
}

impl From<McpError> for McpErrorResponse {
    fn from(err: McpError) -> Self {
        let data = match &err {
            McpError::RateLimited { retry_after_secs } => {
                Some(serde_json::json!({ "retry_after_secs": retry_after_secs }))
            }
            _ => None,
        };

        McpErrorResponse {
            code: err.code(),
            message: err.message(),
            data,
        }
    }
}

// ============================================================================
// MCP Method Names
// ============================================================================

pub mod methods {
    // Lifecycle
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "notifications/initialized";
    pub const SHUTDOWN: &str = "shutdown";

    // Tools
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";

    // Resources
    pub const RESOURCES_LIST: &str = "resources/list";
    pub const RESOURCES_READ: &str = "resources/read";

    // Ping
    pub const PING: &str = "ping";
}

// ============================================================================
// Initialize Messages
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub roots: Option<RootsCapability>,
    #[serde(default)]
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RootsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SamplingCapability {}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourcesCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// ============================================================================
// Tools Messages
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolsCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsCallResult {
    pub content: Vec<ToolResultContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolResultContent {
    Text { text: String },
    // Future: Image, Resource, etc.
}

impl ToolsCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: text.into() }],
            is_error: None,
        }
    }

    pub fn json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        let text = serde_json::to_string_pretty(value)?;
        Ok(Self::text(text))
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text {
                text: message.into(),
            }],
            is_error: Some(true),
        }
    }
}

// ============================================================================
// Resources Messages
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ResourcesListResult {
    pub resources: Vec<ResourceDefinition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourcesReadParams {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourcesReadResult {
    pub contents: Vec<ResourceContent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ResourceContent {
    Text {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        text: String,
    },
    Blob {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        blob: String, // base64 encoded
    },
}

// ============================================================================
// Ping
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct PingResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_deserialize_string() {
        let json = r#""test-id""#;
        let id: RequestId = serde_json::from_str(json).unwrap();
        assert_eq!(id, RequestId::String("test-id".to_string()));
    }

    #[test]
    fn test_request_id_deserialize_number() {
        let json = "42";
        let id: RequestId = serde_json::from_str(json).unwrap();
        assert_eq!(id, RequestId::Number(42));
    }

    #[test]
    fn test_mcp_response_success() {
        let resp = McpResponse::success(RequestId::Number(1), serde_json::json!({"ok": true}));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_mcp_response_error() {
        let resp = McpResponse::error(
            Some(RequestId::Number(1)),
            McpError::MethodNotFound("test".to_string()),
        );
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(McpError::ParseError("".to_string()).code(), -32700);
        assert_eq!(McpError::InvalidRequest("".to_string()).code(), -32600);
        assert_eq!(McpError::MethodNotFound("".to_string()).code(), -32601);
        assert_eq!(McpError::InvalidParams("".to_string()).code(), -32602);
        assert_eq!(McpError::InternalError("".to_string()).code(), -32603);
        assert_eq!(McpError::Unauthorized.code(), -32001);
        assert_eq!(McpError::PermissionDenied("".to_string()).code(), -32002);
        assert_eq!(
            McpError::RateLimited {
                retry_after_secs: 60
            }
            .code(),
            -32003
        );
    }

    #[test]
    fn test_tools_call_result_text() {
        let result = ToolsCallResult::text("Hello, world!");
        assert_eq!(result.content.len(), 1);
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_tools_call_result_error() {
        let result = ToolsCallResult::error("Something went wrong");
        assert_eq!(result.is_error, Some(true));
    }
}
