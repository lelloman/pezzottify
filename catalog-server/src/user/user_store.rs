use super::auth::{AuthToken, AuthTokenValue, UserAuthCredentials};
use super::permissions::{Permission, PermissionGrant, UserRole};
use super::user_models::{LikedContentType, UserPlaylist};
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
    /// Returns None if the token does not exist.
    fn get_user_auth_token(&self, token: &AuthTokenValue) -> Option<AuthToken>;

    /// Deletes an auth token given the token value.
    /// Returns None if the token does not exist.
    fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Option<AuthToken>;

    /// Updates an auth token with the laatest timestamp.
    /// Returns None if the token does not exist.
    fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()>;

    /// Adds a new auth token.
    /// Returns None if the token already exists.
    fn add_user_auth_token(&self, token: AuthToken) -> Result<()>;

    /// Returns all user's authentication tokens.
    fn get_all_user_auth_tokens(&self, user_handle: &str) -> Vec<AuthToken>;

    /// Prunes unused auth tokens that haven't been used for the specified duration.
    /// Returns the number of tokens that were deleted.
    fn prune_unused_auth_tokens(&self, unused_for_days: u64) -> Result<usize>;
}

pub trait UserStore: UserAuthTokenStore + UserAuthCredentialsStore + Send + Sync {
    /// Creates a new user and returns the user id.
    fn create_user(&self, user_handle: &str) -> Result<usize>;

    // Returns a full user object for the given user id.
    // Returns None if the user does not exist.
    fn get_user_handle(&self, user_id: usize) -> Option<String>;

    /// Returns all users' handles.
    fn get_all_user_handles(&self) -> Vec<String>;

    /// Returns a user's handle given the user id.
    /// Returns None if the user does not exist.
    fn get_user_id(&self, user_handle: &str) -> Option<usize>;

    /// Returns if the user liked the content with the given id,
    /// returns None if the user does not exist.
    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Option<bool>;

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
    fn remove_user_extra_permission(&self, permission_id: usize) -> Result<()>;

    /// Decrements the countdown of an extra permission grant.
    /// Returns true if the permission still has uses remaining, false otherwise.
    fn decrement_permission_countdown(&self, permission_id: usize) -> Result<bool>;

    /// Resolves all permissions for a user (roles + active extra permissions).
    fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<Permission>>;
}
