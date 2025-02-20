use super::user_models::{User, UserPlaylist, UserSessionView};
use anyhow::Result;

pub trait UserStore: Send + Sync {
    /// Creates a new user and returns the user id.
    fn create_user(&self, user_handle: &str) -> Result<usize>;

    // Returns a full user object for the given user id.
    // Returns None if the user does not exist.
    //fn get_user(&self, user_id: usize) -> Option<User>;

    /// Returns if the user liked the content with the given id,
    /// returns None if the user does not exist.
    fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Option<bool>;

    /// Sets the liked status of the content with the given id.
    /// Returns None if the user does not exist.
    fn set_user_liked_content(&self, user_id: usize, content_id: &str, liked: bool) -> Result<()>;

    /// Returns the users's playlists.
    /// Returns None if the user does not exist.
    fn get_user_playlists(&self, user_id: &str) -> Option<Vec<UserPlaylist>>;

    // Returns a user object to be used for session management.
    // Returns None if the user does not exist.
    //fn get_user_session_view(&self, user_id: &str) -> Option<UserSessionView>;
}
