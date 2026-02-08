//! Popular content pre-computation job.
//!
//! This job periodically computes popular albums and artists based on
//! listening data and caches the results in UserManager for fast retrieval.
//!
//! ## Composite Popularity Scoring
//!
//! The job computes a composite popularity score from three data sources:
//! 1. **Listening data** (70% weight) - Based on actual play counts from users
//! 2. **Impression data** (25% weight) - Based on page views (artist/album/track screens)
//! 3. **Spotify popularity** (5% weight) - Static fallback from imported Spotify metadata
//!
//! When a data source has no values for an item, its weight is redistributed
//! to the remaining sources proportionally.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::catalog_store::SearchableContentType;
use crate::config::PopularContentJobSettings;
use crate::search::HashedItemType;
use crate::user::user_models::{PopularAlbum, PopularArtist, PopularContent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Weights for composite popularity scoring.
const LISTENING_WEIGHT: f64 = 0.70;
const IMPRESSION_WEIGHT: f64 = 0.25;
const SPOTIFY_WEIGHT: f64 = 0.05;

/// Background job that pre-computes popular content.
///
/// This job runs on startup and then periodically to ensure popular
/// content data is fresh. It aggregates listening statistics to find
/// the most popular albums and artists, and computes composite
/// popularity scores for search ranking.
pub struct PopularContentJob {
    /// Interval in hours between runs
    interval_hours: u64,
    /// Number of top albums to compute
    albums_limit: usize,
    /// Number of top artists to compute
    artists_limit: usize,
    /// Number of days of listening data to consider
    lookback_days: u32,
    /// Number of days of impression data to consider (default: 365)
    impression_lookback_days: u32,
    /// Number of days to retain impression data before pruning (default: 365)
    impression_retention_days: u32,
}

impl PopularContentJob {
    /// Create a new PopularContentJob with default settings.
    pub fn new() -> Self {
        Self::from_settings(&PopularContentJobSettings::default())
    }

    /// Create a new PopularContentJob from settings.
    pub fn from_settings(settings: &PopularContentJobSettings) -> Self {
        Self {
            interval_hours: settings.interval_hours,
            albums_limit: settings.albums_limit,
            artists_limit: settings.artists_limit,
            lookback_days: settings.lookback_days,
            impression_lookback_days: settings.impression_lookback_days,
            impression_retention_days: settings.impression_retention_days,
        }
    }

    /// Create a new PopularContentJob with custom settings (deprecated: use from_settings).
    #[deprecated(note = "Use from_settings instead for better configurability")]
    pub fn with_config(albums_limit: usize, artists_limit: usize, lookback_days: u32) -> Self {
        Self {
            interval_hours: 6,
            albums_limit,
            artists_limit,
            lookback_days,
            impression_lookback_days: 365,
            impression_retention_days: 365,
        }
    }

    /// Compute the date range for the listening lookback period.
    fn compute_date_range(&self) -> (u32, u32) {
        use chrono::{Duration as ChronoDuration, Local};

        let end = Local::now();
        let start = end - ChronoDuration::days(self.lookback_days as i64);

        let start_date = start.format("%Y%m%d").to_string().parse().unwrap_or(0);
        let end_date = end.format("%Y%m%d").to_string().parse().unwrap_or(0);

        (start_date, end_date)
    }

    /// Compute the impression min date for aggregation.
    fn compute_impression_min_date(&self) -> i64 {
        use chrono::{Duration as ChronoDuration, Local};
        let end = Local::now();
        let start = end - ChronoDuration::days(self.impression_lookback_days as i64);
        start.format("%Y%m%d").to_string().parse().unwrap_or(0)
    }

    /// Compute the impression prune date.
    fn compute_impression_prune_date(&self) -> i64 {
        use chrono::{Duration as ChronoDuration, Local};
        let end = Local::now();
        let cutoff = end - ChronoDuration::days(self.impression_retention_days as i64);
        cutoff.format("%Y%m%d").to_string().parse().unwrap_or(0)
    }

    /// Compute composite score from individual normalized scores.
    ///
    /// Weight redistribution: if a score is None, its weight is redistributed
    /// to the other sources proportionally.
    fn compute_composite_score(
        listening_score: Option<f64>,
        impression_score: Option<f64>,
        spotify_score: Option<f64>,
    ) -> f64 {
        // Calculate available weight
        let avail_listening = listening_score.is_some();
        let avail_impression = impression_score.is_some();
        let avail_spotify = spotify_score.is_some();

        // Base weights for available sources
        let w_listening = if avail_listening {
            LISTENING_WEIGHT
        } else {
            0.0
        };
        let w_impression = if avail_impression {
            IMPRESSION_WEIGHT
        } else {
            0.0
        };
        let w_spotify = if avail_spotify { SPOTIFY_WEIGHT } else { 0.0 };

        let total_weight = w_listening + w_impression + w_spotify;

        if total_weight == 0.0 {
            return 0.0;
        }

        // Normalize weights to sum to 1.0
        let w_listening = w_listening / total_weight;
        let w_impression = w_impression / total_weight;
        let w_spotify = w_spotify / total_weight;

        let mut score = 0.0;
        if let Some(s) = listening_score {
            score += s * w_listening;
        }
        if let Some(s) = impression_score {
            score += s * w_impression;
        }
        if let Some(s) = spotify_score {
            score += s * w_spotify;
        }

        score
    }
}

impl Default for PopularContentJob {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundJob for PopularContentJob {
    fn id(&self) -> &'static str {
        "popular_content"
    }

    fn name(&self) -> &'static str {
        "Popular Content"
    }

    fn description(&self) -> &'static str {
        "Pre-compute popular albums and artists based on listening data"
    }

    fn schedule(&self) -> JobSchedule {
        // Run on startup and every configured interval
        JobSchedule::Combined {
            cron: None,
            interval: Some(Duration::from_secs(self.interval_hours * 60 * 60)),
            hooks: vec![HookEvent::OnStartup],
        }
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - it's not critical
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        let (start_date, end_date) = self.compute_date_range();

        info!(
            "Computing popular content for date range {} to {}",
            start_date, end_date
        );

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        audit.log_started(Some(serde_json::json!({
            "start_date": start_date,
            "end_date": end_date,
            "albums_limit": self.albums_limit,
            "artists_limit": self.artists_limit,
            "lookback_days": self.lookback_days,
        })));

        // =====================================================================
        // Compute popular albums from top tracks
        // =====================================================================
        let track_limit = self.albums_limit * 5;
        let top_tracks = match ctx
            .user_store
            .get_top_tracks(start_date, end_date, track_limit)
        {
            Ok(tracks) => tracks,
            Err(e) => {
                let error_msg = format!("Failed to get top tracks: {}", e);
                audit.log_failed(&error_msg, None);
                return Err(JobError::ExecutionFailed(error_msg));
            }
        };

        debug!(
            "Found {} top tracks for album computation",
            top_tracks.len()
        );

        // Aggregate play counts by album
        let mut album_plays: HashMap<String, u64> = HashMap::new();
        for track_stats in &top_tracks {
            if let Some(album_id) = ctx.catalog_store.get_track_album_id(&track_stats.track_id) {
                *album_plays.entry(album_id).or_insert(0) += track_stats.play_count;
            }
        }

        // Sort and get top albums
        let mut album_list: Vec<_> = album_plays.into_iter().collect();
        album_list.sort_by(|a, b| b.1.cmp(&a.1));
        let top_album_ids: Vec<_> = album_list.into_iter().take(self.albums_limit).collect();

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // =====================================================================
        // Compute popular artists from ALL track play counts
        // This ensures artists with many medium-popularity tracks aren't
        // underrepresented compared to artists with one viral hit.
        // =====================================================================
        let all_track_counts = match ctx
            .user_store
            .get_all_track_play_counts(start_date, end_date)
        {
            Ok(counts) => counts,
            Err(e) => {
                let error_msg = format!("Failed to get all track play counts: {}", e);
                audit.log_failed(&error_msg, None);
                return Err(JobError::ExecutionFailed(error_msg));
            }
        };

        debug!(
            "Found {} unique tracks for artist computation",
            all_track_counts.len()
        );

        if all_track_counts.is_empty() {
            info!(
                "No listening data found for the date range, skipping popular content computation"
            );
            audit.log_completed(Some(serde_json::json!({
                "skipped": true,
                "reason": "no_listening_data",
                "albums_count": 0,
                "artists_count": 0,
            })));
            return Ok(());
        }

        // Aggregate play counts by artist from ALL tracks
        let mut artist_plays: HashMap<String, u64> = HashMap::new();
        for track_count in &all_track_counts {
            if let Ok(Some(track_json)) = ctx
                .catalog_store
                .get_resolved_track_json(&track_count.track_id)
            {
                if let Some(artists) = track_json.get("artists").and_then(|a| a.as_array()) {
                    for track_artist in artists {
                        if let Some(artist_id) = track_artist
                            .get("artist")
                            .and_then(|a| a.get("id"))
                            .and_then(|id| id.as_str())
                        {
                            *artist_plays.entry(artist_id.to_string()).or_insert(0) +=
                                track_count.play_count;
                        }
                    }
                }
            }
        }

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // Sort and get top artists
        let mut artist_list: Vec<_> = artist_plays.into_iter().collect();
        artist_list.sort_by(|a, b| b.1.cmp(&a.1));
        let top_artist_ids: Vec<_> = artist_list.into_iter().take(self.artists_limit).collect();

        // =====================================================================
        // Build PopularAlbum objects from resolved album JSON
        // =====================================================================
        let mut popular_albums = Vec::with_capacity(top_album_ids.len());
        for (album_id, play_count) in &top_album_ids {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if let Ok(Some(album_json)) = ctx.catalog_store.get_resolved_album_json(album_id) {
                let name = album_json
                    .get("album")
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown Album")
                    .to_string();

                let artist_names: Vec<String> = album_json
                    .get("artists")
                    .and_then(|a| a.as_array())
                    .map(|artists| {
                        artists
                            .iter()
                            .filter_map(|artist| {
                                artist
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string())
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                popular_albums.push(PopularAlbum {
                    id: album_id.clone(),
                    name,
                    artist_names,
                    play_count: *play_count,
                });
            }
        }

        // =====================================================================
        // Build PopularArtist objects from resolved artist JSON
        // =====================================================================
        let mut popular_artists = Vec::with_capacity(top_artist_ids.len());
        for (artist_id, play_count) in &top_artist_ids {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if let Ok(Some(artist_json)) = ctx.catalog_store.get_resolved_artist_json(artist_id) {
                let name = artist_json
                    .get("artist")
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown Artist")
                    .to_string();

                popular_artists.push(PopularArtist {
                    id: artist_id.clone(),
                    name,
                    play_count: *play_count,
                });
            }
        }

        // =====================================================================
        // Store the results in UserManager's cache
        // =====================================================================
        let content = PopularContent {
            albums: popular_albums.clone(),
            artists: popular_artists.clone(),
        };

        ctx.user_manager
            .lock()
            .unwrap()
            .set_popular_content_cache(content);

        info!(
            "Popular content computed and cached: {} albums, {} artists",
            popular_albums.len(),
            popular_artists.len()
        );

        // =====================================================================
        // Update search vault with composite popularity scores for ranking boost
        // =====================================================================
        if let Some(search_vault) = &ctx.search_vault {
            // Gather all unique items from listening data
            let mut all_items: HashMap<(String, HashedItemType), u64> = HashMap::new();

            // Tracks
            for track in &top_tracks {
                all_items.insert(
                    (track.track_id.clone(), HashedItemType::Track),
                    track.play_count,
                );
            }

            // Albums
            for (album_id, play_count) in &top_album_ids {
                all_items.insert((album_id.clone(), HashedItemType::Album), *play_count);
            }

            // Artists
            for (artist_id, play_count) in &top_artist_ids {
                all_items.insert((artist_id.clone(), HashedItemType::Artist), *play_count);
            }

            // Get impression data
            let impression_min_date = self.compute_impression_min_date();
            let impression_totals = search_vault.get_impression_totals(impression_min_date);

            // Add items that have impressions but no listening data
            for (item_id, item_type) in impression_totals.keys() {
                let hashed_type = match item_type {
                    HashedItemType::Artist => HashedItemType::Artist,
                    HashedItemType::Album => HashedItemType::Album,
                    HashedItemType::Track => HashedItemType::Track,
                };
                all_items.entry((item_id.clone(), hashed_type)).or_insert(0);
            }

            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            // Convert HashedItemType to SearchableContentType for catalog query
            let catalog_items: Vec<(String, SearchableContentType)> = all_items
                .keys()
                .map(|(id, item_type)| {
                    let content_type = match item_type {
                        HashedItemType::Artist => SearchableContentType::Artist,
                        HashedItemType::Album => SearchableContentType::Album,
                        HashedItemType::Track => SearchableContentType::Track,
                    };
                    (id.clone(), content_type)
                })
                .collect();

            // Get Spotify popularity scores from catalog
            let spotify_scores = ctx
                .catalog_store
                .get_items_popularity(&catalog_items)
                .unwrap_or_default();

            // Calculate max values for normalization
            let max_listening = all_items.values().copied().max().unwrap_or(1).max(1) as f64;
            let max_impression = impression_totals
                .values()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1) as f64;
            let max_spotify = 100.0; // Spotify scores are 0-100

            // Compute composite scores
            let mut popularity_items = Vec::new();
            for ((item_id, item_type), play_count) in &all_items {
                // Normalized listening score
                let listening_score = if *play_count > 0 {
                    Some(*play_count as f64 / max_listening)
                } else {
                    None
                };

                // Normalized impression score
                let impression_count = impression_totals
                    .get(&(item_id.clone(), *item_type))
                    .copied()
                    .unwrap_or(0);
                let impression_score = if impression_count > 0 {
                    Some(impression_count as f64 / max_impression)
                } else {
                    None
                };

                // Normalized Spotify score
                let content_type = match item_type {
                    HashedItemType::Artist => SearchableContentType::Artist,
                    HashedItemType::Album => SearchableContentType::Album,
                    HashedItemType::Track => SearchableContentType::Track,
                };
                let spotify_pop = spotify_scores
                    .get(&(item_id.clone(), content_type))
                    .copied();
                let spotify_score = spotify_pop.map(|s| s as f64 / max_spotify);

                // Compute composite score with weight redistribution
                let composite =
                    Self::compute_composite_score(listening_score, impression_score, spotify_score);

                popularity_items.push((
                    item_id.clone(),
                    *item_type,
                    play_count + impression_count,
                    composite,
                ));
            }

            search_vault.update_popularity(&popularity_items);

            // Prune old impression data
            let prune_date = self.compute_impression_prune_date();
            let pruned_count = search_vault.prune_impressions(prune_date);

            debug!(
                "Updated search popularity for {} items (composite scoring); pruned {} old impressions",
                popularity_items.len(),
                pruned_count
            );
        }

        audit.log_completed(Some(serde_json::json!({
            "albums_count": popular_albums.len(),
            "artists_count": popular_artists.len(),
            "tracks_analyzed": all_track_counts.len(),
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_metadata() {
        let job = PopularContentJob::new();

        assert_eq!(job.id(), "popular_content");
        assert_eq!(job.name(), "Popular Content");
        assert!(!job.description().is_empty());
        assert_eq!(job.shutdown_behavior(), ShutdownBehavior::Cancellable);
    }

    #[test]
    fn test_job_schedule() {
        let job = PopularContentJob::new();

        match job.schedule() {
            JobSchedule::Combined {
                cron,
                interval,
                hooks,
            } => {
                assert!(cron.is_none());
                assert!(interval.is_some());
                assert_eq!(interval.unwrap(), Duration::from_secs(6 * 60 * 60));
                assert!(hooks.contains(&HookEvent::OnStartup));
            }
            _ => panic!("Expected Combined schedule"),
        }
    }

    #[test]
    fn test_default_config() {
        let job = PopularContentJob::default();

        assert_eq!(job.albums_limit, 20);
        assert_eq!(job.artists_limit, 20);
        assert_eq!(job.lookback_days, 30);
        assert_eq!(job.impression_lookback_days, 365);
        assert_eq!(job.impression_retention_days, 365);
    }

    #[test]
    fn test_custom_config() {
        let job = PopularContentJob::with_config(10, 15, 7);

        assert_eq!(job.albums_limit, 10);
        assert_eq!(job.artists_limit, 15);
        assert_eq!(job.lookback_days, 7);
        // Impression days use defaults in with_config
        assert_eq!(job.impression_lookback_days, 365);
        assert_eq!(job.impression_retention_days, 365);
    }

    #[test]
    fn test_date_range_computation() {
        let job = PopularContentJob::with_config(10, 10, 30);
        let (start, end) = job.compute_date_range();

        // Basic sanity check - end should be >= start
        assert!(end >= start);

        // Dates should be in YYYYMMDD format (8 digits)
        assert!((10000000..=99999999).contains(&start));
        assert!((10000000..=99999999).contains(&end));
    }

    #[test]
    fn test_composite_score_all_sources() {
        // All three sources available
        let score = PopularContentJob::compute_composite_score(
            Some(1.0), // max listening
            Some(1.0), // max impression
            Some(1.0), // max spotify
        );
        // Should be: 0.70 * 1.0 + 0.25 * 1.0 + 0.05 * 1.0 = 1.0
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_composite_score_only_listening() {
        // Only listening data
        let score = PopularContentJob::compute_composite_score(Some(0.5), None, None);
        // Weight is redistributed: 0.5 * 1.0 = 0.5
        assert!((score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_composite_score_listening_and_spotify() {
        // Listening + Spotify (no impressions)
        let score = PopularContentJob::compute_composite_score(Some(1.0), None, Some(1.0));
        // Weights normalized: listening gets 0.70/(0.70+0.05) = 0.933, spotify gets 0.067
        // Score = 1.0 * 0.933 + 1.0 * 0.067 = 1.0
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_composite_score_no_sources() {
        let score = PopularContentJob::compute_composite_score(None, None, None);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_composite_score_partial_values() {
        // Half values across all sources
        let score = PopularContentJob::compute_composite_score(
            Some(0.5), // half listening
            Some(0.5), // half impression
            Some(0.5), // half spotify
        );
        // Should be: 0.70 * 0.5 + 0.25 * 0.5 + 0.05 * 0.5 = 0.5
        assert!((score - 0.5).abs() < 0.001);
    }
}
