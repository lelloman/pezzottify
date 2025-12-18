//! Popular content pre-computation job.
//!
//! This job periodically computes popular albums and artists based on
//! listening data and caches the results in UserManager for fast retrieval.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::search::HashedItemType;
use crate::user::user_models::{PopularAlbum, PopularArtist, PopularContent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Background job that pre-computes popular content.
///
/// This job runs on startup and then periodically to ensure popular
/// content data is fresh. It aggregates listening statistics to find
/// the most popular albums and artists.
pub struct PopularContentJob {
    /// Number of top albums to compute
    albums_limit: usize,
    /// Number of top artists to compute
    artists_limit: usize,
    /// Number of days of listening data to consider
    lookback_days: u32,
}

impl PopularContentJob {
    /// Create a new PopularContentJob with default settings.
    pub fn new() -> Self {
        Self {
            albums_limit: 20,
            artists_limit: 20,
            lookback_days: 30,
        }
    }

    /// Create a new PopularContentJob with custom settings.
    pub fn with_config(albums_limit: usize, artists_limit: usize, lookback_days: u32) -> Self {
        Self {
            albums_limit,
            artists_limit,
            lookback_days,
        }
    }

    /// Compute the date range for the lookback period.
    fn compute_date_range(&self) -> (u32, u32) {
        use chrono::{Duration as ChronoDuration, Local};

        let end = Local::now();
        let start = end - ChronoDuration::days(self.lookback_days as i64);

        let start_date = start.format("%Y%m%d").to_string().parse().unwrap_or(0);
        let end_date = end.format("%Y%m%d").to_string().parse().unwrap_or(0);

        (start_date, end_date)
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
        // Run on startup and every 6 hours
        JobSchedule::Combined {
            cron: None,
            interval: Some(Duration::from_secs(6 * 60 * 60)), // 6 hours
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
                // ResolvedAlbum has: album: Album, artists: Vec<Artist>, display_image: Option<Image>
                let name = album_json
                    .get("album")
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown Album")
                    .to_string();

                let image_id = album_json
                    .get("display_image")
                    .and_then(|img| img.get("id"))
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());

                // ResolvedAlbum.artists is Vec<Artist>, so we access name directly
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
                    image_id,
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
                // ResolvedArtist has: artist: Artist, display_image: Option<Image>
                let name = artist_json
                    .get("artist")
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown Artist")
                    .to_string();

                let image_id = artist_json
                    .get("display_image")
                    .and_then(|img| img.get("id"))
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());

                popular_artists.push(PopularArtist {
                    id: artist_id.clone(),
                    name,
                    image_id,
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
        // Update search vault with popularity scores for ranking boost
        // =====================================================================
        if let Some(search_vault) = &ctx.search_vault {
            let mut popularity_items = Vec::new();

            // Normalize and add tracks
            // Note: top_tracks is already sorted by play_count descending
            let max_track_plays = top_tracks.first().map(|t| t.play_count).unwrap_or(1);
            for track in &top_tracks {
                let score = track.play_count as f64 / max_track_plays as f64;
                popularity_items.push((
                    track.track_id.clone(),
                    HashedItemType::Track,
                    track.play_count,
                    score,
                ));
            }

            // Normalize and add albums
            let max_album_plays = top_album_ids.first().map(|(_, c)| *c).unwrap_or(1);
            for (album_id, play_count) in &top_album_ids {
                let score = *play_count as f64 / max_album_plays as f64;
                popularity_items.push((
                    album_id.clone(),
                    HashedItemType::Album,
                    *play_count,
                    score,
                ));
            }

            // Normalize and add artists
            let max_artist_plays = top_artist_ids.first().map(|(_, c)| *c).unwrap_or(1);
            for (artist_id, play_count) in &top_artist_ids {
                let score = *play_count as f64 / max_artist_plays as f64;
                popularity_items.push((
                    artist_id.clone(),
                    HashedItemType::Artist,
                    *play_count,
                    score,
                ));
            }

            search_vault
                .lock()
                .unwrap()
                .update_popularity(&popularity_items);

            debug!(
                "Updated search popularity for {} items ({} tracks, {} albums, {} artists)",
                popularity_items.len(),
                top_tracks.len(),
                top_album_ids.len(),
                top_artist_ids.len()
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
    }

    #[test]
    fn test_custom_config() {
        let job = PopularContentJob::with_config(10, 15, 7);

        assert_eq!(job.albums_limit, 10);
        assert_eq!(job.artists_limit, 15);
        assert_eq!(job.lookback_days, 7);
    }

    #[test]
    fn test_date_range_computation() {
        let job = PopularContentJob::with_config(10, 10, 30);
        let (start, end) = job.compute_date_range();

        // Basic sanity check - end should be >= start
        assert!(end >= start);

        // Dates should be in YYYYMMDD format (8 digits)
        assert!(start >= 10000000 && start <= 99999999);
        assert!(end >= 10000000 && end <= 99999999);
    }
}
