use super::pezzott_hash::PezzottHash;
use crate::catalog::Catalog;

use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum HashedItemType {
    Track,
    Artist,
    Album,
}
struct HashedItem {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub hash: PezzottHash,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SearchResult {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub score: usize,
}

impl std::cmp::Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct SearchVault {
    items: Vec<HashedItem>,
}

struct SearchResultsHolder {
    tree: BTreeSet<SearchResult>,
    capacity: usize,
}

impl SearchResultsHolder {
    fn new(capacity: usize) -> SearchResultsHolder {
        SearchResultsHolder {
            capacity,
            tree: BTreeSet::new(),
        }
    }

    fn consume(self) -> impl Iterator<Item = SearchResult> {
        self.tree.into_iter()
    }

    fn maybe_add(&mut self, item: HashedItem, score: usize) {
        let should_add = self.tree.last().map(|i| i.score).unwrap_or(0) < score;

        if should_add {
            let result = SearchResult {
                item_id: item.item_id.clone(),
                item_type: item.item_type,
                score: score,
            };
            let _ = self.tree.insert(result);
            if self.tree.len() > self.capacity {
                self.tree.pop_first();
            }
        }
    }
}

impl SearchVault {
    pub fn new(catalog: &Catalog) -> SearchVault {
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

        SearchVault { items }
    }

    pub fn search<T: AsRef<str>>(&self, query: T) -> impl Iterator<Item = SearchResult> {
        let query_hash = PezzottHash::calc(query);
        let results = SearchResultsHolder::new(10);
        
        results.consume()
    }
}
