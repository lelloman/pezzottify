//! Download manager HTTP routes.
//!
//! Provides endpoints for:
//! - User download requests (request tracks/albums)
//! - User rate limit status
//! - Admin queue management and audit logs

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db_executor::{DbHandle, DbPriority, DbRunError};
use crate::download_manager::{AuditLogFilter, DownloadManager, QueueStatus};
use crate::server::session::Session;
use crate::server::state::{DatabaseHandles, OptionalTrackMaterializer, ServerState};
use crate::server::track_materializer::ProxyMaterializerStatus;
use crate::server::ApiError;
use crate::user::Permission;

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct RequestTrackBody {
    pub track_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestAlbumBody {
    pub album_id: String,
    /// Album name for display (optional, used by clients)
    #[serde(default)]
    pub album_name: Option<String>,
    /// Artist name for display (optional, used by clients)
    #[serde(default)]
    pub artist_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AdminRequestsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub status: Option<String>,
    #[serde(default)]
    pub exclude_completed: bool,
    #[serde(default)]
    pub top_level_only: bool,
}

#[derive(Debug, Serialize)]
pub struct DownloadStatusResponse {
    pub enabled: bool,
    pub pending_count: usize,
}

/// Response for POST /request/album
#[derive(Debug, Serialize)]
pub struct AlbumRequestResponse {
    /// ID of the created queue item (first track's queue item)
    pub request_id: String,
    /// Initial status (usually PENDING)
    pub status: String,
}

/// Legacy response format (kept for backwards compatibility)
#[derive(Debug, Serialize)]
pub struct RequestResponse {
    pub success: bool,
    pub message: String,
    pub queue_item_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
    pub entries: Vec<serde_json::Value>,
    pub total_count: usize,
}

/// Admin stats response with queue status
#[derive(Debug, Serialize)]
pub struct AdminStatsResponse {
    /// Queue statistics
    pub queue: crate::download_manager::QueueStats,
}

// =============================================================================
// Helper to extract DownloadManager
// =============================================================================

fn get_download_manager(
    database: &DatabaseHandles,
) -> Result<DbHandle<DownloadManager>, (StatusCode, &'static str)> {
    database.download.clone().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Download manager not enabled",
    ))
}

// =============================================================================
// User Routes
// =============================================================================

