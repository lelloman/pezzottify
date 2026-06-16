//! Levenshtein distance implementation for typo-tolerant search
//!
//! This module provides edit distance calculation and vocabulary-based
//! query correction without requiring external SQLite extensions.

use std::collections::{HashMap, HashSet};

/// Calculate the Levenshtein (edit) distance between two strings.
/// Returns the minimum number of single-character edits (insertions,
/// deletions, or substitutions) required to change one string into the other.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    // Quick returns for empty strings
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two rows instead of full matrix for space efficiency
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, a_char) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;

        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };

            curr_row[j + 1] = (prev_row[j + 1] + 1) // deletion
                .min(curr_row[j] + 1) // insertion
                .min(prev_row[j] + cost); // substitution
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// A vocabulary for typo-tolerant word matching.
/// Stores unique words and provides fuzzy matching capabilities.
#[derive(Clone)]
pub struct Vocabulary {
    /// All unique words in lowercase
    words: Vec<String>,
    /// Fast lookup for deduplication (O(1) instead of O(n))
    words_set: HashSet<String>,
    /// Index by first character for faster lookups
    by_first_char: HashMap<char, Vec<usize>>,
    /// Index by word length for faster lookups
    by_length: HashMap<usize, Vec<usize>>,
}

impl Vocabulary {
    /// Create a new empty vocabulary
    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            words_set: HashSet::new(),
            by_first_char: HashMap::new(),
            by_length: HashMap::new(),
        }
    }

    /// Add a word to the vocabulary (will be lowercased)
    pub fn add_word(&mut self, word: &str) {
        let word = word.to_lowercase();

        // Skip very short words or if already present (O(1) lookup with HashSet)
        if word.len() < 2 || self.words_set.contains(&word) {
            return;
        }

        let idx = self.words.len();
        let first_char = word.chars().next().unwrap();
        let len = word.len();

        self.words_set.insert(word.clone());
        self.words.push(word);
        self.by_first_char.entry(first_char).or_default().push(idx);
        self.by_length.entry(len).or_default().push(idx);
    }

    /// Add all words from a text (splits on whitespace and punctuation)
    pub fn add_text(&mut self, text: &str) {
        for word in text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation()) {
            if !word.is_empty() {
                self.add_word(word);
            }
        }
    }

    /// Find the best matching word within the given max edit distance.
    /// Returns None if no match is found within the threshold.
    ///
    /// # Arguments
    /// * `query` - The word to match
    /// * `max_distance` - Maximum allowed edit distance (typically 1-2)
    pub fn find_best_match(&self, query: &str, max_distance: usize) -> Option<&str> {
        let query = query.to_lowercase();
        let query_len = query.len();

        // Don't try to correct very short words (1-2 chars).
        // These are often intentional (articles, conjunctions like "e", "le", "la", "de", etc.)
        // and trying to find a "match" within edit distance 2 produces too many false positives.
        // Words < 2 chars aren't in the vocabulary anyway.
        if query_len < 3 {
            return None;
        }

        // If the exact word exists, return it (O(1) lookup)
        if self.words_set.contains(&query) {
            return self.words_set.get(&query).map(|s| s.as_str());
        }

        let candidate_lengths = Self::candidate_lengths(query_len, max_distance);

        // Find the best match among all plausible length buckets. Do not cap this
        // list arbitrarily: in large catalogs a valid correction can be beyond the
        // first N words of the same length bucket.
        let mut best_match: Option<MatchCandidate> = None;

        for len in candidate_lengths {
            let Some(indices) = self.by_length.get(&len) else {
                continue;
            };

            for &idx in indices {
                let word = &self.words[idx];
                let distance = levenshtein_distance(&query, word);

                if distance <= max_distance {
                    let candidate = MatchCandidate::new(idx, distance, &query, word);
                    if best_match
                        .as_ref()
                        .map(|best| candidate.is_better_than(best))
                        .unwrap_or(true)
                    {
                        best_match = Some(candidate);
                    }
                }
            }
        }

        best_match.map(|candidate| self.words[candidate.index].as_str())
    }

    pub fn find_best_matches(&self, query: &str, max_distance: usize, limit: usize) -> Vec<&str> {
        if limit == 0 {
            return Vec::new();
        }

        let query = query.to_lowercase();
        let query_len = query.len();

        if query_len < 3 {
            return Vec::new();
        }

        let candidate_lengths = Self::candidate_lengths(query_len, max_distance);
        let mut matches = Vec::new();

        for len in candidate_lengths {
            let Some(indices) = self.by_length.get(&len) else {
                continue;
            };

            for &idx in indices {
                let word = &self.words[idx];
                let distance = levenshtein_distance(&query, word);

                if distance <= max_distance {
                    matches.push(MatchCandidate::new(idx, distance, &query, word));
                }
            }
        }

        matches.sort_by(|left, right| left.cmp_for_match_order(right));
        matches
            .into_iter()
            .take(limit)
            .map(|candidate| self.words[candidate.index].as_str())
            .collect()
    }

    fn candidate_lengths(query_len: usize, max_distance: usize) -> Vec<usize> {
        let mut lengths = vec![query_len];

        for offset in 1..=max_distance {
            if query_len >= offset {
                lengths.push(query_len - offset);
            }
            lengths.push(query_len + offset);
        }

        lengths
    }

    /// Correct a query by replacing each word with its best vocabulary match.
    /// Words without a good match are kept as-is.
    ///
    /// # Arguments
    /// * `query` - The search query to correct
    /// * `max_distance` - Maximum edit distance per word (typically 1-2)
    pub fn correct_query(&self, query: &str, max_distance: usize) -> String {
        query
            .split_whitespace()
            .map(|word| {
                self.find_best_match(word, max_distance)
                    .unwrap_or(word)
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get the number of words in the vocabulary
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Check if the vocabulary is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self::new()
    }
}

struct MatchCandidate {
    index: usize,
    distance: usize,
    length_diff: usize,
    shared_suffix: usize,
    shared_prefix: usize,
}

impl MatchCandidate {
    fn new(index: usize, distance: usize, query: &str, word: &str) -> Self {
        Self {
            index,
            distance,
            length_diff: (word.len() as isize - query.len() as isize).unsigned_abs(),
            shared_suffix: common_suffix_len(query, word),
            shared_prefix: common_prefix_len(query, word),
        }
    }

    fn cmp_for_match_order(&self, other: &Self) -> std::cmp::Ordering {
        self.distance
            .cmp(&other.distance)
            .then_with(|| self.length_diff.cmp(&other.length_diff))
            .then_with(|| other.shared_suffix.cmp(&self.shared_suffix))
            .then_with(|| other.shared_prefix.cmp(&self.shared_prefix))
            .then_with(|| self.index.cmp(&other.index))
    }

    fn is_better_than(&self, other: &Self) -> bool {
        self.cmp_for_match_order(other).is_lt()
    }
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(a_char, b_char)| a_char == b_char)
        .count()
}

