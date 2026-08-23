#[derive(Serialize)]
struct UserInfo {
    pub user_handle: String,
    pub user_id: usize,
}

#[derive(Serialize)]
struct UserRolesResponse {
    pub user_handle: String,
    pub roles: Vec<String>,
}

#[derive(Serialize)]
struct UserPermissionsResponse {
    pub user_handle: String,
    pub permissions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct AddRoleBody {
    pub role: String,
}

#[derive(Deserialize, Debug)]
struct AddExtraPermissionBody {
    pub permission: String,
    pub duration_seconds: Option<u64>,
    pub countdown: Option<u64>,
}

#[derive(Serialize)]
struct AddExtraPermissionResponse {
    pub permission_id: usize,
}

// Listening stats request/response structs

#[derive(Deserialize, Debug)]
struct ListeningEventRequest {
    pub track_id: String,
    pub session_id: Option<String>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub duration_seconds: u32,
    pub track_duration_seconds: u32,
    pub seek_count: Option<u32>,
    pub pause_count: Option<u32>,
    pub playback_context: Option<String>,
    pub client_type: Option<String>,
}

const MAX_TRACK_DURATION_SECONDS: u32 = 24 * 60 * 60;
const MAX_EVENT_AGE_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_FUTURE_CLOCK_SKEW_SECONDS: u64 = 5 * 60;
const MIN_FINALIZED_LISTEN_SECONDS: u32 = 5;
const MAX_PLAYBACK_COUNTER: u32 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ValidatedListeningEvent {
    started_at: u64,
    ended_at: Option<u64>,
    duration_seconds: u32,
    completed: bool,
}

fn authoritative_track_duration_seconds(duration_ms: i64) -> Option<u32> {
    if duration_ms <= 0 {
        return None;
    }
    let seconds = duration_ms.checked_add(999)?.checked_div(1_000)?;
    let seconds = u32::try_from(seconds).ok()?;
    (seconds <= MAX_TRACK_DURATION_SECONDS).then_some(seconds)
}

fn valid_telemetry_label(value: Option<&str>, max_len: usize) -> bool {
    value.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= max_len
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    })
}

fn validate_listening_event(
    body: &ListeningEventRequest,
    authoritative_duration: u32,
    now: u64,
) -> Result<ValidatedListeningEvent, &'static str> {
    if authoritative_duration == 0 || authoritative_duration > MAX_TRACK_DURATION_SECONDS {
        return Err("invalid authoritative track duration");
    }
    if body.track_duration_seconds == 0 {
        return Err("client track duration must be positive");
    }
    let duration_tolerance = authoritative_duration.saturating_div(5).max(2);
    if body.track_duration_seconds.abs_diff(authoritative_duration) > duration_tolerance {
        return Err("client track duration differs implausibly from catalog");
    }
    if body.duration_seconds > authoritative_duration.saturating_add(10) {
        return Err("listening duration exceeds track duration");
    }
    if body.seek_count.unwrap_or(0) > MAX_PLAYBACK_COUNTER
        || body.pause_count.unwrap_or(0) > MAX_PLAYBACK_COUNTER
    {
        return Err("playback counter is implausible");
    }
    if !valid_telemetry_label(body.playback_context.as_deref(), 64)
        || !valid_telemetry_label(body.client_type.as_deref(), 32)
    {
        return Err("invalid telemetry label");
    }
    if !valid_telemetry_label(body.session_id.as_deref(), 128)
        || body
            .session_id
            .as_deref()
            .is_some_and(|session_id| session_id.len() < 8)
    {
        return Err("invalid session identifier");
    }

    let started_at = body.started_at.unwrap_or(now);
    if started_at > now.saturating_add(MAX_FUTURE_CLOCK_SKEW_SECONDS)
        || started_at < now.saturating_sub(MAX_EVENT_AGE_SECONDS)
    {
        return Err("start timestamp is outside the accepted window");
    }

    if let Some(ended_at) = body.ended_at {
        if body.session_id.is_none() {
            return Err("finalized events require a session identifier");
        }
        if body.duration_seconds < MIN_FINALIZED_LISTEN_SECONDS {
            return Err("finalized listen is shorter than the minimum");
        }
        if ended_at < started_at || ended_at > now.saturating_add(MAX_FUTURE_CLOCK_SKEW_SECONDS) {
            return Err("end timestamp is invalid");
        }
        let wall_seconds = ended_at.saturating_sub(started_at);
        if u64::from(body.duration_seconds) > wall_seconds.saturating_mul(2).saturating_add(10) {
            return Err("listening duration is implausible for elapsed time");
        }
    }

    let duration_seconds = body.duration_seconds.min(authoritative_duration);
    let completed = body.ended_at.is_some()
        && u64::from(duration_seconds) * 100 >= u64::from(authoritative_duration) * 90;
    Ok(ValidatedListeningEvent {
        started_at,
        ended_at: body.ended_at,
        duration_seconds,
        completed,
    })
}

