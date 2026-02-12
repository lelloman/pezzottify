//! Target identification strategies for streaming search.
//!
//! The target identifier determines if search results have a clear "winner" -
//! the single item the user most likely wanted. This enables rich enrichment
//! (popular tracks, albums, related artists) for high-confidence matches.

use tracing::debug;

use crate::search::{HashedItemType, SearchResult};

use super::MatchType;

/// Result of target identification with the matched item and confidence score.
#[derive(Debug, Clone)]
pub struct IdentifiedTarget {
    /// The search result identified as the target
    pub result: SearchResult,
    /// Confidence score (0.0 - 1.0) for this identification
    pub confidence: f64,
    /// The type of match for the target
    pub match_type: MatchType,
}

/// Targets identified per content type.
#[derive(Debug, Clone, Default)]
pub struct PerTypeTargets {
    pub artist: Option<IdentifiedTarget>,
    pub album: Option<IdentifiedTarget>,
    pub track: Option<IdentifiedTarget>,
}

/// Trait for target identification strategies.
///
/// Implementations decide if search results have a clear "winner" that should
/// receive special treatment (enrichment with popular content, related artists, etc.).
///
/// Strategies are swappable to allow easy tuning of the identification logic.
pub trait TargetIdentifier: Send + Sync {
    /// Given a query and ranked search results, identify if there's a clear target.
    ///
    /// Returns `Some(IdentifiedTarget)` if there's a high-confidence match,
    /// or `None` if no clear winner was found.
    fn identify_target(&self, query: &str, results: &[SearchResult]) -> Option<IdentifiedTarget>;

    /// Identify the best target for each content type separately.
    ///
    /// Splits results by type and finds the best match within each category.
    fn identify_targets_by_type(&self, query: &str, results: &[SearchResult]) -> PerTypeTargets;
}

/// Configuration for the ScoreGapStrategy.
#[derive(Debug, Clone)]
pub struct ScoreGapConfig {
    /// Minimum normalized score for top result (0.0 - 1.0).
    /// Results below this threshold won't be considered as targets.
    pub min_absolute_score: f64,

    /// Minimum gap between #1 and #2 as ratio of #1's score.
    /// e.g., 0.15 means #1 must be at least 15% better than #2.
    pub min_score_gap_ratio: f64,

    /// Additional confidence boost for exact name matches.
    pub exact_match_boost: f64,

    /// Maximum raw score value for normalization.
    /// Lower raw scores are better (hamming distance), so this is used to invert.
    pub max_raw_score: u32,
}

impl Default for ScoreGapConfig {
    fn default() -> Self {
        Self {
            min_absolute_score: 0.3,
            min_score_gap_ratio: 0.10,
            exact_match_boost: 0.2,
            // Note: This needs to accommodate different search engines.
            // SimHash uses 0-128, but FTS5 BM25 scores can be in thousands.
            // Using a large value that works for both.
            max_raw_score: 10000,
        }
    }
}

/// Default target identification strategy.
///
/// Combines absolute score threshold with relative score gap between
/// the top two results. Also provides a confidence boost for exact matches.
pub struct ScoreGapStrategy {
    config: ScoreGapConfig,
}

impl ScoreGapStrategy {
    pub fn new(config: ScoreGapConfig) -> Self {
        Self { config }
    }

    /// Normalize a raw score to 0.0-1.0 range.
    /// For FTS5 BM25: scores are (-bm25 * 1000), so higher raw score = better match.
    /// We normalize to higher = better (1.0 = best match).
    fn normalize_score(&self, raw_score: u32) -> f64 {
        let clamped = raw_score.min(self.config.max_raw_score);
        clamped as f64 / self.config.max_raw_score as f64
    }

    /// Check if the query is an exact match for the result's matchable text.
    fn is_exact_match(&self, query: &str, result: &SearchResult) -> bool {
        let query_normalized = query.to_lowercase().trim().to_string();
        let result_normalized = result.matchable_text.to_lowercase().trim().to_string();
        query_normalized == result_normalized
    }
}

impl Default for ScoreGapStrategy {
    fn default() -> Self {
        Self::new(ScoreGapConfig::default())
    }
}

