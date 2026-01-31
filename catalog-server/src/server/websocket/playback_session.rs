//! Playback session manager for server-side playback state.
//!
//! Manages per-user playback sessions, including tracking which device is the
//! audio device, the current playback state, and the queue.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::connection::ConnectionManager;
use super::messages::{msg_types, ServerMessage};
use super::playback_messages::*;

/// Transfer timeout - how long to wait for transfer handshake completion.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum queue size to prevent memory exhaustion.
const MAX_QUEUE_SIZE: usize = 500;

/// Duration for which a disconnected audio device can reclaim its session.
/// After this period, the orphaned session is cleaned up.
const ORPHAN_GRACE_PERIOD: Duration = Duration::from_secs(30);

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

/// Per-user playback session.
#[derive(Debug)]
struct UserPlaybackSession {
    audio_device_id: Option<usize>,
    state: Option<PlaybackState>,
    queue: Vec<QueueItem>,
    queue_version: u64,
    last_state_update: Instant,
    pending_transfer: Option<PendingTransfer>,
    /// Tracks recently disconnected audio devices for reclaim.
    recent_audio_device: Option<RecentAudioDevice>,
}

impl Default for UserPlaybackSession {
    fn default() -> Self {
        Self {
            audio_device_id: None,
            state: None,
            queue: Vec::new(),
            queue_version: 0,
            last_state_update: Instant::now(),
            pending_transfer: None,
            recent_audio_device: None,
        }
    }
}

#[derive(Debug)]
struct PendingTransfer {
    transfer_id: String,
    source_device_id: usize,
    target_device_id: usize,
    #[allow(dead_code)] // Will be used for timeout detection
    started_at: Instant,
}

#[derive(Debug)]
struct RecentAudioDevice {
    device_id: usize,
    #[allow(dead_code)] // Reserved for future use in reclaim flow
    device_name: String,
    #[allow(dead_code)] // Reserved for future use in reclaim flow
    device_type: DeviceType,
    disconnected_at: Instant,
    #[allow(dead_code)] // Reserved for sending state back to reclaiming device
    last_state: PlaybackState,
    #[allow(dead_code)] // Reserved for sending queue back to reclaiming device
    queue: Vec<QueueItem>,
}

/// Errors that can occur during playback operations.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackError {
    /// No active session for user.
    NoSession,
    /// Device is not the current audio device.
    NotAudioDevice,
    /// A transfer is already in progress.
    TransferInProgress,
    /// Queue size limit exceeded.
    QueueLimitExceeded,
    /// Invalid message format or type.
    InvalidMessage(String),
    /// Command execution failed.
    CommandFailed(String),
    /// Transfer not found or invalid.
    InvalidTransfer(String),
    /// Device not found.
    DeviceNotFound,
}

impl std::fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackError::NoSession => write!(f, "No active playback session"),
            PlaybackError::NotAudioDevice => write!(f, "Device is not the audio device"),
            PlaybackError::TransferInProgress => write!(f, "A transfer is already in progress"),
            PlaybackError::QueueLimitExceeded => {
                write!(f, "Queue size limit exceeded (max {})", MAX_QUEUE_SIZE)
            }
            PlaybackError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            PlaybackError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            PlaybackError::InvalidTransfer(msg) => write!(f, "Invalid transfer: {}", msg),
            PlaybackError::DeviceNotFound => write!(f, "Device not found"),
        }
    }
}

impl std::error::Error for PlaybackError {}

