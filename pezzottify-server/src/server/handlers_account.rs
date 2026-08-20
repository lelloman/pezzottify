async fn post_listening_event(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(body): Json<ListeningEventRequest>,
) -> Response {
    use std::time::SystemTime;

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if body.track_id.is_empty() || body.track_id.len() > 128 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let track = match catalog_store.get_track(&body.track_id) {
        Ok(Some(track)) => track,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            error!("Failed to resolve listening-event track: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let authoritative_duration = match authoritative_track_duration_seconds(track.duration_ms) {
        Some(duration) => duration,
        None => {
            error!(
                "Catalog track has an invalid duration: track_id={}",
                track.id
            );
            return StatusCode::UNPROCESSABLE_ENTITY.into_response();
        }
    };
    let validated = match validate_listening_event(&body, authoritative_duration, now_secs) {
        Ok(validated) => validated,
        Err(reason) => {
            debug!("Rejected implausible listening event: {reason}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Calculate date in YYYYMMDD format from the bounded start timestamp.
    let started_at = validated.started_at;
    let date = {
        let datetime =
            chrono::DateTime::from_timestamp(started_at as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    // Capture values for metrics before moving into event
    let client_type_for_metrics = body.client_type.clone();
    let duration_for_metrics = validated.duration_seconds;

    let event = crate::user::ListeningEvent {
        id: None,
        user_id: session.user_id,
        track_id: body.track_id,
        session_id: body.session_id,
        started_at,
        ended_at: validated.ended_at,
        duration_seconds: validated.duration_seconds,
        track_duration_seconds: authoritative_duration,
        completed: validated.completed,
        seek_count: body.seek_count.unwrap_or(0),
        pause_count: body.pause_count.unwrap_or(0),
        playback_context: body.playback_context,
        client_type: body.client_type,
        date,
    };

    match user_manager.record_listening_event(event) {
        Ok((id, created)) => {
            // Record metrics only for newly created events
            if created {
                super::metrics::record_listening_event(
                    client_type_for_metrics.as_deref(),
                    validated.completed,
                    duration_for_metrics,
                );
            }
            Json(ListeningEventResponse { id, created }).into_response()
        }
        Err(err) => {
            error!("Error recording listening event: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_summary(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager.get_user_listening_summary(
        session.user_id,
        start_date,
        end_date,
    ) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting listening summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_history(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<ListeningHistoryQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).min(500);

    match user_manager
        .get_user_listening_history(session.user_id, limit)
    {
        Ok(history) => Json(history).into_response(),
        Err(err) => {
            error!("Error getting listening history: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_events(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<ListeningEventsQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager.get_user_listening_events(
        session.user_id,
        start_date,
        end_date,
        query.limit,
        query.offset,
    ) {
        Ok(events) => Json(events).into_response(),
        Err(err) => {
            error!("Error getting listening events: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Impression Tracking Endpoints
// =============================================================================

/// Request body for recording an impression (page view).
#[derive(Deserialize)]
struct ImpressionBody {
    /// Item type: "artist", "album", or "track"
    item_type: String,
    /// Item ID (Spotify ID)
    item_id: String,
}

/// POST /v1/user/impression - Record a page view impression
///
/// Records that a user viewed an artist, album, or track page.
/// This data is used for popularity scoring.
async fn post_impression(
    session: Session,
    State(search_vault): State<super::state::GuardedSearchVault>,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(body): Json<ImpressionBody>,
) -> StatusCode {
    // Parse item type
    let item_type = match body.item_type.to_lowercase().as_str() {
        "artist" => crate::search::HashedItemType::Artist,
        "album" => crate::search::HashedItemType::Album,
        "track" => crate::search::HashedItemType::Track,
        _ => return StatusCode::BAD_REQUEST,
    };

    // Validate item_id is not empty
    if body.item_id.is_empty() || body.item_id.len() > 128 {
        return StatusCode::BAD_REQUEST;
    }

    let exists = match item_type {
        crate::search::HashedItemType::Artist => catalog_store.get_artist_json(&body.item_id),
        crate::search::HashedItemType::Album => catalog_store.get_album_json(&body.item_id),
        crate::search::HashedItemType::Track => catalog_store.get_track_json(&body.item_id),
    };
    match exists {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(error) => {
            error!("Failed to validate impression catalog entity: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    // Record the impression in a blocking task to avoid blocking the async runtime
    // while waiting for the write_conn mutex (which may be held by long-running index operations)
    let item_id = body.item_id;
    let source = crate::search::ImpressionSource {
        user_id: session.user_id,
        device_id: session.device_id,
    };
    let recorded = tokio::task::spawn_blocking(move || {
        search_vault.record_impression(&item_id, item_type, source);
    })
    .await;
    if let Err(error) = recorded {
        error!("Failed to execute impression recording task: {error}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

/// Helper to get default date range (last 30 days if not specified)
fn get_default_date_range(start_date: Option<u32>, end_date: Option<u32>) -> (u32, u32) {
    use std::time::SystemTime;

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let end = end_date.unwrap_or_else(|| {
        let datetime =
            chrono::DateTime::from_timestamp(now_secs as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    });

    let start = start_date.unwrap_or_else(|| {
        let thirty_days_ago = now_secs - (30 * 24 * 60 * 60);
        let datetime = chrono::DateTime::from_timestamp(thirty_days_ago as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    });

    (start, end)
}

// User settings endpoints

#[derive(Deserialize)]
struct UpdateSettingsBody {
    settings: Vec<UserSetting>,
}

#[derive(Serialize)]
struct UserSettingsResponse {
    settings: Vec<UserSetting>,
}

async fn get_user_settings(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    match user_manager
        .get_all_user_settings(session.user_id)
    {
        Ok(settings) => Json(UserSettingsResponse { settings }).into_response(),
        Err(err) => {
            error!("Error getting user settings: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn update_user_settings(
    session: Session,
    headers: HeaderMap,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Json(body): Json<UpdateSettingsBody>,
) -> Response {
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let stored_events = {
        let locked_manager = &user_manager;
        match locked_manager.set_user_settings_with_events(
            session.user_id,
            body.settings,
            operation_id.as_deref(),
        ) {
            Ok(events) => events,
            Err(err) => {
                error!("Error atomically updating user settings: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Broadcast all events to other devices
    if let Some(device_id) = session.device_id {
        for stored_event in stored_events {
            let ws_msg = super::websocket::messages::ServerMessage::new(
                super::websocket::messages::msg_types::SYNC,
                super::websocket::messages::sync::SyncEventMessage {
                    event: stored_event,
                },
            );
            connection_manager
                .send_to_other_devices(session.user_id, device_id, ws_msg)
                .await;
        }
    }

    StatusCode::OK.into_response()
}

// Device sharing endpoints

#[derive(Deserialize)]
struct DeviceSharePolicyRequest {
    mode: String,
    #[serde(default)]
    allow_users: Vec<usize>,
    #[serde(default)]
    allow_roles: Vec<String>,
    #[serde(default)]
    deny_users: Vec<usize>,
}

#[derive(Serialize)]
struct DeviceSharePolicyResponse {
    mode: String,
    allow_users: Vec<usize>,
    allow_roles: Vec<String>,
    deny_users: Vec<usize>,
}

#[derive(Serialize)]
struct DeviceInfoResponse {
    id: usize,
    device_uuid: String,
    device_type: String,
    device_name: Option<String>,
    os_info: Option<String>,
    first_seen: u64,
    last_seen: u64,
    share_policy: DeviceSharePolicyResponse,
}

#[derive(Serialize)]
struct DevicesResponse {
    devices: Vec<DeviceInfoResponse>,
}

fn policy_mode_to_str(mode: DeviceShareMode) -> &'static str {
    match mode {
        DeviceShareMode::AllowEveryone => "allow_everyone",
        DeviceShareMode::DenyEveryone => "deny_everyone",
        DeviceShareMode::Custom => "custom",
    }
}

fn policy_to_response(policy: DeviceSharePolicy) -> DeviceSharePolicyResponse {
    DeviceSharePolicyResponse {
        mode: policy_mode_to_str(policy.mode).to_string(),
        allow_users: policy.allow_users,
        allow_roles: policy
            .allow_roles
            .into_iter()
            .map(|r| r.as_str().to_lowercase())
            .collect(),
        deny_users: policy.deny_users,
    }
}

async fn get_user_devices(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let devices = match user_manager
        .get_user_devices(session.user_id)
    {
        Ok(devices) => devices,
        Err(err) => {
            error!("Error getting user devices: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut result = Vec::new();
    for device in devices {
        let policy = user_manager
            .get_device_share_policy(device.id)
            .unwrap_or_default();
        result.push(DeviceInfoResponse {
            id: device.id,
            device_uuid: device.device_uuid,
            device_type: device.device_type.as_str().to_string(),
            device_name: device.device_name,
            os_info: device.os_info,
            first_seen: device
                .first_seen
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_seen: device
                .last_seen
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            share_policy: policy_to_response(policy),
        });
    }

    Json(DevicesResponse { devices: result }).into_response()
}

async fn put_device_share_policy(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(playback_session_manager): State<GuardedPlaybackSessionManager>,
    Path(device_id): Path<usize>,
    Json(body): Json<DeviceSharePolicyRequest>,
) -> Response {
    let device = match user_manager.get_device(device_id) {
        Ok(Some(device)) => device,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting device: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if device.user_id != Some(session.user_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let mode = match body.mode.as_str() {
        "allow_everyone" => DeviceShareMode::AllowEveryone,
        "deny_everyone" => DeviceShareMode::DenyEveryone,
        "custom" => DeviceShareMode::Custom,
        _ => return (StatusCode::BAD_REQUEST, "Invalid mode").into_response(),
    };

    let mut allow_roles = Vec::new();
    for role in body.allow_roles {
        let parsed = match UserRole::from_str(&role) {
            Some(r) => r,
            None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
        };
        allow_roles.push(parsed);
    }

    let policy = DeviceSharePolicy {
        mode,
        allow_users: body.allow_users,
        allow_roles,
        deny_users: body.deny_users,
    };

    if let Err(err) = policy.validate() {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }

    if let Err(err) = user_manager
        .set_device_share_policy(device_id, &policy)
    {
        error!("Error setting device share policy: {}", err);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    playback_session_manager
        .broadcast_device_list_refresh(device_id)
        .await;

    Json(policy_to_response(policy)).into_response()
}