impl TargetIdentifier for ScoreGapStrategy {
    fn identify_target(&self, query: &str, results: &[SearchResult]) -> Option<IdentifiedTarget> {
        let first = results.first()?;

        // Normalize the score
        let first_score = self.normalize_score(first.score);

        debug!(
            query = %query,
            first_raw_score = first.score,
            first_normalized = first_score,
            first_text = %first.matchable_text,
            min_absolute = self.config.min_absolute_score,
            "Target identification: checking first result"
        );

        // Check absolute threshold
        if first_score < self.config.min_absolute_score {
            debug!(
                first_score = first_score,
                threshold = self.config.min_absolute_score,
                "Target rejected: below absolute score threshold"
            );
            return None;
        }

        // Check gap to second result (if exists)
        if let Some(second) = results.get(1) {
            let second_score = self.normalize_score(second.score);
            let gap = first_score - second_score;
            let gap_ratio = gap / first_score;

            debug!(
                second_raw_score = second.score,
                second_normalized = second_score,
                second_text = %second.matchable_text,
                gap = gap,
                gap_ratio = gap_ratio,
                min_gap_ratio = self.config.min_score_gap_ratio,
                "Target identification: checking gap to second result"
            );

            // Gap must be at least min_score_gap_ratio of the first score
            if gap_ratio < self.config.min_score_gap_ratio {
                debug!(
                    gap_ratio = gap_ratio,
                    required = self.config.min_score_gap_ratio,
                    "Target rejected: insufficient gap between first and second result"
                );
                return None;
            }
        }
        // If only one result, it's automatically a clear winner

        // Calculate confidence with optional exact match boost
        let is_exact = self.is_exact_match(query, first);
        let confidence = if is_exact {
            (first_score + self.config.exact_match_boost).min(1.0)
        } else {
            first_score
        };

        // Convert HashedItemType to MatchType
        let match_type = match first.item_type {
            HashedItemType::Artist => MatchType::Artist,
            HashedItemType::Album => MatchType::Album,
            HashedItemType::Track => MatchType::Track,
        };

        debug!(
            target_id = %first.item_id,
            target_text = %first.matchable_text,
            confidence = confidence,
            is_exact = is_exact,
            match_type = ?match_type,
            "Target identified successfully"
        );

        Some(IdentifiedTarget {
            result: first.clone(),
            confidence,
            match_type,
        })
    }

