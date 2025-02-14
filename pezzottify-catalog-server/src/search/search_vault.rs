use serde::Serialize;

use super::pezzott_hash::PezzottHash;
use crate::catalog::Catalog;

use std::iter;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum HashedItemType {
    Track,
    Artist,
    Album,
}

struct HashedItem {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub hash: PezzottHash,
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug, Hash, Eq, Serialize)]
pub struct SearchResult {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub score: u32,
    pub adjusted_score: i64,
    pub matchable_text: String,
}

#[derive(Debug, Serialize)]
pub struct SearchedAlbum {
    pub id: String,
    pub name: String,
    pub artists_names: Vec<String>,
    pub image_id: Option<String>,
    pub year: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SearchedArtist {
    pub id: String,
    pub name: String,
    pub image_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchedTrack {
    pub id: String,
    pub name: String,
    pub duration: u32,
    pub artists_names: Vec<String>,
    pub image_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResolvedSearchResult {
    Track(SearchedTrack),
    Album(SearchedAlbum),
    Artist(SearchedArtist),
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.adjusted_score == other.adjusted_score
    }
}

impl std::cmp::Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.adjusted_score.cmp(&other.adjusted_score)
    }
}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg_attr(test, derive(Clone))]
struct SearchResultsHolder {
    items: Vec<SearchResult>,
    capacity: usize,
}

struct CharsTrigrams {
    trigrams: Vec<(char, char, char)>,
    input_length: usize,
}

impl CharsTrigrams {
    pub fn new(input: &str) -> CharsTrigrams {
        let it_1 = iter::once(' ').chain(iter::once(' ')).chain(input.chars());
        let it_2 = iter::once(' ').chain(input.chars());
        let it_3 = input.chars().chain(iter::once(' '));

        let trigrams: Vec<(char, char, char)> = it_1
            .zip(it_2)
            .zip(it_3)
            .map(|((a, b), c): ((char, char), char)| (a, b, c))
            .collect();
        CharsTrigrams {
            trigrams,
            input_length: input.chars().count() + 1, /* because of added spaces */
        }
    }

    pub fn similarity(&self, other: &CharsTrigrams) -> f64 {
        let mut acc = 0.0f64;
        for t_a in &self.trigrams {
            for t_b in &other.trigrams {
                if t_a == t_b {
                    acc += 1.0;
                    break;
                }
            }
        }
        let res = acc / (self.input_length as f64);
        res.clamp(0.0, 1.0)
    }
}

impl SearchResultsHolder {
    fn new(capacity: usize) -> SearchResultsHolder {
        SearchResultsHolder {
            items: vec![],
            capacity,
        }
    }

    fn consume(self) -> Vec<SearchResult> {
        self.items
    }

    fn maybe_add(&mut self, item: &HashedItem, score: u32) {
        let should_add =
            self.items.len() < self.capacity || self.items[self.capacity - 1].score > score;

        if should_add {
            let result = SearchResult {
                item_id: item.item_id.clone(),
                item_type: item.item_type,
                score: score,
                adjusted_score: score as i64,
                matchable_text: item.hash.hashed_text.clone(),
            };
            self.items.push(result);
        }
        self.items.sort();
        while self.items.len() > self.capacity {
            self.items.pop();
        }
    }

    fn re_sort<T: AsRef<str>>(&mut self, query: T) {
        let query_trigrams = CharsTrigrams::new(query.as_ref());
        for item in &mut self.items {
            let item_trigrams = CharsTrigrams::new(item.matchable_text.as_str());
            let partial_ratio = 1.0 - query_trigrams.similarity(&item_trigrams);
            item.adjusted_score = (item.score as f64 * partial_ratio) as i64;
        }
        self.items.sort();
    }
}

pub trait SearchVault {
    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult>;
}

pub struct NoOpSearchVault {}

impl SearchVault for NoOpSearchVault {
    fn search(&self, _query: &str, _max_results: usize) -> Vec<SearchResult> {
        Vec::new()
    }
}

pub struct PezzotHashSearchVault {
    items: Vec<HashedItem>,
}

impl PezzotHashSearchVault {
    pub fn new(catalog: &Catalog) -> PezzotHashSearchVault {
        let mut items: Vec<HashedItem> = vec![];

        for artist in catalog.iter_artists() {
            let item = HashedItem {
                item_type: HashedItemType::Artist,
                item_id: artist.id.clone(),
                hash: PezzottHash::calc(&artist.name),
            };
            items.push(item);
        }

        for album in catalog.iter_albums() {
            let item = HashedItem {
                item_type: HashedItemType::Album,
                item_id: album.id.clone(),
                hash: PezzottHash::calc(&album.name),
            };
            items.push(item);
        }

        for track in catalog.iter_tracks() {
            let item = HashedItem {
                item_type: HashedItemType::Track,
                item_id: track.id.clone(),
                hash: PezzottHash::calc(&track.name),
            };
            items.push(item);
        }

        PezzotHashSearchVault { items }
    }
}

impl SearchVault for PezzotHashSearchVault {
    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let query_hash = PezzottHash::calc(&query);

        let mut results = SearchResultsHolder::new(max_results);
        for item in self.items.iter() {
            results.maybe_add(&item, item.hash.match_query(&query_hash));
        }

        results.re_sort(&query);

        results.consume()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_results(holder: SearchResultsHolder, expected: Vec<u32>) {
        let actual: Vec<u32> = holder.consume().iter().map(|r| r.score).collect();
        assert_eq!(actual, expected);
    }

    fn stub_item() -> HashedItem {
        HashedItem {
            hash: PezzottHash::calc(""),
            item_type: HashedItemType::Artist,
            item_id: "asd".to_owned(),
        }
    }

    #[test]
    fn test_search_results_holder() {
        let mut holder = SearchResultsHolder::new(5);

        holder.maybe_add(&stub_item(), 1);
        assert_results(holder.clone(), vec![1]);

        holder.maybe_add(&stub_item(), 0);
        assert_results(holder.clone(), vec![0, 1]);

        holder.maybe_add(&stub_item(), 0);
        assert_results(holder.clone(), vec![0, 0, 1]);

        holder.maybe_add(&stub_item(), 2);
        holder.maybe_add(&stub_item(), 4);
        assert_results(holder.clone(), vec![0, 0, 1, 2, 4]);

        assert_results(holder.clone(), vec![0, 0, 1, 2, 4]);
        holder.maybe_add(&stub_item(), 5);

        holder.maybe_add(&stub_item(), 1);
        assert_results(holder.clone(), vec![0, 0, 1, 1, 2]);
    }
}
