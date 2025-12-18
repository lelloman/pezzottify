//! Relevance filtering for search results.
//!
//! This module provides various algorithms to filter search results
//! based on relevance, removing low-quality matches while keeping
//! highly relevant results.

use super::SearchResult;
use serde::{Deserialize, Serialize};

/// Configuration for relevance filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum RelevanceFilterConfig {
    /// No filtering - return all results (default)
    None,

    /// Keep results scoring at least `threshold` (0.0-1.0) of the best result's score.
    /// E.g., threshold=0.4 means keep results with score >= 40% of best score.
    PercentageOfBest {
        /// Minimum score ratio (0.0-1.0) relative to best result
        threshold: f64,
    },

    /// Cut off results when score drops by more than `drop_threshold` (0.0-1.0)
    /// from the previous result's score.
    /// E.g., drop_threshold=0.5 means cut when next score < 50% of previous.
    GapDetection {
        /// Maximum allowed drop ratio between consecutive scores
        drop_threshold: f64,
    },

    /// Keep results within `num_std_devs` standard deviations of the mean score.
    /// Only filters if there's meaningful variance in scores.
    StandardDeviation {
        /// Number of standard deviations below mean to include
        num_std_devs: f64,
    },

    /// Keep results scoring at least `threshold` of the best, but only if
    /// the best score exceeds `min_best_score`. If best score is below
    /// the minimum, return all results (nothing matched well anyway).
    PercentageWithMinimum {
        /// Minimum score ratio (0.0-1.0) relative to best result
        threshold: f64,
        /// Minimum score the best result must have to apply filtering
        min_best_score: i64,
    },
}

impl Default for RelevanceFilterConfig {
    fn default() -> Self {
        Self::None
    }
}

impl RelevanceFilterConfig {
    /// Parse configuration from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize configuration to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"method":"none"}"#.to_string())
    }

    /// Create a percentage-of-best filter with the given threshold.
    pub fn percentage_of_best(threshold: f64) -> Self {
        Self::PercentageOfBest {
            threshold: threshold.clamp(0.0, 1.0),
        }
    }

    /// Create a gap detection filter with the given drop threshold.
    pub fn gap_detection(drop_threshold: f64) -> Self {
        Self::GapDetection {
            drop_threshold: drop_threshold.clamp(0.0, 1.0),
        }
    }

    /// Create a standard deviation filter.
    pub fn std_deviation(num_std_devs: f64) -> Self {
        Self::StandardDeviation {
            num_std_devs: num_std_devs.max(0.0),
        }
    }

    /// Create a percentage filter with minimum best score requirement.
    pub fn percentage_with_minimum(threshold: f64, min_best_score: i64) -> Self {
        Self::PercentageWithMinimum {
            threshold: threshold.clamp(0.0, 1.0),
            min_best_score,
        }
    }

    /// Apply the configured filter to search results.
    ///
    /// Results should be pre-sorted by score (highest/best first).
    /// Returns filtered results maintaining the original order.
    pub fn filter(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        if results.is_empty() {
            return results;
        }

        match self {
            Self::None => results,
            Self::PercentageOfBest { threshold } => filter_percentage_of_best(results, *threshold),
            Self::GapDetection { drop_threshold } => filter_gap_detection(results, *drop_threshold),
            Self::StandardDeviation { num_std_devs } => {
                filter_std_deviation(results, *num_std_devs)
            }
            Self::PercentageWithMinimum {
                threshold,
                min_best_score,
            } => filter_percentage_with_minimum(results, *threshold, *min_best_score),
        }
    }
}

/// Keep results with score >= threshold * best_score.
fn filter_percentage_of_best(results: Vec<SearchResult>, threshold: f64) -> Vec<SearchResult> {
    let best_score = results[0].adjusted_score;
    if best_score <= 0 {
        // All scores are zero or negative, return all
        return results;
    }

    let min_score = (best_score as f64 * threshold) as i64;

    results
        .into_iter()
        .take_while(|r| r.adjusted_score >= min_score)
        .collect()
}

/// Cut off when score drops by more than drop_threshold from previous.
fn filter_gap_detection(results: Vec<SearchResult>, drop_threshold: f64) -> Vec<SearchResult> {
    if results.len() <= 1 {
        return results;
    }

    let mut filtered = Vec::with_capacity(results.len());
    let mut prev_score = results[0].adjusted_score;

    for result in results {
        let current_score = result.adjusted_score;

        if current_score == prev_score {
            // Same score as previous, always include
            filtered.push(result);
        } else if prev_score > 0 {
            let ratio = current_score as f64 / prev_score as f64;
            if ratio >= drop_threshold {
                prev_score = current_score;
                filtered.push(result);
            } else {
                // Gap detected, stop here
                break;
            }
        } else {
            // Previous score was 0 or negative, can't compute ratio meaningfully
            prev_score = current_score;
            filtered.push(result);
        }
    }

    filtered
}

