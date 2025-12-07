use super::auth::{AuthToken, AuthTokenValue, UserAuthCredentials};
use super::device::{Device, DeviceRegistration};
use super::permissions::{Permission, PermissionGrant, UserRole};
use super::settings::UserSetting;
use super::user_models::{
    BandwidthSummary, BandwidthUsage, DailyListeningStats, LikedContentType, ListeningEvent,
    ListeningSummary, TrackListeningStats, UserListeningHistoryEntry, UserPlaylist,
};
use anyhow::Result;

pub trait UserAuthCredentialsStore: Send + Sync {
    /// Returns the user's authentication credentials given the user handle.
    /// Returns Ok(None) if the user does not exist.
    /// Returns Err if there is a database error.
    fn get_user_auth_credentials(&self, user_handle: &str) -> Result<Option<UserAuthCredentials>>;

    /// Updates the user's authentication credentials.
    /// Returns None if the user does not exist.
    fn update_user_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()>;
}

pub trait UserAuthTokenStore: Send + Sync {
    /// Returns a user's authentication token given an AuthTokenValue.
    /// Returns Ok(None) if the token does not exist.
    /// Returns Err if there is a database error.
    fn get_user_auth_token(&self, token: &AuthTokenValue) -> Result<Option<AuthToken>>;

    /// Deletes an auth token given the token value.
    /// Returns Ok(None) if the token does not exist.
    /// Returns Err if there is a database error.
    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Result<Option<AuthToken>>;

    /// Updates an auth token with the laatest timestamp.
    /// Returns None if the token does not exist.
    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()>;

    /// Adds a new auth token.
    /// Returns None if the token already exists.
    fn add_user_auth_token(&self, token: AuthToken) -> Result<()>;

    /// Returns all user's authentication tokens.
    /// Returns Err if there is a database error.
    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Result<Vec<AuthToken>>;

    /// Prunes unused auth tokens that haven't been used for the specified duration.
    /// Returns the number of tokens that were deleted.
    fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize>;
}

pub trait UserStore: UserAuthTokenStore + UserAuthCredentialsStore + Send + Sync {
    /// Creates a new user and returns the user id.
    fn create_user(&self, user_handle: &str) -> Result<usize>;

    /// Deletes a user and all associated data.
    /// Returns Ok(true) if user was deleted, Ok(false) if user didn't exist.
    fn delete_user(&self, user_id: usize) -> Result<bool>;

    // Returns a full user object for the given user id.
    // Returns Ok(None) if the user does not exist.
    // Returns Err if there is a database error.
    fn get_user_handle(&self, user_id: usize) -> Result<Option<String>>;

    /// Returns all users' handles.
    /// Returns Err if there is a database error.
    fn get_all_user_handles(&self) -> Result<Vec<String>>;

    /// Returns a user's handle given the user id.
    /// Returns Ok(None) if the user does not exist.
    /// Returns Err if there is a database error.
    fn get_user_id(&self, user_handle: &str) -> Result<Option<usize>>;

    /// Returns if the user liked the content with the given id,
    /// Returns Ok(None) if the user does not exist.
    /// Returns Err if there is a database error.
    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Result<Option<bool>>;

    /// Sets the liked status of the content with the given id.
    /// Returns None if the user does not exist.
    fn set_user_liked_content(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
    ) -> Result<()>;

    /// Returns the users's playlists.
    fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>>;

    /// Returns the user's liked content.
    /// Returns None if the user does not exist.
    fn get_user_liked_content(
        &self,
        user_id: usize,
        content_type: LikedContentType,
    ) -> Result<Vec<String>>;

    /// Creates a new user playlist.
    fn create_user_playlist(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_id: usize,
        track_ids: Vec<String>,
    ) -> Result<String>;

    /// Updates a user playlist.
    fn update_user_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
    ) -> Result<()>;

    /// Deletes a user playlist given the playlist id and its owner's id.
    fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()>;

    /// Get a user playlist given the playlist id and its owner's id.
    fn get_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<UserPlaylist>;

    /// Returns all roles assigned to a user.
    fn get_user_roles(&self, user_id: usize) -> Result<Vec<UserRole>>;

    /// Assigns a role to a user.
    fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()>;

    /// Removes a role from a user.
    fn remove_user_role(&self, user_id: usize, role: UserRole) -> Result<()>;

    /// Adds an extra permission grant to a user. Returns the grant id.
    fn add_user_extra_permission(&self, user_id: usize, grant: PermissionGrant) -> Result<usize>;

    /// Removes an extra permission grant by its id.
    /// Returns the user_id and permission that was deleted, or None if not found.
    fn remove_user_extra_permission(&self, permission_id: usize) -> Result<Option<(usize, Permission)>>;

    /// Decrements the countdown of an extra permission grant.
    /// Returns true if the permission still has uses remaining, false otherwise.
    fn decrement_permission_countdown(&self, permission_id: usize) -> Result<bool>;

    /// Resolves all permissions for a user (roles + active extra permissions).
    fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>>;
}

/// Trait for bandwidth usage tracking operations
pub trait UserBandwidthStore: Send + Sync {
    /// Records bandwidth usage for a user. Uses upsert to aggregate with existing data for the same day/category.
    fn record_bandwidth_usage(
        &self,
        user_id: usize,
        date: u32,
        endpoint_category: &str,
        bytes_sent: u64,
        request_count: u64,
    ) -> Result<()>;

