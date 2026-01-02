//! Target identification strategies for streaming search.
//!
//! The target identifier determines if search results have a clear "winner" -
//! the single item the user most likely wanted. This enables rich enrichment
//! (popular tracks, albums, related artists) for high-confidence matches.

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
            min_absolute_score: 0.5,
            min_score_gap_ratio: 0.15,
            exact_match_boost: 0.2,
            max_raw_score: 128, // SimHash uses 128-bit comparison
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
    /// Raw scores are distances (lower is better), so we invert them.
    fn normalize_score(&self, raw_score: u32) -> f64 {
        let clamped = raw_score.min(self.config.max_raw_score);
        1.0 - (clamped as f64 / self.config.max_raw_score as f64)
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

        // Check absolute threshold
        if first_score < self.config.min_absolute_score {
            return None;
        }

        // Check gap to second result (if exists)
        if let Some(second) = results.get(1) {
            let second_score = self.normalize_score(second.score);
            let gap = first_score - second_score;

            // Gap must be at least min_score_gap_ratio of the first score
            if gap / first_score < self.config.min_score_gap_ratio {
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

        Some(IdentifiedTarget {
            result: first.clone(),
            confidence,
            match_type,
        })
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
        let results = vec![make_result(HashedItemType::Artist, "1", 10, "Prince")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        let target = target.unwrap();
        assert_eq!(target.result.item_id, "1");
        assert!(target.confidence > 0.9); // Low score (10) = high normalized score
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
        // Score 100 out of 128 = ~0.22 normalized, below 0.5 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 100, "Something")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_insufficient_gap_not_target() {
        let strategy = ScoreGapStrategy::default();
        // Two results with very similar scores
        let results = vec![
            make_result(HashedItemType::Artist, "1", 10, "Prince"),
            make_result(HashedItemType::Artist, "2", 12, "The Prince"),
        ];

        let target = strategy.identify_target("Prince", &results);
        // Gap is (0.92 - 0.91) / 0.92 = ~0.01, below 0.15 threshold
        assert!(target.is_none());
    }

    #[test]
    fn test_sufficient_gap_is_target() {
        let strategy = ScoreGapStrategy::default();
        // First result much better than second
        let results = vec![
            make_result(HashedItemType::Artist, "1", 5, "Prince"),
            make_result(HashedItemType::Artist, "2", 50, "Prince of Persia"),
        ];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());
        assert_eq!(target.unwrap().result.item_id, "1");
    }

    #[test]
    fn test_exact_match_boost() {
        let strategy = ScoreGapStrategy::default();
        let results = vec![make_result(HashedItemType::Artist, "1", 20, "Prince")];

        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        let target = target.unwrap();
        // With exact match boost, confidence should be higher
        let base_score: f64 = 1.0 - (20.0 / 128.0); // ~0.84
        let boosted = (base_score + 0.2).min(1.0); // ~1.0
        assert!((target.confidence - boosted).abs() < 0.01);
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        let strategy = ScoreGapStrategy::default();
        let results = vec![make_result(HashedItemType::Artist, "1", 10, "PRINCE")];

        let target = strategy.identify_target("prince", &results);
        assert!(target.is_some());
        // Should get exact match boost despite case difference
        assert!(target.unwrap().confidence > 0.95);
    }

    #[test]
    fn test_match_type_conversion() {
        let strategy = ScoreGapStrategy::default();

        // Test artist
        let results = vec![make_result(HashedItemType::Artist, "1", 5, "Prince")];
        let target = strategy.identify_target("Prince", &results).unwrap();
        assert_eq!(target.match_type, MatchType::Artist);

        // Test album
        let results = vec![make_result(HashedItemType::Album, "1", 5, "Purple Rain")];
        let target = strategy.identify_target("Purple Rain", &results).unwrap();
        assert_eq!(target.match_type, MatchType::Album);

        // Test track
        let results = vec![make_result(HashedItemType::Track, "1", 5, "Kiss")];
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

        // Score 20 = 0.84 normalized, just above 0.8 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 20, "Prince")];
        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_some());

        // Score 30 = 0.77 normalized, below 0.8 threshold
        let results = vec![make_result(HashedItemType::Artist, "1", 30, "Prince")];
        let target = strategy.identify_target("Prince", &results);
        assert!(target.is_none());
    }

    #[test]
    fn test_normalize_score() {
        let strategy = ScoreGapStrategy::default();

        // Score 0 = perfect match = 1.0
        assert!((strategy.normalize_score(0) - 1.0).abs() < 0.001);

        // Score 128 = worst match = 0.0
        assert!((strategy.normalize_score(128) - 0.0).abs() < 0.001);

        // Score 64 = middle = 0.5
        assert!((strategy.normalize_score(64) - 0.5).abs() < 0.001);

        // Scores above max are clamped
        assert!((strategy.normalize_score(200) - 0.0).abs() < 0.001);
    }
}
