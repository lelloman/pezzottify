use super::pezzott_hash::PezzottHash;
use crate::catalog::Catalog;

use std::{collections::BinaryHeap, u32};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Hash, Eq)]
pub struct SearchResult {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub score: u32,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl std::cmp::Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.score.cmp(&self.score)
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
    tree: BinaryHeap<SearchResult>,
    capacity: usize,
}

impl SearchResultsHolder {
    fn new(capacity: usize) -> SearchResultsHolder {
        SearchResultsHolder {
            capacity,
            tree: BinaryHeap::with_capacity(capacity),
        }
    }

    fn consume(mut self) -> impl Iterator<Item = SearchResult> {
        let mut out: Vec<SearchResult> = vec![];
        while !self.tree.is_empty() {
            if let Some(popped) = self.tree.pop() {
                out.push(popped);
            }
        }
        out.into_iter()
    }

    fn maybe_add(&mut self, item: &HashedItem, score: u32) {
        let should_add = self.tree.len() < self.capacity ||
            self.tree.peek().map(|i| i.score).unwrap_or(0) > score;
        
        if should_add {
            let result = SearchResult {
                item_id: item.item_id.clone(),
                item_type: item.item_type,
                score: score,
            };
            self.tree.push(result);
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
        println!("SearchVault search for \"{}\"", query.as_ref());
        let query_hash = PezzottHash::calc(query);

        let mut results = SearchResultsHolder::new(10);
        for item in self.items.iter() {
            results.maybe_add(&item, &item.hash - &query_hash);
        }

        results.consume()
    }
}
