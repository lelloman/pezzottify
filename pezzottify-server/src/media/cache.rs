//! Read interception contract for optional media caches.
//!
//! See docs/media-cache.md. Identity and validity live here; the filesystem
//! implementation and its population policy live in `filesystem_cache.rs`.

use super::vault::{AdapterResult, MediaKey, MediaKind};
#[cfg(test)]
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

/// The authority token a cached copy was derived from.
///
/// Audio uses its catalog locator. Managed locators embed a durable publication
/// revision; other locators are scoped to this process. Images use their source URL.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheVersion(pub String);

/// How long an entry may be trusted without consulting authoritative storage.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CacheGeneration {
    /// The version token is durable and unique per authoritative content.
    Durable,
    /// The version token is only meaningful inside one server process. Entries
    /// from another process are ignored and fall back to the original.
    Process(String),
}

/// Identity of one complete immutable cache object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub media: MediaKey,
    pub version: CacheVersion,
    pub generation: CacheGeneration,
}

impl CacheKey {
    /// Every nonempty audio locator is cacheable. Without a durable publication
    /// revision, entries live for this process and are discarded on restart.
    /// Publication/deletion invalidation protects both kinds during this process.
    pub fn audio(id: &str, uri: &str) -> Option<Self> {
        if uri.is_empty() {
            return None;
        }
        let (version, generation) = match managed_audio_revision(id, uri) {
            Some(revision) => (format!("{uri}#{revision}"), CacheGeneration::Durable),
            None => (
                uri.to_owned(),
                CacheGeneration::Process(super::mutations::process_epoch().to_owned()),
            ),
        };
        Some(Self {
            media: MediaKey {
                kind: MediaKind::Audio,
                id: id.to_owned(),
            },
            version: CacheVersion(version),
            generation,
        })
    }

    /// Images are cached per catalog identity and current source URL.
    pub fn image(id: &str, source_url: &str, process: &str) -> Self {
        Self {
            media: MediaKey {
                kind: MediaKind::Image,
                id: id.to_owned(),
            },
            version: CacheVersion(source_url.to_owned()),
            generation: CacheGeneration::Process(process.to_owned()),
        }
    }
}

fn managed_audio_revision(id: &str, uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("audio/.managed/")?;
    let rest = rest.strip_prefix(id)?.strip_prefix('.')?;
    let (revision, extension) = rest.split_once('.')?;
    if extension.is_empty()
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || uuid::Uuid::parse_str(revision).is_err()
    {
        return None;
    }
    Some(revision.to_owned())
}

/// One open cache object. The manager owns the descriptor and never reopens a path.
pub struct CacheEntry {
    pub file: std::fs::File,
    pub content_length: u64,
    pub filename: String,
    pub content_type: String,
}

/// Bytes for a cache fill, either already validated in memory or reopened from
/// authoritative storage only after the fill is admitted.
pub enum CacheSource {
    Bytes(Arc<[u8]>),
    Open(Box<dyn FnOnce() -> std::io::Result<Box<dyn std::io::Read + Send>> + Send>),
}

/// A memory-only allocation snapshot for operator visibility.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CacheSnapshot {
    pub name: String,
    pub path: std::path::PathBuf,
    pub max_bytes: u64,
    pub allocated_bytes: u64,
    pub reserved_bytes: u64,
    pub lru: bool,
}

impl CacheSource {
    pub fn open(self) -> std::io::Result<Box<dyn std::io::Read + Send>> {
        match self {
            Self::Bytes(bytes) => Ok(Box::new(std::io::Cursor::new(bytes))),
            Self::Open(open) => open(),
        }
    }
}

/// One best-effort population attempt. `content_length` is the length observed
/// on the authoritative read that produced this request.
pub struct CacheFillRequest {
    pub key: CacheKey,
    /// Invalidation epoch observed before the authoritative read. A fill whose
    /// media was invalidated after this point refuses to publish.
    pub epoch: u64,
    pub filename: String,
    pub content_type: String,
    pub content_length: u64,
    pub source: CacheSource,
}

/// Optional read-through cache supplied by server configuration.
///
/// Implementations must:
/// - keep lookup metadata (key, size, type, location) on fast storage, so a
///   successful lookup never touches authoritative media storage;
/// - expose only complete, validated objects, and publish them atomically;
/// - never overwrite an object that a reader may still hold open;
/// - treat `invalidate` as durable enough that a later fill cannot resurrect an
///   entry for the invalidated media without a fresh authoritative read.
pub trait MediaCache: Send + Sync {
    /// Read accounting without inspecting cache files or original media.
    fn snapshots(&self) -> Vec<CacheSnapshot> {
        Vec::new()
    }

    fn lookup(&self, key: &CacheKey) -> AdapterResult<Option<CacheEntry>>;

    /// Drop every entry for this media. Called before an authoritative
    /// publication, replacement or deletion becomes visible.
    fn invalidate(&self, media: &MediaKey);

    /// Current invalidation epoch for a media identity, captured before an
    /// authoritative read so a fill cannot publish bytes read before a mutation.
    fn invalidation_epoch(&self, _media: &MediaKey) -> u64 {
        0
    }

    /// Best-effort population on a background filesystem worker. Implementations
    /// skip a fill that is already in flight or has no room; a failure never
    /// affects the read that produced it.
    fn populate(&self, _request: CacheFillRequest) -> AdapterResult<()> {
        Ok(())
    }
}

#[cfg(test)]
fn storage(error: impl std::fmt::Display) -> super::vault::AdapterError {
    super::vault::AdapterError::Storage(error.to_string())
}

/// Minimal in-memory index used by contract tests. It serves whole files, so the
/// manager owns range seeking. Real caches implement the same interface.
#[cfg(test)]
#[derive(Default)]
pub struct TestCache {
    entries: Mutex<HashMap<CacheKey, std::path::PathBuf>>,
    invalidated: Mutex<Vec<MediaKey>>,
    lookups: AtomicUsize,
}

#[cfg(test)]
impl TestCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, key: CacheKey, path: std::path::PathBuf) {
        self.entries.lock().unwrap().insert(key, path);
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    pub fn invalidated(&self) -> Vec<MediaKey> {
        self.invalidated.lock().unwrap().clone()
    }

    pub fn lookups(&self) -> usize {
        self.lookups.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
impl MediaCache for TestCache {
    fn lookup(&self, key: &CacheKey) -> AdapterResult<Option<CacheEntry>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        let Some(path) = self.entries.lock().unwrap().get(key).cloned() else {
            return Ok(None);
        };
        let file = std::fs::File::open(&path).map_err(storage)?;
        let content_length = file.metadata().map_err(storage)?.len();
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some(CacheEntry {
            file,
            content_length,
            filename,
            content_type: super::audio_content_type(&path).to_owned(),
        }))
    }

    fn invalidate(&self, media: &MediaKey) {
        self.invalidated.lock().unwrap().push(media.clone());
        self.entries
            .lock()
            .unwrap()
            .retain(|key, _| &key.media != media);
    }
}
