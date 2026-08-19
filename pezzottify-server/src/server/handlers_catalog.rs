async fn home(session: Option<Session>, State(state): State<ServerState>) -> impl IntoResponse {
    let stats = ServerStats {
        uptime: format_uptime(state.start_time.elapsed()),
        hash: state.hash.clone(),
        session_token: session.map(|s| s.token),
    };
    Json(stats)
}

async fn get_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Path(id): Path<String>,
) -> Response {
    debug!("get_artist: id={}", id);

    // Queue artist for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_artist(&id);
    }

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();
    let id_for_status = id.clone();

    match tokio::task::spawn_blocking(move || catalog_store.get_resolved_artist_json(&id)).await {
        Ok(Ok(Some(artist))) => {
            let artist =
                attach_enrichment_status(artist, "artist", &id_for_status, &enrichment_store);
            Json(attach_artist_enrichment(
                artist,
                &id_for_status,
                &enrichment_store,
            ))
            .into_response()
        }
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Artist not found").into_response()
        }
        Ok(Err(err)) => ApiError::internal("Failed to load artist", err).into_response(),
        Err(err) => ApiError::internal("Artist read task failed", err).into_response(),
    }
}

async fn get_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Path(id): Path<String>,
) -> Response {
    // Queue album for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_album(&id);
    }

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();
    let id_for_status = id.clone();

    match tokio::task::spawn_blocking(move || catalog_store.get_album_json(&id)).await {
        Ok(Ok(Some(album))) => Json(attach_album_enrichment(
            attach_enrichment_status(album, "album", &id_for_status, &enrichment_store),
            &id_for_status,
            &enrichment_store,
        ))
        .into_response(),
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Album not found").into_response()
        }
        Ok(Err(err)) => ApiError::internal("Failed to load album", err).into_response(),
        Err(err) => ApiError::internal("Album read task failed", err).into_response(),
    }
}

async fn get_resolved_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Path(id): Path<String>,
) -> Response {
    // Queue album for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_album(&id);
    }

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();
    let id_for_status = id.clone();

    match tokio::task::spawn_blocking(move || catalog_store.get_resolved_album_json(&id)).await {
        Ok(Ok(Some(album))) => Json(attach_album_enrichment(
            attach_enrichment_status(album, "album", &id_for_status, &enrichment_store),
            &id_for_status,
            &enrichment_store,
        ))
        .into_response(),
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Album not found").into_response()
        }
        Ok(Err(err)) => ApiError::internal("Failed to load resolved album", err).into_response(),
        Err(err) => ApiError::internal("Resolved album read task failed", err).into_response(),
    }
}

async fn get_artist_discography(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    Path(id): Path<String>,
    Query(query): Query<DiscographyQuery>,
) -> Response {
    // Queue artist for organic search index expansion (discography includes albums)
    if let Some(indexer) = &organic_indexer {
        indexer.touch_artist(&id);
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);
    let sort = match query.sort.as_deref() {
        Some("release_date") => DiscographySort::ReleaseDate,
        _ => DiscographySort::Popularity, // default
    };
    let appears_on = query.appears_on.unwrap_or(false);

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();

    match tokio::task::spawn_blocking(move || {
        catalog_store.get_discography(&id, limit, offset, sort, appears_on)
    })
    .await
    {
        Ok(Ok(Some(discography))) => Json(discography).into_response(),
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Artist not found").into_response()
        }
        Ok(Err(err)) => {
            ApiError::internal("Failed to load artist discography", err).into_response()
        }
        Err(err) => ApiError::internal("Artist discography task failed", err).into_response(),
    }
}

// =========================================================================
// Genre handlers
// =========================================================================

async fn get_genres(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
) -> Response {
    let catalog_store = Arc::clone(&catalog_store);

    match tokio::task::spawn_blocking(move || catalog_store.get_genres_with_counts()).await {
        Ok(Ok(genres)) => Json(genres).into_response(),
        Ok(Err(err)) => ApiError::internal("Failed to load genres", err).into_response(),
        Err(err) => ApiError::internal("Genre read task failed", err).into_response(),
    }
}

async fn get_genre_tracks(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(genre_name): Path<String>,
    Query(query): Query<GenreTracksQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let catalog_store = Arc::clone(&catalog_store);

    match tokio::task::spawn_blocking(move || {
        catalog_store.get_tracks_by_genre(&genre_name, limit, offset)
    })
    .await
    {
        Ok(Ok(result)) => Json(result).into_response(),
        Ok(Err(err)) => ApiError::internal("Failed to load genre tracks", err).into_response(),
        Err(err) => ApiError::internal("Genre tracks task failed", err).into_response(),
    }
}