    /// Gets bandwidth usage records for a user within a date range.
    /// Both start_date and end_date are inclusive and in YYYYMMDD format.
    fn get_user_bandwidth_usage(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>>;

    /// Gets summarized bandwidth usage for a user within a date range.
    fn get_user_bandwidth_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary>;

    /// Gets bandwidth usage for all users (admin only) within a date range.
    fn get_all_bandwidth_usage(&self, start_date: u32, end_date: u32) -> Result<Vec<BandwidthUsage>>;

    /// Gets the total bandwidth summary across all users within a date range.
    fn get_total_bandwidth_summary(&self, start_date: u32, end_date: u32) -> Result<BandwidthSummary>;

    /// Prunes bandwidth usage records older than the specified number of days.
    /// Returns the number of records deleted.
    fn prune_bandwidth_usage(&self, older_than_days: u32) -> Result<usize>;
}

/// Trait for listening statistics tracking operations
pub trait UserListeningStore: Send + Sync {
    /// Records a listening event. If session_id already exists, returns Ok without inserting
    /// (idempotent for offline queue retry). Returns the event id and whether it was created.
    fn record_listening_event(&self, event: ListeningEvent) -> Result<(usize, bool)>;

    /// Gets listening events for a user within a date range (paginated).
    /// Both start_date and end_date are inclusive and in YYYYMMDD format.
    fn get_user_listening_events(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ListeningEvent>>;

    /// Gets summarized listening stats for a user within a date range.
    fn get_user_listening_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<ListeningSummary>;

    /// Gets a user's listening history (recently played tracks, aggregated by track).
    fn get_user_listening_history(
        &self,
        user_id: usize,
        limit: usize,
    ) -> Result<Vec<UserListeningHistoryEntry>>;

    /// Gets listening stats for a specific track within a date range (admin).
    fn get_track_listening_stats(
        &self,
        track_id: &str,
        start_date: u32,
        end_date: u32,
    ) -> Result<TrackListeningStats>;

    /// Gets daily aggregated listening stats within a date range (admin).
    fn get_daily_listening_stats(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<DailyListeningStats>>;

    /// Gets top tracks by play count within a date range (admin).
    fn get_top_tracks(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<TrackListeningStats>>;

    /// Prunes listening events older than the specified number of days.
    /// Returns the number of events deleted.
    fn prune_listening_events(&self, older_than_days: u32) -> Result<usize>;
}

/// Trait for user settings storage operations.
///
/// The trait works with typed `UserSetting` enum values.
/// Implementations handle serialization to/from strings internally.
pub trait UserSettingsStore: Send + Sync {
    /// Gets a user setting by key.
    /// Returns Ok(None) if the setting does not exist.
    /// Returns Err if the stored value cannot be parsed.
    fn get_user_setting(&self, user_id: usize, key: &str) -> Result<Option<UserSetting>>;

    /// Sets a user setting. Creates or updates the setting.
    fn set_user_setting(&self, user_id: usize, setting: UserSetting) -> Result<()>;

    /// Gets all settings for a user.
    /// Unknown keys in the database are skipped (for forward compatibility).
    fn get_all_user_settings(&self, user_id: usize) -> Result<Vec<UserSetting>>;
}

/// Trait for device storage operations
pub trait DeviceStore: Send + Sync {
    /// Register a new device or get existing one by device_uuid.
    /// If device exists, updates device_type/name/os_info and last_seen.
    /// Returns the device ID.
    fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize>;

    /// Get device by ID
    fn get_device(&self, device_id: usize) -> Result<Option<Device>>;

    /// Get device by UUID
    fn get_device_by_uuid(&self, device_uuid: &str) -> Result<Option<Device>>;

    /// Get all devices for a user
    fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>>;

    /// Update device's associated user (called on login)
    fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()>;

    /// Update last_seen timestamp
    fn touch_device(&self, device_id: usize) -> Result<()>;

    /// Prune orphaned devices (user_id IS NULL) that haven't been seen for the specified days.
    /// Returns the number of devices deleted.
    fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize>;

    /// Enforce per-user device limit by pruning oldest devices (by last_seen).
    /// Called after associating a device with a user during login.
    /// Returns the number of devices deleted.
    fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize>;
}

use super::sync_events::{StoredEvent, UserEvent};

/// Trait for sync event storage operations
pub trait UserEventStore: Send + Sync {
    /// Appends an event to the user's event log.
    /// Returns the stored event with sequence number and server timestamp.
    fn append_event(&self, user_id: usize, event: &UserEvent) -> Result<StoredEvent>;

    /// Gets events since a given sequence number.
    /// Returns events with seq > since_seq, ordered by seq ascending.
    fn get_events_since(&self, user_id: usize, since_seq: i64) -> Result<Vec<StoredEvent>>;

    /// Gets the current (latest) sequence number for a user.
    /// Returns 0 if no events exist.
    fn get_current_seq(&self, user_id: usize) -> Result<i64>;

    /// Gets the minimum available sequence number for a user.
    /// Returns None if no events exist.
    /// Used to detect if requested sequence has been pruned.
    fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>>;

    /// Deletes events older than the given Unix timestamp.
    /// Returns the number of deleted events.
    fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64>;
}

/// Combined trait for user storage with bandwidth, listening tracking, settings, devices, and events
pub trait FullUserStore:
    UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore + UserEventStore
{
}

// Blanket implementation for any type that implements all user store traits
impl<T: UserStore + UserBandwidthStore + UserListeningStore + UserSettingsStore + DeviceStore + UserEventStore>
    FullUserStore for T
{
}
