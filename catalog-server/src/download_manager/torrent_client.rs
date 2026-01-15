//! Quentin Torrentino HTTP + WebSocket client.
//!
//! Provides communication with the Quentin Torrentino torrent download service.

use super::torrent_types::*;
use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Trait for Quentin Torrentino client operations needed by DownloadManager.
#[async_trait::async_trait]
pub trait TorrentClientTrait: Send + Sync {
    /// Check if WebSocket is currently connected.
    fn is_connected(&self) -> bool;

    /// Subscribe to ticket events.
    fn subscribe(&self) -> broadcast::Receiver<TorrentEvent>;

    /// Run the WebSocket connection loop.
    async fn run_websocket(&self) -> Result<()>;

    /// Create a new ticket for downloading music.
    async fn create_ticket(&self, request: CreateTicketRequest) -> Result<TicketResponse>;

    /// Cancel a ticket.
    async fn cancel(&self, ticket_id: &str) -> Result<()>;

    /// Get the current state of a ticket.
    async fn get_ticket(&self, ticket_id: &str) -> Result<TicketState>;

    /// Retry a failed ticket.
    async fn retry_ticket(&self, ticket_id: &str) -> Result<()>;

    /// Check if Quentin Torrentino is healthy.
    async fn health_check(&self) -> Result<bool>;

    /// Get QT statistics.
    async fn get_stats(&self) -> Result<QTStats>;
}

/// Client for communicating with Quentin Torrentino.
pub struct TorrentClient {
    http_client: Client,
    base_url: String,
    ws_url: String,
    auth_token: String,
    connected: Arc<AtomicBool>,
    event_tx: broadcast::Sender<TorrentEvent>,
}

impl TorrentClient {
    /// Create a new TorrentClient.
    pub fn new(base_url: String, ws_url: String, auth_token: String) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            http_client: Client::new(),
            base_url,
            ws_url,
            auth_token,
            connected: Arc::new(AtomicBool::new(false)),
            event_tx,
        }
    }

    /// List tickets with optional state filter.
    ///
    /// This method is not part of the trait as it's only used for admin operations.
    pub async fn list_tickets(
        &self,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<TicketState>> {
        let mut url = format!(
            "{}/api/v1/tickets?limit={}&offset={}",
            self.base_url, limit, offset
        );
        if let Some(s) = state {
            url.push_str(&format!("&state={}", s));
        }

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    /// Approve a ticket (for NEEDS_APPROVAL state).
    ///
    /// This method is not part of the trait as it's only used for admin operations.
    pub async fn approve(&self, ticket_id: &str, candidate_idx: Option<usize>) -> Result<()> {
        let url = format!("{}/api/v1/tickets/{}/approve", self.base_url, ticket_id);
        let body = serde_json::json!({ "candidate_idx": candidate_idx });
        self.http_client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Reject a ticket.
    ///
    /// This method is not part of the trait as it's only used for admin operations.
    pub async fn reject(&self, ticket_id: &str, reason: &str) -> Result<()> {
        let url = format!("{}/api/v1/tickets/{}/reject", self.base_url, ticket_id);
        let body = serde_json::json!({ "reason": reason });
        self.http_client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Internal WebSocket implementation (called by trait method).
    async fn run_websocket_internal(&self) -> Result<()> {
        let url = format!("{}?token={}", self.ws_url, self.auth_token);
        info!(
            "Connecting to Quentin Torrentino WebSocket: {}",
            self.ws_url
        );

        let (ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| anyhow!("WebSocket connection failed: {}", e))?;

        self.connected.store(true, Ordering::SeqCst);
        info!("Connected to Quentin Torrentino WebSocket");

        let (mut write, mut read) = ws_stream.split();

        // Send ping periodically to keep connection alive
        let connected = self.connected.clone();
        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            while connected.load(Ordering::SeqCst) {
                interval.tick().await;
                // Ping is handled automatically by tungstenite
            }
        });

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => match serde_json::from_str::<TorrentEvent>(&text) {
                    Ok(event) => {
                        debug!("Received TorrentEvent: {:?}", event);
                        if let Err(e) = self.event_tx.send(event) {
                            warn!("No subscribers for event: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse WebSocket message: {} - {}", e, text);
                    }
                },
                Ok(Message::Ping(data)) => {
                    if let Err(e) = write.send(Message::Pong(data)).await {
                        error!("Failed to send pong: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        self.connected.store(false, Ordering::SeqCst);
        ping_task.abort();
        Err(anyhow!("WebSocket connection closed"))
    }
}

#[async_trait::async_trait]
impl TorrentClientTrait for TorrentClient {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn subscribe(&self) -> broadcast::Receiver<TorrentEvent> {
        self.event_tx.subscribe()
    }

    async fn run_websocket(&self) -> Result<()> {
        self.run_websocket_internal().await
    }

    async fn create_ticket(&self, request: CreateTicketRequest) -> Result<TicketResponse> {
        let url = format!("{}/api/v1/tickets", self.base_url);
        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    async fn cancel(&self, ticket_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/tickets/{}", self.base_url, ticket_id);
        self.http_client
            .delete(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn get_ticket(&self, ticket_id: &str) -> Result<TicketState> {
        let url = format!("{}/api/v1/tickets/{}", self.base_url, ticket_id);
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    async fn retry_ticket(&self, ticket_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/tickets/{}/retry", self.base_url, ticket_id);
        self.http_client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/v1/health", self.base_url);
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;
        Ok(response.status().is_success())
    }

    async fn get_stats(&self) -> Result<QTStats> {
        let url = format!("{}/api/v1/stats", self.base_url);
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = TorrentClient::new(
            "http://localhost:8080".to_string(),
            "ws://localhost:8080/api/v1/ws".to_string(),
            "test-token".to_string(),
        );
        assert!(!client.is_connected());
    }

    #[test]
    fn test_event_subscription() {
        let client = TorrentClient::new(
            "http://localhost:8080".to_string(),
            "ws://localhost:8080/api/v1/ws".to_string(),
            "test-token".to_string(),
        );

        let _rx1 = client.subscribe();
        let _rx2 = client.subscribe();
        // Multiple subscriptions should work
    }
}
