//! Shared ownership of published media, vault adapters and dependent-copy validity.
//! See docs/media-vaults.md for contracts and migration behavior.

pub mod adapters;
mod copies;
pub(crate) mod local;
mod mutations;
mod track_materializer;
pub mod vault;
pub use mutations::{CopyReceipt, Provenance, StagedMedia};
pub(crate) use mutations::{mutation_lock, protect_album};
mod availability;
pub use availability::{directory_size, probe, MediaCatalogView, MediaPresence};

use crate::catalog_store::{CatalogStore, Track};
use crate::db_executor::{DbExecutor, DbHandle, DbLane, DbPriority, DbRunError};
use crate::downloader::DownloadPriority;
use crate::server::filesystem_work::{FilesystemWorkError, FilesystemWorkPool};
use bytes::Bytes;
use futures::Stream;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio_util::io::ReaderStream;
use tracing::{debug, error, warn};

use track_materializer::{InFlightTrack, TrackMaterializer};
pub(crate) use track_materializer::{ProxyMaterializerStatus, TrackStreamMetadata};

const STREAM_BUFFER_SIZE: usize = 64 * 1024;
pub type MediaStream = Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>;

/// Both foreground HTTP consumers and background jobs share this service.
/// The optional remote backend is attached once during server initialization.
pub struct MediaManager {
    catalog: Arc<dyn CatalogStore>,
    catalog_read: DbHandle<dyn CatalogStore>,
    filesystem: FilesystemWorkPool,
    local_adapter: adapters::FilesystemAdapter,
    image_adapter: Arc<dyn vault::VaultAdapter>,
    vaults: std::sync::RwLock<std::collections::HashMap<vault::VaultId, copies::RegisteredVault>>,
    materializer: OnceLock<Arc<TrackMaterializer>>,
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
    content_type: &'static str,
}

impl LocalAudio {
    fn new(file: std::fs::File, path: PathBuf) -> Self {
        Self {
            content_type: audio_content_type(&path),
            filename: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            file,
        }
    }

    pub async fn metadata(&self) -> io::Result<TrackStreamMetadata> {
        let file = tokio::fs::File::from_std(self.file.try_clone()?);
        Ok(TrackStreamMetadata {
            content_length: file.metadata().await?.len(),
            content_type: self.content_type.to_owned(),
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
            image_adapter,
            vaults,
            materializer: OnceLock::new(),
        }
    }

    pub(crate) fn filesystem_work(&self) -> FilesystemWorkPool {
        self.filesystem.clone()
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
                catalog.get_track(&id)
            })
            .await?
        else {
            return Ok(None);
        };
        let adapter = self.local_adapter.clone();
        let uri = track.audio_uri.clone();
        let opened = self
            .filesystem
            .run(move || {
                uri.and_then(|uri| adapter.open_file(&uri).ok())
                    .map(|(file, path)| LocalAudio::new(file, path))
            })
            .await
            .map_err(|error| DbRunError::Store(anyhow::anyhow!(error)))?;
        Ok(Some((track, opened)))
    }

    pub(crate) fn open_local_audio_blocking(
        &self,
        track_id: &str,
    ) -> Result<Option<LocalAudio>, DbRunError> {
        let id = track_id.to_owned();
        let track = self
            .catalog_read
            .run_blocking(DbPriority::Background, move |catalog| {
                catalog.get_track(&id)
            })?;
        track
            .and_then(|track| track.audio_uri)
            .map(|uri| {
                self.local_adapter
                    .open_file(&uri)
                    .map(|(file, path)| LocalAudio::new(file, path))
                    .map_err(DbRunError::Store)
            })
            .transpose()
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
        let manager = self.clone();
        let image_id = id.to_owned();
        let file_path = self
            .filesystem
            .run(move || manager.image_path(&image_id))
            .await?
            .map_err(|error| MediaReadError::Storage(io::Error::other(error)))?;

        // First, check if we have the image cached locally.
        let adapter = self.local_adapter.clone();
        let locator = file_path
            .strip_prefix(&self.root)
            .map_err(|error| MediaReadError::Storage(io::Error::other(error)))?
            .to_string_lossy()
            .into_owned();
        match self
            .filesystem
            .run(move || adapter.read_bytes(&locator))
            .await
        {
            Ok(Ok(buffer)) => return image_bytes(buffer, MediaReadError::InvalidLocalImage),
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(Err(error)) => {
                error!(%error, path = %file_path.display(), "Failed to read cached image");
                return Err(MediaReadError::Storage(error));
            }
            Err(error) => return Err(error.into()),
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
mod tests;

#[cfg(test)]
mod mutation_tests;

#[cfg(test)]
mod vault_tests;
