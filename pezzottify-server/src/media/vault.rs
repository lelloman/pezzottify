//! Location-independent identities and storage capabilities. Policy belongs to MediaManager.
use super::MediaStream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaKind {
    Audio,
    Image,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaKey {
    pub kind: MediaKind,
    pub id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentVersion(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepresentationId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CopyId(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaultRole {
    Authoritative,
    Cache,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    pub id: VaultId,
    pub role: VaultRole,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceReference {
    pub vault: VaultId,
    pub media: MediaKey,
    pub version: ContentVersion,
    pub locator: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaCopy {
    pub id: CopyId,
    pub media: MediaKey,
    pub version: ContentVersion,
    /// A digest of the actual representation bytes, independent of location/copy UUID.
    pub representation: RepresentationId,
    pub vault: VaultId,
    /// Opaque to the manager; interpreted only by the adapter.
    pub locator: String,
    pub source: Option<SourceReference>,
    pub protected: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub read: bool,
    pub ranges: bool,
    pub publish: bool,
    pub delete: bool,
    pub presence: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    Present,
    Missing,
    Unreachable(String),
    Unknown(String),
}
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("unsupported adapter operation: {0}")]
    Unsupported(&'static str),
    #[error("media is missing")]
    Missing,
    #[error("vault is unreachable: {0}")]
    Unreachable(String),
    #[error("media copy is stale")]
    Stale,
    #[error("storage error: {0}")]
    Storage(String),
}
pub type AdapterResult<T> = Result<T, AdapterError>;
#[derive(Clone, Copy, Debug)]
pub struct ByteRange {
    pub start: u64,
    pub length: u64,
}
pub struct ReadRequest {
    pub locator: String,
    pub range: Option<ByteRange>,
    pub priority: crate::downloader::DownloadPriority,
}
pub struct ReadMetadata {
    pub content_length: u64,
    pub content_type: String,
    pub extension: String,
}
pub struct AdapterRead {
    pub metadata: ReadMetadata,
    pub stream: MediaStream,
}

/// Implementations publish complete immutable objects or fail, and delete idempotently.
/// No operation requires a local path. Presence must not trigger materialization.
#[async_trait]
pub trait VaultAdapter: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    async fn read(&self, _request: ReadRequest) -> AdapterResult<AdapterRead> {
        Err(AdapterError::Unsupported("read"))
    }
    async fn publish(&self, _locator: &str, _stream: MediaStream) -> AdapterResult<()> {
        Err(AdapterError::Unsupported("publish"))
    }
    async fn delete(&self, _locator: &str) -> AdapterResult<()> {
        Err(AdapterError::Unsupported("delete"))
    }
    async fn presence(&self, _locator: &str) -> Presence {
        Presence::Unknown("presence is unsupported".into())
    }
}
pub const LOCAL_AUDIO: &str = "local-authoritative-audio";
pub const LOCAL_IMAGES: &str = "local-image-cache";
pub const IMAGE_ORIGIN: &str = "external-image-origin";
pub const AUDIO_ORIGIN: &str = "proxy-audio-source";
