//! Ingestion event notifier.
//!
//! Broadcasts real-time ingestion updates via WebSocket to the user who initiated the upload.

use std::sync::Arc;

use tracing::{debug, warn};

use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::ingestion::{
    CandidateSummary, JobCompletedUpdate, JobFailedUpdate, JobProgressUpdate, MatchFoundUpdate,
    ReviewNeededUpdate, ReviewOptionSummary,
};
use crate::server::websocket::messages::{msg_types, ServerMessage};

use super::models::{IngestionJob, IngestionJobStatus, ReviewOption, TicketType};

/// Handles WebSocket notifications for ingestion events.
pub struct IngestionNotifier {
    connection_manager: Arc<ConnectionManager>,
}

impl IngestionNotifier {
    /// Create a new ingestion notifier.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self { connection_manager }
    }

    /// Parse user_id string to usize for WebSocket lookup.
    fn parse_user_id(user_id: &str) -> Option<usize> {
        user_id.parse::<usize>().ok()
    }

    /// Notify progress during file analysis.
    pub async fn notify_progress(
        &self,
        job: &IngestionJob,
        phase: &str,
        phase_progress: u8,
        files_processed: u32,
    ) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            warn!("Invalid user_id in job {}: {}", job.id, job.user_id);
            return;
        };

        let update = JobProgressUpdate::new(&job.id, job.status.as_str(), phase).with_progress(
            phase_progress,
            files_processed,
            job.file_count as u32,
        );

        let msg = ServerMessage::new(msg_types::INGESTION_PROGRESS, update);
        self.broadcast(user_id, msg).await;
    }

    /// Notify that an album match was found.
    pub async fn notify_match_found(
        &self,
        job: &IngestionJob,
        ticket_type: TicketType,
        candidates: Vec<CandidateSummary>,
    ) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            return;
        };

        let update = MatchFoundUpdate {
            job_id: job.id.clone(),
            album_id: job.matched_album_id.clone(),
            album_name: job.detected_album.clone(),
            artist_name: job.detected_artist.clone(),
            confidence: job.match_confidence.unwrap_or(0.0),
            ticket_type: ticket_type.as_str().to_string(),
            candidates,
        };

        let msg = ServerMessage::new(msg_types::INGESTION_MATCH_FOUND, update);
        self.broadcast(user_id, msg).await;
    }

    /// Notify that a review is needed.
    pub async fn notify_review_needed(
        &self,
        job: &IngestionJob,
        question: &str,
        options: &[ReviewOption],
    ) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            return;
        };

        let options = options
            .iter()
            .map(|o| ReviewOptionSummary {
                id: o.id.clone(),
                label: o.label.clone(),
                description: o.description.clone(),
            })
            .collect();

        let update = ReviewNeededUpdate {
            job_id: job.id.clone(),
            question: question.to_string(),
            options,
        };

        let msg = ServerMessage::new(msg_types::INGESTION_REVIEW_NEEDED, update);
        self.broadcast(user_id, msg).await;
    }

    /// Notify that a job completed successfully.
    pub async fn notify_completed(
        &self,
        job: &IngestionJob,
        tracks_added: u32,
        album_name: &str,
        artist_name: &str,
    ) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            return;
        };

        let update = JobCompletedUpdate {
            job_id: job.id.clone(),
            tracks_added,
            album_name: album_name.to_string(),
            artist_name: artist_name.to_string(),
        };

        let msg = ServerMessage::new(msg_types::INGESTION_COMPLETED, update);
        self.broadcast(user_id, msg).await;
    }

    /// Notify that a job failed.
    pub async fn notify_failed(&self, job: &IngestionJob, error: &str) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            return;
        };

        let update = JobFailedUpdate {
            job_id: job.id.clone(),
            error: error.to_string(),
        };

        let msg = ServerMessage::new(msg_types::INGESTION_FAILED, update);
        self.broadcast(user_id, msg).await;
    }

    /// Notify status change (generic update).
    pub async fn notify_status_change(&self, job: &IngestionJob) {
        let Some(user_id) = Self::parse_user_id(&job.user_id) else {
            return;
        };

        use crate::server::websocket::messages::ingestion::IngestionUpdateMessage;

        let mut msg = IngestionUpdateMessage::new(&job.id, job.status.as_str());

        if let Some(album_id) = &job.matched_album_id {
            msg = msg.with_match(album_id.clone(), job.match_confidence.unwrap_or(0.0));
        }

        if let Some(error) = &job.error_message {
            msg = msg.with_error(error.clone());
        }

        if job.status == IngestionJobStatus::AwaitingReview {
            msg = msg.with_review();
        }

        let ws_msg = ServerMessage::new(msg_types::INGESTION_UPDATE, msg);
        self.broadcast(user_id, ws_msg).await;
    }

    /// Broadcast message to all of user's connected devices.
    async fn broadcast(&self, user_id: usize, message: ServerMessage) {
        let failed = self
            .connection_manager
            .broadcast_to_user(user_id, message)
            .await;

        if !failed.is_empty() {
            debug!(
                "Failed to send ingestion update to {} devices for user {}",
                failed.len(),
                user_id
            );
        }
    }
}
