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

use super::state::ServerState;
use super::{api_error::ApiError, session::Session};

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

fn validate_entity_type(entity_type: &str) -> Result<(), ApiError> {
    match entity_type {
        "track" | "album" | "artist" | "playlist" | "user" => Ok(()),
        other => Err(ApiError::bad_request(
            "unsupported_entity_type",
            format!("Unsupported entity type '{other}'"),
        )),
    }
}

fn validate_namespace(namespace: &str) -> Result<(), ApiError> {
    if namespace.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_namespace",
            "Namespace is required",
        ));
    }
    if namespace.len() > 160 {
        return Err(ApiError::bad_request(
            "invalid_namespace",
            "Namespace is too long; maximum is 160 bytes",
        ));
    }
    Ok(())
}

async fn list_embeddings(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(query): Query<EmbeddingQuery>,
) -> Result<Json<Vec<EntityEmbedding>>, ApiError> {
    validate_entity_type(&entity_type)?;
    state
        .catalog_store
        .list_entity_embeddings(&entity_type, &entity_id, query.include_vector)
        .map(Json)
        .map_err(|err| ApiError::internal("Failed to list embeddings", err))
}

async fn get_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
    Query(query): Query<EmbeddingQuery>,
) -> Result<impl IntoResponse, ApiError> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    match state
        .catalog_store
        .get_entity_embedding(&entity_type, &entity_id, &namespace, query.include_vector)
        .map_err(|err| ApiError::internal("Failed to load embedding", err))?
    {
        Some(embedding) => Ok(Json(embedding).into_response()),
        None => Err(ApiError::not_found(
            "embedding_not_found",
            "Embedding not found",
        )),
    }
}

async fn put_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
    Json(body): Json<UpsertEmbeddingBody>,
) -> Result<Json<EntityEmbedding>, ApiError> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    if body.vector.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_embedding",
            "Embedding vector cannot be empty",
        ));
    }
    if body.dtype != "float32" {
        return Err(ApiError::bad_request(
            "invalid_embedding_dtype",
            "Only dtype=float32 is currently supported",
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
        .map_err(|err| ApiError::internal("Failed to store embedding", err))
}

async fn delete_embedding(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id, namespace)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    validate_entity_type(&entity_type)?;
    validate_namespace(&namespace)?;
    let deleted = state
        .catalog_store
        .delete_entity_embedding(&entity_type, &entity_id, &namespace)
        .map_err(|err| ApiError::internal("Failed to delete embedding", err))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(
            "embedding_not_found",
            "Embedding not found",
        ))
    }
}

async fn search_embeddings(
    _session: Session,
    State(state): State<ServerState>,
    Json(body): Json<SearchEmbeddingsBody>,
) -> Result<Json<SearchEmbeddingsResponse>, ApiError> {
    validate_namespace(&body.namespace)?;
    if let Some(entity_type) = body.entity_type.as_deref() {
        validate_entity_type(entity_type)?;
    }
    if body.vector.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_embedding",
            "Query vector cannot be empty",
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
        .map_err(|err| ApiError::internal("Embedding search failed", err))?;
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
