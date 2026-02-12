//! Analytics Tools
//!
//! Tools for querying listening and bandwidth statistics.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::context::ToolContext;
use crate::mcp::protocol::{McpError, ToolsCallResult};
use crate::mcp::registry::{McpRegistry, ToolBuilder, ToolCategory, ToolResult};
use crate::user::Permission;

/// Register analytics tools with the registry
pub fn register_tools(registry: &mut McpRegistry) {
    registry.register_tool(analytics_query_tool());
}

// ============================================================================
// analytics.query
// ============================================================================

#[derive(Debug, Deserialize)]
struct AnalyticsQueryParams {
    query_type: AnalyticsQueryType,
    #[serde(default)]
    user_id: Option<usize>,
    #[serde(default)]
    start_date: Option<u32>,
    #[serde(default)]
    end_date: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnalyticsQueryType {
    Bandwidth,
    Listening,
    Overview,
}

#[derive(Debug, Serialize)]
struct BandwidthResult {
    user_id: Option<usize>,
    total_bytes_sent: u64,
    total_requests: u64,
    by_category: serde_json::Value,
    period: DateRange,
}

#[derive(Debug, Serialize)]
struct ListeningResult {
    user_id: Option<usize>,
    total_plays: u64,
    total_duration_seconds: u64,
    completed_plays: u64,
    unique_tracks: u64,
    period: DateRange,
}

#[derive(Debug, Serialize)]
struct OverviewResult {
    bandwidth: BandwidthResult,
    listening: ListeningResult,
}

#[derive(Debug, Serialize)]
struct DateRange {
    start_date: u32,
    end_date: u32,
}

fn analytics_query_tool() -> super::super::registry::RegisteredTool {
    ToolBuilder::new("analytics.query")
        .description(
            "Query analytics: bandwidth usage, listening statistics, or an overview of both",
        )
        .input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query_type": {
                    "type": "string",
                    "enum": ["bandwidth", "listening", "overview"],
                    "description": "Type of analytics: 'bandwidth' for transfer stats, 'listening' for playback stats, 'overview' for both"
                },
                "user_id": {
                    "type": "integer",
                    "description": "Optional user ID to filter by (omit for platform-wide stats)"
                },
                "start_date": {
                    "type": "integer",
                    "description": "Start date in YYYYMMDD format (default: 30 days ago)"
                },
                "end_date": {
                    "type": "integer",
                    "description": "End date in YYYYMMDD format (default: today)"
                }
            },
            "required": ["query_type"]
        }))
        .permission(Permission::ViewAnalytics)
        .category(ToolCategory::Read)
        .build(analytics_query_handler)
}

async fn analytics_query_handler(ctx: ToolContext, params: Value) -> ToolResult {
    let params: AnalyticsQueryParams =
        serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    // Default date range: last 30 days
    let (start_date, end_date) = get_date_range(params.start_date, params.end_date);

    match params.query_type {
        AnalyticsQueryType::Bandwidth => {
            get_bandwidth_stats(&ctx, params.user_id, start_date, end_date).await
        }
        AnalyticsQueryType::Listening => {
            get_listening_stats(&ctx, params.user_id, start_date, end_date).await
        }
        AnalyticsQueryType::Overview => {
            get_overview(&ctx, params.user_id, start_date, end_date).await
        }
    }
}

fn get_date_range(start: Option<u32>, end: Option<u32>) -> (u32, u32) {
    let now = chrono::Utc::now();
    let end_date = end.unwrap_or_else(|| {
        let d = now.format("%Y%m%d").to_string();
        d.parse().unwrap_or(20240101)
    });
    let start_date = start.unwrap_or_else(|| {
        let d = (now - chrono::Duration::days(30))
            .format("%Y%m%d")
            .to_string();
        d.parse().unwrap_or(20240101)
    });
    (start_date, end_date)
}

async fn get_bandwidth_stats(
    ctx: &ToolContext,
    user_id: Option<usize>,
    start_date: u32,
    end_date: u32,
) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    let summary = if let Some(uid) = user_id {
        user_manager
            .get_user_bandwidth_summary(uid, start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    } else {
        user_manager
            .get_total_bandwidth_summary(start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    };

    let result = BandwidthResult {
        user_id: summary.user_id,
        total_bytes_sent: summary.total_bytes_sent,
        total_requests: summary.total_requests,
        by_category: serde_json::to_value(&summary.by_category).unwrap_or(serde_json::json!({})),
        period: DateRange {
            start_date,
            end_date,
        },
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}

async fn get_listening_stats(
    ctx: &ToolContext,
    user_id: Option<usize>,
    start_date: u32,
    end_date: u32,
) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    // If user_id is provided, get user-specific stats
    // Otherwise, we need platform-wide stats (which would require aggregation)
    if let Some(uid) = user_id {
        let summary = user_manager
            .get_user_listening_summary(uid, start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

        let result = ListeningResult {
            user_id: summary.user_id,
            total_plays: summary.total_plays,
            total_duration_seconds: summary.total_duration_seconds,
            completed_plays: summary.completed_plays,
            unique_tracks: summary.unique_tracks,
            period: DateRange {
                start_date,
                end_date,
            },
        };

        ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
    } else {
        // Platform-wide listening stats - aggregate across all users
        // For now, return a message indicating this requires iterating users
        let result = serde_json::json!({
            "user_id": null,
            "message": "Platform-wide listening stats require user_id. Use the admin API for aggregated stats.",
            "period": {
                "start_date": start_date,
                "end_date": end_date
            }
        });

        ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
    }
}

async fn get_overview(
    ctx: &ToolContext,
    user_id: Option<usize>,
    start_date: u32,
    end_date: u32,
) -> ToolResult {
    let user_manager = ctx.user_manager.lock().unwrap();

    // Get bandwidth stats
    let bandwidth_summary = if let Some(uid) = user_id {
        user_manager
            .get_user_bandwidth_summary(uid, start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    } else {
        user_manager
            .get_total_bandwidth_summary(start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?
    };

    let bandwidth = BandwidthResult {
        user_id: bandwidth_summary.user_id,
        total_bytes_sent: bandwidth_summary.total_bytes_sent,
        total_requests: bandwidth_summary.total_requests,
        by_category: serde_json::to_value(&bandwidth_summary.by_category)
            .unwrap_or(serde_json::json!({})),
        period: DateRange {
            start_date,
            end_date,
        },
    };

    // Get listening stats
    let listening = if let Some(uid) = user_id {
        let summary = user_manager
            .get_user_listening_summary(uid, start_date, end_date)
            .map_err(|e| McpError::ToolExecutionFailed(e.to_string()))?;

        ListeningResult {
            user_id: summary.user_id,
            total_plays: summary.total_plays,
            total_duration_seconds: summary.total_duration_seconds,
            completed_plays: summary.completed_plays,
            unique_tracks: summary.unique_tracks,
            period: DateRange {
                start_date,
                end_date,
            },
        }
    } else {
        ListeningResult {
            user_id: None,
            total_plays: 0,
            total_duration_seconds: 0,
            completed_plays: 0,
            unique_tracks: 0,
            period: DateRange {
                start_date,
                end_date,
            },
        }
    };

    let result = OverviewResult {
        bandwidth,
        listening,
    };

    ToolsCallResult::json(&result).map_err(|e| McpError::InternalError(e.to_string()))
}
