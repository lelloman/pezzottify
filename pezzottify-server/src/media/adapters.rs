//! Initial adapters. Local paths are confined to this backend and staging leases.
use super::{local, vault::*, LocalAudio, MediaStream};
use async_trait::async_trait;
use futures::StreamExt;
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::AsyncWriteExt;

fn storage(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Storage(error.to_string())
}
fn local_error(error: anyhow::Error) -> AdapterError {
    if error
        .downcast_ref::<io::Error>()
        .is_some_and(|e| e.kind() == io::ErrorKind::NotFound)
    {
        AdapterError::Missing
    } else {
        storage(error)
    }
}
#[derive(Clone)]
pub struct FilesystemAdapter {
    root: PathBuf,
}
impl FilesystemAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn reachable_root(&self) -> AdapterResult<()> {
        match std::fs::metadata(&self.root) {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => Err(AdapterError::Unreachable(
                "media root is not a directory".into(),
            )),
            Err(error) => Err(AdapterError::Unreachable(error.to_string())),
        }
    }
    pub(crate) fn open_file(&self, locator: &str) -> anyhow::Result<(std::fs::File, PathBuf)> {
        local::open_media_file_beneath(&self.root, locator)
    }
    pub(crate) fn read_bytes(&self, locator: &str) -> io::Result<Vec<u8>> {
        let (mut file, _) =
            self.open_file(locator)
                .map_err(|error| match error.downcast::<io::Error>() {
                    Ok(error) => error,
                    Err(error) => io::Error::other(error),
                })?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes)?;
        Ok(bytes)
    }
    pub(crate) fn expose(&self, staging: &str, locator: &str) -> anyhow::Result<()> {
        self.open_file(staging)?;
        local::normalized_media_identifier(locator)?;
        // hard_link creates the immutable destination without ever replacing another
        // generation. The staging link is removed after directory entries are synced.
        let destination = self.root.join(locator);
        super::mutations::prepare_directory(
            &self.root,
            Path::new(locator).parent().unwrap().to_str().unwrap(),
        )?;
        std::fs::hard_link(self.root.join(staging), &destination)?;
        super::mutations::sync_directory(destination.parent().unwrap())?;
        super::mutations::unlink(&self.root.join(staging))?;
        Ok(())
    }
    pub(crate) fn remove_file(&self, locator: &str) -> anyhow::Result<()> {
        local::normalized_media_identifier(locator)?;
        match self.open_file(locator) {
            Ok(_) => super::mutations::unlink(&self.root.join(locator)),
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
#[async_trait]
impl VaultAdapter for FilesystemAdapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            ranges: true,
            publish: true,
            delete: true,
            presence: true,
        }
    }
    async fn read(&self, request: ReadRequest) -> AdapterResult<AdapterRead> {
        let adapter = self.clone();
        let locator = request.locator;
        let (file, path) = tokio::task::spawn_blocking(move || {
            adapter.reachable_root()?;
            adapter.open_file(&locator).map_err(local_error)
        })
        .await
        .map_err(storage)??;
        let audio = LocalAudio::new(file, path.clone());
        let metadata = audio.metadata().await.map_err(storage)?;
        let range = request.range.unwrap_or(ByteRange {
            start: 0,
            length: metadata.content_length,
        });
        if range.start > metadata.content_length
            || range.length > metadata.content_length - range.start
        {
            return Err(storage("range exceeds object length"));
        }
        Ok(AdapterRead {
            metadata: ReadMetadata {
                content_length: range.length,
                content_type: metadata.content_type,
                extension: path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned(),
            },
            stream: audio
                .range_stream(range.start, range.length)
                .await
                .map_err(storage)?,
        })
    }
    async fn publish(&self, locator: &str, mut stream: MediaStream) -> AdapterResult<()> {
        local::normalized_media_identifier(locator).map_err(storage)?;
        let adapter = self.clone();
        tokio::task::spawn_blocking(move || {
            super::mutations::prepare_directory(&adapter.root, ".media/staging")
        })
        .await
        .map_err(storage)?
        .map_err(storage)?;
        let staging = format!(".media/staging/{}.upload", uuid::Uuid::new_v4());
        let path = self.root.join(&staging);
        // The owned guard also cleans up if the async publication is cancelled.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(storage)?;
        let mut size = 0;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(storage)?;
            file.write_all(&bytes).await.map_err(storage)?;
            size += bytes.len();
        }
        if size == 0 {
            return Err(storage("empty publication"));
        }
        file.sync_all().await.map_err(storage)?;
        drop(file);
        let adapter = self.clone();
        let locator = locator.to_owned();
        tokio::task::spawn_blocking(move || adapter.expose(&staging, &locator))
            .await
            .map_err(storage)?
            .map_err(storage)
    }
    async fn delete(&self, locator: &str) -> AdapterResult<()> {
        let adapter = self.clone();
        let locator = locator.to_owned();
        tokio::task::spawn_blocking(move || adapter.remove_file(&locator))
            .await
            .map_err(storage)?
            .map_err(local_error)
    }
    async fn presence(&self, locator: &str) -> Presence {
        let adapter = self.clone();
        let locator = locator.to_owned();
        match tokio::task::spawn_blocking(move || {
            adapter.reachable_root()?;
            adapter.open_file(&locator).map_err(local_error)
        })
        .await
        {
            Ok(Ok(_)) => Presence::Present,
            Ok(Err(e)) => match e {
                AdapterError::Missing => Presence::Missing,
                AdapterError::Unreachable(error) => Presence::Unreachable(error),
                e => Presence::Unknown(e.to_string()),
            },
            Err(e) => Presence::Unknown(e.to_string()),
        }
    }
}

