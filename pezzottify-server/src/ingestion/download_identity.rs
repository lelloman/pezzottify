/// Worker downloads use catalog track IDs as filenames, without embedded tags.
/// Return their indices in album order only for a complete, unique ID match.
fn download_file_order(files: &[IngestionFile], track_ids: &[&str]) -> Option<Vec<usize>> {
    if files.is_empty() || files.len() != track_ids.len() {
        return None;
    }
    let mut by_id = HashMap::new();
    for (index, file) in files.iter().enumerate() {
        let id = Path::new(&file.filename).file_stem()?.to_str()?;
        if by_id.insert(id, index).is_some() {
            return None;
        }
    }
    let order = track_ids.iter().map(|id| by_id.remove(id)).collect();
    if by_id.is_empty() {
        order
    } else {
        None
    }
}

fn is_worker_download(job: &IngestionJob) -> bool {
    job.context_type == Some(IngestionContextType::DownloadRequest)
        && job.upload_session_id.as_deref() == Some(job.id.as_str())
}

impl IngestionManager {
    /// Repair reviews created by the old filename-order comparison. Only the
    /// worker's own complete downloads with a perfect duration match qualify.
    pub(crate) async fn resume_verified_worker_downloads(&self) -> Result<(), IngestionError> {
        let mut jobs = Vec::new();
        for status in [
            IngestionJobStatus::AwaitingReview,
            IngestionJobStatus::MappingTracks,
            IngestionJobStatus::Converting,
        ] {
            jobs.extend(self.store.list_jobs_by_status(status, 1000)?);
        }
        for job in jobs {
            if !is_worker_download(&job)
                || job.match_source != Some(IngestionMatchSource::DownloadRequest)
            {
                continue;
            }
            let awaiting_review = job.status == IngestionJobStatus::AwaitingReview;
            if awaiting_review {
                let Some(review) = self.store.get_review_item(&job.id)? else {
                    continue;
                };
                if review.resolved_at.is_some()
                    || !review.question.starts_with("Downloaded content for '")
                {
                    continue;
                }
            }
            let _claim = self.claim_job(&job.id, "recover_download_identity", &[job.status])?;
            let (Some(album_id), Some(queue_id), Some(downloads)) = (
                &job.matched_album_id,
                &job.context_id,
                &self.download_manager,
            ) else {
                continue;
            };
            let Some(item) = downloads.get_queue_item(queue_id)? else {
                continue;
            };
            Self::validate_download_request_owner(&job.user_id, queue_id, &item, false)?;
            if item.content_id != *album_id {
                continue;
            }
            let actual = self.download_request_durations(&job, album_id).await?;
            let expected = self.catalog.get_album_track_durations(album_id)?;
            let config = FingerprintConfig::default();
            let (matches, delta) = compare_durations(&actual, &expected, config.track_tolerance_ms);
            if actual.is_empty()
                || actual.len() != expected.len()
                || matches != expected.len()
                || delta / expected.len() as i64 >= config.auto_ingest_avg_delta_threshold_ms
            {
                continue;
            }
            info!(job_id = %job.id, "Resuming verified worker download after correcting track order");
            let mut job = job;
            if awaiting_review {
                self.store.resolve_review(&job.id, "system", "retry")?;
                job.status = IngestionJobStatus::IdentifyingAlbum;
                self.store.update_job(&job)?;
            }
            self.process_job_inner(&job.id).await?;
        }
        Ok(())
    }

    async fn download_request_durations(
        &self,
        job: &IngestionJob,
        album_id: &str,
    ) -> Result<Vec<i64>, IngestionError> {
        if !is_worker_download(job) {
            return self.extract_ordered_durations(&job.id).await;
        }
        let album = self
            .catalog
            .get_resolved_album(album_id)?
            .ok_or_else(|| IngestionError::Store(anyhow::anyhow!("Album not found: {album_id}")))?;
        let mut tracks: Vec<_> = album.discs.iter().flat_map(|disc| &disc.tracks).collect();
        tracks.sort_by_key(|track| (track.disc_number, track.track_number));
        let ids: Vec<_> = tracks.iter().map(|track| track.id.as_str()).collect();
        let files = self.store.get_files_for_job(&job.id)?;
        let Some(order) = download_file_order(&files, &ids) else {
            // Missing, duplicate or foreign IDs must still require review.
            return Ok(Vec::new());
        };
        let mut durations = Vec::with_capacity(order.len());
        for index in order {
            let file = &files[index];
            durations.push(match file.duration_ms {
                Some(duration) => duration,
                None => {
                    probe_audio_file(Path::new(&file.temp_file_path))
                        .await?
                        .duration_ms
                }
            });
        }
        Ok(durations)
    }
}

#[cfg(test)]
mod download_identity_tests {
    use super::*;

    fn file(name: &str, duration: i64) -> IngestionFile {
        let mut file = IngestionFile::new(name, "job", name, 1, name);
        file.duration_ms = Some(duration);
        file
    }

    #[test]
    fn id_named_downloads_follow_album_order_including_disc_boundaries() {
        let files = vec![
            file("aaa.ogg", 3000),
            file("zzz.ogg", 1000),
            file("mmm.ogg", 2000),
        ];
        let order = download_file_order(&files, &["zzz", "mmm", "aaa"]).unwrap();
        let durations: Vec<_> = order
            .iter()
            .map(|&i| files[i].duration_ms.unwrap())
            .collect();
        assert_eq!(durations, vec![1000, 2000, 3000]);
        assert_eq!(
            compare_durations(&durations, &[1000, 2000, 3000], 100),
            (3, 0)
        );
    }

    #[test]
    fn incomplete_duplicate_and_foreign_ids_are_not_auto_accepted() {
        assert!(download_file_order(&[file("a.ogg", 1)], &["a", "b"]).is_none());
        assert!(download_file_order(&[file("a.ogg", 1), file("a.mp3", 1)], &["a", "b"]).is_none());
        assert!(
            download_file_order(&[file("a.ogg", 1), file("foreign.ogg", 1)], &["a", "b"]).is_none()
        );
        assert!(download_file_order(&[file("a.ogg", 1), file("b.ogg", 1)], &["a", "a"]).is_none());
    }
}
