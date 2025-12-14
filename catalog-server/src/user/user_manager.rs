use crate::catalog_store::CatalogStore;

use super::{
    auth::PezzottifyHasher,
    device::{Device, DeviceRegistration},
    permissions::{Permission, PermissionGrant, UserRole},
    settings::UserSetting,
    user_models::{
        BandwidthSummary, BandwidthUsage, DailyListeningStats, LikedContentType, ListeningEvent,
        ListeningSummary, PopularAlbum, PopularArtist, PopularContent, TrackListeningStats,
        UserListeningHistoryEntry,
    },
    AuthToken, AuthTokenValue, FullUserStore, UserAuthCredentials, UserPlaylist,
    UsernamePasswordCredentials,
};
use anyhow::{bail, Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};

const MAX_PLAYLIST_SIZE: usize = 300;
const POPULAR_CONTENT_CACHE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// Cached popular content with computation timestamp
struct CachedPopularContent {
    content: PopularContent,
    computed_at: Instant,
}

pub struct UserManager {
    catalog_store: Arc<dyn CatalogStore>,
    user_store: Arc<dyn FullUserStore>,
    popular_content_cache: Mutex<Option<CachedPopularContent>>,
}

impl UserManager {
    pub fn new(catalog_store: Arc<dyn CatalogStore>, user_store: Arc<dyn FullUserStore>) -> Self {
        Self {
            catalog_store,
            user_store,
            popular_content_cache: Mutex::new(None),
        }
    }

    pub fn add_user<T: AsRef<str>>(&self, user_handle: T) -> Result<usize> {
        if self.user_store.get_user_id(user_handle.as_ref())?.is_some() {
            bail!("User handle already exists.");
        }

        if user_handle.as_ref().is_empty() {
            bail!("The user handle cannot be empty.")
        }

        self.user_store.create_user(user_handle.as_ref())
    }

    /// Deletes a user and all associated data.
    /// Returns Ok(true) if user was deleted, Ok(false) if user didn't exist.
    pub fn delete_user(&self, user_id: usize) -> Result<bool> {
        self.user_store.delete_user(user_id)
    }

    pub fn set_user_liked_content(
        &self,
        user_id: usize,
        content_id: &str,
        content_type: LikedContentType,
        liked: bool,
    ) -> anyhow::Result<()> {
        self.user_store
            .set_user_liked_content(user_id, content_id, content_type, liked)
    }

    /// Append an event to the user's sync event log.
    pub fn append_event(
        &self,
        user_id: usize,
        event: &super::sync_events::UserEvent,
    ) -> Result<super::sync_events::StoredEvent> {
        self.user_store.append_event(user_id, event)
    }

    pub fn get_auth_token(&self, value: &AuthTokenValue) -> Result<Option<AuthToken>> {
        self.user_store.get_user_auth_token(value)
    }

    pub fn update_auth_token_last_used(&self, value: &AuthTokenValue) -> Result<()> {
        self.user_store
            .update_user_auth_token_last_used_timestamp(value)
    }

