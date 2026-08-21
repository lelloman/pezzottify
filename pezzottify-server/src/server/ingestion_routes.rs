//! Ingestion HTTP routes.
//!
//! Provides endpoints for:
//! - Uploading audio files for ingestion
//! - Checking job status
//! - Managing the human review queue
//! - Admin job management

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::db_executor::{DbHandle, DbPriority, DbRunError};
use crate::ingestion::{
    IngestionContextType, IngestionError, IngestionFile, IngestionJob, IngestionManager,
    ReviewQueueItem,
};
use crate::server::session::Session;
use crate::server::state::{DatabaseHandles, ServerState};
use crate::user::Permission;

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    /// Primary job ID (first job, for backwards compatibility).
    pub job_id: String,
    /// All job IDs created from this upload.
    pub job_ids: Vec<String>,
    /// Upload session ID (groups jobs from same upload).
    pub session_id: String,
    /// Number of albums detected.
    pub album_count: usize,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub job: IngestionJob,
}

/// Detailed job information including files, candidates, and review data.
#[derive(Debug, Serialize)]
pub struct JobDetailsResponse {
    pub job: IngestionJob,
    pub files: Vec<IngestionFile>,
    pub candidates: Vec<AlbumCandidateSummary>,
    pub review: Option<ReviewQueueItem>,
}

/// Summary of an album candidate for display in the monitor.
#[derive(Debug, Serialize)]
pub struct AlbumCandidateSummary {
    pub id: String,
    pub name: String,
    pub artist_name: String,
    pub track_count: i32,
    pub score: f32,
    pub delta_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct ReviewQueueResponse {
    pub items: Vec<ReviewQueueItem>,
}

/// Request body for file upload (reserved for future base64 upload support).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
    /// Reserved for future pagination support.
    #[serde(default)]
    #[allow(dead_code)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

/// Response for ingestion statistics (reserved for future stats endpoint).
#[derive(Debug, Serialize)]
#[allow(dead_code)]
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
    database: &DatabaseHandles,
) -> Result<&DbHandle<IngestionManager>, (StatusCode, &'static str)> {
    database.ingestion.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Ingestion manager not enabled",
    ))
}

#[derive(Debug, Clone, Copy)]
enum IngestionJobAction {
    View,
    Process,
    Convert,
    ResolveReview,
    Delete,
}

/// Central authorization policy for all ingestion job reads and mutations.
/// Owners need EditCatalog; cross-user access requires the explicit ServerAdmin permission.
fn can_access_ingestion_job(
    session: &Session,
    job: &IngestionJob,
    action: IngestionJobAction,
) -> bool {
    let actor = session.user_id.to_string();
    if job.user_id == actor {
        return session.has_permission(Permission::EditCatalog);
    }
    if session.has_permission(Permission::ServerAdmin) {
        info!(
            actor_user_id = %actor,
            owner_user_id = %job.user_id,
            job_id = %job.id,
            ?action,
            "Server administrator accessed another user's ingestion job"
        );
        return true;
    }
    false
}

async fn authorize_job(
    session: &Session,
    manager: &DbHandle<IngestionManager>,
    job_id: &str,
    action: IngestionJobAction,
) -> Result<IngestionJob, StatusCode> {
    let job_id_owned = job_id.to_string();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_job(&job_id_owned).map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(Some(job)) if can_access_ingestion_job(session, &job, action) => Ok(job),
        Ok(Some(_)) => Err(StatusCode::FORBIDDEN),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(error) => {
            warn!(%error, %job_id, "Failed to load ingestion job for authorization");
            Err(executor_status(&error))
        }
    }
}

