//! Duration fingerprint matching for album identification.
//!
//! Two-phase algorithm:
//! 1. SQL candidate filtering by track count and total duration (±0.1%)
//! 2. Positional track duration comparison

use crate::catalog_store::{AlbumFingerprintCandidate, CatalogStore};
use crate::ingestion::models::TicketType;
use anyhow::Result;
use tracing::debug;

/// Result of fingerprint matching.
#[derive(Debug, Clone)]
pub struct FingerprintMatchResult {
    /// Best matching album candidate (if any).
    pub matched_album: Option<AlbumFingerprintCandidate>,
    /// Match score (0.0 - 1.0, percentage of tracks matching).
    pub match_score: f32,
    /// Total duration delta across all tracks in milliseconds.
    pub total_delta_ms: i64,
    /// Ticket type based on match quality.
    pub ticket_type: TicketType,
    /// Top album candidates for review (up to 5).
    pub candidates: Vec<ScoredCandidate>,
}

/// A scored album candidate for review.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    /// Album candidate.
    pub album: AlbumFingerprintCandidate,
    /// Match score (0.0 - 1.0).
    pub score: f32,
    /// Total duration delta in ms.
    pub delta_ms: i64,
}

/// Configuration for fingerprint matching.
#[derive(Debug, Clone)]
pub struct FingerprintConfig {
    /// Duration tolerance per track in milliseconds (default: 2000ms = 2s).
    pub track_tolerance_ms: i64,
    /// Threshold for auto-ingest (Success ticket).
    /// Requires 100% match and delta < this value.
    pub auto_ingest_delta_threshold_ms: i64,
    /// Minimum score for Review ticket (below this is Failure).
    pub review_score_threshold: f32,
    /// Maximum candidates to return for review.
    pub max_candidates: usize,
}

impl Default for FingerprintConfig {
    fn default() -> Self {
        Self {
            track_tolerance_ms: 2000,
            auto_ingest_delta_threshold_ms: 1000,
            review_score_threshold: 0.90,
            max_candidates: 5,
        }
    }
}

/// Compare uploaded track durations against catalog durations.
///
/// Returns (matches, total_delta):
/// - matches: number of tracks within tolerance
/// - total_delta: sum of absolute differences in ms
fn compare_durations(uploaded: &[i64], catalog: &[i64], tolerance_ms: i64) -> (usize, i64) {
    let mut matches = 0;
    let mut total_delta = 0i64;

    for (u, c) in uploaded.iter().zip(catalog.iter()) {
        let delta = (*u - *c).abs();
        total_delta += delta;
        if delta <= tolerance_ms {
            matches += 1;
        }
    }

    (matches, total_delta)
}

/// Match uploaded tracks against the catalog using duration fingerprints.
///
/// # Arguments
/// * `uploaded_durations` - Track durations in ms, ordered by disc/track number
/// * `catalog_store` - Catalog store for querying album candidates
/// * `config` - Fingerprint matching configuration
///
/// # Returns
/// Fingerprint match result with best candidate and ticket type.
pub fn match_album_by_fingerprint<C: CatalogStore + ?Sized>(
    uploaded_durations: &[i64],
    catalog_store: &C,
    config: &FingerprintConfig,
) -> Result<FingerprintMatchResult> {
    let track_count = uploaded_durations.len() as i32;
    let total_duration_ms: i64 = uploaded_durations.iter().sum();

    debug!(
        track_count = track_count,
        total_duration_ms = total_duration_ms,
        "Starting fingerprint match"
    );

    // Phase 1: Get candidates from catalog
    let candidates = catalog_store.find_albums_by_fingerprint(track_count, total_duration_ms)?;

    debug!(candidate_count = candidates.len(), "Found candidates");

    if candidates.is_empty() {
        return Ok(FingerprintMatchResult {
            matched_album: None,
            match_score: 0.0,
            total_delta_ms: 0,
            ticket_type: TicketType::Failure,
            candidates: vec![],
        });
    }

    // Phase 2: Score each candidate
    let mut scored: Vec<ScoredCandidate> = candidates
        .into_iter()
        .map(|candidate| {
            let (matches, delta) = compare_durations(
                uploaded_durations,
                &candidate.track_durations,
                config.track_tolerance_ms,
            );
            let score = matches as f32 / track_count as f32;
            ScoredCandidate {
                album: candidate,
                score,
                delta_ms: delta,
            }
        })
        .collect();

    // Sort by score descending, then by delta ascending
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.delta_ms.cmp(&b.delta_ms))
    });

    // Determine ticket type based on best match
    let (matched_album, match_score, total_delta_ms, ticket_type) =
        if let Some(best) = scored.first() {
            let ticket_type =
                if best.score >= 1.0 && best.delta_ms < config.auto_ingest_delta_threshold_ms {
                    TicketType::Success
                } else if best.score >= config.review_score_threshold {
                    TicketType::Review
                } else {
                    TicketType::Failure
                };

            (
                Some(best.album.clone()),
                best.score,
                best.delta_ms,
                ticket_type,
            )
        } else {
            (None, 0.0, 0, TicketType::Failure)
        };

    // Return top candidates for review
    let top_candidates: Vec<ScoredCandidate> =
        scored.into_iter().take(config.max_candidates).collect();

    debug!(
        ticket_type = ?ticket_type,
        match_score = match_score,
        total_delta_ms = total_delta_ms,
        "Fingerprint match complete"
    );

    Ok(FingerprintMatchResult {
        matched_album,
        match_score,
        total_delta_ms,
        ticket_type,
        candidates: top_candidates,
    })
}

