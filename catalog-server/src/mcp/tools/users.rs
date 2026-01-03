//! Users Tools
//!
//! Tools for querying and managing users.

use serde::Deserialize;
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::user::{Permission, PermissionGrant, UserRole};

/// Register users tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(users_query_tool());
    registry.register_tool(users_mutate_tool());
}

// ============================================================================
// users.query
// ============================================================================

#[derive(Debug, Deserialize)]
struct UsersQueryParams {
    query_type: UsersQueryType,
    #[serde(default)]
    user_handle: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UsersQueryType {
    List,
    Get,
}

fn users_query_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("users.query")
        .description("Query users: list all users or get details for a specific user")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query_type": {
                    "type": "string",
                    "enum": ["list", "get"],
                    "description": "Type of query: 'list' for all users, 'get' for specific user details"
                },
                "user_handle": {
                    "type": "string",
                    "description": "User handle (required for 'get' query)"
                }
            },
            "required": ["query_type"]
        }))
        .permission(Permission::ManagePermissions)
        .category(ToolCategory::Read)
        .build(users_query_handler)
}

async fn users_query_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: UsersQueryParams = serde_json::from_value(params)
        .map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.query_type {
        UsersQueryType::List => list_users(&ctx).await,
        UsersQueryType::Get => {
            let handle = params.user_handle.ok_or_else(|| {
                McpError::InvalidParams("user_handle required for 'get' query".into())
            })?;
            get_user(&ctx, &handle).await
        }
    }
}

async fn list_users(ctx: &ToolContext) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let handles = user_manager
        .get_all_user_handles()
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let mut users = Vec::new();
    for handle in handles {
        if let Ok(Some(user_id)) = user_manager.get_user_id(&handle) {
            let roles: Vec<String> = user_manager
                .get_user_roles(user_id)
                .unwrap_or_default()
                .into_iter()
                .map(|r| format!("{:?}", r))
                .collect();

            users.push(serde_json::json!({
                "id": user_id,
                "handle": handle,
                "roles": roles,
            }));
        }
    }

    let result = serde_json::json!({
        "users": users,
        "total": users.len(),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_user(ctx: &ToolContext, handle: &str) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let user_id = user_manager
        .get_user_id(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("User not found: {}", handle)))?;

    let roles: Vec<String> = user_manager
        .get_user_roles(user_id)
        .unwrap_or_default()
        .into_iter()
        .map(|r| format!("{:?}", r))
        .collect();

    let permissions: Vec<String> = user_manager
        .get_user_permissions(user_id)
        .unwrap_or_default()
        .into_iter()
        .map(|p| format!("{:?}", p))
        .collect();

    let devices = user_manager
        .get_user_devices(user_id)
        .unwrap_or_default();

    let result = serde_json::json!({
        "user": {
            "id": user_id,
            "handle": handle,
            "roles": roles,
            "permissions": permissions,
            "device_count": devices.len(),
        }
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

// ============================================================================
// users.mutate
// ============================================================================

#[derive(Debug, Deserialize)]
struct UsersMutateParams {
    action: UsersMutateAction,
    user_handle: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    permission: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UsersMutateAction {
    Create,
    Delete,
    AddRole,
    RemoveRole,
    GrantPermission,
}

fn users_mutate_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("users.mutate")
        .description("Manage users: create/delete users, manage roles and permissions. CONFIRMATION REQUIRED before executing.")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "delete", "add_role", "remove_role", "grant_permission"],
                    "description": "Action to perform"
                },
                "user_handle": {
                    "type": "string",
                    "description": "User handle to operate on"
                },
                "role": {
                    "type": "string",
                    "enum": ["Admin", "Regular"],
                    "description": "Role for add_role/remove_role actions"
                },
                "permission": {
                    "type": "string",
                    "enum": ["AccessCatalog", "LikeContent", "OwnPlaylists", "EditCatalog", "ManagePermissions", "ServerAdmin", "ViewAnalytics", "RequestContent", "DownloadManagerAdmin", "ReportBug"],
                    "description": "Permission for grant_permission action (grants permanent extra permission)"
                }
            },
            "required": ["action", "user_handle"]
        }))
        .permission(Permission::ManagePermissions)
        .category(ToolCategory::Write)
        .build(users_mutate_handler)
}

