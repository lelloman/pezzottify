use sha2::{Sha256, Digest};
use std::ops::Sub;

const SIM_HASH_LEN_BITS: usize = 256;
const SIM_HASH_LEN_BYTES: usize = SIM_HASH_LEN_BITS / 8;
const SIM_HASH_LEN_BITS_F64: f64 = SIM_HASH_LEN_BITS as f64;
const SIM_HASH_N_GRAM_LENGTH: usize = 3;

pub struct PezzottHash {
    sim_hashes: Vec<SimHash>,
}

impl PezzottHash {
    pub fn calc<T: AsRef<str>>(source: T) -> PezzottHash {
        todo!()
    }
}

struct SimHash {
    value: [u8; SIM_HASH_LEN_BYTES],
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

fn make_n_grams<T: AsRef<str>>(source: T) -> Vec<String> {
    let mut ngrams = vec![];
    let source_str: String = source
        .as_ref()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if source_str.len() < SIM_HASH_N_GRAM_LENGTH {
        ngrams.push(source_str);
    } else {
        for i in 0..(source_str.len() - SIM_HASH_N_GRAM_LENGTH + 1) {
            let ngram = &source_str[i..i + SIM_HASH_N_GRAM_LENGTH];
            ngrams.push(ngram.to_owned());
        }
    }
    ngrams
}

fn make_sim_hash<T: AsRef<str>>(source: T) -> SimHash {
    let ngrams = make_n_grams(source);
    let mut vector: Vec<i64> = vec![0; SIM_HASH_LEN_BITS];

    for ngram in ngrams {
        let mut hasher = Sha256::new();
        hasher.update(ngram);
        let hash_result = hasher.finalize();

        for i in 0..SIM_HASH_LEN_BITS {
            let bit = (hash_result[i/8] >> (7 - (i % 8))) & 1;
            vector[i] += if bit == 1 { 1 } else { -1 } ;
        }
    }    

    let mut value = [0u8; SIM_HASH_LEN_BYTES];
    for i in 0..SIM_HASH_LEN_BITS {
        if vector[i] > 0 {
            value[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    SimHash {
        value
    }
}

//fn compute_sim_hash(source: &String) -> SimHash {
//
//}

mod tests {
    use super::*;

    #[test]
    fn makes_ngrams() {
        let ngrams1 = make_n_grams("    the Black cat");
        assert_eq!(ngrams1.len(), 9);
        assert_eq!(ngrams1[0], "the");
        assert_eq!(ngrams1[1], "heb");
        assert_eq!(ngrams1[2], "ebl");
    }

    #[test]
    fn makes_sim_hashes() {
        let names= vec!["the rich fat cat", "a cat", "a rich black cat", "a black cat", "the rich cat fat"];
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
}
