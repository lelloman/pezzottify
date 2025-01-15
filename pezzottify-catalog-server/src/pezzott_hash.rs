use sha2::{Digest, Sha256};
use std::ops::Sub;

const SIM_HASH_LEN_BITS: usize = 256;
const SIM_HASH_LEN_BYTES: usize = SIM_HASH_LEN_BITS / 8;
const SIM_HASH_LEN_BITS_F64: f64 = SIM_HASH_LEN_BITS as f64;

const SIM_HASH_N_GRAM_LENGTH: usize = 3;
const SIM_HASH_N_GRAM_OVERLAP: usize = 1;

const SIM_HASH_MAX_CHARS: usize = 32;
const PEZZOT_HASH_CHUNKS_CHARS_OVERLAP: usize = 8;

pub struct PezzottHash {
    sim_hashes: Vec<SimHash>,
}

impl PezzottHash {
    pub fn calc<T: AsRef<str>>(source: T) -> PezzottHash {
        let clean_source = source.as_ref().to_lowercase();
        let source_len = clean_source.len();
        let n_grams = if source_len <= SIM_HASH_MAX_CHARS {
            vec![clean_source]
        } else {
            make_n_grams(clean_source, SIM_HASH_MAX_CHARS, PEZZOT_HASH_CHUNKS_CHARS_OVERLAP)
        };
        println!("generated {} chunks", n_grams.len());
        for (i, s) in n_grams.iter().enumerate() {
            println!("{}: {}", i+1, s);
        }
        todo!()
    }
}

struct SimHash {
    value: [u8; SIM_HASH_LEN_BYTES],
    source_length: usize,
}

fn hamming_distance(a: &[u8; SIM_HASH_LEN_BYTES], b: &[u8; SIM_HASH_LEN_BYTES]) -> f64 {
    let mut count = 0u32;

    for (i, v) in a.iter().enumerate() {
        count += (v ^ b[i]).count_ones();
    }
    count as f64
}

impl Sub for &SimHash {
    type Output = f64;

    fn sub(self, other: &SimHash) -> f64 {
        let hamming_dist = hamming_distance(&self.value, &other.value);
        hamming_dist / SIM_HASH_LEN_BITS_F64
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

fn make_n_grams<T: AsRef<str>>(source: T, n_gram_length: usize, overlap: usize) -> Vec<String> {
    let mut ngrams: Vec<String> = vec![];
    let source_str: &str = source.as_ref();
    let mut left = 0;
    let step = n_gram_length - overlap;
    let max_left = source_str.len() - overlap;
    while left < max_left {
        let right = std::cmp::min(left + n_gram_length, source_str.len());
        let n_gram = &source_str[left..right];
        ngrams.push(n_gram.to_owned());
        left += step; 
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
    let source_length = sanitized_source.len();
    let ngrams = make_n_grams(sanitized_source, SIM_HASH_N_GRAM_LENGTH, SIM_HASH_N_GRAM_OVERLAP);
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
    SimHash {
        value,
        source_length,
    }
}

mod tests {
    use super::*;

    #[test]
    fn makes_ngrams() {
        let ngrams1 = make_n_grams("abcdefgh", 5, 1);
        println!("Generated {} ngrams1:", ngrams1.len());
        for (i, s) in ngrams1.iter().enumerate() {
            println!("{}: {}", i, s);
        }
        //assert_eq!(ngrams1.len(), 4);
        //assert_eq!(ngrams1[0], "the");
        //assert_eq!(ngrams1[1], "he ");
        //assert_eq!(ngrams1[2], "e b");
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
            "a rich fat black cat"
        ];
        let hashes: Vec<SimHash> = names.iter().map(|s| make_sim_hash(s)).collect();
        for h in hashes.iter() {
            println!("{:}", h);
        }

        let target = "a rich fat black cat";
        let target_hash = make_sim_hash(&target);

        let distances: Vec<f64> = hashes.iter().map(|h| h - &target_hash).collect();

        for (i, d) in distances.iter().enumerate() {
            println!("{} - {} => {:.2}", target, names[i], d);
        }
    }

    #[test]
    fn delete_me() {
        PezzottHash::calc("0000000011111111222222223333333344444444" );
    }
}
