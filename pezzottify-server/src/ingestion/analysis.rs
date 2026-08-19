impl IngestionManager {
    // =========================================================================
    // Phase 1: Analyze Files
    // =========================================================================

    /// Analyze all files in a job - extract audio metadata and embedded tags.
    pub async fn analyze_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let _claim = self.claim_job(job_id, "analyze", &[IngestionJobStatus::Pending])?;
        self.analyze_job_inner(job_id).await
    }

    async fn analyze_job_inner(&self, job_id: &str) -> Result<(), IngestionError> {
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
        let total_files = files.len();

        for (idx, mut file) in files.into_iter().enumerate() {
            // Notify progress
            if let Some(notifier) = &self.notifier {
                let progress = ((idx as f32 / total_files as f32) * 100.0) as u8;
                notifier
                    .notify_progress(&job, "analyzing", progress, idx as u32)
                    .await;
            }

            // Probe audio metadata
            let path = Path::new(&file.temp_file_path);
            match probe_audio_file(path).await {
                Ok(metadata) => {
                    file.duration_ms = Some(metadata.duration_ms);
                    file.codec = Some(metadata.codec);
                    file.bitrate = metadata.bitrate;
                    file.sample_rate = metadata.sample_rate;

                    // Determine if conversion is needed based on bitrate
                    file.conversion_reason = Some(self.determine_conversion_need(
                        file.bitrate,
                        &file.codec,
                        Path::new(&file.temp_file_path),
                    ));
                }
                Err(e) => {
                    warn!("Failed to probe {}: {}", file.filename, e);
                    file.error_message = Some(format!("Probe failed: {}", e));
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

        // Notify 100% completion of analyzing phase
        if let Some(notifier) = &self.notifier {
            notifier
                .notify_progress(&job, "analyzing", 100, total_files as u32)
                .await;
        }

        // Count probe failures — fail early if no files could be probed
        let files_after = self.store.get_files_for_job(job_id)?;
        let probed_count = files_after
            .iter()
            .filter(|f| f.duration_ms.is_some())
            .count();
        let failed_count = files_after.len() - probed_count;

        if probed_count == 0 {
            let error_msg = format!(
                "All {} audio files failed to probe — files may be corrupted or in an unsupported format",
                failed_count
            );
            self.fail_job_with_cleanup(&mut job, &error_msg).await?;

            return Err(IngestionError::Store(anyhow::anyhow!(
                "All files failed audio probe"
            )));
        }

        if failed_count > 0 {
            warn!(
                "Job {} — {}/{} files failed to probe",
                job_id,
                failed_count,
                files_after.len()
            );
        }

        // Aggregate detected metadata
        let summary = self.build_metadata_summary(job_id)?;
        job.detected_artist = summary.artist;
        job.detected_album = summary.album;
        job.detected_year = summary.year;

        // Check for low bitrate files before continuing
        if self.check_low_bitrate_files(job_id, &mut job).await? {
            return Ok(()); // Job is now in AwaitingReview
        }

        // No low bitrate issues, continue to identification
        job.status = IngestionJobStatus::IdentifyingAlbum;
        self.store.update_job(&job)?;

        // Verify files still exist after analysis
        let files_for_verify = self.store.get_files_for_job(job_id)?;
        for f in &files_for_verify {
            let path = Path::new(&f.temp_file_path);
            if !path.exists() {
                error!(
                    "File missing after analysis: {} (path: {})",
                    f.filename, f.temp_file_path
                );
            }
        }

        info!(
            "Analyzed job {} - detected: {:?} - {:?}",
            job_id, job.detected_artist, job.detected_album
        );

        Ok(())
    }

    /// Extract embedded tags from an audio file using ffprobe.
    async fn extract_tags(&self, path: &Path) -> Result<HashMap<String, String>> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format"])
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

    /// Determine if a file needs conversion based on bitrate and format.
    fn determine_conversion_need(
        &self,
        bitrate: Option<i32>,
        _codec: &Option<String>,
        _temp_path: &Path,
    ) -> ConversionReason {
        let min_bitrate = self.config.target_bitrate as i32 - self.config.bitrate_tolerance as i32;
        let max_bitrate = self.config.target_bitrate as i32 + self.config.bitrate_tolerance as i32;

        let bitrate = match bitrate {
            Some(b) if b > 0 => b,
            _ => return ConversionReason::UndetectableBitrate,
        };

        if bitrate < min_bitrate {
            return ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: bitrate,
            };
        }

        if bitrate > max_bitrate {
            return ConversionReason::HighBitrate {
                original_bitrate: bitrate,
            };
        }

        // Bitrate is within range - no conversion needed
        ConversionReason::NoConversionNeeded
    }

    /// Check if any files have low bitrate and create review if needed.
    async fn check_low_bitrate_files(
        &self,
        job_id: &str,
        job: &mut IngestionJob,
    ) -> Result<bool, IngestionError> {
        let files = self.store.get_files_for_job(job_id)?;
        let low_bitrate_files: Vec<_> = files
            .iter()
            .filter_map(|f| {
                if let Some(ConversionReason::LowBitratePendingConfirmation { original_bitrate }) =
                    &f.conversion_reason
                {
                    Some((f.filename.clone(), *original_bitrate))
                } else {
                    None
                }
            })
            .collect();

        if !low_bitrate_files.is_empty() {
            let files_list = low_bitrate_files
                .iter()
                .map(|(name, br)| format!("{} ({} kbps)", name, br))
                .collect::<Vec<_>>()
                .join("\n");

            let question = format!(
                "Audio quality is below target ({} kbps).\n\n\
                 The following files have low bitrate:\n{}\n\n\
                 Convert anyway or reject?",
                self.config.target_bitrate, files_list
            );

            let options = vec![
                ReviewOption {
                    id: "convert_low_bitrate".to_string(),
                    label: "Convert anyway".to_string(),
                    description: Some(format!(
                        "Convert low bitrate files to {}kbps OGG",
                        self.config.target_bitrate
                    )),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Reject upload".to_string(),
                    description: Some("Cancel this ingestion due to low quality".to_string()),
                },
            ];

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(job_id, &question, &options_json)?;

            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;
            return Ok(true);
        }

        Ok(false)
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

}
