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

#[derive(Serialize)]
struct BatchContentResponse {
    pub artists: std::collections::HashMap<String, BatchItemResult>,
    pub albums: std::collections::HashMap<String, BatchItemResult>,
    pub tracks: std::collections::HashMap<String, BatchItemResult>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum BatchItemResult {
    Ok { ok: serde_json::Value },
    Error { error: String },
}

async fn admin_get_users(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let manager = &user_manager;
    match manager.get_all_user_handles() {
        Ok(handles) => {
            let mut users: Vec<UserInfo> = vec![];
            for handle in handles {
                if let Ok(Some(user_id)) = manager.get_user_id(&handle) {
                    users.push(UserInfo {
                        user_handle: handle,
                        user_id,
                    });
                }
            }
            Json(users).into_response()
        }
        Err(err) => {
            error!("Error getting users: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
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
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<CreateUserBody>,
) -> Response {
    let manager = &user_manager;

    if body.user_handle.is_empty() {
        return ApiError::bad_request("invalid_user_handle", "User handle cannot be empty")
            .into_response();
    }
    match manager.get_user_id(&body.user_handle) {
        Ok(Some(_)) => {
            return ApiError::conflict("user_handle_exists", "User handle already exists")
                .into_response();
        }
        Ok(None) => {}
        Err(err) => {
            return ApiError::internal("Failed to check user handle", err).into_response();
        }
    }

    let user_id = match manager.add_user(&body.user_handle) {
        Ok(id) => id,
        Err(err) => {
            return ApiError::internal("Failed to create user", err).into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user_id,
            user_handle: body.user_handle,
        }),
    )
        .into_response()
}

async fn admin_delete_user(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = &user_manager;

    // Get user id first
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Prevent self-deletion
    if user_id == session.user_id {
        return (StatusCode::BAD_REQUEST, "Cannot delete your own account").into_response();
    }

    match manager.delete_user(user_id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error deleting user: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Serialize)]
struct UserCredentialsStatusResponse {
    user_handle: String,
    has_password: bool,
}

async fn admin_get_user_credentials_status(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = &user_manager;

    match manager.get_user_credentials(&user_handle) {
        Ok(Some(creds)) => Json(UserCredentialsStatusResponse {
            user_handle,
            has_password: creds.username_password.is_some(),
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
struct SetPasswordBody {
    password: String,
}

async fn admin_set_user_password(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<SetPasswordBody>,
) -> Response {
    let manager = &user_manager;

    // Check if user exists and has password already
    let has_password = match manager.get_user_credentials(&user_handle) {
        Ok(Some(creds)) => creds.username_password.is_some(),
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let result = if has_password {
        manager.update_password_credentials(&user_handle, body.password)
    } else {
        manager.create_password_credentials(&user_handle, body.password)
    };

    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            error!("Error setting user password: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_delete_user_password(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = &user_manager;

    // Check if user exists
    match manager.get_user_credentials(&user_handle) {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.delete_password_credentials(&user_handle) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            error!("Error deleting user password: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_get_user_roles(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = &user_manager;
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_roles(user_id) {
        Ok(roles) => {
            let role_strings: Vec<String> = roles.iter().map(|r| r.as_str().to_owned()).collect();
            Json(UserRolesResponse {
                user_handle,
                roles: role_strings,
            })
            .into_response()
        }
        Err(err) => {
            error!("Error getting user roles: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_add_user_role(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<AddRoleBody>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&body.role) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let (user_id, stored_event) = {
        let manager = &user_manager;
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let stored_event = match manager.set_user_role_with_event(user_id, role, true) {
            Ok(stored) => stored,
            Err(err) => {
                error!("Error atomically adding user role: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        (user_id, stored_event)
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
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((user_handle, role_name)): Path<(String, String)>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&role_name) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let (user_id, stored_event) = {
        let manager = &user_manager;
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let stored_event = match manager.set_user_role_with_event(user_id, role, false) {
            Ok(stored) => stored,
            Err(err) => {
                error!("Error atomically removing user role: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        (user_id, stored_event)
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
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = &user_manager;
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_permissions(user_id) {
        Ok(permissions) => {
            let perm_strings: Vec<String> =
                permissions.iter().map(|p| format!("{:?}", p)).collect();
            Json(UserPermissionsResponse {
                user_handle,
                permissions: perm_strings,
            })
            .into_response()
        }
        Err(err) => {
            error!("Error getting user permissions: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_add_user_extra_permission(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
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

    let (user_id, permission_id, stored_event) = {
        let manager = &user_manager;
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
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
            match manager.add_extra_permission_with_event(user_id, grant) {
                Ok(result) => result,
                Err(err) => {
                    error!("Error atomically adding extra permission: {}", err);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

        (user_id, permission_id, stored_event)
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
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(permission_id): Path<usize>,
) -> Response {
    let (user_id, stored_event) = {
        let manager = &user_manager;
        match manager.remove_extra_permission_with_event(permission_id) {
            Ok(Some((user_id, _permission, stored_event))) => (user_id, stored_event),
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error atomically removing extra permission: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
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
    State(user_manager): State<GuardedUserManager>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match user_manager
        .get_total_bandwidth_summary(params.start_date, params.end_date)
    {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting bandwidth summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get detailed bandwidth usage for all users (admin only)
async fn admin_get_bandwidth_usage(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match user_manager
        .get_all_bandwidth_usage(params.start_date, params.end_date)
    {
        Ok(usage) => Json(usage).into_response(),
        Err(err) => {
            error!("Error getting bandwidth usage: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get bandwidth summary for a specific user (admin only)
async fn admin_get_user_bandwidth_summary(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    let manager = &user_manager;
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_bandwidth_summary(user_id, params.start_date, params.end_date) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting user bandwidth summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get detailed bandwidth usage for a specific user (admin only)
async fn admin_get_user_bandwidth_usage(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    let manager = &user_manager;
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_bandwidth_usage(user_id, params.start_date, params.end_date) {
        Ok(usage) => Json(usage).into_response(),
        Err(err) => {
            error!("Error getting user bandwidth usage: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// Listening statistics admin endpoints (requires ViewAnalytics permission)

/// Get daily listening stats for the platform (admin only)
async fn admin_get_daily_listening_stats(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager
        .get_daily_listening_stats(start_date, end_date)
    {
        Ok(stats) => Json(stats).into_response(),
        Err(err) => {
            error!("Error getting daily listening stats: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get top tracks by play count (admin only)
async fn admin_get_top_tracks(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<TopTracksQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);
    let limit = query.limit.unwrap_or(50).min(500);

    match user_manager
        .get_top_tracks(start_date, end_date, limit)
    {
        Ok(tracks) => Json(tracks).into_response(),
        Err(err) => {
            error!("Error getting top tracks: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get listening stats for a specific track (admin only)
async fn admin_get_track_listening_stats(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(track_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager
        .get_track_listening_stats(&track_id, start_date, end_date)
    {
        Ok(stats) => Json(stats).into_response(),
        Err(err) => {
            error!("Error getting track listening stats: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get listening summary for a specific user (admin only)
async fn admin_get_user_listening_summary(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let manager = &user_manager;
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match manager.get_user_listening_summary(user_id, start_date, end_date) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting user listening summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
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
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
) -> Response {
    // Get connected user IDs from WebSocket connection manager
    let user_ids = connection_manager.get_connected_user_ids().await;
    let count = user_ids.len();

    // Get handles for first 3 users
    let manager = &user_manager;
    let handles: Vec<String> = user_ids
        .into_iter()
        .take(3)
        .filter_map(|user_id| manager.get_user_handle(user_id).ok())
        .flatten()
        .collect();

    Json(OnlineUsersResponse { count, handles }).into_response()
}

/// Get active playback sessions across all users.
async fn admin_get_playback_sessions(
    _session: Session,
    State(playback_session_manager): State<GuardedPlaybackSessionManager>,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let sessions = playback_session_manager.get_active_sessions().await;

    let manager = &user_manager;
    let enriched: Vec<serde_json::Value> = sessions
        .into_iter()
        .map(|s| {
            let handle = manager
                .get_user_handle(s.user_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| format!("user_{}", s.user_id));
            serde_json::json!({
                "user_id": s.user_id,
                "user_handle": handle,
                "devices": s.devices,
            })
        })
        .collect();

    Json(enriched).into_response()
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
    ) -> ServerState {
        // Create connection manager
        let ws_connection_manager = Arc::new(super::websocket::ConnectionManager::new());

        // Create playback session manager for multi-device sync
        let playback_session_manager = Arc::new(super::websocket::PlaybackSessionManager::new(
            ws_connection_manager.clone(),
            user_manager.clone(),
        ));

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

        let database = super::state::DatabaseHandles::new(
            catalog_store.clone(),
            search_vault.clone(),
            user_store,
            user_manager.clone(),
            server_store.clone(),
            show_store.clone(),
            db_registry.clone(),
            enrichment_store.clone(),
        );

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
            playback_session_manager,
            db_registry,
        }
    }
}