async fn get_genre_radio(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(genre_name): Path<String>,
    Query(query): Query<GenreRadioQuery>,
) -> Response {
    let count = query.count.unwrap_or(50).min(200);

    let catalog_store = Arc::clone(&catalog_store);

    match tokio::task::spawn_blocking(move || {
        catalog_store.get_random_tracks_by_genre(&genre_name, count)
    })
    .await
    {
        Ok(Ok(track_ids)) => Json(track_ids).into_response(),
        Ok(Err(err)) => ApiError::internal("Failed to build genre radio", err).into_response(),
        Err(err) => ApiError::internal("Genre radio task failed", err).into_response(),
    }
}

pub async fn get_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Path(id): Path<String>,
) -> Response {
    // Queue track for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_track(&id);
    }

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();
    let id_for_status = id.clone();

    match tokio::task::spawn_blocking(move || catalog_store.get_track_json(&id)).await {
        Ok(Ok(Some(track))) => Json(attach_track_enrichment(
            attach_enrichment_status(track, "track", &id_for_status, &enrichment_store),
            &id_for_status,
            &enrichment_store,
        ))
        .into_response(),
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Track not found").into_response()
        }
        Ok(Err(err)) => ApiError::internal("Failed to load track", err).into_response(),
        Err(err) => ApiError::internal("Track read task failed", err).into_response(),
    }
}

pub async fn get_resolved_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Path(id): Path<String>,
) -> Response {
    // Queue track for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_track(&id);
    }

    let catalog_store = Arc::clone(&catalog_store);
    let id = id.clone();
    let id_for_status = id.clone();

    match tokio::task::spawn_blocking(move || catalog_store.get_resolved_track_json(&id)).await {
        Ok(Ok(Some(track))) => Json(attach_track_enrichment(
            attach_enrichment_status(track, "track", &id_for_status, &enrichment_store),
            &id_for_status,
            &enrichment_store,
        ))
        .into_response(),
        Ok(Ok(None)) => {
            ApiError::not_found("catalog_item_not_found", "Track not found").into_response()
        }
        Ok(Err(err)) => ApiError::internal("Failed to load resolved track", err).into_response(),
        Err(err) => ApiError::internal("Resolved track read task failed", err).into_response(),
    }
}

