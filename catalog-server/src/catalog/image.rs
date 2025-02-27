use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ImageSize {
    DEFAULT,
    SMALL,
    LARGE,
    XLARGE,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Image {
    pub id: String,
    pub size: ImageSize,
    pub width: u16,
    pub height: u16,
}