pub struct HttpImageAdapter {
    client: reqwest::Client,
}
impl Default for HttpImageAdapter {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("image HTTP client"),
        }
    }
}
#[async_trait]
impl VaultAdapter for HttpImageAdapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            ..Default::default()
        }
    }
    async fn read(&self, request: ReadRequest) -> AdapterResult<AdapterRead> {
        if request.range.is_some() {
            return Err(AdapterError::Unsupported("range read"));
        }
        let response = self
            .client
            .get(&request.locator)
            .send()
            .await
            .map_err(|e| AdapterError::Unreachable(e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::Missing);
        }
        if !response.status().is_success() {
            return Err(storage(format!("HTTP {}", response.status())));
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = response.bytes().await.map_err(storage)?;
        Ok(AdapterRead {
            metadata: ReadMetadata {
                content_length: bytes.len() as u64,
                content_type,
                extension: String::new(),
            },
            stream: Box::pin(futures::stream::once(async { Ok(bytes) })),
        })
    }
}

pub struct ProxyAudioAdapter {
    downloader: Arc<dyn crate::downloader::Downloader>,
}
impl ProxyAudioAdapter {
    pub fn new(downloader: Arc<dyn crate::downloader::Downloader>) -> Self {
        Self { downloader }
    }
}
#[async_trait]
impl VaultAdapter for ProxyAudioAdapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            ..Default::default()
        }
    }
    async fn read(&self, request: ReadRequest) -> AdapterResult<AdapterRead> {
        if request.range.is_some() {
            return Err(AdapterError::Unsupported("range read"));
        }
        let download = self
            .downloader
            .open_track_audio(&request.locator, request.priority)
            .await
            .map_err(|error| match error.downcast_ref::<reqwest::Error>() {
                Some(e) if e.status() == Some(reqwest::StatusCode::NOT_FOUND) => {
                    AdapterError::Missing
                }
                Some(e) if e.is_connect() || e.is_timeout() => {
                    AdapterError::Unreachable(error.to_string())
                }
                _ => storage(error),
            })?;
        Ok(AdapterRead {
            metadata: ReadMetadata {
                content_length: download.content_length,
                content_type: download.content_type,
                extension: download.extension,
            },
            stream: Box::pin(download.stream.map(|chunk| chunk.map_err(io::Error::other))),
        })
    }
}
