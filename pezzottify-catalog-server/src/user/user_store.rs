use super::user::{User, UserPlaylist};
use anyhow::Result;

pub trait UserStore {
    /// Creates a new user and returns the user id.
    fn create_user(&self, user: User) -> Option<String>;

    /// Updates the user with the given id.
    fn update_user(&self, user_id: &str, user: User) -> Result<()>;

    /// Returns if the user liked the content with the given id,
    /// returns None if the user does not exist.
    fn user_liked_content(&self, user_id: &str, content_id: &str) -> Option<bool>;

    /// Returns the users's playlists.
    /// Returns None if the user does not exist.
    fn get_user_playlists(&self, user_id: &str) -> Option<Vec<UserPlaylist>>;
}
