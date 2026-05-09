//! Generic embedding API routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalog_store::{EntityEmbedding, EntityEmbeddingSearchResult, EntityEmbeddingUpsert};

use super::session::Session;
use super::state::ServerState;

#[derive(Deserialize)]
struct EmbeddingQuery {
    #[serde(default)]
    include_vector: bool,
}

#[derive(Deserialize)]
struct UpsertEmbeddingBody {
    vector: Vec<f32>,
    #[serde(default = "default_dtype")]
    dtype: String,
    #[serde(default)]
    metadata: Value,
    #[serde(default)]
    model: Value,
}

#[derive(Deserialize)]
struct SearchEmbeddingsBody {
    namespace: String,
    vector: Vec<f32>,
    entity_type: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchEmbeddingsResponse {
    namespace: String,
    results: Vec<EntityEmbeddingSearchResult>,
}

fn default_dtype() -> String {
    "float32".to_string()
}

fn validate_entity_type(entity_type: &str) -> Result<(), (StatusCode, String)> {
    match entity_type {
        "track" | "album" | "artist" | "playlist" | "user" => Ok(()),
        other => Err((
            StatusCode::BAD_REQUEST,
            format!("unsupported entity_type '{other}'"),
        )),
    }
}

fn validate_namespace(namespace: &str) -> Result<(), (StatusCode, String)> {
    if namespace.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "namespace is required".to_string()));
    }
    if namespace.len() > 160 {
        return Err((
            StatusCode::BAD_REQUEST,
            "namespace is too long; max 160 bytes".to_string(),
        ));
    }
    Ok(())
}

async fn list_embeddings(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(query): Query<EmbeddingQuery>,
) -> Result<Json<Vec<EntityEmbedding>>, (StatusCode, String)> {
    validate_entity_type(&entity_type)?;
    state
        .catalog_store
        .list_entity_embeddings(&entity_type, &entity_id, query.include_vector)
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn get_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
    Query(query): Query<EmbeddingQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    match state
        .catalog_store
        .get_entity_embedding(&entity_type, &entity_id, &namespace, query.include_vector)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    {
        Some(embedding) => Ok(Json(embedding).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

async fn put_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
    Json(body): Json<UpsertEmbeddingBody>,
) -> Result<Json<EntityEmbedding>, (StatusCode, String)> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    if body.vector.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "embedding vector cannot be empty".to_string(),
        ));
    }
    if body.dtype != "float32" {
        return Err((
            StatusCode::BAD_REQUEST,
            "only dtype=float32 is currently supported".to_string(),
        ));
    }

    let embedding = EntityEmbeddingUpsert {
        entity_type,
        entity_id,
        namespace,
        vector: body.vector,
        dtype: body.dtype,
        metadata: body.metadata,
        model: body.model,
    };
    state
        .catalog_store
        .upsert_entity_embedding(&embedding)
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn delete_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    let deleted = state
        .catalog_store
        .delete_entity_embedding(&entity_type, &entity_id, &namespace)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(if deleted {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    })
}

async fn search_embeddings(
    _session: Session,
    State(state): State<ServerState>,
    Json(body): Json<SearchEmbeddingsBody>,
) -> Result<Json<SearchEmbeddingsResponse>, (StatusCode, String)> {
    validate_namespace(&body.namespace)?;
    if let Some(entity_type) = body.entity_type.as_deref() {
        validate_entity_type(entity_type)?;
    }
    if body.vector.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "query vector cannot be empty".to_string(),
        ));
    }
    let limit = body.limit.unwrap_or(30).clamp(1, 200);
    let results = state
        .catalog_store
        .search_entity_embeddings(
            &body.namespace,
            &body.vector,
            body.entity_type.as_deref(),
            limit,
        )
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(SearchEmbeddingsResponse {
        namespace: body.namespace,
        results,
    }))
}

pub fn read_routes() -> Router<ServerState> {
    Router::new()
        .route("/embedding/{entity_type}/{entity_id}", get(list_embeddings))
        .route(
            "/embedding/{entity_type}/{entity_id}/{namespace}",
            get(get_embedding),
        )
        .route("/embedding/search", post(search_embeddings))
}

pub fn write_routes() -> Router<ServerState> {
    Router::new().route(
        "/embedding/{entity_type}/{entity_id}/{namespace}",
        put(put_embedding).delete(delete_embedding),
    )
}
