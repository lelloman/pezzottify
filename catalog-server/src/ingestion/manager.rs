//! Ingestion Manager - orchestrates album-first ingestion workflows.
//!
//! Album-first workflow:
//! 1. User uploads zip → extract audio files → create job + file records
//! 2. ANALYZING: Probe each file for audio metadata and embedded tags
//! 3. IDENTIFYING_ALBUM: Agent analyzes collective metadata to identify album
//! 4. AWAITING_REVIEW (if needed): Human confirms album match
//! 5. MAPPING_TRACKS: Map each file to a track in the album
//! 6. CONVERTING: Convert each file to OGG Vorbis
//! 7. COMPLETED: All done

use super::converter::{probe_audio_file, convert_to_ogg};
use super::file_handler::{FileHandler, FileHandlerError};
use super::models::{
    AlbumMetadataSummary, IngestionContextType, IngestionFile, IngestionJob,
    IngestionJobStatus, IngestionMatchSource, ReviewOption,
};
use super::store::IngestionStore;
use crate::agent::llm::LlmProvider;
use crate::catalog_store::CatalogStore;
use crate::search::{HashedItemType, SearchVault};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Errors that can occur during ingestion.
#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("Store error: {0}")]
    Store(#[from] anyhow::Error),

    #[error("File handling error: {0}")]
    FileHandler(#[from] FileHandlerError),

    #[error("Conversion error: {0}")]
    Conversion(#[from] super::converter::ConversionError),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Invalid job state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },

    #[error("No LLM provider configured")]
    NoLlmProvider,

    #[error("No files in upload")]
    NoFiles,

    #[error("Album not matched")]
    AlbumNotMatched,
}

/// Configuration for the IngestionManager.
#[derive(Clone)]
pub struct IngestionManagerConfig {
    /// Directory for temporary files.
    pub temp_dir: PathBuf,
    /// Directory for media output files.
    pub media_dir: PathBuf,
    /// Maximum file size in bytes.
    pub max_file_size: u64,
    /// Target bitrate for audio conversion (kbps).
    pub target_bitrate: u32,
    /// Maximum LLM iterations per job.
    pub max_iterations: usize,
    /// Confidence threshold for auto-matching (0.0 - 1.0).
    pub auto_match_threshold: f32,
}

impl Default for IngestionManagerConfig {
    fn default() -> Self {
        Self {
            temp_dir: PathBuf::from("/tmp/pezzottify-ingestion"),
            media_dir: PathBuf::from("media"),
            max_file_size: 500 * 1024 * 1024, // 500 MB for zip files
            target_bitrate: 320,
            max_iterations: 20,
            auto_match_threshold: 0.85,
        }
    }
}

/// Manages the album-first ingestion workflow.
pub struct IngestionManager {
    store: Arc<dyn IngestionStore>,
    catalog: Arc<dyn CatalogStore>,
    search: Arc<dyn SearchVault>,
    llm: Option<Arc<dyn LlmProvider>>,
    file_handler: FileHandler,
    config: IngestionManagerConfig,
}

impl IngestionManager {
    /// Create a new IngestionManager.
    pub fn new(
        store: Arc<dyn IngestionStore>,
        catalog: Arc<dyn CatalogStore>,
        search: Arc<dyn SearchVault>,
        llm: Option<Arc<dyn LlmProvider>>,
        config: IngestionManagerConfig,
    ) -> Self {
        let file_handler = FileHandler::new(&config.temp_dir, config.max_file_size);

        Self {
            store,
            catalog,
            search,
            llm,
            file_handler,
            config,
        }
    }

    /// Initialize the manager (creates temp directory, etc.).
    pub async fn init(&self) -> Result<()> {
        self.file_handler.init().await?;
        Ok(())
    }

    // =========================================================================
    // Job Creation
    // =========================================================================

    /// Create a new ingestion job from uploaded file bytes (zip or single audio file).
    pub async fn create_job(
        &self,
        user_id: &str,
        filename: &str,
        data: &[u8],
        context_type: IngestionContextType,
        context_id: Option<String>,
    ) -> Result<String, IngestionError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let total_size = data.len() as i64;

        // Save uploaded file to temp storage
        let temp_path = self
            .file_handler
            .save_upload(&job_id, filename, data)
            .await?;

        // Extract audio files if it's a zip, otherwise use the single file
        let audio_files = if FileHandler::is_zip(filename) {
            self.file_handler.extract_zip(&job_id, &temp_path).await?
        } else if FileHandler::is_supported_audio(filename) {
            vec![temp_path.clone()]
        } else {
            return Err(IngestionError::FileHandler(
                FileHandlerError::UnsupportedFileType(filename.to_string()),
            ));
        };

        if audio_files.is_empty() {
            return Err(IngestionError::NoFiles);
        }

        let file_count = audio_files.len() as i32;

        // Create job record
        let job = IngestionJob::new(&job_id, user_id, filename, total_size, file_count)
            .with_context(context_type, context_id);

        self.store.create_job(&job)?;

        // Create file records for each audio file
        for audio_path in &audio_files {
            let file_id = uuid::Uuid::new_v4().to_string();
            let file_name = audio_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let file_size = tokio::fs::metadata(&audio_path)
                .await
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let file = IngestionFile::new(
                &file_id,
                &job_id,
                file_name,
                file_size,
                audio_path.to_string_lossy().to_string(),
            );

            self.store.create_file(&file)?;
        }

        info!(
            "Created ingestion job {} for user {} with {} files from {}",
            job_id, user_id, file_count, filename
        );

        Ok(job_id)
    }

    // =========================================================================
    // Job Queries
    // =========================================================================

    /// Get a job by ID.
    pub fn get_job(&self, job_id: &str) -> Result<Option<IngestionJob>, IngestionError> {
        Ok(self.store.get_job(job_id)?)
    }

    /// Get files for a job.
    pub fn get_files(&self, job_id: &str) -> Result<Vec<IngestionFile>, IngestionError> {
        Ok(self.store.get_files_for_job(job_id)?)
    }

    /// List jobs for a user.
    pub fn list_user_jobs(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<IngestionJob>, IngestionError> {
        Ok(self.store.list_jobs_by_user(user_id, limit)?)
    }

    /// List all jobs (for admin).
    pub fn list_all_jobs(&self, limit: usize) -> Result<Vec<IngestionJob>, IngestionError> {
        Ok(self.store.list_all_jobs(limit)?)
    }

    // =========================================================================
    // Phase 1: Analyze Files
    // =========================================================================

    /// Analyze all files in a job - extract audio metadata and embedded tags.
    pub async fn analyze_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::Pending {
            return Err(IngestionError::InvalidState {
                expected: "PENDING".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        job.status = IngestionJobStatus::Analyzing;
        job.started_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(&job)?;

        let files = self.store.get_files_for_job(job_id)?;

        for mut file in files {
            // Probe audio metadata
            let path = Path::new(&file.temp_file_path);
            match probe_audio_file(path).await {
                Ok(metadata) => {
                    file.duration_ms = Some(metadata.duration_ms);
                    file.codec = Some(metadata.codec);
                    file.bitrate = metadata.bitrate;
                    file.sample_rate = metadata.sample_rate;
                }
                Err(e) => {
                    warn!("Failed to probe {}: {}", file.filename, e);
                }
            }

            // Extract embedded tags using ffprobe
            if let Ok(tags) = self.extract_tags(path).await {
                file.tag_artist = tags.get("artist").cloned();
                file.tag_album = tags.get("album").cloned();
                file.tag_title = tags.get("title").cloned();
                file.tag_track_num = tags
                    .get("track")
                    .and_then(|t| t.split('/').next())
                    .and_then(|t| t.parse().ok());
                file.tag_track_total = tags
                    .get("track")
                    .and_then(|t| t.split('/').nth(1))
                    .and_then(|t| t.parse().ok());
                file.tag_disc_num = tags
                    .get("disc")
                    .and_then(|d| d.split('/').next())
                    .and_then(|d| d.parse().ok());
                file.tag_year = tags
                    .get("date")
                    .and_then(|d| d.get(..4))
                    .and_then(|y| y.parse().ok());
            }

            self.store.update_file(&file)?;
        }

        // Aggregate detected metadata
        let summary = self.build_metadata_summary(job_id)?;
        job.detected_artist = summary.artist;
        job.detected_album = summary.album;
        job.detected_year = summary.year;
        job.status = IngestionJobStatus::IdentifyingAlbum;
        self.store.update_job(&job)?;

        info!("Analyzed job {} - detected: {:?} - {:?}", job_id, job.detected_artist, job.detected_album);

        Ok(())
    }

    /// Extract embedded tags from an audio file using ffprobe.
    async fn extract_tags(&self, path: &Path) -> Result<HashMap<String, String>> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
            ])
            .arg(path)
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("ffprobe failed");
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut tags = HashMap::new();

        if let Some(format_tags) = json.get("format").and_then(|f| f.get("tags")) {
            if let Some(obj) = format_tags.as_object() {
                for (key, value) in obj {
                    if let Some(v) = value.as_str() {
                        tags.insert(key.to_lowercase(), v.to_string());
                    }
                }
            }
        }

        Ok(tags)
    }

    /// Build aggregated metadata summary from all files in a job.
    fn build_metadata_summary(&self, job_id: &str) -> Result<AlbumMetadataSummary, IngestionError> {
        let job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        let files = self.store.get_files_for_job(job_id)?;

        // Count occurrences of each value to find most common
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut album_counts: HashMap<String, usize> = HashMap::new();
        let mut years: Vec<i32> = Vec::new();
        let mut total_duration_ms: i64 = 0;
        let mut track_titles: Vec<(Option<i32>, String)> = Vec::new();

        for file in &files {
            if let Some(artist) = &file.tag_artist {
                *artist_counts.entry(artist.clone()).or_insert(0) += 1;
            }
            if let Some(album) = &file.tag_album {
                *album_counts.entry(album.clone()).or_insert(0) += 1;
            }
            if let Some(year) = file.tag_year {
                years.push(year);
            }
            if let Some(duration) = file.duration_ms {
                total_duration_ms += duration;
            }
            let title = file
                .tag_title
                .clone()
                .unwrap_or_else(|| file.filename.clone());
            track_titles.push((file.tag_track_num, title));
        }

        // Sort tracks by track number
        track_titles.sort_by_key(|(num, _)| num.unwrap_or(999));
        let track_titles: Vec<String> = track_titles.into_iter().map(|(_, t)| t).collect();

        // Get most common values
        let artist = artist_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name);

        let album = album_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name);

        // Use most common year
        let year = if !years.is_empty() {
            let mut year_counts: HashMap<i32, usize> = HashMap::new();
            for y in years {
                *year_counts.entry(y).or_insert(0) += 1;
            }
            year_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(y, _)| y)
        } else {
            None
        };

        Ok(AlbumMetadataSummary {
            artist,
            album,
            year,
            file_count: files.len() as i32,
            total_duration_ms,
            track_titles,
            source_name: job.original_filename,
        })
    }

    // =========================================================================
    // Phase 2: Identify Album (with LLM or heuristics)
    // =========================================================================

    /// Process a job in IDENTIFYING_ALBUM state - search catalog and find matching album.
    pub async fn process_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        // Handle different starting states
        match job.status {
            IngestionJobStatus::Pending => {
                // First analyze, then continue
                self.analyze_job(job_id).await?;
                job = self.store.get_job(job_id)?.unwrap();
            }
            IngestionJobStatus::IdentifyingAlbum => {
                // Continue with album identification
            }
            _ => {
                return Err(IngestionError::InvalidState {
                    expected: "PENDING or IDENTIFYING_ALBUM".to_string(),
                    actual: job.status.as_str().to_string(),
                });
            }
        }

        // Get metadata summary
        let summary = self.build_metadata_summary(job_id)?;

        // Search for matching albums in catalog
        let candidates = self.search_album_candidates(&summary).await?;

        if candidates.is_empty() {
            // No candidates found
            job.status = IngestionJobStatus::AwaitingReview;
            self.create_no_match_review(&job, &summary)?;
            self.store.update_job(&job)?;
            return Ok(());
        }

        // Score candidates
        let scored: Vec<(String, String, f32)> = candidates
            .into_iter()
            .map(|(album_id, album_name, artist_name)| {
                let score = self.score_album_match(&summary, &album_name, &artist_name);
                (album_id, format!("{} - {}", artist_name, album_name), score)
            })
            .collect();

        // Find best match
        let best = scored.iter().max_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

        if let Some((album_id, label, confidence)) = best {
            if *confidence >= self.config.auto_match_threshold {
                // High confidence - auto-match
                job.matched_album_id = Some(album_id.clone());
                job.match_confidence = Some(*confidence);
                job.match_source = Some(IngestionMatchSource::Agent);
                job.status = IngestionJobStatus::MappingTracks;
                self.store.update_job(&job)?;

                info!(
                    "Auto-matched job {} to album {} with {:.0}% confidence",
                    job_id, album_id, confidence * 100.0
                );
            } else {
                // Low confidence - request review
                let options: Vec<ReviewOption> = scored
                    .iter()
                    .take(5)
                    .map(|(id, label, conf)| ReviewOption {
                        id: format!("album:{}", id),
                        label: format!("{} ({:.0}%)", label, conf * 100.0),
                        description: None,
                    })
                    .chain(std::iter::once(ReviewOption {
                        id: "no_match".to_string(),
                        label: "None of these".to_string(),
                        description: Some("Album not in catalog".to_string()),
                    }))
                    .collect();

                let question = format!(
                    "Which album is this?\nDetected: {} - {}",
                    summary.artist.as_deref().unwrap_or("Unknown Artist"),
                    summary.album.as_deref().unwrap_or("Unknown Album")
                );

                let options_json = serde_json::to_string(&options).unwrap_or_default();
                self.store.create_review_item(job_id, &question, &options_json)?;

                job.status = IngestionJobStatus::AwaitingReview;
                self.store.update_job(&job)?;

                info!(
                    "Job {} requires review - best match: {} ({:.0}%)",
                    job_id, label, confidence * 100.0
                );
            }
        }

        Ok(())
    }

    /// Search catalog for album candidates matching the summary.
    async fn search_album_candidates(
        &self,
        summary: &AlbumMetadataSummary,
    ) -> Result<Vec<(String, String, String)>, IngestionError> {
        let mut candidates = Vec::new();

        // Build search query from artist + album
        let query = format!(
            "{} {}",
            summary.artist.as_deref().unwrap_or(""),
            summary.album.as_deref().unwrap_or("")
        )
        .trim()
        .to_string();

        // Search for albums (filter to albums only)
        let search_query = if query.is_empty() {
            summary.source_name.clone()
        } else {
            query
        };

        let album_filter = Some(vec![HashedItemType::Album]);
        let results = self.search.search(&search_query, 10, album_filter);

        for result in results {
            if let Ok(Some(album_json)) = self.catalog.get_resolved_album_json(&result.item_id) {
                // Extract album info from JSON
                if let (Some(album), Some(artists)) = (
                    album_json.get("album"),
                    album_json.get("artists").and_then(|a| a.as_array()),
                ) {
                    let album_id = album.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    let album_name = album.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                    let artist_name = artists
                        .first()
                        .and_then(|a| a.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if !album_id.is_empty() {
                        candidates.push((
                            album_id.to_string(),
                            album_name.to_string(),
                            artist_name.to_string(),
                        ));
                    }
                }
            }
        }

        // Also search for artists and include their albums
        if !search_query.is_empty() && summary.artist.is_some() {
            let artist_filter = Some(vec![HashedItemType::Artist]);
            let artist_results = self.search.search(&search_query, 5, artist_filter);

            for result in artist_results {
                if let Ok(Some(artist_json)) = self.catalog.get_resolved_artist_json(&result.item_id) {
                    let artist_name = artist_json
                        .get("artist")
                        .and_then(|a| a.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if let Some(albums) = artist_json.get("albums").and_then(|a| a.as_array()) {
                        for album in albums.iter().take(5) {
                            let album_id = album.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                            let album_name = album.get("name").and_then(|v| v.as_str()).unwrap_or_default();

                            if !album_id.is_empty() {
                                candidates.push((
                                    album_id.to_string(),
                                    album_name.to_string(),
                                    artist_name.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        candidates.dedup_by(|a, b| a.0 == b.0);

        Ok(candidates)
    }

    /// Score how well an album matches the detected metadata.
    fn score_album_match(&self, summary: &AlbumMetadataSummary, album_name: &str, artist_name: &str) -> f32 {
        let mut score = 0.0;
        let mut factors = 0.0;

        // Artist similarity
        if let Some(detected_artist) = &summary.artist {
            let sim = string_similarity(detected_artist, artist_name);
            score += sim * 0.4; // 40% weight
            factors += 0.4;
        }

        // Album name similarity
        if let Some(detected_album) = &summary.album {
            let sim = string_similarity(detected_album, album_name);
            score += sim * 0.4; // 40% weight
            factors += 0.4;
        }

        // Source filename similarity (lower weight)
        let source_sim = string_similarity(&summary.source_name, &format!("{} - {}", artist_name, album_name));
        score += source_sim * 0.2; // 20% weight
        factors += 0.2;

        if factors > 0.0 {
            score / factors
        } else {
            0.0
        }
    }

    fn create_no_match_review(&self, job: &IngestionJob, summary: &AlbumMetadataSummary) -> Result<(), IngestionError> {
        let question = format!(
            "Could not find album in catalog.\nDetected: {} - {}\nFilename: {}",
            summary.artist.as_deref().unwrap_or("Unknown Artist"),
            summary.album.as_deref().unwrap_or("Unknown Album"),
            job.original_filename
        );

        let options = vec![
            ReviewOption {
                id: "retry".to_string(),
                label: "Search again".to_string(),
                description: None,
            },
            ReviewOption {
                id: "no_match".to_string(),
                label: "Album not in catalog".to_string(),
                description: Some("Mark as failed".to_string()),
            },
        ];

        let options_json = serde_json::to_string(&options).unwrap_or_default();
        self.store.create_review_item(&job.id, &question, &options_json)?;

        Ok(())
    }

    // =========================================================================
    // Phase 3: Map Tracks
    // =========================================================================

    /// Map files to tracks within the matched album.
    pub async fn map_tracks(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::MappingTracks {
            return Err(IngestionError::InvalidState {
                expected: "MAPPING_TRACKS".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        let album_id = job
            .matched_album_id
            .as_ref()
            .ok_or(IngestionError::AlbumNotMatched)?;

        // Get album with tracks via resolved JSON
        let album_json = self
            .catalog
            .get_resolved_album_json(album_id)?
            .ok_or_else(|| IngestionError::Store(anyhow::anyhow!("Album not found: {}", album_id)))?;

        // Parse tracks from discs array
        struct TrackInfo {
            id: String,
            name: String,
            track_number: i32,
            disc_number: i32,
        }

        let mut tracks: Vec<TrackInfo> = Vec::new();
        if let Some(discs) = album_json.get("discs").and_then(|d| d.as_array()) {
            for disc in discs {
                let disc_number = disc.get("number").and_then(|n| n.as_i64()).unwrap_or(1) as i32;
                if let Some(disc_tracks) = disc.get("tracks").and_then(|t| t.as_array()) {
                    for track in disc_tracks {
                        let id = track.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                        let name = track.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                        let track_number = track.get("track_number").and_then(|n| n.as_i64()).unwrap_or(0) as i32;

                        if !id.is_empty() {
                            tracks.push(TrackInfo {
                                id: id.to_string(),
                                name: name.to_string(),
                                track_number,
                                disc_number,
                            });
                        }
                    }
                }
            }
        }

        let mut files = self.store.get_files_for_job(job_id)?;

        info!(
            "Mapping {} files to {} tracks in album {}",
            files.len(),
            tracks.len(),
            album_id
        );

        // Build track lookup by (disc_number, track_number)
        let tracks_by_num: HashMap<(i32, i32), &TrackInfo> = tracks
            .iter()
            .map(|t| ((t.disc_number, t.track_number), t))
            .collect();

        let mut matched = 0;

        for file in &mut files {
            // Try to match by disc + track number first
            let disc_num = file.tag_disc_num.unwrap_or(1);
            if let Some(track_num) = file.tag_track_num {
                if let Some(track) = tracks_by_num.get(&(disc_num, track_num)) {
                    file.matched_track_id = Some(track.id.clone());
                    file.match_confidence = Some(1.0);
                    matched += 1;
                    self.store.update_file(file)?;
                    continue;
                }
            }

            // Fall back to title matching
            if let Some(title) = &file.tag_title {
                let best_match = tracks
                    .iter()
                    .map(|t| (t, string_similarity(title, &t.name)))
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                if let Some((track, confidence)) = best_match {
                    if confidence > 0.7 {
                        file.matched_track_id = Some(track.id.clone());
                        file.match_confidence = Some(confidence);
                        matched += 1;
                        self.store.update_file(file)?;
                    }
                }
            }
        }

        job.tracks_matched = matched;
        job.status = IngestionJobStatus::Converting;
        self.store.update_job(&job)?;

        info!("Mapped {}/{} files for job {}", matched, files.len(), job_id);

        Ok(())
    }

    // =========================================================================
    // Phase 4: Convert Files
    // =========================================================================

    /// Convert all matched files to OGG Vorbis.
    pub async fn convert_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::Converting {
            return Err(IngestionError::InvalidState {
                expected: "CONVERTING".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        let files = self.store.get_files_for_job(job_id)?;
        let mut converted = 0;

        for mut file in files {
            // Skip files without a matched track
            let track_id = match &file.matched_track_id {
                Some(id) => id.clone(),
                None => {
                    debug!("Skipping file {} - no matched track", file.filename);
                    continue;
                }
            };

            let input_path = Path::new(&file.temp_file_path);
            let output_path = self.file_handler.get_output_path(&self.config.media_dir, &track_id);

            match convert_to_ogg(input_path, &output_path, self.config.target_bitrate).await {
                Ok(()) => {
                    file.output_file_path = Some(output_path.to_string_lossy().to_string());
                    file.converted = true;
                    converted += 1;
                    info!("Converted {} -> {}", file.filename, track_id);
                }
                Err(e) => {
                    error!("Failed to convert {}: {}", file.filename, e);
                    file.error_message = Some(e.to_string());
                }
            }

            self.store.update_file(&file)?;
        }

        job.tracks_converted = converted;
        job.status = IngestionJobStatus::Completed;
        job.completed_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(&job)?;

        // Cleanup temp files
        let _ = self.file_handler.cleanup_job(job_id).await;

        info!(
            "Completed job {} - converted {}/{} tracks",
            job_id, converted, job.tracks_matched
        );

        Ok(())
    }

    // =========================================================================
    // Review Handling
    // =========================================================================

    /// Resolve a review and continue processing.
    pub async fn resolve_review(
        &self,
        job_id: &str,
        reviewer_user_id: &str,
        selected_option: &str,
    ) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::AwaitingReview {
            return Err(IngestionError::InvalidState {
                expected: "AWAITING_REVIEW".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        self.store.resolve_review(job_id, reviewer_user_id, selected_option)?;

        if selected_option.starts_with("album:") {
            let album_id = selected_option.trim_start_matches("album:");
            job.matched_album_id = Some(album_id.to_string());
            job.match_confidence = Some(1.0);
            job.match_source = Some(IngestionMatchSource::HumanReview);
            job.status = IngestionJobStatus::MappingTracks;
            self.store.update_job(&job)?;

            info!("Review resolved: job {} matched to album {}", job_id, album_id);
        } else if selected_option == "no_match" {
            job.status = IngestionJobStatus::Failed;
            job.error_message = Some("Album not in catalog".to_string());
            job.completed_at = Some(chrono::Utc::now().timestamp_millis());
            self.store.update_job(&job)?;
        } else if selected_option == "retry" {
            job.status = IngestionJobStatus::IdentifyingAlbum;
            self.store.update_job(&job)?;
        }

        Ok(())
    }

    /// Get pending review items.
    pub fn get_pending_reviews(
        &self,
        limit: usize,
    ) -> Result<Vec<super::models::ReviewQueueItem>, IngestionError> {
        Ok(self.store.get_pending_reviews(limit)?)
    }
}

// =========================================================================
// Utilities
// =========================================================================

/// Calculate string similarity (0.0 to 1.0).
fn string_similarity(a: &str, b: &str) -> f32 {
    let a = a.to_lowercase();
    let b = b.to_lowercase();

    if a == b {
        return 1.0;
    }

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Use Levenshtein distance
    let distance = levenshtein_distance(&a, &b);
    let max_len = a.len().max(b.len());

    1.0 - (distance as f32 / max_len as f32)
}

/// Calculate Levenshtein distance between two strings.
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
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
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
    fn test_config_defaults() {
        let config = IngestionManagerConfig::default();
        assert_eq!(config.target_bitrate, 320);
        assert_eq!(config.max_iterations, 20);
        assert!((config.auto_match_threshold - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_string_similarity() {
        assert!((string_similarity("Abbey Road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!((string_similarity("abbey road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!(string_similarity("Abbey Road", "The Beatles") < 0.5);
        // "Abbey Rd" vs "Abbey Road": distance=2, max_len=10 -> 0.8 similarity
        assert!(string_similarity("Abbey Rd", "Abbey Road") >= 0.8);
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
