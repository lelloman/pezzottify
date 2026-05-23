//! Weekly featured albums discovery job.
//!
//! Builds a global weekly set of low-play albums similar to popular albums and
//! stores the snapshot in server key-value state for the home page.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::catalog_store::{AlbumAvailability, CatalogStore, ResolvedAlbum};
use crate::config::FeaturedAlbumsJobSettings;
use crate::user::user_models::PopularAlbum;
use anyhow::Context;
use chrono::{Datelike, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const STATE_KEY: &str = "featured_albums.current.v1";
const DEFAULT_ALBUM_NAMESPACE: &str = "album.musicfm.median.v1";
const ALL_TIME_START_DATE: u32 = 0;
const ALL_TIME_END_DATE: u32 = 99_991_231;
const FALLBACK_PERCENTILES: &[u32] = &[40, 60, 100];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturedAlbumsSnapshot {
    pub week_key: String,
    pub generated_at: i64,
    pub hero_index: usize,
    pub albums: Vec<FeaturedAlbum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturedAlbum {
    pub id: String,
    pub name: String,
    pub artist_names: Vec<String>,
    pub play_count: u64,
}

#[derive(Debug, Clone)]
struct CandidateAlbum {
    album: FeaturedAlbum,
    primary_artist_id: Option<String>,
    similarity: f32,
}

pub struct FeaturedAlbumsJob {
    settings: FeaturedAlbumsJobSettings,
}

impl FeaturedAlbumsJob {
    pub fn from_settings(settings: &FeaturedAlbumsJobSettings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }

    pub fn state_key() -> &'static str {
        STATE_KEY
    }

    pub fn current_week_key() -> String {
        let iso = Utc::now().date_naive().iso_week();
        format!("{}-W{:02}", iso.year(), iso.week())
    }

    fn execute_inner(
        &self,
        ctx: &JobContext,
        force: bool,
    ) -> Result<FeaturedAlbumsSnapshot, JobError> {
        let week_key = Self::current_week_key();
        if !force {
            match ctx.server_store.get_state(STATE_KEY) {
                Ok(Some(raw)) => match serde_json::from_str::<FeaturedAlbumsSnapshot>(&raw) {
                    Ok(snapshot) if snapshot.week_key == week_key => {
                        info!(
                            "Featured albums snapshot for {} is already current",
                            week_key
                        );
                        return Ok(snapshot);
                    }
                    Ok(_) => {}
                    Err(err) => warn!("Ignoring invalid featured albums snapshot: {}", err),
                },
                Ok(None) => {}
                Err(err) => {
                    return Err(JobError::ExecutionFailed(format!(
                        "Failed to read featured albums snapshot: {err}"
                    )));
                }
            }
        }

        let snapshot = self.compute_snapshot(ctx, &week_key)?;
        let raw = serde_json::to_string(&snapshot).map_err(|err| {
            JobError::ExecutionFailed(format!(
                "Failed to serialize featured albums snapshot: {err}"
            ))
        })?;
        ctx.server_store.set_state(STATE_KEY, &raw).map_err(|err| {
            JobError::ExecutionFailed(format!("Failed to store featured albums snapshot: {err}"))
        })?;
        Ok(snapshot)
    }

    fn compute_snapshot(
        &self,
        ctx: &JobContext,
        week_key: &str,
    ) -> Result<FeaturedAlbumsSnapshot, JobError> {
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        let seeds = self.popular_seed_albums(ctx)?;
        let popular_ids = seeds
            .iter()
            .map(|album| album.id.clone())
            .collect::<HashSet<_>>();
        let play_counts = self.album_play_counts(ctx)?;

        let mut selected = Vec::new();
        let mut percentiles = vec![self.settings.low_play_percentile.clamp(0, 100)];
        percentiles.extend(FALLBACK_PERCENTILES.iter().copied());
        percentiles.sort_unstable();
        percentiles.dedup();

        for percentile in percentiles {
            if selected.len() >= self.settings.count {
                break;
            }
            let threshold = play_count_threshold(&play_counts, percentile);
            let candidates =
                self.collect_candidates(ctx, &seeds, &popular_ids, &play_counts, threshold)?;
            selected = select_candidates(
                candidates,
                self.settings.count,
                self.settings.artist_album_cap,
                week_key,
            );
        }

        let hero_index = if selected.is_empty() {
            0
        } else {
            deterministic_index(week_key, selected.len())
        };

        Ok(FeaturedAlbumsSnapshot {
            week_key: week_key.to_string(),
            generated_at: Utc::now().timestamp(),
            hero_index,
            albums: selected
                .into_iter()
                .map(|candidate| candidate.album)
                .collect(),
        })
    }

    fn popular_seed_albums(&self, ctx: &JobContext) -> Result<Vec<PopularAlbum>, JobError> {
        let now = Utc::now();
        let start = now - ChronoDuration::days(365);
        let start_date = parse_yyyymmdd(start)?;
        let end_date = parse_yyyymmdd(now)?;
        let limit = self.settings.popular_seed_album_count.max(1);

        ctx.user_manager
            .lock()
            .unwrap()
            .get_popular_content(start_date, end_date, limit, 0)
            .map(|content| content.albums)
            .map_err(|err| {
                JobError::ExecutionFailed(format!("Failed to load popular seed albums: {err}"))
            })
    }

    fn album_play_counts(&self, ctx: &JobContext) -> Result<HashMap<String, u64>, JobError> {
        let track_counts = ctx
            .user_store
            .get_all_track_play_counts(ALL_TIME_START_DATE, ALL_TIME_END_DATE)
            .map_err(|err| {
                JobError::ExecutionFailed(format!("Failed to load all-time play counts: {err}"))
            })?;

        let mut album_counts = HashMap::new();
        for track_count in track_counts {
            if let Some(album_id) = ctx.catalog_store.get_track_album_id(&track_count.track_id) {
                *album_counts.entry(album_id).or_insert(0) += track_count.play_count;
            }
        }
        Ok(album_counts)
    }

    fn collect_candidates(
        &self,
        ctx: &JobContext,
        seeds: &[PopularAlbum],
        popular_ids: &HashSet<String>,
        play_counts: &HashMap<String, u64>,
        play_count_threshold: u64,
    ) -> Result<Vec<CandidateAlbum>, JobError> {
        let mut candidates: HashMap<String, CandidateAlbum> = HashMap::new();

        for seed in seeds {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            let Some(seed_embedding) = ctx
                .catalog_store
                .get_entity_embedding("album", &seed.id, DEFAULT_ALBUM_NAMESPACE, true)
                .map_err(|err| {
                    JobError::ExecutionFailed(format!(
                        "Failed to load album embedding for {}: {err}",
                        seed.id
                    ))
                })?
            else {
                continue;
            };
            let Some(seed_vector) = seed_embedding.vector else {
                continue;
            };

            let results = ctx
                .catalog_store
                .search_entity_embeddings(
                    DEFAULT_ALBUM_NAMESPACE,
                    &seed_vector,
                    Some("album"),
                    self.settings.candidate_limit_per_seed,
                )
                .map_err(|err| {
                    JobError::ExecutionFailed(format!(
                        "Failed to search album embeddings for {}: {err}",
                        seed.id
                    ))
                })?;

            for result in results {
                let album_id = result.entity_id;
                if album_id == seed.id || popular_ids.contains(&album_id) {
                    continue;
                }
                let play_count = play_counts.get(&album_id).copied().unwrap_or(0);
                if play_count > play_count_threshold && play_count != 0 {
                    continue;
                }
                let Some(candidate) = self.resolve_candidate(
                    ctx.catalog_store.as_ref(),
                    &album_id,
                    play_count,
                    result.score,
                )?
                else {
                    continue;
                };
                candidates
                    .entry(album_id)
                    .and_modify(|existing| {
                        if candidate.similarity > existing.similarity {
                            existing.similarity = candidate.similarity;
                        }
                    })
                    .or_insert(candidate);
            }
        }

        Ok(candidates.into_values().collect())
    }

    fn resolve_candidate(
        &self,
        catalog_store: &dyn CatalogStore,
        album_id: &str,
        play_count: u64,
        similarity: f32,
    ) -> Result<Option<CandidateAlbum>, JobError> {
        let Some(resolved) = catalog_store.get_resolved_album(album_id).map_err(|err| {
            JobError::ExecutionFailed(format!("Failed to resolve album {album_id}: {err}"))
        })?
        else {
            return Ok(None);
        };
        if !album_is_playable(&resolved) {
            return Ok(None);
        }
        let primary_artist_id = resolved.artists.first().map(|artist| artist.id.clone());
        let artist_names = resolved
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect::<Vec<_>>();

        Ok(Some(CandidateAlbum {
            album: FeaturedAlbum {
                id: resolved.album.id,
                name: resolved.album.name,
                artist_names,
                play_count,
            },
            primary_artist_id,
            similarity,
        }))
    }
}

impl Default for FeaturedAlbumsJob {
    fn default() -> Self {
        Self::from_settings(&FeaturedAlbumsJobSettings::default())
    }
}

impl BackgroundJob for FeaturedAlbumsJob {
    fn id(&self) -> &'static str {
        "featured_albums"
    }

    fn name(&self) -> &'static str {
        "Featured Albums"
    }

    fn description(&self) -> &'static str {
        "Build weekly low-play album recommendations similar to popular albums"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Combined {
            cron: None,
            interval: Some(Duration::from_secs(self.settings.interval_hours * 60 * 60)),
            hooks: vec![HookEvent::OnStartup],
        }
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        self.execute_with_params(ctx, None)
    }

    fn execute_with_params(&self, ctx: &JobContext, params: Option<Value>) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        let force = params
            .as_ref()
            .and_then(|value| value.get("force"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        audit.log_started(Some(serde_json::json!({ "force": force })));
        match self.execute_inner(ctx, force) {
            Ok(snapshot) => {
                audit.log_completed(Some(serde_json::json!({
                    "week_key": snapshot.week_key,
                    "album_count": snapshot.albums.len(),
                    "hero_index": snapshot.hero_index,
                })));
                Ok(())
            }
            Err(err) => {
                let message = err.to_string();
                audit.log_failed(&message, None);
                Err(err)
            }
        }
    }
}

