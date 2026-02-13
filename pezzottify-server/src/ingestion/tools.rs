//! Agent tools for ingestion workflow.

use crate::agent::tools::{AgentTool, AgentToolRegistry, ToolContext, ToolDefinition, ToolError};
use crate::catalog_store::CatalogStore;
use crate::search::{HashedItemType, SearchVault};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// Create the tool registry for ingestion workflows.
pub fn create_ingestion_tools(
    catalog: Arc<dyn CatalogStore>,
    search: Arc<dyn SearchVault>,
) -> AgentToolRegistry {
    let mut registry = AgentToolRegistry::new();

    registry.register(SearchCatalogTool::new(search.clone(), catalog.clone()));
    registry.register(GetAlbumDetailsTool::new(catalog.clone()));
    registry.register(GetTrackDetailsTool::new(catalog.clone()));
    registry.register(GetArtistDetailsTool::new(catalog.clone()));
    registry.register(CompareMetadataTool);
    registry.register(ProposeMatchTool);
    registry.register(RequestReviewTool);
    registry.register(MarkNoMatchTool);

    registry
}

/// Tool to search the catalog for albums, tracks, or artists.
struct SearchCatalogTool {
    search: Arc<dyn SearchVault>,
    catalog: Arc<dyn CatalogStore>,
}

impl SearchCatalogTool {
    fn new(search: Arc<dyn SearchVault>, catalog: Arc<dyn CatalogStore>) -> Self {
        Self { search, catalog }
    }
}

