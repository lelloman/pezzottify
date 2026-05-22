//! Recommendation routes for smart continuation and radio playback.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::catalog_store::{CatalogStore, ResolvedTrack, TrackAvailability};
use crate::config::AudioEmbeddingsSettings;

use super::session::Session;
use super::state::ServerState;

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

#[derive(Debug, Deserialize)]
struct ContinuationRequest {
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

pub fn recommendation_routes() -> Router<ServerState> {
    Router::new()
        .route(
            "/recommendations/continuation",
            post(post_continuation_recommendations),
        )
        .route("/radio/options", get(get_radio_options))
        .route("/radio/build", post(post_radio_build))
        .route("/radio/{entity_type}/{entity_id}", get(get_radio))
}

async fn post_continuation_recommendations(
    _session: Session,
    State(state): State<ServerState>,
    Json(body): Json<ContinuationRequest>,
) -> Response {
    let count = body.count.unwrap_or(1).clamp(1, 10);
    let catalog_store = Arc::clone(&state.catalog_store);
    let namespace = track_namespace(state.config.audio_embeddings.as_ref());

    match tokio::task::spawn_blocking(move || {
        let seed = weighted_track_vector(
            catalog_store.as_ref(),
            &namespace,
            &body.context_track_ids,
            CONTINUATION_CONTEXT_LIMIT,
        )?;
        let Some(seed) = seed else {
            return Ok(Vec::new());
        };

        let exclude = body.exclude_track_ids.into_iter().collect::<HashSet<_>>();
        recommend_tracks(catalog_store.as_ref(), &namespace, &seed, count, &exclude)
    })
    .await
    {
        Ok(Ok(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(Err(err)) => {
            error!("Error generating continuation recommendations: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate recommendations".to_string(),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
            .into_response(),
    }
}

async fn get_radio(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(query): Query<RadioQuery>,
) -> Response {
    let count = query.count.unwrap_or(50).clamp(1, 200);
    let catalog_store = Arc::clone(&state.catalog_store);
    let namespace = track_namespace(state.config.audio_embeddings.as_ref());
    let album_namespace = album_namespace_for_track_namespace(&namespace);

    match tokio::task::spawn_blocking(move || match entity_type.as_str() {
        "track" => track_radio(catalog_store.as_ref(), &namespace, &entity_id, count),
        "album" => album_radio(
            catalog_store.as_ref(),
            &namespace,
            &album_namespace,
            &entity_id,
            count,
        ),
        "artist" => artist_radio(catalog_store.as_ref(), &namespace, &entity_id, count),
        _ => Err(anyhow::anyhow!(
            "unsupported radio entity_type '{}'",
            entity_type
        )),
    })
    .await
    {
        Ok(Ok(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(Err(err)) => {
            let status = if err.to_string().contains("unsupported radio entity_type") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error!("Error generating radio: {}", err);
            (status, err.to_string()).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
            .into_response(),
    }
}

async fn get_radio_options(_session: Session, State(state): State<ServerState>) -> Response {
    no_store_json(radio_options_response(
        state.config.audio_embeddings.as_ref(),
    ))
}

async fn post_radio_build(
    _session: Session,
    State(state): State<ServerState>,
    Json(body): Json<RadioBuildRequest>,
) -> Response {
    let catalog_store = Arc::clone(&state.catalog_store);
    let settings = state.config.audio_embeddings.clone();

    match tokio::task::spawn_blocking(move || {
        build_radio(catalog_store.as_ref(), settings.as_ref(), body)
    })
    .await
    {
        Ok(Ok(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(Err(err)) => {
            let status = if err.to_string().starts_with("invalid radio request:") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error!("Error building advanced radio: {}", err);
            (status, err.to_string()).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
            .into_response(),
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
    settings
        .specs
        .iter()
        .find(|spec| spec.namespace == DEFAULT_TRACK_NAMESPACE)
        .or_else(|| settings.specs.first())
        .map(|spec| spec.namespace.clone())
        .unwrap_or_else(|| DEFAULT_TRACK_NAMESPACE.to_string())
}

fn album_namespace_for_track_namespace(namespace: &str) -> String {
    match namespace {
        "musicfm.mean.v1" => DEFAULT_ALBUM_NAMESPACE.to_string(),
        "ast.audioset.v1" => "album.ast.median.v1".to_string(),
        "ast.instruments.v1" => "album.ast_instruments.median.v1".to_string(),
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
                .map(|spec| spec.namespace.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![DEFAULT_TRACK_NAMESPACE.to_string()]);
    if namespaces.is_empty() {
        namespaces.push(DEFAULT_TRACK_NAMESPACE.to_string());
    }
    namespaces
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
    if has("ast.audioset.v1") {
        balanced.push(("ast.audioset.v1".to_string(), 0.3));
    }
    if has("ast.instruments.v1") {
        balanced.push(("ast.instruments.v1".to_string(), 0.15));
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
        "ast.audioset.v1",
        namespaces,
        "similar",
        0.35,
        0.25,
    ));
    recipes.push(recipe_for_namespace(
        "instrumentation",
        "Instrumentation",
        "ast.instruments.v1",
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
        "ast.audioset.v1" => "Audio scene".to_string(),
        "ast.instruments.v1" => "Instrumentation".to_string(),
        other => other.to_string(),
    }
}

fn build_radio(
    catalog_store: &dyn CatalogStore,
    settings: Option<&AudioEmbeddingsSettings>,
    request: RadioBuildRequest,
) -> anyhow::Result<Vec<String>> {
    validate_entity_type(&request.seed.entity_type)?;
    if let Some(recipe_id) = request.recipe_id.as_deref() {
        if !recipes_contain_recipe(settings, recipe_id) {
            return Err(anyhow::anyhow!(
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
            return Err(anyhow::anyhow!(
                "invalid radio request: reference weights must be positive"
            ));
        }
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
    let mut exclude = HashSet::new();
    let mut result = Vec::with_capacity(count);
    if !include_seed_tracks {
        exclude.extend(seed_track_ids.iter().cloned());
    }

    let oversample = (count * criteria.len().max(1) * 24).clamp(150, 2000);
    let mut candidates: HashMap<String, CandidateScore> = HashMap::new();
    for criterion in &criteria {
        let Some(seed) =
            radio_seed_vector(catalog_store, settings, &request.seed, &criterion.namespace)?
        else {
            continue;
        };
        let Some(query) = steered_vector(
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
        let results = catalog_store.search_entity_embeddings(
            &criterion.namespace,
            &query,
            Some("track"),
            oversample,
        )?;
        for search_result in results {
            if exclude.contains(&search_result.entity_id) {
                continue;
            }
            let similarity = search_result.score;
            let score = match mode {
                RadioMode::Similar => similarity,
                RadioMode::Explore => 1.0 - (similarity.clamp(-1.0, 1.0) - 0.55).abs(),
            } * criterion.weight;
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

    let mut rng = rand::rng();
    let mut scored = candidates.into_iter().collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        let left_score = left.1.score + left.1.best_similarity * 0.05;
        let right_score = right.1.score + right.1.best_similarity * 0.05;
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut album_cooldowns: HashMap<String, f32> = HashMap::new();
    let mut artist_cooldowns: HashMap<String, f32> = HashMap::new();
    let mut artist_last_positions: HashMap<String, usize> = HashMap::new();
    if include_seed_tracks {
        result.clear();
        exclude.clear();
        for track_id in &seed_track_ids {
            if result.len() >= count {
                break;
            }
            if exclude.contains(track_id) {
                continue;
            }
            let Some(resolved) = catalog_store.get_resolved_track(track_id)? else {
                continue;
            };
            if !resolved_track_passes_filters(&resolved, request.filters.as_ref())
                || artist_recently_used(&resolved, result.len(), &artist_last_positions)
            {
                continue;
            }
            exclude.insert(track_id.clone());
            push_radio_result(
                track_id.clone(),
                &resolved,
                &mut result,
                &mut album_cooldowns,
                &mut artist_cooldowns,
                &mut artist_last_positions,
            );
        }
    } else {
        let mut initialized_seed_context = HashSet::new();
        for track_id in &seed_track_ids {
            if !initialized_seed_context.insert(track_id) {
                continue;
            }
            if let Some(resolved) = catalog_store.get_resolved_track(track_id)? {
                if resolved_track_passes_filters(&resolved, request.filters.as_ref()) {
                    apply_track_cooldown(&resolved, &mut album_cooldowns, &mut artist_cooldowns);
                }
            }
        }
    }

    let mut ranked = Vec::with_capacity(scored.len());
    for (track_id, candidate) in scored {
        let Some(resolved) = catalog_store.get_resolved_track(&track_id)? else {
            continue;
        };
        if !resolved_track_passes_filters(&resolved, request.filters.as_ref()) {
            continue;
        }
        let jitter = rng.random_range(0.0..(0.1 * randomness));
        ranked.push(RankedRadioCandidate {
            track_id,
            resolved,
            score: candidate.score + candidate.best_similarity * 0.05 + jitter,
        });
    }

    select_radio_candidates(
        ranked,
        &mut result,
        &mut exclude,
        count,
        diversity,
        &mut album_cooldowns,
        &mut artist_cooldowns,
        &mut artist_last_positions,
    );

    Ok(result)
}

fn select_radio_candidates(
    mut ranked: Vec<RankedRadioCandidate>,
    result: &mut Vec<String>,
    exclude: &mut HashSet<String>,
    count: usize,
    diversity: f32,
    album_cooldowns: &mut HashMap<String, f32>,
    artist_cooldowns: &mut HashMap<String, f32>,
    artist_last_positions: &mut HashMap<String, usize>,
) {
    while result.len() < count && !ranked.is_empty() {
        let Some((selected_index, _)) = ranked
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                !artist_recently_used(&candidate.resolved, result.len(), artist_last_positions)
            })
            .map(|(index, candidate)| {
                (
                    index,
                    adjusted_radio_score(
                        candidate.score,
                        &candidate.resolved,
                        diversity,
                        album_cooldowns,
                        artist_cooldowns,
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
        if !exclude.insert(candidate.track_id.clone()) {
            continue;
        }
        push_radio_result(
            candidate.track_id,
            &candidate.resolved,
            result,
            album_cooldowns,
            artist_cooldowns,
            artist_last_positions,
        );
    }
}

fn push_radio_result(
    track_id: String,
    resolved: &ResolvedTrack,
    result: &mut Vec<String>,
    album_cooldowns: &mut HashMap<String, f32>,
    artist_cooldowns: &mut HashMap<String, f32>,
    artist_last_positions: &mut HashMap<String, usize>,
) {
    let position = result.len();
    result.push(track_id);
    apply_track_cooldown(resolved, album_cooldowns, artist_cooldowns);
    record_artist_positions(resolved, position, artist_last_positions);
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
        "track" | "album" | "artist" => Ok(()),
        other => Err(anyhow::anyhow!(
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
            return Err(anyhow::anyhow!(
                "invalid radio request: release_year_min must be <= release_year_max"
            ));
        }
    }
    if let (Some(min), Some(max)) = (filters.popularity_min, filters.popularity_max) {
        if min > max {
            return Err(anyhow::anyhow!(
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
        return Err(anyhow::anyhow!(
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
    let allowed = available_track_namespaces(settings)
        .into_iter()
        .collect::<HashSet<_>>();
    let raw = if request.criteria.is_empty() {
        let recipe = selected_recipe(recipes, request.recipe_id.as_deref())
            .ok_or_else(|| anyhow::anyhow!("invalid radio request: unknown recipe_id"))?;
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

    let mut criteria = Vec::new();
    for criterion in raw {
        if !allowed.contains(&criterion.namespace) {
            return Err(anyhow::anyhow!(
                "invalid radio request: unsupported namespace '{}'",
                criterion.namespace
            ));
        }
        if !criterion.weight.is_finite() || criterion.weight <= 0.0 {
            return Err(anyhow::anyhow!(
                "invalid radio request: criterion weights must be positive"
            ));
        }
        criteria.push(NormalizedRadioCriterion {
            namespace: criterion.namespace,
            weight: criterion.weight,
        });
    }
    if criteria.is_empty() {
        return Err(anyhow::anyhow!(
            "invalid radio request: at least one criterion is required"
        ));
    }
    let total_weight = criteria
        .iter()
        .map(|criterion| criterion.weight)
        .sum::<f32>();
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
    result: &mut Vec<String>,
    exclude: &mut HashSet<String>,
    album_cooldowns: &mut HashMap<String, f32>,
    artist_cooldowns: &mut HashMap<String, f32>,
    artist_last_positions: &mut HashMap<String, usize>,
) -> anyhow::Result<()> {
    if result.len() >= count {
        return Ok(());
    }

    let oversample = (count.saturating_sub(result.len()) * 16).clamp(100, 1000);
    let results =
        catalog_store.search_entity_embeddings(namespace, seed, Some("track"), oversample)?;

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
        if exclude.contains(&track_id) {
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

    select_radio_candidates(
        ranked,
        result,
        exclude,
        count,
        diversity,
        album_cooldowns,
        artist_cooldowns,
        artist_last_positions,
    );

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

    if let Some(resolved) = catalog_store.get_resolved_track(track_id)? {
        if resolved_track_passes_filters(&resolved, None) {
            exclude.insert(track_id.to_string());
            push_radio_result(
                track_id.to_string(),
                &resolved,
                &mut result,
                &mut album_cooldowns,
                &mut artist_cooldowns,
                &mut artist_last_positions,
            );
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
        &mut result,
        &mut exclude,
        &mut album_cooldowns,
        &mut artist_cooldowns,
        &mut artist_last_positions,
    )?;
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
    append_radio_recommendations(
        catalog_store,
        track_namespace,
        &seed,
        count,
        DEFAULT_RADIO_DIVERSITY,
        &mut result,
        &mut exclude,
        &mut album_cooldowns,
        &mut artist_cooldowns,
        &mut artist_last_positions,
    )?;
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

    if let Some(first) = top_tracks.first() {
        if let Some(resolved) = catalog_store.get_resolved_track(first)? {
            if resolved_track_passes_filters(&resolved, None) {
                exclude.insert(first.clone());
                push_radio_result(
                    first.clone(),
                    &resolved,
                    &mut result,
                    &mut album_cooldowns,
                    &mut artist_cooldowns,
                    &mut artist_last_positions,
                );
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
        &mut result,
        &mut exclude,
        &mut album_cooldowns,
        &mut artist_cooldowns,
        &mut artist_last_positions,
    )?;
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

fn recommend_tracks(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    seed: &[f32],
    count: usize,
    exclude: &HashSet<String>,
) -> anyhow::Result<Vec<String>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let oversample = (count * 16).clamp(100, 1000);
    let results =
        catalog_store.search_entity_embeddings(namespace, seed, Some("track"), oversample)?;

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

    let mut selected = Vec::with_capacity(count);
    let mut seen = exclude.clone();
    for (track_id, _) in scored {
        if selected.len() >= count {
            break;
        }
        if !seen.insert(track_id.clone()) {
            continue;
        }
        if !track_is_available(catalog_store, &track_id)? {
            continue;
        }
        selected.push(track_id);
    }

    Ok(selected)
}

fn track_is_available(catalog_store: &dyn CatalogStore, track_id: &str) -> anyhow::Result<bool> {
    Ok(catalog_store
        .get_track(track_id)?
        .map(|track| track.availability == TrackAvailability::Available)
        .unwrap_or(false))
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

        select_radio_candidates(
            ranked,
            &mut result,
            &mut exclude,
            2,
            1.0,
            &mut album_cooldowns,
            &mut artist_cooldowns,
            &mut artist_last_positions,
        );

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

        select_radio_candidates(
            ranked,
            &mut result,
            &mut exclude,
            1,
            1.0,
            &mut album_cooldowns,
            &mut artist_cooldowns,
            &mut artist_last_positions,
        );

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
        push_radio_result(
            "seed".to_string(),
            &seed,
            &mut result,
            &mut album_cooldowns,
            &mut artist_cooldowns,
            &mut artist_last_positions,
        );
        select_radio_candidates(
            ranked,
            &mut result,
            &mut exclude,
            5,
            1.0,
            &mut album_cooldowns,
            &mut artist_cooldowns,
            &mut artist_last_positions,
        );

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
}
