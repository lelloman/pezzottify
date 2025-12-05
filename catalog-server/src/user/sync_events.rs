//! Sync event types for multi-device synchronization.
//!
//! This module defines the event types used in the append-only event log
//! for synchronizing user data across devices.

use serde::{Deserialize, Serialize};

use crate::user::permissions::Permission;
use crate::user::settings::UserSetting;
use crate::user::user_models::LikedContentType;

/// All sync event types that can be recorded in the user event log.
///
/// Events are serialized using serde's adjacently tagged representation:
/// `{"type": "event_name", "payload": {...}}`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum UserEvent {
    // Likes
    #[serde(rename = "content_liked")]
    ContentLiked {
        content_type: LikedContentType,
        content_id: String,
    },

    #[serde(rename = "content_unliked")]
    ContentUnliked {
        content_type: LikedContentType,
        content_id: String,
    },

    // Settings
    #[serde(rename = "setting_changed")]
    SettingChanged { setting: UserSetting },

    // Playlists
    #[serde(rename = "playlist_created")]
    PlaylistCreated { playlist_id: String, name: String },

    #[serde(rename = "playlist_renamed")]
    PlaylistRenamed { playlist_id: String, name: String },

    #[serde(rename = "playlist_deleted")]
    PlaylistDeleted { playlist_id: String },

    #[serde(rename = "playlist_tracks_updated")]
    PlaylistTracksUpdated {
        playlist_id: String,
        track_ids: Vec<String>,
    },

    // Permissions (triggered by CLI admin actions)
    #[serde(rename = "permission_granted")]
    PermissionGranted { permission: Permission },

    #[serde(rename = "permission_revoked")]
    PermissionRevoked { permission: Permission },

    #[serde(rename = "permissions_reset")]
    PermissionsReset { permissions: Vec<Permission> },
}

impl UserEvent {
    /// Get the event type string for database storage.
    pub fn event_type(&self) -> &'static str {
        match self {
            UserEvent::ContentLiked { .. } => "content_liked",
            UserEvent::ContentUnliked { .. } => "content_unliked",
            UserEvent::SettingChanged { .. } => "setting_changed",
            UserEvent::PlaylistCreated { .. } => "playlist_created",
            UserEvent::PlaylistRenamed { .. } => "playlist_renamed",
            UserEvent::PlaylistDeleted { .. } => "playlist_deleted",
            UserEvent::PlaylistTracksUpdated { .. } => "playlist_tracks_updated",
            UserEvent::PermissionGranted { .. } => "permission_granted",
            UserEvent::PermissionRevoked { .. } => "permission_revoked",
            UserEvent::PermissionsReset { .. } => "permissions_reset",
        }
    }
}

/// An event stored in the database with its sequence number and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEvent {
    pub seq: i64,
    #[serde(flatten)]
    pub event: UserEvent,
    pub server_timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_liked_serialization() {
        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("content_liked"));
        assert!(json.contains("album"));
        assert!(json.contains("album_123"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_content_unliked_serialization() {
        let event = UserEvent::ContentUnliked {
            content_type: LikedContentType::Track,
            content_id: "track_456".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("content_unliked"));
        assert!(json.contains("track"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_setting_changed_serialization() {
        let event = UserEvent::SettingChanged {
            setting: UserSetting::DirectDownloadsEnabled(true),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("setting_changed"));
        assert!(json.contains("enable_direct_downloads"));
        assert!(json.contains("true"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_playlist_created_serialization() {
        let event = UserEvent::PlaylistCreated {
            playlist_id: "pl_abc123".to_string(),
            name: "My Playlist".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("playlist_created"));
        assert!(json.contains("pl_abc123"));
        assert!(json.contains("My Playlist"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_playlist_renamed_serialization() {
        let event = UserEvent::PlaylistRenamed {
            playlist_id: "pl_abc123".to_string(),
            name: "New Name".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("playlist_renamed"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_playlist_deleted_serialization() {
        let event = UserEvent::PlaylistDeleted {
            playlist_id: "pl_abc123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("playlist_deleted"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_playlist_tracks_updated_serialization() {
        let event = UserEvent::PlaylistTracksUpdated {
            playlist_id: "pl_abc123".to_string(),
            track_ids: vec!["t1".to_string(), "t2".to_string(), "t3".to_string()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("playlist_tracks_updated"));
        assert!(json.contains("t1"));
        assert!(json.contains("t2"));
        assert!(json.contains("t3"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_permission_granted_serialization() {
        let event = UserEvent::PermissionGranted {
            permission: Permission::EditCatalog,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("permission_granted"));
        assert!(json.contains("EditCatalog"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_permission_revoked_serialization() {
        let event = UserEvent::PermissionRevoked {
            permission: Permission::IssueContentDownload,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("permission_revoked"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_permissions_reset_serialization() {
        let event = UserEvent::PermissionsReset {
            permissions: vec![
                Permission::AccessCatalog,
                Permission::LikeContent,
                Permission::OwnPlaylists,
            ],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("permissions_reset"));
        assert!(json.contains("AccessCatalog"));
        assert!(json.contains("LikeContent"));
        assert!(json.contains("OwnPlaylists"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_stored_event_serialization() {
        let stored = StoredEvent {
            seq: 42,
            event: UserEvent::ContentLiked {
                content_type: LikedContentType::Artist,
                content_id: "artist_789".to_string(),
            },
            server_timestamp: 1701700000,
        };
        let json = serde_json::to_string(&stored).unwrap();
        assert!(json.contains("\"seq\":42"));
        assert!(json.contains("content_liked"));
        assert!(json.contains("1701700000"));

        let parsed: StoredEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(stored, parsed);
    }

    #[test]
    fn test_event_type_method() {
        assert_eq!(
            UserEvent::ContentLiked {
                content_type: LikedContentType::Album,
                content_id: "x".to_string()
            }
            .event_type(),
            "content_liked"
        );
        assert_eq!(
            UserEvent::ContentUnliked {
                content_type: LikedContentType::Track,
                content_id: "x".to_string()
            }
            .event_type(),
            "content_unliked"
        );
        assert_eq!(
            UserEvent::SettingChanged {
                setting: UserSetting::DirectDownloadsEnabled(false)
            }
            .event_type(),
            "setting_changed"
        );
        assert_eq!(
            UserEvent::PlaylistCreated {
                playlist_id: "x".to_string(),
                name: "y".to_string()
            }
            .event_type(),
            "playlist_created"
        );
        assert_eq!(
            UserEvent::PlaylistRenamed {
                playlist_id: "x".to_string(),
                name: "y".to_string()
            }
            .event_type(),
            "playlist_renamed"
        );
        assert_eq!(
            UserEvent::PlaylistDeleted {
                playlist_id: "x".to_string()
            }
            .event_type(),
            "playlist_deleted"
        );
        assert_eq!(
            UserEvent::PlaylistTracksUpdated {
                playlist_id: "x".to_string(),
                track_ids: vec![]
            }
            .event_type(),
            "playlist_tracks_updated"
        );
        assert_eq!(
            UserEvent::PermissionGranted {
                permission: Permission::AccessCatalog
            }
            .event_type(),
            "permission_granted"
        );
        assert_eq!(
            UserEvent::PermissionRevoked {
                permission: Permission::AccessCatalog
            }
            .event_type(),
            "permission_revoked"
        );
        assert_eq!(
            UserEvent::PermissionsReset { permissions: vec![] }.event_type(),
            "permissions_reset"
        );
    }
}
