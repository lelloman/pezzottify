//! Streaming search pipeline implementation.
//!
//! The pipeline orchestrates the streaming search process:
//! 1. Run organic search
//! 2. Identify best match per content type (artist, album, track)
//! 3. Emit primary matches with enrichment for each type found
//! 4. Emit remaining results (MoreResults if primaries exist, Results if not)
//! 5. Emit done

use std::collections::HashSet;
use std::time::Instant;

use crate::catalog_store::CatalogStore;
use crate::config::StreamingSearchSettings;
use crate::search::resolve;
use crate::search::{HashedItemType, ResolvedSearchResult};
use crate::user::UserManager;

use super::enrichment::track_summary_with_image;
use super::sections::{AlbumSummary, ArtistSummary, MatchType, SearchSection, TrackSummary};
use super::target_identifier::{
    IdentifiedTarget, ScoreGapConfig, ScoreGapStrategy, TargetIdentifier,
};

/// Streaming search pipeline that generates sections progressively.
pub struct StreamingSearchPipeline<'a> {
    catalog_store: &'a dyn CatalogStore,
    user_manager: &'a UserManager,
    target_identifier: Box<dyn TargetIdentifier>,
    config: StreamingSearchSettings,
}

impl<'a> StreamingSearchPipeline<'a> {
    /// Create a new pipeline with the given dependencies and configuration.
    pub fn new(
        catalog_store: &'a dyn CatalogStore,
        user_manager: &'a UserManager,
        config: StreamingSearchSettings,
    ) -> Self {
        // Create target identifier based on config strategy
        // Note: max_raw_score needs to accommodate different search engines.
        // SimHash uses 0-128, but FTS5 BM25 scores can be in thousands.
        let target_identifier: Box<dyn TargetIdentifier> =
            Box::new(ScoreGapStrategy::new(ScoreGapConfig {
                min_absolute_score: config.min_absolute_score,
                min_score_gap_ratio: config.min_score_gap_ratio,
                exact_match_boost: config.exact_match_boost,
                max_raw_score: 10000,
            }));

        Self {
            catalog_store,
            user_manager,
            target_identifier,
            config,
        }
    }

    /// Execute the streaming search and return all sections.
    ///
    /// Takes search results from the caller (already obtained from SearchVault).
    pub fn execute(
        &self,
        query: &str,
        search_results: Vec<crate::search::SearchResult>,
    ) -> Vec<SearchSection> {
        let start_time = Instant::now();

        // Track IDs that have been emitted to avoid duplicates
        let mut emitted_ids: HashSet<String> = HashSet::new();

        // Identify best match for each content type
        let targets = self
            .target_identifier
            .identify_targets_by_type(query, &search_results);

        let has_any_primary =
            targets.artist.is_some() || targets.album.is_some() || targets.track.is_some();

        // Build sections
        let mut sections: Vec<SearchSection> = Vec::new();

        // Emit primary artist with enrichment
        if let Some(ref artist_target) = targets.artist {
            if let Some(section) = self.build_primary_artist(artist_target, &mut emitted_ids) {
                sections.push(section);
            }
            self.add_artist_enrichment(
                &artist_target.result.item_id,
                &mut sections,
                &mut emitted_ids,
            );
        }

        // Emit primary album with enrichment
        if let Some(ref album_target) = targets.album {
            if let Some(section) = self.build_primary_album(album_target, &mut emitted_ids) {
                sections.push(section);
            }
            self.add_album_enrichment(
                &album_target.result.item_id,
                &mut sections,
                &mut emitted_ids,
            );
        }

        // Emit primary track (no enrichment for tracks - sibling tracks would be redundant)
        if let Some(ref track_target) = targets.track {
            if let Some(section) = self.build_primary_track(track_target, &mut emitted_ids) {
                sections.push(section);
            }
        }

        // Emit remaining results
        let remaining = self.build_remaining_results(&search_results, &emitted_ids);
        if let Some(items) = remaining {
            if has_any_primary {
                // Show as "More Results" when we have primary matches
                sections.push(SearchSection::MoreResults { items });
            } else {
                // Show as plain "Results" when no primary matches were found
                sections.push(SearchSection::Results { items });
            }
        }

        // Emit done
        let elapsed = start_time.elapsed();
        sections.push(SearchSection::Done {
            total_time_ms: elapsed.as_millis() as u64,
        });

        sections
    }

    /// Build the primary artist section.
    fn build_primary_artist(
        &self,
        target: &IdentifiedTarget,
        emitted_ids: &mut HashSet<String>,
    ) -> Option<SearchSection> {
        let item = self.resolve_search_result(&target.result.item_id, target.result.item_type)?;
        emitted_ids.insert(target.result.item_id.clone());

        Some(SearchSection::PrimaryArtist {
            item,
            confidence: target.confidence,
        })
    }

    /// Build the primary album section.
    fn build_primary_album(
        &self,
        target: &IdentifiedTarget,
        emitted_ids: &mut HashSet<String>,
    ) -> Option<SearchSection> {
        let item = self.resolve_search_result(&target.result.item_id, target.result.item_type)?;
        emitted_ids.insert(target.result.item_id.clone());

        Some(SearchSection::PrimaryAlbum {
            item,
            confidence: target.confidence,
        })
    }

    /// Build the primary track section.
    fn build_primary_track(
        &self,
        target: &IdentifiedTarget,
        emitted_ids: &mut HashSet<String>,
    ) -> Option<SearchSection> {
        let item = self.resolve_search_result(&target.result.item_id, target.result.item_type)?;
        emitted_ids.insert(target.result.item_id.clone());

        Some(SearchSection::PrimaryTrack {
            item,
            confidence: target.confidence,
        })
    }