/// GET /limits - Get user's rate limit status
async fn get_user_limits(
    session: Session,
    State(database): State<DatabaseHandles>,
) -> impl IntoResponse {
    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();
    let is_admin = session.has_permission(Permission::DownloadManagerAdmin);
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_user_limits(&user_id, is_admin)
        })
        .await
    {
        Ok(limits) => Json(limits).into_response(),
        Err(e) => {
            warn!("Failed to get user limits: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// Response for GET /my-requests - includes requests and rate limit stats
#[derive(Debug, Serialize)]
pub struct MyRequestsResponse {
    pub requests: Vec<crate::download_manager::QueueItem>,
    pub stats: crate::download_manager::UserLimitStatus,
}

/// GET /my-requests - Get user's queued download requests
async fn get_my_requests(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();

    let is_admin = session.has_permission(Permission::DownloadManagerAdmin);
    let result = manager
        .run(DbPriority::Interactive, move |manager| {
            Ok((
                manager.get_user_requests(&user_id, pagination.limit, pagination.offset)?,
                manager.get_user_limits(&user_id, is_admin)?,
            ))
        })
        .await;
    let (requests, stats) = match result {
        Ok(result) => result,
        Err(e) => {
            warn!("Failed to get user download requests: {}", e);
            return ApiError::from(e).into_response();
        }
    };

    Json(MyRequestsResponse { requests, stats }).into_response()
}

/// POST /request/track - Request a single track download
async fn request_track(
    session: Session,
    State(database): State<DatabaseHandles>,
    Json(body): Json<RequestTrackBody>,
) -> impl IntoResponse {
    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();
    let is_admin = session.has_permission(Permission::DownloadManagerAdmin);
    debug!("User {} requesting track {}", user_id, body.track_id);

    let track_id = body.track_id;
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.request_track(&user_id, &track_id, is_admin)
        })
        .await
    {
        Ok(item) => Json(RequestResponse {
            success: true,
            message: "Track queued for download".to_string(),
            queue_item_id: Some(item.id),
        })
        .into_response(),
        Err(DbRunError::Store(e)) => {
            let msg = e.to_string();
            debug!("Track request failed: {}", msg);
            (
                StatusCode::BAD_REQUEST,
                Json(RequestResponse {
                    success: false,
                    message: msg,
                    queue_item_id: None,
                }),
            )
                .into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// POST /request/album - Request all tracks in an album
async fn request_album(
    session: Session,
    State(database): State<DatabaseHandles>,
    Json(body): Json<RequestAlbumBody>,
) -> impl IntoResponse {
    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();
    debug!(
        "User {} requesting album {} ({:?} by {:?})",
        user_id, body.album_id, body.album_name, body.artist_name
    );

    let is_admin = session.has_permission(Permission::DownloadManagerAdmin);
    let album_id = body.album_id;
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.request_album(&user_id, &album_id, is_admin)
        })
        .await
    {
        Ok(items) => Json(AlbumRequestResponse {
            request_id: items.first().map(|i| i.id.clone()).unwrap_or_default(),
            status: "PENDING".to_string(),
        })
        .into_response(),
        Err(DbRunError::Store(e)) => {
            let msg = e.to_string();
            debug!("Album request failed: {}", msg);
            (StatusCode::BAD_REQUEST, msg).into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// GET /status - Get download manager status
async fn get_status(State(database): State<DatabaseHandles>) -> impl IntoResponse {
    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let status = match manager
        .run(DbPriority::Interactive, |manager| Ok(manager.get_status()))
        .await
    {
        Ok(status) => status,
        Err(e) => return ApiError::from(e).into_response(),
    };
    Json(DownloadStatusResponse {
        enabled: status.enabled,
        pending_count: status.pending_count,
    })
    .into_response()
}

// =============================================================================
// Admin Routes
// =============================================================================

/// GET /admin/stats - Get queue statistics
async fn get_admin_stats(
    session: Session,
    State(database): State<DatabaseHandles>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    match manager
        .run(DbPriority::Interactive, |manager| manager.get_queue_stats())
        .await
    {
        Ok(queue_stats) => Json(AdminStatsResponse { queue: queue_stats }).into_response(),
        Err(e) => {
            warn!("Failed to get queue stats: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// GET /admin/proxy - Get active and recent on-demand track materializations.
async fn get_admin_proxy_status(
    session: Session,
    State(materializer): State<OptionalTrackMaterializer>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::DownloadManagerAdmin) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let status = materializer
        .map(|materializer| materializer.status())
        .unwrap_or(ProxyMaterializerStatus {
            enabled: false,
            active: Vec::new(),
            recent: Vec::new(),
            memory_used_bytes: 0,
            memory_limit_bytes: 0,
            foreground_active: 0,
            foreground_limit: 0,
            prefetch_active: 0,
            prefetch_limit: 0,
        });
    Json(status).into_response()
}

/// GET /admin/failed - Get failed download items
async fn get_admin_failed(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_failed_items(pagination.limit, pagination.offset)
        })
        .await
    {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            warn!("Failed to get failed items: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// GET /admin/requests - Get all queued requests
async fn get_admin_requests(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(query): Query<AdminRequestsQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let status = query.status.and_then(|s| match s.to_uppercase().as_str() {
        "PENDING" => Some(QueueStatus::Pending),
        "IN_PROGRESS" => Some(QueueStatus::InProgress),
        "COMPLETED" => Some(QueueStatus::Completed),
        "FAILED" => Some(QueueStatus::Failed),
        "RETRY_WAITING" => Some(QueueStatus::RetryWaiting),
        _ => None,
    });

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_all_requests_filtered(
                status,
                query.exclude_completed,
                query.top_level_only,
                query.limit,
                query.offset,
            )
        })
        .await
    {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            warn!("Failed to get requests: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// POST /admin/retry/:id - Retry a failed download
async fn retry_failed(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();
    let retry_id = item_id.clone();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.retry_failed(&retry_id, &user_id)
        })
        .await
    {
        Ok(()) => Json(RequestResponse {
            success: true,
            message: "Item queued for retry".to_string(),
            queue_item_id: Some(item_id),
        })
        .into_response(),
        Err(DbRunError::Store(e)) => {
            warn!("Failed to retry item: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// DELETE /admin/request/:id - Delete a download request
async fn delete_request(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let user_id = session.user_id.to_string();
    let delete_id = item_id.clone();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.delete_request(&delete_id, &user_id)
        })
        .await
    {
        Ok(true) => Json(RequestResponse {
            success: true,
            message: "Request deleted".to_string(),
            queue_item_id: Some(item_id),
        })
        .into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "Request not found").into_response(),
        Err(DbRunError::Store(e)) => {
            warn!("Failed to delete request: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// GET /admin/audit - Query audit log
async fn get_audit_log(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(query): Query<AuditLogQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let filter = AuditLogFilter {
        event_types: None, // Could parse from query.event_type if needed
        queue_item_id: None,
        user_id: None,
        content_type: None,
        content_id: None,
        since: query.start_time,
        until: query.end_time,
        limit: query.limit,
        offset: query.offset,
    };

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_audit_log(filter)
        })
        .await
    {
        Ok((entries, total)) => {
            let json_entries: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .collect();
            Json(AuditLogResponse {
                entries: json_entries,
                total_count: total,
            })
            .into_response()
        }
        Err(e) => {
            warn!("Failed to get audit log: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// GET /admin/audit/item/:id - Get audit log for a specific queue item
async fn get_audit_for_item(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(item_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let filter = AuditLogFilter {
        event_types: None,
        queue_item_id: Some(item_id),
        user_id: None,
        content_type: None,
        content_id: None,
        since: None,
        until: None,
        limit: pagination.limit,
        offset: pagination.offset,
    };

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_audit_log(filter)
        })
        .await
    {
        Ok((entries, total)) => {
            let json_entries: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .collect();
            Json(AuditLogResponse {
                entries: json_entries,
                total_count: total,
            })
            .into_response()
        }
        Err(e) => {
            warn!("Failed to get audit log: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

/// GET /admin/audit/user/:user_id - Get audit log for a specific user
async fn get_audit_for_user(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_download_manager(&database) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };

    let filter = AuditLogFilter {
        event_types: None,
        queue_item_id: None,
        user_id: Some(user_id),
        content_type: None,
        content_id: None,
        since: None,
        until: None,
        limit: pagination.limit,
        offset: pagination.offset,
    };

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_audit_log(filter)
        })
        .await
    {
        Ok((entries, total)) => {
            let json_entries: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .collect();
            Json(AuditLogResponse {
                entries: json_entries,
                total_count: total,
            })
            .into_response()
        }
        Err(e) => {
            warn!("Failed to get audit log: {}", e);
            ApiError::from(e).into_response()
        }
    }
}

// =============================================================================
// Router Construction
// =============================================================================

/// Build the download manager routes.
///
/// User routes (require RequestContent permission):
/// - GET /limits
/// - GET /my-requests
/// - POST /request/track
/// - POST /request/album
/// - GET /status
///
/// Admin routes (require ViewAnalytics/EditCatalog):
/// - GET /admin/stats
/// - GET /admin/failed
/// - GET /admin/requests
/// - DELETE /admin/request/:id
/// - POST /admin/retry/:id
/// - GET /admin/audit
/// - GET /admin/audit/item/:id
/// - GET /admin/audit/user/:user_id
pub fn download_routes() -> Router<ServerState> {
    // User routes
    let user_routes = Router::new()
        .route("/limits", get(get_user_limits))
        .route("/my-requests", get(get_my_requests))
        .route("/request/track", post(request_track))
        .route("/request/album", post(request_album))
        .route("/status", get(get_status));

    // Admin routes
    let admin_routes = Router::new()
        .route("/stats", get(get_admin_stats))
        .route("/proxy", get(get_admin_proxy_status))
        .route("/failed", get(get_admin_failed))
        .route("/requests", get(get_admin_requests))
        .route("/request/{id}", delete(delete_request))
        .route("/retry/{id}", post(retry_failed))
        .route("/audit", get(get_audit_log))
        .route("/audit/item/{id}", get(get_audit_for_item))
        .route("/audit/user/{user_id}", get(get_audit_for_user));

    user_routes.nest("/admin", admin_routes)
}
