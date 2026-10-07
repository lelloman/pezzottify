//! WebSocket connection manager.
//!
//! Tracks all active WebSocket connections, organized by user and device.
//! Provides methods to send messages to specific devices or broadcast to users.

use std::collections::HashMap;

use tokio::sync::{mpsc, RwLock};

use super::messages::ServerMessage;

/// Information about an active WebSocket connection.
struct ConnectionEntry {
    sender: mpsc::Sender<ServerMessage>,
    #[allow(dead_code)] // Will be used for filtering by device type
    device_type: String,
}

/// Error type for send operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SendError {
    /// The target device is not connected.
    NotConnected,
    /// The connection channel is closed (device disconnected).
    Disconnected,
}

/// Manages all active WebSocket connections.
///
/// Connections are organized by user_id, then by device_id.
/// This allows efficient broadcast to all devices of a user,
/// or targeted sends to specific devices.
pub struct ConnectionManager {
    /// user_id -> (device_id -> connection entry)
    connections: RwLock<HashMap<usize, HashMap<usize, ConnectionEntry>>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    /// Create a new connection manager.
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new connection.
    ///
    /// Returns a receiver for outgoing messages. The caller should forward
    /// messages from this receiver to the WebSocket.
    ///
    /// If a connection already exists for this device, the old connection
    /// is replaced (drop-and-replace behavior).
    pub async fn register(
        &self,
        user_id: usize,
        device_id: usize,
        device_type: String,
    ) -> mpsc::Receiver<ServerMessage> {
        let (tx, rx) = mpsc::channel(32);

        let mut conns = self.connections.write().await;
        let user_conns = conns.entry(user_id).or_default();

        // If device already connected, old sender is dropped (drop-and-replace)
        user_conns.insert(
            device_id,
            ConnectionEntry {
                sender: tx,
                device_type,
            },
        );

        rx
    }

    /// Unregister a connection (called on disconnect).
    ///
    /// Removes the connection entry and cleans up empty user maps.
    pub async fn unregister(&self, user_id: usize, device_id: usize) {
        let mut conns = self.connections.write().await;
        if let Some(user_conns) = conns.get_mut(&user_id) {
            user_conns.remove(&device_id);
            if user_conns.is_empty() {
                conns.remove(&user_id);
            }
        }
    }

    /// Send a message to a specific device.
    pub async fn send_to_device(
        &self,
        user_id: usize,
        device_id: usize,
        message: ServerMessage,
    ) -> Result<(), SendError> {
        let conns = self.connections.read().await;
        if let Some(user_conns) = conns.get(&user_id) {
            if let Some(entry) = user_conns.get(&device_id) {
                entry
                    .sender
                    .send(message)
                    .await
                    .map_err(|_| SendError::Disconnected)?;
                return Ok(());
            }
        }
        Err(SendError::NotConnected)
    }

    /// Send a message to all OTHER devices of a user (excludes the sender).
    ///
    /// Returns list of device_ids that failed (disconnected).
    pub async fn send_to_other_devices(
        &self,
        user_id: usize,
        exclude_device_id: usize,
        message: ServerMessage,
    ) -> Vec<usize> {
        let conns = self.connections.read().await;
        let mut failed = Vec::new();

        if let Some(user_conns) = conns.get(&user_id) {
            for (device_id, entry) in user_conns.iter() {
                if *device_id != exclude_device_id
                    && entry.sender.send(message.clone()).await.is_err()
                {
                    failed.push(*device_id);
                }
            }
        }

        failed
    }

    /// Send a message to ALL devices of a user.
    ///
    /// Returns list of device_ids that failed (disconnected).
    pub async fn broadcast_to_user(&self, user_id: usize, message: ServerMessage) -> Vec<usize> {
        let conns = self.connections.read().await;
        let mut failed = Vec::new();

        if let Some(user_conns) = conns.get(&user_id) {
            for (device_id, entry) in user_conns.iter() {
                if entry.sender.send(message.clone()).await.is_err() {
                    failed.push(*device_id);
                }
            }
        }

        failed
    }