fn common_suffix_len(a: &str, b: &str) -> usize {
    a.chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(a_char, b_char)| a_char == b_char)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        // Same strings
        assert_eq!(levenshtein_distance("hello", "hello"), 0);

        // One character different
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("hello", "jello"), 1);

        // Insertions/deletions
        assert_eq!(levenshtein_distance("hello", "hell"), 1);
        assert_eq!(levenshtein_distance("hello", "helloo"), 1);

        // Multiple edits
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);

        // Empty strings
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
        assert_eq!(levenshtein_distance("", ""), 0);

        // Common typos
        assert_eq!(levenshtein_distance("beatles", "beatels"), 2); // transposition
        assert_eq!(levenshtein_distance("metallica", "metalica"), 1); // missing letter
    }

    #[test]
    fn test_vocabulary_basic() {
        let mut vocab = Vocabulary::new();
        vocab.add_word("beatles");
        vocab.add_word("metallica");
        vocab.add_word("nirvana");

        assert_eq!(vocab.len(), 3);

        // Exact match
        assert_eq!(vocab.find_best_match("beatles", 2), Some("beatles"));

        // Typo correction
        assert_eq!(vocab.find_best_match("beatels", 2), Some("beatles"));
        assert_eq!(vocab.find_best_match("metalica", 2), Some("metallica"));
        assert_eq!(vocab.find_best_match("nirvna", 2), Some("nirvana"));

        // No match within distance
        assert_eq!(vocab.find_best_match("xyz", 1), None);
    }

    #[test]
    fn test_vocabulary_from_text() {
        let mut vocab = Vocabulary::new();
        vocab.add_text("The Beatles Abbey Road");
        vocab.add_text("Led Zeppelin Houses of the Holy");

        // Should have unique words (lowercase)
        assert!(vocab.find_best_match("beatles", 0).is_some());
        assert!(vocab.find_best_match("zeppelin", 0).is_some());
        assert!(vocab.find_best_match("abbey", 0).is_some());
    }

    #[test]
    fn test_correct_query() {
        let mut vocab = Vocabulary::new();
        vocab.add_text("The Beatles Abbey Road Come Together");

        // Correct a query with typos
        let corrected = vocab.correct_query("beatels abey road", 2);
        assert_eq!(corrected, "beatles abbey road");

        // Query with no typos
        let corrected = vocab.correct_query("beatles abbey", 2);
        assert_eq!(corrected, "beatles abbey");

        // Query with unknown words (kept as-is)
        let corrected = vocab.correct_query("beatels xyz", 2);
        assert_eq!(corrected, "beatles xyz");
    }

    #[test]
    fn test_vocabulary_case_insensitive() {
        let mut vocab = Vocabulary::new();
        vocab.add_word("Beatles");
        vocab.add_word("METALLICA");

        assert_eq!(vocab.find_best_match("beatles", 0), Some("beatles"));
        assert_eq!(vocab.find_best_match("BEATLES", 0), Some("beatles"));
        assert_eq!(vocab.find_best_match("metallica", 0), Some("metallica"));
    }

    #[test]
    fn test_lucio_dalla_typo_correction() {
        let mut vocab = Vocabulary::new();
        vocab.add_text("Lucio Dalla");

        // Verify words are in vocabulary
        assert_eq!(vocab.find_best_match("lucio", 0), Some("lucio"));
        assert_eq!(vocab.find_best_match("dalla", 0), Some("dalla"));

        // Verify typo correction works
        // "fucio" -> "lucio" (distance 1: f->l)
        assert_eq!(vocab.find_best_match("fucio", 2), Some("lucio"));
        // "palla" -> "dalla" (distance 1: p->d)
        assert_eq!(vocab.find_best_match("palla", 2), Some("dalla"));

        // Full query correction
        let corrected = vocab.correct_query("fucio palla", 2);
        assert_eq!(corrected, "lucio dalla");
    }

    #[test]
    fn test_lucio_dalla_correction_after_large_same_length_bucket() {
        let mut vocab = Vocabulary::new();

        for i in 0..6_000 {
            vocab.add_word(&format!("x{:04}", i));
        }
        vocab.add_text("Lucio Dalla");

        assert_eq!(vocab.find_best_match("fucio", 2), Some("lucio"));
        assert_eq!(vocab.correct_query("fucio dalla", 2), "lucio dalla");
    }

    #[test]
    fn test_find_best_matches_includes_exact_and_fuzzy_alternatives() {
        let mut vocab = Vocabulary::new();
        vocab.add_text("Lucio Dalla Palla Alla");

        assert_eq!(
            vocab.find_best_matches("palla", 2, 3),
            vec!["palla", "dalla", "alla"]
        );
        assert_eq!(
            vocab.find_best_matches("alla", 2, 3),
            vec!["alla", "dalla", "palla"]
        );
    }

    #[test]
    fn test_tie_breaking_prefers_same_length() {
        let mut vocab = Vocabulary::new();
        // Add both "alla" (length 4) and "dalla" (length 5)
        vocab.add_word("alla");
        vocab.add_word("dalla");

        // "palla" (length 5) should match "dalla" (length 5), not "alla" (length 4)
        // Both have edit distance 1, but "dalla" has same length (substitution)
        // while "alla" requires a deletion
        assert_eq!(vocab.find_best_match("palla", 2), Some("dalla"));
    }

    #[test]
    fn test_short_words_not_corrected() {
        // Test that very short words (1-2 chars) are not incorrectly "corrected"
        // to random longer words. This is important for languages with short
        // articles/conjunctions like Italian "e", "le", "la", etc.
        let mut vocab = Vocabulary::new();
        vocab.add_text("Elio e le Storie Tese");
        vocab.add_text("some random words like le la de");

        // Single character words should return None (not get corrected)
        assert_eq!(
            vocab.find_best_match("e", 2),
            None,
            "Single char 'e' should not be corrected"
        );
        assert_eq!(
            vocab.find_best_match("a", 2),
            None,
            "Single char 'a' should not be corrected"
        );

        // Two character words should also return None (not get corrected)
        // since they're likely intentional short words
        assert_eq!(
            vocab.find_best_match("xy", 2),
            None,
            "Two char 'xy' should not be corrected"
        );

        // But 3+ char words should still be corrected if close match exists
        assert_eq!(vocab.find_best_match("elio", 2), Some("elio"));
        assert_eq!(vocab.find_best_match("eloi", 2), Some("elio")); // typo corrected
    }

    #[test]
    fn test_elio_e_le_storie_tese_query() {
        // Regression test for the specific bug where "elio e le storie tese"
        // search was returning no results because "e" was being corrected
        let mut vocab = Vocabulary::new();
        vocab.add_text("Elio e le Storie Tese");
        vocab.add_text("The Beatles Abbey Road");
        vocab.add_text("some la de words");

        // The corrected query should preserve "e" and "le" as-is
        let corrected = vocab.correct_query("elio e le storie tese", 2);
        assert_eq!(
            corrected, "elio e le storie tese",
            "Short words 'e' and 'le' should not be modified"
        );

        // Even with typos in longer words, short words should be preserved
        let corrected = vocab.correct_query("eloi e le storei tesi", 2);
        assert_eq!(
            corrected, "elio e le storie tese",
            "Typos in longer words corrected, but short words preserved"
        );
    }
}
