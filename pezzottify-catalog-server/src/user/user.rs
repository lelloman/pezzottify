use std::collections::HashMap;
use std::time::SystemTime;

use super::permissions::Permission;

pub struct User {
    pub id: String,
    pub handle: String,
    pub liked_content: HashMap<String, UserLikedContent>,
    pub playlists: HashMap<String, UserPlaylist>,
}

pub struct UserSessionView {
    pub id: String,
    pub permissions: Vec<Permission>,
}

pub enum LikedContentType {
    Album,
    Artist,
    Track,
}

pub struct UserLikedContent {
    pub contentType: LikedContentType,
    pub timestamp: SystemTime,
    pub contentId: String,
}

pub struct UserPlaylist {
    pub id: String,
    pub name: String,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub tracks: Vec<String>,
}
