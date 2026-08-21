fn parse_content_type(content_type_str: &str) -> Option<LikedContentType> {
    match content_type_str {
        "artist" => Some(LikedContentType::Artist),
        "album" => Some(LikedContentType::Album),
        "track" => Some(LikedContentType::Track),
        _ => None,
    }
}

async fn add_user_liked_content(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_event = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .set_user_liked_content_with_event(
                    user_id,
                    &content_id,
                    content_type,
                    true,
                    operation_id.as_deref(),
                )
        })
        .await
    {
        Ok(stored) => stored,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices if we have a stored event and device_id
    if let Some(device_id) = session.device_id {
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

    StatusCode::OK.into_response()
}

async fn delete_user_liked_content(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_event = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .set_user_liked_content_with_event(
                    user_id,
                    &content_id,
                    content_type,
                    false,
                    operation_id.as_deref(),
                )
        })
        .await
    {
        Ok(stored) => stored,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices if we have a stored event and device_id
    if let Some(device_id) = session.device_id {
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

    StatusCode::OK.into_response()
}

async fn get_user_liked_content(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(content_type_str): Path<String>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let user_id = session.user_id;
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_user_liked_content(user_id, content_type)
        })
        .await
    {
        Ok(liked_content) => Json(liked_content).into_response(),
        Err(error) => ApiError::user_database(error).into_response(),
    }
}

async fn post_playlist(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Json(body): Json<CreatePlaylistBody>,
) -> Response {
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let (id, stored_event) = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .create_user_playlist_with_event(
                    user_id,
                    &body.name,
                    user_id,
                    body.track_ids,
                    operation_id.as_deref(),
                )
                .map_err(Into::into)
        })
        .await
    {
        Ok(result) => result,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices
    if let Some(device_id) = session.device_id {
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

    Json(id).into_response()
}

async fn put_playlist(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePlaylistBody>,
) -> Response {
    debug!("Updating playlist with id {}", id);
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_events = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .update_user_playlist_with_events(
                    &id,
                    user_id,
                    body.name,
                    body.track_ids,
                    operation_id.as_deref(),
                )
                .map_err(Into::into)
        })
        .await
    {
        Ok(events) => events,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices
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

async fn delete_playlist(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
) -> Response {
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_event = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .delete_user_playlist_with_event(&id, user_id, operation_id.as_deref())
                .map_err(Into::into)
        })
        .await
    {
        Ok(stored) => stored,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices
    if let Some(device_id) = session.device_id {
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

    StatusCode::OK.into_response()
}

async fn get_playlist(
    session: Session,
    State(database): State<DatabaseHandles>,
    Path(id): Path<String>,
) -> Response {
    let user_id = session.user_id;
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .get_user_playlist(&id, user_id)
                .map_err(Into::into)
        })
        .await
    {
        Ok(playlist) => Json(playlist).into_response(),
        Err(error) => ApiError::user_database(error).into_response(),
    }
}

async fn add_playlist_tracks(
    session: Session,
    headers: HeaderMap,
    State(catalog_store): State<GuardedCatalogStore>,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<AddTracksToPlaylistBody>,
) -> Response {
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_events = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .add_playlist_tracks_with_event(
                    catalog_store.as_ref(),
                    &id,
                    user_id,
                    body.tracks_ids,
                    operation_id.as_deref(),
                )
                .map_err(Into::into)
        })
        .await
    {
        Ok(events) => events,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices
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

async fn remove_tracks_from_playlist(
    session: Session,
    headers: HeaderMap,
    State(database): State<DatabaseHandles>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<RemoveTracksFromPlaylist>,
) -> Response {
    let operation_id = match idempotency_key(&headers) {
        Ok(value) => value,
        Err(status) => return status.into_response(),
    };
    let user_id = session.user_id;
    let stored_events = match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager
                .remove_tracks_from_playlist_with_event(
                    &id,
                    user_id,
                    body.tracks_positions,
                    operation_id.as_deref(),
                )
                .map_err(Into::into)
        })
        .await
    {
        Ok(events) => events,
        Err(error) => return ApiError::user_database(error).into_response(),
    };

    // Broadcast to other devices
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

async fn get_user_playlists(
    session: Session,
    State(database): State<DatabaseHandles>,
) -> Response {
    let user_id = session.user_id;
    match database
        .user_manager
        .run(DbPriority::Interactive, move |manager| {
            manager.get_user_playlists(user_id)
        })
        .await
    {
        Ok(playlists) => Json(playlists).into_response(),
        Err(error) => ApiError::user_database(error).into_response(),
    }
}

fn attach_enrichment_status(
    mut value: serde_json::Value,
    entity_type: &str,
    entity_id: &str,
    enrichment_store: &OptionalEnrichmentStore,
) -> serde_json::Value {
    let Some(store) = enrichment_store else {
        return value;
    };
    match store.get_entity_enrichment_status(entity_type, entity_id) {
        Ok(Some(status)) => {
            if let Some(obj) = value.as_object_mut() {
                if let Ok(status_value) = serde_json::to_value(status) {
                    obj.insert("enrichment_status".to_string(), status_value);
                }
            }
        }
        Ok(None) => {}
        Err(err) => debug!(
            "Failed to load enrichment status for {} {}: {}",
            entity_type, entity_id, err
        ),
    }
    value
}

