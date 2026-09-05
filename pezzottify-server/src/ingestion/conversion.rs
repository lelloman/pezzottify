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

            if file.converted {
                converted += 1;
                continue;
            }
            let extension = if needs_conversion {
                "ogg"
            } else {
                Path::new(&file.filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("ogg")
            }
            .to_owned();
            let published = if needs_conversion {
                match self
                    .media
                    .stage(
                        track_id.clone(),
                        extension,
                        crate::media::Provenance::Ingested,
                    )
                    .await
                {
                    Ok(stage) => {
                        match convert_to_ogg(input_path, &stage.path(), self.config.target_bitrate)
                            .await
                        {
                            Ok(()) => self.media.commit(stage).await,
                            Err(error) => Err(error.into()),
                        }
                    }
                    Err(error) => Err(error),
                }
            } else {
                self.media
                    .publish_file(
                        track_id.clone(),
                        extension,
                        input_path.to_owned(),
                        crate::media::Provenance::Ingested,
                    )
                    .await
            };
            match published {
                Ok(receipt) => {
                    file.output_file_path = Some(
                        self.config
                            .media_dir
                            .join(receipt.uri)
                            .to_string_lossy()
                            .into_owned(),
                    );
                    file.converted = true;
                    file.error_message = None;
                    converted += 1;
                }
                Err(error) => {
                    file.error_message = Some(error.to_string());
                    self.store.update_file(&file)?;
                    return Err(IngestionError::Store(error));
                }
            }
            self.store.update_file(&file)?;
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
