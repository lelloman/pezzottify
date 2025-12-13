//! Popular content pre-computation job.
//!
//! This job periodically computes popular albums and artists based on
//! listening data. While the actual caching happens in UserManager,
//! this job ensures the underlying database queries are warmed up.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
};
use std::collections::HashMap;
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
        let (start_date, end_date) = self.compute_date_range();

        info!(
            "Computing popular content for date range {} to {}",
            start_date, end_date
        );

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // =====================================================================
        // Compute popular albums from top tracks
        // =====================================================================
        let track_limit = self.albums_limit * 5;
        let top_tracks = ctx
            .user_store
            .get_top_tracks(start_date, end_date, track_limit)
            .map_err(|e| JobError::ExecutionFailed(format!("Failed to get top tracks: {}", e)))?;

        debug!("Found {} top tracks for album computation", top_tracks.len());

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
        let top_albums: Vec<_> = album_list.into_iter().take(self.albums_limit).collect();

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // =====================================================================
        // Compute popular artists from ALL track play counts
        // This ensures artists with many medium-popularity tracks aren't
        // underrepresented compared to artists with one viral hit.
        // =====================================================================
        let all_track_counts = ctx
            .user_store
            .get_all_track_play_counts(start_date, end_date)
            .map_err(|e| {
                JobError::ExecutionFailed(format!("Failed to get all track play counts: {}", e))
            })?;

        debug!(
            "Found {} unique tracks for artist computation",
            all_track_counts.len()
        );

        if all_track_counts.is_empty() {
            info!(
                "No listening data found for the date range, skipping popular content computation"
            );
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
        let top_artists: Vec<_> = artist_list.into_iter().take(self.artists_limit).collect();

        // =====================================================================
        // Warm up catalog store caches by resolving album and artist JSON
        // =====================================================================
        let mut albums_resolved = 0;
        let mut artists_resolved = 0;

        for (album_id, _) in &top_albums {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if ctx.catalog_store.get_resolved_album_json(album_id).is_ok() {
                albums_resolved += 1;
            }
        }

        for (artist_id, _) in &top_artists {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if ctx
                .catalog_store
                .get_resolved_artist_json(artist_id)
                .is_ok()
            {
                artists_resolved += 1;
            }
        }

        info!(
            "Popular content computed: {} albums, {} artists (resolved {}/{} albums, {}/{} artists)",
            top_albums.len(),
            top_artists.len(),
            albums_resolved,
            top_albums.len(),
            artists_resolved,
            top_artists.len()
        );

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
