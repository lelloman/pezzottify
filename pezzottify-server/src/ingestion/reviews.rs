impl IngestionManager {
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
        let _claim = self.claim_job(
            job_id,
            "resolve_review",
            &[IngestionJobStatus::AwaitingReview],
        )?;
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

        self.store
            .resolve_review(job_id, reviewer_user_id, selected_option)?;

        if selected_option.starts_with("album:") {
            let album_id = selected_option.trim_start_matches("album:");
            job.matched_album_id = Some(album_id.to_string());
            job.match_confidence = Some(1.0);
            job.match_source = Some(IngestionMatchSource::HumanReview);
            job.status = IngestionJobStatus::MappingTracks;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} matched to album {}",
                job_id, album_id
            );

            // Continue to track mapping and conversion
            // Skip duration review since user already approved this album
            self.map_tracks_inner(job_id, true).await?;

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
        } else if selected_option == "no_match" {
            self.fail_job_with_cleanup(&mut job, "Album not in catalog")
                .await?;
        } else if selected_option == "continue" {
            // User accepted duration mismatches, continue to conversion
            job.status = IngestionJobStatus::Converting;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} continuing despite duration mismatches",
                job_id
            );

            self.convert_job_inner(job_id).await?;
        } else if selected_option == "convert_low_bitrate" {
            // User approved converting low bitrate files
            let mut files = self.store.get_files_for_job(job_id)?;
            for file in &mut files {
                if let Some(ConversionReason::LowBitratePendingConfirmation { original_bitrate }) =
                    file.conversion_reason
                {
                    file.conversion_reason =
                        Some(ConversionReason::LowBitrateApproved { original_bitrate });
                    self.store.update_file(file)?;
                }
            }

            // Continue to identification phase
            job.status = IngestionJobStatus::IdentifyingAlbum;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} low bitrate files approved for conversion",
                job_id
            );

            // Continue processing
            self.process_job_inner(job_id).await?;
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

    pub fn get_pending_reviews_for_user(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<super::models::ReviewQueueItem>, IngestionError> {
        Ok(self.store.get_pending_reviews_by_user(user_id, limit)?)
    }

    /// Delete a job and its associated files.
    pub async fn delete_job(&self, job_id: &str) -> Result<(), IngestionError> {
        // Clean up temp files
        if let Err(e) = self.file_handler.cleanup_job(job_id).await {
            warn!("Failed to cleanup files for job {}: {}", job_id, e);
        }

        // Delete from database (cascades to files and review queue)
        self.store.delete_job(job_id)?;

        info!("Deleted job {}", job_id);
        Ok(())
    }
}