#[derive(Serialize)]
struct ListeningEventResponse {
    pub id: usize,
    pub created: bool,
}

#[derive(Deserialize, Debug)]
struct DateRangeQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct ListeningEventsQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct ListeningHistoryQuery {
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct TopTracksQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct PopularContentQuery {
    pub albums_limit: Option<usize>,
    pub artists_limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct FeaturedAlbumsQuery {
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct DiscographyQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
    pub appears_on: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct GenreTracksQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct GenreRadioQuery {
    pub count: Option<usize>,
}

// Batch content request/response types
const BATCH_MAX_ITEMS: usize = 100;

#[derive(Deserialize, Debug)]
struct BatchItemRequest {
    pub id: String,
    #[serde(default)]
    pub resolved: bool,
}

#[derive(Deserialize, Debug, Default)]
struct BatchContentRequest {
    #[serde(default)]
    pub artists: Vec<BatchItemRequest>,
    #[serde(default)]
    pub albums: Vec<BatchItemRequest>,
    #[serde(default)]
    pub tracks: Vec<BatchItemRequest>,
}

#[derive(Clone, Serialize)]
struct BatchContentResponse {
    pub artists: std::collections::HashMap<String, BatchItemResult>,
    pub albums: std::collections::HashMap<String, BatchItemResult>,
    pub tracks: std::collections::HashMap<String, BatchItemResult>,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum BatchItemResult {
    Ok { ok: serde_json::Value },
    Error { error: String },
}

fn admin_user_db_error(context: &'static str, error: crate::db_executor::DbRunError) -> Response {
    match error {
        crate::db_executor::DbRunError::Store(source) => {
            error!("{context}: {source}");
            ApiError::internal(context, source).into_response()
        }
        error => ApiError::from(error).into_response(),
    }
}

enum DeleteUserOutcome {
    Missing,
    SelfDelete,
    Deleted(bool),
}

async fn admin_get_users(
    _session: Session,
    State(database): State<DatabaseHandles>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, |manager| {
            let handles = manager.get_all_user_handles()?;
            let mut users = Vec::with_capacity(handles.len());
            for handle in handles {
                if let Some(user_id) = manager.get_user_id(&handle)? {
                    users.push(UserInfo { user_handle: handle, user_id });
                }
            }
            Ok(users)
        })
        .await
    {
        Ok(users) => Json(users).into_response(),
        Err(error) => admin_user_db_error("Failed to get users", error),
    }
}

#[derive(Deserialize)]
struct CreateUserBody {
    user_handle: String,
}

#[derive(Serialize)]
struct CreateUserResponse {
    user_id: usize,
    user_handle: String,
}

async fn admin_create_user(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Json(body): Json<CreateUserBody>,
) -> Response {
    if body.user_handle.is_empty() {
        return ApiError::bad_request("invalid_user_handle", "User handle cannot be empty")
            .into_response();
    }
    let user_handle = body.user_handle;
    let handle_for_insert = user_handle.clone();
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            if manager.get_user_id(&handle_for_insert)?.is_some() {
                return Ok(None);
            }
            Ok(Some(manager.add_user(&handle_for_insert)?))
        })
        .await
    {
        Ok(Some(user_id)) => (
            StatusCode::CREATED,
            Json(CreateUserResponse { user_id, user_handle }),
        )
            .into_response(),
        Ok(None) => ApiError::conflict("user_handle_exists", "User handle already exists")
            .into_response(),
        Err(error) => admin_user_db_error("Failed to create user", error),
    }
}

async fn admin_delete_user(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
) -> Response {
    let requesting_user_id = session.user_id;
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(DeleteUserOutcome::Missing);
            };
            if user_id == requesting_user_id {
                return Ok(DeleteUserOutcome::SelfDelete);
            }
            Ok(DeleteUserOutcome::Deleted(manager.delete_user(user_id)?))
        })
        .await
    {
        Ok(DeleteUserOutcome::Missing | DeleteUserOutcome::Deleted(false)) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Ok(DeleteUserOutcome::SelfDelete) => {
            (StatusCode::BAD_REQUEST, "Cannot delete your own account").into_response()
        }
        Ok(DeleteUserOutcome::Deleted(true)) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => admin_user_db_error("Failed to delete user", error),
    }
}