    pub fn generate_auth_token(
        &mut self,
        credentials: &UserAuthCredentials,
        device_id: usize,
    ) -> Result<AuthToken> {
        let token = AuthToken {
            user_id: credentials.user_id,
            device_id: Some(device_id),
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        self.user_store.add_user_auth_token(token.clone())?;
        Ok(token)
    }

    fn create_hashed_password(
        user_id: usize,
        password: String,
    ) -> Result<UsernamePasswordCredentials> {
        let hasher = PezzottifyHasher::Argon2;
        let salt = hasher.generate_b64_salt();
        let hash = hasher.hash(password.as_bytes(), &salt)?;
        Ok(UsernamePasswordCredentials {
            user_id,
            salt,
            hash,
            hasher,
            created: SystemTime::now(),
            last_tried: None,
            last_used: None,
        })
    }

    pub fn create_password_credentials(
        &mut self,
        user_handle: &String,
        password: String,
    ) -> Result<()> {
        if let Some(true) = self
            .user_store
            .get_user_auth_credentials(user_handle)?
            .map(|x| x.username_password.is_some())
        {
            bail!("User with handle {} already has password credentials method. Maybe you want to modify it?", user_handle);
        }

        let user_id = self
            .user_store
            .get_user_id(user_handle)?
            .with_context(|| format!("User with handle {} not found.", user_handle))?;

        let mut new_credentials = self
            .user_store
            .get_user_auth_credentials(user_handle)?
            .unwrap_or_else(|| UserAuthCredentials {
                user_id,
                username_password: None,
                keys: vec![],
            });
        new_credentials.username_password = Some(Self::create_hashed_password(user_id, password)?);

        self.user_store
            .update_user_auth_credentials(new_credentials.clone())
    }

    pub fn update_password_credentials(
        &mut self,
        user_handle: &String,
        password: String,
    ) -> Result<()> {
        let mut credentials = self
            .user_store
            .get_user_auth_credentials(user_handle)?
            .with_context(|| format!("User with handle {} not found.", user_handle))?;
        if credentials.username_password.is_none() {
            bail!(
                "Cannot update passowrd of user with handle {} since it never had one.",
                user_handle
            );
        }
        credentials.username_password =
            Some(Self::create_hashed_password(credentials.user_id, password)?);
        self.user_store
            .update_user_auth_credentials(credentials.clone())
    }

    pub fn delete_password_credentials(&mut self, user_handle: &String) -> Result<()> {
        let mut credentials = self
            .user_store
            .get_user_auth_credentials(user_handle)?
            .with_context(|| format!("User with handle {} not found.", user_handle))?;
        credentials.username_password = None;
        self.user_store
            .update_user_auth_credentials(credentials.clone())
    }

    pub fn get_user_credentials(&self, user_handle: &str) -> Result<Option<UserAuthCredentials>> {
        self.user_store.get_user_auth_credentials(user_handle)
    }

    pub fn delete_auth_token(
        &mut self,
        user_id: &usize,
        token_value: &AuthTokenValue,
    ) -> Result<()> {
        let removed = self.user_store.delete_user_auth_token(token_value)?;
        match removed {
            Some(removed) => {
                if &removed.user_id == user_id {
                    Ok(())
                } else {
                    let _ = self.user_store.add_user_auth_token(removed.clone());
                    bail!("Tried to delete auth token {}, but the authenticated user {} was not the owner {} of the token.", token_value.0, user_id, &removed.user_id)
                }
            }
            None => bail!("Did not found auth token {}", token_value.0),
        }
    }

    pub fn get_user_tokens(&self, user_handle: &str) -> Result<Vec<AuthToken>> {
        self.user_store.get_all_user_auth_tokens(user_handle)
    }

    pub fn get_all_user_handles(&self) -> Result<Vec<String>> {
        self.user_store.get_all_user_handles()
    }

    pub fn get_user_id(&self, user_handle: &str) -> Result<Option<usize>> {
        self.user_store.get_user_id(user_handle)
    }

    pub fn get_user_handle(&self, user_id: usize) -> Result<Option<String>> {
        self.user_store.get_user_handle(user_id)
    }

    pub fn get_user_liked_content(
        &self,
        user_id: usize,
        conten_type: LikedContentType,
    ) -> Result<Vec<String>> {
        self.user_store.get_user_liked_content(user_id, conten_type)
    }

    pub fn create_user_playlist(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_id: usize,
        track_ids: Vec<String>,
    ) -> Result<String> {
        if track_ids.len() > MAX_PLAYLIST_SIZE {
            bail!(
                "Playlist size exceeds maximum limit of {} songs (attempted: {}).",
                MAX_PLAYLIST_SIZE,
                track_ids.len()
            );
        }
        self.user_store
            .create_user_playlist(user_id, playlist_name, creator_id, track_ids)
    }

    pub fn update_user_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
    ) -> Result<()> {
        if let Some(ref tracks) = track_ids {
            if tracks.len() > MAX_PLAYLIST_SIZE {
                bail!(
                    "Playlist size exceeds maximum limit of {} songs (attempted: {}).",
                    MAX_PLAYLIST_SIZE,
                    tracks.len()
                );
            }
        }
        self.user_store
            .update_user_playlist(playlist_id, user_id, playlist_name, track_ids)
    }

    pub fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()> {
        self.user_store.delete_user_playlist(playlist_id, user_id)
    }

