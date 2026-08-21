async fn reboot_server(session: Session) -> Response {
    info!(
        "Server reboot requested by user_id={}, initiating shutdown...",
        session.user_id
    );

    // Spawn a task to exit the process after responding
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        info!("Server shutting down for reboot");
        std::process::exit(0);
    });

    (StatusCode::ACCEPTED, "Server reboot initiated").into_response()
}

async fn admin_prepare_backup(
    session: Session,
    State(database): State<DatabaseHandles>,
) -> Response {
    info!("Backup prepare requested by user_id={}", session.user_id);

    let result = database
        .backup
        .run(DbPriority::Interactive, |db_registry| {
            Ok(crate::backup::prepare_backup(db_registry))
        })
        .await;

    match result {
        Ok(backup_result) => {
            let status = if backup_result.all_succeeded {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, axum::Json(backup_result)).into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

async fn admin_get_storage_report(
    session: Session,
    State(config): State<ServerConfig>,
    State(db_registry): State<super::state::GuardedDbRegistry>,
) -> Response {
    debug!("Storage report requested by user_id={}", session.user_id);

    let db_paths = db_registry.all();
    let result = tokio::task::spawn_blocking(move || {
        super::storage_report::collect_storage_report(&config, db_paths)
    })
    .await;

    match result {
        Ok(report) => (StatusCode::OK, Json(report)).into_response(),
        Err(e) => {
            error!("Storage report task panicked: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ============================================================================
// Admin Job API handlers
// ============================================================================

#[derive(Serialize)]
struct ListJobsResponse {
    jobs: Vec<JobInfo>,
}

#[derive(Serialize)]
struct AdminAudioEmbeddingSpecInfo {
    model: String,
    namespace: String,
}

#[derive(Serialize)]
struct AdminAlbumEmbeddingSpecInfo {
    source_namespace: String,
    target_namespace: String,
    aggregation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantile: Option<f32>,
}

#[derive(Serialize)]
struct AdminAudioEmbeddingCoverageResponse {
    enabled: bool,
    specs: Vec<AdminAudioEmbeddingSpecInfo>,
    coverage: crate::catalog_store::TrackEmbeddingCoverage,
    album_derived: AdminAlbumEmbeddingCoverageResponse,
}

#[derive(Serialize)]
struct AdminAlbumEmbeddingCoverageResponse {
    enabled: bool,
    specs: Vec<AdminAlbumEmbeddingSpecInfo>,
    coverage: crate::catalog_store::AlbumEmbeddingCoverage,
}

async fn admin_list_jobs(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    match handle.list_jobs().await {
        Ok(jobs) => {
            debug!("User {} listed {} jobs", session.user_id, jobs.len());
            (StatusCode::OK, Json(ListJobsResponse { jobs })).into_response()
        }
        Err(e) => {
            error!("Failed to list jobs: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to list jobs"})),
            )
                .into_response()
        }
    }
}

async fn admin_get_audio_embedding_coverage(
    session: Session,
    State(state): State<ServerState>,
) -> Response {
    let (enabled, specs, album_enabled, album_specs) = match state.config.audio_embeddings.as_ref()
    {
        Some(settings) => (
            settings.enabled,
            settings.specs.clone(),
            settings.album_derivations.enabled,
            settings.album_derivations.specs.clone(),
        ),
        None => (
            false,
            AudioEmbeddingSpec::defaults(),
            false,
            AlbumEmbeddingDerivationSpec::defaults(),
        ),
    };
    let namespaces = specs
        .iter()
        .map(|spec| spec.namespace.clone())
        .collect::<Vec<_>>();
    let album_namespaces = album_specs
        .iter()
        .map(|spec| spec.target_namespace.clone())
        .collect::<Vec<_>>();

    let catalog_store = Arc::clone(&state.catalog_store);
    let media_path = state.config.media_path.clone();
    let coverage_result = tokio::time::timeout(
        AUDIO_EMBEDDING_COVERAGE_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            let track_coverage = catalog_store.get_track_embedding_coverage(&namespaces);
            let album_coverage =
                catalog_store.get_album_embedding_coverage(&album_namespaces, &media_path);
            (track_coverage, album_coverage)
        }),
    )
    .await;

    let (coverage_result, album_coverage_result) = match coverage_result {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            error!("Failed to join audio embedding coverage task: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get audio embedding coverage"})),
            )
                .into_response();
        }
        Err(_) => {
            warn!(
                "Audio embedding coverage timed out after {:?}",
                AUDIO_EMBEDDING_COVERAGE_TIMEOUT
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Audio embedding coverage timed out"})),
            )
                .into_response();
        }
    };

    match (coverage_result, album_coverage_result) {
        (Ok(coverage), Ok(album_coverage)) => {
            debug!(
                "User {} retrieved audio embedding coverage: missing_any={}",
                session.user_id, coverage.tracks_missing_any_embedding
            );
            let specs = specs
                .into_iter()
                .map(|spec| AdminAudioEmbeddingSpecInfo {
                    model: spec.model,
                    namespace: spec.namespace,
                })
                .collect();
            let album_specs = album_specs
                .into_iter()
                .map(|spec| AdminAlbumEmbeddingSpecInfo {
                    source_namespace: spec.source_namespace,
                    target_namespace: spec.target_namespace,
                    aggregation: spec.aggregation.as_str().to_string(),
                    quantile: spec.aggregation.quantile(),
                })
                .collect();
            (
                StatusCode::OK,
                Json(AdminAudioEmbeddingCoverageResponse {
                    enabled,
                    specs,
                    coverage,
                    album_derived: AdminAlbumEmbeddingCoverageResponse {
                        enabled: album_enabled,
                        specs: album_specs,
                        coverage: album_coverage,
                    },
                }),
            )
                .into_response()
        }
        (Err(e), _) | (_, Err(e)) => {
            error!("Failed to get audio embedding coverage: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get audio embedding coverage"})),
            )
                .into_response()
        }
    }
}