fn executor_status(error: &DbRunError) -> StatusCode {
    match error {
        DbRunError::QueueTimeout | DbRunError::ExecutionTimeout | DbRunError::ShuttingDown => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        DbRunError::Panicked(_) | DbRunError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// =============================================================================
// User Routes
// =============================================================================

/// POST /upload - Upload a file for ingestion (multipart/form-data)
async fn upload_file(
    session: Session,
    State(database): State<DatabaseHandles>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog)
        && !session.has_permission(Permission::ServerAdmin)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let user_id = session.user_id.to_string();

    let mut filename: Option<String> = None;
    let mut data: Option<Vec<u8>> = None;
    let mut context_type = IngestionContextType::Spontaneous;
    let mut context_id: Option<String> = None;

    // Process multipart fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => data = Some(bytes.to_vec()),
                    Err(e) => {
                        warn!("Failed to read file data: {}", e);
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: "Failed to read file".to_string(),
                            }),
                        )
                            .into_response();
                    }
                }
            }
            "context_type" => {
                if let Ok(bytes) = field.bytes().await {
                    let value = String::from_utf8_lossy(&bytes);
                    if value == "download_request" {
                        context_type = IngestionContextType::DownloadRequest;
                    }
                }
            }
            "context_id" => {
                if let Ok(bytes) = field.bytes().await {
                    let value = String::from_utf8_lossy(&bytes).to_string();
                    if !value.is_empty() {
                        context_id = Some(value);
                    }
                }
            }
            _ => {}
        }
    }

    let filename = match filename {
        Some(f) if !f.is_empty() => f,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No filename provided".to_string(),
                }),
            )
                .into_response();
        }
    };

    let data = match data {
        Some(d) if !d.is_empty() => d,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No file data provided".to_string(),
                }),
            )
                .into_response();
        }
    };

    debug!(
        "User {} uploading file: {} ({} bytes)",
        user_id,
        filename,
        data.len()
    );

    let runtime = tokio::runtime::Handle::current();
    let upload_user_id = user_id.clone();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            runtime
                .block_on(manager.process_upload(
                    &upload_user_id,
                    &filename,
                    &data,
                    context_type,
                    context_id,
                    session.has_permission(Permission::ServerAdmin),
                ))
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(result) => {
            info!(
                "Created {} ingestion job(s) for user {} (session: {})",
                result.job_ids.len(),
                user_id,
                result.session_id
            );

            // Spawn background tasks to auto-process each job
            for job_id in &result.job_ids {
                let manager_clone = manager.clone();
                let job_id_clone = job_id.clone();
                tokio::spawn(async move {
                    debug!("Auto-processing job {}", job_id_clone);
                    let runtime = tokio::runtime::Handle::current();
                    let logged_job_id = job_id_clone.clone();
                    if let Err(e) = manager_clone
                        .run(DbPriority::Background, move |manager| {
                            runtime
                                .block_on(manager.process_job(&job_id_clone))
                                .map_err(anyhow::Error::new)
                        })
                        .await
                    {
                        warn!("Auto-process failed for job {}: {}", logged_job_id, e);
                    }
                });
            }

            let primary_job_id = result.job_ids.first().cloned().unwrap_or_default();

            Json(UploadResponse {
                job_id: primary_job_id,
                job_ids: result.job_ids,
                session_id: result.session_id,
                album_count: result.album_count,
                status: "PENDING".to_string(),
            })
            .into_response()
        }
        Err(e) => {
            warn!("Failed to create ingestion job: {}", e);
            let status = if matches!(
                &e,
                DbRunError::Store(error)
                    if error.downcast_ref::<IngestionError>().is_some_and(|error| matches!(error, IngestionError::InvalidContext(_)))
            ) {
                StatusCode::BAD_REQUEST
            } else {
                executor_status(&e)
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /job/:id - Get job status
async fn get_job(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match authorize_job(&session, manager, &job_id, IngestionJobAction::View).await {
        Ok(job) => Json(JobStatusResponse { job }).into_response(),
        Err(status) => status.into_response(),
    }
}

/// GET /job/:id/details - Get detailed job information including files, candidates, and review
async fn get_job_details(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    // Get the job
    let job = match authorize_job(&session, manager, &job_id, IngestionJobAction::View).await {
        Ok(job) => job,
        Err(status) => return status.into_response(),
    };

    // Get files for the job
    let files_job_id = job_id.clone();
    let files = match manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_files(&files_job_id).map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(files) => files,
        Err(e) => {
            warn!("Failed to get files for job {}: {}", job_id, e);
            Vec::new()
        }
    };

    // Get candidates from the pending review (if any)
    let details_job_id = job_id.clone();
    let (candidates, review) = match manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .get_job_details(&details_job_id)
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok((cands, rev)) => (cands, rev),
        Err(e) => {
            warn!("Failed to get job details for {}: {}", job_id, e);
            (Vec::new(), None)
        }
    };

    // Convert candidates to summary format
    let candidates = candidates
        .into_iter()
        .map(|c| AlbumCandidateSummary {
            id: c.id,
            name: c.name,
            artist_name: c.artist_name,
            track_count: c.track_count,
            score: c.score,
            delta_ms: c.delta_ms,
        })
        .collect();

    Json(JobDetailsResponse {
        job,
        files,
        candidates,
        review,
    })
    .into_response()
}