fn attach_artist_enrichment(
    mut value: serde_json::Value,
    artist_id: &str,
    enrichment_store: &OptionalEnrichmentStore,
) -> serde_json::Value {
    let Some(store) = enrichment_store else {
        return value;
    };

    let profile = match store.get_artist_enrichment_v1(artist_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => return value,
        Err(err) => {
            debug!(
                "Failed to load artist enrichment for {}: {}",
                artist_id, err
            );
            return value;
        }
    };

    let tags = match store.list_entity_tags("artist", artist_id) {
        Ok(tags) => tags,
        Err(err) => {
            debug!(
                "Failed to load artist enrichment tags for {}: {}",
                artist_id, err
            );
            Vec::new()
        }
    };
    let contributors = match store.list_entity_contributors("artist", artist_id) {
        Ok(contributors) => contributors,
        Err(err) => {
            debug!(
                "Failed to load artist enrichment contributors for {}: {}",
                artist_id, err
            );
            Vec::new()
        }
    };
    let relations = match store.list_visible_entity_relations("artist", artist_id, 0.8) {
        Ok(relations) => relations,
        Err(err) => {
            debug!(
                "Failed to load artist enrichment relations for {}: {}",
                artist_id, err
            );
            Vec::new()
        }
    };

    if let Some(obj) = value.as_object_mut() {
        let payload = ArtistEnrichmentPayload {
            profile,
            tags,
            contributors,
            relations,
        };
        if let Ok(enrichment_value) = serde_json::to_value(payload) {
            obj.insert("enrichment".to_string(), enrichment_value);
        }
    }

    value
}

fn attach_album_enrichment(
    mut value: serde_json::Value,
    album_id: &str,
    enrichment_store: &OptionalEnrichmentStore,
) -> serde_json::Value {
    let Some(store) = enrichment_store else {
        return value;
    };

    let profile = match store.get_album_enrichment_v1(album_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => return value,
        Err(err) => {
            debug!("Failed to load album enrichment for {}: {}", album_id, err);
            return value;
        }
    };

    let tags = match store.list_entity_tags("album", album_id) {
        Ok(tags) => tags,
        Err(err) => {
            debug!(
                "Failed to load album enrichment tags for {}: {}",
                album_id, err
            );
            Vec::new()
        }
    };
    let contributors = match store.list_entity_contributors("album", album_id) {
        Ok(contributors) => contributors,
        Err(err) => {
            debug!(
                "Failed to load album enrichment contributors for {}: {}",
                album_id, err
            );
            Vec::new()
        }
    };
    let relations = match store.list_visible_entity_relations("album", album_id, 0.8) {
        Ok(relations) => relations,
        Err(err) => {
            debug!(
                "Failed to load album enrichment relations for {}: {}",
                album_id, err
            );
            Vec::new()
        }
    };

    if let Some(obj) = value.as_object_mut() {
        let payload = AlbumEnrichmentPayload {
            profile,
            tags,
            contributors,
            relations,
        };
        if let Ok(enrichment_value) = serde_json::to_value(payload) {
            obj.insert("enrichment".to_string(), enrichment_value);
        }
    }

    value
}

fn attach_track_enrichment(
    mut value: serde_json::Value,
    track_id: &str,
    enrichment_store: &OptionalEnrichmentStore,
) -> serde_json::Value {
    let Some(store) = enrichment_store else {
        return value;
    };

    let profile = match store.get_track_enrichment_v1(track_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => return value,
        Err(err) => {
            debug!("Failed to load track enrichment for {}: {}", track_id, err);
            return value;
        }
    };

    let tags = match store.list_entity_tags("track", track_id) {
        Ok(tags) => tags,
        Err(err) => {
            debug!(
                "Failed to load track enrichment tags for {}: {}",
                track_id, err
            );
            Vec::new()
        }
    };
    let contributors = match store.list_entity_contributors("track", track_id) {
        Ok(contributors) => contributors,
        Err(err) => {
            debug!(
                "Failed to load track enrichment contributors for {}: {}",
                track_id, err
            );
            Vec::new()
        }
    };
    let relations = match store.list_visible_entity_relations("track", track_id, 0.8) {
        Ok(relations) => relations,
        Err(err) => {
            debug!(
                "Failed to load track enrichment relations for {}: {}",
                track_id, err
            );
            Vec::new()
        }
    };

    if let Some(obj) = value.as_object_mut() {
        let payload = TrackEnrichmentPayload {
            profile,
            tags,
            contributors,
            relations,
        };
        if let Ok(enrichment_value) = serde_json::to_value(payload) {
            obj.insert("enrichment".to_string(), enrichment_value);
        }
    }

    value
}

// User listening stats endpoints
