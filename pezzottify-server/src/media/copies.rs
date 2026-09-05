//! Copy descriptors and fail-closed dependent-copy access. No cache selection policy.
use super::{vault::*, CopyReceipt, MediaManager};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::Read, sync::Arc};

pub(super) struct RegisteredVault {
    pub vault: Vault,
    pub adapter: Arc<dyn VaultAdapter>,
}
fn storage(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Storage(error.to_string())
}

impl MediaManager {
    pub fn vaults(&self) -> Vec<Vault> {
        let mut vaults = self
            .vaults
            .read()
            .unwrap()
            .values()
            .map(|v| v.vault.clone())
            .collect::<Vec<_>>();
        vaults.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        vaults
    }
    /// Adapters supplied here are trusted configuration, not user-provided locators.
    /// A cache adapter must own a namespace disjoint from authoritative objects.
    pub fn register_vault(&self, vault: Vault, adapter: Arc<dyn VaultAdapter>) -> Result<()> {
        anyhow::ensure!(!vault.id.0.is_empty(), "empty vault identity");
        let mut vaults = self.vaults.write().unwrap();
        anyhow::ensure!(!vaults.contains_key(&vault.id), "duplicate vault identity");
        vaults.insert(vault.id.clone(), RegisteredVault { vault, adapter });
        Ok(())
    }
    pub fn vault_capabilities(&self, id: &VaultId) -> Option<Capabilities> {
        self.vaults
            .read()
            .unwrap()
            .get(id)
            .map(|v| v.adapter.capabilities())
    }
    pub(super) fn describe_publication(
        &self,
        record: &CopyReceipt,
        file: &std::fs::File,
    ) -> Result<MediaCopy> {
        // The staging producer is finished before representation hashing begins.
        let mut file = file.try_clone()?;
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let representation = RepresentationId(format!("sha256:{:x}", hash.finalize()));
        let image = record.provenance == super::Provenance::ImageCache;
        let media = MediaKey {
            kind: if image {
                MediaKind::Image
            } else {
                MediaKind::Audio
            },
            id: record.media_id.clone(),
        };
        let version = ContentVersion(uuid::Uuid::new_v4().to_string());
        let source = if image {
            record.source_locator.as_ref().map(|url| SourceReference {
                vault: VaultId(IMAGE_ORIGIN.into()),
                media: media.clone(),
                // The fetched representation establishes this observation, not the URL alone.
                version: ContentVersion(representation.0.clone()),
                locator: Some(url.clone()),
            })
        } else {
            None
        };
        Ok(MediaCopy {
            id: CopyId(record.revision.clone()),
            media,
            version,
            representation,
            vault: VaultId(if image { LOCAL_IMAGES } else { LOCAL_AUDIO }.into()),
            locator: record.uri.clone(),
            protected: !image || source.is_none(),
            source,
        })
    }
    /// Legacy copies remain readable in place. They cannot be used as a validated
    /// cache source until a publication has established an explicit content version.
    pub fn authoritative_audio(&self, id: &str) -> Result<Option<MediaCopy>> {
        let Some(track) = self.catalog.get_track(id)? else {
            return Ok(None);
        };
        let Some(uri) = track.audio_uri else {
            return Ok(None);
        };
        let revision = std::path::Path::new(&uri)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('.').next());
        let Some(revision) = revision.filter(|r| uuid::Uuid::parse_str(r).is_ok()) else {
            return Ok(None);
        };
        let record =
            match super::mutations::read_record(&super::mutations::copy_path(&self.root, revision))
            {
                Ok(record) => record,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                {
                    return Ok(None)
                }
                Err(error) => return Err(error),
            };
        Ok(record.copy.filter(|copy| {
            copy.media
                == (MediaKey {
                    kind: MediaKind::Audio,
                    id: id.to_owned(),
                })
                && copy.locator == uri
                && copy.vault.0 == LOCAL_AUDIO
        }))
    }
    fn cache_source_is_current(&self, copy: &MediaCopy) -> Result<bool> {
        let Some(source) = &copy.source else {
            return Ok(false);
        };
        if source.vault.0 != LOCAL_AUDIO
            || source.media.kind != MediaKind::Audio
            || source.media != copy.media
        {
            return Ok(false);
        }
        Ok(self
            .authoritative_audio(&source.media.id)?
            .is_some_and(|current| {
                current.version == source.version && current.version == copy.version
            }))
    }
    fn cache_record_path(&self, id: &CopyId) -> Result<std::path::PathBuf> {
        uuid::Uuid::parse_str(&id.0)?;
        Ok(self
            .root
            .join(".media/cache-index")
            .join(format!("{}.json", id.0)))
    }
    /// Registers an already complete immutable cache object. Actual read-through
    /// placement/selection is a later story. Old registrations stay invalid after restart.
    pub fn register_cache_copy(&self, copy: &MediaCopy) -> Result<()> {
        let _guard = self.mutations.lock().unwrap();
        let vaults = self.vaults.read().unwrap();
        let vault = vaults.get(&copy.vault).context("unknown cache vault")?;
        anyhow::ensure!(vault.vault.role == VaultRole::Cache, "not a cache vault");
        anyhow::ensure!(
            !copy.protected,
            "protected media cannot be registered for cache eviction"
        );
        // Built-in image cache publication is coordinated by its existing journal.
        anyhow::ensure!(
            copy.vault.0 != LOCAL_IMAGES,
            "image cache uses publication journal"
        );
        anyhow::ensure!(self.cache_source_is_current(copy)?, "stale cache source");
        let path = self.cache_record_path(&copy.id)?;
        anyhow::ensure!(
            !path.with_extension("deleted").exists(),
            "copy identity was already evicted"
        );
        if path.exists() {
            anyhow::ensure!(
                serde_json::from_slice::<MediaCopy>(&std::fs::read(path)?)? == *copy,
                "copy identity already registered"
            );
            return Ok(());
        }
        super::mutations::prepare_directory(&self.root, ".media/cache-index")?;
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&serde_json::to_vec(copy)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(temp, &path)?;
        super::mutations::sync_directory(path.parent().unwrap())
    }
    fn registered_copy(&self, id: &CopyId) -> Result<MediaCopy> {
        let copy: MediaCopy = serde_json::from_slice(&std::fs::read(self.cache_record_path(id)?)?)?;
        anyhow::ensure!(copy.id == *id, "cache identity mismatch");
        Ok(copy)
    }
    /// The authority must be revalidated on every new read, including after reconnect.
    /// A handle already opened may finish; invalidation prevents subsequent opens.
    pub async fn read_cache_copy(
        self: &Arc<Self>,
        id: CopyId,
        range: Option<ByteRange>,
    ) -> AdapterResult<AdapterRead> {
        let manager = self.clone();
        let copy = self
            .filesystem
            .run(move || -> AdapterResult<MediaCopy> {
                let copy = manager.registered_copy(&id).map_err(storage)?;
                if !manager.cache_source_is_current(&copy).map_err(storage)? {
                    return Err(AdapterError::Stale);
                }
                Ok(copy)
            })
            .await
            .map_err(storage)??;
        let adapter = self.cache_adapter(&copy)?;
        let capabilities = adapter.capabilities();
        if !capabilities.read {
            return Err(AdapterError::Unsupported("read"));
        }
        if range.is_some() && !capabilities.ranges {
            return Err(AdapterError::Unsupported("range read"));
        }
        let read = adapter
            .read(ReadRequest {
                locator: copy.locator.clone(),
                range,
                priority: crate::downloader::DownloadPriority::Foreground,
            })
            .await?;
        let manager = self.clone();
        let current = self
            .filesystem
            .run(move || manager.cache_source_is_current(&copy))
            .await
            .map_err(storage)?
            .map_err(storage)?;
        if !current {
            return Err(AdapterError::Stale);
        }
        Ok(read)
    }
    fn cache_adapter(&self, copy: &MediaCopy) -> AdapterResult<Arc<dyn VaultAdapter>> {
        let vaults = self.vaults.read().unwrap();
        let vault = vaults
            .get(&copy.vault)
            .ok_or_else(|| AdapterError::Unreachable("vault is not connected".into()))?;
        if vault.vault.role != VaultRole::Cache || copy.protected {
            return Err(storage("copy is protected"));
        }
        Ok(vault.adapter.clone())
    }
    pub async fn evict_cache_copy(self: &Arc<Self>, id: CopyId) -> AdapterResult<()> {
        let manager = self.clone();
        let lookup = id.clone();
        let copy = self
            .filesystem
            .run(move || manager.registered_copy(&lookup))
            .await
            .map_err(storage)?;
        let copy = match copy {
            Ok(copy) => copy,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(())
            }
            Err(error) => return Err(storage(error)),
        };
        let adapter = self.cache_adapter(&copy)?;
        if !adapter.capabilities().delete {
            return Err(AdapterError::Unsupported("delete"));
        }
        adapter.delete(&copy.locator).await?;
        let manager = self.clone();
        self.filesystem
            .run(move || -> Result<()> {
                let _guard = manager.mutations.lock().unwrap();
                let path = manager.cache_record_path(&id)?;
                if !path.exists() {
                    return Ok(());
                }
                std::fs::rename(&path, path.with_extension("deleted"))?;
                super::mutations::sync_directory(path.parent().unwrap())
            })
            .await
            .map_err(storage)?
            .map_err(storage)
    }
    /// Physical cleanup is retryable; validity never depends on successful deletion.
    pub async fn cleanup_invalidated_caches(self: &Arc<Self>) -> Result<usize> {
        let manager = self.clone();
        let stale = self
            .filesystem
            .run(move || -> Result<Vec<CopyId>> {
                let entries = match std::fs::read_dir(manager.root.join(".media/cache-index")) {
                    Ok(entries) => entries,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
                    Err(e) => return Err(e.into()),
                };
                let mut stale = Vec::new();
                let mut paths = entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<std::io::Result<Vec<_>>>()?;
                paths.retain(|p| p.extension().and_then(|s| s.to_str()) == Some("json"));
                paths.sort();
                let cursor = manager.cache_cleanup_cursor.lock().unwrap().clone();
                let start = cursor
                    .map(|cursor| {
                        paths.partition_point(|p| p.to_string_lossy().as_ref() <= cursor.as_str())
                    })
                    .unwrap_or(0);
                let start = if start == paths.len() { 0 } else { start };
                for path in paths.into_iter().skip(start).take(1000) {
                    *manager.cache_cleanup_cursor.lock().unwrap() =
                        Some(path.to_string_lossy().into_owned());
                    let id = CopyId(
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .context("invalid cache filename")?
                            .to_owned(),
                    );
                    let copy = manager.registered_copy(&id)?;
                    if !manager.cache_source_is_current(&copy)? {
                        stale.push(id);
                    }
                }
                Ok(stale)
            })
            .await??;
        let mut removed = 0;
        for id in stale {
            match self.evict_cache_copy(id).await {
                Ok(()) => removed += 1,
                Err(error) => tracing::warn!(%error, "Cache cleanup remains pending"),
            }
        }
        Ok(removed)
    }
}
pub(super) fn initial_vaults(
    root: &std::path::Path,
    images: Arc<dyn VaultAdapter>,
) -> HashMap<VaultId, RegisteredVault> {
    let local: Arc<dyn VaultAdapter> =
        Arc::new(super::adapters::FilesystemAdapter::new(root.to_owned()));
    [
        (LOCAL_AUDIO, VaultRole::Authoritative, local.clone()),
        (LOCAL_IMAGES, VaultRole::Cache, local),
        (IMAGE_ORIGIN, VaultRole::Authoritative, images),
    ]
    .into_iter()
    .map(|(id, role, adapter)| {
        let vault = Vault {
            id: VaultId(id.into()),
            role,
        };
        (vault.id.clone(), RegisteredVault { vault, adapter })
    })
    .collect()
}
