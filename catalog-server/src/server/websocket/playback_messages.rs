//! Playback-related WebSocket message payloads.
//!
//! These types define the payloads for server-side playback state synchronization
//! between multiple connected devices for the same user.

use serde::{Deserialize, Serialize};

/// Track information for playback state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackTrack {
    pub id: String,
    pub title: String,
    pub artist_id: String,
    pub artist_name: String,
    pub album_id: String,
    pub album_title: String,
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
}

/// Current playback state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackState {
    pub current_track: Option<PlaybackTrack>,
    pub queue_position: usize,
    pub queue_version: u64,
    pub position: f64,
    pub is_playing: bool,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub timestamp: u64,
}

/// Repeat mode for playback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// Queue item with track ID and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueueItem {
    pub id: String,
    pub added_at: u64,
}

/// Device type identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Web,
    Android,
    Ios,
}

/// Information about a connected device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectedDevice {
    pub id: usize,
    pub name: String,
    pub device_type: DeviceType,
    pub is_audio_device: bool,
    pub connected_at: u64,
}

// ============================================================================
// Client -> Server payloads
// ============================================================================

/// Payload for playback.hello message.
#[derive(Debug, Clone, Deserialize)]
pub struct HelloPayload {
    pub device_name: String,
    pub device_type: DeviceType,
}

/// Payload for playback.command message.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackCommandPayload {
    pub command: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Payload for playback.transfer_ready message.
#[derive(Debug, Clone, Deserialize)]
pub struct TransferReadyPayload {
    pub transfer_id: String,
    pub state: PlaybackState,
    pub queue: Vec<QueueItem>,
}

/// Payload for playback.queue_update message.
#[derive(Debug, Clone, Deserialize)]
pub struct QueueUpdatePayload {
    pub queue: Vec<QueueItem>,
    pub queue_version: u64,
}

/// Payload for playback.transfer_complete message.
#[derive(Debug, Clone, Deserialize)]
pub struct TransferCompletePayload {
    pub transfer_id: String,
}

// ============================================================================
// Server -> Client payloads
// ============================================================================

/// Payload for playback.welcome message.
#[derive(Debug, Clone, Serialize)]
pub struct WelcomePayload {
    pub device_id: usize,
    pub session: SessionInfo,
    pub devices: Vec<ConnectedDevice>,
}

/// Session information included in welcome message.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<PlaybackState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<Vec<QueueItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_device_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reclaimable: Option<bool>,
}

/// Payload for playback.device_list_changed message.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceListChangedPayload {
    pub devices: Vec<ConnectedDevice>,
    pub change: DeviceChange,
}

/// Description of a device change event.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceChange {
    /// Type of change: "connected", "disconnected", "became_audio_device", "stopped_audio_device"
    #[serde(rename = "type")]
    pub change_type: String,
    pub device_id: usize,
}

/// Payload for playback.prepare_transfer message.
#[derive(Debug, Clone, Serialize)]
pub struct PrepareTransferPayload {
    pub transfer_id: String,
    pub target_device_id: usize,
    pub target_device_name: String,
}

/// Payload for playback.become_audio_device message.
#[derive(Debug, Clone, Serialize)]
pub struct BecomeAudioDevicePayload {
    pub transfer_id: String,
    pub state: PlaybackState,
    pub queue: Vec<QueueItem>,
}

/// Payload for playback.transfer_aborted message.
#[derive(Debug, Clone, Serialize)]
pub struct TransferAbortedPayload {
    pub transfer_id: String,
    /// Reason for abort: "timeout", "source_disconnected", "target_disconnected"
    pub reason: String,
}

/// Payload for playback.queue_sync message.
#[derive(Debug, Clone, Serialize)]
pub struct QueueSyncPayload {
    pub queue: Vec<QueueItem>,
    pub queue_version: u64,
}

/// Payload for playback.session_ended message.
#[derive(Debug, Clone, Serialize)]
pub struct SessionEndedPayload {
    pub reason: String,
}

/// Payload for playback error messages.
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ErrorContext>,
}

/// Additional context for playback errors.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_state_serializes_correctly() {
        let state = PlaybackState {
            current_track: Some(PlaybackTrack {
                id: "track-1".to_string(),
                title: "Test Song".to_string(),
                artist_id: "artist-1".to_string(),
                artist_name: "Test Artist".to_string(),
                album_id: "album-1".to_string(),
                album_title: "Test Album".to_string(),
                duration: 180.5,
                track_number: Some(1),
                image_id: Some("img-1".to_string()),
            }),
            queue_position: 0,
            queue_version: 1,
            position: 30.5,
            is_playing: true,
            volume: 0.8,
            shuffle: false,
            repeat: RepeatMode::Off,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"title\":\"Test Song\""));
        assert!(json.contains("\"is_playing\":true"));
        assert!(json.contains("\"repeat\":\"off\""));
    }

    #[test]
    fn repeat_mode_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&RepeatMode::Off).unwrap(),
            "\"off\""
        );
        assert_eq!(
            serde_json::to_string(&RepeatMode::All).unwrap(),
            "\"all\""
        );
        assert_eq!(
            serde_json::to_string(&RepeatMode::One).unwrap(),
            "\"one\""
        );
    }

    #[test]
    fn device_type_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&DeviceType::Web).unwrap(),
            "\"web\""
        );
        assert_eq!(
            serde_json::to_string(&DeviceType::Android).unwrap(),
            "\"android\""
        );
        assert_eq!(
            serde_json::to_string(&DeviceType::Ios).unwrap(),
            "\"ios\""
        );
    }

    #[test]
    fn hello_payload_deserializes_correctly() {
        let json = r#"{"device_name":"Chrome on Windows","device_type":"web"}"#;
        let payload: HelloPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.device_name, "Chrome on Windows");
        assert_eq!(payload.device_type, DeviceType::Web);
    }

    #[test]
    fn welcome_payload_serializes_correctly() {
        let payload = WelcomePayload {
            device_id: 42,
            session: SessionInfo {
                exists: false,
                state: None,
                queue: None,
                audio_device_id: None,
                reclaimable: None,
            },
            devices: vec![ConnectedDevice {
                id: 42,
                name: "Chrome on Windows".to_string(),
                device_type: DeviceType::Web,
                is_audio_device: false,
                connected_at: 1234567890,
            }],
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"device_id\":42"));
        assert!(json.contains("\"exists\":false"));
        // Optional fields should not appear when None
        assert!(!json.contains("\"state\""));
        assert!(!json.contains("\"reclaimable\""));
    }

    #[test]
    fn command_payload_deserializes_with_default_payload() {
        let json = r#"{"command":"play"}"#;
        let payload: PlaybackCommandPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.command, "play");
        assert!(payload.payload.is_null());
    }

    #[test]
    fn command_payload_deserializes_with_payload() {
        let json = r#"{"command":"seek","payload":{"position":45.5}}"#;
        let payload: PlaybackCommandPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.command, "seek");
        assert_eq!(payload.payload["position"], 45.5);
    }
}
