//! Jobs Tools
//!
//! Tools for querying background jobs and their history.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::user::Permission;

/// Register jobs tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(jobs_query_tool());
    registry.register_tool(jobs_action_tool());
}

// ============================================================================
// jobs.query
// ============================================================================

#[derive(Debug, Deserialize)]
struct JobsQueryParams {
    query_type: JobsQueryType,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JobsQueryType {
    List,
    Get,
    History,
    AuditLog,
}

#[derive(Debug, Serialize)]
struct JobsListResult {
    jobs: Vec<JobSummary>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct JobSummary {
    id: String,
    name: String,
    description: String,
    is_running: bool,
    schedule_type: String,
    next_run_at: Option<String>,
    last_status: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobHistoryResult {
    job_id: String,
    history: Vec<JobRunEntry>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct JobRunEntry {
    started_at: String,
    finished_at: Option<String>,
    status: String,
    error_message: Option<String>,
    triggered_by: String,
}

fn jobs_query_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("jobs.query")
        .description("Query background jobs: list all jobs, get job details, or view job history")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query_type": {
                    "type": "string",
                    "enum": ["list", "get", "history", "audit_log"],
                    "description": "Type of query: 'list' for all jobs, 'get' for specific job, 'history' for job runs, 'audit_log' for detailed audit entries"
                },
                "job_id": {
                    "type": "string",
                    "description": "Job ID (required for 'get', 'history', optional for 'audit_log')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results for history/audit queries (default 20)",
                    "minimum": 1,
                    "maximum": 100
                },
                "offset": {
                    "type": "integer",
                    "description": "Offset for pagination (default 0)",
                    "minimum": 0
                }
            },
            "required": ["query_type"]
        }))
        .permission(Permission::ServerAdmin)
        .category(ToolCategory::Read)
        .build(jobs_query_handler)
}

async fn jobs_query_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: JobsQueryParams = serde_json::from_value(params)
        .map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.query_type {
        JobsQueryType::List => list_jobs(&ctx).await,
        JobsQueryType::Get => {
            let job_id = params
                .job_id
                .ok_or_else(|| McpError::InvalidParams("job_id required for 'get' query".into()))?;
            get_job(&ctx, &job_id).await
        }
        JobsQueryType::History => {
            let job_id = params.job_id.ok_or_else(|| {
                McpError::InvalidParams("job_id required for 'history' query".into())
            })?;
            get_job_history(&ctx, &job_id, params.limit).await
        }
        JobsQueryType::AuditLog => {
            get_job_audit_log(&ctx, params.job_id.as_deref(), params.limit, params.offset).await
        }
    }
}

async fn list_jobs(ctx: &ToolContext) -> ToolResult {
    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    let jobs = scheduler
        .list_jobs()
        .await
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let summaries: Vec<JobSummary> = jobs
        .into_iter()
        .map(|j| JobSummary {
            id: j.id,
            name: j.name,
            description: j.description,
            is_running: j.is_running,
            schedule_type: j.schedule.schedule_type,
            next_run_at: j.next_run_at,
            last_status: j.last_run.map(|r| r.status),
        })
        .collect();

    let total = summaries.len();
    let result = JobsListResult {
        jobs: summaries,
        total,
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_job(ctx: &ToolContext, job_id: &str) -> ToolResult {
    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    let job = scheduler
        .get_job(job_id)
        .await
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Job not found: {}", job_id)))?;

    ToolsCallResult::json(&job).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_job_history(ctx: &ToolContext, job_id: &str, limit: usize) -> ToolResult {
    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    let history = scheduler
        .get_job_history(job_id, limit.min(100))
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let entries: Vec<JobRunEntry> = history
        .into_iter()
        .map(|r| JobRunEntry {
            started_at: r.started_at,
            finished_at: r.finished_at,
            status: r.status,
            error_message: r.error_message,
            triggered_by: r.triggered_by,
        })
        .collect();

    let total = entries.len();
    let result = JobHistoryResult {
        job_id: job_id.to_string(),
        history: entries,
        total,
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_job_audit_log(
    ctx: &ToolContext,
    job_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> ToolResult {
    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    let entries = if let Some(job_id) = job_id {
        scheduler
            .get_job_audit_log_by_job(job_id, limit.min(100), offset)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    } else {
        scheduler
            .get_job_audit_log(limit.min(100), offset)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    };

    let result = serde_json::json!({
        "entries": entries,
        "count": entries.len(),
        "offset": offset,
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

// ============================================================================
// jobs.action
// ============================================================================

#[derive(Debug, Deserialize)]
struct JobsActionParams {
    action: JobActionType,
    job_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JobActionType {
    Trigger,
}

fn jobs_action_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("jobs.action")
        .description("Perform actions on background jobs. CONFIRMATION REQUIRED before executing.")
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["trigger"],
                    "description": "Action to perform: 'trigger' to manually run a job"
                },
                "job_id": {
                    "type": "string",
                    "description": "Job ID to act on"
                }
            },
            "required": ["action", "job_id"]
        }))
        .permission(Permission::ServerAdmin)
        .category(ToolCategory::Write)
        .build(jobs_action_handler)
}

async fn jobs_action_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: JobsActionParams = serde_json::from_value(params)
        .map_err(|e| McpError::InvalidParams(e.to_string()))?;

    match params.action {
        JobActionType::Trigger => trigger_job(&ctx, &params.job_id).await,
    }
}

async fn trigger_job(ctx: &ToolContext, job_id: &str) -> ToolResult {
    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    // Verify job exists
    let job = scheduler
        .get_job(job_id)
        .await
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Job not found: {}", job_id)))?;

    // Trigger the job
    scheduler
        .trigger_job(job_id, Some(serde_json::json!({"source": "mcp"})))
        .await
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "action": "trigger",
        "job_id": job_id,
        "job_name": job.name,
        "message": format!("Job '{}' triggered manually via MCP", job.name),
    });

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}