#[derive(Serialize)]
struct UserCredentialsStatusResponse {
    user_handle: String,
    has_password: bool,
}

async fn admin_get_user_credentials_status(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
) -> Response {
    let handle_for_query = user_handle.clone();
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_user_credentials(&handle_for_query)
        })
        .await
    {
        Ok(Some(creds)) => Json(UserCredentialsStatusResponse {
            user_handle,
            has_password: creds.username_password.is_some(),
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user credentials", error),
    }
}

#[derive(Deserialize)]
struct SetPasswordBody {
    password: String,
}

async fn admin_set_user_password(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(password_work): State<PasswordWorkPool>,
    Path(user_handle): Path<String>,
    Json(body): Json<SetPasswordBody>,
) -> Response {
    let credentials = match database
        .user_manager
        .run(DbPriority::Interactive, {
            let user_handle = user_handle.clone();
            move |manager| manager.get_user_credentials(&user_handle)
        })
        .await
    {
        Ok(Some(credentials)) => credentials,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return admin_user_db_error("Failed to read user credentials", error),
    };

    let password = match password_work.hash(credentials.user_id, body.password).await {
        Ok(Ok(password)) => password,
        Ok(Err(error)) => {
            return ApiError::internal("Failed to hash user password", error).into_response()
        }
        Err(error) => return ApiError::from(error).into_response(),
    };

    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.set_prehashed_password_credentials(&user_handle, password)
        })
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to persist user password", error),
    }
}

async fn admin_delete_user_password(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            if manager.get_user_credentials(&user_handle)?.is_none() {
                return Ok(false);
            }
            manager.delete_password_credentials(&user_handle)?;
            Ok(true)
        })
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to delete user password", error),
    }
}

async fn admin_get_user_roles(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
) -> Response {
    let handle_for_query = user_handle.clone();
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&handle_for_query)? else {
                return Ok(None);
            };
            Ok(Some(manager.get_user_roles(user_id)?))
        })
        .await
    {
        Ok(Some(roles)) => {
            let role_strings: Vec<String> = roles.iter().map(|r| r.as_str().to_owned()).collect();
            Json(UserRolesResponse {
                user_handle,
                roles: role_strings,
            })
            .into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user roles", error),
    }
}

async fn admin_add_user_role(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<AddRoleBody>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&body.role) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let result = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
            let stored_event = manager.set_user_role_with_event(user_id, role, true)?;
            Ok(Some((user_id, stored_event)))
        })
        .await;
    let (user_id, stored_event) = match result {
        Ok(Some(result)) => result,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return admin_user_db_error("Failed to add user role", error),
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::CREATED.into_response()
}

