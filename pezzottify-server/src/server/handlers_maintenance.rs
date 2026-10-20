// Maintenance mode handlers and the request gate shared by every API route.

/// Public, unauthenticated status probe used by the web and Android clients.
async fn server_status(State(maintenance): State<MaintenanceHandle>) -> Response {
    (StatusCode::OK, Json(maintenance.snapshot())).into_response()
}

async fn admin_get_maintenance(State(maintenance): State<MaintenanceHandle>) -> Response {
    (StatusCode::OK, Json(maintenance.snapshot())).into_response()
}

#[derive(Debug, Deserialize)]
struct MaintenanceUpdateRequest {
    enabled: bool,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    estimated_end: Option<String>,
}

async fn admin_set_maintenance(
    Extract(session): Extract<Session>,
    State(maintenance): State<MaintenanceHandle>,
    State(database): State<DatabaseHandles>,
    Json(request): Json<MaintenanceUpdateRequest>,
) -> Response {
    let message = request
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_owned);
    if message
        .as_ref()
        .is_some_and(|message| message.chars().count() > 2_000)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "message must be at most 2000 characters"})),
        )
            .into_response();
    }

    let estimated_end =
        match MaintenanceState::normalize_estimated_end(request.estimated_end.as_deref()) {
            Ok(value) => value,
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": error })),
                )
                    .into_response();
            }
        };

    let next = MaintenanceState {
        enabled: request.enabled,
        message,
        estimated_end,
    };
    let result = maintenance
        .persist_and_replace(database.server.clone(), move |store| {
            next.persist(store)?;
            Ok(next)
        })
        .await;

    match result {
        Ok(state) => {
            info!(
                "User {} set maintenance mode enabled={}",
                session.user_id, state.enabled
            );
            (StatusCode::OK, Json(state)).into_response()
        }
        Err(error) => {
            error!(%error, "Failed to persist maintenance mode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to persist maintenance mode"})),
            )
                .into_response()
        }
    }
}

/// Reject player traffic while maintenance mode is on. Status, auth and admin
/// routes stay reachable so clients can render the banner and admins can disable it.
async fn maintenance_gate(
    State(maintenance): State<MaintenanceHandle>,
    request: simple_server::web::extract::Request,
    next: simple_server::web::middleware::Next,
) -> Response {
    let path = request.uri().path();
    if maintenance.is_enabled()
        && path.starts_with("/v1/")
        && !crate::server::maintenance::path_is_exempt(path)
    {
        let state = maintenance.snapshot();
        let mut response = (StatusCode::SERVICE_UNAVAILABLE, Json(state)).into_response();
        response
            .headers_mut()
            .insert(MAINTENANCE_HEADER, HeaderValue::from_static("1"));
        return response;
    }
    next.run(request).await
}
