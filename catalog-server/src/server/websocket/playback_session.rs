//! Playback session manager for server-side playback state.
//!
//! Manages per-user playback sessions with independent per-device state tracking.
//! Each device independently reports its playback state, and the server relays
//! updates to other devices of the same user.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::connection::ConnectionManager;
use super::messages::{msg_types, ServerMessage};
use super::playback_messages::*;
use crate::user::UserManager;
use crate::user::device::{DeviceShareMode, DeviceSharePolicy};
use crate::user::permissions::UserRole;

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
    user_manager: Arc<Mutex<UserManager>>,
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
    /// Forbidden action.
    Forbidden,
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
            PlaybackError::Forbidden => write!(f, "Forbidden"),
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
            PlaybackError::Forbidden => "forbidden",
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
    pub fn new(connection_manager: Arc<ConnectionManager>, user_manager: Arc<Mutex<UserManager>>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            devices: RwLock::new(HashMap::new()),
            connection_manager,
            user_manager,
        }
    }

    fn get_owner_handle(&self, user_id: usize) -> Option<String> {
        let manager = self.user_manager.lock().unwrap();
        manager.get_user_handle(user_id).ok().flatten()
    }

    async fn get_device_owner_id(&self, device_id: usize) -> Option<usize> {
        {
            let manager = self.user_manager.lock().unwrap();
            if let Ok(Some(device)) = manager.get_device(device_id) {
                if let Some(user_id) = device.user_id {
                    return Some(user_id);
                }
            }
        }
        let devices = self.devices.read().await;
        devices
            .keys()
            .find(|(_, did)| *did == device_id)
            .map(|(uid, _)| *uid)
    }

    fn get_device_share_policy(&self, device_id: usize) -> DeviceSharePolicy {
        let manager = self.user_manager.lock().unwrap();
        manager
            .get_device_share_policy(device_id)
            .unwrap_or_default()
    }

    fn get_user_roles(&self, user_id: usize) -> Vec<UserRole> {
        let manager = self.user_manager.lock().unwrap();
        manager.get_user_roles(user_id).unwrap_or_default()
    }

    fn is_user_allowed_for_device(
        &self,
        owner_user_id: usize,
        device_id: usize,
        user_id: usize,
    ) -> bool {
        if owner_user_id == user_id {
            return true;
        }

        let policy = self.get_device_share_policy(device_id);
        match policy.mode {
            DeviceShareMode::AllowEveryone => true,
            DeviceShareMode::DenyEveryone => false,
            DeviceShareMode::Custom => {
                if policy.deny_users.contains(&user_id) {
                    return false;
                }
                if policy.allow_users.contains(&user_id) {
                    return true;
                }
                let roles = self.get_user_roles(user_id);
                roles.iter().any(|role| policy.allow_roles.contains(role))
            }
        }
    }

    async fn get_allowed_external_users(&self, owner_user_id: usize, device_id: usize) -> Vec<usize> {
        let connected_users = self.connection_manager.get_connected_user_ids().await;
        connected_users
            .into_iter()
            .filter(|uid| *uid != owner_user_id)
            .filter(|uid| self.is_user_allowed_for_device(owner_user_id, device_id, *uid))
            .collect()
    }

    async fn broadcast_to_authorized_users(
        &self,
        owner_user_id: usize,
        device_id: usize,
        exclude_device_id: usize,
        message: ServerMessage,
    ) {
        let _ = self
            .connection_manager
            .send_to_other_devices(owner_user_id, exclude_device_id, message.clone())
            .await;

        let external_users = self
            .get_allowed_external_users(owner_user_id, device_id)
            .await;
        for user_id in external_users {
            let _ = self
                .connection_manager
                .broadcast_to_user(user_id, message.clone())
                .await;
        }
    }

    async fn broadcast_device_list_change(
        &self,
        owner_user_id: usize,
        device_id: usize,
        change_type: &str,
        exclude_device_id: usize,
    ) {
        let owner_devices = self.build_device_list(owner_user_id).await;
        let owner_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: owner_devices,
                change: DeviceChange {
                    change_type: change_type.to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(owner_user_id, exclude_device_id, owner_msg)
            .await;

        let external_users = self.get_allowed_external_users(owner_user_id, device_id).await;
        for user_id in external_users {
            let devices = self.build_device_list(user_id).await;
            let msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
                DeviceListChangedPayload {
                    devices,
                    change: DeviceChange {
                        change_type: change_type.to_string(),
                        device_id,
                    },
                },
            );
            let _ = self
                .connection_manager
                .broadcast_to_user(user_id, msg)
                .await;
        }
    }

    pub async fn broadcast_device_list_refresh(&self, device_id: usize) {
        let owner_user_id = match self.get_device_owner_id(device_id).await {
            Some(id) => id,
            None => return,
        };

        // Always notify the owner
        let owner_devices = self.build_device_list(owner_user_id).await;
        let owner_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: owner_devices,
                change: DeviceChange {
                    change_type: "policy_changed".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .broadcast_to_user(owner_user_id, owner_msg)
            .await;

        // Notify only users who currently have visibility to this device
        let visible_users = self
            .get_allowed_external_users(owner_user_id, device_id)
            .await;
        for user_id in visible_users {
            let devices = self.build_device_list(user_id).await;
            let msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
                DeviceListChangedPayload {
                    devices,
                    change: DeviceChange {
                        change_type: "policy_changed".to_string(),
                        device_id,
                    },
                },
            );
            let _ = self
                .connection_manager
                .broadcast_to_user(user_id, msg)
                .await;
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

        let active_devices = self.build_active_devices_for_user(user_id).await;
        let session_info = SessionInfo { active_devices };

        info!(
            "[playback] session_info: {} active devices",
            session_info.active_devices.len(),
        );

        // Get list of connected devices
        let connected_devices = self.build_device_list(user_id).await;

        // Broadcast device list changed to other devices
        self
            .broadcast_device_list_change(user_id, device_id, "connected", device_id)
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
    async fn build_device_list(&self, user_id: usize) -> Vec<ConnectedDevice> {
        let device_ids = self.connection_manager.get_all_connected_devices().await;
        let devices = self.devices.read().await;
        let sessions = self.sessions.read().await;

        debug!(
            "[playback] build_device_list: user={} connection_manager_devices={:?}",
            user_id, device_ids
        );

        device_ids
            .into_iter()
            .filter_map(|(owner_user_id, id)| {
                if owner_user_id != user_id
                    && !self.is_user_allowed_for_device(owner_user_id, id, user_id)
                {
                    return None;
                }

                devices.get(&(owner_user_id, id)).map(|meta| {
                    let is_playing = sessions
                        .get(&owner_user_id)
                        .map(|s| s.device_states.contains_key(&id))
                        .unwrap_or(false);
                    let owner_handle = self
                        .get_owner_handle(owner_user_id)
                        .unwrap_or_else(|| "Unknown".to_string());
                    ConnectedDevice {
                        id,
                        name: meta.name.clone(),
                        device_type: meta.device_type,
                        is_playing,
                        connected_at: meta.connected_at,
                        owner_user_id,
                        owner_handle,
                        is_shared: owner_user_id != user_id,
                    }
                })
            })
            .collect()
    }

    async fn build_active_devices_for_user(&self, user_id: usize) -> Vec<DevicePlaybackInfo> {
        let sessions = self.sessions.read().await;
        let devices_guard = self.devices.read().await;
        let mut active_devices = Vec::new();

        for (owner_user_id, session) in sessions.iter() {
            for (device_id, ds) in session.device_states.iter() {
                if *owner_user_id != user_id
                    && !self.is_user_allowed_for_device(*owner_user_id, *device_id, user_id)
                {
                    continue;
                }

                let name = devices_guard
                    .get(&(*owner_user_id, *device_id))
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                active_devices.push(DevicePlaybackInfo {
                    device_id: *device_id,
                    device_name: name,
                    state: ds.state.clone(),
                    queue: ds.queue.clone(),
                    queue_version: ds.queue_version,
                });
            }
        }

        active_devices
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
            self
                .broadcast_to_authorized_users(user_id, device_id, device_id, stopped_msg)
                .await;

            // Update device list
            self
                .broadcast_device_list_change(user_id, device_id, "stopped_playing", device_id)
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
        self
            .broadcast_to_authorized_users(user_id, device_id, device_id, relay_msg)
            .await;

        // If this is the first state from this device, broadcast device list changed
        if was_new {
            self
                .broadcast_device_list_change(user_id, device_id, "started_playing", device_id)
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
        self
            .broadcast_to_authorized_users(user_id, device_id, device_id, relay_msg)
            .await;

        Ok(())
    }

    /// Handle command from one device targeting another device.
    ///
    /// Validates the target device is connected and forwards the command to it.
    /// The forwarded message is `playback.command` with only `command` and `payload`
    /// (no `target_device_id`).
    pub async fn handle_command(
        &self,
        user_id: usize,
        from_device_id: usize,
        target_device_id: usize,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] handle_command: user={} from={} target={} command={:?}",
            user_id, from_device_id, target_device_id, command
        );

        let owner_user_id = self
            .get_device_owner_id(target_device_id)
            .await
            .ok_or(PlaybackError::DeviceNotFound)?;

        if owner_user_id != user_id
            && !self.is_user_allowed_for_device(owner_user_id, target_device_id, user_id)
        {
            return Err(PlaybackError::Forbidden);
        }

        // Build the forwarded command message (without target_device_id)
        let forwarded = ServerMessage::new(
            msg_types::PLAYBACK_COMMAND,
            PlaybackCommandPayload {
                command: command.to_string(),
                payload,
                target_device_id: None,
            },
        );

        // Send to target device - returns NotConnected if not found
        self.connection_manager
            .send_to_device(owner_user_id, target_device_id, forwarded)
            .await
            .map_err(|_| PlaybackError::DeviceNotFound)
    }

    /// Handle queue sync request - returns combined queue from all devices.
    pub async fn handle_request_queue(
        &self,
        user_id: usize,
        device_id: usize,
        target_device_id: Option<usize>,
    ) -> Result<QueueSyncPayload, PlaybackError> {
        let sessions = self.sessions.read().await;

        let (owner_user_id, target_id) = if let Some(target_id) = target_device_id {
            let owner_user_id = self
                .get_device_owner_id(target_id)
                .await
                .ok_or(PlaybackError::DeviceNotFound)?;
            (owner_user_id, target_id)
        } else {
            (user_id, device_id)
        };

        if owner_user_id != user_id
            && !self.is_user_allowed_for_device(owner_user_id, target_id, user_id)
        {
            return Err(PlaybackError::Forbidden);
        }

        // Try to find the requested device's queue if specified
        if let Some(session) = sessions.get(&owner_user_id) {
            if let Some(ds) = session.device_states.get(&target_id) {
                return Ok(QueueSyncPayload {
                    device_id: target_id,
                    queue: ds.queue.clone(),
                    queue_version: ds.queue_version,
                });
            }

            if target_device_id.is_none() {
                // Prefer the requesting device's own queue, otherwise pick any
                if let Some(ds) = session.device_states.get(&device_id) {
                    return Ok(QueueSyncPayload {
                        device_id,
                        queue: ds.queue.clone(),
                        queue_version: ds.queue_version,
                    });
                }
                // Return first available device's queue
                if let Some((found_id, ds)) = session.device_states.iter().next() {
                    return Ok(QueueSyncPayload {
                        device_id: *found_id,
                        queue: ds.queue.clone(),
                        queue_version: ds.queue_version,
                    });
                }
            }
        }

        Ok(QueueSyncPayload {
            device_id,
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

        // Broadcast device_stopped if the device had active state
        if had_state {
            let stopped_msg = ServerMessage::new(
                msg_types::PLAYBACK_DEVICE_STOPPED,
                DeviceStoppedPayload {
                    device_id,
                    reason: "disconnected".to_string(),
                },
            );
            self
                .broadcast_to_authorized_users(user_id, device_id, device_id, stopped_msg)
                .await;
        }

        // Broadcast device list changed
        self
            .broadcast_device_list_change(user_id, device_id, "disconnected", device_id)
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
                self
                    .broadcast_to_authorized_users(*user_id, device_id, device_id, stopped_msg)
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

    /// Return a snapshot of all active playback sessions for admin visibility.
    pub async fn get_active_sessions(&self) -> Vec<ActiveSessionSnapshot> {
        let sessions = self.sessions.read().await;
        let devices = self.devices.read().await;

        sessions
            .iter()
            .filter(|(_, session)| !session.device_states.is_empty())
            .map(|(user_id, session)| {
                let device_snapshots: Vec<DeviceSnapshot> = session
                    .device_states
                    .iter()
                    .map(|(device_id, ds)| {
                        let meta = devices.get(&(*user_id, *device_id));
                        DeviceSnapshot {
                            device_id: *device_id,
                            device_name: meta
                                .map(|m| m.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string()),
                            device_type: meta.map(|m| m.device_type),
                            is_playing: ds.state.is_playing,
                            track_title: ds
                                .state
                                .current_track
                                .as_ref()
                                .map(|t| t.title.clone()),
                            artist_name: ds
                                .state
                                .current_track
                                .as_ref()
                                .map(|t| t.artist_name.clone()),
                            position: ds.state.position,
                            queue_length: ds.queue.len(),
                            last_update_secs_ago: ds.last_update.elapsed().as_secs(),
                        }
                    })
                    .collect();

                ActiveSessionSnapshot {
                    user_id: *user_id,
                    devices: device_snapshots,
                }
            })
            .collect()
    }
}

/// Admin-facing snapshot of one user's active playback session.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveSessionSnapshot {
    pub user_id: usize,
    pub devices: Vec<DeviceSnapshot>,
}

/// Admin-facing snapshot of one device's playback state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceSnapshot {
    pub device_id: usize,
    pub device_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<DeviceType>,
    pub is_playing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_name: Option<String>,
    pub position: f64,
    pub queue_length: usize,
    pub last_update_secs_ago: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::NullCatalogStore;
    use crate::user::SqliteUserStore;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    async fn setup() -> (Arc<ConnectionManager>, PlaybackSessionManager) {
        let conn_manager = Arc::new(ConnectionManager::new());
        let temp_dir = tempdir().unwrap();
        let store = SqliteUserStore::new(temp_dir.path().join("user.db")).unwrap();
        let user_store: Arc<dyn crate::user::FullUserStore> = Arc::new(store);
        let catalog_store: Arc<dyn crate::catalog_store::CatalogStore> =
            Arc::new(NullCatalogStore);
        let user_manager = Arc::new(Mutex::new(UserManager::new(
            catalog_store,
            user_store,
        )));
        let session_manager = PlaybackSessionManager::new(conn_manager.clone(), user_manager);
        (conn_manager, session_manager)
    }

    async fn setup_with_user_manager(
    ) -> (
        Arc<ConnectionManager>,
        PlaybackSessionManager,
        Arc<Mutex<UserManager>>,
    ) {
        let conn_manager = Arc::new(ConnectionManager::new());
        let temp_dir = tempdir().unwrap();
        let store = SqliteUserStore::new(temp_dir.path().join("user.db")).unwrap();
        let user_store: Arc<dyn crate::user::FullUserStore> = Arc::new(store);
        let catalog_store: Arc<dyn crate::catalog_store::CatalogStore> =
            Arc::new(NullCatalogStore);
        let user_manager = Arc::new(Mutex::new(UserManager::new(
            catalog_store,
            user_store,
        )));
        let session_manager = PlaybackSessionManager::new(conn_manager.clone(), user_manager.clone());
        (conn_manager, session_manager, user_manager)
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

    #[tokio::test]
    async fn policy_allow_everyone_allows_any_user() {
        let (_conn, manager, user_manager) = setup_with_user_manager().await;

        let owner_id = user_manager.lock().unwrap().add_user("owner").unwrap();
        let other_id = user_manager.lock().unwrap().add_user("other").unwrap();

        let device_id = user_manager
            .lock()
            .unwrap()
            .register_or_update_device(&crate::user::device::DeviceRegistration {
                device_uuid: "device-uuid-allow-all".to_string(),
                device_type: crate::user::device::DeviceType::Web,
                device_name: Some("Owner Device".to_string()),
                os_info: None,
            })
            .unwrap();
        user_manager
            .lock()
            .unwrap()
            .associate_device_with_user(device_id, owner_id)
            .unwrap();

        user_manager
            .lock()
            .unwrap()
            .set_device_share_policy(device_id, &DeviceSharePolicy::allow_everyone())
            .unwrap();

        assert!(manager
            .is_user_allowed_for_device(owner_id, device_id, other_id));
    }

    #[tokio::test]
    async fn policy_deny_everyone_denies_non_owner() {
        let (_conn, manager, user_manager) = setup_with_user_manager().await;

        let owner_id = user_manager.lock().unwrap().add_user("owner").unwrap();
        let other_id = user_manager.lock().unwrap().add_user("other").unwrap();

        let device_id = user_manager
            .lock()
            .unwrap()
            .register_or_update_device(&crate::user::device::DeviceRegistration {
                device_uuid: "device-uuid-deny-all".to_string(),
                device_type: crate::user::device::DeviceType::Web,
                device_name: Some("Owner Device".to_string()),
                os_info: None,
            })
            .unwrap();
        user_manager
            .lock()
            .unwrap()
            .associate_device_with_user(device_id, owner_id)
            .unwrap();

        user_manager
            .lock()
            .unwrap()
            .set_device_share_policy(device_id, &DeviceSharePolicy::deny_everyone())
            .unwrap();

        assert!(!manager
            .is_user_allowed_for_device(owner_id, device_id, other_id));
        assert!(manager
            .is_user_allowed_for_device(owner_id, device_id, owner_id));
    }

    #[tokio::test]
    async fn policy_disallow_overrides_allow_user() {
        let (_conn, manager, user_manager) = setup_with_user_manager().await;

        let owner_id = user_manager.lock().unwrap().add_user("owner").unwrap();
        let other_id = user_manager.lock().unwrap().add_user("other").unwrap();

        let device_id = user_manager
            .lock()
            .unwrap()
            .register_or_update_device(&crate::user::device::DeviceRegistration {
                device_uuid: "device-uuid-disallow".to_string(),
                device_type: crate::user::device::DeviceType::Web,
                device_name: Some("Owner Device".to_string()),
                os_info: None,
            })
            .unwrap();
        user_manager
            .lock()
            .unwrap()
            .associate_device_with_user(device_id, owner_id)
            .unwrap();

        let policy = DeviceSharePolicy {
            mode: DeviceShareMode::Custom,
            allow_users: vec![other_id],
            allow_roles: vec![],
            deny_users: vec![other_id],
        };

        user_manager
            .lock()
            .unwrap()
            .set_device_share_policy(device_id, &policy)
            .unwrap();

        assert!(!manager
            .is_user_allowed_for_device(owner_id, device_id, other_id));
    }

    #[tokio::test]
    async fn policy_allows_by_role_when_not_denied() {
        let (_conn, manager, user_manager) = setup_with_user_manager().await;

        let owner_id = user_manager.lock().unwrap().add_user("owner").unwrap();
        let other_id = user_manager.lock().unwrap().add_user("other").unwrap();
        user_manager
            .lock()
            .unwrap()
            .add_user_role(other_id, UserRole::Admin)
            .unwrap();

        let device_id = user_manager
            .lock()
            .unwrap()
            .register_or_update_device(&crate::user::device::DeviceRegistration {
                device_uuid: "device-uuid-role".to_string(),
                device_type: crate::user::device::DeviceType::Web,
                device_name: Some("Owner Device".to_string()),
                os_info: None,
            })
            .unwrap();
        user_manager
            .lock()
            .unwrap()
            .associate_device_with_user(device_id, owner_id)
            .unwrap();

        let policy = DeviceSharePolicy {
            mode: DeviceShareMode::Custom,
            allow_users: vec![],
            allow_roles: vec![UserRole::Admin],
            deny_users: vec![],
        };

        user_manager
            .lock()
            .unwrap()
            .set_device_share_policy(device_id, &policy)
            .unwrap();

        assert!(manager
            .is_user_allowed_for_device(owner_id, device_id, other_id));
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

    /// Drain all pending messages from a receiver, returning the last one received.
    async fn drain_messages(rx: &mut mpsc::Receiver<ServerMessage>) {
        while rx.try_recv().is_ok() {}
    }

    #[tokio::test]
    async fn command_forwards_to_target_device() {
        let (conn_manager, manager) = setup().await;

        // Register two devices
        let _rx_sender = conn_manager.register(1, 100, "web".to_string()).await;
        let mut rx_target = conn_manager.register(1, 200, "android".to_string()).await;

        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        let _ = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        // Drain hello broadcast messages
        drain_messages(&mut rx_target).await;

        // Send command from device 100 targeting device 200
        let result = manager
            .handle_command(1, 100, 200, "play", serde_json::Value::Null)
            .await;
        assert!(result.is_ok());

        // Verify target device received the command
        let msg = rx_target.recv().await.unwrap();
        assert_eq!(msg.msg_type, "playback.command");
        let payload: PlaybackCommandPayload =
            serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.command, "play");
        assert!(payload.target_device_id.is_none());
    }

    #[tokio::test]
    async fn command_returns_device_not_found_for_unknown_target() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Target device 999 doesn't exist
        let result = manager
            .handle_command(1, 100, 999, "play", serde_json::Value::Null)
            .await;
        assert!(matches!(result, Err(PlaybackError::DeviceNotFound)));
    }

    #[tokio::test]
    async fn command_forwards_payload_to_target() {
        let (conn_manager, manager) = setup().await;

        let _rx_sender = conn_manager.register(1, 100, "web".to_string()).await;
        let mut rx_target = conn_manager.register(1, 200, "android".to_string()).await;

        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        let _ = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        // Drain hello broadcast messages
        drain_messages(&mut rx_target).await;

        let seek_payload = serde_json::json!({"position": 45.5});
        let result = manager
            .handle_command(1, 100, 200, "seek", seek_payload)
            .await;
        assert!(result.is_ok());

        let msg = rx_target.recv().await.unwrap();
        let payload: PlaybackCommandPayload =
            serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.command, "seek");
        assert_eq!(payload.payload["position"], 45.5);
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
