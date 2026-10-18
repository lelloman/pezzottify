//! Shared ownership of published media, vault adapters and dependent-copy validity.
//! See docs/media-vaults.md for contracts and migration behavior.

pub mod adapters;
pub mod cache;
mod copies;
pub mod filesystem_cache;
pub(crate) mod local;
mod mutations;
mod track_materializer;
pub mod vault;
pub(crate) use mutations::{mutation_lock, protect_album};
pub use mutations::{CopyReceipt, Provenance, StagedMedia};
mod availability;
pub use availability::{directory_size, probe, MediaCatalogView, MediaPresence};

use crate::catalog_store::{CatalogStore, Track};
use crate::db_executor::{DbExecutor, DbHandle, DbLane, DbPriority, DbRunError};
use crate::downloader::DownloadPriority;
use crate::server::filesystem_work::{FilesystemWorkError, FilesystemWorkPool};
use bytes::Bytes;
use futures::Stream;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio_util::io::ReaderStream;
use tracing::{debug, error, warn};

use track_materializer::{InFlightTrack, TrackMaterializer};
pub(crate) use track_materializer::{ProxyMaterializerStatus, TrackStreamMetadata};

const STREAM_BUFFER_SIZE: usize = 64 * 1024;
const CACHE_FILL_CONCURRENCY: usize = 2;
const CACHE_FILL_QUEUE_TIMEOUT: Duration = Duration::from_millis(250);
const CACHE_FILL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(600);
pub type MediaStream = Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>;

