//! Custom hashing for search functionality
#![allow(dead_code)] // Feature-gated search functionality

use sha2::{Digest, Sha256};
use std::{ops::Sub, u32};
use unicode_segmentation::UnicodeSegmentation;

const SIM_HASH_LEN_BITS: usize = 256;
const SIM_HASH_LEN_BYTES: usize = SIM_HASH_LEN_BITS / 8;

const SIM_HASH_N_GRAM_LENGTH: usize = 3;
const SIM_HASH_N_GRAM_OVERLAP: usize = 1;

pub struct PezzottHash {
    sim_hashes: Vec<SimHash>,
    pub hashed_text: String,
}

impl PezzottHash {
    pub fn calc<T: AsRef<str>>(source: T) -> PezzottHash {
        let hashes = source
            .as_ref()
            .to_lowercase()
            .unicode_words()
            .map(|w| make_sim_hash(w))
            .collect();

        PezzottHash {
            sim_hashes: hashes,
            hashed_text: source.as_ref().to_lowercase().to_string(),
        }
    }

    pub fn match_query(&self, query: &PezzottHash) -> u32 {
        let mut min_scores: Vec<u32> = vec![];
        for self_hash in self.sim_hashes.iter() {
            let mut self_hash_min_score = u32::MAX;
            for other_hash in query.sim_hashes.iter() {
                self_hash_min_score = std::cmp::min(self_hash_min_score, self_hash - other_hash);
            }
            min_scores.push(self_hash_min_score);
        }
        if min_scores.is_empty() {
            return u32::MAX;
        }
        let mut total = 0f64;
        let min_scores_count = min_scores.len() as f64;
        for score in min_scores {
            total += score as f64;
        }
        let avg_score = total / min_scores_count;

        let length_adjustment = if self.hashed_text.len() > query.hashed_text.len() {
            1.0f64
        } else {
            let diff = query.hashed_text.len() - self.hashed_text.len();
            1.0 + diff as f64 * 0.1f64
        };

        (avg_score * length_adjustment) as u32
    }
}

struct SimHash {
    value: [u8; SIM_HASH_LEN_BYTES],
}

fn hamming_distance(a: &[u8; SIM_HASH_LEN_BYTES], b: &[u8; SIM_HASH_LEN_BYTES]) -> u32 {
    let mut count = 0u32;

    for (i, v) in a.iter().enumerate() {
        count += (v ^ b[i]).count_ones();
    }
    count
}

impl Sub for &SimHash {
    type Output = u32;

    fn sub(self, other: &SimHash) -> u32 {
        let hamming_dist = hamming_distance(&self.value, &other.value);
        hamming_dist
    }
}

impl std::fmt::Display for SimHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.value {
            write!(f, "{:08b}", byte)?;
        }
        Ok(())
    }
}

fn make_n_grams<T>(source: T, n_gram_length: usize, overlap: usize) -> Vec<String>
where
    T: IntoIterator,
    T::Item: AsRef<str>,
{
    if n_gram_length <= overlap {
        panic!("The overlap must be smaller than the length of the n gram.")
    }
    let source: Vec<String> = source
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect();
    let mut ngrams: Vec<String> = vec![];
    let mut left = 0;
    let step = n_gram_length - overlap;
    let max_left = if source.len() > overlap {
        source.len() - overlap
    } else {
        source.len()
    };
    loop {
        let right = std::cmp::min(left + n_gram_length, source.len());
        let n_gram = &source[left..right];
        ngrams.push(n_gram.concat());
        left += step;
        if left >= max_left {
            break;
        }
    }

    ngrams
}

fn make_sim_hash<T: AsRef<str>>(source: T) -> SimHash {
    let sanitized_source: String = source
        .as_ref()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let graphemes: Vec<&str> = sanitized_source.graphemes(true).collect();
    let ngrams = make_n_grams(
        graphemes.iter(),
        SIM_HASH_N_GRAM_LENGTH,
        SIM_HASH_N_GRAM_OVERLAP,
    );
    let mut vector: Vec<i64> = vec![0; SIM_HASH_LEN_BITS];

    for ngram in ngrams {
        let mut hasher = Sha256::new();
        hasher.update(ngram);
        let hash_result = hasher.finalize();

        for i in 0..SIM_HASH_LEN_BITS {
            let bit = (hash_result[i / 8] >> (7 - (i % 8))) & 1;
            vector[i] += if bit == 1 { 1 } else { -1 };
        }
    }

    let mut value = [0u8; SIM_HASH_LEN_BYTES];
    for i in 0..SIM_HASH_LEN_BITS {
        if vector[i] > 0 {
            value[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    SimHash { value }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn makes_ngrams() {
        let ngrams = make_n_grams("12345678".graphemes(true), 5, 1);
        assert_eq!(ngrams, vec!["12345", "5678"]);

        let ngrams = make_n_grams("12345678".graphemes(true), 4, 2);
        assert_eq!(ngrams, vec!["1234", "3456", "5678"]);

        let ngrams = make_n_grams("12345678".graphemes(true), 5, 0);
        assert_eq!(ngrams, vec!["12345", "678"]);

        let ngrams = make_n_grams("12345678".graphemes(true), 6, 3);
        assert_eq!(ngrams, vec!["123456", "45678"]);
    }

    #[test]
    fn makes_sim_hashes() {
        let names = vec![
            "ma che c'entra sta stringa qui?",
            "the rich fat cat",
            "a cat",
            "a rich black cat",
            "a black cat",
            "the rich cat fat",
            "a rich fat black cat",
        ];
        let hashes: Vec<SimHash> = names.iter().map(|s| make_sim_hash(s)).collect();
        for h in hashes.iter() {
            println!("{:}", h);
        }

        let target = "a rich fat black cat";
        let target_hash = make_sim_hash(&target);

        let distances: Vec<u32> = hashes.iter().map(|h| h - &target_hash).collect();

        for (i, d) in distances.iter().enumerate() {
            println!("{} - {} => {:.2}", target, names[i], d);
        }
    }
}
