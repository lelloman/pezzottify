use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::BufReader;
use tokio_util::io::ReaderStream;
use tracing::{error, warn};
use uuid::Uuid;

use crate::agent::{CompletionOptions, LlmProvider, Message, OllamaProvider, OpenAIProvider};

use super::{
    CreateShowDraftRequest, Show, ShowSegment, ShowSegmentKind, ShowSource, ShowSpeaker,
    ShowStatus, UpdateShowScriptRequest,
};
use crate::server::session::Session;
use crate::server::state::{GuardedCatalogStore, GuardedShowStore, ServerState};
use crate::server::ServerConfig;

#[derive(Debug, Deserialize)]
struct ListShowsQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

#[derive(Debug, Serialize)]
struct SynthesizeShowResponse {
    id: String,
    synthesized_segments: usize,
    status: ShowStatus,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct TtsRequest {
    text: String,
    #[serde(rename = "voiceId")]
    voice_id: String,
    format: String,
}

fn default_limit() -> usize {
    50
}

pub fn public_routes() -> Router<ServerState> {
    Router::new()
        .route("/shows", get(list_published_shows))
        .route("/show/{id}", get(get_published_show))
        .route(
            "/show/{show_id}/segment/{segment_id}/stream",
            get(stream_show_segment),
        )
}

pub fn admin_routes() -> Router<ServerState> {
    Router::new()
        .route("/admin/shows", get(admin_list_shows))
        .route("/admin/shows/drafts", post(create_show_draft))
        .route(
            "/admin/shows/{id}",
            get(admin_get_show).delete(admin_delete_show),
        )
        .route("/admin/shows/{id}/script", put(update_show_script))
        .route("/admin/shows/{id}/synthesize", post(synthesize_show))
        .route("/admin/shows/{id}/publish", post(publish_show))
}

async fn list_published_shows(
    State(show_store): State<GuardedShowStore>,
    Query(query): Query<ListShowsQuery>,
) -> Response {
    match show_store.list_published(query.limit.min(100), query.offset) {
        Ok(shows) => Json(shows).into_response(),
        Err(err) => {
            error!("Failed to list published shows: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_published_show(
    State(show_store): State<GuardedShowStore>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    match show_store.get_show(&id) {
        Ok(Some(show)) if show.status == ShowStatus::Published => Json(show).into_response(),
        Ok(Some(_)) | Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {}: {}", id, err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_list_shows(
    State(show_store): State<GuardedShowStore>,
    Query(query): Query<ListShowsQuery>,
) -> Response {
    match show_store.list_admin(query.limit.min(100), query.offset) {
        Ok(shows) => Json(shows).into_response(),
        Err(err) => {
            error!("Failed to list admin shows: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_get_show(
    State(show_store): State<GuardedShowStore>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    match show_store.get_show(&id) {
        Ok(Some(show)) => Json(show).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {}: {}", id, err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_delete_show(
    State(show_store): State<GuardedShowStore>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    match show_store.delete_show(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to delete show {}: {}", id, err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_show_draft(
    session: Session,
    State(show_store): State<GuardedShowStore>,
    State(catalog_store): State<GuardedCatalogStore>,
    State(config): State<ServerConfig>,
    Json(request): Json<CreateShowDraftRequest>,
) -> Response {
    let brief = request.brief.trim();
    if brief.is_empty() {
        return bad_request("brief cannot be empty");
    }

    let target_duration_minutes = request.target_duration_minutes.unwrap_or(75).clamp(10, 120);
    let language = request.language.unwrap_or_else(|| "en".to_string());
    let track_count = ((target_duration_minutes as usize) / 6).clamp(6, 16);
    let track_refs = match catalog_store.list_available_track_ids_with_audio_uri(track_count, 0) {
        Ok(ids) => ids,
        Err(err) => {
            error!("Failed to pick tracks for show draft: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if track_refs.is_empty() {
        return bad_request("catalog has no available tracks to place in the show");
    }

    let default_voice = config.shows.default_voice_id.clone();
    let speakers = vec![
        ShowSpeaker {
            id: "host_1".to_string(),
            name: "Host".to_string(),
            voice_id: Some(default_voice.clone()),
        },
        ShowSpeaker {
            id: "host_2".to_string(),
            name: "Co-host".to_string(),
            voice_id: Some(default_voice),
        },
    ];

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let mut segments = Vec::new();
    let mut track_context = Vec::new();
    let mut sources = vec![ShowSource {
        id: "catalog".to_string(),
        title: "Pezzottify catalog".to_string(),
        url: None,
        excerpt: Some("Seed tracks selected from locally available catalog items.".to_string()),
    }];

    segments.push(ShowSegment {
        id: Uuid::new_v4().to_string(),
        kind: ShowSegmentKind::Narration,
        title: "Opening".to_string(),
        track_id: None,
        speaker_id: Some("host_1".to_string()),
        text: Some(format!(
            "Welcome. Today's show brief is: {}. We'll move through selected tracks and leave space for context, reactions, and transitions.",
            brief
        )),
        audio_path: None,
        mime_type: None,
        duration_ms: None,
        source_ids: vec!["catalog".to_string()],
    });

    for (index, (track_id, _audio_uri)) in track_refs.iter().enumerate() {
        let track_title = catalog_store
            .get_track(track_id)
            .ok()
            .flatten()
            .map(|track| track.name)
            .unwrap_or_else(|| track_id.clone());
        track_context.push((track_id.clone(), track_title.clone()));
        let speaker_id = if index % 2 == 0 { "host_1" } else { "host_2" };
        let source_id = format!("track_{}", index + 1);
        sources.push(ShowSource {
            id: source_id.clone(),
            title: track_title.clone(),
            url: None,
            excerpt: Some(format!("Catalog track id: {}", track_id)),
        });
        segments.push(ShowSegment {
            id: Uuid::new_v4().to_string(),
            kind: ShowSegmentKind::Narration,
            title: format!("Set up {}", track_title),
            track_id: None,
            speaker_id: Some(speaker_id.to_string()),
            text: Some(format!(
                "Next up: {}. Replace this placeholder with researched context, dialogue, or a tighter transition before synthesis.",
                track_title
            )),
            audio_path: None,
            mime_type: None,
            duration_ms: None,
            source_ids: vec![source_id.clone()],
        });
        segments.push(ShowSegment {
            id: Uuid::new_v4().to_string(),
            kind: ShowSegmentKind::Track,
            title: track_title,
            track_id: Some(track_id.clone()),
            speaker_id: None,
            text: None,
            audio_path: None,
            mime_type: None,
            duration_ms: None,
            source_ids: vec![source_id],
        });
    }

    segments.push(ShowSegment {
        id: Uuid::new_v4().to_string(),
        kind: ShowSegmentKind::Narration,
        title: "Closing".to_string(),
        track_id: None,
        speaker_id: Some("host_2".to_string()),
        text: Some("That closes the show. Replace this placeholder with final credits, links, and a stronger sign-off before publishing.".to_string()),
        audio_path: None,
        mime_type: None,
        duration_ms: None,
        source_ids: vec!["catalog".to_string()],
    });

    let show = Show {
        id: id.clone(),
        title: title_from_brief(brief),
        status: ShowStatus::ScriptReady,
        brief: brief.to_string(),
        summary: format!("AI show draft from brief: {}", brief),
        language,
        target_duration_minutes,
        created_by_user_id: session.user_id,
        created_at: now,
        updated_at: now,
        published_at: None,
        speakers,
        segments,
        sources,
        error: None,
    };

    let mut show = show;
    if let Some(generated) = generate_script_with_llm(&show, &track_context, &config).await {
        let mut candidate = show.clone();
        candidate.title = generated.title;
        candidate.summary = generated.summary;
        candidate.language = generated.language;
        candidate.target_duration_minutes = generated.target_duration_minutes;
        candidate.speakers = generated.speakers;
        candidate.segments = generated.segments;
        candidate.sources = generated.sources;
        match validate_show_script(&candidate, config.shows.max_speakers, &catalog_store) {
            Ok(()) => show = candidate,
            Err(err) => warn!("Ignoring invalid LLM show script for {}: {}", show.id, err),
        }
    }

    if let Err(err) = validate_show_script(&show, config.shows.max_speakers, &catalog_store) {
        return bad_request(err);
    }

    match show_store.upsert_show(&show) {
        Ok(()) => (StatusCode::CREATED, Json(show)).into_response(),
        Err(err) => {
            error!("Failed to store show draft: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn generate_script_with_llm(
    show: &Show,
    tracks: &[(String, String)],
    config: &ServerConfig,
) -> Option<UpdateShowScriptRequest> {
    if !config.agent.enabled {
        return None;
    }

    let provider: Box<dyn LlmProvider> = match config.agent.llm.provider.as_str() {
        "openai" => match &config.agent.llm.api_key_command {
            Some(command) => Box::new(OpenAIProvider::with_key_command(
                config.agent.llm.base_url.clone(),
                config.agent.llm.model.clone(),
                command.clone(),
            )),
            None => Box::new(OpenAIProvider::new(
                config.agent.llm.base_url.clone(),
                config.agent.llm.model.clone(),
                config.agent.llm.api_key.clone(),
            )),
        },
        _ => Box::new(OllamaProvider::new(
            config.agent.llm.base_url.clone(),
            config.agent.llm.model.clone(),
        )),
    };

    let track_lines = tracks
        .iter()
        .enumerate()
        .map(|(index, (id, title))| format!("{}. {} [{}]", index + 1, title, id))
        .collect::<Vec<_>>()
        .join("\n");

    let messages = vec![
        Message::system(
            "You write long-form radio show scripts as strict JSON. Return only valid JSON, no markdown.",
        ),
        Message::user(format!(
            "Create a publishable music show script. Brief: {brief}\nLanguage: {language}\nTarget duration minutes: {duration}\nTracks must stay in this order and track segments must use exactly these track_id values:\n{tracks}\n\nReturn JSON with keys title, summary, language, target_duration_minutes, speakers, sources, segments. speakers are objects with id, name, voice_id. segments are ordered objects with id, kind (narration or track), title, track_id, speaker_id, text, source_ids. Use multiple narration segments between tracks as dialogue, alternate speakers, cite source ids, and include every track exactly once as a track segment.",
            brief = show.brief,
            language = show.language,
            duration = show.target_duration_minutes,
            tracks = track_lines,
        )),
    ];

    let options = CompletionOptions {
        temperature: config.agent.llm.temperature,
        max_tokens: Some(12_000),
        timeout: Duration::from_secs(config.agent.llm.timeout_secs),
    };

    let response = match provider.complete(&messages, None, &options).await {
        Ok(response) => response,
        Err(err) => {
            warn!("Show script LLM generation failed: {}", err);
            return None;
        }
    };

    let json = extract_json_object(&response.message.content);
    match serde_json::from_str::<UpdateShowScriptRequest>(&json) {
        Ok(script) => Some(script),
        Err(err) => {
            warn!("Show script LLM returned invalid JSON: {}", err);
            None
        }
    }
}

fn extract_json_object(content: &str) -> String {
    let trimmed = content.trim();
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }
    trimmed.to_string()
}

async fn update_show_script(
    State(show_store): State<GuardedShowStore>,
    State(catalog_store): State<GuardedCatalogStore>,
    State(config): State<ServerConfig>,
    AxumPath(id): AxumPath<String>,
    Json(request): Json<UpdateShowScriptRequest>,
) -> Response {
    let mut show = match show_store.get_show(&id) {
        Ok(Some(show)) => show,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {}: {}", id, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    show.title = request.title;
    show.summary = request.summary;
    show.language = request.language;
    show.target_duration_minutes = request.target_duration_minutes;
    show.speakers = request.speakers;
    show.segments = request.segments;
    show.sources = request.sources;
    show.status = ShowStatus::ScriptReady;
    show.error = None;
    show.published_at = None;
    show.updated_at = Utc::now().timestamp();

    if let Err(err) = validate_show_script(&show, config.shows.max_speakers, &catalog_store) {
        return bad_request(err);
    }

    match show_store.upsert_show(&show) {
        Ok(()) => Json(show).into_response(),
        Err(err) => {
            error!("Failed to update show {}: {}", id, err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn synthesize_show(
    State(show_store): State<GuardedShowStore>,
    State(config): State<ServerConfig>,
    State(client): State<crate::server::state::HttpClient>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let mut show = match show_store.get_show(&id) {
        Ok(Some(show)) => show,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {} for synthesis: {}", id, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if show.status == ShowStatus::Published {
        return bad_request("published shows cannot be synthesized in place");
    }

    show.status = ShowStatus::Synthesizing;
    show.error = None;
    show.updated_at = Utc::now().timestamp();
    if let Err(err) = show_store.upsert_show(&show) {
        error!("Failed to mark show {} synthesizing: {}", id, err);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let result = synthesize_narration_segments(&mut show, &config, &client).await;
    match result {
        Ok(count) => {
            show.status = ShowStatus::Ready;
            show.error = None;
            show.updated_at = Utc::now().timestamp();
            match show_store.upsert_show(&show) {
                Ok(()) => Json(SynthesizeShowResponse {
                    id,
                    synthesized_segments: count,
                    status: show.status,
                })
                .into_response(),
                Err(err) => {
                    error!("Failed to persist synthesized show: {}", err);
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Err(message) => {
            show.status = ShowStatus::Failed;
            show.error = Some(message.clone());
            show.updated_at = Utc::now().timestamp();
            if let Err(err) = show_store.upsert_show(&show) {
                error!("Failed to persist failed show state: {}", err);
            }
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse { error: message }),
            )
                .into_response()
        }
    }
}

async fn publish_show(
    State(show_store): State<GuardedShowStore>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let mut show = match show_store.get_show(&id) {
        Ok(Some(show)) => show,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {} for publish: {}", id, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if show.status != ShowStatus::Ready && show.status != ShowStatus::Published {
        return bad_request("only ready shows can be published");
    }

    let now = Utc::now().timestamp();
    show.status = ShowStatus::Published;
    show.published_at = Some(show.published_at.unwrap_or(now));
    show.updated_at = now;

    match show_store.upsert_show(&show) {
        Ok(()) => Json(show).into_response(),
        Err(err) => {
            error!("Failed to publish show {}: {}", id, err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn stream_show_segment(
    State(show_store): State<GuardedShowStore>,
    State(config): State<ServerConfig>,
    AxumPath((show_id, segment_id)): AxumPath<(String, String)>,
) -> Response {
    let show = match show_store.get_show(&show_id) {
        Ok(Some(show)) if show.status == ShowStatus::Published => show,
        Ok(Some(_)) | Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Failed to load show {}: {}", show_id, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let segment = match show
        .segments
        .iter()
        .find(|segment| segment.id == segment_id)
    {
        Some(segment) if segment.kind == ShowSegmentKind::Narration => segment,
        Some(_) | None => return StatusCode::NOT_FOUND.into_response(),
    };

    let relative_path = match &segment.audio_path {
        Some(path) => path,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let path = resolve_show_audio_path(&config.shows_media_dir(), relative_path);
    let file = match File::open(&path).await {
        Ok(file) => file,
        Err(err) => {
            warn!("Failed to open show audio {}: {}", path.display(), err);
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    let length = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let mime_type = segment.mime_type.as_deref().unwrap_or("audio/ogg");
    let stream = ReaderStream::with_capacity(BufReader::with_capacity(4096 * 16, file), 4096 * 16);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_LENGTH, length)
        .body(Body::from_stream(stream))
        .unwrap()
}

async fn synthesize_narration_segments(
    show: &mut Show,
    config: &ServerConfig,
    client: &reqwest::Client,
) -> Result<usize, String> {
    tokio::fs::create_dir_all(config.shows_media_dir().join(&show.id))
        .await
        .map_err(|err| format!("failed to create show audio directory: {}", err))?;

    let mut count = 0;
    for index in 0..show.segments.len() {
        if show.segments[index].kind != ShowSegmentKind::Narration {
            continue;
        }
        let text = show.segments[index]
            .text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("narration segment {} has no text", show.segments[index].id))?;
        let voice_id = voice_id_for_segment(&show.segments[index], &show.speakers, config);
        let bytes = request_tts_audio(client, config, text, &voice_id).await?;
        let relative_path = format!("{}/{}.ogg", show.id, show.segments[index].id);
        let absolute_path = config.shows_media_dir().join(&relative_path);
        tokio::fs::write(&absolute_path, bytes)
            .await
            .map_err(|err| format!("failed to write TTS audio: {}", err))?;
        show.segments[index].audio_path = Some(relative_path);
        show.segments[index].mime_type = Some("audio/ogg".to_string());
        count += 1;
    }
    Ok(count)
}

async fn request_tts_audio(
    client: &reqwest::Client,
    config: &ServerConfig,
    text: &str,
    voice_id: &str,
) -> Result<Vec<u8>, String> {
    let url = format!(
        "{}/v1/audio/tts",
        config.shows.simple_ai_base_url.trim_end_matches('/')
    );
    let request = TtsRequest {
        text: text.to_string(),
        voice_id: voice_id.to_string(),
        format: "ogg".to_string(),
    };

    let mut builder = client.post(url).json(&request);
    if let Some(env_name) = &config.shows.api_key_env {
        if let Ok(api_key) = std::env::var(env_name) {
            if !api_key.trim().is_empty() {
                builder = builder.bearer_auth(api_key);
            }
        }
    }

    let response = builder
        .send()
        .await
        .map_err(|err| format!("simple-ai TTS request failed: {}", err))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "simple-ai TTS returned {}{}",
            status,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {}", body)
            }
        ));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("failed to read simple-ai TTS audio: {}", err))
}

fn validate_show_script(
    show: &Show,
    max_speakers: usize,
    catalog_store: &GuardedCatalogStore,
) -> Result<(), String> {
    if show.title.trim().is_empty() {
        return Err("title cannot be empty".to_string());
    }
    if show.summary.trim().is_empty() {
        return Err("summary cannot be empty".to_string());
    }
    if show.language.trim().is_empty() {
        return Err("language cannot be empty".to_string());
    }
    if show.target_duration_minutes <= 0 || show.target_duration_minutes > 180 {
        return Err("target_duration_minutes must be between 1 and 180".to_string());
    }
    if show.speakers.is_empty() {
        return Err("at least one speaker is required".to_string());
    }
    if show.speakers.len() > max_speakers {
        return Err(format!("at most {} speakers are allowed", max_speakers));
    }
    if show.segments.is_empty() {
        return Err("at least one segment is required".to_string());
    }

    let speaker_ids = collect_ids(show.speakers.iter().map(|speaker| speaker.id.as_str()))?;
    let source_ids = collect_ids(show.sources.iter().map(|source| source.id.as_str()))?;
    let mut segment_ids = HashSet::new();

    for segment in &show.segments {
        if segment.id.trim().is_empty() || !segment_ids.insert(segment.id.clone()) {
            return Err("segment ids must be unique and non-empty".to_string());
        }
        for source_id in &segment.source_ids {
            if !source_ids.contains(source_id) {
                return Err(format!(
                    "segment {} references missing source {}",
                    segment.id, source_id
                ));
            }
        }
        match segment.kind {
            ShowSegmentKind::Narration => {
                let speaker_id = segment.speaker_id.as_deref().ok_or_else(|| {
                    format!("narration segment {} needs a speaker_id", segment.id)
                })?;
                if !speaker_ids.contains(speaker_id) {
                    return Err(format!(
                        "segment {} references missing speaker {}",
                        segment.id, speaker_id
                    ));
                }
                if segment
                    .text
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    return Err(format!("narration segment {} needs text", segment.id));
                }
            }
            ShowSegmentKind::Track => {
                let track_id = segment
                    .track_id
                    .as_deref()
                    .ok_or_else(|| format!("track segment {} needs a track_id", segment.id))?;
                match catalog_store.get_track(track_id) {
                    Ok(Some(track)) if track.audio_uri.is_some() => {}
                    Ok(Some(_)) => {
                        return Err(format!("track {} has no available audio", track_id))
                    }
                    Ok(None) => return Err(format!("track {} does not exist", track_id)),
                    Err(err) => {
                        return Err(format!("failed to validate track {}: {}", track_id, err))
                    }
                }
            }
        }
    }

    Ok(())
}

fn collect_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Result<HashSet<String>, String> {
    let mut out = HashSet::new();
    for id in ids {
        if id.trim().is_empty() || !out.insert(id.to_string()) {
            return Err("ids must be unique and non-empty".to_string());
        }
    }
    Ok(out)
}

fn voice_id_for_segment(
    segment: &ShowSegment,
    speakers: &[ShowSpeaker],
    config: &ServerConfig,
) -> String {
    segment
        .speaker_id
        .as_deref()
        .and_then(|id| speakers.iter().find(|speaker| speaker.id == id))
        .and_then(|speaker| speaker.voice_id.clone())
        .unwrap_or_else(|| config.shows.default_voice_id.clone())
}

fn resolve_show_audio_path(base: &Path, relative_path: &str) -> PathBuf {
    relative_path
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn title_from_brief(brief: &str) -> String {
    let trimmed = brief.trim();
    let mut title: String = trimmed.chars().take(72).collect();
    if title.len() < trimmed.len() {
        title.push_str("...");
    }
    if title.is_empty() {
        "Untitled Show".to_string()
    } else {
        title
    }
}

fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}