#[async_trait]
impl AgentTool for SearchCatalogTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_catalog".to_string(),
            description: "Search the music catalog for albums, tracks, or artists by name. Use this to find potential matches for uploaded audio files.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (artist name, album title, track title, or combination)"
                    },
                    "search_type": {
                        "type": "string",
                        "enum": ["all", "albums", "tracks", "artists"],
                        "description": "Type of content to search for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'query' parameter".to_string()))?;

        let search_type = args
            .get("search_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        // Map search_type to filter
        let filter = match search_type {
            "albums" => Some(vec![HashedItemType::Album]),
            "tracks" => Some(vec![HashedItemType::Track]),
            "artists" => Some(vec![HashedItemType::Artist]),
            _ => None, // "all" - no filter
        };

        // Search using the SearchVault
        let results = self.search.search(query, limit, filter);

        // Resolve the results to get full details
        let mut albums = vec![];
        let mut tracks = vec![];
        let mut artists = vec![];

        for result in results {
            match result.item_type {
                HashedItemType::Album => {
                    if let Ok(Some(resolved)) = self.catalog.get_resolved_album(&result.item_id) {
                        // Count tracks across all discs
                        let track_count: usize =
                            resolved.discs.iter().map(|d| d.tracks.len()).sum();
                        albums.push(json!({
                            "id": resolved.album.id,
                            "name": resolved.album.name,
                            "artist_name": resolved.artists.first().map(|a| &a.name),
                            "release_date": resolved.album.release_date,
                            "track_count": track_count,
                            "score": result.score
                        }));
                    }
                }
                HashedItemType::Track => {
                    if let Ok(Some(resolved)) = self.catalog.get_resolved_track(&result.item_id) {
                        tracks.push(json!({
                            "id": resolved.track.id,
                            "name": resolved.track.name,
                            "album_name": resolved.album.name,
                            "artist_name": resolved.artists.first().map(|a| &a.artist.name),
                            "duration_ms": resolved.track.duration_ms,
                            "track_number": resolved.track.track_number,
                            "score": result.score
                        }));
                    }
                }
                HashedItemType::Artist => {
                    if let Ok(Some(resolved)) = self.catalog.get_resolved_artist(&result.item_id) {
                        artists.push(json!({
                            "id": resolved.artist.id,
                            "name": resolved.artist.name,
                            "score": result.score
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "query": query,
            "albums": albums,
            "tracks": tracks,
            "artists": artists
        }))
    }
}

/// Tool to get detailed album information.
struct GetAlbumDetailsTool {
    catalog: Arc<dyn CatalogStore>,
}

impl GetAlbumDetailsTool {
    fn new(catalog: Arc<dyn CatalogStore>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl AgentTool for GetAlbumDetailsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_album_details".to_string(),
            description: "Get detailed information about an album including all its tracks. Use this after search to verify a potential match.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "album_id": {
                        "type": "string",
                        "description": "The album ID to look up"
                    }
                },
                "required": ["album_id"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let album_id = args
            .get("album_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments("Missing 'album_id' parameter".to_string())
            })?;

        let resolved = self
            .catalog
            .get_resolved_album(album_id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Album '{}' not found", album_id)))?;

        // Flatten tracks from all discs
        let track_list: Vec<Value> = resolved
            .discs
            .iter()
            .flat_map(|disc| {
                disc.tracks.iter().map(|t| {
                    json!({
                        "id": t.id,
                        "name": t.name,
                        "track_number": t.track_number,
                        "disc_number": t.disc_number,
                        "duration_ms": t.duration_ms,
                        "has_audio": t.audio_uri.is_some()
                    })
                })
            })
            .collect();

        let track_count: usize = resolved.discs.iter().map(|d| d.tracks.len()).sum();

        Ok(json!({
            "id": resolved.album.id,
            "name": resolved.album.name,
            "artist": {
                "id": resolved.artists.first().map(|a| &a.id),
                "name": resolved.artists.first().map(|a| &a.name)
            },
            "release_date": resolved.album.release_date,
            "track_count": track_count,
            "tracks": track_list
        }))
    }
}

/// Tool to get track details.
struct GetTrackDetailsTool {
    catalog: Arc<dyn CatalogStore>,
}

impl GetTrackDetailsTool {
    fn new(catalog: Arc<dyn CatalogStore>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl AgentTool for GetTrackDetailsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_track_details".to_string(),
            description: "Get detailed information about a specific track.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "track_id": {
                        "type": "string",
                        "description": "The track ID to look up"
                    }
                },
                "required": ["track_id"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let track_id = args
            .get("track_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments("Missing 'track_id' parameter".to_string())
            })?;

        let resolved = self
            .catalog
            .get_resolved_track(track_id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Track '{}' not found", track_id)))?;

        Ok(json!({
            "id": resolved.track.id,
            "name": resolved.track.name,
            "track_number": resolved.track.track_number,
            "disc_number": resolved.track.disc_number,
            "duration_ms": resolved.track.duration_ms,
            "has_audio": resolved.track.audio_uri.is_some(),
            "album": {
                "id": resolved.album.id,
                "name": resolved.album.name,
                "release_date": resolved.album.release_date
            },
            "artist_name": resolved.artists.first().map(|a| &a.artist.name)
        }))
    }
}

/// Tool to get artist details including discography.
struct GetArtistDetailsTool {
    catalog: Arc<dyn CatalogStore>,
}

impl GetArtistDetailsTool {
    fn new(catalog: Arc<dyn CatalogStore>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl AgentTool for GetArtistDetailsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_artist_details".to_string(),
            description: "Get detailed information about an artist including their albums."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "artist_id": {
                        "type": "string",
                        "description": "The artist ID to look up"
                    }
                },
                "required": ["artist_id"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let artist_id = args
            .get("artist_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments("Missing 'artist_id' parameter".to_string())
            })?;

        let resolved = self
            .catalog
            .get_resolved_artist(artist_id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Artist '{}' not found", artist_id))
            })?;

        // Get discography
        let discography = self
            .catalog
            .get_discography(
                artist_id,
                50,                                               // limit
                0,                                                // offset
                crate::catalog_store::DiscographySort::default(), // Use default (Popularity)
                false,                                            // appears_on
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let album_list: Vec<Value> = discography
            .map(|d| {
                d.albums
                    .into_iter()
                    .map(|a| {
                        json!({
                            "id": a.id,
                            "name": a.name,
                            "release_date": a.release_date,
                            "album_type": format!("{:?}", a.album_type)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(json!({
            "id": resolved.artist.id,
            "name": resolved.artist.name,
            "album_count": album_list.len(),
            "albums": album_list
        }))
    }
}

/// Tool to compare uploaded file metadata with catalog track.
struct CompareMetadataTool;

#[async_trait]
impl AgentTool for CompareMetadataTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compare_metadata".to_string(),
            description: "Compare metadata from an uploaded file with a catalog track to assess match quality. Returns a similarity score and details.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_metadata": {
                        "type": "object",
                        "description": "Metadata from the uploaded file",
                        "properties": {
                            "duration_ms": { "type": "integer" },
                            "filename": { "type": "string" },
                            "title_tag": { "type": "string" },
                            "artist_tag": { "type": "string" },
                            "album_tag": { "type": "string" },
                            "track_number_tag": { "type": "integer" }
                        }
                    },
                    "catalog_track": {
                        "type": "object",
                        "description": "Track from the catalog",
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" },
                            "duration_ms": { "type": "integer" },
                            "track_number": { "type": "integer" },
                            "album_name": { "type": "string" },
                            "artist_name": { "type": "string" }
                        }
                    }
                },
                "required": ["file_metadata", "catalog_track"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let file_meta = args
            .get("file_metadata")
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'file_metadata'".to_string()))?;
        let catalog_track = args
            .get("catalog_track")
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'catalog_track'".to_string()))?;

        let mut scores = vec![];
        let mut details = vec![];

        // Duration comparison (within 5 seconds is good, within 10 is acceptable)
        if let (Some(file_dur), Some(cat_dur)) = (
            file_meta.get("duration_ms").and_then(|v| v.as_i64()),
            catalog_track.get("duration_ms").and_then(|v| v.as_i64()),
        ) {
            let diff_ms = (file_dur - cat_dur).abs();
            let duration_score = if diff_ms < 5000 {
                1.0
            } else if diff_ms < 10000 {
                0.7
            } else if diff_ms < 30000 {
                0.3
            } else {
                0.0
            };
            scores.push(("duration", duration_score, 0.4));
            details.push(json!({
                "field": "duration",
                "file_value": file_dur,
                "catalog_value": cat_dur,
                "diff_ms": diff_ms,
                "score": duration_score
            }));
        }

        // Title comparison
        if let (Some(file_title), Some(cat_title)) = (
            file_meta.get("title_tag").and_then(|v| v.as_str()),
            catalog_track.get("name").and_then(|v| v.as_str()),
        ) {
            let title_score = string_similarity(file_title, cat_title);
            scores.push(("title", title_score, 0.3));
            details.push(json!({
                "field": "title",
                "file_value": file_title,
                "catalog_value": cat_title,
                "score": title_score
            }));
        }

        // Track number comparison
        if let (Some(file_num), Some(cat_num)) = (
            file_meta.get("track_number_tag").and_then(|v| v.as_i64()),
            catalog_track.get("track_number").and_then(|v| v.as_i64()),
        ) {
            let num_score = if file_num == cat_num { 1.0 } else { 0.0 };
            scores.push(("track_number", num_score, 0.15));
            details.push(json!({
                "field": "track_number",
                "file_value": file_num,
                "catalog_value": cat_num,
                "score": num_score
            }));
        }

        // Artist comparison
        if let (Some(file_artist), Some(cat_artist)) = (
            file_meta.get("artist_tag").and_then(|v| v.as_str()),
            catalog_track.get("artist_name").and_then(|v| v.as_str()),
        ) {
            let artist_score = string_similarity(file_artist, cat_artist);
            scores.push(("artist", artist_score, 0.15));
            details.push(json!({
                "field": "artist",
                "file_value": file_artist,
                "catalog_value": cat_artist,
                "score": artist_score
            }));
        }

        // Calculate weighted average
        let total_weight: f64 = scores.iter().map(|(_, _, w)| w).sum();
        let weighted_sum: f64 = scores.iter().map(|(_, s, w)| s * w).sum();
        let overall_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        let confidence = if overall_score >= 0.85 {
            "high"
        } else if overall_score >= 0.6 {
            "medium"
        } else {
            "low"
        };

        Ok(json!({
            "overall_score": overall_score,
            "confidence": confidence,
            "details": details,
            "recommendation": if overall_score >= 0.85 {
                "Auto-match recommended"
            } else if overall_score >= 0.6 {
                "Review recommended"
            } else {
                "Likely not a match"
            }
        }))
    }
}

/// Tool to propose a match for an uploaded file.
struct ProposeMatchTool;

#[async_trait]
impl AgentTool for ProposeMatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "propose_match".to_string(),
            description: "Propose that an uploaded file matches a specific track in the catalog. Use this when confident about the match.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "track_id": {
                        "type": "string",
                        "description": "The catalog track ID this file matches"
                    },
                    "confidence": {
                        "type": "number",
                        "description": "Confidence score (0.0 to 1.0)"
                    },
                    "reasoning": {
                        "type": "string",
                        "description": "Brief explanation of why this match is proposed"
                    }
                },
                "required": ["track_id", "confidence", "reasoning"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let track_id = args
            .get("track_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'track_id'".to_string()))?;

        let confidence = args
            .get("confidence")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'confidence'".to_string()))?;

        let reasoning = args
            .get("reasoning")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'reasoning'".to_string()))?;

        // This tool just returns the proposal - the workflow executor handles the actual state update
        Ok(json!({
            "action": "propose_match",
            "track_id": track_id,
            "confidence": confidence,
            "reasoning": reasoning,
            "requires_review": confidence < 0.8
        }))
    }
}

