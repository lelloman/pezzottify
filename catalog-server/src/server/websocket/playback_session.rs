//! Playback session manager for server-side playback state.
//!
//! Manages per-user playback sessions with independent per-device state tracking.
//! Each device independently reports its playback state, and the server relays
//! updates to other devices of the same user.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::connection::ConnectionManager;
use super::messages::{msg_types, ServerMessage};
use super::playback_messages::*;

/// Maximum queue size to prevent memory exhaustion.
const MAX_QUEUE_SIZE: usize = 500;

/// Duration after which a device's playback state is considered stale
/// and automatically removed.
const STALE_DEVICE_TIMEOUT: Duration = Duration::from_secs(120);

/// Manages playback sessions for all users.
pub struct PlaybackSessionManager {
    sessions: Arc<RwLock<HashMap<usize, UserPlaybackSession>>>,
    /// Device metadata indexed by (user_id, device_id).
    devices: RwLock<HashMap<(usize, usize), DeviceMetadata>>,
    connection_manager: Arc<ConnectionManager>,
}

/// Metadata about a connected device.
#[derive(Debug, Clone)]
struct DeviceMetadata {
    name: String,
    device_type: DeviceType,
    connected_at: u64,
}

/// Per-device playback state stored on the server.
#[derive(Debug, Clone)]
struct DevicePlaybackState {
    state: PlaybackState,
    queue: Vec<QueueItem>,
    queue_version: u64,
    last_update: Instant,
}

/// Per-user playback session tracking all device states.
#[derive(Debug, Default)]
struct UserPlaybackSession {
    device_states: HashMap<usize, DevicePlaybackState>,
}

/// Errors that can occur during playback operations.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackError {
    /// Queue size limit exceeded.
    QueueLimitExceeded,
    /// Invalid message format or type.
    InvalidMessage(String),
    /// Command execution failed.
    CommandFailed(String),
    /// Device not found.
    DeviceNotFound,
}

impl std::fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackError::QueueLimitExceeded => {
                write!(f, "Queue size limit exceeded (max {})", MAX_QUEUE_SIZE)
            }
            PlaybackError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            PlaybackError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            PlaybackError::DeviceNotFound => write!(f, "Device not found"),
        }
    }
}

impl std::error::Error for PlaybackError {}

impl From<PlaybackError> for PlaybackErrorPayload {
    fn from(e: PlaybackError) -> Self {
        let code = match &e {
            PlaybackError::QueueLimitExceeded => "queue_limit_exceeded",
            PlaybackError::InvalidMessage(_) => "invalid_message",
            PlaybackError::CommandFailed(_) => "command_failed",
            PlaybackError::DeviceNotFound => "device_not_found",
        };
        PlaybackErrorPayload {
            code: code.to_string(),
            message: e.to_string(),
            context: None,
        }
    }
}