async fn admin_remove_user_role(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((user_handle, role_name)): Path<(String, String)>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&role_name) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let result = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
            let stored_event = manager.set_user_role_with_event(user_id, role, false)?;
            Ok(Some((user_id, stored_event)))
        })
        .await;
    let (user_id, stored_event) = match result {
        Ok(Some(result)) => result,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return admin_user_db_error("Failed to remove user role", error),
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::OK.into_response()
}

async fn admin_get_user_permissions(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
) -> Response {
    let handle_for_query = user_handle.clone();
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&handle_for_query)? else {
                return Ok(None);
            };
            Ok(Some(manager.get_user_permissions(user_id)?))
        })
        .await
    {
        Ok(Some(permissions)) => {
            let perm_strings: Vec<String> =
                permissions.iter().map(|p| format!("{:?}", p)).collect();
            Json(UserPermissionsResponse {
                user_handle,
                permissions: perm_strings,
            })
            .into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user permissions", error),
    }
}

async fn admin_add_user_extra_permission(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<AddExtraPermissionBody>,
) -> Response {
    use crate::user::PermissionGrant;
    use std::time::{Duration, SystemTime};

    let permission = match body.permission.as_str() {
        "AccessCatalog" => Permission::AccessCatalog,
        "LikeContent" => Permission::LikeContent,
        "OwnPlaylists" => Permission::OwnPlaylists,
        "EditCatalog" => Permission::EditCatalog,
        "ManagePermissions" => Permission::ManagePermissions,
        "ServerAdmin" => Permission::ServerAdmin,
        "ViewAnalytics" => Permission::ViewAnalytics,
        "RequestContent" => Permission::RequestContent,
        "DownloadManagerAdmin" => Permission::DownloadManagerAdmin,
        _ => return (StatusCode::BAD_REQUEST, "Invalid permission").into_response(),
    };

    let result = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
        let start_time = SystemTime::now();
        let end_time = body
            .duration_seconds
            .map(|secs| start_time + Duration::from_secs(secs));

        let grant = PermissionGrant::Extra {
            start_time,
            end_time,
            permission,
            countdown: body.countdown,
        };

            let (permission_id, stored_event) =
                manager.add_extra_permission_with_event(user_id, grant)?;
            Ok(Some((user_id, permission_id, stored_event)))
        })
        .await;
    let (user_id, permission_id, stored_event) = match result {
        Ok(Some(result)) => result,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return admin_user_db_error("Failed to add user permission", error),
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    (
        StatusCode::CREATED,
        Json(AddExtraPermissionResponse { permission_id }),
    )
        .into_response()
}

async fn admin_remove_extra_permission(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(permission_id): Path<usize>,
) -> Response {
    let result = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.remove_extra_permission_with_event(permission_id)
        })
        .await;
    let (user_id, stored_event) = match result {
        Ok(Some((user_id, _permission, stored_event))) => (user_id, stored_event),
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return admin_user_db_error("Failed to remove user permission", error),
    };

    // Broadcast the sync event to user's connected devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::OK.into_response()
}

// Bandwidth statistics endpoints

#[derive(Deserialize, Debug)]
struct BandwidthQueryParams {
    /// Start date in YYYYMMDD format
    start_date: u32,
    /// End date in YYYYMMDD format
    end_date: u32,
}

/// Get bandwidth summary for all users (admin only)
async fn admin_get_bandwidth_summary(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_total_bandwidth_summary(params.start_date, params.end_date)
        })
        .await
    {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => admin_user_db_error("Failed to get bandwidth summary", error),
    }
}

/// Get detailed bandwidth usage for all users (admin only)
async fn admin_get_bandwidth_usage(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_all_bandwidth_usage(params.start_date, params.end_date)
        })
        .await
    {
        Ok(usage) => Json(usage).into_response(),
        Err(error) => admin_user_db_error("Failed to get bandwidth usage", error),
    }
}