/// GET /my-jobs - Get user's jobs
async fn get_my_jobs(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog)
        && !session.has_permission(Permission::ServerAdmin)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let user_id = session.user_id.to_string();

    let queried_user_id = user_id.clone();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .list_user_jobs(&queried_user_id, pagination.limit)
                .map_err(anyhow::Error::new)
        })
        .await
    {
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
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    if let Err(status) =
        authorize_job(&session, manager, &job_id, IngestionJobAction::Process).await
    {
        return status.into_response();
    }

    let process_job_id = job_id.clone();
    let runtime = tokio::runtime::Handle::current();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            runtime
                .block_on(manager.process_job(&process_job_id))
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(()) => {
            // Return updated job status
            let updated_job_id = job_id.clone();
            match manager
                .run(DbPriority::Interactive, move |manager| {
                    manager.get_job(&updated_job_id).map_err(anyhow::Error::new)
                })
                .await
            {
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
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /job/:id/convert - Trigger conversion of a matched job
async fn convert_job(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    if let Err(status) =
        authorize_job(&session, manager, &job_id, IngestionJobAction::Convert).await
    {
        return status.into_response();
    }

    let convert_job_id = job_id.clone();
    let runtime = tokio::runtime::Handle::current();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            runtime
                .block_on(manager.convert_job(&convert_job_id))
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(()) => {
            // Return updated job status
            let updated_job_id = job_id.clone();
            match manager
                .run(DbPriority::Interactive, move |manager| {
                    manager.get_job(&updated_job_id).map_err(anyhow::Error::new)
                })
                .await
            {
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
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Review Queue Routes
// =============================================================================

/// GET /reviews - Get pending review items
async fn get_pending_reviews(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::EditCatalog)
        && !session.has_permission(Permission::ServerAdmin)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let reviews = if session.has_permission(Permission::ServerAdmin) {
        manager
            .run(DbPriority::Interactive, move |manager| {
                manager
                    .get_pending_reviews(pagination.limit)
                    .map_err(anyhow::Error::new)
            })
            .await
    } else {
        let user_id = session.user_id.to_string();
        manager
            .run(DbPriority::Interactive, move |manager| {
                manager
                    .get_pending_reviews_for_user(&user_id, pagination.limit)
                    .map_err(anyhow::Error::new)
            })
            .await
    };
    match reviews {
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
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
    Json(body): Json<ResolveReviewBody>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    if let Err(status) = authorize_job(
        &session,
        manager,
        &job_id,
        IngestionJobAction::ResolveReview,
    )
    .await
    {
        return status.into_response();
    }

    let reviewer_id = session.user_id.to_string();

    let resolved_job_id = job_id.clone();
    let resolving_user_id = reviewer_id.clone();
    let selected_option = body.selected_option.clone();
    let runtime = tokio::runtime::Handle::current();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            runtime
                .block_on(manager.resolve_review(
                    &resolved_job_id,
                    &resolving_user_id,
                    &selected_option,
                ))
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(()) => {
            info!(
                "Review resolved for job {} by {}: {}",
                job_id, reviewer_id, body.selected_option
            );
            // Return updated job status
            let updated_job_id = job_id.clone();
            match manager
                .run(DbPriority::Interactive, move |manager| {
                    manager.get_job(&updated_job_id).map_err(anyhow::Error::new)
                })
                .await
            {
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
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Admin Routes
// =============================================================================

/// GET /admin/jobs - List all ingestion jobs
async fn admin_list_jobs(
    session: Session,
    State(database): State<DatabaseHandles>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    if !session.has_permission(Permission::ServerAdmin) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .list_all_jobs(pagination.limit)
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(jobs) => Json(jobs).into_response(),
        Err(e) => {
            warn!("Failed to list all jobs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to list jobs").into_response()
        }
    }
}

/// DELETE /job/:id - Delete a job (user can delete their own jobs)
async fn delete_job(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let manager = match get_ingestion_manager(&database) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    if let Err(status) = authorize_job(&session, manager, &job_id, IngestionJobAction::Delete).await
    {
        return status.into_response();
    }

    let deleted_job_id = job_id.clone();
    let runtime = tokio::runtime::Handle::current();
    match manager
        .run(DbPriority::Interactive, move |manager| {
            runtime
                .block_on(manager.delete_job(&deleted_job_id))
                .map_err(anyhow::Error::new)
        })
        .await
    {
        Ok(()) => {
            info!("Deleted ingestion job {}", job_id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            warn!("Failed to delete job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
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
/// Admin routes (require ServerAdmin):
/// - GET /admin/jobs - List all jobs
/// - DELETE /admin/job/:id - Delete a job
pub fn ingestion_routes() -> Router<ServerState> {
    // Upload route with 5GB body limit for large FLAC box sets
    // Actual limit is enforced by IngestionManager config (max_upload_size_mb)
    let upload_route = Router::new()
        .route("/upload", post(upload_file))
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024 * 1024)); // 5GB

    // User routes
    let user_routes = Router::new()
        .merge(upload_route)
        .route("/job/{id}", get(get_job).delete(delete_job))
        .route("/job/{id}/details", get(get_job_details))
        .route("/my-jobs", get(get_my_jobs))
        .route("/job/{id}/process", post(process_job))
        .route("/job/{id}/convert", post(convert_job));

    // Review routes
    let review_routes = Router::new()
        .route("/reviews", get(get_pending_reviews))
        .route("/review/{job_id}/resolve", post(resolve_review));

    // Admin routes
    let admin_routes = Router::new().route("/jobs", get(admin_list_jobs));

    user_routes
        .merge(review_routes)
        .nest("/admin", admin_routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(user_id: usize, permissions: Vec<Permission>) -> Session {
        Session {
            user_id,
            token: "test".to_string(),
            permissions,
            device_id: None,
            device_type: None,
        }
    }

    #[test]
    fn owner_with_edit_catalog_can_access_job() {
        let session = session(42, vec![Permission::EditCatalog]);
        let job = IngestionJob::new("job", "42", "album.zip", 1, 1);

        assert!(can_access_ingestion_job(
            &session,
            &job,
            IngestionJobAction::Process
        ));
    }

    #[test]
    fn edit_catalog_does_not_grant_cross_user_access() {
        let session = session(42, vec![Permission::EditCatalog]);
        let job = IngestionJob::new("job", "7", "album.zip", 1, 1);

        assert!(!can_access_ingestion_job(
            &session,
            &job,
            IngestionJobAction::ResolveReview
        ));
    }

    #[test]
    fn server_admin_grants_audited_cross_user_access() {
        let session = session(42, vec![Permission::ServerAdmin]);
        let job = IngestionJob::new("job", "7", "album.zip", 1, 1);

        assert!(can_access_ingestion_job(
            &session,
            &job,
            IngestionJobAction::Delete
        ));
    }

    #[test]
    fn owner_without_edit_catalog_cannot_access_job() {
        let session = session(42, vec![]);
        let job = IngestionJob::new("job", "42", "album.zip", 1, 1);

        assert!(!can_access_ingestion_job(
            &session,
            &job,
            IngestionJobAction::View
        ));
    }
}