    fn identify_targets_by_type(&self, query: &str, results: &[SearchResult]) -> PerTypeTargets {
        // Split results by type
        let artists: Vec<_> = results
            .iter()
            .filter(|r| r.item_type == HashedItemType::Artist)
            .cloned()
            .collect();
        let albums: Vec<_> = results
            .iter()
            .filter(|r| r.item_type == HashedItemType::Album)
            .cloned()
            .collect();
        let tracks: Vec<_> = results
            .iter()
            .filter(|r| r.item_type == HashedItemType::Track)
            .cloned()
            .collect();

        debug!(
            query = %query,
            artist_count = artists.len(),
            album_count = albums.len(),
            track_count = tracks.len(),
            "Identifying targets by type"
        );

        // Find best target in each category
        PerTypeTargets {
            artist: self.identify_target(query, &artists),
            album: self.identify_target(query, &albums),
            track: self.identify_target(query, &tracks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(item_type: HashedItemType, id: &str, score: u32, text: &str) -> SearchResult {
        SearchResult {
            item_type,
            item_id: id.to_string(),
            score,
            adjusted_score: score as i64,
            matchable_text: text.to_string(),
        }
    }

    #[test]
    fn test_single_result_is_target() {
        let strategy = ScoreGapStrategy::default();
        // Score 9500 out of 10000 = 0.95 normalized, well above threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 9500, "Prince")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        let target = target.unwrap();
        assert_eq!(target.result.item_id, "1");
        assert!(target.confidence > 0.9); // High score (9500) = high normalized score
        assert_eq!(target.match_type, MatchType::Artist);
    }

    #[test]
    fn test_empty_results_returns_none() {
        let strategy = ScoreGapStrategy::default();
        let results: Vec<SearchResult> = vec![];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_low_score_not_target() {
        let strategy = ScoreGapStrategy::default();
        // Score 2000 out of 10000 = 0.20 normalized, below 0.3 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 2000, "Something")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_insufficient_gap_not_target() {
        let strategy = ScoreGapStrategy::default();
        // Two results with very similar scores (9000 and 8950)
        // Gap ratio = (0.90 - 0.895) / 0.90 = ~0.005, below 0.10 threshold
        let results = vec![
            make_result(HashedItemType::Artist, "1", 9000, "Prince"),
            make_result(HashedItemType::Artist, "2", 8950, "The Prince"),
        ];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_sufficient_gap_is_target() {
        let strategy = ScoreGapStrategy::default();
        // First result much better than second (9500 vs 7000)
        // Gap ratio = (0.95 - 0.70) / 0.95 = ~0.26, above 0.10 threshold
        let results = vec![
            make_result(HashedItemType::Artist, "1", 9500, "Prince"),
            make_result(HashedItemType::Artist, "2", 7000, "Prince of Persia"),
        ];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());
        assert_eq!(target.unwrap().result.item_id, "1");
    }

    #[test]
    fn test_exact_match_boost() {
        let strategy = ScoreGapStrategy::default();
        // Score 7000 out of 10000 = 0.70 normalized
        let results = vec![make_result(HashedItemType::Artist, "1", 7000, "Prince")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        let target = target.unwrap();
        // With exact match boost, confidence should be higher
        let base_score: f64 = 7000.0 / 10000.0; // 0.70
        let boosted = (base_score + 0.2).min(1.0); // 0.90
        assert!((target.confidence - boosted).abs() < 0.01);
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        let strategy = ScoreGapStrategy::default();
        // Score 8000 out of 10000 = 0.80 normalized
        let results = vec![make_result(HashedItemType::Artist, "1", 8000, "PRINCE")];

        let target = strategy.identify_target("prince", &results);
        assert!(target.is_some());
        // Should get exact match boost despite case difference (0.80 + 0.2 = 1.0)
        assert!(target.unwrap().confidence > 0.95);
    }

    #[test]
    fn test_match_type_conversion() {
        let strategy = ScoreGapStrategy::default();

        // Test artist - score 9500 = 0.95 normalized
        let results = vec![make_result(HashedItemType::Artist, "1", 9500, "Prince")];
        let target = strategy.identify_target("Prince", &results).unwrap();
        assert_eq!(target.match_type, MatchType::Artist);

        // Test album
        let results = vec![make_result(HashedItemType::Album, "1", 9500, "Purple Rain")];
        let target = strategy.identify_target("Purple Rain", &results).unwrap();
        assert_eq!(target.match_type, MatchType::Album);

        // Test track
        let results = vec![make_result(HashedItemType::Track, "1", 9500, "Kiss")];
        let target = strategy.identify_target("Kiss", &results).unwrap();
        assert_eq!(target.match_type, MatchType::Track);
    }

    #[test]
    fn test_custom_config() {
        let config = ScoreGapConfig {
            min_absolute_score: 0.8,   // Higher threshold
            min_score_gap_ratio: 0.25, // Wider gap required
            exact_match_boost: 0.1,    // Smaller boost
            max_raw_score: 128,
        };
        let strategy = ScoreGapStrategy::new(config);

        // Score 108 out of 128 = 0.84 normalized, just above 0.8 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 108, "Prince")];
        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        // Score 98 out of 128 = 0.77 normalized, below 0.8 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 98, "Prince")];
        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_normalize_score() {
        let strategy = ScoreGapStrategy::default();

        // Score 0 = worst match = 0.0
        assert!((strategy.normalize_score(0) - 0.0).abs() < 0.001);

        // Score 10000 (max_raw_score) = perfect match = 1.0
        assert!((strategy.normalize_score(10000) - 1.0).abs() < 0.001);

        // Score 5000 = middle = 0.5
        assert!((strategy.normalize_score(5000) - 0.5).abs() < 0.001);

        // Scores above max are clamped to 1.0
        assert!((strategy.normalize_score(15000) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_identify_targets_by_type_all_types() {
        let strategy = ScoreGapStrategy::default();

        // Mixed results with clear winners in each category
        // First result in each type has score 9500 (0.95 normalized)
        // Second result has score 6000 (0.60 normalized) - big gap
        let results = vec![
            make_result(HashedItemType::Artist, "artist1", 9500, "Prince"),
            make_result(HashedItemType::Album, "album1", 9500, "Purple Rain"),
            make_result(HashedItemType::Track, "track1", 9500, "Kiss"),
            make_result(HashedItemType::Artist, "artist2", 6000, "The Prince"),
            make_result(HashedItemType::Album, "album2", 6000, "Purple Rain Deluxe"),
        ];

        let targets = strategy.identify_targets_by_type("Prince", &results);

        assert!(targets.artist.is_some());
        assert_eq!(targets.artist.unwrap().result.item_id, "artist1");

        assert!(targets.album.is_some());
        assert_eq!(targets.album.unwrap().result.item_id, "album1");

        assert!(targets.track.is_some());
        assert_eq!(targets.track.unwrap().result.item_id, "track1");
    }

    #[test]
    fn test_identify_targets_by_type_partial() {
        let strategy = ScoreGapStrategy::default();

        // Only artist results - score 9500 (0.95 normalized)
        let results = vec![make_result(
            HashedItemType::Artist,
            "artist1",
            9500,
            "Prince",
        )];

        let targets = strategy.identify_targets_by_type("Prince", &results);

        assert!(targets.artist.is_some());
        assert!(targets.album.is_none());
        assert!(targets.track.is_none());
    }

    #[test]
    fn test_identify_targets_by_type_no_clear_winner() {
        let strategy = ScoreGapStrategy::default();

        // Two artists with similar scores - no clear winner
        // 9000 vs 8950 = gap ratio ~0.005, below 0.10 threshold
        let results = vec![
            make_result(HashedItemType::Artist, "artist1", 9000, "Prince"),
            make_result(HashedItemType::Artist, "artist2", 8950, "The Prince"),
        ];

        let targets = strategy.identify_targets_by_type("Prince", &results);

        // No artist target because gap is insufficient
        assert!(targets.artist.is_none());
        assert!(targets.album.is_none());
        assert!(targets.track.is_none());
    }

    #[test]
    fn test_identify_targets_by_type_empty_results() {
        let strategy = ScoreGapStrategy::default();
        let results: Vec<SearchResult> = vec![];

        let targets = strategy.identify_targets_by_type("Prince", &results);

        assert!(targets.artist.is_none());
        assert!(targets.album.is_none());
        assert!(targets.track.is_none());
    }
}