impl From<PlaybackError> for PlaybackErrorPayload {
    fn from(e: PlaybackError) -> Self {
        let code = match &e {
            PlaybackError::NoSession => "no_session",
            PlaybackError::NotAudioDevice => "not_audio_device",
            PlaybackError::TransferInProgress => "transfer_in_progress",
            PlaybackError::QueueLimitExceeded => "queue_limit_exceeded",
            PlaybackError::InvalidMessage(_) => "invalid_message",
            PlaybackError::CommandFailed(_) => "command_failed",
            PlaybackError::InvalidTransfer(_) => "invalid_transfer",
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
            let device_count = devices.len();
            devices.insert(
                (user_id, device_id),
                DeviceMetadata {
                    name: device_name.clone(),
                    device_type,
                    connected_at: now,
                },
            );
            info!(
                "[playback] stored device metadata, total devices: {}",
                device_count + 1
            );
        }

        // Get or create session
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(user_id).or_default();

        info!(
            "[playback] session state: audio_device_id={:?}, has_state={}, queue_len={}, has_recent={}, pending_transfer={:?}",
            session.audio_device_id,
            session.state.is_some(),
            session.queue.len(),
            session.recent_audio_device.is_some(),
            session.pending_transfer.as_ref().map(|t| &t.transfer_id)
        );

        // Check if this device can reclaim audio device status
        let reclaimable = session
            .recent_audio_device
            .as_ref()
            .map(|r| r.device_id == device_id && r.disconnected_at.elapsed() < ORPHAN_GRACE_PERIOD)
            .unwrap_or(false);

        if reclaimable {
            info!("[playback] device {} can reclaim audio device status", device_id);
        }

        // Build session info
        let session_info = SessionInfo {
            exists: session.audio_device_id.is_some() || reclaimable,
            state: session.state.clone(),
            queue: if session.state.is_some() || reclaimable {
                Some(session.queue.clone())
            } else {
                None
            },
            audio_device_id: session.audio_device_id,
            reclaimable: if reclaimable { Some(true) } else { None },
            queue_version: session.queue_version,
        };

        info!(
            "[playback] session_info: exists={}, has_state={}, audio_device_id={:?}, reclaimable={:?}",
            session_info.exists,
            session_info.state.is_some(),
            session_info.audio_device_id,
            session_info.reclaimable
        );

        // Get list of connected devices
        let connected_devices = self.build_device_list(user_id, session).await;
        info!(
            "[playback] connected_devices count: {}, ids: {:?}",
            connected_devices.len(),
            connected_devices.iter().map(|d| d.id).collect::<Vec<_>>()
        );

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
            "[playback] handle_hello complete: returning welcome with {} devices, session_exists={}",
            connected_devices.len(),
            session_info.exists
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
        session: &UserPlaybackSession,
    ) -> Vec<ConnectedDevice> {
        let device_ids = self.connection_manager.get_connected_devices(user_id).await;
        let devices = self.devices.read().await;

        debug!(
            "[playback] build_device_list: user={} connection_manager_devices={:?} metadata_count={}",
            user_id, device_ids, devices.len()
        );

        let result: Vec<ConnectedDevice> = device_ids
            .into_iter()
            .filter_map(|id| {
                let meta = devices.get(&(user_id, id));
                if meta.is_none() {
                    debug!(
                        "[playback] build_device_list: no metadata for device {} (user={})",
                        id, user_id
                    );
                }
                meta.map(|meta| ConnectedDevice {
                    id,
                    name: meta.name.clone(),
                    device_type: meta.device_type,
                    is_audio_device: session.audio_device_id == Some(id),
                    connected_at: meta.connected_at,
                })
            })
            .collect();

        debug!(
            "[playback] build_device_list result: {} devices, audio_device_id={:?}",
            result.len(),
            session.audio_device_id
        );

        result
    }

