//! Ingestion HTTP routes.
//!
//! Provides endpoints for:
//! - Uploading audio files for ingestion
//! - Checking job status
//! - Managing the human review queue
//! - Admin job management

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::ingestion::{IngestionContextType, IngestionJob, IngestionManager, ReviewQueueItem};
use crate::server::session::Session;
use crate::server::state::{OptionalIngestionManager, ServerState};
use crate::user::Permission;

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub job_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub job: IngestionJob,
}

#[derive(Debug, Serialize)]
pub struct ReviewQueueResponse {
    pub items: Vec<ReviewQueueItem>,
}

/// Request body for file upload
#[derive(Debug, Deserialize)]
pub struct UploadBody {
    /// Original filename
    pub filename: String,
    /// Base64-encoded file data
    pub data: String,
    /// Context type: "spontaneous" or "download_request"
    #[serde(default)]
    pub context_type: Option<String>,
    /// Context ID (e.g., download_queue_item_id)
    #[serde(default)]
    pub context_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveReviewBody {
    /// The selected option ID (e.g., "track:abc123" or "no_match")
    pub selected_option: String,
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

#[derive(Debug, Serialize)]
pub struct IngestionStatsResponse {
    pub pending_jobs: usize,
    pub processing_jobs: usize,
    pub awaiting_review: usize,
    pub completed_jobs: usize,
    pub failed_jobs: usize,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// =============================================================================
// Helper to extract IngestionManager
// =============================================================================

fn get_ingestion_manager(
    im: &OptionalIngestionManager,
) -> Result<&IngestionManager, (StatusCode, &'static str)> {
    im.as_ref().map(|arc| arc.as_ref()).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Ingestion manager not enabled",
    ))
}

// =============================================================================
// User Routes
// =============================================================================

/// POST /upload - Upload a file for ingestion
async fn upload_file(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Json(body): Json<UploadBody>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let user_id = session.user_id.to_string();

    // Parse context type
    let context_type = match body.context_type.as_deref() {
        Some("download_request") => IngestionContextType::DownloadRequest,
        _ => IngestionContextType::Spontaneous,
    };

    // Decode base64 data
    let data = match BASE64.decode(&body.data) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to decode base64 data: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "Invalid base64 data".to_string() }),
            ).into_response();
        }
    };

    if body.filename.is_empty() || data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "No file provided".to_string() }),
        ).into_response();
    }

    debug!("User {} uploading file: {} ({} bytes)", user_id, body.filename, data.len());

    // Clone what we need before the await
    let filename = body.filename.clone();
    let context_id = body.context_id.clone();

    match manager.create_job(&user_id, &filename, &data, context_type, context_id).await {
        Ok(job_id) => {
            info!("Created ingestion job {} for user {}", job_id, user_id);
            Json(UploadResponse {
                job_id,
                status: "PENDING".to_string(),
            }).into_response()
        }
        Err(e) => {
            warn!("Failed to create ingestion job: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            ).into_response()
        }
    }
}

/// GET /job/:id - Get job status
async fn get_job(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager.get_job(&job_id) {
        Ok(Some(job)) => {
            // Only allow users to see their own jobs (unless admin)
            let user_id_str = session.user_id.to_string();
            if job.user_id != user_id_str && !session.has_permission(Permission::ViewAnalytics) {
                return StatusCode::FORBIDDEN.into_response();
            }
            Json(JobStatusResponse { job }).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            warn!("Failed to get job {}: {}", job_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get job").into_response()
        }
    }
}

/// GET /my-jobs - Get user's jobs
async fn get_my_jobs(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let user_id = session.user_id.to_string();

    match manager.list_user_jobs(&user_id, pagination.limit) {
        Ok(jobs) => Json(jobs).into_response(),
        Err(e) => {
            warn!("Failed to list jobs for user {}: {}", user_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to list jobs").into_response()
        }
    }
}

/// POST /job/:id/process - Trigger processing of a pending job
async fn process_job(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager.process_job(&job_id).await {
        Ok(()) => {
            // Return updated job status
            match manager.get_job(&job_id) {
                Ok(Some(job)) => Json(JobStatusResponse { job }).into_response(),
                Ok(None) => StatusCode::NOT_FOUND.into_response(),
                Err(e) => {
                    warn!("Failed to get job after processing: {}", e);
                    StatusCode::OK.into_response()
                }
            }
        }
        Err(e) => {
            warn!("Failed to process job {}: {}", job_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: e.to_string() }),
            ).into_response()
        }
    }
}