    /// Get list of connected device IDs for a user.
    pub async fn get_connected_devices(&self, user_id: usize) -> Vec<usize> {
        let conns = self.connections.read().await;
        conns
            .get(&user_id)
            .map(|user_conns| user_conns.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Get list of all connected devices with their user IDs.
    pub async fn get_all_connected_devices(&self) -> Vec<(usize, usize)> {
        let conns = self.connections.read().await;
        let mut devices = Vec::new();
        for (user_id, user_conns) in conns.iter() {
            for device_id in user_conns.keys() {
                devices.push((*user_id, *device_id));
            }
        }
        devices
    }

    /// Check if a specific device is connected.
    pub async fn is_device_connected(&self, user_id: usize, device_id: usize) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(&user_id)
            .map(|user_conns| user_conns.contains_key(&device_id))
            .unwrap_or(false)
    }

    /// Get the number of connected devices for a user.
    #[allow(dead_code)] // Useful for testing/debugging
    pub async fn connection_count(&self, user_id: usize) -> usize {
        let conns = self.connections.read().await;
        conns
            .get(&user_id)
            .map(|user_conns| user_conns.len())
            .unwrap_or(0)
    }

    /// Get the total number of active connections across all users.
    #[allow(dead_code)] // Useful for metrics
    pub async fn total_connections(&self) -> usize {
        let conns = self.connections.read().await;
        conns.values().map(|user_conns| user_conns.len()).sum()
    }

    /// Get list of all connected user IDs.
    pub async fn get_connected_user_ids(&self) -> Vec<usize> {
        let conns = self.connections.read().await;
        conns.keys().copied().collect()
    }

    /// Get count of unique connected users.
    pub async fn connected_user_count(&self) -> usize {
        let conns = self.connections.read().await;
        conns.len()
    }

    /// Broadcast a message to ALL connected users (all devices of all users).
    ///
    /// Used for system-wide notifications like catalog updates.
    /// Returns count of failed sends.
    pub async fn broadcast_to_all(&self, message: ServerMessage) -> usize {
        let conns = self.connections.read().await;
        let mut failed_count = 0;

        for user_conns in conns.values() {
            for entry in user_conns.values() {
                if entry.sender.send(message.clone()).await.is_err() {
                    failed_count += 1;
                }
            }
        }

        failed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_creates_valid_receiver() {
        let manager = ConnectionManager::new();
        let mut rx = manager.register(1, 100, "web".to_string()).await;

        // Should be able to receive messages
        let msg = ServerMessage::empty("test");
        manager.send_to_device(1, 100, msg.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.msg_type, "test");
    }

    #[tokio::test]
    async fn unregister_removes_connection() {
        let manager = ConnectionManager::new();
        let _rx = manager.register(1, 100, "web".to_string()).await;

        assert!(manager.is_device_connected(1, 100).await);

        manager.unregister(1, 100).await;

        assert!(!manager.is_device_connected(1, 100).await);
    }

    #[tokio::test]
    async fn send_to_device_delivers_message() {
        let manager = ConnectionManager::new();
        let mut rx = manager.register(1, 100, "web".to_string()).await;

        let msg = ServerMessage::new("greeting", serde_json::json!({"text": "hello"}));
        manager.send_to_device(1, 100, msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.msg_type, "greeting");
        assert_eq!(received.payload["text"], "hello");
    }

    #[tokio::test]
    async fn send_to_device_returns_not_connected_for_unknown() {
        let manager = ConnectionManager::new();

        let msg = ServerMessage::empty("test");
        let result = manager.send_to_device(1, 100, msg).await;

        assert_eq!(result, Err(SendError::NotConnected));
    }

    #[tokio::test]
    async fn send_to_other_devices_excludes_source() {
        let manager = ConnectionManager::new();
        let mut rx1 = manager.register(1, 100, "web".to_string()).await;
        let mut rx2 = manager.register(1, 200, "android".to_string()).await;

        let msg = ServerMessage::empty("sync");
        let failed = manager.send_to_other_devices(1, 100, msg).await;

        assert!(failed.is_empty());

        // Device 100 should NOT receive
        assert!(rx1.try_recv().is_err());

        // Device 200 should receive
        let received = rx2.recv().await.unwrap();
        assert_eq!(received.msg_type, "sync");
    }

    #[tokio::test]
    async fn send_to_other_devices_returns_failed() {
        let manager = ConnectionManager::new();
        let _rx1 = manager.register(1, 100, "web".to_string()).await;
        let rx2 = manager.register(1, 200, "android".to_string()).await;

        // Drop rx2 to simulate disconnection
        drop(rx2);

        let msg = ServerMessage::empty("sync");
        let failed = manager.send_to_other_devices(1, 100, msg).await;

        // Device 200 should be reported as failed
        assert_eq!(failed, vec![200]);
    }

    #[tokio::test]
    async fn broadcast_to_user_sends_to_all() {
        let manager = ConnectionManager::new();
        let mut rx1 = manager.register(1, 100, "web".to_string()).await;
        let mut rx2 = manager.register(1, 200, "android".to_string()).await;

        let msg = ServerMessage::empty("notification");
        let failed = manager.broadcast_to_user(1, msg).await;

        assert!(failed.is_empty());

        // Both should receive
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1.msg_type, "notification");
        assert_eq!(received2.msg_type, "notification");
    }

    #[tokio::test]
    async fn get_connected_devices_returns_correct_list() {
        let manager = ConnectionManager::new();
        let _rx1 = manager.register(1, 100, "web".to_string()).await;
        let _rx2 = manager.register(1, 200, "android".to_string()).await;
        let _rx3 = manager.register(2, 300, "web".to_string()).await;

        let mut devices = manager.get_connected_devices(1).await;
        devices.sort();

        assert_eq!(devices, vec![100, 200]);

        let devices2 = manager.get_connected_devices(2).await;
        assert_eq!(devices2, vec![300]);
    }

    #[tokio::test]
    async fn is_device_connected_returns_correct_boolean() {
        let manager = ConnectionManager::new();
        let _rx = manager.register(1, 100, "web".to_string()).await;

        assert!(manager.is_device_connected(1, 100).await);
        assert!(!manager.is_device_connected(1, 200).await);
        assert!(!manager.is_device_connected(2, 100).await);
    }

    #[tokio::test]
    async fn drop_and_replace_replaces_old_connection() {
        let manager = ConnectionManager::new();
        let mut rx1 = manager.register(1, 100, "web".to_string()).await;

        // Register same device again
        let mut rx2 = manager.register(1, 100, "web".to_string()).await;

        // Send message
        let msg = ServerMessage::empty("test");
        manager.send_to_device(1, 100, msg).await.unwrap();

        // Old receiver should get nothing (channel closed)
        assert!(rx1.recv().await.is_none());

        // New receiver should get the message
        let received = rx2.recv().await.unwrap();
        assert_eq!(received.msg_type, "test");
    }

    #[tokio::test]
    async fn connection_count_is_correct() {
        let manager = ConnectionManager::new();

        assert_eq!(manager.connection_count(1).await, 0);

        let _rx1 = manager.register(1, 100, "web".to_string()).await;
        assert_eq!(manager.connection_count(1).await, 1);

        let _rx2 = manager.register(1, 200, "android".to_string()).await;
        assert_eq!(manager.connection_count(1).await, 2);

        manager.unregister(1, 100).await;
        assert_eq!(manager.connection_count(1).await, 1);
    }

    #[tokio::test]
    async fn total_connections_counts_all_users() {
        let manager = ConnectionManager::new();

        assert_eq!(manager.total_connections().await, 0);

        let _rx1 = manager.register(1, 100, "web".to_string()).await;
        let _rx2 = manager.register(1, 200, "android".to_string()).await;
        let _rx3 = manager.register(2, 300, "web".to_string()).await;

        assert_eq!(manager.total_connections().await, 3);
    }

    #[tokio::test]
    async fn unregister_cleans_up_empty_user_map() {
        let manager = ConnectionManager::new();
        let _rx = manager.register(1, 100, "web".to_string()).await;

        manager.unregister(1, 100).await;

        // Verify internal state is cleaned up
        let conns = manager.connections.read().await;
        assert!(!conns.contains_key(&1));
    }
}
