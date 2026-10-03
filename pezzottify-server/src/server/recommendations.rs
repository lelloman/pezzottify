//! Recommendation routes for smart continuation and radio playback.

use simple_server::extract::Extract;
use std::collections::{HashMap, HashSet};

use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_server::web::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tracing::error;

use crate::catalog_store::{CatalogStore, ResolvedTrack, TrackAvailability};
use crate::config::AudioEmbeddingsSettings;
use crate::db_executor::{DbPriority, DbRunError};

use super::api_error::ApiError;
use super::session::Session;
use super::state::ServerState;

/// A recommendation request that is malformed rather than a store failure.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct InvalidRecommendationRequest(String);

macro_rules! invalid_request {
    ($($arg:tt)*) => {
        anyhow::Error::new(InvalidRecommendationRequest(format!($($arg)*)))
    };
}

fn is_invalid_request(err: &anyhow::Error) -> bool {
    err.downcast_ref::<InvalidRecommendationRequest>().is_some()
}

const DEFAULT_TRACK_NAMESPACE: &str = "musicfm.mean.v1";
const DEFAULT_ALBUM_NAMESPACE: &str = "album.musicfm.median.v1";
const CONTINUATION_CONTEXT_LIMIT: usize = 10;
const ARTIST_SEED_TRACK_LIMIT: usize = 50;
const DEFAULT_RADIO_RANDOMNESS: f32 = 0.3;
const DEFAULT_RADIO_DIVERSITY: f32 = 0.3;
const RADIO_COOLDOWN_DECAY: f32 = 0.65;
const RADIO_ALBUM_PENALTY_WEIGHT: f32 = 0.12;
const RADIO_ARTIST_PENALTY_WEIGHT: f32 = 0.10;
const RADIO_COOLDOWN_MIN: f32 = 0.01;
const RADIO_ARTIST_REPEAT_MIN_DISTANCE: usize = 4;
const RADIO_MAX_REFERENCES: usize = 8;
const CONTINUATION_SOURCE_TRACKS_MAX: usize = 200;
const CONTINUATION_SOURCE_SAMPLE: usize = 64;
const CONTINUATION_DEFAULT_RECENCY_WEIGHT: f32 = 0.2;
const CONTINUATION_OVERSAMPLE_MIN: usize = 60;
const CONTINUATION_OVERSAMPLE_MAX: usize = 400;

/// Smart continuation request.
///
/// Legacy clients send only `context_track_ids`. Current clients send the user-chosen
/// `source_track_ids` (or `source_references`) plus `recent_track_ids`, and optionally a
/// `destination` with a `progress` to steer the queue.
#[derive(Debug, Deserialize)]
struct ContinuationRequest {
    #[serde(default)]
    context_track_ids: Vec<String>,
    #[serde(default)]
    exclude_track_ids: Vec<String>,
    count: Option<usize>,
    #[serde(default)]
    source_track_ids: Vec<String>,
    #[serde(default)]
    source_references: Vec<RadioReference>,
    #[serde(default)]
    recent_track_ids: Vec<String>,
    recency_weight: Option<f32>,
    /// One reference object or an array of up to 8: a weighted mix.
    #[serde(default, deserialize_with = "one_or_many_references")]
    destination: Vec<RadioReference>,
    progress: Option<f32>,
    #[serde(default)]
    criteria: Vec<RadioCriterionRequest>,
    diversity: Option<f32>,
    randomness: Option<f32>,
    mode: Option<RadioMode>,
    #[serde(default)]
    away: Vec<RadioReference>,
}

#[derive(Debug, Serialize)]
struct ContinuationResponse {
    track_ids: Vec<String>,
    recency_weight: Option<f32>,
    progress: Option<f32>,
    namespaces: Vec<ContinuationNamespaceDiagnostics>,
}

#[derive(Debug, Serialize)]
struct ContinuationNamespaceDiagnostics {
    namespace: String,
    weight: f32,
    source_to_destination: Option<f32>,
    query_to_source: Option<f32>,
    query_to_destination: Option<f32>,
    destination_components: Vec<DestinationComponentDiagnostics>,
}

/// Similarity between the current query and one destination component.
#[derive(Debug, Serialize)]
struct DestinationComponentDiagnostics {
    entity_type: String,
    entity_id: String,
    similarity: Option<f32>,
}

fn one_or_many_references<'de, D>(deserializer: D) -> Result<Vec<RadioReference>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(RadioReference),
        Many(Vec<RadioReference>),
        Null(()),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(reference) => vec![reference],
        OneOrMany::Many(references) => references,
        OneOrMany::Null(()) => Vec::new(),
    })
}

