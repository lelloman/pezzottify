impl IngestionManager {
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

    /// Process an upload with automatic type detection and fingerprint matching.
    ///
    /// This is the main entry point for the redesigned ingestion flow:
    /// 1. Extracts files from upload
    /// 2. Detects upload type (Track, Album, Collection)
    /// 3. For collections: creates separate jobs per album
    /// 4. Runs duration fingerprint matching
    /// 5. Creates tickets based on match quality
    pub async fn process_upload(
        &self,
        user_id: &str,
        filename: &str,
        data: &[u8],
        context_type: IngestionContextType,
        context_id: Option<String>,
        allow_foreign_context: bool,
    ) -> Result<UploadResult, IngestionError> {
        self.validate_upload_context(
            user_id,
            context_type,
            context_id.as_deref(),
            allow_foreign_context,
        )?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let total_size = data.len() as i64;

        // Create a temp session directory
        let session_dir = self.file_handler.create_job_dir(&session_id).await?;

        // Save uploaded file
        let temp_path = self
            .file_handler
            .save_upload(&session_id, filename, data)
            .await?;

        // Extract if zip
        let extract_dir = if FileHandler::is_zip(filename) {
            let audio_files = self
                .file_handler
                .extract_zip(&session_id, &temp_path)
                .await?;
            if audio_files.is_empty() {
                return Err(IngestionError::NoFiles);
            }
            session_dir.join("extracted")
        } else if FileHandler::is_supported_audio(filename) {
            // Single file - just use the session dir
            session_dir.clone()
        } else {
            return Err(IngestionError::FileHandler(
                FileHandlerError::UnsupportedFileType(filename.to_string()),
            ));
        };

        // Detect upload type
        let upload_type = self.file_handler.detect_upload_type(&extract_dir).await?;

        info!(
            session_id = %session_id,
            upload_type = ?upload_type,
            "Detected upload type"
        );

        // Save context_id for later use (it gets moved into JobCreationParams)
        let saved_context_id = context_id.clone();

        // Create jobs based on upload type
        let job_ids = match upload_type {
            UploadType::Track => {
                // Single track - create one job
                let job_id = self
                    .create_job_internal(JobCreationParams {
                        user_id,
                        name: filename,
                        total_size,
                        dir: &extract_dir,
                        session_id: Some(session_id.clone()),
                        upload_type,
                        context_type,
                        context_id,
                    })
                    .await?;
                vec![job_id]
            }
            UploadType::Album => {
                // Single album - create one job
                let job_id = self
                    .create_job_internal(JobCreationParams {
                        user_id,
                        name: filename,
                        total_size,
                        dir: &extract_dir,
                        session_id: Some(session_id.clone()),
                        upload_type,
                        context_type,
                        context_id,
                    })
                    .await?;
                vec![job_id]
            }
            UploadType::Collection => {
                // Collection - create one job per album directory
                let albums = self.file_handler.group_files_by_album(&extract_dir).await?;
                let mut job_ids = Vec::with_capacity(albums.len());

                for (album_dir, _files) in &albums {
                    let album_name = album_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(filename);

                    let job_id = self
                        .create_job_internal(JobCreationParams {
                            user_id,
                            name: album_name,
                            total_size: 0, // Individual album size not tracked
                            dir: album_dir,
                            session_id: Some(session_id.clone()),
                            upload_type: UploadType::Album, // Each sub-job is an album
                            context_type,
                            context_id: context_id.clone(),
                        })
                        .await?;
                    job_ids.push(job_id);
                }

                job_ids
            }
        };

        let album_count = job_ids.len();

        // If this upload is from a download request, mark the queue item as IN_PROGRESS
        // so the cron downloader won't re-download the same content.
        if context_type == IngestionContextType::DownloadRequest {
            if let Some(ref ctx_id) = saved_context_id {
                if let Some(dm) = &self.download_manager {
                    if let Err(e) = dm.mark_request_in_progress(ctx_id) {
                        warn!(
                            "Failed to mark download request {} as in-progress: {}",
                            ctx_id, e
                        );
                    }
                }
            }
        }

        info!(
            session_id = %session_id,
            job_count = album_count,
            "Created ingestion jobs"
        );

        Ok(UploadResult {
            session_id,
            upload_type,
            job_ids,
            album_count,
        })
    }

    fn validate_upload_context(
        &self,
        user_id: &str,
        context_type: IngestionContextType,
        context_id: Option<&str>,
        allow_foreign_context: bool,
    ) -> Result<(), IngestionError> {
        match context_type {
            IngestionContextType::Spontaneous => {
                if context_id.is_some() {
                    return Err(IngestionError::InvalidContext(
                        "spontaneous uploads cannot reference a context ID".to_string(),
                    ));
                }
            }
            IngestionContextType::DownloadRequest => {
                let context_id = context_id.ok_or_else(|| {
                    IngestionError::InvalidContext(
                        "download request uploads require a context ID".to_string(),
                    )
                })?;
                let manager = self.download_manager.as_ref().ok_or_else(|| {
                    IngestionError::InvalidContext("download manager is not configured".to_string())
                })?;
                let item = manager.get_queue_item(context_id)?.ok_or_else(|| {
                    IngestionError::InvalidContext("download request was not found".to_string())
                })?;
                Self::validate_download_request_owner(
                    user_id,
                    context_id,
                    &item,
                    allow_foreign_context,
                )?;
            }
        }
        Ok(())
    }

    fn validate_download_request_owner(
        user_id: &str,
        context_id: &str,
        item: &QueueItemInfo,
        allow_foreign_context: bool,
    ) -> Result<(), IngestionError> {
        if item.id != context_id {
            return Err(IngestionError::InvalidContext(
                "download request identity does not match the context ID".to_string(),
            ));
        }
        if item.requested_by_user_id.as_deref() != Some(user_id) && !allow_foreign_context {
            return Err(IngestionError::InvalidContext(
                "download request belongs to another user".to_string(),
            ));
        }
        Ok(())
    }

    fn claim_job(
        &self,
        job_id: &str,
        operation: &str,
        expected: &[IngestionJobStatus],
    ) -> Result<JobClaimGuard, IngestionError> {
        match self.store.try_claim_job(job_id, operation, expected)? {
            JobClaimResult::Claimed => Ok(JobClaimGuard {
                store: Arc::clone(&self.store),
                job_id: job_id.to_string(),
            }),
            JobClaimResult::NotFound => Err(IngestionError::JobNotFound(job_id.to_string())),
            JobClaimResult::Busy => Err(IngestionError::JobBusy(job_id.to_string())),
            JobClaimResult::InvalidState(actual) => Err(IngestionError::InvalidState {
                expected: expected
                    .iter()
                    .map(IngestionJobStatus::as_str)
                    .collect::<Vec<_>>()
                    .join(" or "),
                actual: actual.as_str().to_string(),
            }),
        }
    }

    /// Internal helper to create a job from a directory of audio files.
    async fn create_job_internal(
        &self,
        params: JobCreationParams<'_>,
    ) -> Result<String, IngestionError> {
        let job_id = uuid::Uuid::new_v4().to_string();

        // Get audio files
        let audio_files = self
            .file_handler
            .list_audio_files_recursive(params.dir)
            .await?;
        if audio_files.is_empty() {
            return Err(IngestionError::NoFiles);
        }

        let file_count = audio_files.len() as i32;

        // Create job record with upload info
        let job = IngestionJob::new(
            &job_id,
            params.user_id,
            params.name,
            params.total_size,
            file_count,
        )
        .with_context(params.context_type, params.context_id)
        .with_upload_info(params.session_id, params.upload_type);

        self.store.create_job(&job)?;

        // Create file records
        for audio_path in &audio_files {
            let file_id = uuid::Uuid::new_v4().to_string();
            let file_name = audio_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let file_size = tokio::fs::metadata(audio_path)
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
            job_id = %job_id,
            file_count = file_count,
            upload_type = ?params.upload_type,
            "Created ingestion job"
        );

        Ok(job_id)
    }

}