    /// Register a device as the audio device.
    pub async fn register_audio_device(
        &self,
        user_id: usize,
        device_id: usize,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] register_audio_device: user={} device={}",
            user_id, device_id
        );

        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(user_id).or_default();

        // Check if there's already an active audio device
        if let Some(existing) = session.audio_device_id {
            warn!(
                "[playback] register_audio_device FAILED: device {} already registered as audio device",
                existing
            );
            return Err(PlaybackError::CommandFailed(
                "Another device is already the audio device".to_string(),
            ));
        }

        session.audio_device_id = Some(device_id);
        session.last_state_update = Instant::now();

        // Clear any recent audio device info since we have a new audio device
        session.recent_audio_device = None;

        // Get device list for broadcast
        let connected_devices = self.build_device_list(user_id, session).await;

        info!(
            "[playback] broadcasting device_list_changed (became_audio_device) to {} devices",
            connected_devices.len()
        );

        // Broadcast device list changed
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices,
                change: DeviceChange {
                    change_type: "became_audio_device".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, change_msg)
            .await;

        info!(
            "[playback] register_audio_device SUCCESS: device {} is now the audio device",
            device_id
        );
        Ok(())
    }

    /// Unregister audio device (voluntary stop).
    pub async fn unregister_audio_device(
        &self,
        user_id: usize,
        device_id: usize,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] unregister_audio_device: user={} device={}",
            user_id, device_id
        );

        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&user_id).ok_or_else(|| {
            warn!("[playback] unregister_audio_device FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        if session.audio_device_id != Some(device_id) {
            warn!(
                "[playback] unregister_audio_device FAILED: device {} is not audio device (current: {:?})",
                device_id, session.audio_device_id
            );
            return Err(PlaybackError::NotAudioDevice);
        }

        // Clear session
        session.audio_device_id = None;
        session.state = None;
        session.queue.clear();
        session.queue_version = 0;

        // Get device list for broadcast
        let connected_devices = self.build_device_list(user_id, session).await;

        info!("[playback] broadcasting session_ended to other devices");

        // Broadcast session ended to other devices
        let ended_msg = ServerMessage::new(
            msg_types::PLAYBACK_SESSION_ENDED,
            SessionEndedPayload {
                reason: "stopped".to_string(),
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, ended_msg)
            .await;

        // Broadcast device list changed
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices,
                change: DeviceChange {
                    change_type: "stopped_audio_device".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, change_msg)
            .await;

        info!("[playback] unregister_audio_device SUCCESS: device {} unregistered", device_id);
        Ok(())
    }

    /// Handle state broadcast from audio device.
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

        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&user_id).ok_or_else(|| {
            warn!("[playback] handle_state_update FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        if session.audio_device_id != Some(device_id) {
            warn!(
                "[playback] handle_state_update FAILED: device {} is not audio device (current: {:?})",
                device_id, session.audio_device_id
            );
            return Err(PlaybackError::NotAudioDevice);
        }

        // Update session state and timestamp
        session.state = Some(state.clone());
        session.last_state_update = Instant::now();

        // Relay state to other devices
        let state_msg = ServerMessage::new(msg_types::PLAYBACK_STATE, state);
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, state_msg)
            .await;

        Ok(())
    }

    /// Handle queue update from audio device.
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
        let session = sessions.get_mut(&user_id).ok_or(PlaybackError::NoSession)?;

        if session.audio_device_id != Some(device_id) {
            return Err(PlaybackError::NotAudioDevice);
        }

        // Update session queue
        session.queue = queue.clone();
        session.queue_version = queue_version;
        session.last_state_update = Instant::now();

        // Broadcast to other devices
        let queue_msg = ServerMessage::new(
            msg_types::PLAYBACK_QUEUE_UPDATE,
            QueueSyncPayload {
                queue,
                queue_version,
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, queue_msg)
            .await;

        Ok(())
    }

    /// Handle command from remote device.
    pub async fn handle_command(
        &self,
        user_id: usize,
        from_device_id: usize,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] handle_command: user={} from_device={} command={:?}",
            user_id, from_device_id, command
        );

        let sessions = self.sessions.read().await;
        let session = sessions.get(&user_id).ok_or_else(|| {
            warn!("[playback] handle_command FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        let audio_device_id = session.audio_device_id.ok_or_else(|| {
            warn!("[playback] handle_command FAILED: no audio device for user {}", user_id);
            PlaybackError::NoSession
        })?;

        info!(
            "[playback] handle_command: audio_device_id={}, from_device={}",
            audio_device_id, from_device_id
        );

        // Handle special command to become audio device (initiates transfer)
        if command == "becomeAudioDevice" {
            let transfer_id = payload
                .get("transfer_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PlaybackError::InvalidMessage("missing transfer_id".to_string()))?
                .to_string();

            info!(
                "[playback] handle_command: initiating transfer, transfer_id={}",
                transfer_id
            );

            drop(sessions);
            return self
                .initiate_transfer(user_id, from_device_id, transfer_id)
                .await;
        }

        // Forward command to audio device
        info!(
            "[playback] forwarding command {:?} to audio device {}",
            command, audio_device_id
        );

        let cmd_msg = ServerMessage::new(
            msg_types::PLAYBACK_COMMAND,
            PlaybackCommandPayload {
                command: command.to_string(),
                payload,
            },
        );
        let _ = self
            .connection_manager
            .send_to_device(user_id, audio_device_id, cmd_msg)
            .await;

        Ok(())
    }

    /// Handle queue sync request from remote device.
    pub async fn handle_request_queue(
        &self,
        user_id: usize,
        _device_id: usize,
    ) -> Result<QueueSyncPayload, PlaybackError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&user_id).ok_or(PlaybackError::NoSession)?;

        Ok(QueueSyncPayload {
            queue: session.queue.clone(),
            queue_version: session.queue_version,
        })
    }

    /// Initiate playback transfer.
    pub async fn initiate_transfer(
        &self,
        user_id: usize,
        requesting_device_id: usize,
        transfer_id: String,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] initiate_transfer: user={} requesting_device={} transfer_id={}",
            user_id, requesting_device_id, transfer_id
        );

        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&user_id).ok_or_else(|| {
            warn!("[playback] initiate_transfer FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        // Check no transfer in progress
        if let Some(ref pending) = session.pending_transfer {
            warn!(
                "[playback] initiate_transfer FAILED: transfer {} already in progress",
                pending.transfer_id
            );
            return Err(PlaybackError::TransferInProgress);
        }

        let source_device_id = session.audio_device_id.ok_or_else(|| {
            warn!("[playback] initiate_transfer FAILED: no audio device for user {}", user_id);
            PlaybackError::NoSession
        })?;

        // Get target device name
        let devices = self.devices.read().await;
        let target_meta = devices
            .get(&(user_id, requesting_device_id))
            .ok_or_else(|| {
                warn!(
                    "[playback] initiate_transfer FAILED: target device {} not found",
                    requesting_device_id
                );
                PlaybackError::DeviceNotFound
            })?;
        let target_device_name = target_meta.name.clone();
        drop(devices);

        info!(
            "[playback] transfer: source={} -> target={} ({:?})",
            source_device_id, requesting_device_id, target_device_name
        );

        // Create pending transfer
        session.pending_transfer = Some(PendingTransfer {
            transfer_id: transfer_id.clone(),
            source_device_id,
            target_device_id: requesting_device_id,
            started_at: Instant::now(),
        });

        // Send prepare_transfer to current audio device
        let prepare_msg = ServerMessage::new(
            msg_types::PLAYBACK_PREPARE_TRANSFER,
            PrepareTransferPayload {
                transfer_id: transfer_id.clone(),
                target_device_id: requesting_device_id,
                target_device_name,
            },
        );
        let _ = self
            .connection_manager
            .send_to_device(user_id, source_device_id, prepare_msg)
            .await;

        info!(
            "[playback] initiate_transfer: sent prepare_transfer to source device {}",
            source_device_id
        );

        // Spawn timeout task
        let manager = self.clone_for_timeout();
        let tid = transfer_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(TRANSFER_TIMEOUT).await;
            manager.handle_transfer_timeout(user_id, &tid).await;
        });

        Ok(())
    }

    /// Clone self for use in timeout task (shares sessions and connection_manager).
    fn clone_for_timeout(&self) -> PlaybackSessionTimeoutHandler {
        PlaybackSessionTimeoutHandler {
            sessions: self.sessions.clone(),
            connection_manager: self.connection_manager.clone(),
        }
    }

    /// Handle transfer_ready from current audio device.
    pub async fn handle_transfer_ready(
        &self,
        user_id: usize,
        device_id: usize,
        transfer_id: String,
        state: PlaybackState,
        queue: Vec<QueueItem>,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] handle_transfer_ready: user={} device={} transfer_id={} queue_len={}",
            user_id, device_id, transfer_id, queue.len()
        );

        let sessions = self.sessions.read().await;
        let session = sessions.get(&user_id).ok_or_else(|| {
            warn!("[playback] handle_transfer_ready FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        // Verify device is current audio device
        if session.audio_device_id != Some(device_id) {
            warn!(
                "[playback] handle_transfer_ready FAILED: device {} is not audio device (current: {:?})",
                device_id, session.audio_device_id
            );
            return Err(PlaybackError::NotAudioDevice);
        }

        // Verify transfer matches pending
        let pending = session
            .pending_transfer
            .as_ref()
            .ok_or_else(|| {
                warn!("[playback] handle_transfer_ready FAILED: no pending transfer");
                PlaybackError::InvalidTransfer("no pending transfer".to_string())
            })?;

        if pending.transfer_id != transfer_id {
            warn!(
                "[playback] handle_transfer_ready FAILED: transfer_id mismatch (expected={}, got={})",
                pending.transfer_id, transfer_id
            );
            return Err(PlaybackError::InvalidTransfer(
                "transfer_id mismatch".to_string(),
            ));
        }

        let target_device_id = pending.target_device_id;

        info!(
            "[playback] sending become_audio_device to target device {}",
            target_device_id
        );

        // Send become_audio_device to target
        let become_msg = ServerMessage::new(
            msg_types::PLAYBACK_BECOME_AUDIO_DEVICE,
            BecomeAudioDevicePayload {
                transfer_id,
                state,
                queue,
            },
        );
        let _ = self
            .connection_manager
            .send_to_device(user_id, target_device_id, become_msg)
            .await;

        info!(
            "[playback] handle_transfer_ready: sent become_audio_device to device {}",
            target_device_id
        );
        Ok(())
    }

    /// Handle transfer_complete from new audio device.
    pub async fn handle_transfer_complete(
        &self,
        user_id: usize,
        device_id: usize,
        transfer_id: String,
    ) -> Result<(), PlaybackError> {
        info!(
            "[playback] handle_transfer_complete: user={} device={} transfer_id={}",
            user_id, device_id, transfer_id
        );

        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&user_id).ok_or_else(|| {
            warn!("[playback] handle_transfer_complete FAILED: no session for user {}", user_id);
            PlaybackError::NoSession
        })?;

        // Verify transfer matches pending
        let pending = session
            .pending_transfer
            .take()
            .ok_or_else(|| {
                warn!("[playback] handle_transfer_complete FAILED: no pending transfer");
                PlaybackError::InvalidTransfer("no pending transfer".to_string())
            })?;

        if pending.transfer_id != transfer_id {
            warn!(
                "[playback] handle_transfer_complete FAILED: transfer_id mismatch (expected={}, got={})",
                pending.transfer_id, transfer_id
            );
            // Put it back
            session.pending_transfer = Some(pending);
            return Err(PlaybackError::InvalidTransfer(
                "transfer_id mismatch".to_string(),
            ));
        }

        if pending.target_device_id != device_id {
            warn!(
                "[playback] handle_transfer_complete FAILED: device_id mismatch (expected={}, got={})",
                pending.target_device_id, device_id
            );
            session.pending_transfer = Some(pending);
            return Err(PlaybackError::InvalidTransfer(
                "device_id mismatch".to_string(),
            ));
        }

        let old_audio_device = pending.source_device_id;

        // Update audio_device_id
        session.audio_device_id = Some(device_id);
        session.last_state_update = Instant::now();

        info!(
            "[playback] transfer complete: old_audio_device={} -> new_audio_device={}",
            old_audio_device, device_id
        );

        // Send transfer_complete to old device
        let complete_msg = ServerMessage::new(
            msg_types::PLAYBACK_TRANSFER_COMPLETE,
            serde_json::json!({ "transfer_id": transfer_id }),
        );
        let _ = self
            .connection_manager
            .send_to_device(user_id, old_audio_device, complete_msg)
            .await;

        // Get device list for broadcast
        let connected_devices = self.build_device_list(user_id, session).await;

        // Broadcast device_list_changed
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices,
                change: DeviceChange {
                    change_type: "became_audio_device".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .broadcast_to_user(user_id, change_msg)
            .await;

        info!(
            "[playback] handle_transfer_complete SUCCESS: device {} is now the audio device",
            device_id
        );
        Ok(())
    }

    /// Handle reclaim request after reconnection.
    pub async fn handle_reclaim(
        &self,
        user_id: usize,
        device_id: usize,
        state: PlaybackState,
    ) -> Result<(), PlaybackError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&user_id).ok_or(PlaybackError::NoSession)?;

        // Check recent_audio_device matches
        let recent = session
            .recent_audio_device
            .as_ref()
            .ok_or(PlaybackError::CommandFailed("no recent session to reclaim".to_string()))?;

        if recent.device_id != device_id {
            return Err(PlaybackError::CommandFailed(
                "device_id mismatch for reclaim".to_string(),
            ));
        }

        if recent.disconnected_at.elapsed() >= ORPHAN_GRACE_PERIOD {
            return Err(PlaybackError::CommandFailed(
                "reclaim window expired".to_string(),
            ));
        }

        // Restore audio device status
        session.audio_device_id = Some(device_id);
        session.state = Some(state.clone());
        session.last_state_update = Instant::now();

        // Clear recent_audio_device
        session.recent_audio_device = None;

        // Get device list for broadcast
        let connected_devices = self.build_device_list(user_id, session).await;

        // Broadcast device_list_changed
        let change_msg = ServerMessage::new(
            msg_types::PLAYBACK_DEVICE_LIST_CHANGED,
            DeviceListChangedPayload {
                devices: connected_devices,
                change: DeviceChange {
                    change_type: "became_audio_device".to_string(),
                    device_id,
                },
            },
        );
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, change_msg)
            .await;

        // Broadcast state to other devices
        let state_msg = ServerMessage::new(msg_types::PLAYBACK_STATE, state);
        let _ = self
            .connection_manager
            .send_to_other_devices(user_id, device_id, state_msg)
            .await;

        Ok(())
    }

    /// Called when a device disconnects.
    pub async fn handle_device_disconnect(
        &self,
        user_id: usize,
        device_id: usize,
        device_name: &str,
        device_type: DeviceType,
    ) {
        info!(
            "[playback] handle_device_disconnect: user={} device={} name={:?} type={:?}",
            user_id, device_id, device_name, device_type
        );

        // Remove device metadata
        {
            let mut devices = self.devices.write().await;
            devices.remove(&(user_id, device_id));
            info!("[playback] removed device metadata, remaining devices: {}", devices.len());
        }

        let mut sessions = self.sessions.write().await;
        let session = match sessions.get_mut(&user_id) {
            Some(s) => s,
            None => {
                info!("[playback] no session for user {}, nothing to clean up", user_id);
                return;
            }
        };

        info!(
            "[playback] session before disconnect: audio_device_id={:?}, pending_transfer={:?}",
            session.audio_device_id,
            session.pending_transfer.as_ref().map(|t| &t.transfer_id)
        );

        // Check if this affects a pending transfer
        let transfer_abort_info = session.pending_transfer.as_ref().and_then(|pending| {
            if pending.source_device_id == device_id || pending.target_device_id == device_id {
                let is_source = pending.source_device_id == device_id;
                Some((
                    pending.transfer_id.clone(),
                    if is_source {
                        pending.target_device_id
                    } else {
                        pending.source_device_id
                    },
                    if is_source {
                        "source_disconnected"
                    } else {
                        "target_disconnected"
                    },
                ))
            } else {
                None
            }
        });

        if let Some((transfer_id, other_device_id, reason)) = transfer_abort_info {
            warn!(
                "[playback] aborting transfer {} due to disconnect: {}",
                transfer_id, reason
            );
            session.pending_transfer = None;

            // Notify other device
            let abort_msg = ServerMessage::new(
                msg_types::PLAYBACK_TRANSFER_ABORTED,
                TransferAbortedPayload {
                    transfer_id,
                    reason: reason.to_string(),
                },
            );
            let _ = self
                .connection_manager
                .send_to_device(user_id, other_device_id, abort_msg)
                .await;
        }

        // Check if this was the audio device
        if session.audio_device_id == Some(device_id) {
            info!(
                "[playback] audio device {} disconnected, saving for reclaim",
                device_id
            );

            // Save for reclaim
            if let Some(state) = session.state.clone() {
                session.recent_audio_device = Some(RecentAudioDevice {
                    device_id,
                    device_name: device_name.to_string(),
                    device_type,
                    disconnected_at: Instant::now(),
                    last_state: state,
                    queue: session.queue.clone(),
                });
                info!("[playback] saved recent_audio_device for potential reclaim");
            }

            session.audio_device_id = None;
            // Don't clear state/queue yet - keep for potential reclaim
        }

        // Get device list for broadcast
        let connected_devices = self.build_device_list(user_id, session).await;
        info!(
            "[playback] broadcasting device_list_changed (disconnected), remaining devices: {}",
            connected_devices.len()
        );

        // Broadcast device list changed
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

        info!("[playback] handle_device_disconnect complete for device {}", device_id);
    }

    /// Check for orphaned sessions whose grace period has expired.
    pub async fn check_orphaned_sessions(&self) {
        let mut sessions = self.sessions.write().await;

        for (user_id, session) in sessions.iter_mut() {
            // Check if recent_audio_device expired past the orphan grace period
            if let Some(ref recent) = session.recent_audio_device {
                if recent.disconnected_at.elapsed() >= ORPHAN_GRACE_PERIOD {
                    info!(
                        "[playback] orphan grace period expired for user={} device={}",
                        user_id, recent.device_id
                    );

                    // Clear session data since reclaim is no longer possible
                    session.state = None;
                    session.queue.clear();
                    session.queue_version = 0;
                    session.recent_audio_device = None;

                    // Broadcast session ended
                    let ended_msg = ServerMessage::new(
                        msg_types::PLAYBACK_SESSION_ENDED,
                        SessionEndedPayload {
                            reason: "device_timeout".to_string(),
                        },
                    );
                    let _ = self
                        .connection_manager
                        .broadcast_to_user(*user_id, ended_msg)
                        .await;
                }
            }
        }
    }

    /// Get the current audio device ID for a user (if any).
    #[allow(dead_code)]
    pub async fn get_audio_device(&self, user_id: usize) -> Option<usize> {
        let sessions = self.sessions.read().await;
        sessions
            .get(&user_id)
            .and_then(|s| s.audio_device_id)
    }

    /// Check if a user has an active playback session.
    #[allow(dead_code)]
    pub async fn has_session(&self, user_id: usize) -> bool {
        let sessions = self.sessions.read().await;
        sessions
            .get(&user_id)
            .map(|s| s.audio_device_id.is_some())
            .unwrap_or(false)
    }

    /// Get the name of a connected device.
    pub async fn get_device_name(&self, user_id: usize, device_id: usize) -> Option<String> {
        let devices = self.devices.read().await;
        devices
            .get(&(user_id, device_id))
            .map(|meta| meta.name.clone())
    }
}