fn parse_yyyymmdd<Tz: chrono::TimeZone>(date: chrono::DateTime<Tz>) -> Result<u32, JobError>
where
    Tz::Offset: std::fmt::Display,
{
    date.format("%Y%m%d")
        .to_string()
        .parse::<u32>()
        .context("invalid date")
        .map_err(|err| JobError::ExecutionFailed(err.to_string()))
}

fn album_is_playable(resolved: &ResolvedAlbum) -> bool {
    resolved.album.album_availability != AlbumAvailability::Missing
        && resolved.discs.iter().any(|disc| {
            disc.tracks.iter().any(|track| {
                track.availability == crate::catalog_store::TrackAvailability::Available
            })
        })
}

fn play_count_threshold(play_counts: &HashMap<String, u64>, percentile: u32) -> u64 {
    let mut positives = play_counts
        .values()
        .copied()
        .filter(|count| *count > 0)
        .collect::<Vec<_>>();
    if positives.is_empty() {
        return 0;
    }
    positives.sort_unstable();
    if percentile >= 100 {
        return *positives.last().unwrap_or(&0);
    }
    let position = ((positives.len() as f64) * (percentile as f64 / 100.0)).ceil() as usize;
    positives[position.saturating_sub(1).min(positives.len() - 1)]
}

