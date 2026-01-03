//! Job Resources
//!
//! Resources for accessing job output and audit logs.

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ResourceContent};
use crate::mcp::registry::{McpRegistry, ResourceBuilder, ResourceResult};
use crate::user::Permission;

/// Register job resources with the registry
pub fn register_resources(registry: &mut McpRegistry) {
    registry.register_resource(job_output_resource());
}

// ============================================================================
// jobs://{job_id}/output
// ============================================================================

fn job_output_resource() -> super::super::registry::RegisteredResource {
    ResourceBuilder::new("jobs://{job_id}/output", "Job Output")
        .description("Recent audit log entries for a specific job, showing run history and results")
        .mime_type("application/json")
        .permission(Permission::ServerAdmin)
        .build(job_output_handler)
}

async fn job_output_handler(ctx: ToolContext, uri: String) -> ResourceResult {
    // Parse job_id from URI: jobs://{job_id}/output
    let job_id = extract_job_id(&uri).ok_or_else(|| {
        McpError::InvalidParams(format!("Invalid job URI format: {}", uri))
    })?;

    let scheduler = ctx.scheduler_handle.as_ref().ok_or_else(|| {
        McpError::ToolExecutionFailed("Job scheduler not available".to_string())
    })?;

    // Get job info to verify it exists
    let job = scheduler
        .get_job(&job_id)
        .await
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
        .ok_or_else(|| McpError::ResourceNotFound(format!("Job not found: {}", job_id)))?;

    // Get recent audit log entries for this job
    let audit_entries = scheduler
        .get_job_audit_log_by_job(&job_id, 50, 0)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    // Get recent run history
    let history = scheduler
        .get_job_history(&job_id, 10)
        .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

    let output = serde_json::json!({
        "job": {
            "id": job.id,
            "name": job.name,
            "description": job.description,
            "is_running": job.is_running,
            "schedule_type": job.schedule.schedule_type,
            "next_run_at": job.next_run_at,
        },
        "recent_runs": history.iter().map(|r| serde_json::json!({
            "started_at": r.started_at,
            "finished_at": r.finished_at,
            "status": r.status,
            "error_message": r.error_message,
            "triggered_by": r.triggered_by,
        })).collect::<Vec<_>>(),
        "audit_log": audit_entries,
    });

    let content = ResourceContent::Text {
        uri,
        mime_type: Some("application/json".to_string()),
        text: serde_json::to_string_pretty(&output).unwrap_or_default(),
    };

    Ok(vec![content])
}

/// Extract job_id from URI like "jobs://job_id/output"
fn extract_job_id(uri: &str) -> Option<String> {
    let stripped = uri.strip_prefix("jobs://")?;
    let parts: Vec<&str> = stripped.split('/').collect();
    if !parts.is_empty() && !parts[0].is_empty() {
        Some(parts[0].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_job_id() {
        assert_eq!(
            extract_job_id("jobs://popular_content/output"),
            Some("popular_content".to_string())
        );
        assert_eq!(
            extract_job_id("jobs://integrity_watchdog/output"),
            Some("integrity_watchdog".to_string())
        );
        assert_eq!(extract_job_id("jobs:///output"), None);
        assert_eq!(extract_job_id("invalid://format"), None);
    }
}
