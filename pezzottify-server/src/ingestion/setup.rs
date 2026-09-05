impl IngestionManager {
    /// Create a new IngestionManager.
    pub fn new(
        store: Arc<dyn IngestionStore>,
        catalog: Arc<dyn CatalogStore>,
        search: Arc<dyn SearchVault>,
        config: IngestionManagerConfig,
        download_manager: Option<Arc<dyn DownloadManagerTrait>>,
    ) -> Self {
        let file_handler = FileHandler::new(&config.temp_dir, config.max_file_size);

        let media = Arc::new(crate::media::MediaManager::new(catalog.clone(), crate::db_executor::DbExecutor::new(Default::default())));
        media.configure_search(search.clone());
        Self {
            media,
            store,
            catalog,
            search,
            file_handler,
            config,
            download_manager,
            notifier: None,
            notification_service: None,
        }
    }

    pub fn with_media(mut self, media: Arc<crate::media::MediaManager>) -> Self {
        self.media = media;
        self
    }

    /// Set the notifier for WebSocket updates.
    pub fn with_notifier(mut self, notifier: Arc<super::notifier::IngestionNotifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// Set the notification service for download completion notifications.
    pub fn with_notification_service(
        mut self,
        service: Arc<crate::notifications::NotificationService>,
    ) -> Self {
        self.notification_service = Some(service);
        self
    }

    /// Initialize the manager (creates temp directory, etc.).
    pub async fn init(&self) -> Result<()> {
        self.file_handler.init().await?;
        Ok(())
    }

    /// Send a download-completed notification to a user.
    /// All errors are logged as warnings, never propagated.
    async fn send_download_notification(
        &self,
        user_id_str: &str,
        request_id: &str,
        album_id: &str,
        album_name: &str,
        artist_name: &str,
    ) {
        let notification_service = match &self.notification_service {
            Some(svc) => svc,
            None => return,
        };

        let user_id = match user_id_str.parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                warn!(
                    "Cannot send download notification: failed to parse user_id '{}'",
                    user_id_str
                );
                return;
            }
        };

        let image_id = match self.catalog.get_album_image_url(album_id) {
            Ok(Some(_)) => Some(album_id.to_string()),
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to get album image for notification: {}", e);
                None
            }
        };

        let data = crate::notifications::DownloadCompletedData {
            album_id: album_id.to_string(),
            album_name: album_name.to_string(),
            artist_name: artist_name.to_string(),
            image_id,
            request_id: request_id.to_string(),
        };

        let data_json = match serde_json::to_value(&data) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to serialize notification data: {}", e);
                return;
            }
        };

        if let Err(e) = notification_service
            .create_notification(
                user_id,
                crate::notifications::NotificationType::DownloadCompleted,
                format!("{} is ready", album_name),
                Some(format!("by {}", artist_name)),
                data_json,
            )
            .await
        {
            warn!(
                "Failed to create download notification for user {}: {}",
                user_id, e
            );
        }
    }

    /// Mark a job as failed, clean up temp files, and notify.
    ///
    /// This is the centralized failure handling for ingestion jobs. It:
    /// 1. Sets the job status to Failed
    /// 2. Records the error message
    /// 3. Sets the completed_at timestamp
    /// 4. Updates the job in the store
    /// 5. Cleans up temporary files
    /// 6. Notifies via WebSocket (if notifier is configured)
    async fn fail_job_with_cleanup(
        &self,
        job: &mut IngestionJob,
        error_message: &str,
    ) -> Result<(), IngestionError> {
        job.status = IngestionJobStatus::Failed;
        job.error_message = Some(error_message.to_string());
        job.completed_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(job)?;

        // Clean up temp files
        if let Err(e) = self.file_handler.cleanup_job(&job.id).await {
            warn!("Failed to cleanup temp files for job {}: {}", job.id, e);
        }

        // Notify failure
        if let Some(notifier) = &self.notifier {
            notifier.notify_failed(job, error_message).await;
        }

        // If this job is from a download request, mark the queue item as failed
        if let (Some(IngestionContextType::DownloadRequest), Some(context_id)) =
            (job.context_type, &job.context_id)
        {
            if let Some(dm) = &self.download_manager {
                if let Err(e) = dm.mark_request_failed(context_id, error_message) {
                    warn!(
                        "Failed to mark download request {} as failed: {}",
                        context_id, e
                    );
                }
            }
        }

        Ok(())
    }

}