/// A query vector for one embedding namespace, with its normalised criterion weight.
struct NamespaceQuery {
    namespace: String,
    weight: f32,
    vector: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct RadioContinuationRequest {
    source: String,
    seed: RadioSeed,
    settings: Option<serde_json::Value>,
    #[serde(default)]
    context_track_ids: Vec<String>,
    #[serde(default)]
    exclude_track_ids: Vec<String>,
    count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RadioQuery {
    count: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RadioBuildRequest {
    seed: RadioSeed,
    count: Option<usize>,
    recipe_id: Option<String>,
    #[serde(default)]
    criteria: Vec<RadioCriterionRequest>,
    mode: Option<RadioMode>,
    #[serde(default)]
    toward: Vec<RadioReference>,
    #[serde(default)]
    away: Vec<RadioReference>,
    diversity: Option<f32>,
    randomness: Option<f32>,
    include_seed_tracks: Option<bool>,
    filters: Option<RadioFilters>,
}

#[derive(Debug, Clone, Deserialize)]
struct RadioSeed {
    entity_type: String,
    entity_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RadioCriterionRequest {
    namespace: String,
    weight: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct RadioReference {
    entity_type: String,
    entity_id: String,
    weight: Option<f32>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RadioMode {
    Similar,
    Explore,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RadioFilters {
    #[serde(default)]
    genres: Vec<String>,
    release_year_min: Option<i32>,
    release_year_max: Option<i32>,
    popularity_min: Option<i32>,
    popularity_max: Option<i32>,
    explicit: Option<ExplicitFilter>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExplicitFilter {
    Include,
    Exclude,
    Only,
}

#[derive(Debug, Serialize)]
struct TrackIdsResponse {
    track_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RadioOptionsResponse {
    recipes: Vec<RadioRecipe>,
    criteria: Vec<RadioCriterionOption>,
    default_recipe_id: String,
    modes: Vec<&'static str>,
    explicit_filters: Vec<&'static str>,
    count: RadioRangeUsize,
    diversity: RadioRangeF32,
    randomness: RadioRangeF32,
}

#[derive(Debug, Clone, Serialize)]
struct RadioRecipe {
    id: String,
    name: String,
    criteria: Vec<RadioCriterionRequestForResponse>,
    mode: &'static str,
    diversity: f32,
    randomness: f32,
}

#[derive(Debug, Clone, Serialize)]
struct RadioCriterionRequestForResponse {
    namespace: String,
    weight: f32,
}

#[derive(Debug, Clone, Serialize)]
struct RadioCriterionOption {
    namespace: String,
    label: String,
}

#[derive(Debug, Clone, Serialize)]
struct RadioRangeUsize {
    min: usize,
    max: usize,
    default: usize,
}

#[derive(Debug, Clone, Serialize)]
struct RadioRangeF32 {
    min: f32,
    max: f32,
    default: f32,
}

#[derive(Debug, Clone)]
struct NormalizedRadioCriterion {
    namespace: String,
    weight: f32,
}

#[derive(Debug)]
struct CandidateScore {
    score: f32,
    best_similarity: f32,
}

#[derive(Debug, Clone)]
struct RankedRadioCandidate {
    track_id: String,
    resolved: ResolvedTrack,
    score: f32,
}

struct RadioSelectionState<'a> {
    result: &'a mut Vec<String>,
    exclude: &'a mut HashSet<String>,
    album_cooldowns: &'a mut HashMap<String, f32>,
    artist_cooldowns: &'a mut HashMap<String, f32>,
    artist_last_positions: &'a mut HashMap<String, usize>,
}

pub fn recommendation_routes() -> Router<ServerState> {
    Router::new()
        .route(
            "/recommendations/continuation",
            post(post_continuation_recommendations),
        )
        .route("/artist/{id}/greatest-hits", get(get_artist_greatest_hits))
        .route("/radio/continue", post(post_radio_continuation))
        .route("/radio/options", get(get_radio_options))
        .route("/concepts", get(get_concepts))
        .route("/radio/build", post(post_radio_build))
        .route("/radio/{entity_type}/{entity_id}", get(get_radio))
}

async fn get_artist_greatest_hits(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Response {
    match state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |store| {
            if store.get_artist_json(&id)?.is_none() {
                return Ok(None);
            }
            store.get_artist_greatest_hits_track_ids(&id).map(Some)
        })
        .await
    {
        Ok(Some(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(None) => {
            ApiError::not_found("catalog_item_not_found", "Artist not found").into_response()
        }
        Err(err) => ApiError::from(err).into_response(),
    }
}

async fn post_radio_continuation(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Json(body): Json<RadioContinuationRequest>,
) -> Response {
    if !matches!(body.source.as_str(), "basic" | "custom" | "genre")
        || (body.source == "genre" && body.seed.entity_type != "genre")
        || (body.source != "genre"
            && !matches!(body.seed.entity_type.as_str(), "track" | "album" | "artist"))
    {
        return ApiError::bad_request("invalid_radio_source", "Unsupported radio source or seed")
            .into_response();
    }
    let settings = state.config.audio_embeddings.clone();
    match state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |store| {
            continue_radio(store, settings.as_ref(), body)
        })
        .await
    {
        Ok(track_ids) => no_store_json(TrackIdsResponse { track_ids }),
        Err(DbRunError::Store(err)) if is_invalid_request(&err) => {
            ApiError::bad_request("invalid_radio_request", err.to_string()).into_response()
        }
        Err(err) => ApiError::from(err).into_response(),
    }
}

fn continue_radio(
    store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    request: RadioContinuationRequest,
) -> anyhow::Result<Vec<String>> {
    let count = request.count.unwrap_or(10).clamp(1, 10);
    let mut exclude: HashSet<String> = request.exclude_track_ids.into_iter().collect();
    let recent = request
        .context_track_ids
        .into_iter()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    exclude.extend(recent.iter().cloned());
    if request.source == "genre" {
        return Ok(store
            .get_random_tracks_by_genre(
                &request.seed.entity_id,
                count.saturating_add(exclude.len()),
            )?
            .into_iter()
            .filter(|id| !exclude.contains(id))
            .take(count)
            .collect());
    }
    if request.source == "custom" {
        let mut recipe: RadioBuildRequest =
            serde_json::from_value(request.settings.unwrap_or_default())
                .map_err(|err| invalid_request!("invalid radio request: {err}"))?;
        recipe.seed = request.seed;
        recipe.count = Some(count);
        return build_radio_with_history(store, settings, recipe, exclude, &recent);
    }
    let namespace = track_namespace(settings);
    let Some(vector) = radio_seed_vector(store, settings, &request.seed, &namespace)? else {
        return Ok(Vec::new());
    };
    if request.seed.entity_type == "album" {
        exclude.extend(store.get_available_album_track_ids(&request.seed.entity_id)?);
    }
    let mut result = Vec::new();
    let mut album_cooldowns = HashMap::new();
    let mut artist_cooldowns = HashMap::new();
    let mut artist_last_positions = HashMap::new();
    let mut selection = RadioSelectionState {
        result: &mut result,
        exclude: &mut exclude,
        album_cooldowns: &mut album_cooldowns,
        artist_cooldowns: &mut artist_cooldowns,
        artist_last_positions: &mut artist_last_positions,
    };
    initialize_radio_history(store, &recent, &mut selection)?;
    let prefix = selection.result.len();
    append_radio_recommendations(
        store,
        &namespace,
        &vector,
        count + prefix,
        DEFAULT_RADIO_DIVERSITY,
        &mut selection,
    )?;
    Ok(result.into_iter().skip(prefix).collect())
}

fn initialize_radio_history(
    store: &dyn CatalogStore,
    recent: &[String],
    selection: &mut RadioSelectionState<'_>,
) -> anyhow::Result<()> {
    for id in recent {
        if let Some(track) = store.get_resolved_track(id)? {
            push_radio_result(id.clone(), &track, selection);
        }
    }
    Ok(())
}

async fn post_continuation_recommendations(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Json(body): Json<ContinuationRequest>,
) -> Response {
    let settings = state.config.audio_embeddings.clone();
    match state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |catalog_store| {
            continue_queue(catalog_store, settings.as_ref(), body)
        })
        .await
    {
        Ok(response) => no_store_json(response),
        Err(DbRunError::Store(err)) if is_invalid_request(&err) => {
            ApiError::bad_request("invalid_continuation_request", err.to_string()).into_response()
        }
        Err(DbRunError::Store(err)) => {
            ApiError::internal("Failed to generate continuation recommendations", err)
                .into_response()
        }
        Err(err) => ApiError::from(err).into_response(),
    }
}

async fn get_radio(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(query): Query<RadioQuery>,
) -> Response {
    if !matches!(entity_type.as_str(), "track" | "album" | "artist") {
        return ApiError::bad_request(
            "unsupported_entity_type",
            "Radio entity type must be track, album, or artist",
        )
        .into_response();
    }
    let count = query.count.unwrap_or(50).clamp(1, 200);
    let catalog = state.database.catalog_read.clone();
    let namespace = track_namespace(state.config.audio_embeddings.as_ref());
    let album_namespace = album_namespace_for_track_namespace_with_settings(
        state.config.audio_embeddings.as_ref(),
        &namespace,
    );

    match catalog
        .run(
            DbPriority::Interactive,
            move |catalog_store| match entity_type.as_str() {
                "track" => track_radio(catalog_store, &namespace, &entity_id, count),
                "album" => album_radio(
                    catalog_store,
                    &namespace,
                    &album_namespace,
                    &entity_id,
                    count,
                ),
                "artist" => artist_radio(catalog_store, &namespace, &entity_id, count),
                _ => unreachable!("entity type was validated before spawning"),
            },
        )
        .await
    {
        Ok(track_ids) => no_store_json(TrackIdsResponse { track_ids }),
        Err(DbRunError::Store(err)) => {
            ApiError::internal("Failed to generate radio", err).into_response()
        }
        Err(err) => ApiError::from(err).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ConceptsQuery {
    q: Option<String>,
    family: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq)]
struct ConceptSummary {
    id: String,
    family: String,
    label: String,
    example_count: u64,
}

#[derive(Debug, Serialize)]
struct ConceptsResponse {
    concepts: Vec<ConceptSummary>,
}

const CONCEPT_FAMILY_ORDER: &[&str] = &[
    "sound_genre",
    "instrument",
    "vocals",
    "mood",
    "genre_tag",
    "recorded",
    "composed",
];

async fn get_concepts(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Query(query): Query<ConceptsQuery>,
) -> Response {
    let preferred_namespace = track_namespace(state.config.audio_embeddings.as_ref());
    match state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |store| {
            store.list_entity_embedding_metadata("concept")
        })
        .await
    {
        Ok(rows) => no_store_json(ConceptsResponse {
            concepts: summarize_concepts(rows, &preferred_namespace, &query),
        }),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// One summary per concept (metadata from the preferred namespace when present), filtered
/// and ordered by family, then label; decade families sort chronologically by id.
fn summarize_concepts(
    rows: Vec<(String, String, serde_json::Value)>,
    preferred_namespace: &str,
    query: &ConceptsQuery,
) -> Vec<ConceptSummary> {
    let mut by_id: HashMap<String, (bool, serde_json::Value)> = HashMap::new();
    for (id, namespace, metadata) in rows {
        let preferred = namespace == preferred_namespace;
        match by_id.get(&id) {
            Some((true, _)) => {}
            Some((false, _)) if !preferred => {}
            _ => {
                by_id.insert(id, (preferred, metadata));
            }
        }
    }
    let needle = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let mut concepts = by_id
        .into_iter()
        .map(|(id, (_, metadata))| ConceptSummary {
            family: metadata["family"].as_str().unwrap_or("other").to_string(),
            label: metadata["label"].as_str().unwrap_or(&id).to_string(),
            example_count: metadata["example_count"].as_u64().unwrap_or(0),
            id,
        })
        .filter(|concept| query.family.as_deref().is_none_or(|family| concept.family == family))
        .filter(|concept| {
            needle
                .as_deref()
                .is_none_or(|needle| concept.label.to_lowercase().contains(needle))
        })
        .collect::<Vec<_>>();
    let family_rank = |family: &str| {
        CONCEPT_FAMILY_ORDER
            .iter()
            .position(|candidate| *candidate == family)
            .unwrap_or(CONCEPT_FAMILY_ORDER.len())
    };
    concepts.sort_by(|left, right| {
        family_rank(&left.family)
            .cmp(&family_rank(&right.family))
            .then_with(|| match left.family.as_str() {
                "recorded" | "composed" => decade_of(&left.id).cmp(&decade_of(&right.id)),
                _ => left.label.to_lowercase().cmp(&right.label.to_lowercase()),
            })
    });
    concepts.truncate(query.limit.unwrap_or(50).clamp(1, 500));
    concepts
}

fn decade_of(concept_id: &str) -> i64 {
    concept_id
        .rsplit(':')
        .next()
        .and_then(|decade| decade.trim_end_matches('s').parse().ok())
        .unwrap_or(i64::MAX)
}

async fn get_radio_options(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
) -> Response {
    no_store_json(radio_options_response(
        state.config.audio_embeddings.as_ref(),
    ))
}

async fn post_radio_build(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Json(body): Json<RadioBuildRequest>,
) -> Response {
    let catalog = state.database.catalog_read.clone();
    let settings = state.config.audio_embeddings.clone();

    match catalog
        .run(DbPriority::Interactive, move |catalog_store| {
            build_radio(catalog_store, settings.as_ref(), body)
        })
        .await
    {
        Ok(track_ids) => no_store_json(TrackIdsResponse { track_ids }),
        Err(DbRunError::Store(err)) => {
            let status = if is_invalid_request(&err) {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error!("Error building advanced radio: {}", err);
            (status, err.to_string()).into_response()
        }
        Err(err) => ApiError::from(err).into_response(),
    }
}

fn no_store_json<T: Serialize>(value: T) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}

fn track_namespace(settings: Option<&AudioEmbeddingsSettings>) -> String {
    let Some(settings) = settings else {
        return DEFAULT_TRACK_NAMESPACE.to_string();
    };
    let served = settings
        .specs
        .iter()
        .filter(|spec| spec.serve)
        .collect::<Vec<_>>();
    served
        .iter()
        .find(|spec| spec.namespace == DEFAULT_TRACK_NAMESPACE)
        .or_else(|| served.first())
        .map(|spec| spec.namespace.clone())
        .unwrap_or_else(|| DEFAULT_TRACK_NAMESPACE.to_string())
}

fn album_namespace_for_track_namespace(namespace: &str) -> String {
    match namespace {
        "musicfm.mean.v1" => DEFAULT_ALBUM_NAMESPACE.to_string(),
        "ast.audioset.v1" => "album.ast.median.v1".to_string(),
        "ast.instruments.v1" => "album.ast_instruments.median.v1".to_string(),
        "ast.audioset.v2" => "album.ast.median.v2".to_string(),
        "ast.instruments.v2" => "album.ast_instruments.median.v2".to_string(),
        _ => format!("album.{namespace}.median"),
    }
}

fn radio_options_response(settings: Option<&AudioEmbeddingsSettings>) -> RadioOptionsResponse {
    let namespaces = available_track_namespaces(settings);
    let criteria = namespaces
        .iter()
        .map(|namespace| RadioCriterionOption {
            namespace: namespace.clone(),
            label: criterion_label(namespace),
        })
        .collect::<Vec<_>>();
    RadioOptionsResponse {
        recipes: radio_recipes_for_namespaces(&namespaces),
        criteria,
        default_recipe_id: "balanced".to_string(),
        modes: vec!["similar", "explore"],
        explicit_filters: vec!["include", "exclude", "only"],
        count: RadioRangeUsize {
            min: 1,
            max: 200,
            default: 50,
        },
        diversity: RadioRangeF32 {
            min: 0.0,
            max: 1.0,
            default: DEFAULT_RADIO_DIVERSITY,
        },
        randomness: RadioRangeF32 {
            min: 0.0,
            max: 1.0,
            default: DEFAULT_RADIO_RANDOMNESS,
        },
    }
}

fn available_track_namespaces(settings: Option<&AudioEmbeddingsSettings>) -> Vec<String> {
    let mut namespaces = settings
        .map(|settings| {
            settings
                .specs
                .iter()
                .filter(|spec| spec.serve)
                .map(|spec| spec.namespace.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![DEFAULT_TRACK_NAMESPACE.to_string()]);
    if namespaces.is_empty() {
        namespaces.push(DEFAULT_TRACK_NAMESPACE.to_string());
    }
    namespaces
}

/// AST namespaces in order of preference: whole-track windows first, single clip last.
const AST_AUDIOSET_NAMESPACES: &[&str] = &["ast.audioset.v2", "ast.audioset.v1"];
const AST_INSTRUMENTS_NAMESPACES: &[&str] = &["ast.instruments.v2", "ast.instruments.v1"];

fn preferred_namespace<'a>(namespaces: &[String], preference: &[&'a str]) -> Option<&'a str> {
    preference
        .iter()
        .copied()
        .find(|candidate| namespaces.iter().any(|namespace| namespace == candidate))
}

fn radio_recipes_for_namespaces(namespaces: &[String]) -> Vec<RadioRecipe> {
    let has = |namespace: &str| namespaces.iter().any(|item| item == namespace);
    let mut recipes = Vec::new();

    recipes.push(recipe(
        "classic",
        "Classic",
        vec![(track_namespace_from_available(namespaces), 1.0)],
        "similar",
        0.2,
        0.3,
    ));

    let mut balanced = Vec::new();
    if has("musicfm.mean.v1") {
        balanced.push(("musicfm.mean.v1".to_string(), 0.55));
    }
    let audio_scene = preferred_namespace(namespaces, AST_AUDIOSET_NAMESPACES);
    let instrumentation = preferred_namespace(namespaces, AST_INSTRUMENTS_NAMESPACES);
    if let Some(namespace) = audio_scene {
        balanced.push((namespace.to_string(), 0.3));
    }
    if let Some(namespace) = instrumentation {
        balanced.push((namespace.to_string(), 0.15));
    }
    if balanced.is_empty() {
        balanced.push((track_namespace_from_available(namespaces), 1.0));
    }
    recipes.push(recipe(
        "balanced", "Balanced", balanced, "similar", 0.3, 0.3,
    ));

    recipes.push(recipe_for_namespace(
        "sound_similarity",
        "Sound similarity",
        "musicfm.mean.v1",
        namespaces,
        "similar",
        0.25,
        0.2,
    ));
    recipes.push(recipe_for_namespace(
        "audio_scene",
        "Audio scene",
        audio_scene.unwrap_or("ast.audioset.v1"),
        namespaces,
        "similar",
        0.35,
        0.25,
    ));
    recipes.push(recipe_for_namespace(
        "instrumentation",
        "Instrumentation",
        instrumentation.unwrap_or("ast.instruments.v1"),
        namespaces,
        "similar",
        0.35,
        0.25,
    ));
    recipes.push(recipe(
        "deep_discovery",
        "Deep discovery",
        namespaces
            .iter()
            .map(|namespace| (namespace.clone(), 1.0 / namespaces.len() as f32))
            .collect(),
        "explore",
        0.65,
        0.55,
    ));

    recipes
}

fn recipe_for_namespace(
    id: &str,
    name: &str,
    preferred_namespace: &str,
    namespaces: &[String],
    mode: &'static str,
    diversity: f32,
    randomness: f32,
) -> RadioRecipe {
    let namespace = if namespaces.iter().any(|item| item == preferred_namespace) {
        preferred_namespace.to_string()
    } else {
        track_namespace_from_available(namespaces)
    };
    recipe(
        id,
        name,
        vec![(namespace, 1.0)],
        mode,
        diversity,
        randomness,
    )
}

fn recipe(
    id: &str,
    name: &str,
    criteria: Vec<(String, f32)>,
    mode: &'static str,
    diversity: f32,
    randomness: f32,
) -> RadioRecipe {
    RadioRecipe {
        id: id.to_string(),
        name: name.to_string(),
        criteria: criteria
            .into_iter()
            .map(|(namespace, weight)| RadioCriterionRequestForResponse { namespace, weight })
            .collect(),
        mode,
        diversity,
        randomness,
    }
}

fn track_namespace_from_available(namespaces: &[String]) -> String {
    namespaces
        .iter()
        .find(|namespace| namespace.as_str() == DEFAULT_TRACK_NAMESPACE)
        .or_else(|| namespaces.first())
        .cloned()
        .unwrap_or_else(|| DEFAULT_TRACK_NAMESPACE.to_string())
}

fn criterion_label(namespace: &str) -> String {
    match namespace {
        "musicfm.mean.v1" => "Sound profile".to_string(),
        "ast.audioset.v1" | "ast.audioset.v2" => "Audio scene".to_string(),
        "ast.instruments.v1" | "ast.instruments.v2" => "Instrumentation".to_string(),
        other => other.to_string(),
    }
}

fn build_radio(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    request: RadioBuildRequest,
) -> anyhow::Result<Vec<String>> {
    build_radio_with_history(catalog_store, settings, request, HashSet::new(), &[])
}

fn build_radio_with_history(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    request: RadioBuildRequest,
    mut exclude: HashSet<String>,
    recent: &[String],
) -> anyhow::Result<Vec<String>> {
    let continuing = !exclude.is_empty() || !recent.is_empty();
    validate_entity_type(&request.seed.entity_type)?;
    if request.criteria.len() > 8 || request.toward.len() > 8 || request.away.len() > 8 {
        return Err(invalid_request!(
            "invalid radio request: at most 8 criteria and 8 references per direction are allowed"
        ));
    }
    if let Some(recipe_id) = request.recipe_id.as_deref() {
        if !recipes_contain_recipe(settings, recipe_id) {
            return Err(invalid_request!(
                "invalid radio request: unknown recipe_id '{recipe_id}'"
            ));
        }
    }
    for reference in request.toward.iter().chain(request.away.iter()) {
        validate_entity_type(&reference.entity_type)?;
        if reference
            .weight
            .is_some_and(|weight| !weight.is_finite() || weight <= 0.0)
        {
            return Err(invalid_request!(
                "invalid radio request: reference weights must be positive"
            ));
        }
    }

    if request.diversity.is_some_and(|value| !value.is_finite())
        || request.randomness.is_some_and(|value| !value.is_finite())
    {
        return Err(invalid_request!(
            "invalid radio request: diversity and randomness must be finite"
        ));
    }

    let count = request.count.unwrap_or(50).clamp(1, 200);
    let recipes = radio_recipes_for_namespaces(&available_track_namespaces(settings));
    let criteria = normalize_radio_criteria(settings, &recipes, &request)?;
    let recipe = selected_recipe(&recipes, request.recipe_id.as_deref());
    let mode = request.mode.unwrap_or_else(|| recipe_mode(recipe));
    let diversity = request
        .diversity
        .unwrap_or_else(|| {
            recipe
                .map(|recipe| recipe.diversity)
                .unwrap_or(DEFAULT_RADIO_DIVERSITY)
        })
        .clamp(0.0, 1.0);
    let randomness = request
        .randomness
        .unwrap_or_else(|| {
            recipe
                .map(|recipe| recipe.randomness)
                .unwrap_or(DEFAULT_RADIO_RANDOMNESS)
        })
        .clamp(0.0, 1.0);
    validate_filters(request.filters.as_ref())?;

    let include_seed_tracks = request
        .include_seed_tracks
        .unwrap_or_else(|| request.seed.entity_type != "album");
    let seed_track_ids = seed_track_ids(catalog_store, &request.seed)?;
    let mut result = Vec::with_capacity(count);
    if !include_seed_tracks {
        exclude.extend(seed_track_ids.iter().cloned());
    }

    let oversample = (count * criteria.len().max(1) * 24).clamp(150, 2000);
    let mut queries = Vec::with_capacity(criteria.len());
    for criterion in &criteria {
        let Some(seed) =
            radio_seed_vector(catalog_store, settings, &request.seed, &criterion.namespace)?
        else {
            continue;
        };
        let Some(vector) = steered_vector(
            catalog_store,
            settings,
            &criterion.namespace,
            seed,
            &request.toward,
            &request.away,
        )?
        else {
            continue;
        };
        queries.push(NamespaceQuery {
            namespace: criterion.namespace.clone(),
            weight: criterion.weight,
            vector,
        });
    }
    let candidates = score_radio_candidates(
        catalog_store,
        &queries,
        mode,
        oversample,
        &exclude,
        continuing,
        request.filters.as_ref(),
    )?;
    let ranked = rank_radio_candidates(
        catalog_store,
        candidates,
        request.filters.as_ref(),
        randomness,
    )?;

    let mut album_cooldowns: HashMap<String, f32> = HashMap::new();
    let mut artist_cooldowns: HashMap<String, f32> = HashMap::new();
    let mut artist_last_positions: HashMap<String, usize> = HashMap::new();
    {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        initialize_radio_history(catalog_store, recent, &mut selection)?;
        if include_seed_tracks && !continuing {
            for track_id in &seed_track_ids {
                if selection.result.len() >= count {
                    break;
                }
                if selection.exclude.contains(track_id) {
                    continue;
                }
                let Some(resolved) = catalog_store.get_resolved_track(track_id)? else {
                    continue;
                };
                if !resolved_track_passes_filters(&resolved, request.filters.as_ref()) {
                    continue;
                }
                selection.exclude.insert(track_id.clone());
                push_radio_result(track_id.clone(), &resolved, &mut selection);
            }
        } else {
            let mut initialized_seed_context = HashSet::new();
            for track_id in &seed_track_ids {
                if !initialized_seed_context.insert(track_id) {
                    continue;
                }
                if let Some(resolved) = catalog_store.get_resolved_track(track_id)? {
                    if resolved_track_passes_filters(&resolved, request.filters.as_ref()) {
                        apply_track_cooldown(
                            &resolved,
                            selection.album_cooldowns,
                            selection.artist_cooldowns,
                        );
                    }
                }
            }
        }
    }

    {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        let prefix = selection.result.len();
        select_radio_candidates(
            ranked,
            if continuing { count + prefix } else { count },
            diversity,
            &mut selection,
        );
        if continuing {
            selection.result.drain(..prefix);
        }
    }

    Ok(result)
}

/// Search every namespace query and accumulate weighted candidate scores.
fn score_radio_candidates(
    catalog_store: &dyn CatalogStore,
    queries: &[NamespaceQuery],
    mode: RadioMode,
    oversample: usize,
    exclude: &HashSet<String>,
    continuing: bool,
    filters: Option<&RadioFilters>,
) -> anyhow::Result<HashMap<String, CandidateScore>> {
    let mut candidates: HashMap<String, CandidateScore> = HashMap::new();
    for query in queries {
        let results = if continuing {
            continuation_candidates(
                catalog_store,
                &query.namespace,
                &query.vector,
                oversample,
                exclude,
                filters,
            )?
        } else {
            catalog_store.search_available_track_embeddings(
                &query.namespace,
                &query.vector,
                oversample,
            )?
        };
        for search_result in results {
            if exclude.contains(&search_result.entity_id) {
                continue;
            }
            let similarity = search_result.score;
            let score = match mode {
                RadioMode::Similar => similarity,
                RadioMode::Explore => 1.0 - (similarity.clamp(-1.0, 1.0) - 0.55).abs(),
            } * query.weight;
            let entry = candidates
                .entry(search_result.entity_id)
                .or_insert(CandidateScore {
                    score: 0.0,
                    best_similarity: similarity,
                });
            entry.score += score;
            entry.best_similarity = entry.best_similarity.max(similarity);
        }
    }
    Ok(candidates)
}

/// Resolve, filter and jitter scored candidates, best first.
fn rank_radio_candidates(
    catalog_store: &dyn CatalogStore,
    candidates: HashMap<String, CandidateScore>,
    filters: Option<&RadioFilters>,
    randomness: f32,
) -> anyhow::Result<Vec<RankedRadioCandidate>> {
    let mut rng = rand::rng();
    let mut scored = candidates.into_iter().collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        let left_score = left.1.score + left.1.best_similarity * 0.05;
        let right_score = right.1.score + right.1.best_similarity * 0.05;
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranked = Vec::with_capacity(scored.len());
    for (track_id, candidate) in scored {
        let Some(resolved) = catalog_store.get_resolved_track(&track_id)? else {
            continue;
        };
        if !resolved_track_passes_filters(&resolved, filters) {
            continue;
        }
        let jitter = if randomness > 0.0 {
            rng.random_range(0.0..(0.1 * randomness))
        } else {
            0.0
        };
        ranked.push(RankedRadioCandidate {
            track_id,
            resolved,
            score: candidate.score + candidate.best_similarity * 0.05 + jitter,
        });
    }
    Ok(ranked)
}

// Grow the search window until there are enough unseen, filter-matching candidates.
// Do not resolve metadata for the entire catalog on every continuation request.
fn continuation_candidates(
    store: &dyn CatalogStore,
    namespace: &str,
    query: &[f32],
    target: usize,
    exclude: &HashSet<String>,
    filters: Option<&RadioFilters>,
) -> anyhow::Result<Vec<crate::catalog_store::EntityEmbeddingSearchResult>> {
    let mut limit = target.saturating_add(exclude.len());
    let mut scanned = 0;
    let mut candidates = Vec::new();
    loop {
        let results = store.search_available_track_embeddings(namespace, query, limit)?;
        let exhausted = results.len() < limit;
        let result_count = results.len();
        for result in results.into_iter().skip(scanned) {
            if exclude.contains(&result.entity_id) {
                continue;
            }
            let Some(track) = store.get_resolved_track(&result.entity_id)? else {
                continue;
            };
            if resolved_track_passes_filters(&track, filters) {
                candidates.push(result);
                if candidates.len() == target {
                    return Ok(candidates);
                }
            }
        }
        if exhausted || result_count == scanned {
            return Ok(candidates);
        }
        scanned = result_count;
        limit = limit.saturating_mul(2);
    }
}

fn select_radio_candidates(
    mut ranked: Vec<RankedRadioCandidate>,
    count: usize,
    diversity: f32,
    selection: &mut RadioSelectionState<'_>,
) {
    while selection.result.len() < count && !ranked.is_empty() {
        ranked.retain(|candidate| !selection.exclude.contains(&candidate.track_id));
        let has_fresh_artist = ranked.iter().any(|candidate| {
            !artist_recently_used(
                &candidate.resolved,
                selection.result.len(),
                selection.artist_last_positions,
            )
        });
        let Some((selected_index, _)) = ranked
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                !has_fresh_artist
                    || !artist_recently_used(
                        &candidate.resolved,
                        selection.result.len(),
                        selection.artist_last_positions,
                    )
            })
            .map(|(index, candidate)| {
                (
                    index,
                    adjusted_radio_score(
                        candidate.score,
                        &candidate.resolved,
                        diversity,
                        selection.album_cooldowns,
                        selection.artist_cooldowns,
                    ),
                )
            })
            .max_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            break;
        };
        let candidate = ranked.swap_remove(selected_index);
        if !selection.exclude.insert(candidate.track_id.clone()) {
            continue;
        }
        push_radio_result(candidate.track_id, &candidate.resolved, selection);
    }
}

fn push_radio_result(
    track_id: String,
    resolved: &ResolvedTrack,
    selection: &mut RadioSelectionState<'_>,
) {
    let position = selection.result.len();
    selection.result.push(track_id);
    apply_track_cooldown(
        resolved,
        selection.album_cooldowns,
        selection.artist_cooldowns,
    );
    record_artist_positions(resolved, position, selection.artist_last_positions);
}

fn artist_recently_used(
    resolved: &ResolvedTrack,
    next_position: usize,
    artist_last_positions: &HashMap<String, usize>,
) -> bool {
    artist_ids_for_track(resolved).into_iter().any(|artist_id| {
        artist_last_positions
            .get(&artist_id)
            .is_some_and(|last_position| {
                next_position.saturating_sub(*last_position) < RADIO_ARTIST_REPEAT_MIN_DISTANCE
            })
    })
}

fn record_artist_positions(
    resolved: &ResolvedTrack,
    position: usize,
    artist_last_positions: &mut HashMap<String, usize>,
) {
    for artist_id in artist_ids_for_track(resolved) {
        artist_last_positions.insert(artist_id, position);
    }
}

fn adjusted_radio_score(
    score: f32,
    resolved: &ResolvedTrack,
    diversity: f32,
    album_cooldowns: &HashMap<String, f32>,
    artist_cooldowns: &HashMap<String, f32>,
) -> f32 {
    let album_penalty = album_cooldowns
        .get(&resolved.album.id)
        .copied()
        .unwrap_or(0.0)
        * diversity
        * RADIO_ALBUM_PENALTY_WEIGHT;
    let artist_penalty = artist_ids_for_track(resolved)
        .into_iter()
        .filter_map(|artist_id| artist_cooldowns.get(&artist_id).copied())
        .fold(0.0, f32::max)
        * diversity
        * RADIO_ARTIST_PENALTY_WEIGHT;
    score - album_penalty - artist_penalty
}

fn apply_track_cooldown(
    resolved: &ResolvedTrack,
    album_cooldowns: &mut HashMap<String, f32>,
    artist_cooldowns: &mut HashMap<String, f32>,
) {
    decay_cooldowns(album_cooldowns);
    decay_cooldowns(artist_cooldowns);
    album_cooldowns.insert(resolved.album.id.clone(), 1.0);
    for artist_id in artist_ids_for_track(resolved) {
        artist_cooldowns.insert(artist_id, 1.0);
    }
}

fn decay_cooldowns(cooldowns: &mut HashMap<String, f32>) {
    for value in cooldowns.values_mut() {
        *value *= RADIO_COOLDOWN_DECAY;
    }
    cooldowns.retain(|_, value| *value >= RADIO_COOLDOWN_MIN);
}

fn artist_ids_for_track(resolved: &ResolvedTrack) -> Vec<String> {
    let mut seen = HashSet::new();
    resolved
        .artists
        .iter()
        .filter_map(|track_artist| {
            if seen.insert(track_artist.artist.id.as_str()) {
                Some(track_artist.artist.id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn recipes_contain_recipe(settings: Option<&AudioEmbeddingsSettings>, recipe_id: &str) -> bool {
    radio_recipes_for_namespaces(&available_track_namespaces(settings))
        .iter()
        .any(|recipe| recipe.id == recipe_id)
}

fn validate_entity_type(entity_type: &str) -> anyhow::Result<()> {
    match entity_type {
        "track" | "album" | "artist" | "concept" => Ok(()),
        other => Err(invalid_request!(
            "invalid radio request: unsupported entity_type '{other}'"
        )),
    }
}

fn validate_filters(filters: Option<&RadioFilters>) -> anyhow::Result<()> {
    let Some(filters) = filters else {
        return Ok(());
    };
    if let (Some(min), Some(max)) = (filters.release_year_min, filters.release_year_max) {
        if min > max {
            return Err(invalid_request!(
                "invalid radio request: release_year_min must be <= release_year_max"
            ));
        }
    }
    if let (Some(min), Some(max)) = (filters.popularity_min, filters.popularity_max) {
        if min > max {
            return Err(invalid_request!(
                "invalid radio request: popularity_min must be <= popularity_max"
            ));
        }
    }
    if filters
        .popularity_min
        .is_some_and(|value| !(0..=100).contains(&value))
        || filters
            .popularity_max
            .is_some_and(|value| !(0..=100).contains(&value))
    {
        return Err(invalid_request!(
            "invalid radio request: popularity filters must be between 0 and 100"
        ));
    }
    Ok(())
}

fn normalize_radio_criteria(
    settings: Option<&AudioEmbeddingsSettings>,
    recipes: &[RadioRecipe],
    request: &RadioBuildRequest,
) -> anyhow::Result<Vec<NormalizedRadioCriterion>> {
    let raw = if request.criteria.is_empty() {
        let recipe = selected_recipe(recipes, request.recipe_id.as_deref())
            .ok_or_else(|| invalid_request!("invalid radio request: unknown recipe_id"))?;
        recipe
            .criteria
            .iter()
            .map(|criterion| RadioCriterionRequest {
                namespace: criterion.namespace.clone(),
                weight: criterion.weight,
            })
            .collect::<Vec<_>>()
    } else {
        request.criteria.clone()
    };
    normalize_criteria_list(settings, raw)
}

fn normalize_criteria_list(
    settings: Option<&AudioEmbeddingsSettings>,
    raw: Vec<RadioCriterionRequest>,
) -> anyhow::Result<Vec<NormalizedRadioCriterion>> {
    let allowed = available_track_namespaces(settings)
        .into_iter()
        .collect::<HashSet<_>>();
    let mut criteria = Vec::new();
    for criterion in raw {
        if !allowed.contains(&criterion.namespace) {
            return Err(invalid_request!(
                "invalid radio request: unsupported namespace '{}'",
                criterion.namespace
            ));
        }
        if !criterion.weight.is_finite() || criterion.weight <= 0.0 {
            return Err(invalid_request!(
                "invalid radio request: criterion weights must be positive"
            ));
        }
        criteria.push(NormalizedRadioCriterion {
            namespace: criterion.namespace,
            weight: criterion.weight,
        });
    }
    if criteria.is_empty() {
        return Err(invalid_request!(
            "invalid radio request: at least one criterion is required"
        ));
    }
    let total_weight = criteria
        .iter()
        .map(|criterion| criterion.weight)
        .sum::<f32>();
    if !total_weight.is_finite() {
        return Err(invalid_request!(
            "invalid radio request: combined criterion weights are too large"
        ));
    }
    for criterion in &mut criteria {
        criterion.weight /= total_weight;
    }
    Ok(criteria)
}

fn selected_recipe<'a>(
    recipes: &'a [RadioRecipe],
    recipe_id: Option<&str>,
) -> Option<&'a RadioRecipe> {
    let requested = recipe_id.unwrap_or("balanced");
    recipes
        .iter()
        .find(|recipe| recipe.id == requested)
        .or_else(|| recipes.iter().find(|recipe| recipe.id == "balanced"))
}

fn recipe_mode(recipe: Option<&RadioRecipe>) -> RadioMode {
    match recipe.map(|recipe| recipe.mode) {
        Some("explore") => RadioMode::Explore,
        _ => RadioMode::Similar,
    }
}

fn seed_track_ids(
    catalog_store: &dyn CatalogStore,
    seed: &RadioSeed,
) -> anyhow::Result<Vec<String>> {
    match seed.entity_type.as_str() {
        "track" => Ok(vec![seed.entity_id.clone()]),
        "album" => catalog_store.get_available_album_track_ids(&seed.entity_id),
        "artist" => {
            catalog_store.get_artist_top_track_ids(&seed.entity_id, ARTIST_SEED_TRACK_LIMIT)
        }
        _ => Ok(Vec::new()),
    }
}

fn radio_seed_vector(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    seed: &RadioSeed,
    track_namespace: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    match seed.entity_type.as_str() {
        "track" => get_vector(catalog_store, "track", &seed.entity_id, track_namespace),
        "concept" => get_vector(catalog_store, "concept", &seed.entity_id, track_namespace),
        "album" => {
            let album_namespace =
                album_namespace_for_track_namespace_with_settings(settings, track_namespace);
            match get_vector(catalog_store, "album", &seed.entity_id, &album_namespace)? {
                Some(vector) => Ok(Some(vector)),
                None => {
                    let track_ids = catalog_store.get_available_album_track_ids(&seed.entity_id)?;
                    mean_track_vector(catalog_store, track_namespace, &track_ids)
                }
            }
        }
        "artist" => {
            let track_ids =
                catalog_store.get_artist_top_track_ids(&seed.entity_id, ARTIST_SEED_TRACK_LIMIT)?;
            mean_track_vector(catalog_store, track_namespace, &track_ids)
        }
        _ => Ok(None),
    }
}

fn album_namespace_for_track_namespace_with_settings(
    settings: Option<&AudioEmbeddingsSettings>,
    track_namespace: &str,
) -> String {
    settings
        .and_then(|settings| {
            settings
                .album_derivations
                .specs
                .iter()
                .find(|spec| spec.source_namespace == track_namespace)
        })
        .map(|spec| spec.target_namespace.clone())
        .unwrap_or_else(|| album_namespace_for_track_namespace(track_namespace))
}

fn steered_vector(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    namespace: &str,
    mut seed: Vec<f32>,
    toward: &[RadioReference],
    away: &[RadioReference],
) -> anyhow::Result<Option<Vec<f32>>> {
    let dim = seed.len();
    for reference in toward {
        if let Some(vector) = reference_vector(catalog_store, settings, reference, namespace)? {
            add_scaled_vector(&mut seed, &vector, reference.weight.unwrap_or(1.0), dim);
        }
    }
    for reference in away {
        if let Some(vector) = reference_vector(catalog_store, settings, reference, namespace)? {
            add_scaled_vector(&mut seed, &vector, -reference.weight.unwrap_or(1.0), dim);
        }
    }
    if seed.iter().all(|value| value.abs() <= f32::EPSILON) {
        return Ok(None);
    }
    Ok(Some(seed))
}

fn reference_vector(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    reference: &RadioReference,
    namespace: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    radio_seed_vector(
        catalog_store,
        settings,
        &RadioSeed {
            entity_type: reference.entity_type.clone(),
            entity_id: reference.entity_id.clone(),
        },
        namespace,
    )
}

fn add_scaled_vector(seed: &mut [f32], vector: &[f32], weight: f32, dim: usize) {
    if !weight.is_finite() || vector.len() != dim {
        return;
    }
    for (idx, value) in vector.iter().enumerate() {
        seed[idx] += *value * weight;
    }
}

fn resolved_track_passes_filters(resolved: &ResolvedTrack, filters: Option<&RadioFilters>) -> bool {
    if resolved.track.availability != TrackAvailability::Available {
        return false;
    }
    let Some(filters) = filters else {
        return true;
    };
    if let Some(min) = filters.popularity_min {
        if resolved.track.popularity < min {
            return false;
        }
    }
    if let Some(max) = filters.popularity_max {
        if resolved.track.popularity > max {
            return false;
        }
    }
    match filters.explicit.unwrap_or(ExplicitFilter::Include) {
        ExplicitFilter::Include => {}
        ExplicitFilter::Exclude if resolved.track.explicit => return false,
        ExplicitFilter::Only if !resolved.track.explicit => return false,
        _ => {}
    }
    if let Some(year) = release_year(&resolved.album.release_date) {
        if let Some(min) = filters.release_year_min {
            if year < min {
                return false;
            }
        }
        if let Some(max) = filters.release_year_max {
            if year > max {
                return false;
            }
        }
    } else if filters.release_year_min.is_some() || filters.release_year_max.is_some() {
        return false;
    }
    if !filters.genres.is_empty() {
        let requested = filters
            .genres
            .iter()
            .map(|genre| genre.to_lowercase())
            .collect::<HashSet<_>>();
        let has_genre = resolved.artists.iter().any(|track_artist| {
            track_artist
                .artist
                .genres
                .iter()
                .any(|genre| requested.contains(&genre.to_lowercase()))
        });
        if !has_genre {
            return false;
        }
    }
    true
}

fn release_year(release_date: &Option<String>) -> Option<i32> {
    release_date
        .as_deref()
        .and_then(|date| date.get(0..4))
        .and_then(|year| year.parse::<i32>().ok())
}

fn append_radio_recommendations(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    seed: &[f32],
    count: usize,
    diversity: f32,
    selection: &mut RadioSelectionState<'_>,
) -> anyhow::Result<()> {
    if selection.result.len() >= count {
        return Ok(());
    }

    let oversample = (count.saturating_sub(selection.result.len()) * 16)
        .clamp(100, 1000)
        .saturating_add(selection.exclude.len());
    let results = catalog_store.search_available_track_embeddings(namespace, seed, oversample)?;

    let mut rng = rand::rng();
    let mut scored = results
        .into_iter()
        .map(|result| (result.entity_id, result.score + rng.random_range(0.0..0.03)))
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut ranked = Vec::with_capacity(scored.len());
    for (track_id, score) in scored {
        if selection.exclude.contains(&track_id) {
            continue;
        }
        let Some(resolved) = catalog_store.get_resolved_track(&track_id)? else {
            continue;
        };
        if !resolved_track_passes_filters(&resolved, None) {
            continue;
        }
        ranked.push(RankedRadioCandidate {
            track_id,
            resolved,
            score,
        });
    }

    select_radio_candidates(ranked, count, diversity, selection);

    Ok(())
}

fn track_radio(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let mut result = Vec::new();
    let mut exclude = HashSet::new();
    let mut album_cooldowns = HashMap::new();
    let mut artist_cooldowns = HashMap::new();
    let mut artist_last_positions = HashMap::new();

    {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        if let Some(resolved) = catalog_store.get_resolved_track(track_id)? {
            if resolved_track_passes_filters(&resolved, None) {
                selection.exclude.insert(track_id.to_string());
                push_radio_result(track_id.to_string(), &resolved, &mut selection);
            }
        }

        let Some(seed) = get_vector(catalog_store, "track", track_id, namespace)? else {
            return Ok(result);
        };
        append_radio_recommendations(
            catalog_store,
            namespace,
            &seed,
            count,
            DEFAULT_RADIO_DIVERSITY,
            &mut selection,
        )?;
    }
    Ok(result)
}

fn album_radio(
    catalog_store: &dyn CatalogStore,
    track_namespace: &str,
    album_namespace: &str,
    album_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let album_track_ids = catalog_store.get_available_album_track_ids(album_id)?;
    let exclude = album_track_ids.iter().cloned().collect::<HashSet<_>>();

    let seed = match get_vector(catalog_store, "album", album_id, album_namespace)? {
        Some(vector) => Some(vector),
        None => mean_track_vector(catalog_store, track_namespace, &album_track_ids)?,
    };
    let Some(seed) = seed else {
        return Ok(Vec::new());
    };

    let mut result = Vec::with_capacity(count);
    let mut exclude = exclude;
    let mut album_cooldowns = HashMap::new();
    let mut artist_cooldowns = HashMap::new();
    let mut artist_last_positions = HashMap::new();
    {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        append_radio_recommendations(
            catalog_store,
            track_namespace,
            &seed,
            count,
            DEFAULT_RADIO_DIVERSITY,
            &mut selection,
        )?;
    }
    Ok(result)
}

fn artist_radio(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    artist_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let top_tracks = catalog_store.get_artist_top_track_ids(artist_id, ARTIST_SEED_TRACK_LIMIT)?;
    let mut result = Vec::new();
    let mut exclude = HashSet::new();
    let mut album_cooldowns = HashMap::new();
    let mut artist_cooldowns = HashMap::new();
    let mut artist_last_positions = HashMap::new();

    {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        if let Some(first) = top_tracks.first() {
            if let Some(resolved) = catalog_store.get_resolved_track(first)? {
                if resolved_track_passes_filters(&resolved, None) {
                    selection.exclude.insert(first.clone());
                    push_radio_result(first.clone(), &resolved, &mut selection);
                }
            }
        }

        let Some(seed) = mean_track_vector(catalog_store, namespace, &top_tracks)? else {
            return Ok(result);
        };
        append_radio_recommendations(
            catalog_store,
            namespace,
            &seed,
            count,
            DEFAULT_RADIO_DIVERSITY,
            &mut selection,
        )?;
    }
    Ok(result)
}

fn weighted_track_vector(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_ids: &[String],
    max_tracks: usize,
) -> anyhow::Result<Option<Vec<f32>>> {
    let recent = track_ids.iter().rev().take(max_tracks);
    let mut weighted_vectors = Vec::new();
    let mut weight = 1.0_f32;

    for track_id in recent {
        if let Some(vector) = get_vector(catalog_store, "track", track_id, namespace)? {
            weighted_vectors.push((vector, weight));
        }
        weight *= 0.85;
    }

    weighted_mean(weighted_vectors)
}

fn mean_track_vector(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_ids: &[String],
) -> anyhow::Result<Option<Vec<f32>>> {
    let mut vectors = Vec::new();
    for track_id in track_ids {
        if let Some(vector) = get_vector(catalog_store, "track", track_id, namespace)? {
            vectors.push((vector, 1.0));
        }
    }
    weighted_mean(vectors)
}

fn weighted_mean(vectors: Vec<(Vec<f32>, f32)>) -> anyhow::Result<Option<Vec<f32>>> {
    let Some((first, _)) = vectors.first() else {
        return Ok(None);
    };
    let dim = first.len();
    let mut out = vec![0.0_f32; dim];
    let mut total_weight = 0.0_f32;

    for (vector, weight) in vectors {
        if vector.len() != dim {
            continue;
        }
        for (idx, value) in vector.iter().enumerate() {
            out[idx] += *value * weight;
        }
        total_weight += weight;
    }

    if total_weight <= f32::EPSILON {
        return Ok(None);
    }
    for value in &mut out {
        *value /= total_weight;
    }
    Ok(Some(out))
}

fn get_vector(
    catalog_store: &dyn CatalogStore,
    entity_type: &str,
    entity_id: &str,
    namespace: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    Ok(catalog_store
        .get_entity_embedding(entity_type, entity_id, namespace, true)?
        .and_then(|embedding| embedding.vector))
}

/// Smart continuation: pick the next tracks for a non-radio queue.
///
/// The query is anchored on the user-chosen source, optionally interpolated toward a
/// destination by `progress`, then blended with a small recency term. Server-appended
/// tracks must not be part of the source, otherwise the anchor random-walks away from
/// what the user chose.
fn continue_queue(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    request: ContinuationRequest,
) -> anyhow::Result<ContinuationResponse> {
    validate_continuation_request(&request)?;
    let count = request.count.unwrap_or(1).clamp(1, 10);
    let mode = request.mode.unwrap_or(RadioMode::Similar);
    let diversity = request
        .diversity
        .unwrap_or(DEFAULT_RADIO_DIVERSITY)
        .clamp(0.0, 1.0);
    let randomness = request
        .randomness
        .unwrap_or(DEFAULT_RADIO_RANDOMNESS)
        .clamp(0.0, 1.0);

    let mut exclude: HashSet<String> = request.exclude_track_ids.iter().cloned().collect();
    exclude.extend(request.context_track_ids.iter().cloned());
    exclude.extend(request.source_track_ids.iter().cloned());
    exclude.extend(request.recent_track_ids.iter().cloned());

    let is_legacy = request.source_track_ids.is_empty()
        && request.source_references.is_empty()
        && request.destination.is_empty();
    if is_legacy {
        let namespace = track_namespace(settings);
        let Some(seed) = weighted_track_vector(
            catalog_store,
            &namespace,
            &request.context_track_ids,
            CONTINUATION_CONTEXT_LIMIT,
        )?
        else {
            return Ok(ContinuationResponse {
                track_ids: Vec::new(),
                recency_weight: None,
                progress: None,
                namespaces: Vec::new(),
            });
        };
        let queries = [NamespaceQuery {
            namespace,
            weight: 1.0,
            vector: seed,
        }];
        let track_ids = select_continuation(
            catalog_store,
            &queries,
            mode,
            count,
            diversity,
            randomness,
            exclude,
            &[],
        )?;
        return Ok(ContinuationResponse {
            track_ids,
            recency_weight: None,
            progress: None,
            namespaces: Vec::new(),
        });
    }

    let criteria = if request.criteria.is_empty() {
        vec![NormalizedRadioCriterion {
            namespace: track_namespace(settings),
            weight: 1.0,
        }]
    } else {
        normalize_criteria_list(settings, request.criteria.clone())?
    };
    let recent = last_n(&request.recent_track_ids, CONTINUATION_CONTEXT_LIMIT);
    let recency_weight = request
        .recency_weight
        .unwrap_or(CONTINUATION_DEFAULT_RECENCY_WEIGHT)
        .clamp(0.0, 1.0);
    let progress = (!request.destination.is_empty())
        .then(|| request.progress.unwrap_or(0.0).clamp(0.0, 1.0));
    let source_ids = sample_evenly(&request.source_track_ids, CONTINUATION_SOURCE_SAMPLE);

    let mut queries = Vec::with_capacity(criteria.len());
    let mut namespaces = Vec::with_capacity(criteria.len());
    let mut recency_applied = false;
    for criterion in criteria {
        let namespace = criterion.namespace;
        let mut diagnostics = ContinuationNamespaceDiagnostics {
            namespace: namespace.clone(),
            weight: criterion.weight,
            source_to_destination: None,
            query_to_source: None,
            query_to_destination: None,
            destination_components: Vec::new(),
        };
        let from_tracks = normalised_mean_track_vector(catalog_store, &namespace, &source_ids)?;
        let from_references = normalised_reference_sum(
            catalog_store,
            settings,
            &request.source_references,
            &namespace,
        )?;
        let source = match (from_tracks, from_references) {
            (Some(tracks), Some(references)) => {
                blend(&tracks, 1.0, &references, 1.0).unwrap_or(tracks)
            }
            (Some(vector), None) | (None, Some(vector)) => vector,
            (None, None) => {
                namespaces.push(diagnostics);
                continue;
            }
        };

        // The destination mix: normalised weighted sum of unit component vectors.
        // Components without a vector in this namespace are skipped here.
        let mut components = Vec::with_capacity(request.destination.len());
        for reference in &request.destination {
            let vector = reference_vector(catalog_store, settings, reference, &namespace)?
                .and_then(normalised)
                .filter(|vector| vector.len() == source.len());
            components.push((reference, vector));
        }
        let destination = {
            let mut sum: Option<Vec<f32>> = None;
            for (reference, vector) in &components {
                let Some(vector) = vector else { continue };
                let weight = reference.weight.unwrap_or(1.0);
                match sum.as_mut() {
                    Some(sum) => {
                        let dim = sum.len();
                        add_scaled_vector(sum, vector, weight, dim);
                    }
                    None => sum = Some(vector.iter().map(|value| value * weight).collect()),
                }
            }
            sum.and_then(normalised)
        };
        let mut query = source.clone();
        if let (Some(destination), Some(progress)) = (&destination, progress) {
            query = blend(&source, 1.0 - progress, destination, progress)
                .unwrap_or_else(|| source.clone());
        }

        if recency_weight > 0.0 {
            if let Some(recent_vector) =
                normalised_mean_track_vector(catalog_store, &namespace, &recent)?
            {
                if let Some(blended) =
                    blend(&query, 1.0 - recency_weight, &recent_vector, recency_weight)
                {
                    query = blended;
                    recency_applied = true;
                }
            }
        }

        if !request.away.is_empty() {
            let mut pushed = query.clone();
            let dim = pushed.len();
            for reference in &request.away {
                if let Some(vector) = reference_vector(catalog_store, settings, reference, &namespace)?
                    .and_then(normalised)
                {
                    add_scaled_vector(&mut pushed, &vector, -reference.weight.unwrap_or(1.0), dim);
                }
            }
            if let Some(pushed) = normalised(pushed) {
                query = pushed;
            }
        }

        diagnostics.query_to_source = cosine(&query, &source);
        if let Some(destination) = &destination {
            diagnostics.source_to_destination = cosine(&source, destination);
            diagnostics.query_to_destination = cosine(&query, destination);
        }
        diagnostics.destination_components = components
            .iter()
            .map(|(reference, vector)| DestinationComponentDiagnostics {
                entity_type: reference.entity_type.clone(),
                entity_id: reference.entity_id.clone(),
                similarity: vector.as_ref().and_then(|vector| cosine(&query, vector)),
            })
            .collect();
        namespaces.push(diagnostics);
        queries.push(NamespaceQuery {
            namespace,
            weight: criterion.weight,
            vector: query,
        });
    }

    let track_ids = if queries.is_empty() {
        Vec::new()
    } else {
        select_continuation(
            catalog_store,
            &queries,
            mode,
            count,
            diversity,
            randomness,
            exclude,
            &recent,
        )?
    };
    Ok(ContinuationResponse {
        track_ids,
        recency_weight: Some(if recency_applied { recency_weight } else { 0.0 }),
        progress,
        namespaces,
    })
}

#[allow(clippy::too_many_arguments)]
fn select_continuation(
    catalog_store: &dyn CatalogStore,
    queries: &[NamespaceQuery],
    mode: RadioMode,
    count: usize,
    diversity: f32,
    randomness: f32,
    mut exclude: HashSet<String>,
    recent: &[String],
) -> anyhow::Result<Vec<String>> {
    let oversample = (count * 16).clamp(CONTINUATION_OVERSAMPLE_MIN, CONTINUATION_OVERSAMPLE_MAX);
    let candidates =
        score_radio_candidates(catalog_store, queries, mode, oversample, &exclude, true, None)?;
    let ranked = rank_radio_candidates(catalog_store, candidates, None, randomness)?;

    let mut result = Vec::with_capacity(count + recent.len());
    let mut album_cooldowns = HashMap::new();
    let mut artist_cooldowns = HashMap::new();
    let mut artist_last_positions = HashMap::new();
    let prefix = {
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        initialize_radio_history(catalog_store, recent, &mut selection)?;
        let prefix = selection.result.len();
        select_radio_candidates(ranked, count + prefix, diversity, &mut selection);
        prefix
    };
    result.drain(..prefix);
    Ok(result)
}

fn validate_continuation_request(request: &ContinuationRequest) -> anyhow::Result<()> {
    if request.source_track_ids.len() > CONTINUATION_SOURCE_TRACKS_MAX {
        return Err(invalid_request!(
            "invalid continuation request: at most {CONTINUATION_SOURCE_TRACKS_MAX} source_track_ids are allowed"
        ));
    }
    if request.criteria.len() > RADIO_MAX_REFERENCES {
        return Err(invalid_request!(
            "invalid continuation request: at most {RADIO_MAX_REFERENCES} criteria are allowed"
        ));
    }
    validate_reference_list(&request.source_references, "source_references")?;
    validate_reference_list(&request.away, "away")?;
    validate_reference_list(&request.destination, "destination")?;
    for (field, value) in [
        ("recency_weight", request.recency_weight),
        ("progress", request.progress),
        ("diversity", request.diversity),
        ("randomness", request.randomness),
    ] {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(invalid_request!(
                "invalid continuation request: {field} must be a finite number"
            ));
        }
    }
    Ok(())
}

fn validate_reference_list(references: &[RadioReference], field: &str) -> anyhow::Result<()> {
    if references.len() > RADIO_MAX_REFERENCES {
        return Err(invalid_request!(
            "invalid continuation request: at most {RADIO_MAX_REFERENCES} {field} are allowed"
        ));
    }
    for reference in references {
        if !matches!(
            reference.entity_type.as_str(),
            "track" | "album" | "artist" | "concept"
        ) {
            return Err(invalid_request!(
                "invalid continuation request: unsupported {field} entity_type '{}'",
                reference.entity_type
            ));
        }
        if reference
            .weight
            .is_some_and(|weight| !weight.is_finite() || weight <= 0.0)
        {
            return Err(invalid_request!(
                "invalid continuation request: {field} weights must be positive"
            ));
        }
    }
    Ok(())
}

fn last_n(ids: &[String], n: usize) -> Vec<String> {
    ids[ids.len().saturating_sub(n)..].to_vec()
}

/// Deduplicate preserving order, then take an evenly strided, deterministic sample so a
/// long playlist is represented end to end and the anchor is stable between calls.
fn sample_evenly(ids: &[String], max: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let unique = ids
        .iter()
        .filter(|id| seen.insert(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unique.len() <= max {
        return unique;
    }
    (0..max)
        .map(|index| unique[index * unique.len() / max].clone())
        .collect()
}

fn normalised(mut vector: Vec<f32>) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return None;
    }
    for value in &mut vector {
        *value /= norm;
    }
    Some(vector)
}

fn cosine(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    let denominator = left_norm * right_norm;
    if !denominator.is_finite() || denominator <= f32::EPSILON {
        return None;
    }
    Some(dot / denominator)
}

/// `normalise(left * left_weight + right * right_weight)`; `None` on dimension mismatch
/// or when the two cancel out.
fn blend(left: &[f32], left_weight: f32, right: &[f32], right_weight: f32) -> Option<Vec<f32>> {
    if left.len() != right.len() {
        return None;
    }
    normalised(
        left.iter()
            .zip(right)
            .map(|(a, b)| a * left_weight + b * right_weight)
            .collect(),
    )
}

/// Mean of unit-length track vectors, so a few loud embeddings do not dominate.
fn normalised_mean_track_vector(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_ids: &[String],
) -> anyhow::Result<Option<Vec<f32>>> {
    let mut vectors = Vec::new();
    for track_id in track_ids {
        if let Some(vector) =
            get_vector(catalog_store, "track", track_id, namespace)?.and_then(normalised)
        {
            vectors.push((vector, 1.0));
        }
    }
    Ok(weighted_mean(vectors)?.and_then(normalised))
}

/// Weighted sum of unit-length reference vectors. Normalising each one first removes the
/// scale difference between track means and derived album medians.
fn normalised_reference_sum(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    references: &[RadioReference],
    namespace: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    let mut sum: Option<Vec<f32>> = None;
    for reference in references {
        let Some(vector) =
            reference_vector(catalog_store, settings, reference, namespace)?.and_then(normalised)
        else {
            continue;
        };
        let weight = reference.weight.unwrap_or(1.0);
        match sum.as_mut() {
            Some(sum) => {
                let dim = sum.len();
                add_scaled_vector(sum, &vector, weight, dim);
            }
            None => sum = Some(vector.into_iter().map(|value| value * weight).collect()),
        }
    }
    Ok(sum.and_then(normalised))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::{
        Album, AlbumAvailability, AlbumType, Artist, ArtistRole, Track, TrackArtist,
    };

    fn resolved_track(track_id: &str, album_id: &str, artist_ids: &[&str]) -> ResolvedTrack {
        ResolvedTrack {
            track: Track {
                id: track_id.to_string(),
                name: track_id.to_string(),
                album_id: album_id.to_string(),
                disc_number: 1,
                track_number: 1,
                duration_ms: 180_000,
                explicit: false,
                popularity: 50,
                language: None,
                external_id_isrc: None,
                audio_uri: Some(format!("{track_id}.ogg")),
                availability: TrackAvailability::Available,
            },
            album: Album {
                id: album_id.to_string(),
                name: album_id.to_string(),
                album_type: AlbumType::Album,
                label: None,
                release_date: Some("2024".to_string()),
                release_date_precision: Some("year".to_string()),
                external_id_upc: None,
                popularity: 50,
                album_availability: AlbumAvailability::Complete,
            },
            artists: artist_ids
                .iter()
                .map(|artist_id| TrackArtist {
                    artist: Artist {
                        id: (*artist_id).to_string(),
                        name: (*artist_id).to_string(),
                        genres: Vec::new(),
                        followers_total: 0,
                        popularity: 50,
                        available: true,
                    },
                    role: ArtistRole::MainArtist,
                })
                .collect(),
        }
    }

    #[test]
    fn radio_selection_keeps_track_ids_unique() {
        let track = resolved_track("track-a", "album-a", &["artist-a"]);
        let ranked = vec![
            RankedRadioCandidate {
                track_id: "track-a".to_string(),
                resolved: track.clone(),
                score: 1.0,
            },
            RankedRadioCandidate {
                track_id: "track-a".to_string(),
                resolved: track,
                score: 0.9,
            },
        ];
        let mut result = Vec::new();
        let mut exclude = HashSet::new();
        let mut album_cooldowns = HashMap::new();
        let mut artist_cooldowns = HashMap::new();
        let mut artist_last_positions = HashMap::new();

        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        select_radio_candidates(ranked, 2, 1.0, &mut selection);

        assert_eq!(result, vec!["track-a"]);
    }

    #[test]
    fn radio_selection_penalizes_immediate_same_artist_and_album() {
        let seed = resolved_track("seed", "album-a", &["artist-a"]);
        let same_artist_album = resolved_track("same", "album-a", &["artist-a"]);
        let different_artist_album = resolved_track("different", "album-b", &["artist-b"]);
        let mut album_cooldowns = HashMap::new();
        let mut artist_cooldowns = HashMap::new();
        apply_track_cooldown(&seed, &mut album_cooldowns, &mut artist_cooldowns);

        let ranked = vec![
            RankedRadioCandidate {
                track_id: "same".to_string(),
                resolved: same_artist_album,
                score: 0.95,
            },
            RankedRadioCandidate {
                track_id: "different".to_string(),
                resolved: different_artist_album,
                score: 0.84,
            },
        ];
        let mut result = Vec::new();
        let mut exclude = HashSet::new();
        let mut artist_last_positions = HashMap::new();

        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        select_radio_candidates(ranked, 1, 1.0, &mut selection);

        assert_eq!(result, vec!["different"]);
    }

    #[test]
    fn radio_selection_forbids_same_artist_until_fifth_track() {
        let seed = resolved_track("seed", "album-a", &["artist-a"]);
        let same_artist = resolved_track("same", "album-b", &["artist-a"]);
        let other_one = resolved_track("other-1", "album-c", &["artist-b"]);
        let other_two = resolved_track("other-2", "album-d", &["artist-c"]);
        let other_three = resolved_track("other-3", "album-e", &["artist-d"]);
        let ranked = vec![
            RankedRadioCandidate {
                track_id: "same".to_string(),
                resolved: same_artist,
                score: 10.0,
            },
            RankedRadioCandidate {
                track_id: "other-1".to_string(),
                resolved: other_one,
                score: 0.9,
            },
            RankedRadioCandidate {
                track_id: "other-2".to_string(),
                resolved: other_two,
                score: 0.8,
            },
            RankedRadioCandidate {
                track_id: "other-3".to_string(),
                resolved: other_three,
                score: 0.7,
            },
        ];
        let mut result = Vec::new();
        let mut exclude = HashSet::new();
        let mut album_cooldowns = HashMap::new();
        let mut artist_cooldowns = HashMap::new();
        let mut artist_last_positions = HashMap::new();

        exclude.insert("seed".to_string());
        let mut selection = RadioSelectionState {
            result: &mut result,
            exclude: &mut exclude,
            album_cooldowns: &mut album_cooldowns,
            artist_cooldowns: &mut artist_cooldowns,
            artist_last_positions: &mut artist_last_positions,
        };
        push_radio_result("seed".to_string(), &seed, &mut selection);
        select_radio_candidates(ranked, 5, 1.0, &mut selection);

        assert_eq!(
            result,
            vec!["seed", "other-1", "other-2", "other-3", "same"]
        );
    }

    #[test]
    fn radio_cooldown_decays_after_later_tracks() {
        let first = resolved_track("first", "album-a", &["artist-a"]);
        let second = resolved_track("second", "album-b", &["artist-b"]);
        let third = resolved_track("third", "album-c", &["artist-c"]);
        let mut album_cooldowns = HashMap::new();
        let mut artist_cooldowns = HashMap::new();

        apply_track_cooldown(&first, &mut album_cooldowns, &mut artist_cooldowns);
        apply_track_cooldown(&second, &mut album_cooldowns, &mut artist_cooldowns);
        apply_track_cooldown(&third, &mut album_cooldowns, &mut artist_cooldowns);

        assert!((album_cooldowns["album-a"] - 0.4225).abs() < f32::EPSILON);
        assert!((artist_cooldowns["artist-a"] - 0.4225).abs() < f32::EPSILON);
        assert!((album_cooldowns["album-b"] - 0.65).abs() < f32::EPSILON);
        assert!((artist_cooldowns["artist-c"] - 1.0).abs() < f32::EPSILON);
    }

    fn embedding(
        entity_type: &str,
        id: &str,
        namespace: &str,
        vector: Vec<f32>,
    ) -> crate::catalog_store::EntityEmbeddingUpsert {
        crate::catalog_store::EntityEmbeddingUpsert {
            entity_type: entity_type.into(),
            entity_id: id.into(),
            namespace: namespace.into(),
            vector,
            dtype: "float32".into(),
            metadata: serde_json::json!({}),
            model: serde_json::json!({}),
        }
    }

    #[test]
    fn album_radio_builder_handles_seed_inclusion_zero_randomness_and_small_catalogs() {
        use crate::catalog_store::SqliteCatalogStore;
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteCatalogStore::new(
            dir.path().join("catalog.db"),
            dir.path(),
            2,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        let mut first = resolved_track("seed-1", "album-seed", &["artist"]);
        first.artists[0].artist.genres = vec!["rock".into()];
        store.create_artist(&first.artists[0].artist).unwrap();
        store
            .create_album(&first.album, &["artist".into()])
            .unwrap();
        let mut other = resolved_track("other", "album-other", &["artist"]);
        other.track.popularity = 100;
        store
            .create_album(&other.album, &["artist".into()])
            .unwrap();
        for track in [
            first,
            resolved_track("seed-2", "album-seed", &["artist"]),
            other,
        ] {
            store
                .create_track(&track.track, &["artist".into()])
                .unwrap();
            store
                .set_track_audio_uri(&track.track.id, "audio.ogg")
                .unwrap();
            store
                .upsert_entity_embedding(&embedding(
                    "track",
                    &track.track.id,
                    DEFAULT_TRACK_NAMESPACE,
                    if track.track.id == "other" {
                        vec![0.7, 0.7]
                    } else {
                        vec![1.0, 0.0]
                    },
                ))
                .unwrap();
        }
        let request = |include| {
            serde_json::from_value::<RadioBuildRequest>(serde_json::json!({
                "seed": {"entity_type": "album", "entity_id": "album-seed"},
                "count": 10, "randomness": 0, "include_seed_tracks": include,
            }))
            .unwrap()
        };
        let included = build_radio(&store, None, request(true)).unwrap();
        assert_eq!(included.len(), 3);
        assert_eq!(&included[..2], &["seed-1", "seed-2"]);
        assert_eq!(
            build_radio(&store, None, request(false)).unwrap(),
            vec!["other"]
        );
        assert_eq!(
            album_radio(
                &store,
                DEFAULT_TRACK_NAMESPACE,
                DEFAULT_ALBUM_NAMESPACE,
                "album-seed",
                10
            )
            .unwrap(),
            vec!["other"]
        );

        let filtered = continuation_candidates(
            &store,
            DEFAULT_TRACK_NAMESPACE,
            &[1.0, 0.0],
            1,
            &HashSet::new(),
            Some(&RadioFilters {
                popularity_min: Some(90),
                ..Default::default()
            }),
        )
        .unwrap();
        assert_eq!(
            filtered
                .iter()
                .map(|r| r.entity_id.as_str())
                .collect::<Vec<_>>(),
            vec!["other"]
        );
        let genre_request = |excluded: Vec<&str>| {
            serde_json::from_value::<RadioContinuationRequest>(serde_json::json!({
                "source": "genre", "seed": {"entity_type":"genre", "entity_id":"rock"},
                "exclude_track_ids": excluded, "count": 10,
            }))
            .unwrap()
        };
        assert_eq!(
            continue_radio(&store, None, genre_request(vec!["seed-1", "seed-2"])).unwrap(),
            vec!["other"]
        );
        assert!(continue_radio(
            &store,
            None,
            genre_request(vec!["seed-1", "seed-2", "other"])
        )
        .unwrap()
        .is_empty());
        for entity_type in ["track", "artist"] {
            let entity_id = if entity_type == "track" {
                "seed-1"
            } else {
                "artist"
            };
            let request = serde_json::from_value::<RadioContinuationRequest>(serde_json::json!({
                "source":"basic", "seed":{"entity_type":entity_type,"entity_id":entity_id},
                "exclude_track_ids":["seed-1","seed-2"], "count":10,
            }))
            .unwrap();
            assert_eq!(
                continue_radio(&store, None, request).unwrap(),
                vec!["other"]
            );
        }

        // Continuation keeps the original seed and excludes earlier batches, including seeds.
        for source in ["basic", "custom"] {
            let continuing = |excluded: Vec<&str>| {
                serde_json::from_value::<RadioContinuationRequest>(serde_json::json!({
                "source": source,
                "seed": {"entity_type": "album", "entity_id": "album-seed"},
                "settings": {"seed": {"entity_type": "album", "entity_id": "album-seed"}, "include_seed_tracks": false, "randomness": 0},
                "context_track_ids": ["seed-1"], "exclude_track_ids": excluded, "count": 10,
            })).unwrap()
            };
            assert_eq!(
                continue_radio(&store, None, continuing(vec!["seed-1"])).unwrap(),
                vec!["other"]
            );
            assert!(
                continue_radio(&store, None, continuing(vec!["seed-1", "other"]))
                    .unwrap()
                    .is_empty()
            );
        }
        let included_then_continued = build_radio_with_history(
            &store,
            None,
            request(true),
            ["seed-1".into(), "seed-2".into()].into_iter().collect(),
            &["seed-1".into(), "seed-2".into()],
        )
        .unwrap();
        assert_eq!(included_then_continued, vec!["other"]);

        // An unavailable nearest neighbour must not consume the result limit.
        store
            .upsert_entity_embedding(&embedding(
                "track",
                "missing",
                DEFAULT_TRACK_NAMESPACE,
                vec![1.0, 0.0],
            ))
            .unwrap();
        store.clear_track_audio_uri("seed-1").unwrap();
        store.clear_track_audio_uri("seed-2").unwrap();
        let available = store
            .search_available_track_embeddings(DEFAULT_TRACK_NAMESPACE, &[1.0, 0.0], 1)
            .unwrap();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].entity_id, "other");

        let settings = AudioEmbeddingsSettings {
            enabled: true,
            simple_ai_base_url: String::new(),
            api_key: String::new(),
            interval_hours: 24,
            jitter_minutes: 0,
            max_tracks_per_run: 10,
            request_timeout_secs: 60,
            specs: vec![crate::config::AudioEmbeddingSpec {
                model: "test".into(),
                namespace: DEFAULT_TRACK_NAMESPACE.into(),
                serve: true,
            }],
            album_derivations: crate::config::AlbumEmbeddingDerivationsSettings {
                enabled: true,
                interval_hours: 24,
                jitter_minutes: 0,
                max_albums_per_run: 10,
                specs: vec![crate::config::AlbumEmbeddingDerivationSpec {
                    source_namespace: DEFAULT_TRACK_NAMESPACE.into(),
                    target_namespace: "custom.album".into(),
                    aggregation: crate::config::AlbumEmbeddingAggregation::Median,
                }],
            },
            concepts: Default::default(),
        };
        store
            .upsert_entity_embedding(&embedding(
                "album",
                "album-seed",
                "custom.album",
                vec![0.0, 1.0],
            ))
            .unwrap();
        let namespace = album_namespace_for_track_namespace_with_settings(
            Some(&settings),
            DEFAULT_TRACK_NAMESPACE,
        );
        assert_eq!(namespace, "custom.album");
        // Only the configured album embedding remains: both entry points must use it.
        assert_eq!(
            album_radio(
                &store,
                DEFAULT_TRACK_NAMESPACE,
                &namespace,
                "album-seed",
                10
            )
            .unwrap(),
            vec!["other"]
        );
        assert_eq!(
            build_radio(&store, Some(&settings), request(false)).unwrap(),
            vec!["other"]
        );
    }

    #[test]
    fn excessive_radio_inputs_are_rejected_before_search() {
        let request = serde_json::from_value::<RadioBuildRequest>(serde_json::json!({
            "seed": {"entity_type": "album", "entity_id": "seed"},
            "criteria": vec![serde_json::json!({"namespace": DEFAULT_TRACK_NAMESPACE, "weight": 1}); 9],
        })).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = crate::catalog_store::SqliteCatalogStore::new(
            dir.path().join("catalog.db"),
            dir.path(),
            2,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        assert!(build_radio(&store, None, request)
            .unwrap_err()
            .to_string()
            .contains("at most 8"));
    }

    // ---- smart continuation ----

    /// a1 [1,0], a2 [0.95,0.31] (artist-a); b1 [0,1], b2 [0.31,0.95] (artist-b);
    /// c1 [0.707,0.707] (artist-c). Each artist has its own album.
    fn continuation_store() -> (tempfile::TempDir, crate::catalog_store::SqliteCatalogStore) {
        use crate::catalog_store::SqliteCatalogStore;
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteCatalogStore::new(
            dir.path().join("catalog.db"),
            dir.path(),
            2,
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        let tracks: [(&str, &str, &str, [f32; 2]); 5] = [
            ("a1", "album-a", "artist-a", [1.0, 0.0]),
            ("a2", "album-a", "artist-a", [0.95, 0.31]),
            ("b1", "album-b", "artist-b", [0.0, 1.0]),
            ("b2", "album-b", "artist-b", [0.31, 0.95]),
            ("c1", "album-c", "artist-c", [0.707, 0.707]),
        ];
        let mut created_albums = HashSet::new();
        for (track_id, album_id, artist_id, vector) in tracks {
            let resolved = resolved_track(track_id, album_id, &[artist_id]);
            if created_albums.insert(album_id) {
                store.create_artist(&resolved.artists[0].artist).unwrap();
                store
                    .create_album(&resolved.album, &[artist_id.into()])
                    .unwrap();
            }
            store
                .create_track(&resolved.track, &[artist_id.into()])
                .unwrap();
            store
                .set_track_audio_uri(track_id, "audio.ogg")
                .unwrap();
            store
                .upsert_entity_embedding(&embedding(
                    "track",
                    track_id,
                    DEFAULT_TRACK_NAMESPACE,
                    vector.to_vec(),
                ))
                .unwrap();
        }
        (dir, store)
    }

    fn continuation(value: serde_json::Value) -> ContinuationRequest {
        let mut value = value;
        value
            .as_object_mut()
            .unwrap()
            .entry("randomness")
            .or_insert(serde_json::json!(0));
        serde_json::from_value(value).unwrap()
    }

    fn assert_close(actual: Option<f32>, expected: f32) {
        let actual = actual.expect("expected a value");
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn continuation_anchors_on_source_not_appended_context() {
        let (_dir, store) = continuation_store();
        let anchored = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "recent_track_ids": ["b1"],
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(anchored.track_ids, vec!["a2"]);
        assert_eq!(anchored.namespaces.len(), 1);
        assert_close(anchored.namespaces[0].query_to_source, 1.0);

        // The legacy decayed centroid of [a1, b1] has drifted halfway to b1.
        let legacy = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({ "context_track_ids": ["a1", "b1"] })),
        )
        .unwrap();
        assert_eq!(legacy.track_ids, vec!["c1"]);
        assert_eq!(legacy.recency_weight, None);
        assert!(legacy.namespaces.is_empty());
    }

    #[test]
    fn continuation_recency_weight_blends_toward_recent() {
        let (_dir, store) = continuation_store();
        let blended = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "recent_track_ids": ["b1"],
                "recency_weight": 0.5,
            })),
        )
        .unwrap();
        assert_eq!(blended.track_ids, vec!["c1"]);
        assert_eq!(blended.recency_weight, Some(0.5));

        let without_recent = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({ "source_track_ids": ["a1"] })),
        )
        .unwrap();
        assert_eq!(without_recent.track_ids, vec!["a2"]);
        assert_eq!(without_recent.recency_weight, Some(0.0));
    }

    #[test]
    fn continuation_normalises_reference_scales() {
        let (_dir, store) = continuation_store();
        // A derived album vector with a much larger magnitude than the track vectors.
        store
            .upsert_entity_embedding(&embedding(
                "album",
                "album-b",
                DEFAULT_ALBUM_NAMESPACE,
                vec![0.0, 50.0],
            ))
            .unwrap();
        let response = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_references": [
                    {"entity_type": "track", "entity_id": "a1"},
                    {"entity_type": "album", "entity_id": "album-b"},
                ],
                "exclude_track_ids": ["a1", "b1"],
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(response.track_ids, vec!["c1"]);
    }

    #[test]
    fn continuation_diversity_defers_recent_artist() {
        let (_dir, store) = continuation_store();
        let response = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "recent_track_ids": ["a1"],
                "count": 2,
            })),
        )
        .unwrap();
        assert_eq!(response.track_ids, vec!["c1", "b2"]);
    }

    #[test]
    fn continuation_destination_progress_lerps_query() {
        let (_dir, store) = continuation_store();
        let at = |progress: f32| {
            continue_queue(
                &store,
                None,
                continuation(serde_json::json!({
                    "source_track_ids": ["a1"],
                    "destination": {"entity_type": "track", "entity_id": "b1"},
                    "progress": progress,
                    "recency_weight": 0,
                })),
            )
            .unwrap()
        };
        assert_eq!(at(0.0).track_ids, vec!["a2"]);
        let halfway = at(0.5);
        assert_eq!(halfway.track_ids, vec!["c1"]);
        assert_eq!(halfway.progress, Some(0.5));
        let diagnostics = &halfway.namespaces[0];
        assert_close(diagnostics.source_to_destination, 0.0);
        assert_close(diagnostics.query_to_source, 0.707);
        assert_close(diagnostics.query_to_destination, 0.707);
        let arrived = at(1.0);
        assert_eq!(arrived.track_ids, vec!["b1"]);
        assert_close(arrived.namespaces[0].query_to_destination, 1.0);
    }

    #[test]
    fn continuation_zero_vector_guards_fall_back_to_source() {
        let (_dir, store) = continuation_store();
        let antipode = resolved_track("anti", "album-a", &["artist-a"]);
        store
            .create_track(&antipode.track, &["artist-a".into()])
            .unwrap();
        store
            .upsert_entity_embedding(&embedding(
                "track",
                "anti",
                DEFAULT_TRACK_NAMESPACE,
                vec![-1.0, 0.0],
            ))
            .unwrap();
        let cancelled = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "destination": {"entity_type": "track", "entity_id": "anti"},
                "progress": 0.5,
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(cancelled.track_ids, vec!["a2"]);

        let pushed_to_zero = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "away": [{"entity_type": "track", "entity_id": "a1"}],
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(pushed_to_zero.track_ids, vec!["a2"]);
    }

    #[test]
    fn continuation_rejects_invalid_requests() {
        let (_dir, store) = continuation_store();
        let reference = serde_json::json!({"entity_type": "track", "entity_id": "a1"});
        let invalid = [
            serde_json::json!({"source_track_ids": ["a1"], "away": vec![reference.clone(); 9]}),
            serde_json::json!({"source_references": vec![reference.clone(); 9]}),
            serde_json::json!({"destination": {"entity_type": "playlist", "entity_id": "p"}}),
            serde_json::json!({"source_references": [{"entity_type": "track", "entity_id": "a1", "weight": 0}]}),
            serde_json::json!({"source_track_ids": vec!["a1"; 201]}),
            serde_json::json!({"source_track_ids": ["a1"], "criteria": [{"namespace": "unknown.v1", "weight": 1}]}),
        ];
        for body in invalid {
            let err = continue_queue(&store, None, continuation(body.clone())).unwrap_err();
            assert!(is_invalid_request(&err), "{body} -> {err}");
        }
        let mut nan = continuation(serde_json::json!({"source_track_ids": ["a1"]}));
        nan.recency_weight = Some(f32::NAN);
        assert!(is_invalid_request(
            &continue_queue(&store, None, nan).unwrap_err()
        ));
    }

    #[test]
    fn continuation_legacy_context_only_path_is_unchanged() {
        let (_dir, store) = continuation_store();
        let response = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "context_track_ids": ["a1"],
                "exclude_track_ids": ["a1"],
            })),
        )
        .unwrap();
        assert_eq!(response.track_ids, vec!["a2"]);
        assert_eq!(response.recency_weight, None);
        assert_eq!(response.progress, None);
        assert!(response.namespaces.is_empty());

        let empty = continue_queue(&store, None, continuation(serde_json::json!({}))).unwrap();
        assert!(empty.track_ids.is_empty());
    }

    #[test]
    fn continuation_source_sampling_is_even_and_deterministic() {
        let ids = (0..200).map(|index| index.to_string()).collect::<Vec<_>>();
        let sample = sample_evenly(&ids, 64);
        assert_eq!(sample.len(), 64);
        assert_eq!(sample[0], "0");
        assert_eq!(sample, sample_evenly(&ids, 64));
        let duplicated = vec!["x".to_string(), "x".to_string(), "y".to_string()];
        assert_eq!(sample_evenly(&duplicated, 64), vec!["x", "y"]);
    }

    #[test]
    fn unserved_namespaces_are_hidden_and_v2_ast_is_preferred() {
        let spec = |namespace: &str, serve: bool| crate::config::AudioEmbeddingSpec {
            model: "m".into(),
            namespace: namespace.into(),
            serve,
        };
        let settings = |specs| AudioEmbeddingsSettings {
            enabled: true,
            simple_ai_base_url: String::new(),
            api_key: String::new(),
            interval_hours: 24,
            jitter_minutes: 0,
            max_tracks_per_run: 10,
            request_timeout_secs: 60,
            specs,
            album_derivations: crate::config::AlbumEmbeddingDerivationsSettings {
                enabled: false,
                interval_hours: 24,
                jitter_minutes: 0,
                max_albums_per_run: 10,
                specs: Vec::new(),
            },
            concepts: Default::default(),
        };
        let audio_scene = |settings: &AudioEmbeddingsSettings| {
            radio_recipes_for_namespaces(&available_track_namespaces(Some(settings)))
                .into_iter()
                .find(|recipe| recipe.id == "audio_scene")
                .unwrap()
                .criteria[0]
                .namespace
                .clone()
        };

        // Backfilling: v2 is computed but not served yet.
        let backfilling = settings(vec![
            spec(DEFAULT_TRACK_NAMESPACE, true),
            spec("ast.audioset.v1", true),
            spec("ast.audioset.v2", false),
        ]);
        assert_eq!(
            available_track_namespaces(Some(&backfilling)),
            vec![DEFAULT_TRACK_NAMESPACE, "ast.audioset.v1"]
        );
        assert_eq!(audio_scene(&backfilling), "ast.audioset.v1");

        // Switched over: v2 wins even while v1 is still configured.
        let switched = settings(vec![
            spec(DEFAULT_TRACK_NAMESPACE, true),
            spec("ast.audioset.v1", true),
            spec("ast.audioset.v2", true),
        ]);
        assert_eq!(audio_scene(&switched), "ast.audioset.v2");
        assert_eq!(
            album_namespace_for_track_namespace("ast.audioset.v2"),
            "album.ast.median.v2"
        );

        // The default track namespace is never an unserved one.
        let unserved_default = settings(vec![
            spec(DEFAULT_TRACK_NAMESPACE, false),
            spec("ast.audioset.v2", true),
        ]);
        assert_eq!(track_namespace(Some(&unserved_default)), "ast.audioset.v2");
    }

    #[test]
    fn continuation_destination_accepts_concepts_and_weighted_mixes() {
        let (_dir, store) = continuation_store();
        store
            .upsert_entity_embedding(&embedding(
                "concept",
                "audioset:Jazz",
                DEFAULT_TRACK_NAMESPACE,
                vec![0.0, 1.0],
            ))
            .unwrap();
        // A single concept behaves like any reference.
        let to_concept = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["a1"],
                "destination": [{"entity_type": "concept", "entity_id": "audioset:Jazz"}],
                "progress": 1.0,
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(to_concept.track_ids, vec!["b1"]);

        // An equal mix of a1 [1,0] and the concept [0,1] points at c1; the unknown
        // concept is skipped and reported without a similarity.
        let mix = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["b2"],
                "destination": [
                    {"entity_type": "track", "entity_id": "a1"},
                    {"entity_type": "concept", "entity_id": "audioset:Jazz"},
                    {"entity_type": "concept", "entity_id": "audioset:Missing"}
                ],
                "progress": 1.0,
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(mix.track_ids, vec!["c1"]);
        let components = &mix.namespaces[0].destination_components;
        assert_eq!(components.len(), 3);
        assert_close(components[0].similarity, 0.707);
        assert_close(components[1].similarity, 0.707);
        assert_eq!(components[2].similarity, None);

        // Weights shift the mix toward a component.
        let weighted = continue_queue(
            &store,
            None,
            continuation(serde_json::json!({
                "source_track_ids": ["b2"],
                "destination": [
                    {"entity_type": "track", "entity_id": "a1", "weight": 4.0},
                    {"entity_type": "concept", "entity_id": "audioset:Jazz", "weight": 1.0}
                ],
                "progress": 1.0,
                "recency_weight": 0,
            })),
        )
        .unwrap();
        assert_eq!(weighted.track_ids, vec!["a2"]);
    }

    #[test]
    fn continuation_destination_still_accepts_a_single_object() {
        let request = continuation(serde_json::json!({
            "destination": {"entity_type": "track", "entity_id": "b1"},
        }));
        assert_eq!(request.destination.len(), 1);
        let request = continuation(serde_json::json!({ "destination": null }));
        assert!(request.destination.is_empty());
        let mut too_many = continuation(serde_json::json!({ "source_track_ids": ["a1"] }));
        too_many.destination = (0..9)
            .map(|index| RadioReference {
                entity_type: "concept".into(),
                entity_id: format!("c{index}"),
                weight: None,
            })
            .collect();
        let (_dir, store) = continuation_store();
        assert!(is_invalid_request(
            &continue_queue(&store, None, too_many).unwrap_err()
        ));
    }

    #[test]
    fn concepts_are_listed_once_filtered_and_ordered() {
        let meta = |family: &str, label: &str, count: u64| {
            serde_json::json!({"family": family, "label": label, "example_count": count})
        };
        let rows = vec![
            ("composed:1810s".to_string(), "ns.b".to_string(), meta("composed", "Composed in the 1810s", 40)),
            ("composed:990s".to_string(), "ns.b".to_string(), meta("composed", "Composed in the 990s", 31)),
            ("audioset:Jazz".to_string(), "ns.b".to_string(), meta("sound_genre", "Jazz", 1)),
            ("audioset:Jazz".to_string(), "ns.a".to_string(), meta("sound_genre", "Jazz", 200)),
            ("audioset:Piano".to_string(), "ns.a".to_string(), meta("instrument", "Piano", 150)),
            ("genre:jazz fusion".to_string(), "ns.a".to_string(), meta("genre_tag", "jazz fusion", 60)),
        ];
        let all = summarize_concepts(
            rows.clone(),
            "ns.a",
            &ConceptsQuery { q: None, family: None, limit: None },
        );
        assert_eq!(
            all.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["audioset:Jazz", "audioset:Piano", "genre:jazz fusion", "composed:990s", "composed:1810s"]
        );
        // Metadata comes from the preferred namespace.
        assert_eq!(all[0].example_count, 200);
        let jazz = summarize_concepts(
            rows.clone(),
            "ns.a",
            &ConceptsQuery { q: Some(" JAZZ ".into()), family: None, limit: None },
        );
        assert_eq!(jazz.len(), 2);
        let tags = summarize_concepts(
            rows,
            "ns.a",
            &ConceptsQuery { q: None, family: Some("genre_tag".into()), limit: Some(1) },
        );
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].label, "jazz fusion");
    }
}