async fn users_mutate_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: UsersMutateParams = serde_json::from_value(params)
        .map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.action {
        UsersMutateAction::Create => create_user(&ctx, &params.user_handle).await,
        UsersMutateAction::Delete => delete_user(&ctx, &params.user_handle).await,
        UsersMutateAction::AddRole => {
            let role = parse_role(&params.role)?;
            add_role(&ctx, &params.user_handle, role).await
        }
        UsersMutateAction::RemoveRole => {
            let role = parse_role(&params.role)?;
            remove_role(&ctx, &params.user_handle, role).await
        }
        UsersMutateAction::GrantPermission => {
            let permission = parse_permission(&params.permission)?;
            grant_permission(&ctx, &params.user_handle, permission).await
        }
    }
}

fn parse_role(role: &Option<String>) -> Result<UserRole, McpError> {
    let role_str = role
        .as_ref()
        .ok_or_else(|| McpError::InvalidParams("role is required".into()))?;

    match role_str.as_str() {
        "Admin" => Ok(UserRole::Admin),
        "Regular" => Ok(UserRole::Regular),
        _ => Err(McpError::InvalidParams(format!("Invalid role: {}", role_str))),
    }
}

fn parse_permission(permission: &Option<String>) -> Result<Permission, McpError> {
    let perm_str = permission
        .as_ref()
        .ok_or_else(|| McpError::InvalidParams("permission is required".into()))?;

    match perm_str.as_str() {
        "AccessCatalog" => Ok(Permission::AccessCatalog),
        "LikeContent" => Ok(Permission::LikeContent),
        "OwnPlaylists" => Ok(Permission::OwnPlaylists),
        "EditCatalog" => Ok(Permission::EditCatalog),
        "ManagePermissions" => Ok(Permission::ManagePermissions),
        "ServerAdmin" => Ok(Permission::ServerAdmin),
        "ViewAnalytics" => Ok(Permission::ViewAnalytics),
        "RequestContent" => Ok(Permission::RequestContent),
        "DownloadManagerAdmin" => Ok(Permission::DownloadManagerAdmin),
        "ReportBug" => Ok(Permission::ReportBug),
        _ => Err(McpError::InvalidParams(format!("Invalid permission: {}", perm_str))),
    }
}

async fn create_user(ctx: &ToolContext, handle: &str) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let user_id = user_manager
        .add_user(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "action": "create",
        "user_id": user_id,
        "user_handle": handle,
        "message": format!("User '{}' created with ID {}", handle, user_id),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn delete_user(ctx: &ToolContext, handle: &str) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    // Get user ID first
    let user_id = user_manager
        .get_user_id(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("User not found: {}", handle)))?;

    let deleted = user_manager
        .delete_user(user_id)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": deleted,
        "action": "delete",
        "user_handle": handle,
        "message": if deleted {
            format!("User '{}' deleted", handle)
        } else {
            format!("User '{}' not found", handle)
        },
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn add_role(ctx: &ToolContext, handle: &str, role: UserRole) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let user_id = user_manager
        .get_user_id(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("User not found: {}", handle)))?;

    user_manager
        .add_user_role(user_id, role)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "action": "add_role",
        "user_handle": handle,
        "role": format!("{:?}", role),
        "message": format!("Role {:?} added to user '{}'", role, handle),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn remove_role(ctx: &ToolContext, handle: &str, role: UserRole) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let user_id = user_manager
        .get_user_id(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("User not found: {}", handle)))?;

    user_manager
        .remove_user_role(user_id, role)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "action": "remove_role",
        "user_handle": handle,
        "role": format!("{:?}", role),
        "message": format!("Role {:?} removed from user '{}'", role, handle),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn grant_permission(ctx: &ToolContext, handle: &str, permission: Permission) -> ToolResult {
    use std::time::SystemTime;

    let user_manager = ctx.user_manager.lock().unwrap();

    let user_id = user_manager
        .get_user_id(handle)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("User not found: {}", handle)))?;

    // Create a permanent extra permission (no end_time, no countdown)
    let grant = PermissionGrant::Extra {
        start_time: SystemTime::now(),
        end_time: None,
        permission,
        countdown: None,
    };
    let grant_id = user_manager
        .add_user_extra_permission(user_id, grant)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "action": "grant_permission",
        "user_handle": handle,
        "permission": format!("{:?}", permission),
        "grant_id": grant_id,
        "message": format!("Permission {:?} granted to user '{}'", permission, handle),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}
