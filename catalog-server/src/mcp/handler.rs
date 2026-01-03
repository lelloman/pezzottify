//! MCP WebSocket Handler
//!
//! Handles WebSocket connections for MCP protocol.

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use tracing::{debug, error, info};

use super::context::ToolContext;
use super::protocol::{
    methods, InitializeParams, InitializeResult, McpError, McpRequest, McpResponse, PingResult,
    ResourcesCapability, ResourcesListResult, ResourcesReadParams, ResourcesReadResult,
    ServerCapabilities, ServerInfo, ToolsCallParams, ToolsCapability, ToolsListResult,
    MCP_PROTOCOL_VERSION,
};
use super::rate_limit::McpRateLimiter;
use super::registry::McpRegistry;
use crate::server::session::Session;
use crate::server::state::{GuardedMcpState, ServerState};

/// State shared across MCP connections
pub struct McpState {
    pub registry: Arc<McpRegistry>,
    pub rate_limiter: Arc<McpRateLimiter>,
}

/// WebSocket upgrade handler for MCP
pub async fn mcp_handler(
    ws: WebSocketUpgrade,
    session: Session,
    State(server_state): State<ServerState>,
    State(mcp_state): State<GuardedMcpState>,
) -> Response {
    info!(
        "MCP WebSocket upgrade for user {} (permissions: {:?})",
        session.user_id,
        session.permissions.len()
    );

    ws.on_upgrade(move |socket| handle_mcp_socket(socket, session, server_state, mcp_state))
}

/// Handle an established MCP WebSocket connection
async fn handle_mcp_socket(
    socket: WebSocket,
    session: Session,
    server_state: ServerState,
    mcp_state: Arc<McpState>,
) {
    let user_id = session.user_id;
    debug!("MCP connection established for user {}", user_id);

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Process messages
    let mut initialized = false;

    while let Some(result) = ws_stream.next().await {
        match result {
            Ok(Message::Text(text)) => {
                let response =
                    handle_message(&text, &session, &server_state, &mcp_state, &mut initialized)
                        .await;

                if let Some(response) = response {
                    match serde_json::to_string(&response) {
                        Ok(json) => {
                            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize MCP response: {}", e);
                        }
                    }
                }
            }
            Ok(Message::Binary(_)) => {
                debug!("Received binary message, ignoring");
            }
            Ok(Message::Ping(_)) => {
                // Axum/tungstenite handles pong automatically
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                debug!("Received close frame");
                break;
            }
            Err(e) => {
                debug!("WebSocket error: {}", e);
                break;
            }
        }
    }

    debug!("MCP connection closed for user {}", user_id);
}

/// Handle a single MCP message
async fn handle_message(
    text: &str,
    session: &Session,
    server_state: &ServerState,
    mcp_state: &McpState,
    initialized: &mut bool,
) -> Option<McpResponse> {
    // Parse the request
    let request: McpRequest = match serde_json::from_str(text) {
        Ok(req) => req,
        Err(e) => {
            return Some(McpResponse::error(
                None,
                McpError::ParseError(e.to_string()),
            ));
        }
    };

    let request_id = request.id.clone();

    // Dispatch based on method
    let result = match request.method.as_str() {
        methods::INITIALIZE => handle_initialize(&request, initialized).await,
        methods::INITIALIZED => {
            // Notification, no response needed
            return None;
        }
        methods::PING => handle_ping(&request).await,
        methods::TOOLS_LIST => {
            if !*initialized {
                Err(McpError::InvalidRequest("Not initialized".to_string()))
            } else {
                handle_tools_list(session, mcp_state).await
            }
        }
        methods::TOOLS_CALL => {
            if !*initialized {
                Err(McpError::InvalidRequest("Not initialized".to_string()))
            } else {
                handle_tools_call(&request, session, server_state, mcp_state).await
            }
        }
        methods::RESOURCES_LIST => {
            if !*initialized {
                Err(McpError::InvalidRequest("Not initialized".to_string()))
            } else {
                handle_resources_list(session, mcp_state).await
            }
        }
        methods::RESOURCES_READ => {
            if !*initialized {
                Err(McpError::InvalidRequest("Not initialized".to_string()))
            } else {
                handle_resources_read(&request, session, server_state, mcp_state).await
            }
        }
        methods::SHUTDOWN => {
            // Client is disconnecting gracefully
            return None;
        }
        other => Err(McpError::MethodNotFound(other.to_string())),
    };

    Some(match result {
        Ok(value) => McpResponse::success(request_id, value),
        Err(error) => McpResponse::error(Some(request_id), error),
    })
}

