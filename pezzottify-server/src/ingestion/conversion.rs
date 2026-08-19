impl IngestionManager {
    // =========================================================================
    // Phase 4: Convert Files
    // =========================================================================

    /// Convert all matched files to OGG Vorbis.
    pub async fn convert_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let _claim = self.claim_job(
            job_id,
            "convert",
            &[
                IngestionJobStatus::Converting,
                IngestionJobStatus::Completed,
            ],
        )?;
        self.convert_job_inner(job_id).await
    }

    async fn convert_job_inner(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status == IngestionJobStatus::Completed {
            return Ok(());
        }
        if job.status != IngestionJobStatus::Converting {
            return Err(IngestionError::InvalidState {
                expected: "CONVERTING".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        let files = self.store.get_files_for_job(job_id)?;
        let mut converted = 0;
        let mut converted_track_ids: Vec<String> = Vec::new();

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

            // Check if conversion is needed
            let needs_conversion = matches!(
                file.conversion_reason,
                Some(ConversionReason::HighBitrate { .. })
                    | Some(ConversionReason::LowBitrateApproved { .. })
                    | Some(ConversionReason::UndetectableBitrate)
            );

            if !needs_conversion {
                // No conversion needed - copy file directly to output
                let output_path = self
                    .file_handler
                    .get_output_path(&self.config.media_dir, &track_id);

                // Determine output extension based on original format
                let extension = Path::new(&file.filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ogg");

                let output_path_with_ext = output_path.with_extension(extension);

                // Ensure output directory exists
                if let Some(parent) = output_path_with_ext.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        error!("Failed to create output directory {:?}: {}", parent, e);
                    }
                }

                // Check if input file exists before attempting copy
                let input_exists = input_path.exists();
                if !input_exists {
                    error!(
                        "Input file does not exist: {:?} (temp_file_path: {})",
                        input_path, file.temp_file_path
                    );
                }

                match tokio::fs::copy(&input_path, &output_path_with_ext).await {
                    Ok(_) => {
                        file.output_file_path =
                            Some(output_path_with_ext.to_string_lossy().to_string());
                        file.converted = true; // Mark as processed even if not transcoded
                        converted += 1;
                        converted_track_ids.push(track_id.clone());

                        // Update catalog with appropriate extension (sharded path)
                        let (dir1, dir2) = super::file_handler::FileHandler::shard_dirs(&track_id);
                        let audio_uri =
                            format!("audio/{}/{}/{}.{}", dir1, dir2, track_id, extension);
                        if let Err(e) = self.catalog.set_track_audio_uri(&track_id, &audio_uri) {
                            warn!("Failed to update track {} audio_uri: {}", track_id, e);
                        }

                        info!(
                            "Copied {} -> {} (no conversion needed, {} kbps)",
                            file.filename,
                            track_id,
                            file.bitrate.unwrap_or(0)
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to copy {} from {:?} to {:?}: {}",
                            file.filename, input_path, output_path_with_ext, e
                        );
                        file.error_message = Some(e.to_string());
                    }
                }

                self.store.update_file(&file)?;
                continue;
            }

            // Original conversion logic for files that need conversion
            let output_path = self
                .file_handler
                .get_output_path(&self.config.media_dir, &track_id);

            match convert_to_ogg(input_path, &output_path, self.config.target_bitrate).await {
                Ok(()) => {
                    file.output_file_path = Some(output_path.to_string_lossy().to_string());
                    file.converted = true;
                    converted += 1;
                    converted_track_ids.push(track_id.clone());

                    // Update catalog: set audio_uri for the track (sharded path)
                    let (dir1, dir2) = super::file_handler::FileHandler::shard_dirs(&track_id);
                    let audio_uri = format!("audio/{}/{}/{}.ogg", dir1, dir2, track_id);
                    if let Err(e) = self.catalog.set_track_audio_uri(&track_id, &audio_uri) {
                        warn!("Failed to update track {} audio_uri: {}", track_id, e);
                    }

                    info!(
                        "Converted {} -> {} (target: {} kbps)",
                        file.filename, track_id, self.config.target_bitrate
                    );
                }
                Err(e) => {
                    error!("Failed to convert {}: {}", file.filename, e);
                    file.error_message = Some(e.to_string());
                }
            }

            self.store.update_file(&file)?;
        }

        // Update album availability in catalog
        if let Some(album_id) = &job.matched_album_id {
            match self.catalog.recompute_album_availability(album_id) {
                Ok(availability) => {
                    info!(
                        "Album {} availability updated to {:?}",
                        album_id, availability
                    );

                    // Update search index for album
                    let album_available =
                        availability != crate::catalog_store::AlbumAvailability::Missing;
                    self.search.update_availability(&[(
                        album_id.clone(),
                        HashedItemType::Album,
                        album_available,
                    )]);

                    // Update artist availability for album's artists
                    match self.catalog.get_album_artist_ids(album_id) {
                        Ok(artist_ids) => {
                            for artist_id in artist_ids {
                                match self.catalog.recompute_artist_availability(&artist_id) {
                                    Ok(artist_available) => {
                                        info!(
                                            "Artist {} availability updated to {}",
                                            artist_id, artist_available
                                        );
                                        self.search.update_availability(&[(
                                            artist_id,
                                            HashedItemType::Artist,
                                            artist_available,
                                        )]);
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to recompute artist {} availability: {}",
                                            artist_id, e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to get artist IDs for album {}: {}", album_id, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to recompute album {} availability: {}", album_id, e);
                }
            }
        }

        // Update search index for converted tracks
        let track_availability_updates: Vec<_> = converted_track_ids
            .iter()
            .map(|id| (id.clone(), HashedItemType::Track, true))
            .collect();
        if !track_availability_updates.is_empty() {
            self.search.update_availability(&track_availability_updates);
        }

        job.tracks_converted = converted;
        job.status = IngestionJobStatus::Completed;
        job.completed_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(&job)?;

        // If this job is associated with a download request, mark it as completed
        let mut primary_requester_user_id: Option<String> = None;
        let mut primary_requester_request_id: Option<String> = None;
        let mut download_request_album_name: Option<String> = None;
        let mut download_request_artist_name: Option<String> = None;
        if let (Some(IngestionContextType::DownloadRequest), Some(context_id)) =
            (job.context_type, &job.context_id)
        {
            if let Some(download_manager) = &self.download_manager {
                // Capture the requesting user and names before marking completed
                if let Ok(Some(queue_item)) = download_manager.get_queue_item(context_id) {
                    primary_requester_user_id = queue_item.requested_by_user_id.clone();
                    primary_requester_request_id = Some(queue_item.id.clone());
                    download_request_album_name = queue_item.content_name.clone();
                    download_request_artist_name = queue_item.artist_name.clone();
                }

                let duration_ms = job.started_at.map_or(0, |started| {
                    job.completed_at
                        .unwrap_or(chrono::Utc::now().timestamp_millis())
                        - started
                });

                if let Err(e) = download_manager.mark_request_completed(
                    context_id,
                    job.total_size_bytes as u64,
                    duration_ms,
                ) {
                    error!(
                        "Failed to mark download request {} as completed: {}",
                        context_id, e
                    );
                } else {
                    info!(
                        "Marked download request {} as completed for job {}",
                        context_id, job_id
                    );
                }
            }
        }

        // Auto-complete any other pending download requests for the same album
        let mut auto_completed_requests: Vec<CompletedRequestInfo> = Vec::new();
        if let Some(album_id) = &job.matched_album_id {
            if let Some(download_manager) = &self.download_manager {
                let duration_ms = job.started_at.map_or(0, |started| {
                    job.completed_at
                        .unwrap_or(chrono::Utc::now().timestamp_millis())
                        - started
                });

                match download_manager.complete_requests_for_album(
                    album_id,
                    job.total_size_bytes as u64,
                    duration_ms,
                ) {
                    Ok(completed) if !completed.is_empty() => {
                        info!(
                            "Auto-completed {} additional download request(s) for album {}: {:?}",
                            completed.len(),
                            album_id,
                            completed
                        );
                        auto_completed_requests = completed;
                    }
                    Ok(_) => {} // No additional requests to complete
                    Err(e) => {
                        warn!(
                            "Failed to auto-complete download requests for album {}: {}",
                            album_id, e
                        );
                    }
                }
            }
        }

        // Cleanup temp files
        let _ = self.file_handler.cleanup_job(job_id).await;

        // Notify completion — look up names from catalog for accuracy,
        // falling back to download request names, detected metadata, or "Unknown" defaults.
        let (album_name, artist_name) = match &job.matched_album_id {
            Some(album_id) => {
                let catalog_album_name = match self.catalog.get_album_json(album_id) {
                    Ok(Some(v)) => v.get("name").and_then(|n| n.as_str()).map(String::from),
                    Ok(None) => {
                        warn!(
                            "Job {} - album {} not found in catalog for notification name",
                            job_id, album_id
                        );
                        None
                    }
                    Err(e) => {
                        warn!(
                            "Job {} - failed to look up album {} for notification name: {}",
                            job_id, album_id, e
                        );
                        None
                    }
                };

                let catalog_artist_name = match self.catalog.get_album_artist_ids(album_id) {
                    Ok(ids) => ids.into_iter().next().and_then(|aid| {
                        match self.catalog.get_artist_json(&aid) {
                            Ok(Some(v)) => {
                                v.get("name").and_then(|n| n.as_str()).map(String::from)
                            }
                            Ok(None) => {
                                warn!(
                                    "Job {} - artist {} not found in catalog for notification name",
                                    job_id, aid
                                );
                                None
                            }
                            Err(e) => {
                                warn!(
                                    "Job {} - failed to look up artist {} for notification name: {}",
                                    job_id, aid, e
                                );
                                None
                            }
                        }
                    }),
                    Err(e) => {
                        warn!(
                            "Job {} - failed to get artist IDs for album {} for notification name: {}",
                            job_id, album_id, e
                        );
                        None
                    }
                };

                let album_name = catalog_album_name
                    .or_else(|| download_request_album_name.clone())
                    .or_else(|| job.detected_album.clone())
                    .unwrap_or_else(|| {
                        warn!(
                            "Job {} - all album name sources exhausted, using Unknown Album",
                            job_id
                        );
                        "Unknown Album".to_string()
                    });

                let artist_name = catalog_artist_name
                    .or_else(|| download_request_artist_name.clone())
                    .or_else(|| job.detected_artist.clone())
                    .unwrap_or_else(|| {
                        warn!(
                            "Job {} - all artist name sources exhausted, using Unknown Artist",
                            job_id
                        );
                        "Unknown Artist".to_string()
                    });

                (album_name, artist_name)
            }
            None => {
                warn!(
                    "Job {} - no matched_album_id at notification time, using fallbacks",
                    job_id
                );
                (
                    download_request_album_name
                        .clone()
                        .or_else(|| job.detected_album.clone())
                        .unwrap_or_else(|| "Unknown Album".to_string()),
                    download_request_artist_name
                        .clone()
                        .or_else(|| job.detected_artist.clone())
                        .unwrap_or_else(|| "Unknown Artist".to_string()),
                )
            }
        };

        if let Some(notifier) = &self.notifier {
            notifier
                .notify_completed(&job, converted as u32, &album_name, &artist_name)
                .await;

            // Emit catalog invalidation event for the album
            if let Some(album_id) = &job.matched_album_id {
                notifier
                    .emit_catalog_event(
                        crate::server_store::CatalogEventType::AlbumUpdated,
                        crate::server_store::CatalogContentType::Album,
                        album_id,
                        "ingestion",
                    )
                    .await;
            }
        }

        // Send download-completed notifications to requesting users
        if let Some(album_id) = &job.matched_album_id {
            // Notify the primary requester
            if let (Some(user_id), Some(request_id)) =
                (&primary_requester_user_id, &primary_requester_request_id)
            {
                self.send_download_notification(
                    user_id,
                    request_id,
                    album_id,
                    &album_name,
                    &artist_name,
                )
                .await;
            }

            // Notify auto-completed requesters
            for completed in &auto_completed_requests {
                if let Some(user_id) = &completed.requested_by_user_id {
                    self.send_download_notification(
                        user_id,
                        &completed.id,
                        album_id,
                        &album_name,
                        &artist_name,
                    )
                    .await;
                }
            }
        }

        info!(
            "Completed job {} - converted {}/{} tracks",
            job_id, converted, job.tracks_matched
        );

        Ok(())
    }

}