/// Keep results within num_std_devs standard deviations below the mean.
fn filter_std_deviation(results: Vec<SearchResult>, num_std_devs: f64) -> Vec<SearchResult> {
    if results.len() <= 2 {
        // Not enough data for meaningful statistics
        return results;
    }

    let scores: Vec<f64> = results.iter().map(|r| r.adjusted_score as f64).collect();
    let n = scores.len() as f64;

    let mean = scores.iter().sum::<f64>() / n;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    if std_dev < 1.0 {
        // Very low variance, all scores are similar - return all
        return results;
    }

    // Keep results with score >= mean - (num_std_devs * std_dev)
    let min_score = mean - (num_std_devs * std_dev);

    results
        .into_iter()
        .filter(|r| r.adjusted_score as f64 >= min_score)
        .collect()
}

/// Keep results with score >= threshold * best_score, but only if
/// best_score >= min_best_score. Otherwise return all.
fn filter_percentage_with_minimum(
    results: Vec<SearchResult>,
    threshold: f64,
    min_best_score: i64,
) -> Vec<SearchResult> {
    let best_score = results[0].adjusted_score;

    if best_score < min_best_score {
        // Best result isn't good enough, return all results
        // (nothing matched well, let user see everything)
        return results;
    }

    let min_score = (best_score as f64 * threshold) as i64;

    results
        .into_iter()
        .take_while(|r| r.adjusted_score >= min_score)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::HashedItemType;

    fn make_result(id: &str, score: i64) -> SearchResult {
        SearchResult {
            item_id: id.to_string(),
            item_type: HashedItemType::Track,
            score: score as u32,
            adjusted_score: score,
            matchable_text: id.to_string(),
        }
    }

    fn make_results(scores: &[i64]) -> Vec<SearchResult> {
        scores
            .iter()
            .enumerate()
            .map(|(i, &s)| make_result(&format!("item_{}", i), s))
            .collect()
    }

    // ==========================================================================
    // Serialization tests
    // ==========================================================================

    #[test]
    fn test_config_serialization_none() {
        let config = RelevanceFilterConfig::None;
        let json = config.to_json();
        assert_eq!(json, r#"{"method":"none"}"#);

        let parsed = RelevanceFilterConfig::from_json(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_config_serialization_percentage() {
        let config = RelevanceFilterConfig::percentage_of_best(0.4);
        let json = config.to_json();
        assert!(json.contains("percentage_of_best"));
        assert!(json.contains("0.4"));

        let parsed = RelevanceFilterConfig::from_json(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_config_serialization_gap() {
        let config = RelevanceFilterConfig::gap_detection(0.5);
        let json = config.to_json();
        assert!(json.contains("gap_detection"));

        let parsed = RelevanceFilterConfig::from_json(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_config_serialization_std_dev() {
        let config = RelevanceFilterConfig::std_deviation(2.0);
        let json = config.to_json();
        assert!(json.contains("standard_deviation"));

        let parsed = RelevanceFilterConfig::from_json(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_config_serialization_percentage_with_min() {
        let config = RelevanceFilterConfig::percentage_with_minimum(0.4, 100);
        let json = config.to_json();
        assert!(json.contains("percentage_with_minimum"));

        let parsed = RelevanceFilterConfig::from_json(&json).unwrap();
        assert_eq!(parsed, config);
    }

    // ==========================================================================
    // None filter tests
    // ==========================================================================

    #[test]
    fn test_none_filter_returns_all() {
        let results = make_results(&[1000, 500, 100, 10]);
        let config = RelevanceFilterConfig::None;

        let filtered = config.filter(results);
        assert_eq!(filtered.len(), 4);
    }

    // ==========================================================================
    // Percentage of best tests
    // ==========================================================================

    #[test]
    fn test_percentage_filters_low_scores() {
        // Best=1000, threshold=0.4, min_score=400
        let results = make_results(&[1000, 800, 500, 300, 100]);
        let config = RelevanceFilterConfig::percentage_of_best(0.4);

        let filtered = config.filter(results);

        // Should keep 1000, 800, 500 (all >= 400), drop 300, 100
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].adjusted_score, 1000);
        assert_eq!(filtered[1].adjusted_score, 800);
        assert_eq!(filtered[2].adjusted_score, 500);
    }

    #[test]
    fn test_percentage_keeps_all_when_similar() {
        let results = make_results(&[100, 95, 90, 85, 80]);
        let config = RelevanceFilterConfig::percentage_of_best(0.4);

        let filtered = config.filter(results);

        // All scores >= 40 (40% of 100), so all kept
        assert_eq!(filtered.len(), 5);
    }

    #[test]
    fn test_percentage_empty_results() {
        let results: Vec<SearchResult> = vec![];
        let config = RelevanceFilterConfig::percentage_of_best(0.4);

        let filtered = config.filter(results);
        assert!(filtered.is_empty());
    }

    // ==========================================================================
    // Gap detection tests
    // ==========================================================================

    #[test]
    fn test_gap_detection_finds_gap() {
        // 1000 -> 900 (90%, ok) -> 800 (89%, ok) -> 200 (25%, gap!)
        let results = make_results(&[1000, 900, 800, 200, 100]);
        let config = RelevanceFilterConfig::gap_detection(0.5);

        let filtered = config.filter(results);

        // Should stop at the 800->200 gap
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[2].adjusted_score, 800);
    }

    #[test]
    fn test_gap_detection_no_gap() {
        let results = make_results(&[100, 90, 81, 73, 66]);
        let config = RelevanceFilterConfig::gap_detection(0.5);

        let filtered = config.filter(results);

        // No significant gaps, keep all
        assert_eq!(filtered.len(), 5);
    }

    #[test]
    fn test_gap_detection_immediate_gap() {
        // Huge gap right after first result
        let results = make_results(&[1000, 100, 90, 80]);
        let config = RelevanceFilterConfig::gap_detection(0.5);

        let filtered = config.filter(results);

        // Should only keep the first result
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].adjusted_score, 1000);
    }

    // ==========================================================================
    // Standard deviation tests
    // ==========================================================================

    #[test]
    fn test_std_dev_filters_outliers() {
        // Mean ≈ 520, stddev will be significant
        let results = make_results(&[1000, 900, 800, 100, 50, 10]);
        let config = RelevanceFilterConfig::std_deviation(1.0);

        let filtered = config.filter(results);

        // Should filter out the very low scores
        assert!(filtered.len() < 6);
        // High scores should remain
        assert!(filtered.iter().any(|r| r.adjusted_score == 1000));
    }

    #[test]
    fn test_std_dev_keeps_all_when_similar() {
        let results = make_results(&[100, 99, 98, 97, 96]);
        let config = RelevanceFilterConfig::std_deviation(2.0);

        let filtered = config.filter(results);

        // With 2 std devs, should keep all similar scores
        assert_eq!(filtered.len(), 5);
    }

    // ==========================================================================
    // Percentage with minimum tests
    // ==========================================================================

    #[test]
    fn test_percentage_with_min_applies_when_best_is_high() {
        let results = make_results(&[1000, 500, 100]);
        let config = RelevanceFilterConfig::percentage_with_minimum(0.4, 500);

        let filtered = config.filter(results);

        // Best (1000) > min (500), so apply 40% threshold
        // Keep scores >= 400: 1000, 500
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_percentage_with_min_returns_all_when_best_is_low() {
        let results = make_results(&[100, 50, 10]);
        let config = RelevanceFilterConfig::percentage_with_minimum(0.4, 500);

        let filtered = config.filter(results);

        // Best (100) < min (500), so return all
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_percentage_with_min_applies_when_best_equals_min() {
        // Boundary case: best_score == min_best_score
        let results = make_results(&[500, 300, 100]);
        let config = RelevanceFilterConfig::percentage_with_minimum(0.4, 500);

        let filtered = config.filter(results);

        // Best (500) >= min (500), so apply 40% threshold
        // Keep scores >= 200: 500, 300
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].adjusted_score, 500);
        assert_eq!(filtered[1].adjusted_score, 300);
    }

    // ==========================================================================
    // Edge cases
    // ==========================================================================

    #[test]
    fn test_single_result() {
        let results = make_results(&[1000]);

        for config in [
            RelevanceFilterConfig::None,
            RelevanceFilterConfig::percentage_of_best(0.4),
            RelevanceFilterConfig::gap_detection(0.5),
            RelevanceFilterConfig::std_deviation(1.0),
        ] {
            let filtered = config.filter(results.clone());
            assert_eq!(filtered.len(), 1);
        }
    }

    #[test]
    fn test_all_same_score() {
        let results = make_results(&[500, 500, 500, 500]);

        for config in [
            RelevanceFilterConfig::None,
            RelevanceFilterConfig::percentage_of_best(0.4),
            RelevanceFilterConfig::gap_detection(0.5),
            RelevanceFilterConfig::std_deviation(1.0),
        ] {
            let filtered = config.filter(results.clone());
            assert_eq!(filtered.len(), 4, "Config {:?} failed", config);
        }
    }

    #[test]
    fn test_zero_scores() {
        let results = make_results(&[0, 0, 0]);
        let config = RelevanceFilterConfig::percentage_of_best(0.4);

        let filtered = config.filter(results);

        // All zero scores, should return all
        assert_eq!(filtered.len(), 3);
    }
}
