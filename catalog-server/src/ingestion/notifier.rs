//! Ingestion event notifier.
//!
//! Broadcasts real-time ingestion updates via WebSocket to the user who initiated the upload.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::catalog::CatalogInvalidationMessage;
use crate::server::websocket::messages::ingestion::{
    CandidateSummary, JobCompletedUpdate, JobFailedUpdate, JobProgressUpdate, MatchFoundUpdate,
    ReviewNeededUpdate, ReviewOptionSummary,
};
use crate::server::websocket::messages::{msg_types, ServerMessage};
use crate::server_store::{CatalogContentType, CatalogEvent, CatalogEventType, ServerStore};

use super::models::{IngestionJob, IngestionJobStatus, ReviewOption, TicketType};

/// Handles WebSocket notifications for ingestion events.
pub struct IngestionNotifier {
    connection_manager: Arc<ConnectionManager>,
    server_store: Option<Arc<dyn ServerStore>>,
}

impl IngestionNotifier {
    /// Create a new ingestion notifier.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
            server_store: None,
        }
    }

    /// Set the server store for catalog event persistence.
    pub fn with_server_store(mut self, server_store: Arc<dyn ServerStore>) -> Self {
        self.server_store = Some(server_store);
        self
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

    /// Emit a catalog invalidation event and broadcast to ALL connected clients.
    ///
    /// This is called when new content is added to the catalog during ingestion,
    /// allowing all clients to invalidate their cached data.
    pub async fn emit_catalog_event(
        &self,
        event_type: CatalogEventType,
        content_type: CatalogContentType,
        content_id: &str,
        triggered_by: &str,
    ) {
        // Store the event in the server database if we have a store
        let event = if let Some(store) = &self.server_store {
            match store.append_catalog_event(
                event_type.clone(),
                content_type.clone(),
                content_id,
                Some(triggered_by),
            ) {
                Ok(seq) => CatalogEvent {
                    seq,
                    event_type: event_type.clone(),
                    content_type: content_type.clone(),
                    content_id: content_id.to_string(),
                    timestamp: chrono::Utc::now().timestamp(),
                    triggered_by: Some(triggered_by.to_string()),
                },
                Err(e) => {
                    warn!("Failed to store catalog event: {}", e);
                    CatalogEvent {
                        seq: 0,
                        event_type: event_type.clone(),
                        content_type: content_type.clone(),
                        content_id: content_id.to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                        triggered_by: Some(triggered_by.to_string()),
                    }
                }
            }
        } else {
            CatalogEvent {
                seq: 0,
                event_type: event_type.clone(),
                content_type: content_type.clone(),
                content_id: content_id.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                triggered_by: Some(triggered_by.to_string()),
            }
        };

        // Broadcast to all connected clients
        let ws_msg = ServerMessage::new(
            msg_types::CATALOG_INVALIDATION,
            CatalogInvalidationMessage {
                seq: event.seq,
                event_type: event.event_type.as_str().to_string(),
                content_type: event.content_type.as_str().to_string(),
                content_id: event.content_id.clone(),
                timestamp: event.timestamp,
            },
        );

        let failed_count = self.connection_manager.broadcast_to_all(ws_msg).await;

        if failed_count > 0 {
            debug!(
                "Failed to send catalog_invalidation to {} connections",
                failed_count
            );
        }

        info!(
            "Broadcast catalog_invalidation: {} {} {} (seq={})",
            event.event_type.as_str(),
            event.content_type.as_str(),
            event.content_id,
            event.seq
        );
    }
}