/// Try matching with relaxed constraints for edge cases.
///
/// Fallback strategies:
/// 1. Widen duration tolerance to ±1% (for hidden tracks)
/// 2. Allow ±2 track count tolerance (for different editions)
pub fn match_album_with_fallbacks<C: CatalogStore + ?Sized>(
    uploaded_durations: &[i64],
    catalog_store: &C,
    config: &FingerprintConfig,
) -> Result<FingerprintMatchResult> {
    // Try exact match first
    let result = match_album_by_fingerprint(uploaded_durations, catalog_store, config)?;

    if result.ticket_type != TicketType::Failure {
        return Ok(result);
    }

    debug!("Exact match failed, trying relaxed tolerance");

    // Fallback 1: Widen track tolerance to 5 seconds
    let relaxed_config = FingerprintConfig {
        track_tolerance_ms: 5000,
        ..config.clone()
    };
    let result = match_album_by_fingerprint(uploaded_durations, catalog_store, &relaxed_config)?;

    if result.ticket_type != TicketType::Failure {
        debug!("Relaxed tolerance match succeeded");
        return Ok(result);
    }

    // Fallback 2: Try with ±2 track count tolerance
    // This requires additional queries with modified track counts
    let track_count = uploaded_durations.len() as i32;
    let total_duration_ms: i64 = uploaded_durations.iter().sum();

    let mut all_candidates = Vec::new();

    // Query for ±1 and ±2 track counts
    for delta in [-2, -1, 1, 2] {
        let adjusted_count = track_count + delta;
        if adjusted_count <= 0 {
            continue;
        }

        // Adjust expected duration proportionally
        let expected_duration = total_duration_ms * adjusted_count as i64 / track_count as i64;

        if let Ok(candidates) =
            catalog_store.find_albums_by_fingerprint(adjusted_count, expected_duration)
        {
            for candidate in candidates {
                // For different track counts, we need a different comparison strategy
                // Compare the subset of tracks that overlap
                let overlap_count = uploaded_durations
                    .len()
                    .min(candidate.track_durations.len());
                let (matches, delta_ms) = compare_durations(
                    &uploaded_durations[..overlap_count],
                    &candidate.track_durations[..overlap_count],
                    config.track_tolerance_ms,
                );
                let score = matches as f32 / overlap_count as f32;

                all_candidates.push(ScoredCandidate {
                    album: candidate,
                    score,
                    delta_ms,
                });
            }
        }
    }

    if all_candidates.is_empty() {
        return Ok(result); // Return original failure
    }

    // Sort and pick best
    all_candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.delta_ms.cmp(&b.delta_ms))
    });

    let best = &all_candidates[0];
    let ticket_type = if best.score >= config.review_score_threshold {
        TicketType::Review // Never auto-ingest with fallback
    } else {
        TicketType::Failure
    };

    Ok(FingerprintMatchResult {
        matched_album: Some(best.album.clone()),
        match_score: best.score,
        total_delta_ms: best.delta_ms,
        ticket_type,
        candidates: all_candidates
            .into_iter()
            .take(config.max_candidates)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_durations_exact_match() {
        let uploaded = vec![180000, 240000, 300000, 210000];
        let catalog = vec![180000, 240000, 300000, 210000];

        let (matches, delta) = compare_durations(&uploaded, &catalog, 2000);

        assert_eq!(matches, 4);
        assert_eq!(delta, 0);
    }

    #[test]
    fn test_compare_durations_within_tolerance() {
        let uploaded = vec![180000, 240000, 300000, 210000];
        let catalog = vec![180500, 239500, 301000, 209000];

        let (matches, delta) = compare_durations(&uploaded, &catalog, 2000);

        assert_eq!(matches, 4); // All within 2s tolerance
        assert_eq!(delta, 500 + 500 + 1000 + 1000); // 3000ms total
    }

    #[test]
    fn test_compare_durations_one_outlier() {
        let uploaded = vec![180000, 240000, 300000, 210000];
        let catalog = vec![180000, 250000, 300000, 210000]; // Track 2 is 10s off

        let (matches, delta) = compare_durations(&uploaded, &catalog, 2000);

        assert_eq!(matches, 3); // Track 2 is outside tolerance
        assert_eq!(delta, 10000);
    }

    #[test]
    fn test_compare_durations_different_length() {
        let uploaded = vec![180000, 240000];
        let catalog = vec![180000, 240000, 300000];

        let (matches, delta) = compare_durations(&uploaded, &catalog, 2000);

        // Only compares up to the shorter list
        assert_eq!(matches, 2);
        assert_eq!(delta, 0);
    }
}