/// Get bandwidth summary for a specific user (admin only)
async fn admin_get_user_bandwidth_summary(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
            Ok(Some(manager.get_user_bandwidth_summary(
                user_id,
                params.start_date,
                params.end_date,
            )?))
        })
        .await
    {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user bandwidth summary", error),
    }
}

/// Get detailed bandwidth usage for a specific user (admin only)
async fn admin_get_user_bandwidth_usage(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
            Ok(Some(manager.get_user_bandwidth_usage(
                user_id,
                params.start_date,
                params.end_date,
            )?))
        })
        .await
    {
        Ok(Some(usage)) => Json(usage).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user bandwidth usage", error),
    }
}

// Listening statistics admin endpoints (requires ViewAnalytics permission)

/// Get daily listening stats for the platform (admin only)
async fn admin_get_daily_listening_stats(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_daily_listening_stats(start_date, end_date)
        })
        .await
    {
        Ok(stats) => Json(stats).into_response(),
        Err(error) => admin_user_db_error("Failed to get daily listening stats", error),
    }
}

/// Get top tracks by play count (admin only)
async fn admin_get_top_tracks(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Query(query): Query<TopTracksQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);
    let limit = query.limit.unwrap_or(50).min(500);

    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_top_tracks(start_date, end_date, limit)
        })
        .await
    {
        Ok(tracks) => Json(tracks).into_response(),
        Err(error) => admin_user_db_error("Failed to get top tracks", error),
    }
}

/// Get listening stats for a specific track (admin only)
async fn admin_get_track_listening_stats(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(track_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_track_listening_stats(&track_id, start_date, end_date)
        })
        .await
    {
        Ok(stats) => Json(stats).into_response(),
        Err(error) => admin_user_db_error("Failed to get track listening stats", error),
    }
}

/// Get listening summary for a specific user (admin only)
async fn admin_get_user_listening_summary(
    _session: Session,
    State(database): State<DatabaseHandles>,
    Path(user_handle): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(user_id) = manager.get_user_id(&user_handle)? else {
                return Ok(None);
            };
            Ok(Some(manager.get_user_listening_summary(
                user_id, start_date, end_date,
            )?))
        })
        .await
    {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => admin_user_db_error("Failed to get user listening summary", error),
    }
}

/// Response for online users endpoint
#[derive(Serialize)]
struct OnlineUsersResponse {
    /// Total count of unique users connected via WebSocket
    count: usize,
    /// Handles of first few connected users (up to 3)
    handles: Vec<String>,
}

/// Get count and handles of currently connected users
async fn admin_get_online_users(
    _session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
) -> Response {
    // Get connected user IDs from WebSocket connection manager
    let user_ids = connection_manager.get_connected_user_ids().await;
    let count = user_ids.len();

    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            user_ids
                .into_iter()
                .take(3)
                .map(|user_id| manager.get_user_handle(user_id))
                .collect::<anyhow::Result<Vec<_>>>()
                .map(|handles| handles.into_iter().flatten().collect::<Vec<_>>())
        })
        .await
    {
        Ok(handles) => Json(OnlineUsersResponse { count, handles }).into_response(),
        Err(error) => admin_user_db_error("Failed to get online user handles", error),
    }
}