/// POST /job/:id/convert - Trigger conversion of a matched job
async fn convert_job(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager.convert_job(&job_id).await {
        Ok(()) => {
            // Return updated job status
            match manager.get_job(&job_id) {
                Ok(Some(job)) => Json(JobStatusResponse { job }).into_response(),
                Ok(None) => StatusCode::NOT_FOUND.into_response(),
                Err(e) => {
                    warn!("Failed to get job after conversion: {}", e);
                    StatusCode::OK.into_response()
                }
            }
        }
        Err(e) => {
            warn!("Failed to convert job {}: {}", job_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: e.to_string() }),
            ).into_response()
        }
    }
}

// =============================================================================
// Review Queue Routes
// =============================================================================

/// GET /reviews - Get pending review items
async fn get_pending_reviews(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager.get_pending_reviews(pagination.limit) {
        Ok(items) => Json(ReviewQueueResponse { items }).into_response(),
        Err(e) => {
            warn!("Failed to get pending reviews: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get reviews").into_response()
        }
    }
}

/// POST /review/:job_id/resolve - Resolve a review
async fn resolve_review(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Path(job_id): Path<String>,
    Json(body): Json<ResolveReviewBody>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let reviewer_id = session.user_id.to_string();

    match manager.resolve_review(&job_id, &reviewer_id, &body.selected_option).await {
        Ok(()) => {
            info!("Review resolved for job {} by {}: {}", job_id, reviewer_id, body.selected_option);
            // Return updated job status
            match manager.get_job(&job_id) {
                Ok(Some(job)) => Json(JobStatusResponse { job }).into_response(),
                Ok(None) => StatusCode::NOT_FOUND.into_response(),
                Err(e) => {
                    warn!("Failed to get job after review: {}", e);
                    StatusCode::OK.into_response()
                }
            }
        }
        Err(e) => {
            warn!("Failed to resolve review for job {}: {}", job_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: e.to_string() }),
            ).into_response()
        }
    }
}

// =============================================================================
// Admin Routes
// =============================================================================

/// GET /admin/jobs - List all ingestion jobs
async fn admin_list_jobs(
    session: Session,
    State(im): State<OptionalIngestionManager>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ViewAnalytics) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&im) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager.list_all_jobs(pagination.limit) {
        Ok(jobs) => Json(jobs).into_response(),
        Err(e) => {
            warn!("Failed to list all jobs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to list jobs").into_response()
        }
    }
}

/// DELETE /admin/job/:id - Delete a job
async fn admin_delete_job(
    session: Session,
    State(_im): State<OptionalIngestionManager>,
    Path(_job_id): Path<String>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // TODO: Implement job deletion
    (StatusCode::NOT_IMPLEMENTED, "Job deletion not implemented").into_response()
}

// =============================================================================
// Router Construction
// =============================================================================

/// Build the ingestion routes.
///
/// User routes (require EditCatalog permission):
/// - POST /upload - Upload file for ingestion
/// - GET /job/:id - Get job status
/// - GET /my-jobs - Get user's jobs
/// - POST /job/:id/process - Trigger processing
/// - POST /job/:id/convert - Trigger conversion
///
/// Review routes (require EditCatalog permission):
/// - GET /reviews - Get pending reviews
/// - POST /review/:job_id/resolve - Resolve a review
///
/// Admin routes (require ViewAnalytics/EditCatalog):
/// - GET /admin/jobs - List all jobs
/// - DELETE /admin/job/:id - Delete a job
pub fn ingestion_routes() -> Router<ServerState> {
    // User routes
    let user_routes = Router::new()
        .route("/upload", post(upload_file))
        .route("/job/{id}", get(get_job))
        .route("/my-jobs", get(get_my_jobs))
        .route("/job/{id}/process", post(process_job))
        .route("/job/{id}/convert", post(convert_job));

    // Review routes
    let review_routes = Router::new()
        .route("/reviews", get(get_pending_reviews))
        .route("/review/{job_id}/resolve", post(resolve_review));

    // Admin routes
    let admin_routes = Router::new()
        .route("/jobs", get(admin_list_jobs))
        .route("/job/{id}", delete(admin_delete_job));

    user_routes
        .merge(review_routes)
        .nest("/admin", admin_routes)
}
