//! Levenshtein distance implementation for typo-tolerant search
//!
//! This module provides edit distance calculation and vocabulary-based
//! query correction without requiring external SQLite extensions.

use std::collections::HashMap;

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
            by_first_char: HashMap::new(),
            by_length: HashMap::new(),
        }
    }

    /// Add a word to the vocabulary (will be lowercased)
    pub fn add_word(&mut self, word: &str) {
        let word = word.to_lowercase();

        // Skip very short words or if already present
        if word.len() < 2 || self.words.contains(&word) {
            return;
        }

        let idx = self.words.len();
        let first_char = word.chars().next().unwrap();
        let len = word.len();

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

        // If the exact word exists, return it
        if self.words.contains(&query) {
            return Some(&self.words[self.words.iter().position(|w| w == &query).unwrap()]);
        }

        // Collect candidate indices - words within length range
        let mut candidates: Vec<usize> = Vec::new();

        // Only consider words with length within max_distance of query length
        let min_len = query_len.saturating_sub(max_distance);
        let max_len = query_len + max_distance;

        for len in min_len..=max_len {
            if let Some(indices) = self.by_length.get(&len) {
                candidates.extend(indices);
            }
        }

        // Find the best match among candidates
        // Track (index, distance, length_diff) for tie-breaking
        let mut best_match: Option<(usize, usize, usize)> = None;

        for &idx in &candidates {
            let word = &self.words[idx];
            let distance = levenshtein_distance(&query, word);

            if distance <= max_distance {
                let length_diff = (word.len() as isize - query_len as isize).unsigned_abs();

                match best_match {
                    None => best_match = Some((idx, distance, length_diff)),
                    Some((_, best_dist, best_len_diff)) => {
                        // Prefer lower distance, or same distance but closer length
                        // (substitutions are better than insertions/deletions)
                        if distance < best_dist
                            || (distance == best_dist && length_diff < best_len_diff)
                        {
                            best_match = Some((idx, distance, length_diff));
                        }
                    }
                }
            }

            // Early exit if we found an exact match
            if distance == 0 {
                break;
            }
        }

        best_match.map(|(idx, _, _)| self.words[idx].as_str())
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
}