/// Helper struct for handling transfer timeouts without holding the main manager.
struct PlaybackSessionTimeoutHandler {
    sessions: Arc<RwLock<HashMap<usize, UserPlaybackSession>>>,
    connection_manager: Arc<ConnectionManager>,
}

impl PlaybackSessionTimeoutHandler {
    async fn handle_transfer_timeout(&self, user_id: usize, transfer_id: &str) {
        // Check if the transfer is still pending (race check)
        let should_abort = {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&user_id) {
                if let Some(pending) = &session.pending_transfer {
                    if pending.transfer_id == transfer_id {
                        session.pending_transfer = None;
                        true
                    } else {
                        false // Different transfer, already completed/replaced
                    }
                } else {
                    false // No pending transfer, already completed
                }
            } else {
                false
            }
        };

        if should_abort {
            let abort_msg = ServerMessage::new(
                msg_types::PLAYBACK_TRANSFER_ABORTED,
                TransferAbortedPayload {
                    transfer_id: transfer_id.to_string(),
                    reason: "timeout".to_string(),
                },
            );
            let _ = self
                .connection_manager
                .broadcast_to_user(user_id, abort_msg)
                .await;
        }
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

    #[tokio::test]
    async fn handle_hello_returns_welcome() {
        let (_, manager) = setup().await;

        let welcome = manager
            .handle_hello(1, 100, "Chrome on Windows".to_string(), DeviceType::Web)
            .await;

        assert_eq!(welcome.device_id, 100);
        assert!(!welcome.session.exists);
        assert!(welcome.session.state.is_none());
    }

