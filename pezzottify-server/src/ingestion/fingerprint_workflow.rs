impl IngestionManager {
    /// Run fingerprint matching for a job and update its ticket type.
    pub async fn run_fingerprint_match(
        &self,
        job_id: &str,
    ) -> Result<FingerprintMatchResult, IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        // Get all files and their durations
        let files = self.store.get_files_for_job(job_id)?;

        // Extract durations (need to analyze if not already done)
        let mut durations: Vec<(i32, i64)> = Vec::with_capacity(files.len());

        for file in &files {
            // Get track number and duration
            let track_num = file.tag_track_num.unwrap_or(0);
            let duration = match file.duration_ms {
                Some(d) => d,
                None => {
                    // Need to probe the file
                    let path = Path::new(&file.temp_file_path);
                    let metadata = probe_audio_file(path).await?;
                    metadata.duration_ms as i64
                }
            };
            durations.push((track_num, duration));
        }

        // Sort by track number to ensure correct order
        durations.sort_by_key(|(track_num, _)| *track_num);
        let ordered_durations: Vec<i64> = durations.into_iter().map(|(_, d)| d).collect();

        // Run fingerprint matching
        let config = FingerprintConfig::default();
        let result =
            match_album_with_fallbacks(&ordered_durations, self.catalog.as_ref(), &config)?;

        // Update job with fingerprint results
        job.ticket_type = Some(result.ticket_type);
        job.match_score = Some(result.match_score);
        job.match_delta_ms = Some(result.total_delta_ms);

        if let Some(ref album) = result.matched_album {
            job.matched_album_id = Some(album.id.clone());
            job.match_confidence = Some(result.match_score);
            job.match_source = Some(IngestionMatchSource::Fingerprint);

            // Update detected metadata from the matched album
            job.detected_album = Some(album.name.clone());
            job.detected_artist = Some(album.artist_name.clone());
        }

        // Update status based on ticket type
        match result.ticket_type {
            TicketType::Success => {
                // Auto-matched, proceed to track mapping
                job.status = IngestionJobStatus::MappingTracks;
            }
            TicketType::Review => {
                // Needs human review
                job.status = IngestionJobStatus::AwaitingReview;
                // Create review item with top candidates
                self.create_fingerprint_review(&job, &result.candidates)?;
            }
            TicketType::Failure => {
                // No match - needs manual resolution
                job.status = IngestionJobStatus::AwaitingReview;
                self.create_failure_review(&job)?;
            }
        }

        self.store.update_job(&job)?;

        info!(
            job_id = %job_id,
            ticket_type = ?result.ticket_type,
            match_score = result.match_score,
            delta_ms = result.total_delta_ms,
            "Fingerprint matching complete"
        );

        Ok(result)
    }

    /// Create a review item for fingerprint match candidates.
    fn create_fingerprint_review(
        &self,
        job: &IngestionJob,
        candidates: &[ScoredCandidate],
    ) -> Result<(), IngestionError> {
        let options: Vec<ReviewOption> = candidates
            .iter()
            .map(|c| ReviewOption {
                id: format!("album:{}", c.album.id),
                label: format!("{} - {}", c.album.artist_name, c.album.name),
                description: Some(format!(
                    "Match: {:.0}%, Delta: {}ms, {} tracks",
                    c.score * 100.0,
                    c.delta_ms,
                    c.album.track_count
                )),
            })
            .collect();

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        self.store.create_review_item(
            &job.id,
            "Multiple album candidates found. Please select the correct album:",
            &options_json,
        )?;

        Ok(())
    }

    /// Create a review item for failed fingerprint match.
    fn create_failure_review(&self, job: &IngestionJob) -> Result<(), IngestionError> {
        let options = vec![
            ReviewOption {
                id: "search".to_string(),
                label: "Search manually".to_string(),
                description: Some("Search the catalog for the correct album".to_string()),
            },
            ReviewOption {
                id: "dismiss".to_string(),
                label: "Dismiss upload".to_string(),
                description: Some("Reject this upload - album not in catalog".to_string()),
            },
        ];

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        self.store.create_review_item(
            &job.id,
            &format!(
                "No matching album found for '{}'. Would you like to search manually?",
                job.original_filename
            ),
            &options_json,
        )?;

        Ok(())
    }

    /// Extract ordered track durations from ingestion files for a job.
    ///
    /// Returns durations in ms sorted by track number when available,
    /// falling back to filename sort when embedded tags are missing.
    async fn extract_ordered_durations(&self, job_id: &str) -> Result<Vec<i64>, IngestionError> {
        let files = self.store.get_files_for_job(job_id)?;

        let has_track_nums = files.iter().any(|f| f.tag_track_num.is_some());

        let mut entries: Vec<(i32, String, i64)> = Vec::with_capacity(files.len());

        for file in &files {
            let track_num = file.tag_track_num.unwrap_or(0);
            let duration = match file.duration_ms {
                Some(d) => d,
                None => {
                    let path = Path::new(&file.temp_file_path);
                    let metadata = probe_audio_file(path).await?;
                    metadata.duration_ms as i64
                }
            };
            entries.push((track_num, file.filename.clone(), duration));
        }

        if has_track_nums {
            entries.sort_by_key(|(track_num, _, _)| *track_num);
        } else {
            entries.sort_by(|(_, name_a, _), (_, name_b, _)| name_a.cmp(name_b));
        }

        Ok(entries.into_iter().map(|(_, _, d)| d).collect())
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

    /// Get detailed job information including candidates and review.
    ///
    /// Returns (candidates, review) where candidates are parsed from the review options
    /// if the job is awaiting review.
    pub fn get_job_details(
        &self,
        job_id: &str,
    ) -> Result<
        (
            Vec<AlbumCandidateInfo>,
            Option<super::models::ReviewQueueItem>,
        ),
        IngestionError,
    > {
        let review = self.store.get_review_item(job_id)?;

        let mut candidates = Vec::new();

        // Parse candidates from review options if available
        if let Some(ref review_item) = review {
            // Only parse if this is a pending review (not resolved)
            if review_item.resolved_at.is_none() {
                if let Ok(options) =
                    serde_json::from_str::<Vec<super::models::ReviewOption>>(&review_item.options)
                {
                    for opt in options {
                        // Parse album candidates from options like "album:abc123"
                        if opt.id.starts_with("album:") {
                            let album_id = opt.id.trim_start_matches("album:");

                            // Try to extract info from the option label/description
                            // Format: "Artist - Album (XX%, N tracks)"
                            let (score, track_count, delta_ms) =
                                parse_option_metadata(&opt.label, opt.description.as_deref());

                            // Try to get album name and artist from catalog
                            let (name, artist_name) =
                                if let Some(candidate) = self.build_album_candidate(album_id) {
                                    (candidate.name, candidate.artist_name)
                                } else {
                                    // Fallback: parse from label
                                    let parts: Vec<&str> = opt.label.splitn(2, " - ").collect();
                                    if parts.len() == 2 {
                                        (
                                            parts[1].split(" (").next().unwrap_or("").to_string(),
                                            parts[0].to_string(),
                                        )
                                    } else {
                                        (opt.label.clone(), "Unknown".to_string())
                                    }
                                };

                            candidates.push(AlbumCandidateInfo {
                                id: album_id.to_string(),
                                name,
                                artist_name,
                                track_count,
                                score,
                                delta_ms,
                            });
                        }
                    }
                }
            }
        }

        Ok((candidates, review))
    }
}
