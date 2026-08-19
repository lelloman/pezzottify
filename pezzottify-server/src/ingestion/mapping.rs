impl IngestionManager {
    // =========================================================================
    // Phase 3: Map Tracks
    // =========================================================================

    /// Map files to tracks within the matched album.
    ///
    /// If `skip_duration_review` is true, duration mismatches will not create a review
    /// (used when called from resolve_review to avoid infinite loops).
    pub async fn map_tracks(
        &self,
        job_id: &str,
        skip_duration_review: bool,
    ) -> Result<(), IngestionError> {
        let _claim = self.claim_job(job_id, "map_tracks", &[IngestionJobStatus::MappingTracks])?;
        self.map_tracks_inner(job_id, skip_duration_review).await
    }

    async fn map_tracks_inner(
        &self,
        job_id: &str,
        skip_duration_review: bool,
    ) -> Result<(), IngestionError> {
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
            .ok_or_else(|| {
                IngestionError::Store(anyhow::anyhow!("Album not found: {}", album_id))
            })?;

        // Parse tracks from discs array
        struct TrackInfo {
            id: String,
            name: String,
            track_number: i32,
            disc_number: i32,
            duration_ms: i64,
        }

        let mut tracks: Vec<TrackInfo> = Vec::new();
        if let Some(discs) = album_json.get("discs").and_then(|d| d.as_array()) {
            for disc in discs {
                let disc_number = disc.get("number").and_then(|n| n.as_i64()).unwrap_or(1) as i32;
                if let Some(disc_tracks) = disc.get("tracks").and_then(|t| t.as_array()) {
                    for track in disc_tracks {
                        let id = track.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                        let name = track
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let track_number = track
                            .get("track_number")
                            .and_then(|n| n.as_i64())
                            .unwrap_or(0) as i32;
                        let duration_ms = track
                            .get("duration_ms")
                            .and_then(|n| n.as_i64())
                            .unwrap_or(0);

                        if !id.is_empty() {
                            tracks.push(TrackInfo {
                                id: id.to_string(),
                                name: name.to_string(),
                                track_number,
                                disc_number,
                                duration_ms,
                            });
                        }
                    }
                }
            }
        }

        let mut files = self.store.get_files_for_job(job_id)?;

        // Verify files exist at start of mapping
        for f in &files {
            let path = Path::new(&f.temp_file_path);
            if !path.exists() {
                error!(
                    "File missing at map_tracks start: {} (path: {})",
                    f.filename, f.temp_file_path
                );
            }
        }

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
        let mut claimed_track_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for file in &mut files {
            // Try to match by disc + track number first
            let disc_num = file.tag_disc_num.unwrap_or(1);
            if let Some(track_num) = file.tag_track_num {
                if let Some(track) = tracks_by_num.get(&(disc_num, track_num)) {
                    file.matched_track_id = Some(track.id.clone());
                    file.match_confidence = Some(1.0);
                    claimed_track_ids.insert(track.id.clone());
                    matched += 1;
                    self.store.update_file(file)?;
                    continue;
                }
            }

            // Fall back to title matching
            if let Some(title) = &file.tag_title {
                let best_match = tracks
                    .iter()
                    .filter(|t| !claimed_track_ids.contains(&t.id))
                    .map(|t| (t, string_similarity(title, &t.name)))
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                if let Some((track, confidence)) = best_match {
                    if confidence > 0.7 {
                        file.matched_track_id = Some(track.id.clone());
                        file.match_confidence = Some(confidence);
                        claimed_track_ids.insert(track.id.clone());
                        matched += 1;
                        self.store.update_file(file)?;
                    }
                }
            }
        }

        // Duration-based fallback: match remaining files by closest track duration.
        // This handles the case where files have no embedded tags but durations
        // are unique enough to identify tracks (common after fingerprint matching).
        let unmatched_count = files
            .iter()
            .filter(|f| f.matched_track_id.is_none())
            .count();
        if unmatched_count > 0 {
            debug!(
                "Tag-based matching left {} unmatched files, trying duration-based mapping",
                unmatched_count
            );

            // Build scored pairs: (file_index, track_index, duration_delta, name_similarity)
            let mut pairs: Vec<(usize, usize, i64, f32)> = Vec::new();
            for (fi, file) in files.iter().enumerate() {
                if file.matched_track_id.is_some() {
                    continue;
                }
                let file_duration = match file.duration_ms {
                    Some(d) => d,
                    None => continue,
                };
                let file_stem = Path::new(&file.filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&file.filename);
                for (ti, track) in tracks.iter().enumerate() {
                    if claimed_track_ids.contains(&track.id) {
                        continue;
                    }
                    let delta = (file_duration - track.duration_ms).abs();
                    let name_sim = string_similarity(file_stem, &track.name);
                    pairs.push((fi, ti, delta, name_sim));
                }
            }

            // Sort by duration delta ascending, then name similarity descending as tiebreaker
            pairs.sort_by(|a, b| a.2.cmp(&b.2).then(b.3.partial_cmp(&a.3).unwrap()));

            let mut claimed_files: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            for (fi, ti, delta, name_sim) in pairs {
                if claimed_files.contains(&fi) || claimed_track_ids.contains(&tracks[ti].id) {
                    continue;
                }
                // Confidence based on duration proximity (10s threshold → 1.0, worse → lower)
                let duration_confidence = (1.0 - (delta as f64 / 10_000.0).min(1.0)) as f32;
                // Blend: 70% duration, 30% name similarity
                let confidence = duration_confidence * 0.7 + name_sim * 0.3;
                if confidence > 0.3 {
                    files[fi].matched_track_id = Some(tracks[ti].id.clone());
                    files[fi].match_confidence = Some(confidence);
                    claimed_track_ids.insert(tracks[ti].id.clone());
                    claimed_files.insert(fi);
                    matched += 1;
                    self.store.update_file(&files[fi])?;
                    debug!(
                        "Duration-matched '{}' → '{}' (delta={}ms, name_sim={:.2}, conf={:.2})",
                        files[fi].filename, tracks[ti].name, delta, name_sim, confidence
                    );
                }
            }
        }

        job.tracks_matched = matched;

        info!(
            "Mapped {}/{} files for job {}",
            matched,
            files.len(),
            job_id
        );

        // Validate durations - flag for review if any track differs by > 10 seconds
        const DURATION_THRESHOLD_MS: i64 = 10_000;
        let tracks_by_id: HashMap<&str, &TrackInfo> =
            tracks.iter().map(|t| (t.id.as_str(), t)).collect();

        // Re-fetch files to get the updated matched_track_id values
        let files = self.store.get_files_for_job(job_id)?;
        let mut duration_mismatches: Vec<String> = Vec::new();

        for file in &files {
            if let (Some(track_id), Some(file_duration)) =
                (&file.matched_track_id, file.duration_ms)
            {
                if let Some(track) = tracks_by_id.get(track_id.as_str()) {
                    let delta = (file_duration - track.duration_ms).abs();
                    if delta > DURATION_THRESHOLD_MS {
                        debug!(
                            "Duration mismatch for {}: file={}ms, catalog={}ms, delta={}ms",
                            file.filename, file_duration, track.duration_ms, delta
                        );
                        duration_mismatches.push(format!(
                            "{}: {}s vs {}s (delta: {}s)",
                            track.name,
                            file_duration / 1000,
                            track.duration_ms / 1000,
                            delta / 1000
                        ));
                    }
                }
            }
        }

        if !duration_mismatches.is_empty() && !skip_duration_review {
            // Flag for review due to duration mismatches (unless skipped)
            let question = format!(
                "Duration mismatch detected for {} track(s):\n{}",
                duration_mismatches.len(),
                duration_mismatches.join("\n")
            );

            let options = vec![
                ReviewOption {
                    id: "continue".to_string(),
                    label: "Continue anyway".to_string(),
                    description: Some("Accept the files despite duration differences".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Reject".to_string(),
                    description: Some("These files don't match the album".to_string()),
                },
            ];

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(job_id, &question, &options_json)?;

            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(&job)?;

            // Notify review needed via WebSocket
            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(&job, &question, &options)
                    .await;
            }

            warn!(
                "Job {} flagged for review: {} duration mismatches",
                job_id,
                duration_mismatches.len()
            );

            return Ok(());
        }

        job.status = IngestionJobStatus::Converting;
        self.store.update_job(&job)?;

        Ok(())
    }

}