    #[tokio::test]
    async fn register_audio_device_succeeds() {
        let (conn_manager, manager) = setup().await;

        // Register a device connection first
        let _rx = conn_manager.register(1, 100, "web".to_string()).await;

        // Send hello
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;

        // Register as audio device
        let result = manager.register_audio_device(1, 100).await;
        assert!(result.is_ok());

        // Verify
        assert_eq!(manager.get_audio_device(1).await, Some(100));
    }

    #[tokio::test]
    async fn register_audio_device_fails_if_already_exists() {
        let (conn_manager, manager) = setup().await;

        let _rx1 = conn_manager.register(1, 100, "web".to_string()).await;
        let _rx2 = conn_manager.register(1, 200, "android".to_string()).await;

        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        let _ = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        // First register succeeds
        manager.register_audio_device(1, 100).await.unwrap();

        // Second register fails
        let result = manager.register_audio_device(1, 200).await;
        assert!(matches!(result, Err(PlaybackError::CommandFailed(_))));
    }

    #[tokio::test]
    async fn unregister_audio_device_clears_session() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        manager.register_audio_device(1, 100).await.unwrap();

        // Unregister
        manager.unregister_audio_device(1, 100).await.unwrap();

        assert!(manager.get_audio_device(1).await.is_none());
    }

    #[tokio::test]
    async fn state_update_fails_if_not_audio_device() {
        let (conn_manager, manager) = setup().await;

        let _rx1 = conn_manager.register(1, 100, "web".to_string()).await;
        let _rx2 = conn_manager.register(1, 200, "android".to_string()).await;

        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        let _ = manager
            .handle_hello(1, 200, "Phone".to_string(), DeviceType::Android)
            .await;

        manager.register_audio_device(1, 100).await.unwrap();

        // Try to send state from non-audio device
        let state = PlaybackState {
            current_track: None,
            queue_position: 0,
            queue_version: 1,
            position: 0.0,
            is_playing: false,
            volume: 1.0,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            timestamp: 0,
        };

        let result = manager.handle_state_update(1, 200, state).await;
        assert!(matches!(result, Err(PlaybackError::NotAudioDevice)));
    }

    #[tokio::test]
    async fn queue_update_rejects_oversized_queue() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        manager.register_audio_device(1, 100).await.unwrap();

        // Try to send oversized queue
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
    async fn handle_device_disconnect_saves_for_reclaim() {
        let (conn_manager, manager) = setup().await;

        let _rx = conn_manager.register(1, 100, "web".to_string()).await;
        let _ = manager
            .handle_hello(1, 100, "Chrome".to_string(), DeviceType::Web)
            .await;
        manager.register_audio_device(1, 100).await.unwrap();

        // Send some state
        let state = PlaybackState {
            current_track: Some(PlaybackTrack {
                id: "track-1".to_string(),
                title: "Test".to_string(),
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
            is_playing: true,
            volume: 0.8,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            timestamp: 1000,
        };
        manager.handle_state_update(1, 100, state).await.unwrap();

        // Disconnect
        manager
            .handle_device_disconnect(1, 100, "Chrome", DeviceType::Web)
            .await;

        // Audio device should be cleared
        assert!(manager.get_audio_device(1).await.is_none());

        // But session info should be preserved for reclaim
        let sessions = manager.sessions.read().await;
        let session = sessions.get(&1).unwrap();
        assert!(session.recent_audio_device.is_some());
        assert!(session.state.is_some());
    }
}