/// Tool to request human review.
struct RequestReviewTool;

#[async_trait]
impl AgentTool for RequestReviewTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "request_review".to_string(),
            description: "Request human review when uncertain about a match. Provide candidate options for the reviewer to choose from.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the reviewer"
                    },
                    "options": {
                        "type": "array",
                        "description": "Possible options for the reviewer",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "label": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["id", "label"]
                        }
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context to help the reviewer"
                    }
                },
                "required": ["question", "options"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let question = args
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'question'".to_string()))?;

        let options = args
            .get("options")
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'options'".to_string()))?;

        let context = args.get("context").and_then(|v| v.as_str());

        Ok(json!({
            "action": "request_review",
            "question": question,
            "options": options,
            "context": context
        }))
    }
}

/// Tool to mark that no match was found.
struct MarkNoMatchTool;

#[async_trait]
impl AgentTool for MarkNoMatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "mark_no_match".to_string(),
            description: "Mark that the uploaded file does not match any track in the catalog. Use when certain there is no match after thorough search.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "reasoning": {
                        "type": "string",
                        "description": "Explanation of why no match was found"
                    },
                    "suggestions": {
                        "type": "array",
                        "description": "Optional suggestions (e.g., 'Track may not be in catalog', 'Try different search terms')",
                        "items": { "type": "string" }
                    }
                },
                "required": ["reasoning"]
            }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let reasoning = args
            .get("reasoning")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'reasoning'".to_string()))?;

        let suggestions = args
            .get("suggestions")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        Ok(json!({
            "action": "no_match",
            "reasoning": reasoning,
            "suggestions": suggestions
        }))
    }
}

/// Simple string similarity (case-insensitive containment and edit distance).
fn string_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match
    if a_lower == b_lower {
        return 1.0;
    }

    // One contains the other
    if a_lower.contains(&b_lower) || b_lower.contains(&a_lower) {
        return 0.8;
    }

    // Levenshtein-based similarity
    let max_len = a_lower.len().max(b_lower.len()) as f64;
    if max_len == 0.0 {
        return 1.0;
    }

    let distance = levenshtein_distance(&a_lower, &b_lower) as f64;
    let similarity = 1.0 - (distance / max_len);

    similarity.max(0.0)
}

/// Calculate Levenshtein edit distance.
#[allow(clippy::needless_range_loop)]
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_similarity() {
        assert_eq!(string_similarity("Hello", "hello"), 1.0);
        assert!(string_similarity("Hello World", "Hello") >= 0.8);
        assert!(string_similarity("test", "testing") >= 0.5);
        assert!(string_similarity("abc", "xyz") < 0.5);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }
}
