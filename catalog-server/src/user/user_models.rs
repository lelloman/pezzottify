//! User data models
#![allow(dead_code)] // Models for future API endpoints

use std::collections::HashMap;
use std::time::SystemTime;

use serde::Serialize;

pub struct User {
    pub id: String,
    pub handle: String,
    pub liked_content: HashMap<String, UserLikedContent>,
    pub playlists: HashMap<String, UserPlaylist>,
}

pub enum LikedContentType {
    Artist,
    Album,
    Track,
    Unknown,
}

impl LikedContentType {
    pub fn to_int(&self) -> i32 {
        match self {
            LikedContentType::Artist => 1,
            LikedContentType::Album => 2,
            LikedContentType::Track => 3,
            LikedContentType::Unknown => 0,
        }
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            1 => LikedContentType::Artist,
            2 => LikedContentType::Album,
            3 => LikedContentType::Track,
            _ => LikedContentType::Unknown,
        }
    }

    pub fn from_id(id: &str) -> Self {
        if id.is_empty() {
            return LikedContentType::Unknown;
        }
        let first_char = id.chars().next().unwrap();
        match first_char {
            'R' => LikedContentType::Artist,
            'A' => LikedContentType::Album,
            'T' => LikedContentType::Track,
            _ => LikedContentType::Unknown,
        }
    }
}

pub struct UserLikedContent {
    pub timestamp: SystemTime,
    pub content_id: String,
    pub content_type: LikedContentType,
}

#[derive(Serialize, Debug)]
pub struct UserPlaylist {
    pub id: String,
    pub user_id: usize,
    pub creator: String,
    pub name: String,
    pub created: SystemTime,
    pub tracks: Vec<String>,
}
