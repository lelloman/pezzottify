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

/// Bandwidth usage record for a specific user, date, and endpoint category
#[derive(Serialize, Debug, Clone)]
pub struct BandwidthUsage {
    pub user_id: usize,
    /// Date in YYYYMMDD format
    pub date: u32,
    pub endpoint_category: String,
    pub bytes_sent: u64,
    pub request_count: u64,
}

/// Summary of bandwidth usage across multiple records
#[derive(Serialize, Debug, Clone)]
pub struct BandwidthSummary {
    pub user_id: Option<usize>,
    pub total_bytes_sent: u64,
    pub total_requests: u64,
    /// Breakdown by endpoint category
    pub by_category: HashMap<String, CategoryBandwidth>,
}

/// Bandwidth stats for a specific category
#[derive(Serialize, Debug, Clone)]
pub struct CategoryBandwidth {
    pub bytes_sent: u64,
    pub request_count: u64,
}
