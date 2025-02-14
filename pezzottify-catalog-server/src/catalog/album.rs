use super::{Image, Track};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::Artist;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum AlbumType {
    ALBUM = 1,
    SINGLE = 2,
    COMPILATION = 3,
    EP = 4,
    AUDIOBOOK = 5,
    PODCAST = 6,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Disc {
    pub number: i32,
    pub name: String,
    pub tracks: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub album_type: AlbumType,

    pub artists_ids: Vec<String>,
    pub label: String,
    pub date: i64,
    pub genres: Vec<String>,

    pub covers: Vec<Image>,
    pub discs: Vec<Disc>,
    pub related: Vec<String>,
    pub cover_group: Vec<Image>,
    pub original_title: String,
    pub version_title: String,
    pub type_str: String,
}

#[derive(Serialize)]
pub struct ResolvedAlbum {
    pub album: Album,
    pub tracks: HashMap<String, Track>,
    pub artists: HashMap<String, Artist>,
}