/// Batch fetch multiple artists, albums, and tracks in a single request.
/// Returns per-item results with `{"ok": ...}` or `{"error": "..."}` wrapper.
async fn post_batch_content(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<super::state::OptionalOrganicIndexer>,
    State(enrichment_store): State<OptionalEnrichmentStore>,
    Json(request): Json<BatchContentRequest>,
) -> Response {
    let total_items = request.artists.len() + request.albums.len() + request.tracks.len();
    debug!(
        "post_batch_content: {} artists, {} albums, {} tracks ({} total)",
        request.artists.len(),
        request.albums.len(),
        request.tracks.len(),
        total_items
    );

    if total_items > BATCH_MAX_ITEMS {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "Batch request exceeds maximum of {} items (requested {})",
                BATCH_MAX_ITEMS, total_items
            ),
        )
            .into_response();
    }

    let mut response = BatchContentResponse {
        artists: std::collections::HashMap::with_capacity(request.artists.len()),
        albums: std::collections::HashMap::with_capacity(request.albums.len()),
        tracks: std::collections::HashMap::with_capacity(request.tracks.len()),
    };

    // Fetch artists
    for item in request.artists {
        if let Some(indexer) = &organic_indexer {
            indexer.touch_artist(&item.id);
        }

        let result = if item.resolved {
            catalog_store.get_resolved_artist_json(&item.id)
        } else {
            catalog_store.get_artist_json(&item.id)
        };

        let batch_result = match result {
            Ok(Some(data)) => {
                let data = attach_artist_enrichment(
                    attach_enrichment_status(data, "artist", &item.id, &enrichment_store),
                    &item.id,
                    &enrichment_store,
                );
                BatchItemResult::Ok { ok: data }
            }
            Ok(None) => BatchItemResult::Error {
                error: "not_found".to_string(),
            },
            Err(e) => {
                error!("Batch fetch artist {}: {}", item.id, e);
                BatchItemResult::Error {
                    error: "internal_error".to_string(),
                }
            }
        };
        response.artists.insert(item.id, batch_result);
    }

    // Fetch albums
    for item in request.albums {
        if let Some(indexer) = &organic_indexer {
            indexer.touch_album(&item.id);
        }

        let result = if item.resolved {
            catalog_store.get_resolved_album_json(&item.id)
        } else {
            catalog_store.get_album_json(&item.id)
        };

        let batch_result = match result {
            Ok(Some(data)) => {
                let data = attach_album_enrichment(
                    attach_enrichment_status(data, "album", &item.id, &enrichment_store),
                    &item.id,
                    &enrichment_store,
                );
                BatchItemResult::Ok { ok: data }
            }
            Ok(None) => BatchItemResult::Error {
                error: "not_found".to_string(),
            },
            Err(e) => {
                error!("Batch fetch album {}: {}", item.id, e);
                BatchItemResult::Error {
                    error: "internal_error".to_string(),
                }
            }
        };
        response.albums.insert(item.id, batch_result);
    }

    // Fetch tracks
    for item in request.tracks {
        if let Some(indexer) = &organic_indexer {
            indexer.touch_track(&item.id);
        }

        let result = if item.resolved {
            catalog_store.get_resolved_track_json(&item.id)
        } else {
            catalog_store.get_track_json(&item.id)
        };

        let batch_result = match result {
            Ok(Some(data)) => {
                let data = attach_track_enrichment(
                    attach_enrichment_status(data, "track", &item.id, &enrichment_store),
                    &item.id,
                    &enrichment_store,
                );
                BatchItemResult::Ok { ok: data }
            }
            Ok(None) => BatchItemResult::Error {
                error: "not_found".to_string(),
            },
            Err(e) => {
                error!("Batch fetch track {}: {}", item.id, e);
                BatchItemResult::Error {
                    error: "internal_error".to_string(),
                }
            }
        };
        response.tracks.insert(item.id, batch_result);
    }

    Json(response).into_response()
}