async fn handle_initialize(
    request: &McpRequest,
    initialized: &mut bool,
) -> Result<serde_json::Value, McpError> {
    let _params: InitializeParams = request
        .params
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::InvalidParams(e.to_string()))?
        .unwrap_or(InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: Default::default(),
            client_info: super::protocol::ClientInfo {
                name: "unknown".to_string(),
                version: "unknown".to_string(),
            },
        });

    *initialized = true;

    let result = InitializeResult {
        protocol_version: MCP_PROTOCOL_VERSION.to_string(),
        capabilities: ServerCapabilities {
            tools: Some(ToolsCapability { list_changed: None }),
            resources: Some(ResourcesCapability {
                subscribe: Some(false),
                list_changed: None,
            }),
        },
        server_info: ServerInfo {
            name: "pezzottify-mcp".to_string(),
            version: format!("{}-{}", env!("APP_VERSION"), env!("GIT_HASH")),
        },
    };

    serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn handle_ping(_request: &McpRequest) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(PingResult {}).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn handle_tools_list(
    session: &Session,
    mcp_state: &McpState,
) -> Result<serde_json::Value, McpError> {
    let tools = mcp_state.registry.get_available_tools(&session.permissions);

    let result = ToolsListResult { tools };

    serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn handle_tools_call(
    request: &McpRequest,
    session: &Session,
    server_state: &ServerState,
    mcp_state: &McpState,
) -> Result<serde_json::Value, McpError> {
    let params: ToolsCallParams = request
        .params
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::InvalidParams(e.to_string()))?
        .ok_or_else(|| McpError::InvalidParams("Missing params".to_string()))?;

    // Find the tool
    let tool = mcp_state
        .registry
        .get_tool(&params.name, &session.permissions)
        .ok_or_else(|| {
            // Check if tool exists but user lacks permission
            if mcp_state.registry.get_tool(&params.name, &[]).is_some() {
                McpError::PermissionDenied(format!("No permission for tool: {}", params.name))
            } else {
                McpError::MethodNotFound(format!("Unknown tool: {}", params.name))
            }
        })?;

    // Check rate limit
    if let Err(retry_after) = mcp_state
        .rate_limiter
        .check_and_record(session.user_id, tool.category)
    {
        return Err(McpError::RateLimited {
            retry_after_secs: retry_after,
        });
    }

    // Build tool context
    let ctx = ToolContext {
        session: Session {
            user_id: session.user_id,
            token: session.token.clone(),
            permissions: session.permissions.clone(),
            device_id: session.device_id,
            device_type: session.device_type.clone(),
        },
        catalog_store: server_state.catalog_store.clone(),
        search_vault: server_state.search_vault.clone(),
        user_manager: server_state.user_manager.clone(),
        server_store: server_state.server_store.clone(),
        scheduler_handle: server_state.scheduler_handle.clone(),
        download_manager: server_state.download_manager.clone(),
        config: server_state.config.clone(),
        server_version: format!("{}-{}", env!("APP_VERSION"), env!("GIT_HASH")),
        start_time: server_state.start_time,
    };

    // Execute the tool
    let arguments = params.arguments.unwrap_or(serde_json::json!({}));
    let result = (tool.handler)(ctx, arguments).await?;

    serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn handle_resources_list(
    session: &Session,
    mcp_state: &McpState,
) -> Result<serde_json::Value, McpError> {
    let resources = mcp_state
        .registry
        .get_available_resources(&session.permissions);

    let result = ResourcesListResult { resources };

    serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn handle_resources_read(
    request: &McpRequest,
    session: &Session,
    server_state: &ServerState,
    mcp_state: &McpState,
) -> Result<serde_json::Value, McpError> {
    let params: ResourcesReadParams = request
        .params
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::InvalidParams(e.to_string()))?
        .ok_or_else(|| McpError::InvalidParams("Missing params".to_string()))?;

    // Find matching resource
    let resource = mcp_state
        .registry
        .find_resource(&params.uri, &session.permissions)
        .ok_or_else(|| McpError::ResourceNotFound(params.uri.clone()))?;

    // Check rate limit (resources count as reads)
    if let Err(retry_after) = mcp_state
        .rate_limiter
        .check_and_record(session.user_id, super::registry::ToolCategory::Read)
    {
        return Err(McpError::RateLimited {
            retry_after_secs: retry_after,
        });
    }

    // Build context
    let ctx = ToolContext {
        session: Session {
            user_id: session.user_id,
            token: session.token.clone(),
            permissions: session.permissions.clone(),
            device_id: session.device_id,
            device_type: session.device_type.clone(),
        },
        catalog_store: server_state.catalog_store.clone(),
        search_vault: server_state.search_vault.clone(),
        user_manager: server_state.user_manager.clone(),
        server_store: server_state.server_store.clone(),
        scheduler_handle: server_state.scheduler_handle.clone(),
        download_manager: server_state.download_manager.clone(),
        config: server_state.config.clone(),
        server_version: format!("{}-{}", env!("APP_VERSION"), env!("GIT_HASH")),
        start_time: server_state.start_time,
    };

    // Read the resource
    let contents = (resource.handler)(ctx, params.uri).await?;

    let result = ResourcesReadResult { contents };

    serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
}

/// Create the MCP state with registered tools and resources
pub fn create_mcp_state() -> McpState {
    let mut registry = McpRegistry::new();

    // Register all tools
    super::tools::register_all_tools(&mut registry);

    // Register all resources
    super::resources::register_all_resources(&mut registry);

    info!(
        "MCP registry initialized with {} tools and {} resources",
        registry.tool_count(),
        registry.resource_count()
    );

    McpState {
        registry: Arc::new(registry),
        rate_limiter: Arc::new(McpRateLimiter::default()),
    }
}