impl PlaybackSessionManager {
    /// Create a new playback session manager.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            devices: RwLock::new(HashMap::new()),
            connection_manager,
        }
    }

    /// Called when a device sends hello - returns welcome payload.
    pub async fn handle_hello(
        &self,
        user_id: usize,
        device_id: usize,
        device_name: String,
        device_type: DeviceType,
    ) -> WelcomePayload {
        info!(
            "[playback] handle_hello: user={} device={} name={:?} type={:?}",
            user_id, device_id, device_name, device_type
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Store device metadata
        {
            let mut devices = self.devices.write().await;
            devices.insert(
                (user_id, device_id),
                DeviceMetadata {
                    name: device_name.clone(),
                    device_type,
                    connected_at: now,
                },
            );
        }

        // Get or create session
        let sessions = self.sessions.read().await;
        let session = sessions.get(&user_id);

        // Build active devices list for session info
        let devices_guard = self.devices.read().await;
        let active_devices: Vec<DevicePlaybackInfo> = session
            .map(|s| {
                s.device_states
                    .iter()
                    .map(|(did, ds)| {
                        let name = devices_guard
                            .get(&(user_id, *did))
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        DevicePlaybackInfo {
                            device_id: *did,
                            device_name: name,
                            state: ds.state.clone(),
                            queue: ds.queue.clone(),
                            queue_version: ds.queue_version,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        drop(devices_guard);

        let session_info = SessionInfo { active_devices };

        info!(
            "[playback] session_info: {} active devices",
            session_info.active_devices.len(),
        );

        // Get list of connected devices
        let connected_devices = self.build_device_list(user_id, session).await;

        drop(sessions);

        // Broadcast device list changed to other devices
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices.clone(),
                change: DeviceChange {
                    change_type: "connected".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, change_msg)
            .await;

        info!(
            "[playback] handle_hello complete: returning welcome with {} devices",
            connected_devices.len(),
        );

        WelcomePayload {
            device_id,
            session: session_info,
            devices: connected_devices,
        }
    }

    /// Build the list of connected devices for a user.
    async fn build_device_list(
        &self,
        user_id: usize,
        session: Option<&UserPlaybackSession>,
    ) -> Vec<ConnectedDevice> {
        let device_ids = self.connection_manager.get_connected_devices(user_id).await;
        let devices = self.devices.read().await;

        debug!(
            "[playback] build_device_list: user={} connection_manager_devices={:?}",
            user_id, device_ids
        );

        device_ids
            .into_iter()
            .filter_map(|id| {
                devices.get(&(user_id, id)).map(|meta| ConnectedDevice {
                    id,
                    name: meta.name.clone(),
                    device_type: meta.device_type,
                    is_playing: session
                        .map(|s| s.device_states.contains_key(&id))
                        .unwrap_or(false),
                    connected_at: meta.connected_at,
                })
            })
            .collect()
    }

    /// Handle state broadcast from any device.
    pub async fn handle_state_update(
        &self,
        user_id: usize,
        device_id: usize,
        state: PlaybackState,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] handle_state_update: user={} device={} track={:?} pos={:.1}s playing={}",
            user_id,
            device_id,
            state.current_track.as_ref().map(|t| &t.title),
            state.position,
            state.is_playing
        );

        // Get device name for relay
        let device_name = self
            .get_device_name(user_id, device_id)
            .await
            .unwrap_or_else(|| "Unknown".to_string());

        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(user_id).or_default();

        // If the state has no current track and is not playing, the device is stopping
        if state.current_track.is_none() && !state.is_playing {
            // Remove device state
            session.device_states.remove(&device_id);

            // Relay stopped to other devices
            let stopped_msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_STOPPED,
                DeviceStoppedPayload {
                    device_id,
                    reason: "stopped".to_string(),
                },
            );
            let _ = self
                .connection_manager
                .send_to_other_devices(user_id, device_id, stopped_msg)
                .await;

            // Update device list
            let connected_devices = self.build_device_list(user_id, Some(session)).await;
            let change_msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
                DeviceListChangedPayload {
                    devices: connected_devices,
                    change: DeviceChange {
                        change_type: "stopped_playing".to_string(),
                        device_id,
                    },
                },
            );
            let _ = self
                .connection_manager
                .send_to_other_devices(user_id, device_id, change_msg)
                .await;

            return Ok(());
        }

        // Update or create device state
        let was_new = !session.device_states.contains_key(&device_id);
        let entry = session
            .device_states
            .entry(device_id)
            .or_insert_with(|| DevicePlaybackState {
                state: state.clone(),
                queue: Vec::new(),
                queue_version: 0,
                last_update: Instant::now(),
            });
        entry.state = state.clone();
        entry.last_update = Instant::now();

        // Relay state to other devices
        let relay_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_STATE,
            DeviceStatePayload {
                device_id,
                device_name,
                state,
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, relay_msg)
            .await;

        // If this is the first state from this device, broadcast device list changed
        if was_new {
            let connected_devices = self.build_device_list(user_id, Some(session)).await;
            let change_msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
                DeviceListChangedPayload {
                    devices: connected_devices,
                    change: DeviceChange {
                        change_type: "started_playing".to_string(),
                        device_id,
                    },
                },
            );
            let _ = self
                .connection_manager
                .send_to_other_devices(user_id, device_id, change_msg)
                .await;
        }

        Ok(())
    }

    /// Handle queue update from any device.
    pub async fn handle_queue_update(
        &self,
        user_id: usize,
        device_id: usize,
        queue: Vec<QueueItem>,
        queue_version: u64,
    ) -> Result<(), PlaybackError> {
        if queue.len() > MAX_QUEUE_SIZE {
            return Err(PlaybackError::QueueLimitExceeded);
        }

        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(user_id).or_default();

        // Update or create device state with queue
        let entry = session
            .device_states
            .entry(device_id)
            .or_insert_with(|| DevicePlaybackState {
                state: PlaybackState {
                    current_track: None,
                    queue_position: 0,
                    queue_version,
                    position: 0.0,
                    is_playing: false,
                    volume: 1.0,
                    muted: false,
                    shuffle: false,
                    repeat: RepeatMode::Off,
                    timestamp: 0,
                },
                queue: Vec::new(),
                queue_version: 0,
                last_update: Instant::now(),
            });
        entry.queue = queue.clone();
        entry.queue_version = queue_version;
        entry.last_update = Instant::now();

        // Relay to other devices
        let relay_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_QUEUE,
            DeviceQueuePayload {
                device_id,
                queue,
                queue_version,
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, relay_msg)
            .await;

        Ok(())
    }

    /// Handle command from remote device (stub - remote control not yet implemented).
    pub async fn handle_command(
        &self,
        _user_id: usize,
        _from_device_id: usize,
        _command: &str,
        _payload: serde_json::Value,
    ) -> Result<(), PlaybackError> {
        Err(PlaybackError::CommandFailed(
            "remote control not implemented yet".to_string(),
        ))
    }

    /// Handle queue sync request - returns combined queue from all devices.
    pub async fn handle_request_queue(
        &self,
        user_id: usize,
        device_id: usize,
    ) -> Result<QueueSyncPayload, PlaybackError> {
        let sessions = self.sessions.read().await;

        // Try to find any device's queue to return
        if let Some(session) = sessions.get(&user_id) {
            // Prefer the requesting device's own queue, otherwise pick any
            if let Some(ds) = session.device_states.get(&device_id) {
                return Ok(QueueSyncPayload {
                    queue: ds.queue.clone(),
                    queue_version: ds.queue_version,
                });
            }
            // Return first available device's queue
            if let Some(ds) = session.device_states.values().next() {
                return Ok(QueueSyncPayload {
                    queue: ds.queue.clone(),
                    queue_version: ds.queue_version,
                });
            }
        }

        Ok(QueueSyncPayload {
            queue: Vec::new(),
            queue_version: 0,
        })
    }

    /// Called when a device disconnects.
    pub async fn handle_device_disconnect(&self, user_id: usize, device_id: usize) {
        info!(
            "[playback] handle_device_disconnect: user={} device={}",
            user_id, device_id
        );

        // Remove device metadata
        {
            let mut devices = self.devices.write().await;
            devices.remove(&(user_id, device_id));
        }

        // Remove device's playback state
        let mut sessions = self.sessions.write().await;
        let had_state = if let Some(session) = sessions.get_mut(&user_id) {
            session.device_states.remove(&device_id).is_some()
        } else {
            false
        };

        let session = sessions.get(&user_id);

        // Broadcast device_stopped if the device had active state
        if had_state {
            let stopped_msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_STOPPED,
                DeviceStoppedPayload {
                    device_id,
                    reason: "disconnected".to_string(),
                },
            );
            let _ = self
                .connection_manager
                .broadcast_to_user(user_id, stopped_msg)
                .await;
        }

        // Broadcast device list changed
        let connected_devices = self.build_device_list(user_id, session).await;
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices,
                change: DeviceChange {
                    change_type: "disconnected".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .broadcast_to_user(user_id, change_msg)
            .await;

        info!(
            "[playback] handle_device_disconnect complete for device {}",
            device_id
        );
    }

    /// Check for stale device states and remove them.
    pub async fn check_stale_devices(&self) {
        let mut sessions = self.sessions.write().await;

        for (user_id, session) in sessions.iter_mut() {
            let stale_ids: Vec<usize> = session
                .device_states
                .iter()
                .filter(|(_, ds)| ds.last_update.elapsed() >= STALE_DEVICE_TIMEOUT)
                .map(|(id, _)| *id)
                .collect();

            for device_id in stale_ids {
                info!(
                    "[playback] stale device timeout: user={} device={}",
                    user_id, device_id
                );
                session.device_states.remove(&device_id);

                let stopped_msg = ServerMessage::new(
                    msg_types::PLAYBACK_DEVICE_STOPPED,
                    DeviceStoppedPayload {
                        device_id,
                        reason: "stale".to_string(),
                    },
                );
                let _ = self
                    .connection_manager
                    .broadcast_to_user(*user_id, stopped_msg)
                    .await;
            }
        }
    }

    /// Get the name of a connected device.
    pub async fn get_device_name(&self, user_id: usize, device_id: usize) -> Option<String> {
        let devices = self.devices.read().await;
        devices
            .get(&(user_id, device_id))
            .map(|meta| meta.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (Arc<ConnectionManager>, PlaybackSessionManager) {
        let conn_manager = Arc::new(ConnectionManager::new());
        let session_manager = PlaybackSessionManager::new(conn_manager.clone());
        (conn_manager, session_manager)
    }

    fn make_state(track_title: &str, is_playing: bool) -> PlaybackState {
        PlaybackState {
            current_track: Some(PlaybackTrack {
                id: "track-1".to_string(),
                title: track_title.to_string(),
                artist_id: "artist-1".to_string(),
                artist_name: "Artist".to_string(),
                artists_ids: vec!["artist-1".to_string()],
                album_id: "album-1".to_string(),
                album_title: "Album".to_string(),
                duration: 180.0,
                track_number: None,
                image_id: None,
            }),
            queue_position: 0,
            queue_version: 1,
            position: 30.0,
            is_playing,
            volume: 0.8,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            timestamp: 1000,
        }
    }

    fn make_stopped_state() -> PlaybackState {
        PlaybackState {
            current_track: None,
            queue_position: 0,
            queue_version: 0,
            position: 0.0,
            is_playing: false,
            volume: 1.0,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            timestamp: 0,
        }
    }

    #[tokio::test]
    async fn handle_hello_returns_welcome() {
        let (_, manager) = setup().await;

        let welcome = manager
            .handle_hello(1, 100, "Chrome on Windows".to_string(), DeviceType::Web)
            .await;

        assert_eq!(welcome.device_id, 100);
        assert!(welcome.session.active_devices.is_empty());
    }

    #[tokio::test]
    async fn any_device_can_send_state() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        let state = make_state("Test Song", true);
        let result = manager.handle_state_update(1, 100, state).await;
        assert!(result.is_ok());

        // Verify state is stored
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert!(session.device_states.contains_key(&100));
        assert_eq!(
            session.device_states[&100].state.current_track.as_ref().unwrap().title,
            "Test Song"
        );
    }

    #[tokio::test]
    async fn multiple_devices_independent_state() {
        let (conn_manager, manager) = setup().await;

        let _rx1 = conn_manager.register(1, 100, "web".to_string()).await;
        let _rx2 = conn_manager.register(1, 200, "android".to_string()).await;

        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        let _ = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        // Both devices send state
        let state1 = make_state("Song A", true);
        let state2 = make_state("Song B", false);
        manager.handle_state_update(1, 100, state1).await.unwrap();
        manager.handle_state_update(1, 200, state2).await.unwrap();

        // Both states should be stored independently
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert_eq!(session.device_states.len(), 2);
        assert_eq!(
            session.device_states[&100].state.current_track.as_ref().unwrap().title,
            "Song A"
        );
        assert_eq!(
            session.device_states[&200].state.current_track.as_ref().unwrap().title,
            "Song B"
        );
        assert!(session.device_states[&100].state.is_playing);
        assert!(!session.device_states[&200].state.is_playing);
    }

    #[tokio::test]
    async fn welcome_includes_active_device_states() {
        let (conn_manager, manager) = setup().await;

        let _rx1 = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Device 100 sends state
        let state = make_state("Active Song", true);
        manager.handle_state_update(1, 100, state).await.unwrap();

        // Device 200 connects and gets welcome
        let _rx2 = conn_manager.register(1, 200, "android".to_string()).await;
        let welcome = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        assert_eq!(welcome.session.active_devices.len(), 1);
        assert_eq!(welcome.session.active_devices[0].device_id, 100);
        assert_eq!(welcome.session.active_devices[0].device_name, "Chrome");
        assert_eq!(
            welcome.session.active_devices[0].state.current_track.as_ref().unwrap().title,
            "Active Song"
        );
    }

    #[tokio::test]
    async fn device_state_removed_on_disconnect() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Send state
        let state = make_state("Test Song", true);
        manager.handle_state_update(1, 100, state).await.unwrap();

        // Verify state exists
        {
            let sessions = manager.sessions.read().await;
            assert!(sessions.get(&1).unwrap().device_states.contains_key(&100));
        }

        // Disconnect
        manager.handle_device_disconnect(1, 100).await;

        // State should be removed
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert!(!session.device_states.contains_key(&100));
    }

    #[tokio::test]
    async fn device_stopped_via_empty_state() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Send state
        let state = make_state("Test Song", true);
        manager.handle_state_update(1, 100, state).await.unwrap();

        // Send stopped state
        let stopped = make_stopped_state();
        manager.handle_state_update(1, 100, stopped).await.unwrap();

        // State should be removed
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert!(!session.device_states.contains_key(&100));
    }

    #[tokio::test]
    async fn queue_update_rejects_oversized_queue() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        let queue: Vec<QueueItem> = (0..=MAX_QUEUE_SIZE)
            .map(|i| QueueItem {
                id: format!("track-{}", i),
                added_at: 0,
            })
            .collect();

        let result = manager.handle_queue_update(1, 100, queue, 1).await;
        assert!(matches!(result, Err(PlaybackError::QueueLimitExceeded)));
    }

    #[tokio::test]
    async fn command_returns_not_implemented() {
        let (_, manager) = setup().await;

        let result = manager
            .handle_command(1, 100, "play", serde_json::Value::Null)
            .await;
        assert!(matches!(result, Err(PlaybackError::CommandFailed(_))));
    }

    #[tokio::test]
    async fn stale_device_cleanup() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Send state
        let state = make_state("Test Song", true);
        manager.handle_state_update(1, 100, state).await.unwrap();

        // Manually set last_update to be stale
        {
            let mut sessions = manager.sessions.write().await;
            let session = sessions.get_mut(&1).unwrap();
            let ds = session.device_states.get_mut(&100).unwrap();
            ds.last_update = Instant::now() - STALE_DEVICE_TIMEOUT - Duration::from_secs(1);
        }

        // Run cleanup
        manager.check_stale_devices().await;

        // State should be removed
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert!(!session.device_states.contains_key(&100));
    }
}