async fn admin_get_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    match handle.get_job(&job_id).await {
        Ok(Some(job)) => {
            debug!("User {} retrieved job {}", session.user_id, job_id);
            (StatusCode::OK, Json(job)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Job not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job"})),
            )
                .into_response()
        }
    }
}

/// Request body for triggering a job with optional parameters.
#[derive(Debug, Deserialize, Default)]
struct TriggerJobRequest {
    /// Optional parameters to pass to the job's execute_with_params() method.
    #[serde(default)]
    params: Option<serde_json::Value>,
}

async fn admin_trigger_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
    body: Result<Json<TriggerJobRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    // Accept either valid JSON body or no body at all (treat as empty params)
    let params = match body {
        Ok(Json(req)) => req.params,
        Err(_) => None, // No body or invalid JSON = no params
    };

    info!(
        "User {} triggering job {} with params: {:?}",
        session.user_id, job_id, params
    );

    match handle.trigger_job(&job_id, params).await {
        Ok(()) => {
            info!(
                "Job {} triggered successfully by user {}",
                job_id, session.user_id
            );
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({"status": "triggered", "job_id": job_id})),
            )
                .into_response()
        }
        Err(JobError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Job not found"})),
        )
            .into_response(),
        Err(JobError::AlreadyRunning) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Job is already running"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to trigger job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to trigger job: {}", e)})),
            )
                .into_response()
        }
    }
}

async fn admin_cancel_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    info!("User {} cancelling job {}", session.user_id, job_id);

    match handle.cancel_job(&job_id).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"status": "cancelling", "job_id": job_id})),
        )
            .into_response(),
        Err(JobError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Job not found"})),
        )
            .into_response(),
        Err(JobError::NotRunning) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Job is not running"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to cancel job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to cancel job: {}", e)})),
            )
                .into_response()
        }
    }
}

async fn admin_get_job_history(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    match handle.get_job_history(&job_id, limit) {
        Ok(history) => {
            debug!(
                "User {} retrieved {} history entries for job {}",
                session.user_id,
                history.len(),
                job_id
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"history": history})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job history for {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job history"})),
            )
                .into_response()
        }
    }
}

async fn get_challenge(State(_state): State<ServerState>) -> Response {
    todo!()
}

async fn post_challenge(State(_state): State<ServerState>) -> Response {
    todo!()
}

/// Get job audit log entries (all jobs).
async fn admin_get_job_audit_log(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    match handle.get_job_audit_log(limit, offset) {
        Ok(entries) => {
            debug!(
                "User {} retrieved {} job audit log entries",
                session.user_id,
                entries.len()
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"entries": entries})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job audit log: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job audit log"})),
            )
                .into_response()
        }
    }
}

/// Get job audit log entries for a specific job.
async fn admin_get_job_audit_log_by_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    match handle.get_job_audit_log_by_job(&job_id, limit, offset) {
        Ok(entries) => {
            debug!(
                "User {} retrieved {} job audit log entries for job {}",
                session.user_id,
                entries.len(),
                job_id
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"entries": entries})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job audit log for {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job audit log"})),
            )
                .into_response()
        }
    }
}

// Admin endpoint types and handlers