async fn get_image(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(http_client): State<HttpClient>,
    Path(id): Path<String>,
) -> Response {
    let file_path = catalog_store.get_image_path(&id);

    // First, check if we have the image cached locally
    if file_path.exists() {
        return serve_image_file(&file_path).await;
    }

    // Image not cached locally - try to fetch from external URL
    let image_url = match catalog_store.get_item_image_url(&id) {
        Ok(Some(url)) => url,
        Ok(None) => {
            debug!("No image URL found for item: {}", id);
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(e) => {
            error!("Failed to query image URL for {}: {}", id, e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Download the image from the external URL
    let response = match http_client.get(&image_url.url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to download image from {}: {}", image_url.url, e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if !response.status().is_success() {
        error!(
            "Failed to download image from {}: status {}",
            image_url.url,
            response.status()
        );
        return StatusCode::BAD_GATEWAY.into_response();
    }

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to read image bytes from {}: {}", image_url.url, e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // Verify it's actually an image
    let mime_type = match infer::get(&bytes) {
        Some(kind) if kind.mime_type().starts_with("image/") => kind.mime_type().to_string(),
        _ => {
            error!("Downloaded content is not an image: {}", image_url.url);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // Save the image to disk for future requests
    if let Some(parent) = file_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create images directory: {}", e);
        }
    }
    if let Err(e) = std::fs::write(&file_path, &bytes) {
        warn!("Failed to cache image to {}: {}", file_path.display(), e);
        // Continue anyway - we can still serve the image
    } else {
        debug!("Cached image for {} to {}", id, file_path.display());
    }

    // Return the image
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .body(bytes.to_vec().into())
        .unwrap()
}

/// Helper function to serve an image file from disk.
async fn serve_image_file(file_path: &std::path::Path) -> Response {
    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Some(kind) = infer::get(&buffer) {
        if kind.mime_type().starts_with("image/") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, kind.mime_type().to_string())
                .body(buffer.into())
                .unwrap();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

// =============================================================================
// Catalog Editing Handlers
// =============================================================================

#[derive(Debug, Deserialize)]
struct CreateArtistRequest {
    id: String,
    name: String,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    followers_total: i64,
    #[serde(default = "default_popularity")]
    popularity: i32,
}

#[derive(Debug, Deserialize)]
struct UpdateArtistMetadataRequest {
    name: String,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    followers_total: i64,
    #[serde(default = "default_popularity")]
    popularity: i32,
}

fn default_popularity() -> i32 {
    50
}

#[derive(Debug, Deserialize)]
struct CreateAlbumRequest {
    id: String,
    name: String,
    #[serde(default)]
    album_type: String,
    #[serde(default)]
    artist_ids: Vec<String>,
    label: Option<String>,
    release_date: Option<String>,
    release_date_precision: Option<String>,
    external_id_upc: Option<String>,
    #[serde(default = "default_popularity")]
    popularity: i32,
}

#[derive(Debug, Deserialize)]
struct UpdateAlbumMetadataRequest {
    name: String,
    #[serde(default)]
    album_type: String,
    artist_ids: Option<Vec<String>>,
    label: Option<String>,
    release_date: Option<String>,
    release_date_precision: Option<String>,
    external_id_upc: Option<String>,
    #[serde(default = "default_popularity")]
    popularity: i32,
}

#[derive(Debug, Deserialize)]
struct CreateTrackRequest {
    id: String,
    name: String,
    album_id: String,
    #[serde(default)]
    artist_ids: Vec<String>,
    #[serde(default = "default_disc")]
    disc_number: i32,
    #[serde(default = "default_track")]
    track_number: i32,
    #[serde(default)]
    duration_ms: i64,
    #[serde(default)]
    explicit: bool,
    #[serde(default = "default_popularity")]
    popularity: i32,
    language: Option<String>,
    external_id_isrc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTrackMetadataRequest {
    name: String,
    album_id: String,
    artist_ids: Option<Vec<String>>,
    #[serde(default = "default_disc")]
    disc_number: i32,
    #[serde(default = "default_track")]
    track_number: i32,
    #[serde(default)]
    duration_ms: i64,
    #[serde(default)]
    explicit: bool,
    #[serde(default = "default_popularity")]
    popularity: i32,
    language: Option<String>,
    external_id_isrc: Option<String>,
}

fn default_disc() -> i32 {
    1
}

fn default_track() -> i32 {
    1
}

async fn create_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<CreateArtistRequest>,
) -> Response {
    use crate::catalog_store::{validate_artist, Artist};

    let artist = Artist {
        id: data.id,
        name: data.name,
        genres: data.genres,
        followers_total: data.followers_total,
        popularity: data.popularity,
        available: false, // Will be updated when tracks are added
    };

    if let Err(e) = validate_artist(&artist) {
        return ApiError::bad_request("invalid_artist", e.to_string()).into_response();
    }

    match catalog_store.create_artist(&artist) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn update_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<UpdateArtistMetadataRequest>,
) -> Response {
    use crate::catalog_store::{validate_artist, Artist};

    let artist = Artist {
        id,
        name: data.name,
        genres: data.genres,
        followers_total: data.followers_total,
        popularity: data.popularity,
        available: false, // Preserved by update logic
    };

    if let Err(e) = validate_artist(&artist) {
        return ApiError::bad_request("invalid_artist", e.to_string()).into_response();
    }

    match catalog_store.update_artist(&artist) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn delete_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_artist(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => {
            ApiError::not_found("catalog_item_not_found", "Artist not found").into_response()
        }
        Err(e) => ApiError::internal("Failed to delete artist", e).into_response(),
    }
}

async fn create_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<CreateAlbumRequest>,
) -> Response {
    use crate::catalog_store::{validate_album, Album, AlbumAvailability, AlbumType};

    let album = Album {
        id: data.id,
        name: data.name,
        album_type: AlbumType::from_db_str(&data.album_type),
        label: data.label,
        release_date: data.release_date,
        release_date_precision: data.release_date_precision,
        external_id_upc: data.external_id_upc,
        popularity: data.popularity,
        album_availability: AlbumAvailability::Missing,
    };

    if let Err(e) = validate_album(&album) {
        return ApiError::bad_request("invalid_album", e.to_string()).into_response();
    }

    match catalog_store.create_album(&album, &data.artist_ids) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn update_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<UpdateAlbumMetadataRequest>,
) -> Response {
    use crate::catalog_store::{validate_album_metadata, AlbumMetadataUpdate, AlbumType};

    let metadata = AlbumMetadataUpdate {
        name: data.name,
        album_type: AlbumType::from_db_str(&data.album_type),
        label: data.label,
        release_date: data.release_date,
        release_date_precision: data.release_date_precision,
        external_id_upc: data.external_id_upc,
        popularity: data.popularity,
    };

    if let Err(e) = validate_album_metadata(&id, &metadata) {
        return ApiError::bad_request("invalid_album", e.to_string()).into_response();
    }

    match catalog_store.update_album_metadata(&id, &metadata, data.artist_ids.as_deref()) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn delete_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_album(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => {
            ApiError::not_found("catalog_item_not_found", "Album not found").into_response()
        }
        Err(e) => ApiError::internal("Failed to delete album", e).into_response(),
    }
}

async fn create_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<CreateTrackRequest>,
) -> Response {
    use crate::catalog_store::{validate_track, Track, TrackAvailability};

    let track = Track {
        id: data.id,
        name: data.name,
        album_id: data.album_id,
        disc_number: data.disc_number,
        track_number: data.track_number,
        duration_ms: data.duration_ms,
        explicit: data.explicit,
        popularity: data.popularity,
        language: data.language,
        external_id_isrc: data.external_id_isrc,
        audio_uri: None,
        availability: TrackAvailability::default(),
    };

    if let Err(e) = validate_track(&track) {
        return ApiError::bad_request("invalid_track", e.to_string()).into_response();
    }

    match catalog_store.create_track(&track, &data.artist_ids) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn update_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<UpdateTrackMetadataRequest>,
) -> Response {
    use crate::catalog_store::{validate_track_metadata, TrackMetadataUpdate};

    let metadata = TrackMetadataUpdate {
        name: data.name,
        album_id: data.album_id,
        disc_number: data.disc_number,
        track_number: data.track_number,
        duration_ms: data.duration_ms,
        explicit: data.explicit,
        popularity: data.popularity,
        language: data.language,
        external_id_isrc: data.external_id_isrc,
    };

    if let Err(e) = validate_track_metadata(&id, &metadata) {
        return ApiError::bad_request("invalid_track", e.to_string()).into_response();
    }

    match catalog_store.update_track_metadata(&id, &metadata, data.artist_ids.as_deref()) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => ApiError::catalog_mutation(e).into_response(),
    }
}

async fn delete_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_track(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => {
            ApiError::not_found("catalog_item_not_found", "Track not found").into_response()
        }
        Err(e) => ApiError::internal("Failed to delete track", e).into_response(),
    }
}

// Image CRUD not yet implemented
async fn create_image(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Json(_data): Json<serde_json::Value>,
) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Image CRUD not yet implemented",
    )
        .into_response()
}