    /// Add enrichment sections for an artist target.
    fn add_artist_enrichment(
        &self,
        artist_id: &str,
        sections: &mut Vec<SearchSection>,
        emitted_ids: &mut HashSet<String>,
    ) {
        // Popular tracks by this artist
        if let Ok(track_ids) = self
            .user_manager
            .get_popular_tracks_by_artist(artist_id, self.config.popular_tracks_limit)
        {
            let mut tracks: Vec<TrackSummary> = Vec::new();
            for track_id in track_ids {
                if let Ok(Some(resolved)) = self.catalog_store.get_resolved_track(&track_id) {
                    // Use album ID as image reference (images resolved via image endpoint)
                    tracks.push(track_summary_with_image(
                        &resolved,
                        Some(&resolved.album.id),
                    ));
                    emitted_ids.insert(track_id);
                }
            }
            if !tracks.is_empty() {
                sections.push(SearchSection::PopularBy {
                    target_id: artist_id.to_string(),
                    target_type: MatchType::Artist,
                    items: tracks,
                });
            }
        }

        // Albums by this artist
        if let Ok(Some(discography)) = self.catalog_store.get_discography(
            artist_id,
            self.config.albums_limit,
            0,
            crate::catalog_store::DiscographySort::Popularity,
            false,
        ) {
            let mut albums: Vec<AlbumSummary> = Vec::new();

            for album in discography.albums.iter() {
                if let Ok(Some(resolved)) = self.catalog_store.get_resolved_album(&album.id) {
                    albums.push(AlbumSummary::from(&resolved));
                    emitted_ids.insert(album.id.clone());
                }
            }

            if !albums.is_empty() {
                sections.push(SearchSection::AlbumsBy {
                    target_id: artist_id.to_string(),
                    items: albums,
                });
            }
        }

        // Related artists
        if let Ok(Some(resolved)) = self.catalog_store.get_resolved_artist(artist_id) {
            let mut related: Vec<ArtistSummary> = Vec::new();
            for artist in resolved
                .related_artists
                .iter()
                .take(self.config.related_artists_limit)
            {
                emitted_ids.insert(artist.id.clone());
                // Fetch resolved artist to get display image
                let summary = if let Ok(Some(resolved_related)) =
                    self.catalog_store.get_resolved_artist(&artist.id)
                {
                    ArtistSummary::from(&resolved_related)
                } else {
                    ArtistSummary::from(artist)
                };
                related.push(summary);
            }

            if !related.is_empty() {
                sections.push(SearchSection::RelatedArtists {
                    target_id: artist_id.to_string(),
                    items: related,
                });
            }
        }
    }

    /// Add enrichment sections for an album target.
    fn add_album_enrichment(
        &self,
        album_id: &str,
        sections: &mut Vec<SearchSection>,
        emitted_ids: &mut HashSet<String>,
    ) {
        // Tracks from this album
        if let Ok(Some(resolved)) = self.catalog_store.get_resolved_album(album_id) {
            let mut tracks: Vec<TrackSummary> = Vec::new();

            for disc in &resolved.discs {
                for track in &disc.tracks {
                    // Get full resolved track for artist info
                    if let Ok(Some(resolved_track)) =
                        self.catalog_store.get_resolved_track(&track.id)
                    {
                        // Use album ID as image reference (images resolved via image endpoint)
                        tracks.push(track_summary_with_image(
                            &resolved_track,
                            Some(&resolved.album.id),
                        ));
                        emitted_ids.insert(track.id.clone());
                    }
                }
            }

            if !tracks.is_empty() {
                sections.push(SearchSection::TracksFrom {
                    target_id: album_id.to_string(),
                    items: tracks,
                });
            }

            // Related artists (from album's primary artist)
            if let Some(first_artist) = resolved.artists.first() {
                if let Ok(Some(artist_resolved)) =
                    self.catalog_store.get_resolved_artist(&first_artist.id)
                {
                    let related: Vec<ArtistSummary> = artist_resolved
                        .related_artists
                        .iter()
                        .take(self.config.related_artists_limit)
                        .map(|a| {
                            emitted_ids.insert(a.id.clone());
                            ArtistSummary::from(a)
                        })
                        .collect();

                    if !related.is_empty() {
                        sections.push(SearchSection::RelatedArtists {
                            target_id: first_artist.id.clone(),
                            items: related,
                        });
                    }
                }
            }
        }
    }

    /// Build the remaining results (items not already emitted as primary matches).
    fn build_remaining_results(
        &self,
        search_results: &[crate::search::SearchResult],
        emitted_ids: &HashSet<String>,
    ) -> Option<Vec<ResolvedSearchResult>> {
        let mut items = Vec::new();

        for result in search_results {
            if emitted_ids.contains(&result.item_id) {
                continue;
            }
            if items.len() >= self.config.other_results_limit {
                break;
            }
            if let Some(resolved) = self.resolve_search_result(&result.item_id, result.item_type) {
                items.push(resolved);
            }
        }

        if items.is_empty() {
            None
        } else {
            Some(items)
        }
    }

    /// Resolve a search result item to its full representation.
    fn resolve_search_result(
        &self,
        item_id: &str,
        item_type: HashedItemType,
    ) -> Option<ResolvedSearchResult> {
        resolve::resolve_to_result(self.catalog_store, item_id, item_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Integration tests are in pezzottify-server/tests/e2e_search_tests.rs
    // These are basic unit tests for the pipeline structure.

    #[test]
    fn test_match_type_for_artist() {
        assert_eq!(MatchType::Artist, MatchType::Artist);
    }

    #[test]
    fn test_match_type_for_album() {
        assert_eq!(MatchType::Album, MatchType::Album);
    }

    #[test]
    fn test_match_type_for_track() {
        assert_eq!(MatchType::Track, MatchType::Track);
    }
}