/// Get active playback sessions across all users.
async fn admin_get_playback_sessions(
    _session: Session,
    State(playback_session_manager): State<GuardedPlaybackSessionManager>,
    State(database): State<DatabaseHandles>,
) -> Response {
    let sessions = playback_session_manager.get_active_sessions().await;
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            sessions
                .into_iter()
                .map(|session| {
                    let handle = manager
                        .get_user_handle(session.user_id)?
                        .unwrap_or_else(|| format!("user_{}", session.user_id));
                    Ok(serde_json::json!({
                        "user_id": session.user_id,
                        "user_handle": handle,
                        "devices": session.devices,
                    }))
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .await
    {
        Ok(enriched) => Json(enriched).into_response(),
        Err(error) => admin_user_db_error("Failed to get playback session users", error),
    }
}

// ============================================================================
// Changelog admin endpoints (requires EditCatalog permission)
// ============================================================================

#[derive(Deserialize)]
struct CreateBatchBody {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct ListBatchesQuery {
    is_open: Option<bool>,
}

/// Create a new changelog batch
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_create_changelog_batch(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Json(_body): Json<CreateBatchBody>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// List changelog batches with optional filter
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_list_changelog_batches(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Query(_query): Query<ListBatchesQuery>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// Get a specific changelog batch by ID
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_get_changelog_batch(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_batch_id): Path<String>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// Close a changelog batch
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_close_changelog_batch(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_batch_id): Path<String>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// Delete a changelog batch (only if empty)
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_delete_changelog_batch(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_batch_id): Path<String>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// Get all changes in a changelog batch
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_get_changelog_batch_changes(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_batch_id): Path<String>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

/// Get change history for a specific entity
/// TODO: Re-enable after implementing changelog for Spotify schema
async fn admin_get_changelog_entity_history(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path((_entity_type, _entity_id)): Path<(String, String)>,
) -> Response {
    // Changelog functionality disabled - Spotify schema is read-only
    (
        StatusCode::NOT_IMPLEMENTED,
        "Changelog not available for Spotify catalog",
    )
        .into_response()
}

impl ServerState {
    /// Create a new ServerState with an already-guarded search vault.
    /// This allows sharing the search vault with background tasks.
    #[allow(clippy::arc_with_non_send_sync, clippy::too_many_arguments)]
    fn new_with_guarded_search_vault(
        config: ServerConfig,
        catalog_store: Arc<dyn CatalogStore>,
        search_vault: super::state::GuardedSearchVault,
        user_manager: GuardedUserManager,
        user_store: Arc<dyn FullUserStore>,
        scheduler_handle: Option<SchedulerHandle>,
        server_store: Arc<dyn crate::server_store::ServerStore>,
        show_store: Arc<dyn crate::shows::ShowStore>,
        db_registry: Arc<crate::backup::DbRegistry>,
        enrichment_store: OptionalEnrichmentStore,
        db_executor: crate::db_executor::DbExecutor,
    ) -> ServerState {
        // Create connection manager
        let ws_connection_manager = Arc::new(super::websocket::ConnectionManager::new());

        // Create auth state store for OIDC flow (always created, even if OIDC is disabled)
        let auth_state_store = Arc::new(crate::oidc::AuthStateStore::new());

        // Create MCP state
        let mcp_state = Arc::new(crate::mcp::handler::create_mcp_state());

        // Create organic indexer for on-demand search index growth
        // Note: This requires a tokio runtime, so we wrap in Option and create later
        let organic_indexer = None;

        // HTTP client for downloading images from external URLs
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let database = super::state::DatabaseHandles::new_with_executor(
            catalog_store.clone(),
            search_vault.clone(),
            user_store,
            user_manager.clone(),
            server_store.clone(),
            show_store.clone(),
            db_registry.clone(),
            enrichment_store.clone(),
            db_executor,
        );

        // Create playback session manager for multi-device sync
        let playback_session_manager = Arc::new(super::websocket::PlaybackSessionManager::new(
            ws_connection_manager.clone(),
            database.user_manager.clone(),
        ));

        ServerState {
            config,
            start_time: Instant::now(),
            catalog_store,
            search_vault,
            user_manager,
            ws_connection_manager,
            scheduler_handle,
            server_store,
            show_store,
            hash: "123456".to_owned(),
            oidc_client: None, // Will be set by make_app if OIDC is configured
            auth_state_store,
            mcp_state,
            organic_indexer,
            http_client,
            download_manager: None, // Will be set by make_app if download manager is enabled
            ingestion_manager: None, // Will be set by make_app if ingestion is enabled
            enrichment_store,
            database,
            password_work: PasswordWorkPool::default(),
            playback_session_manager,
            db_registry,
        }
    }
}