fn select_candidates(
    mut candidates: Vec<CandidateAlbum>,
    count: usize,
    artist_album_cap: usize,
    week_key: &str,
) -> Vec<CandidateAlbum> {
    candidates.sort_by(|left, right| {
        candidate_rank(right, week_key)
            .partial_cmp(&candidate_rank(left, week_key))
            .unwrap_or(Ordering::Equal)
    });

    let mut selected = Vec::with_capacity(count);
    let mut artist_counts: HashMap<String, usize> = HashMap::new();
    let mut selected_ids = HashSet::new();

    for candidate in &candidates {
        if selected.len() >= count {
            break;
        }
        if let Some(artist_id) = &candidate.primary_artist_id {
            if artist_counts.get(artist_id).copied().unwrap_or(0) >= artist_album_cap {
                continue;
            }
            *artist_counts.entry(artist_id.clone()).or_insert(0) += 1;
        }
        selected_ids.insert(candidate.album.id.clone());
        selected.push(candidate.clone());
    }

    if selected.len() < count {
        for candidate in candidates {
            if selected.len() >= count {
                break;
            }
            if selected_ids.insert(candidate.album.id.clone()) {
                selected.push(candidate);
            }
        }
    }

    selected
}

fn candidate_rank(candidate: &CandidateAlbum, week_key: &str) -> f32 {
    let novelty = if candidate.album.play_count == 0 {
        0.20
    } else {
        0.20 / (candidate.album.play_count as f32 + 1.0)
    };
    candidate.similarity + novelty + deterministic_jitter(week_key, &candidate.album.id)
}

fn deterministic_jitter(week_key: &str, album_id: &str) -> f32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    week_key.hash(&mut hasher);
    album_id.hash(&mut hasher);
    let value = hasher.finish() % 10_000;
    value as f32 / 10_000.0 * 0.02
}

fn deterministic_index(week_key: &str, len: usize) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    week_key.hash(&mut hasher);
    "hero".hash(&mut hasher);
    (hasher.finish() as usize) % len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, artist: &str, play_count: u64, similarity: f32) -> CandidateAlbum {
        CandidateAlbum {
            album: FeaturedAlbum {
                id: id.to_string(),
                name: id.to_string(),
                artist_names: vec![artist.to_string()],
                play_count,
            },
            primary_artist_id: Some(artist.to_string()),
            similarity,
        }
    }

    #[test]
    fn threshold_uses_low_tail_percentile() {
        let counts = HashMap::from([
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 10),
            ("d".to_string(), 20),
        ]);
        assert_eq!(play_count_threshold(&counts, 25), 1);
        assert_eq!(play_count_threshold(&counts, 50), 2);
        assert_eq!(play_count_threshold(&counts, 100), 20);
    }

    #[test]
    fn selection_caps_primary_artist_before_relaxing() {
        let selected = select_candidates(
            vec![
                candidate("a", "artist-1", 0, 1.0),
                candidate("b", "artist-1", 0, 0.99),
                candidate("c", "artist-1", 0, 0.98),
                candidate("d", "artist-2", 0, 0.8),
            ],
            3,
            2,
            "2026-W21",
        );
        assert_eq!(selected.len(), 3);
        assert!(selected.iter().any(|album| album.album.id == "d"));
    }

    #[test]
    fn weekly_jitter_is_stable_within_week() {
        let first = deterministic_jitter("2026-W21", "album-1");
        let second = deterministic_jitter("2026-W21", "album-1");
        assert_eq!(first, second);
        assert_ne!(first, deterministic_jitter("2026-W22", "album-1"));
    }
}