async fn update_image(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_id): Path<String>,
    Json(_data): Json<serde_json::Value>,
) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Image CRUD not yet implemented",
    )
        .into_response()
}

async fn delete_image(
    _session: Session,
    State(_catalog_store): State<GuardedCatalogStore>,
    Path(_id): Path<String>,
) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Image CRUD not yet implemented",
    )
        .into_response()
}

// ============================================================================
// What's New Endpoint (user-facing changelog)
// ============================================================================

#[derive(Deserialize)]
struct WhatsNewQuery {
    #[serde(default = "default_whats_new_limit")]
    limit: usize,
}

fn default_whats_new_limit() -> usize {
    10
}

#[derive(Serialize)]
struct WhatsNewResponse {
    batches: Vec<WhatsNewBatchResponse>,
}

#[derive(Serialize)]
struct WhatsNewBatchResponse {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    closed_at: i64,
    summary: BatchSummary,
}

#[derive(Serialize, Default)]
struct BatchSummary {
    artists: EntitySummary,
    albums: EntitySummary,
    tracks: TrackSummary,
    images: EntitySummary,
}

#[derive(Serialize)]
struct EntityRef {
    id: String,
    name: String,
}

#[derive(Serialize, Default)]
struct EntitySummary {
    added: Vec<EntityRef>,
    updated_count: i32,
    deleted: Vec<EntityRef>,
}

#[derive(Serialize, Default)]
struct TrackSummary {
    added_count: i32,
    updated_count: i32,
    deleted_count: i32,
}

