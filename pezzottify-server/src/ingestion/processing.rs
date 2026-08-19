impl IngestionManager {
    // =========================================================================
    // Phase 2: Identify Album (with LLM or heuristics)
    // =========================================================================

    /// Process a job in IDENTIFYING_ALBUM state - search catalog and find matching album.
    ///
    /// For download request jobs (context_type == DownloadRequest), the album is already
    /// known from the queue item, so we skip the search/scoring phase and directly verify
    /// the uploaded content matches the expected album.
    pub async fn process_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let _claim = self.claim_job(
            job_id,
            "process",
            &[
                IngestionJobStatus::Pending,
                IngestionJobStatus::IdentifyingAlbum,
                IngestionJobStatus::MappingTracks,
                IngestionJobStatus::Converting,
                IngestionJobStatus::AwaitingReview,
                IngestionJobStatus::Completed,
            ],
        )?;
        self.process_job_inner(job_id).await
    }

    async fn process_job_inner(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        // Handle different starting states
        match job.status {
            IngestionJobStatus::Pending => {
                // First analyze, then continue
                self.analyze_job_inner(job_id).await?;
                job = self.store.get_job(job_id)?.unwrap();
            }
            IngestionJobStatus::IdentifyingAlbum => {
                // Continue with album identification
            }
            IngestionJobStatus::MappingTracks => {
                self.map_tracks_inner(job_id, false).await?;
                return self.convert_job_inner(job_id).await;
            }
            IngestionJobStatus::Converting => return self.convert_job_inner(job_id).await,
            IngestionJobStatus::AwaitingReview | IngestionJobStatus::Completed => return Ok(()),
            _ => {
                return Err(IngestionError::InvalidState {
                    expected: "PENDING or IDENTIFYING_ALBUM".to_string(),
                    actual: job.status.as_str().to_string(),
                });
            }
        }

        // Get metadata summary
        let summary = self.build_metadata_summary(job_id)?;

        // Check if this is a download request - if so, use the fast path
        if job.context_type == Some(IngestionContextType::DownloadRequest) {
            if let Some(context_id) = job.context_id.clone() {
                return self
                    .process_download_request_job(&mut job, &context_id, &summary)
                    .await;
            }
        }

        // Otherwise, use the normal search-based identification

        debug!(
            "Job {} metadata summary: artist={:?}, album={:?}, year={:?}, files={}, duration={}ms, tracks={:?}",
            job_id,
            summary.artist,
            summary.album,
            summary.year,
            summary.file_count,
            summary.total_duration_ms,
            summary.track_titles
        );

        // Try fingerprint matching first (works even without metadata tags)
        let ordered_durations = self.extract_ordered_durations(job_id).await?;
        if !ordered_durations.is_empty() {
            let fp_config = FingerprintConfig::default();
            let fp_result =
                match_album_with_fallbacks(&ordered_durations, self.catalog.as_ref(), &fp_config)?;

            match fp_result.ticket_type {
                TicketType::Success => {
                    let album = fp_result.matched_album.as_ref().unwrap();
                    job.matched_album_id = Some(album.id.clone());
                    job.match_confidence = Some(fp_result.match_score);
                    job.match_source = Some(IngestionMatchSource::Fingerprint);
                    job.status = IngestionJobStatus::MappingTracks;
                    self.store.update_job(&job)?;

                    info!(
                        "Fingerprint auto-matched job {} to album {} ({} - {}) with {:.0}% confidence, delta={}ms",
                        job_id, album.id, album.artist_name, album.name,
                        fp_result.match_score * 100.0, fp_result.total_delta_ms
                    );

                    if let Some(notifier) = &self.notifier {
                        use crate::server::websocket::messages::ingestion::CandidateSummary;
                        let candidates: Vec<CandidateSummary> = fp_result
                            .candidates
                            .iter()
                            .map(|c| CandidateSummary {
                                id: c.album.id.clone(),
                                name: c.album.name.clone(),
                                artist_name: c.album.artist_name.clone(),
                                track_count: c.album.track_count,
                                score: c.score,
                                delta_ms: c.delta_ms,
                            })
                            .collect();
                        notifier
                            .notify_match_found(&job, TicketType::Success, candidates)
                            .await;
                    }

                    self.map_tracks_inner(job_id, false).await?;

                    let job_after_map = self.store.get_job(job_id)?.unwrap();
                    if job_after_map.tracks_matched == 0 {
                        let mut job = job_after_map;
                        self.fail_job_with_cleanup(
                            &mut job,
                            "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
                        ).await?;

                        return Err(IngestionError::Store(anyhow::anyhow!(
                            "Zero tracks matched for job {}",
                            job_id
                        )));
                    }

                    self.convert_job_inner(job_id).await?;
                    return Ok(());
                }
                TicketType::Review => {
                    let mut options: Vec<ReviewOption> = fp_result
                        .candidates
                        .iter()
                        .map(|c| ReviewOption {
                            id: format!("album:{}", c.album.id),
                            label: format!(
                                "{} - {} ({:.0}%, {} tracks, delta={}ms)",
                                c.album.artist_name,
                                c.album.name,
                                c.score * 100.0,
                                c.album.track_count,
                                c.delta_ms
                            ),
                            description: None,
                        })
                        .collect();
                    options.push(ReviewOption {
                        id: "no_match".to_string(),
                        label: "None of these".to_string(),
                        description: Some("Album not in catalog".to_string()),
                    });

                    let question = format!(
                        "Fingerprint matched candidates for '{}' ({} files).\nDetected: {} - {}",
                        job.original_filename,
                        summary.file_count,
                        summary.artist.as_deref().unwrap_or("Unknown Artist"),
                        summary.album.as_deref().unwrap_or("Unknown Album"),
                    );

                    let options_json = serde_json::to_string(&options).unwrap_or_default();
                    self.store
                        .create_review_item(job_id, &question, &options_json)?;

                    job.status = IngestionJobStatus::AwaitingReview;
                    self.store.update_job(&job)?;

                    if let Some(notifier) = &self.notifier {
                        notifier
                            .notify_review_needed(&job, &question, &options)
                            .await;
                    }

                    info!(
                        "Fingerprint review needed for job {} - best match: {:.0}%",
                        job_id,
                        fp_result.match_score * 100.0
                    );
                    return Ok(());
                }
                TicketType::Failure => {
                    debug!(
                        "Fingerprint matching failed for job {}, falling through to search-based identification",
                        job_id
                    );
                }
            }
        }

        // Search for matching albums in catalog
        let candidates = self.search_album_candidates(&summary).await?;

        debug!("Job {} found {} album candidates", job_id, candidates.len());

        if candidates.is_empty() {
            // No candidates found
            info!(
                "Job {} - no album candidates found for query: artist={:?}, album={:?}",
                job_id, summary.artist, summary.album
            );
            job.status = IngestionJobStatus::AwaitingReview;
            self.create_no_match_review(&job, &summary)?;
            self.store.update_job(&job)?;
            return Ok(());
        }

        // Score candidates (now includes track-based scoring)
        let mut scored: Vec<(AlbumCandidate, f32)> = candidates
            .into_iter()
            .map(|candidate| {
                let score = self.score_album_match(&summary, &candidate);
                debug!(
                    "Candidate {} - {} ({} tracks): score={:.2}",
                    candidate.artist_name, candidate.name, candidate.track_count, score
                );
                (candidate, score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if !scored.is_empty() {
            debug!(
                "Top candidate: {} - {} with score {:.2} (threshold: {:.2})",
                scored[0].0.artist_name,
                scored[0].0.name,
                scored[0].1,
                self.config.auto_match_threshold
            );
        }

        // Find best match
        if let Some((best_candidate, confidence)) = scored.first() {
            let label = format!("{} - {}", best_candidate.artist_name, best_candidate.name);

            if *confidence >= self.config.auto_match_threshold {
                // High confidence - auto-match
                job.matched_album_id = Some(best_candidate.id.clone());
                job.match_confidence = Some(*confidence);
                job.match_source = Some(IngestionMatchSource::Agent);
                job.status = IngestionJobStatus::MappingTracks;
                self.store.update_job(&job)?;

                info!(
                    "Auto-matched job {} to album {} with {:.0}% confidence (tracks: {}/{})",
                    job_id,
                    best_candidate.id,
                    confidence * 100.0,
                    summary.file_count,
                    best_candidate.track_count
                );

                // Notify match found
                if let Some(notifier) = &self.notifier {
                    use crate::server::websocket::messages::ingestion::CandidateSummary;
                    let candidates: Vec<CandidateSummary> = scored
                        .iter()
                        .take(5)
                        .map(|(c, s)| CandidateSummary {
                            id: c.id.clone(),
                            name: c.name.clone(),
                            artist_name: c.artist_name.clone(),
                            track_count: c.track_count,
                            score: *s,
                            delta_ms: 0, // Not available from this scoring path
                        })
                        .collect();
                    notifier
                        .notify_match_found(&job, TicketType::Success, candidates)
                        .await;
                }

                // Continue to track mapping and conversion
                self.map_tracks_inner(job_id, false).await?;

                // Fail if no tracks could be matched
                let job_after_map = self.store.get_job(job_id)?.unwrap();
                if job_after_map.tracks_matched == 0 {
                    let mut job = job_after_map;
                    self.fail_job_with_cleanup(
                        &mut job,
                        "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
                    ).await?;

                    return Err(IngestionError::Store(anyhow::anyhow!(
                        "Zero tracks matched for job {}",
                        job_id
                    )));
                }

                self.convert_job_inner(job_id).await?;
            } else {
                // Low confidence - request review
                let options: Vec<ReviewOption> = scored
                    .iter()
                    .take(5)
                    .map(|(candidate, conf)| ReviewOption {
                        id: format!("album:{}", candidate.id),
                        label: format!(
                            "{} - {} ({:.0}%, {} tracks)",
                            candidate.artist_name,
                            candidate.name,
                            conf * 100.0,
                            candidate.track_count
                        ),
                        description: None,
                    })
                    .chain(std::iter::once(ReviewOption {
                        id: "no_match".to_string(),
                        label: "None of these".to_string(),
                        description: Some("Album not in catalog".to_string()),
                    }))
                    .collect();

                let question = format!(
                    "Which album is this?\nDetected: {} - {} ({} files)",
                    summary.artist.as_deref().unwrap_or("Unknown Artist"),
                    summary.album.as_deref().unwrap_or("Unknown Album"),
                    summary.file_count
                );

                let options_json = serde_json::to_string(&options).unwrap_or_default();
                self.store
                    .create_review_item(job_id, &question, &options_json)?;

                job.status = IngestionJobStatus::AwaitingReview;
                self.store.update_job(&job)?;

                // Notify review needed
                if let Some(notifier) = &self.notifier {
                    notifier
                        .notify_review_needed(&job, &question, &options)
                        .await;
                }

                info!(
                    "Job {} requires review - best match: {} ({:.0}%)",
                    job_id,
                    label,
                    confidence * 100.0
                );
            }
        }

        Ok(())
    }

    /// Process a download request job - album is already known, just verify and proceed.
    ///
    /// For download requests, we skip the search/scoring phase because the album ID
    /// is already specified in the queue item. We still verify the uploaded content
    /// is a reasonable match (track count, duration) before proceeding.
    async fn process_download_request_job(
        &self,
        job: &mut IngestionJob,
        queue_item_id: &str,
        summary: &AlbumMetadataSummary,
    ) -> Result<(), IngestionError> {
        let job_id = job.id.clone();

        // Get the queue item to find the album ID
        let queue_item = match &self.download_manager {
            Some(dm) => dm.get_queue_item(queue_item_id).map_err(|e| {
                IngestionError::Store(anyhow::anyhow!(
                    "Failed to get queue item {}: {}",
                    queue_item_id,
                    e
                ))
            })?,
            None => {
                warn!(
                    "Job {} has download request context but no download manager configured",
                    job_id
                );
                return Err(IngestionError::Store(anyhow::anyhow!(
                    "Download manager not configured"
                )));
            }
        };

        let queue_item = match queue_item {
            Some(item) => item,
            None => {
                warn!(
                    "Job {} references non-existent queue item {}",
                    job_id, queue_item_id
                );
                return Err(IngestionError::Store(anyhow::anyhow!(
                    "Queue item {} not found",
                    queue_item_id
                )));
            }
        };

        let album_id = &queue_item.content_id;
        let album_name = queue_item
            .content_name
            .as_deref()
            .unwrap_or("Unknown Album");
        let artist_name = queue_item
            .artist_name
            .as_deref()
            .unwrap_or("Unknown Artist");

        info!(
            "Job {} is a download request for album {} ({} - {})",
            job_id, album_id, artist_name, album_name
        );

        // Verify using duration fingerprint comparison
        let uploaded_durations = self.extract_ordered_durations(&job_id).await?;
        let catalog_durations = self
            .catalog
            .get_album_track_durations(album_id)
            .map_err(|e| {
                IngestionError::Store(anyhow::anyhow!(
                    "Failed to get track durations for album {}: {}",
                    album_id,
                    e
                ))
            })?;

        // Also compute metadata score as a secondary signal
        let metadata_score = if let Some(candidate) = self.build_album_candidate(album_id) {
            self.score_album_match(summary, &candidate)
        } else {
            0.0
        };

        let fp_config = FingerprintConfig::default();
        let (fp_matches, fp_delta) =
            if !uploaded_durations.is_empty() && !catalog_durations.is_empty() {
                compare_durations(
                    &uploaded_durations,
                    &catalog_durations,
                    fp_config.track_tolerance_ms,
                )
            } else {
                (0, 0)
            };
        let total_tracks = uploaded_durations.len().max(catalog_durations.len());
        let fp_score = if total_tracks > 0 {
            fp_matches as f32 / total_tracks as f32
        } else {
            0.0
        };

        debug!(
            "Job {} download request verification: album={}, fp_score={:.2} ({}/{} tracks, delta={}ms), metadata_score={:.2}",
            job_id, album_id, fp_score, fp_matches, total_tracks, fp_delta, metadata_score
        );

        let fp_avg_delta = if total_tracks > 0 {
            fp_delta / total_tracks as i64
        } else {
            fp_delta
        };
        if fp_score >= 1.0 && fp_avg_delta < fp_config.auto_ingest_avg_delta_threshold_ms {
            // Perfect fingerprint match — auto-proceed
        } else if fp_score >= 0.9 {
            // High but not perfect — review with details
            let options = vec![
                ReviewOption {
                    id: format!("album:{}", album_id),
                    label: format!(
                        "{} - {} (fingerprint {:.0}%, metadata {:.0}%)",
                        artist_name,
                        album_name,
                        fp_score * 100.0,
                        metadata_score * 100.0
                    ),
                    description: Some("Proceed with this album".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Content doesn't match".to_string(),
                    description: Some("Reject this upload".to_string()),
                },
            ];

            let question = format!(
                "Downloaded content for '{}' has near-match fingerprint ({:.0}%, delta={}ms).\n\
                 Metadata score: {:.0}%\n\
                 Expected: {} tracks, Uploaded: {} files\n\
                 Confirm this is the correct album:",
                album_name,
                fp_score * 100.0,
                fp_delta,
                metadata_score * 100.0,
                catalog_durations.len(),
                summary.file_count
            );

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(&job_id, &question, &options_json)?;

            job.matched_album_id = Some(album_id.clone());
            job.match_confidence = Some(fp_score);
            job.match_source = Some(IngestionMatchSource::DownloadRequest);
            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;

            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(job, &question, &options)
                    .await;
            }

            return Ok(());
        } else {
            // Low fingerprint score — review with warning
            warn!(
                "Job {} - download request fingerprint score low ({:.2}), requesting review",
                job_id, fp_score
            );

            let options = vec![
                ReviewOption {
                    id: format!("album:{}", album_id),
                    label: format!(
                        "{} - {} (fingerprint {:.0}%, metadata {:.0}%)",
                        artist_name,
                        album_name,
                        fp_score * 100.0,
                        metadata_score * 100.0
                    ),
                    description: Some("Proceed with this album anyway".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Content doesn't match".to_string(),
                    description: Some("Reject this upload".to_string()),
                },
            ];

            let question = format!(
                "Downloaded content for '{}' has low fingerprint match ({:.0}%, delta={}ms).\n\
                 Metadata score: {:.0}%\n\
                 Expected: {} tracks, Uploaded: {} files\n\
                 Confirm this is the correct album:",
                album_name,
                fp_score * 100.0,
                fp_delta,
                metadata_score * 100.0,
                catalog_durations.len(),
                summary.file_count
            );

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(&job_id, &question, &options_json)?;

            job.matched_album_id = Some(album_id.clone());
            job.match_confidence = Some(fp_score);
            job.match_source = Some(IngestionMatchSource::DownloadRequest);
            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;

            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(job, &question, &options)
                    .await;
            }

            return Ok(());
        }

        // Perfect fingerprint match - proceed directly to track mapping
        job.matched_album_id = Some(album_id.clone());
        job.match_confidence = Some(fp_score);
        job.match_source = Some(IngestionMatchSource::DownloadRequest);
        job.status = IngestionJobStatus::MappingTracks;
        self.store.update_job(job)?;

        info!(
            "Download request job {} matched to album {} with fingerprint {:.0}% (delta={}ms, metadata {:.0}%, tracks: {}/{})",
            job_id, album_id,
            fp_score * 100.0, fp_delta,
            metadata_score * 100.0,
            summary.file_count,
            catalog_durations.len()
        );

        // Continue to track mapping and conversion
        self.map_tracks_inner(&job_id, false).await?;

        // Fail if no tracks could be matched
        let job_after_map = self.store.get_job(&job_id)?.unwrap();
        if job_after_map.tracks_matched == 0 {
            let mut job = job_after_map;
            self.fail_job_with_cleanup(
                &mut job,
                "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
            ).await?;

            return Err(IngestionError::Store(anyhow::anyhow!(
                "Zero tracks matched for job {}",
                job_id
            )));
        }

        self.convert_job_inner(&job_id).await?;

        Ok(())
    }

    /// Search catalog for album candidates matching the summary.
    async fn search_album_candidates(
        &self,
        summary: &AlbumMetadataSummary,
    ) -> Result<Vec<AlbumCandidate>, IngestionError> {
        let mut candidates = Vec::new();

        // Collect album IDs first, then fetch full details
        let mut album_ids: Vec<String> = Vec::new();
        let album_filter = Some(vec![HashedItemType::Album]);

        // Strategy 1: Search for album by name only
        if let Some(album_name) = &summary.album {
            debug!("Searching albums by name: {:?}", album_name);
            let results = self.search.search(album_name, 10, album_filter.clone());
            debug!("Album name search returned {} results", results.len());
            for result in results {
                album_ids.push(result.item_id.clone());
            }
        }

        // Strategy 2: Search for artist and include their albums
        if let Some(artist_name) = &summary.artist {
            debug!("Searching artists by name: {:?}", artist_name);
            let artist_filter = Some(vec![HashedItemType::Artist]);
            let artist_results = self.search.search(artist_name, 5, artist_filter);
            debug!("Artist search returned {} results", artist_results.len());

            for result in artist_results {
                if let Ok(Some(artist_json)) =
                    self.catalog.get_resolved_artist_json(&result.item_id)
                {
                    if let Some(albums) = artist_json.get("albums").and_then(|a| a.as_array()) {
                        // Include all albums from matched artists for scoring
                        for album in albums.iter().take(20) {
                            if let Some(album_id) = album.get("id").and_then(|v| v.as_str()) {
                                if !album_id.is_empty() {
                                    album_ids.push(album_id.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Strategy 3: Fallback to source filename if no artist/album detected
        if album_ids.is_empty() {
            debug!(
                "Falling back to source filename search: {:?}",
                summary.source_name
            );
            let results = self.search.search(&summary.source_name, 10, album_filter);
            debug!("Filename search returned {} results", results.len());
            for result in results {
                album_ids.push(result.item_id.clone());
            }
        }

        // Deduplicate IDs
        album_ids.sort();
        album_ids.dedup();

        debug!(
            "Total unique album IDs to evaluate: {} - {:?}",
            album_ids.len(),
            album_ids
        );

        // Fetch full album details with tracks for each candidate
        for album_id in &album_ids {
            if let Some(candidate) = self.build_album_candidate(album_id) {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    /// Build an AlbumCandidate from a resolved album JSON.
    fn build_album_candidate(&self, album_id: &str) -> Option<AlbumCandidate> {
        let album_json = self.catalog.get_resolved_album_json(album_id).ok()??;

        let album = album_json.get("album")?;
        let artists = album_json.get("artists")?.as_array()?;

        let id = album.get("id")?.as_str()?.to_string();
        let name = album.get("name")?.as_str()?.to_string();
        let artist_name = artists
            .first()
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Artist")
            .to_string();

        // Extract track info from discs
        let mut track_titles = Vec::new();
        let mut total_duration_ms: i64 = 0;
        let mut track_count = 0;

        if let Some(discs) = album_json.get("discs").and_then(|d| d.as_array()) {
            for disc in discs {
                if let Some(tracks) = disc.get("tracks").and_then(|t| t.as_array()) {
                    for track in tracks {
                        track_count += 1;
                        if let Some(title) = track.get("name").and_then(|v| v.as_str()) {
                            track_titles.push(title.to_string());
                        }
                        if let Some(duration) = track.get("duration_ms").and_then(|v| v.as_i64()) {
                            total_duration_ms += duration;
                        }
                    }
                }
            }
        }

        Some(AlbumCandidate {
            id,
            name,
            artist_name,
            track_count,
            total_duration_ms,
            track_titles,
        })
    }

    /// Score how well an album candidate matches the detected metadata.
    ///
    /// Scoring weights:
    /// - 25% Artist name similarity
    /// - 25% Album name similarity
    /// - 15% Track count match
    /// - 15% Track title overlap
    /// - 10% Total duration similarity
    /// - 10% Source filename similarity
    fn score_album_match(&self, summary: &AlbumMetadataSummary, candidate: &AlbumCandidate) -> f32 {
        let mut score = 0.0;
        let mut factors = 0.0;

        // Artist similarity (25%)
        if let Some(detected_artist) = &summary.artist {
            let sim = string_similarity(detected_artist, &candidate.artist_name);
            score += sim * 0.25;
            factors += 0.25;
        }

        // Album name similarity (25%)
        if let Some(detected_album) = &summary.album {
            let sim = string_similarity(detected_album, &candidate.name);
            score += sim * 0.25;
            factors += 0.25;
        }

        // Track count match (15%)
        // Perfect match = 1.0, each track difference reduces by 0.1
        let track_diff = (summary.file_count - candidate.track_count).abs();
        let track_count_score = (1.0 - (track_diff as f32 * 0.1)).max(0.0);
        score += track_count_score * 0.15;
        factors += 0.15;

        // Track title overlap (15%)
        // Calculate how many uploaded track titles match catalog track titles
        if !summary.track_titles.is_empty() && !candidate.track_titles.is_empty() {
            let mut matched_titles = 0;
            for upload_title in &summary.track_titles {
                // Find best match in candidate tracks
                let best_sim = candidate
                    .track_titles
                    .iter()
                    .map(|t| string_similarity(upload_title, t))
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                if best_sim > 0.7 {
                    matched_titles += 1;
                }
            }
            let title_overlap = matched_titles as f32 / summary.track_titles.len() as f32;
            score += title_overlap * 0.15;
            factors += 0.15;
        }

        // Duration similarity (10%)
        // Allow 10% tolerance, perfect match = 1.0
        if summary.total_duration_ms > 0 && candidate.total_duration_ms > 0 {
            let duration_ratio =
                summary.total_duration_ms as f64 / candidate.total_duration_ms as f64;
            let duration_diff = (1.0 - duration_ratio).abs();
            let duration_score = (1.0 - duration_diff * 5.0).max(0.0) as f32; // 20% diff = 0 score
            score += duration_score * 0.10;
            factors += 0.10;
        }

        // Source filename similarity (10%)
        let source_sim = string_similarity(
            &summary.source_name,
            &format!("{} - {}", candidate.artist_name, candidate.name),
        );
        score += source_sim * 0.10;
        factors += 0.10;

        if factors > 0.0 {
            score / factors
        } else {
            0.0
        }
    }

    fn create_no_match_review(
        &self,
        job: &IngestionJob,
        summary: &AlbumMetadataSummary,
    ) -> Result<(), IngestionError> {
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
        self.store
            .create_review_item(&job.id, &question, &options_json)?;

        Ok(())
    }

}