/// Both foreground HTTP consumers and background jobs share this service.
/// The optional remote backend is attached once during server initialization.
pub struct MediaManager {
    catalog: Arc<dyn CatalogStore>,
    catalog_read: DbHandle<dyn CatalogStore>,
    filesystem: FilesystemWorkPool,
    image_reads: FilesystemWorkPool,
    cache_fills: FilesystemWorkPool,
    local_adapter: adapters::FilesystemAdapter,
    image_adapter: Arc<dyn vault::VaultAdapter>,
    vaults: std::sync::RwLock<std::collections::HashMap<vault::VaultId, copies::RegisteredVault>>,
    materializer: OnceLock<Arc<TrackMaterializer>>,
    cache: OnceLock<Arc<dyn cache::MediaCache>>,
    root: PathBuf,
    mutations: Arc<std::sync::Mutex<()>>,
    effects: OnceLock<mutations::Effects>,
    recovery_cursor: std::sync::Mutex<Option<String>>,
    cache_cleanup_cursor: std::sync::Mutex<Option<String>>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum MediaReadError {
    #[error("media not found")]
    NotFound,
    #[error("invalid local image")]
    InvalidLocalImage,
    #[error("local media read failed: {0}")]
    Storage(#[from] io::Error),
    #[error("upstream media read failed")]
    Upstream,
    #[error(transparent)]
    Database(#[from] DbRunError),
    #[error(transparent)]
    Filesystem(#[from] FilesystemWorkError),
}

pub(crate) struct ImageRead {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// Holds the validated descriptor, so consumers cannot reopen an unchecked path.
pub(crate) struct LocalAudio {
    file: std::fs::File,
    filename: String,
    content_type: String,
}

struct AudioFillPlan {
    key: cache::CacheKey,
    epoch: u64,
    filename: String,
    content_type: String,
    content_length: u64,
    uri: String,
}

fn audio_fill_plan(
    cache: &dyn cache::MediaCache,
    id: &str,
    uri: &str,
    file: &std::fs::File,
    path: &Path,
) -> Option<AudioFillPlan> {
    let key = cache::CacheKey::audio(id, uri)?;
    let epoch = cache.invalidation_epoch(&key.media);
    let content_length = file.metadata().ok()?.len();
    if content_length == 0 {
        return None;
    }
    Some(AudioFillPlan {
        key,
        epoch,
        filename: path.file_name()?.to_string_lossy().into_owned(),
        content_type: audio_content_type(path).to_owned(),
        content_length,
        uri: uri.to_owned(),
    })
}

impl LocalAudio {
    fn new(file: std::fs::File, path: PathBuf) -> Self {
        Self {
            content_type: audio_content_type(&path).to_owned(),
            filename: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            file,
        }
    }

    fn from_cache(entry: cache::CacheEntry) -> Self {
        Self {
            file: entry.file,
            filename: entry.filename,
            content_type: entry.content_type,
        }
    }

    pub async fn metadata(&self) -> io::Result<TrackStreamMetadata> {
        let file = tokio::fs::File::from_std(self.file.try_clone()?);
        Ok(TrackStreamMetadata {
            content_length: file.metadata().await?.len(),
            content_type: self.content_type.clone(),
        })
    }

    pub async fn range_stream(self, start: u64, length: u64) -> io::Result<MediaStream> {
        let mut file = tokio::fs::File::from_std(self.file);
        if start != 0 {
            file.seek(SeekFrom::Start(start)).await?;
        }
        let reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, file).take(length);
        Ok(Box::pin(ReaderStream::with_capacity(
            reader,
            STREAM_BUFFER_SIZE,
        )))
    }

    pub fn into_reader(self) -> (std::fs::File, String) {
        (self.file, self.filename)
    }
}

/// Progressive reader keeps the existing in-flight download and memory lease alive.
pub(crate) struct RemoteAudio(Arc<InFlightTrack>);
impl RemoteAudio {
    pub async fn metadata(&self) -> anyhow::Result<TrackStreamMetadata> {
        self.0.metadata().await
    }
    pub fn range_stream(self, start: u64, length: u64) -> MediaStream {
        self.0.range_stream(start, length)
    }
}

impl MediaManager {
    pub fn new(catalog: Arc<dyn CatalogStore>, executor: DbExecutor) -> Self {
        let root = catalog.media_root();
        let image_adapter: Arc<dyn vault::VaultAdapter> =
            Arc::new(adapters::HttpImageAdapter::default());
        let vaults = std::sync::RwLock::new(copies::initial_vaults(&root, image_adapter.clone()));
        Self {
            mutations: mutations::mutation_lock(&root),
            local_adapter: adapters::FilesystemAdapter::new(root.clone()),
            root,
            effects: OnceLock::new(),
            recovery_cursor: std::sync::Mutex::new(None),
            cache_cleanup_cursor: std::sync::Mutex::new(None),
            catalog_read: DbHandle::new(catalog.clone(), executor, DbLane::CatalogRead),
            catalog,
            filesystem: FilesystemWorkPool::default(),
            image_reads: FilesystemWorkPool::image_reads(),
            cache_fills: FilesystemWorkPool::with_limits(
                CACHE_FILL_CONCURRENCY,
                CACHE_FILL_QUEUE_TIMEOUT,
                CACHE_FILL_EXECUTION_TIMEOUT,
            ),
            image_adapter,
            vaults,
            materializer: OnceLock::new(),
            cache: OnceLock::new(),
        }
    }

    pub(crate) fn filesystem_work(&self) -> FilesystemWorkPool {
        self.filesystem.clone()
    }

    /// Install the optional read cache. Without one every lookup keeps using
    /// authoritative storage exactly as before.
    pub fn configure_cache(&self, cache: Arc<dyn cache::MediaCache>) {
        let _ = self.cache.set(cache);
    }

    /// Validate and install ordered filesystem caches from startup configuration.
    pub fn configure_filesystem_caches(
        &self,
        settings: &[crate::config::MediaCacheDirectorySettings],
    ) -> anyhow::Result<()> {
        let settings = settings
            .iter()
            .map(|setting| filesystem_cache::FilesystemCacheSettings {
                name: setting.name.clone(),
                path: setting.path.clone(),
                max_bytes: setting.max_bytes,
            })
            .collect();
        let caches = filesystem_cache::FilesystemCaches::new(settings, &self.root)?;
        debug!(caches = ?caches.names(), "Installed media caches");
        self.configure_cache(Arc::new(caches));
        Ok(())
    }

    /// Compatibility lookup preserves missing-track and local-open fallback behavior.
    pub(crate) async fn lookup_audio(
        &self,
        track_id: &str,
    ) -> Result<Option<(Track, Option<LocalAudio>)>, DbRunError> {
        let id = track_id.to_owned();
        let Some(track) = self
            .catalog_read
            .run(DbPriority::Interactive, move |catalog| {
                catalog.get_track_unprobed(&id)
            })
            .await?
        else {
            return Ok(None);
        };
        let adapter = self.local_adapter.clone();
        let cache = self.cache.get().cloned();
        let id = track_id.to_owned();
        let uri = track.audio_uri.clone();
        let opened = self
            .filesystem
            .run(move || {
                if let (Some(cache), Some(uri)) = (cache.as_deref(), uri.as_deref()) {
                    if let Some(cached) = cached_audio(cache, &id, uri) {
                        return Some((cached, None));
                    }
                }
                let uri = uri?;
                let (file, path) = adapter.open_file(&uri).ok()?;
                let fill = cache
                    .as_deref()
                    .and_then(|cache| audio_fill_plan(cache, &id, &uri, &file, &path));
                Some((LocalAudio::new(file, path), fill))
            })
            .await
            .map_err(|error| DbRunError::Store(anyhow::anyhow!(error)))?;
        let Some((audio, fill)) = opened else {
            return Ok(Some((track, None)));
        };
        if let Some(fill) = fill {
            self.schedule_audio_fill(fill);
        }
        Ok(Some((track, Some(audio))))
    }

    pub(crate) fn open_local_audio_blocking(
        &self,
        track_id: &str,
    ) -> Result<Option<LocalAudio>, DbRunError> {
        let id = track_id.to_owned();
        let track = self
            .catalog_read
            .run_blocking(DbPriority::Background, move |catalog| {
                catalog.get_track_unprobed(&id)
            })?;
        let Some(uri) = track.and_then(|track| track.audio_uri) else {
            return Ok(None);
        };
        if let Some(cache) = self.cache.get() {
            if let Some(cached) = cached_audio(cache.as_ref(), track_id, &uri) {
                return Ok(Some(cached));
            }
        }
        let (file, path) = self
            .local_adapter
            .open_file(&uri)
            .map_err(DbRunError::Store)?;
        if let Some(cache) = self.cache.get() {
            if let Some(fill) = audio_fill_plan(cache.as_ref(), track_id, &uri, &file, &path) {
                self.schedule_audio_fill(fill);
            }
        }
        Ok(Some(LocalAudio::new(file, path)))
    }

    fn schedule_audio_fill(&self, plan: AudioFillPlan) {
        let adapter = self.local_adapter.clone();
        let uri = plan.uri;
        let source = cache::CacheSource::Open(Box::new(move || {
            adapter
                .open_file(&uri)
                .map(|(file, _)| Box::new(file) as Box<dyn Read + Send>)
                .map_err(io::Error::other)
        }));
        self.schedule_cache_fill(cache::CacheFillRequest {
            key: plan.key,
            epoch: plan.epoch,
            filename: plan.filename,
            content_type: plan.content_type,
            content_length: plan.content_length,
            source,
        });
    }

    fn schedule_cache_fill(&self, request: cache::CacheFillRequest) {
        let Some(cache) = self.cache.get().cloned() else {
            return;
        };
        let key = request.key.clone();
        let pool = self.cache_fills.clone();
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            debug!(?key, "No runtime available for media cache fill");
            return;
        };
        handle.spawn(async move {
            match pool.run(move || cache.populate(request)).await {
                Ok(Ok(())) => debug!(?key, "Media cache fill finished"),
                Ok(Err(error)) => warn!(?key, %error, "Media cache fill failed"),
                Err(error) => debug!(?key, %error, "Media cache fill skipped"),
            }
        });
    }

    fn schedule_image_fill(
        &self,
        id: &str,
        source_url: Option<&str>,
        epoch: u64,
        bytes: &[u8],
        content_type: &str,
    ) {
        let (Some(source_url), Some(_), false) = (source_url, self.cache.get(), bytes.is_empty())
        else {
            return;
        };
        let key = cache::CacheKey::image(id, source_url, mutations::process_epoch());
        let extension = infer::get(bytes)
            .map(|kind| kind.extension())
            .unwrap_or("img");
        self.schedule_cache_fill(cache::CacheFillRequest {
            key,
            epoch,
            filename: format!("{id}.{extension}"),
            content_type: content_type.to_owned(),
            content_length: bytes.len() as u64,
            source: cache::CacheSource::Bytes(Arc::from(bytes)),
        });
    }

    /// Invalidation is ordered before an authoritative switch, never after it.
    pub(super) fn invalidate_cache(&self, media: &vault::MediaKey) {
        if let Some(cache) = self.cache.get() {
            cache.invalidate(media);
        }
    }

    pub async fn stage(
        self: &Arc<Self>,
        id: String,
        extension: String,
        provenance: Provenance,
    ) -> anyhow::Result<StagedMedia> {
        let manager = self.clone();
        self.filesystem
            .run(move || manager.begin_publication(&id, &extension, provenance))
            .await?
    }
    pub async fn commit(self: &Arc<Self>, stage: StagedMedia) -> anyhow::Result<CopyReceipt> {
        let manager = self.clone();
        self.filesystem
            .run(move || manager.commit_publication(stage))
            .await?
    }
    pub async fn publish_file(
        self: &Arc<Self>,
        id: String,
        extension: String,
        input: PathBuf,
        provenance: Provenance,
    ) -> anyhow::Result<CopyReceipt> {
        let manager = self.clone();
        self.filesystem
            .run(move || {
                let stage = manager.begin_publication(&id, &extension, provenance)?;
                std::fs::copy(input, stage.path())?;
                manager.commit_publication(stage)
            })
            .await?
    }

    /// Caller must have checked proxy permission AND the user's proxy preference.
    /// Remote reads are explicit, so a local-only consumer cannot trigger fallback.
    pub(crate) fn open_remote_audio(
        &self,
        track_id: &str,
        priority: DownloadPriority,
    ) -> Option<RemoteAudio> {
        self.materializer
            .get()
            .map(|backend| RemoteAudio(backend.get_or_start(track_id, priority)))
    }

    pub(crate) fn proxy_enabled(&self) -> bool {
        self.materializer.get().is_some()
    }
    pub(crate) fn proxy_status(&self) -> Option<ProxyMaterializerStatus> {
        self.materializer.get().map(|backend| backend.status())
    }

    pub(crate) fn enable_proxy(
        self: &Arc<Self>,
        downloader: Arc<dyn crate::downloader::Downloader>,
        search: Arc<dyn crate::search::SearchVault>,
        server: Arc<dyn crate::server_store::ServerStore>,
        _media_path: PathBuf,
        settings: crate::config::ProxyModeSettings,
    ) {
        self.configure_effects(search, server);
        let adapter = Arc::new(adapters::ProxyAudioAdapter::new(downloader));
        self.register_vault(
            vault::Vault {
                id: vault::VaultId(vault::AUDIO_ORIGIN.into()),
                role: vault::VaultRole::Authoritative,
            },
            adapter.clone(),
        )
        .expect("proxy vault initialized twice");
        let backend = TrackMaterializer::new(
            adapter,
            self.catalog.clone(),
            settings,
            Arc::downgrade(self),
        );
        assert!(
            self.materializer.set(backend).is_ok(),
            "media proxy initialized twice"
        );
    }

    pub(crate) async fn read_image(
        self: &Arc<Self>,
        id: &str,
    ) -> Result<ImageRead, MediaReadError> {
        // Cache identity needs only catalog metadata, never the media disk. A
        // catalog hiccup must not break a local read, so it only skips the cache.
        // The invalidation epoch is captured before the authoritative read so a
        // concurrent replacement cannot make this read populate a stale entry.
        let (cached_source, cached_epoch) = if let Some(cache) = self.cache.get().cloned() {
            let image_id = id.to_owned();
            match self
                .catalog_read
                .run(DbPriority::Interactive, move |catalog_store| {
                    catalog_store.get_item_image_url(&image_id)
                })
                .await
            {
                Ok(url) => {
                    let epoch = cache.invalidation_epoch(&vault::MediaKey {
                        kind: vault::MediaKind::Image,
                        id: id.to_owned(),
                    });
                    (url.map(|url| url.url), epoch)
                }
                Err(error) => {
                    warn!(%error, image_id = id, "Skipping image cache lookup");
                    (None, 0)
                }
            }
        } else {
            (None, 0)
        };
        if let (Some(cache), Some(source)) = (self.cache.get().cloned(), cached_source.as_deref()) {
            let key = cache::CacheKey::image(id, source, mutations::process_epoch());
            let lookup = self.image_reads.run(move || cache.lookup(&key)).await;
            match lookup {
                Ok(Ok(Some(entry))) => match read_cache_bytes(entry) {
                    Ok(bytes) => match image_bytes(bytes, MediaReadError::InvalidLocalImage) {
                        Ok(image) => return Ok(image),
                        Err(_) => warn!(
                            image_id = id,
                            "Invalid image cache entry; using authoritative storage"
                        ),
                    },
                    Err(error) => warn!(
                        %error,
                        image_id = id,
                        "Image cache read failed; using authoritative storage"
                    ),
                },
                Ok(Ok(None)) => {}
                Ok(Err(error)) => warn!(
                    %error,
                    image_id = id,
                    "Image cache lookup failed; using authoritative storage"
                ),
                Err(error) => warn!(
                    %error,
                    image_id = id,
                    "Image cache work failed; using authoritative storage"
                ),
            }
        }

        let manager = self.clone();
        let image_id = id.to_owned();
        // Resolve the pointer and read under one admission. Both can block on a
        // cold disk; neither should wait behind cache publication or recovery.
        match self
            .image_reads
            .run(move || -> Result<_, MediaReadError> {
                let file_path = manager
                    .image_path(&image_id)
                    .map_err(|error| MediaReadError::Storage(io::Error::other(error)))?;
                let locator = file_path
                    .strip_prefix(&manager.root)
                    .map_err(|error| MediaReadError::Storage(io::Error::other(error)))?
                    .to_string_lossy()
                    .into_owned();
                Ok(manager.local_adapter.read_bytes(&locator))
            })
            .await??
        {
            Ok(buffer) => {
                let image = image_bytes(buffer, MediaReadError::InvalidLocalImage)?;
                self.schedule_image_fill(
                    id,
                    cached_source.as_deref(),
                    cached_epoch,
                    &image.bytes,
                    &image.content_type,
                );
                return Ok(image);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                error!(%error, image_id = id, "Failed to read cached image");
                return Err(MediaReadError::Storage(error));
            }
        }

        // Image not cached locally - try to fetch from external URL
        let image_id = id.to_owned();
        let image_url = match self
            .catalog_read
            .run(DbPriority::Interactive, move |catalog_store| {
                catalog_store.get_item_image_url(&image_id)
            })
            .await
        {
            Ok(Some(url)) => url,
            Ok(None) => {
                debug!("No image URL found for item: {}", id);
                return Err(MediaReadError::NotFound);
            }
            Err(err) => {
                error!("Failed to query image URL for {}: {}", id, err);
                return Err(err.into());
            }
        };

        let response = self
            .image_adapter
            .read(vault::ReadRequest {
                locator: image_url.url.clone(),
                range: None,
                priority: DownloadPriority::Foreground,
            })
            .await
            .map_err(|_| MediaReadError::Upstream)?;
        use futures::StreamExt;
        let mut stream = response.stream;
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.map_err(|_| MediaReadError::Upstream)?);
        }

        // Verify it's actually an image
        let mime_type = match infer::get(&bytes) {
            Some(kind) if kind.mime_type().starts_with("image/") => kind.mime_type().to_string(),
            _ => {
                error!("Downloaded content is not an image: {}", image_url.url);
                return Err(MediaReadError::Upstream);
            }
        };

        // Save the image atomically for future requests. Cache failure does not fail
        // this response because the validated bytes are already available.
        let manager = self.clone();
        let image_id = id.to_owned();
        let cached_bytes = bytes.to_vec();
        let source_url = image_url.url.clone();
        let cached = self
            .filesystem
            .run(move || -> anyhow::Result<()> {
                let mut stage =
                    manager.begin_publication(&image_id, "jpg", Provenance::ImageCache)?;
                stage.record.source_locator = Some(source_url);
                std::fs::write(stage.path(), cached_bytes)?;
                manager.commit_publication(stage)?;
                Ok(())
            })
            .await;
        if !matches!(cached, Ok(Ok(()))) {
            warn!("Failed to persist image for {id}");
        }

        Ok(ImageRead {
            bytes: bytes.to_vec(),
            content_type: mime_type,
        })
    }
}

fn read_cache_bytes(entry: cache::CacheEntry) -> io::Result<Vec<u8>> {
    let mut file = entry.file;
    let mut bytes = Vec::with_capacity(entry.content_length.min(8 * 1024 * 1024) as usize);
    std::io::Read::read_to_end(&mut file, &mut bytes)?;
    Ok(bytes)
}

fn cached_audio(cache: &dyn cache::MediaCache, id: &str, uri: &str) -> Option<LocalAudio> {
    let key = cache::CacheKey::audio(id, uri)?;
    match cache.lookup(&key) {
        Ok(Some(entry)) => Some(LocalAudio::from_cache(entry)),
        Ok(None) => None,
        Err(error) => {
            warn!(track_id = id, %error, "Media cache lookup failed; using authoritative storage");
            None
        }
    }
}

fn image_bytes(bytes: Vec<u8>, invalid: MediaReadError) -> Result<ImageRead, MediaReadError> {
    let content_type = infer::get(&bytes)
        .filter(|kind| kind.mime_type().starts_with("image/"))
        .ok_or(invalid)?
        .mime_type()
        .to_owned();
    Ok(ImageRead {
        bytes,
        content_type,
    })
}

pub(crate) fn audio_content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("aac") => "audio/aac",
        Some("flac") => "audio/flac",
        Some("m4a" | "mp4") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("oga" | "ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod cache_tests;

#[cfg(test)]
mod filesystem_cache_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mutation_tests;

#[cfg(test)]
mod vault_tests;