/// GET /v1/content/whatsnew - List recent catalog updates
async fn get_whats_new(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(server_store): State<GuardedServerStore>,
    Query(query): Query<WhatsNewQuery>,
) -> Response {
    let limit = query.limit.min(50);

    let batches = match server_store.list_whatsnew_batches(limit) {
        Ok(b) => b,
        Err(e) => {
            return ApiError::internal("Failed to load What's New batches", e).into_response();
        }
    };

    let mut response_batches = Vec::new();
    for batch in batches {
        let album_ids = match server_store.get_whatsnew_batch_album_ids(&batch.id) {
            Ok(ids) => ids,
            Err(e) => {
                warn!(
                    "Failed to get album IDs for What's New batch {}: {}",
                    batch.id, e
                );
                continue;
            }
        };

        // Enrich with album names from catalog
        let albums: Vec<EntityRef> = album_ids
            .iter()
            .filter_map(|id| {
                catalog_store.get_album_json(id).ok().flatten().map(|json| {
                    let name = json
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("Unknown Album");
                    EntityRef {
                        id: id.clone(),
                        name: name.to_string(),
                    }
                })
            })
            .collect();

        response_batches.push(WhatsNewBatchResponse {
            id: batch.id,
            name: None, // Clients derive from closed_at
            description: None,
            closed_at: batch.closed_at,
            summary: BatchSummary {
                albums: EntitySummary {
                    added: albums,
                    ..Default::default()
                },
                ..Default::default()
            },
        });
    }

    Json(WhatsNewResponse {
        batches: response_batches,
    })
    .into_response()
}

/// Get popular albums and artists based on listening data from the last 365 days.
/// Uses a large window so that low-traffic instances still return meaningful results.
/// Results are cached for 24 hours in UserManager.
async fn get_popular_content(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<PopularContentQuery>,
) -> Response {
    use std::time::SystemTime;

    // Use 365-day window - on busy instances the query limit caps results,
    // on quiet instances we get meaningful data from further back
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let end_date = {
        let datetime =
            chrono::DateTime::from_timestamp(now_secs as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    let start_date = {
        let one_year_ago = now_secs - (365 * 24 * 60 * 60);
        let datetime = chrono::DateTime::from_timestamp(one_year_ago as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    // Default and cap limits
    let albums_limit = query.albums_limit.unwrap_or(10).min(20);
    let artists_limit = query.artists_limit.unwrap_or(10).min(20);

    info!(
        "get_popular_content: date range {} - {}, limits albums={} artists={}",
        start_date, end_date, albums_limit, artists_limit
    );

    match user_manager.lock().unwrap().get_popular_content(
        start_date,
        end_date,
        albums_limit,
        artists_limit,
    ) {
        Ok(content) => {
            info!(
                "get_popular_content: returning {} albums, {} artists",
                content.albums.len(),
                content.artists.len()
            );
            Json(content).into_response()
        }
        Err(err) => {
            error!("Error getting popular content: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get the current weekly featured album discovery snapshot.
async fn get_featured_albums(
    _session: Session,
    State(server_store): State<GuardedServerStore>,
    Query(query): Query<FeaturedAlbumsQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let snapshot = match server_store.get_state(FeaturedAlbumsJob::state_key()) {
        Ok(Some(raw)) => match serde_json::from_str::<FeaturedAlbumsSnapshot>(&raw) {
            Ok(mut snapshot) => {
                snapshot.albums.truncate(limit);
                if !snapshot.albums.is_empty() {
                    snapshot.hero_index %= snapshot.albums.len();
                }
                snapshot
            }
            Err(err) => {
                warn!("Invalid featured albums snapshot: {}", err);
                FeaturedAlbumsSnapshot {
                    week_key: FeaturedAlbumsJob::current_week_key(),
                    generated_at: 0,
                    hero_index: 0,
                    albums: Vec::new(),
                }
            }
        },
        Ok(None) => FeaturedAlbumsSnapshot {
            week_key: FeaturedAlbumsJob::current_week_key(),
            generated_at: 0,
            hero_index: 0,
            albums: Vec::new(),
        },
        Err(err) => {
            error!("Failed to read featured albums snapshot: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut response = Json(snapshot).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}

/// Get latest persisted catalog availability statistics snapshot.
async fn get_catalog_stats_snapshot(
    _session: Session,
    State(server_store): State<GuardedServerStore>,
) -> Response {
    let key = CatalogAvailabilityStatsJob::snapshot_state_key();
    let Some(raw) = (match server_store.get_state(key) {
        Ok(v) => v,
        Err(err) => {
            error!(
                "Failed to read catalog stats snapshot from server store: {}",
                err
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "catalog availability stats not available yet"
            })),
        )
            .into_response();
    };

    match serde_json::from_str::<CatalogAvailabilityStatsSnapshot>(&raw) {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => {
            error!("Failed to deserialize catalog stats snapshot: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