    pub fn get_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<UserPlaylist> {
        self.user_store.get_user_playlist(playlist_id, user_id)
    }

    pub fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
        self.user_store.get_user_playlists(user_id)
    }

    pub fn add_playlist_tracks(
        &self,
        playlist_id: &str,
        user_id: usize,
        track_ids: Vec<String>,
    ) -> Result<()> {
        let playlist = self.user_store.get_user_playlist(playlist_id, user_id)?;

        // Check if resulting playlist size would exceed the limit
        let resulting_size = playlist.tracks.len() + track_ids.len();
        if resulting_size > MAX_PLAYLIST_SIZE {
            bail!(
                "Adding {} tracks would exceed maximum playlist limit of {} songs (current: {}, resulting: {}).",
                track_ids.len(),
                MAX_PLAYLIST_SIZE,
                playlist.tracks.len(),
                resulting_size
            );
        }

        // verify that all tracks to add exist
        for track_id in &track_ids {
            match self.catalog_store.get_track_json(track_id) {
                Ok(None) | Err(_) => {
                    bail!("Track with id {} does not exist.", track_id);
                }
                Ok(Some(_)) => {}
            }
        }

        let mut new_tracks = playlist.tracks.clone();
        new_tracks.extend(track_ids);
        self.update_user_playlist(playlist_id, user_id, None, Some(new_tracks))
    }

    pub fn remove_tracks_from_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        tracks_positions: Vec<usize>,
    ) -> Result<()> {
        let playlist = self.user_store.get_user_playlist(playlist_id, user_id)?;

        if playlist.user_id != user_id {
            bail!(
                "User {} is not the owner of playlist {}.",
                user_id,
                playlist_id
            );
        }

        let mut new_tracks: Vec<String> = vec![];
        for (i, track_id) in playlist.tracks.iter().enumerate() {
            if !tracks_positions.contains(&i) {
                new_tracks.push(track_id.clone());
            }
        }
        self.update_user_playlist(playlist_id, user_id, None, Some(new_tracks))
    }

    pub fn get_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>> {
        self.user_store.resolve_user_permissions(user_id)
    }

    pub fn get_user_roles(&self, user_id: usize) -> Result<Vec<UserRole>> {
        self.user_store.get_user_roles(user_id)
    }

    pub fn add_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        self.user_store.add_user_role(user_id, role)
    }

    pub fn remove_user_role(&self, user_id: usize, role: UserRole) -> Result<()> {
        self.user_store.remove_user_role(user_id, role)
    }

    pub fn add_user_extra_permission(
        &self,
        user_id: usize,
        grant: PermissionGrant,
    ) -> Result<usize> {
        self.user_store.add_user_extra_permission(user_id, grant)
    }

    pub fn remove_user_extra_permission(
        &self,
        permission_id: usize,
    ) -> Result<Option<(usize, Permission)>> {
        self.user_store.remove_user_extra_permission(permission_id)
    }

    pub fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize> {
        self.user_store.prune_unused_auth_tokens(unused_for_days)
    }

    // Device management methods

    pub fn register_or_update_device(&self, registration: &DeviceRegistration) -> Result<usize> {
        self.user_store.register_or_update_device(registration)
    }

    pub fn associate_device_with_user(&self, device_id: usize, user_id: usize) -> Result<()> {
        self.user_store
            .associate_device_with_user(device_id, user_id)
    }

    pub fn get_device(&self, device_id: usize) -> Result<Option<Device>> {
        self.user_store.get_device(device_id)
    }

    pub fn get_user_devices(&self, user_id: usize) -> Result<Vec<Device>> {
        self.user_store.get_user_devices(user_id)
    }

    pub fn prune_orphaned_devices(&self, inactive_for_days: u32) -> Result<usize> {
        self.user_store.prune_orphaned_devices(inactive_for_days)
    }

    pub fn enforce_user_device_limit(&self, user_id: usize, max_devices: usize) -> Result<usize> {
        self.user_store
            .enforce_user_device_limit(user_id, max_devices)
    }

    // Bandwidth tracking methods

    pub fn record_bandwidth_usage(
        &self,
        user_id: usize,
        date: u32,
        endpoint_category: &str,
        bytes_sent: u64,
        request_count: u64,
    ) -> Result<()> {
        self.user_store.record_bandwidth_usage(
            user_id,
            date,
            endpoint_category,
            bytes_sent,
            request_count,
        )
    }

    pub fn get_user_bandwidth_usage(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
        self.user_store
            .get_user_bandwidth_usage(user_id, start_date, end_date)
    }

    pub fn get_user_bandwidth_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        self.user_store
            .get_user_bandwidth_summary(user_id, start_date, end_date)
    }

    pub fn get_all_bandwidth_usage(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<BandwidthUsage>> {
        self.user_store
            .get_all_bandwidth_usage(start_date, end_date)
    }

    pub fn get_total_bandwidth_summary(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<BandwidthSummary> {
        self.user_store
            .get_total_bandwidth_summary(start_date, end_date)
    }

    pub fn prune_bandwidth_usage(&self, older_than_days: u32) -> Result<usize> {
        self.user_store.prune_bandwidth_usage(older_than_days)
    }

    // Listening stats methods

    pub fn record_listening_event(&self, event: ListeningEvent) -> Result<(usize, bool)> {
        self.user_store.record_listening_event(event)
    }

    pub fn get_user_listening_events(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ListeningEvent>> {
        self.user_store
            .get_user_listening_events(user_id, start_date, end_date, limit, offset)
    }

    pub fn get_user_listening_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<ListeningSummary> {
        self.user_store
            .get_user_listening_summary(user_id, start_date, end_date)
    }

    pub fn get_user_listening_history(
        &self,
        user_id: usize,
        limit: usize,
    ) -> Result<Vec<UserListeningHistoryEntry>> {
        self.user_store.get_user_listening_history(user_id, limit)
    }

    pub fn get_track_listening_stats(
        &self,
        track_id: &str,
        start_date: u32,
        end_date: u32,
    ) -> Result<TrackListeningStats> {
        self.user_store
            .get_track_listening_stats(track_id, start_date, end_date)
    }

    pub fn get_daily_listening_stats(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<DailyListeningStats>> {
        self.user_store
            .get_daily_listening_stats(start_date, end_date)
    }

    pub fn get_top_tracks(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<TrackListeningStats>> {
        self.user_store.get_top_tracks(start_date, end_date, limit)
    }

    pub fn prune_listening_events(&self, older_than_days: u32) -> Result<usize> {
        self.user_store.prune_listening_events(older_than_days)
    }

    // User settings methods

    pub fn get_user_setting(&self, user_id: usize, key: &str) -> Result<Option<UserSetting>> {
        self.user_store.get_user_setting(user_id, key)
    }

    pub fn set_user_setting(&self, user_id: usize, setting: UserSetting) -> Result<()> {
        self.user_store.set_user_setting(user_id, setting)
    }

    pub fn get_all_user_settings(&self, user_id: usize) -> Result<Vec<UserSetting>> {
        self.user_store.get_all_user_settings(user_id)
    }

    // ========================================================================
    // Notification Methods
    // ========================================================================

    /// Get all notifications for a user, ordered by created_at DESC.
    pub fn get_user_notifications(
        &self,
        user_id: usize,
    ) -> Result<Vec<crate::notifications::Notification>> {
        self.user_store.get_user_notifications(user_id)
    }

    /// Get a single notification by ID (verifies ownership).
    pub fn get_notification(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        self.user_store.get_notification(notification_id, user_id)
    }

    /// Mark a notification as read. Returns the updated notification.
    pub fn mark_notification_read(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<crate::notifications::Notification>> {
        self.user_store
            .mark_notification_read(notification_id, user_id)
    }

    /// Get count of unread notifications for a user.
    pub fn get_unread_count(&self, user_id: usize) -> Result<usize> {
        self.user_store.get_unread_count(user_id)
    }

    /// Create a notification for a user.
    pub fn create_notification(
        &self,
        user_id: usize,
        notification_type: crate::notifications::NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<crate::notifications::Notification> {
        self.user_store
            .create_notification(user_id, notification_type, title, body, data)
    }

    // ========================================================================
    // Popular Content Methods
    // ========================================================================

    /// Get popular albums and artists based on listening data.
    /// Results are cached for 24 hours to avoid repeated expensive queries.
    /// The cache stores max results (20 albums, 20 artists) and slices to requested limits.
    pub fn get_popular_content(
        &self,
        start_date: u32,
        end_date: u32,
        albums_limit: usize,
        artists_limit: usize,
    ) -> Result<PopularContent> {
        const MAX_ALBUMS: usize = 20;
        const MAX_ARTISTS: usize = 20;

        // Check cache first
        {
            let cache = self.popular_content_cache.lock().unwrap();
            if let Some(cached) = cache.as_ref() {
                if cached.computed_at.elapsed().as_secs() < POPULAR_CONTENT_CACHE_TTL_SECS {
                    // Cache hit - slice to requested limits and return
                    return Ok(PopularContent {
                        albums: cached
                            .content
                            .albums
                            .iter()
                            .take(albums_limit)
                            .cloned()
                            .collect(),
                        artists: cached
                            .content
                            .artists
                            .iter()
                            .take(artists_limit)
                            .cloned()
                            .collect(),
                    });
                }
            }
        }

        // Cache miss or stale - compute fresh with max limits
        let content =
            self.compute_popular_content(start_date, end_date, MAX_ALBUMS, MAX_ARTISTS)?;

        // Store in cache
        {
            let mut cache = self.popular_content_cache.lock().unwrap();
            *cache = Some(CachedPopularContent {
                content: content.clone(),
                computed_at: Instant::now(),
            });
        }

        // Slice to requested limits
        Ok(PopularContent {
            albums: content.albums.into_iter().take(albums_limit).collect(),
            artists: content.artists.into_iter().take(artists_limit).collect(),
        })
    }

    /// Sets the popular content cache directly.
    ///
    /// This is used by background jobs to pre-compute and cache popular content,
    /// avoiding redundant computation when the endpoint is called.
    pub fn set_popular_content_cache(&self, content: PopularContent) {
        let mut cache = self.popular_content_cache.lock().unwrap();
        *cache = Some(CachedPopularContent {
            content,
            computed_at: Instant::now(),
        });
    }

    /// Computes popular content by aggregating play counts.
    ///
    /// Albums are computed from top tracks (albums with many low-play tracks
    /// aren't necessarily "popular albums").
    ///
    /// Artists are computed from ALL tracks to ensure artists with many
    /// medium-popularity tracks aren't underrepresented compared to artists
    /// with one viral hit.
    fn compute_popular_content(
        &self,
        start_date: u32,
        end_date: u32,
        albums_limit: usize,
        artists_limit: usize,
    ) -> Result<PopularContent> {
        // Compute popular albums from top tracks
        let popular_albums = self.compute_popular_albums(start_date, end_date, albums_limit)?;

        // Compute popular artists from ALL track play counts
        let popular_artists = self.compute_popular_artists(start_date, end_date, artists_limit)?;

        Ok(PopularContent {
            albums: popular_albums,
            artists: popular_artists,
        })
    }

    /// Computes popular albums by aggregating play counts from top tracks.
    fn compute_popular_albums(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<PopularAlbum>> {
        // Get top tracks with a higher limit to ensure we have enough to aggregate
        let track_limit = limit * 5;
        let top_tracks = self
            .user_store
            .get_top_tracks(start_date, end_date, track_limit)?;

        // Aggregate play counts by album
        let mut album_plays: HashMap<String, u64> = HashMap::new();
        for track_stats in &top_tracks {
            if let Some(album_id) = self.catalog_store.get_track_album_id(&track_stats.track_id) {
                *album_plays.entry(album_id).or_insert(0) += track_stats.play_count;
            }
        }

        // Sort albums by play count and take top N
        let mut album_list: Vec<_> = album_plays.into_iter().collect();
        album_list.sort_by(|a, b| b.1.cmp(&a.1));

        let popular_albums: Vec<PopularAlbum> = album_list
            .into_iter()
            .take(limit)
            .filter_map(|(album_id, play_count)| {
                // Get resolved album JSON to get name, image, and artists
                if let Ok(Some(album_json)) = self.catalog_store.get_resolved_album_json(&album_id)
                {
                    let id = album_json
                        .get("album")
                        .and_then(|a| a.get("id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or(&album_id)
                        .to_string();
                    let name = album_json
                        .get("album")
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let image_id = album_json
                        .get("display_image")
                        .and_then(|img| img.get("id"))
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string());
                    let artist_names: Vec<String> = album_json
                        .get("artists")
                        .and_then(|a| a.as_array())
                        .map(|artists| {
                            artists
                                .iter()
                                .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    Some(PopularAlbum {
                        id,
                        name,
                        image_id,
                        artist_names,
                        play_count,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(popular_albums)
    }

    /// Computes popular artists by aggregating play counts from ALL tracks.
    ///
    /// Unlike albums, we consider ALL listened tracks to avoid underrepresenting
    /// artists who have many tracks with moderate play counts versus artists
    /// with a single viral hit.
    fn compute_popular_artists(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<PopularArtist>> {
        // Get ALL track play counts (no limit)
        let all_track_counts = self
            .user_store
            .get_all_track_play_counts(start_date, end_date)?;

        // Aggregate play counts by artist
        let mut artist_plays: HashMap<String, u64> = HashMap::new();
        for track_count in &all_track_counts {
            // Get track artists from resolved track JSON
            if let Ok(Some(track_json)) = self
                .catalog_store
                .get_resolved_track_json(&track_count.track_id)
            {
                if let Some(artists) = track_json.get("artists").and_then(|a| a.as_array()) {
                    for track_artist in artists {
                        // TrackArtist has nested structure: { "artist": { "id": "..." }, "role": "..." }
                        if let Some(artist_id) = track_artist
                            .get("artist")
                            .and_then(|a| a.get("id"))
                            .and_then(|id| id.as_str())
                        {
                            *artist_plays.entry(artist_id.to_string()).or_insert(0) +=
                                track_count.play_count;
                        }
                    }
                }
            }
        }

        // Sort artists by play count and take top N
        let mut artist_list: Vec<_> = artist_plays.into_iter().collect();
        artist_list.sort_by(|a, b| b.1.cmp(&a.1));

        let popular_artists: Vec<PopularArtist> = artist_list
            .into_iter()
            .take(limit)
            .filter_map(|(artist_id, play_count)| {
                // Get resolved artist JSON to get name and image
                if let Ok(Some(artist_json)) =
                    self.catalog_store.get_resolved_artist_json(&artist_id)
                {
                    let id = artist_json
                        .get("artist")
                        .and_then(|a| a.get("id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or(&artist_id)
                        .to_string();
                    let name = artist_json
                        .get("artist")
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let image_id = artist_json
                        .get("display_image")
                        .and_then(|img| img.get("id"))
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string());

                    Some(PopularArtist {
                        id,
                        name,
                        image_id,
                        play_count,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(popular_artists)
    }

    // ========================================================================
    // Sync Event Methods
    // ========================================================================

    /// Get events since a given sequence number.
    pub fn get_events_since(
        &self,
        user_id: usize,
        since_seq: i64,
    ) -> Result<Vec<super::sync_events::StoredEvent>> {
        self.user_store.get_events_since(user_id, since_seq)
    }

    /// Get the current (latest) sequence number for a user.
    pub fn get_current_seq(&self, user_id: usize) -> Result<i64> {
        self.user_store.get_current_seq(user_id)
    }

    /// Get the minimum available sequence number for a user.
    pub fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>> {
        self.user_store.get_min_seq(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::NullCatalogStore;
    use crate::user::SqliteUserStore;
    use std::thread;
    use tempfile::TempDir;

    fn create_test_manager() -> (UserManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let temp_file_path = temp_dir.path().join("test.db");
        let user_store = Arc::new(SqliteUserStore::new(&temp_file_path).unwrap());
        let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
        let manager = UserManager::new(catalog_store, user_store);
        (manager, temp_dir)
    }

    /// Test that UserManager can be safely shared across multiple threads
    /// and that concurrent operations on different users don't conflict.
    #[test]
    fn test_concurrent_operations_on_different_users() {
        let (manager, _temp_dir) = create_test_manager();
        let manager = Arc::new(manager);

        // Create users first
        let user1_id = manager.add_user("user1").unwrap();
        let user2_id = manager.add_user("user2").unwrap();

        let num_threads = 4;
        let operations_per_thread = 5;

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let manager = Arc::clone(&manager);
                let user_id = if i % 2 == 0 { user1_id } else { user2_id };
                thread::spawn(move || {
                    for j in 0..operations_per_thread {
                        // Alternate between set_user_setting and get operations
                        let setting = UserSetting::ExternalSearchEnabled(j % 2 == 0);
                        manager.set_user_setting(user_id, setting).unwrap();
                        let _ = manager.get_all_user_settings(user_id).unwrap();
                        let _ = manager
                            .get_user_setting(user_id, "enable_external_search")
                            .unwrap();
                    }
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify we can still read the settings
        let settings1 = manager.get_all_user_settings(user1_id).unwrap();
        let settings2 = manager.get_all_user_settings(user2_id).unwrap();
        assert_eq!(settings1.len(), 1);
        assert_eq!(settings2.len(), 1);
    }

    /// Test that concurrent writes to the same user's settings are handled correctly.
    /// The last write should win, and no data corruption should occur.
    #[test]
    fn test_concurrent_writes_same_user() {
        let (manager, _temp_dir) = create_test_manager();
        let manager = Arc::new(manager);

        let user_id = manager.add_user("test_user").unwrap();

        let num_threads = 4;
        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    // Half the threads set to true, half to false
                    let enabled = i % 2 == 0;
                    for _ in 0..5 {
                        manager
                            .set_user_setting(user_id, UserSetting::ExternalSearchEnabled(enabled))
                            .unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify the setting exists and is valid (either true or false)
        let setting = manager
            .get_user_setting(user_id, "enable_external_search")
            .unwrap();
        assert!(matches!(
            setting,
            Some(UserSetting::ExternalSearchEnabled(_))
        ));
    }

    /// Test concurrent read and write operations don't cause deadlocks or errors.
    #[test]
    fn test_concurrent_read_write_no_deadlock() {
        let (manager, _temp_dir) = create_test_manager();
        let manager = Arc::new(manager);

        let user_id = manager.add_user("test_user").unwrap();

        // Initialize the setting
        manager
            .set_user_setting(user_id, UserSetting::ExternalSearchEnabled(false))
            .unwrap();

        let num_readers = 4;
        let num_writers = 2;

        let mut handles = Vec::new();

        // Spawn reader threads
        for _ in 0..num_readers {
            let manager = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _ = manager
                        .get_user_setting(user_id, "enable_external_search")
                        .unwrap();
                    let _ = manager.get_all_user_settings(user_id).unwrap();
                }
            }));
        }

        // Spawn writer threads
        for i in 0..num_writers {
            let manager = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for j in 0..5 {
                    let enabled = (i + j) % 2 == 0;
                    manager
                        .set_user_setting(user_id, UserSetting::ExternalSearchEnabled(enabled))
                        .unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked - potential deadlock");
        }

        // Verify we can still read after all concurrent operations
        let _ = manager.get_all_user_settings(user_id).unwrap();
    }

    /// Test that multiple operations (add_user, set_setting, get_setting) from
    /// multiple threads on the same UserManager instance work correctly.
    #[test]
    fn test_mixed_concurrent_operations() {
        let (manager, _temp_dir) = create_test_manager();
        let manager = Arc::new(manager);

        let num_threads = 4;
        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    let user_handle = format!("concurrent_user_{}", i);
                    let user_id = manager.add_user(&user_handle).unwrap();

                    // Each thread works on its own user
                    for j in 0..5 {
                        manager
                            .set_user_setting(
                                user_id,
                                UserSetting::ExternalSearchEnabled(j % 2 == 0),
                            )
                            .unwrap();
                        let _ = manager.get_all_user_settings(user_id).unwrap();
                    }

                    user_id
                })
            })
            .collect();

        let user_ids: Vec<usize> = handles
            .into_iter()
            .map(|h| h.join().expect("Thread panicked"))
            .collect();

        // Verify all users were created and have settings
        for user_id in user_ids {
            let settings = manager.get_all_user_settings(user_id).unwrap();
            assert_eq!(settings.len(), 1);
        }
    }
}
