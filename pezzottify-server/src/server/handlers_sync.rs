#[derive(Deserialize)]
struct LoginBody {
    pub user_handle: String,
    pub password: String,
    pub device_uuid: String,
    pub device_type: String,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CreatePlaylistBody {
    pub name: String,
    pub track_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct UpdatePlaylistBody {
    pub name: Option<String>,
    pub track_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
struct LoginSuccessResponse {
    token: String,
    user_handle: String,
    permissions: Vec<Permission>,
}

#[derive(Serialize)]
struct SessionResponse {
    user_handle: String,
    permissions: Vec<Permission>,
}

#[derive(Deserialize, Debug)]
struct AddTracksToPlaylistBody {
    pub tracks_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct RemoveTracksFromPlaylist {
    pub tracks_positions: Vec<usize>,
}

// ========================================================================
// Sync API Types
// ========================================================================

#[derive(Serialize)]
struct SyncStateResponse {
    seq: i64,
    likes: LikesState,
    settings: Vec<UserSetting>,
    playlists: Vec<PlaylistState>,
    permissions: Vec<Permission>,
    notifications: Vec<crate::notifications::Notification>,
}

#[derive(Serialize)]
struct LikesState {
    albums: Vec<String>,
    artists: Vec<String>,
    tracks: Vec<String>,
}

#[derive(Serialize)]
struct PlaylistState {
    id: String,
    name: String,
    tracks: Vec<String>,
}

#[derive(Serialize)]
struct SyncEventsResponse {
    events: Vec<crate::user::sync_events::StoredEvent>,
    current_seq: i64,
}

#[derive(Deserialize)]
struct SyncEventsQuery {
    since: i64,
}

#[derive(Deserialize)]
struct CatalogSyncQuery {
    #[serde(default)]
    since: i64,
}

#[derive(Serialize)]
struct CatalogSyncResponse {
    events: Vec<crate::server_store::CatalogEvent>,
    current_seq: i64,
}

// ========================================================================
// Bug Report API Types
// ========================================================================

#[derive(Deserialize, Debug)]
struct SubmitBugReportBody {
    pub title: Option<String>,
    pub description: String,
    pub client_type: String,
    pub client_version: Option<String>,
    pub device_info: Option<String>,
    pub logs: Option<String>,
    /// JSON array of base64-encoded images
    pub attachments: Option<Vec<String>>,
}

#[derive(Serialize)]
struct SubmitBugReportResponse {
    id: String,
}

#[derive(Deserialize)]
struct ListBugReportsQuery {
    #[serde(default = "default_bug_report_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_bug_report_limit() -> usize {
    50
}

// ========================================================================
// Sync API Handlers
// ========================================================================

fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>, StatusCode> {
    let Some(value) = headers.get("idempotency-key") else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| StatusCode::BAD_REQUEST)?.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Some(value.to_owned()))
}

/// GET /v1/sync/state - Returns full user state for initial sync
async fn get_sync_state(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let um = user_manager.lock().unwrap();
    let snapshot = match um.get_sync_snapshot(session.user_id) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            error!("Error getting consistent sync snapshot: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let playlists = snapshot
        .playlists
        .into_iter()
        .map(|playlist| PlaylistState {
            id: playlist.id,
            name: playlist.name,
            tracks: playlist.tracks,
        })
        .collect();

    Json(SyncStateResponse {
        seq: snapshot.seq,
        likes: LikesState {
            albums: snapshot.liked_albums,
            artists: snapshot.liked_artists,
            tracks: snapshot.liked_tracks,
        },
        settings: snapshot.settings,
        playlists,
        permissions: snapshot.permissions,
        notifications: snapshot.notifications,
    })
    .into_response()
}

/// GET /v1/sync/events - Returns events since a given sequence number
async fn get_sync_events(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<SyncEventsQuery>,
) -> Response {
    let um = user_manager.lock().unwrap();

    // Get current sequence number first (needed for pruning check)
    let current_seq = match um.get_current_seq(session.user_id) {
        Ok(seq) => seq,
        Err(err) => {
            error!("Error getting current seq: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Check if requested sequence has been pruned
    // Return 410 GONE if the event *after* the requested sequence has been pruned.
    // For example, if since=5 and min_seq=10, events 6-9 have been pruned,
    // so we can't provide a continuous stream from seq 5.
    // However, if since=0 and min_seq=1, that's fine - we return event 1.
    if query.since > 0 {
        match um.get_min_seq(session.user_id) {
            Ok(Some(min_seq)) if query.since + 1 < min_seq => {
                // Requested sequence is no longer available
                return StatusCode::GONE.into_response();
            }
            Ok(None) => {
                // No events exist but client is asking for events after some sequence.
                // Either all events were pruned or the client has invalid state.
                // Return 410 to signal the client should reset their sync state.
                return StatusCode::GONE.into_response();
            }
            Err(err) => {
                error!("Error getting min seq: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            _ => {}
        }
    }

    // Get events since the requested sequence
    let events = match um.get_events_since(session.user_id, query.since) {
        Ok(e) => e,
        Err(err) => {
            error!("Error getting events since {}: {}", query.since, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(SyncEventsResponse {
        events,
        current_seq,
    })
    .into_response()
}

/// GET /v1/sync/catalog - Returns catalog events since a given sequence number
///
/// This endpoint allows clients to catch up on catalog changes they may have missed.
/// Use `since=0` to get all events (up to a reasonable limit).
async fn get_catalog_sync(
    _session: Session, // Authentication required but not user-specific
    State(server_store): State<GuardedServerStore>,
    Query(query): Query<CatalogSyncQuery>,
) -> Response {
    // Get events since the requested sequence
    let events = match server_store.get_catalog_events_since(query.since) {
        Ok(e) => e,
        Err(err) => {
            error!(
                "Error getting catalog events since {}: {}",
                query.since, err
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get current sequence number
    let current_seq = match server_store.get_catalog_events_current_seq() {
        Ok(seq) => seq,
        Err(err) => {
            error!("Error getting catalog events current seq: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(CatalogSyncResponse {
        events,
        current_seq,
    })
    .into_response()
}

// ========================================================================
// Bug Report API Handlers
// ========================================================================

use crate::server_store::{
    BugReport, BUG_REPORT_ATTACHMENT_MAX_SIZE, BUG_REPORT_DESCRIPTION_MAX_SIZE,
    BUG_REPORT_LOGS_MAX_SIZE, BUG_REPORT_MAX_ATTACHMENTS, BUG_REPORT_TITLE_MAX_LEN,
    BUG_REPORT_TOTAL_MAX_SIZE,
};

/// POST /v1/user/bug-report - Submit a bug report
async fn submit_bug_report(
    session: Session,
    State(server_store): State<GuardedServerStore>,
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<SubmitBugReportBody>,
) -> Response {
    // Validate title length
    if let Some(ref title) = body.title {
        if title.len() > BUG_REPORT_TITLE_MAX_LEN {
            return (
                StatusCode::BAD_REQUEST,
                format!(
                    "Title exceeds maximum length of {} characters",
                    BUG_REPORT_TITLE_MAX_LEN
                ),
            )
                .into_response();
        }
    }

    // Validate description is not empty and within size limit
    if body.description.is_empty() {
        return (StatusCode::BAD_REQUEST, "Description cannot be empty").into_response();
    }
    if body.description.len() > BUG_REPORT_DESCRIPTION_MAX_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "Description exceeds maximum size of {} bytes",
                BUG_REPORT_DESCRIPTION_MAX_SIZE
            ),
        )
            .into_response();
    }

    // Validate logs size
    if let Some(ref logs) = body.logs {
        if logs.len() > BUG_REPORT_LOGS_MAX_SIZE {
            return (
                StatusCode::BAD_REQUEST,
                format!(
                    "Logs exceed maximum size of {} bytes",
                    BUG_REPORT_LOGS_MAX_SIZE
                ),
            )
                .into_response();
        }
    }

    // Validate attachments
    let attachments_json = if let Some(ref attachments) = body.attachments {
        if attachments.len() > BUG_REPORT_MAX_ATTACHMENTS {
            return (
                StatusCode::BAD_REQUEST,
                format!(
                    "Too many attachments. Maximum is {} images",
                    BUG_REPORT_MAX_ATTACHMENTS
                ),
            )
                .into_response();
        }

        for (i, attachment) in attachments.iter().enumerate() {
            if attachment.len() > BUG_REPORT_ATTACHMENT_MAX_SIZE {
                return (
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Attachment {} exceeds maximum size of {} bytes",
                        i + 1,
                        BUG_REPORT_ATTACHMENT_MAX_SIZE
                    ),
                )
                    .into_response();
            }
        }

        // Convert to JSON string for storage
        match serde_json::to_string(attachments) {
            Ok(json) => Some(json),
            Err(err) => {
                error!("Failed to serialize attachments: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else {
        None
    };

    // Get user handle
    let user_handle = {
        let um = user_manager.lock().unwrap();
        match um.get_user_handle(session.user_id) {
            Ok(Some(handle)) => handle,
            Ok(None) => {
                error!("User {} not found", session.user_id);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            Err(err) => {
                error!("Error getting user handle: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Generate a unique ID
    let id = uuid::Uuid::new_v4().to_string();

    let report = BugReport {
        id: id.clone(),
        user_id: session.user_id,
        user_handle,
        title: body.title,
        description: body.description,
        client_type: body.client_type,
        client_version: body.client_version,
        device_info: body.device_info,
        logs: body.logs,
        attachments: attachments_json,
        created_at: chrono::Utc::now(),
    };

    // Insert the bug report
    if let Err(err) = server_store.insert_bug_report(&report) {
        error!("Failed to insert bug report: {}", err);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Cleanup old reports if total size exceeds limit
    match server_store.cleanup_bug_reports_to_size(BUG_REPORT_TOTAL_MAX_SIZE) {
        Ok(deleted) if deleted > 0 => {
            info!(
                "Cleaned up {} old bug reports to stay under size limit",
                deleted
            );
        }
        Err(err) => {
            warn!("Failed to cleanup old bug reports: {}", err);
            // Don't fail the request, cleanup is best-effort
        }
        _ => {}
    }

    Json(SubmitBugReportResponse { id }).into_response()
}

/// GET /v1/admin/bug-reports - List all bug reports (admin only)
async fn admin_list_bug_reports(
    State(server_store): State<GuardedServerStore>,
    Query(query): Query<ListBugReportsQuery>,
) -> Response {
    match server_store.list_bug_reports(query.limit, query.offset) {
        Ok(reports) => Json(reports).into_response(),
        Err(err) => {
            error!("Failed to list bug reports: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// GET /v1/admin/bug-report/{id} - Get a specific bug report (admin only)
async fn admin_get_bug_report(
    State(server_store): State<GuardedServerStore>,
    Path(id): Path<String>,
) -> Response {
    match server_store.get_bug_report(&id) {
        Ok(Some(report)) => Json(report).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to get bug report: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// DELETE /v1/admin/bug-report/{id} - Delete a bug report (admin only)
async fn admin_delete_bug_report(
    State(server_store): State<GuardedServerStore>,
    Path(id): Path<String>,
) -> Response {
    match server_store.delete_bug_report(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to delete bug report: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// POST /v1/user/notifications/{id}/read - Mark notification as read
async fn mark_notification_read(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(notification_id): Path<String>,
) -> Response {
    let (notification, stored_event) = {
        let um = user_manager.lock().unwrap();

        // Mark as read
        let notification = match um.mark_notification_read(&notification_id, session.user_id) {
            Ok(Some(n)) => n,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error marking notification read: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Log sync event
        let read_at = notification
            .read_at
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        let event = UserEvent::NotificationRead {
            notification_id: notification_id.clone(),
            read_at,
        };

        let stored_event = match um.append_event(session.user_id, &event) {
            Ok(e) => Some(e),
            Err(err) => {
                warn!("Failed to log notification_read event: {}", err);
                None
            }
        };

        (notification, stored_event)
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
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

    // Return the notification (useful for knowing read_at timestamp)
    Json(notification).into_response()
}
