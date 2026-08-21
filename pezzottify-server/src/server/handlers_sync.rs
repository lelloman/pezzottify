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
    has_more: bool,
    next_since: i64,
}

impl CatalogSyncResponse {
    fn complete(events: Vec<crate::server_store::CatalogEvent>, current_seq: i64) -> Self {
        Self {
            events,
            current_seq,
            has_more: false,
            next_since: current_seq,
        }
    }
}

#[cfg(test)]
mod catalog_sync_response_tests {
    use super::CatalogSyncResponse;

    #[test]
    fn complete_page_advances_to_the_server_sequence() {
        let response = CatalogSyncResponse::complete(Vec::new(), 42);
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["events"], serde_json::json!([]));
        assert_eq!(json["current_seq"], 42);
        assert_eq!(json["has_more"], false);
        assert_eq!(json["next_since"], 42);
    }
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
    State(database): State<DatabaseHandles>,
) -> Response {
    let user_id = session.user_id;
    let snapshot = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_sync_snapshot(user_id)
        })
        .await
    {
        Ok(snapshot) => snapshot,
        Err(error) => return ApiError::user_database(error).into_response(),
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
    State(database): State<DatabaseHandles>,
    Query(query): Query<SyncEventsQuery>,
) -> Response {
    let user_id = session.user_id;
    let since = query.since;
    let sync = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let current_seq = manager.get_current_seq(user_id)?;

            // Return no payload when the event after `since` has been pruned.
            if since > 0 {
                match manager.get_min_seq(user_id)? {
                    Some(min_seq) if since + 1 >= min_seq => {}
                    _ => return Ok(None),
                }
            }

            let events = manager.get_events_since(user_id, since)?;
            Ok(Some((events, current_seq)))
        })
        .await;
    let (events, current_seq) = match sync {
        Ok(Some(sync)) => sync,
        Ok(None) => return StatusCode::GONE.into_response(),
        Err(error) => return ApiError::user_database(error).into_response(),
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
    State(database): State<DatabaseHandles>,
    Query(query): Query<CatalogSyncQuery>,
) -> Response {
    let since = query.since;
    let (events, current_seq) = match database
        .server
        .run(DbPriority::Interactive, move |store| {
            Ok((
                store.get_catalog_events_since(since)?,
                store.get_catalog_events_current_seq()?,
            ))
        })
        .await
    {
        Ok(sync) => sync,
        Err(err) => return ApiError::from(err).into_response(),
    };

    Json(CatalogSyncResponse::complete(events, current_seq)).into_response()
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
    State(database): State<DatabaseHandles>,
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
    let user_id = session.user_id;
    let user_handle = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_user_handle(user_id)
        })
        .await
    {
        Ok(Some(handle)) => handle,
        Ok(None) => {
            error!("User {} not found", session.user_id);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(err) => return ApiError::user_database(err).into_response(),
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

    let cleanup = match database
        .server
        .run(DbPriority::Interactive, move |store| {
            store.insert_bug_report(&report)?;
            Ok(store.cleanup_bug_reports_to_size(BUG_REPORT_TOTAL_MAX_SIZE))
        })
        .await
    {
        Ok(cleanup) => cleanup,
        Err(err) => return ApiError::from(err).into_response(),
    };

    match cleanup {
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
    State(database): State<DatabaseHandles>,
    Query(query): Query<ListBugReportsQuery>,
) -> Response {
    match database
        .server
        .run(DbPriority::Interactive, move |store| {
            store.list_bug_reports(query.limit, query.offset)
        })
        .await
    {
        Ok(reports) => Json(reports).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// GET /v1/admin/bug-report/{id} - Get a specific bug report (admin only)
async fn admin_get_bug_report(
    State(database): State<DatabaseHandles>,
    Path(id): Path<String>,
) -> Response {
    match database
        .server
        .run(DbPriority::Interactive, move |store| store.get_bug_report(&id))
        .await
    {
        Ok(Some(report)) => Json(report).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// DELETE /v1/admin/bug-report/{id} - Delete a bug report (admin only)
async fn admin_delete_bug_report(
    State(database): State<DatabaseHandles>,
    Path(id): Path<String>,
) -> Response {
    match database
        .server
        .run(DbPriority::Interactive, move |store| store.delete_bug_report(&id))
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// POST /v1/user/notifications/{id}/read - Mark notification as read
async fn mark_notification_read(
    session: Session,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(notification_id): Path<String>,
) -> Response {
    let user_id = session.user_id;
    let updated = database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            let Some(notification) = manager.mark_notification_read(&notification_id, user_id)?
            else {
                return Ok(None);
            };

            let read_at = notification
                .read_at
                .unwrap_or_else(|| chrono::Utc::now().timestamp());
            let event = UserEvent::NotificationRead {
                notification_id,
                read_at,
            };
            let stored_event = match manager.append_event(user_id, &event) {
                Ok(event) => Some(event),
                Err(error) => {
                    warn!("Failed to log notification_read event: {}", error);
                    None
                }
            };
            Ok(Some((notification, stored_event)))
        })
        .await;
    let (notification, stored_event) = match updated {
        Ok(Some(updated)) => updated,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return ApiError::user_database(error).into_response(),
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
