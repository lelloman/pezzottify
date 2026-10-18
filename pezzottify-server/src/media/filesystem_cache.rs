//! Bounded, ordered filesystem caches for published audio and images.
//!
//! See docs/media-cache.md. Each configured directory owns only disposable
//! copies: authoritative storage remains the source of truth, and a fill is
//! written to a temporary file and renamed into place only after validation.

use super::cache::{CacheEntry, CacheFillRequest, CacheKey, CacheSource, MediaCache};
use super::vault::{AdapterError, AdapterResult, MediaKey, MediaKind};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::warn;

const OBJECTS: &str = "objects";
const TRASH: &str = "trash";

/// One configured cache directory, in lookup order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesystemCacheSettings {
    pub name: String,
    pub path: PathBuf,
    pub max_bytes: u64,
}

struct CacheDirectory {
    name: String,
    root: PathBuf,
    max_bytes: u64,
}

#[derive(Default)]
struct CacheState {
    used: Vec<u64>,
    reserved: Vec<u64>,
    epochs: HashMap<MediaKey, u64>,
    fills: HashMap<CacheKey, u64>,
    #[cfg(test)]
    fills_started: u64,
}

/// Ordered set of filesystem caches with a shared policy state.
pub struct FilesystemCaches {
    caches: Vec<CacheDirectory>,
    state: Mutex<CacheState>,
}

impl FilesystemCaches {
    /// Validate configuration, prepare directories and recover persistent state.
    ///
    /// Cache directories may not overlap media storage or each other: publishing,
    /// invalidation and removal must never be able to touch authoritative media.
    pub fn new(settings: Vec<FilesystemCacheSettings>, media_root: &Path) -> anyhow::Result<Self> {
        anyhow::ensure!(!settings.is_empty(), "at least one media cache is required");
        let media_root = media_root
            .canonicalize()
            .map_err(|error| anyhow::anyhow!("media storage is unavailable: {error}"))?;

        let mut caches = Vec::with_capacity(settings.len());
        let mut prepared: Vec<(String, PathBuf)> = Vec::with_capacity(settings.len());
        for setting in settings {
            anyhow::ensure!(
                !setting.name.trim().is_empty(),
                "media cache name must not be empty"
            );
            anyhow::ensure!(
                setting.max_bytes > 0,
                "media cache '{}' max_bytes must be greater than zero",
                setting.name
            );
            anyhow::ensure!(
                !prepared.iter().any(|(name, _)| name == &setting.name),
                "media cache name '{}' is duplicated",
                setting.name
            );

            std::fs::create_dir_all(&setting.path).map_err(|error| {
                anyhow::anyhow!(
                    "media cache '{}' directory {} is unavailable: {error}",
                    setting.name,
                    setting.path.display()
                )
            })?;
            let root = setting.path.canonicalize().map_err(|error| {
                anyhow::anyhow!(
                    "media cache '{}' directory {} is unavailable: {error}",
                    setting.name,
                    setting.path.display()
                )
            })?;
            anyhow::ensure!(
                !overlaps(&media_root, &root),
                "media cache '{}' at {} overlaps media storage",
                setting.name,
                root.display()
            );
            for (other_name, other) in &prepared {
                anyhow::ensure!(
                    !overlaps(other, &root),
                    "media cache '{}' overlaps media cache '{}'",
                    setting.name,
                    other_name
                );
            }

            std::fs::create_dir_all(root.join(OBJECTS)).map_err(|error| {
                anyhow::anyhow!(
                    "media cache '{}' at {} is unavailable: {error}",
                    setting.name,
                    root.display()
                )
            })?;
            recover_directory(&root)?;
            prepared.push((setting.name.clone(), root.clone()));
            caches.push(CacheDirectory {
                name: setting.name,
                root,
                max_bytes: setting.max_bytes,
            });
        }

        let used = caches
            .iter()
            .map(|cache| super::availability::directory_size(&cache.root.join(OBJECTS)))
            .collect::<Vec<_>>();
        let reserved = vec![0; caches.len()];
        Ok(Self {
            caches,
            state: Mutex::new(CacheState {
                used,
                reserved,
                ..Default::default()
            }),
        })
    }

    pub fn names(&self) -> Vec<&str> {
        self.caches
            .iter()
            .map(|cache| cache.name.as_str())
            .collect()
    }

    fn object_dir(&self, index: usize, media: &MediaKey) -> PathBuf {
        self.caches[index]
            .root
            .join(OBJECTS)
            .join(kind_name(&media.kind))
            .join(id_hash(&media.id))
    }

    fn select_target(&self, state: &CacheState, content_length: u64) -> Option<usize> {
        self.caches.iter().enumerate().find_map(|(index, cache)| {
            let total = state.used[index]
                .checked_add(state.reserved[index])?
                .checked_add(content_length)?;
            (total <= cache.max_bytes).then_some(index)
        })
    }

    fn write_entry(
        &self,
        index: usize,
        epoch: u64,
        key: &CacheKey,
        filename: &str,
        content_length: u64,
        source: CacheSource,
    ) -> AdapterResult<()> {
        let directory = self.object_dir(index, &key.media);
        std::fs::create_dir_all(&directory).map_err(storage)?;
        let temporary = directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        let extension = extension_for(filename);
        let destination = directory.join(format!("{}.{}", key_hash(key), extension));

        let result = (|| -> AdapterResult<()> {
            let mut reader = source.open().map_err(storage)?;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(storage)?;
            let copied = std::io::copy(&mut reader, &mut file).map_err(storage)?;
            if copied != content_length {
                return Err(storage("source length changed during cache fill"));
            }
            file.flush().map_err(storage)?;
            file.sync_all().map_err(storage)?;
            drop(file);

            // Publish under the policy lock so invalidation cannot interleave
            // between the epoch check and the rename.
            let state = self.state.lock().unwrap();
            let current = state.epochs.get(&key.media).copied().unwrap_or(0);
            if current != epoch {
                return Err(storage("media invalidated during cache fill"));
            }
            if destination.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&temporary);
                return Err(storage("cache object already exists"));
            }
            std::fs::rename(&temporary, &destination).map_err(storage)?;
            sync_directory(&directory);
            Ok(())
        })();

        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    #[cfg(test)]
    pub fn used_bytes(&self, index: usize) -> u64 {
        self.state.lock().unwrap().used[index]
    }

    #[cfg(test)]
    pub fn fills_started(&self) -> u64 {
        self.state.lock().unwrap().fills_started
    }

    #[cfg(test)]
    pub fn pending_fills(&self) -> usize {
        self.state.lock().unwrap().fills.len()
    }

    #[cfg(test)]
    pub fn entry_path(&self, index: usize, key: &CacheKey) -> Option<PathBuf> {
        let directory = self.object_dir(index, &key.media);
        let hash = key_hash(key);
        let mut names = std::fs::read_dir(directory)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| entry_matches(name, &hash))
            .collect::<Vec<_>>();
        names.sort();
        names
            .first()
            .map(|name| self.object_dir(index, &key.media).join(name))
    }
}

impl MediaCache for FilesystemCaches {
    fn lookup(&self, key: &CacheKey) -> AdapterResult<Option<CacheEntry>> {
        let hash = key_hash(key);
        let mut last_error = None;
        for index in 0..self.caches.len() {
            let directory = self.object_dir(index, &key.media);
            match find_object(&directory, &hash) {
                Ok(Some((path, extension))) => match open_object(&path, &extension, &key.media) {
                    Ok(entry) => return Ok(Some(entry)),
                    Err(error) => last_error = Some(error),
                },
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }

    fn invalidate(&self, media: &MediaKey) {
        let mut trash = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            *state.epochs.entry(media.clone()).or_insert(0) += 1;
            for index in 0..self.caches.len() {
                let directory = self.object_dir(index, media);
                if directory.symlink_metadata().is_err() {
                    continue;
                }
                let tombstone = self.caches[index]
                    .root
                    .join(TRASH)
                    .join(uuid::Uuid::new_v4().to_string());
                if std::fs::create_dir_all(self.caches[index].root.join(TRASH)).is_err() {
                    warn!(
                        cache = self.caches[index].name,
                        media_id = media.id,
                        "Failed to prepare media cache trash; keeping stale entries"
                    );
                    continue;
                }
                match std::fs::rename(&directory, &tombstone) {
                    Ok(()) => trash.push((index, tombstone)),
                    Err(error) => warn!(
                        cache = self.caches[index].name,
                        media_id = media.id,
                        %error,
                        "Failed to invalidate media cache entries"
                    ),
                }
            }
        }
        // Removal happens outside the policy lock. Accounting stays conservative
        // until the tombstone is gone, so a concurrent fill cannot overshoot.
        for (index, tombstone) in trash {
            let removed = super::availability::directory_size(&tombstone);
            if let Err(error) = std::fs::remove_dir_all(&tombstone) {
                warn!(
                    cache = self.caches[index].name,
                    media_id = media.id,
                    %error,
                    "Failed to remove invalidated media cache entries"
                );
                continue;
            }
            let mut state = self.state.lock().unwrap();
            state.used[index] = state.used[index].saturating_sub(removed);
        }
    }

    fn invalidation_epoch(&self, media: &MediaKey) -> u64 {
        self.state
            .lock()
            .unwrap()
            .epochs
            .get(media)
            .copied()
            .unwrap_or(0)
    }

    fn populate(&self, request: CacheFillRequest) -> AdapterResult<()> {
        if request.content_length == 0 {
            return Ok(());
        }
        // A completed fill needs no second source read. This also keeps retries
        // and concurrent readers from reopening authoritative storage.
        if self.lookup(&request.key)?.is_some() {
            return Ok(());
        }

        let (index, epoch) = {
            let mut state = self.state.lock().unwrap();
            #[cfg(test)]
            {
                state.fills_started += 1;
            }
            // Recheck while serialized: a lookup that ran before another fill
            // published its entry must not start a second fill.
            if self.lookup(&request.key)?.is_some() {
                return Ok(());
            }
            // A read that began before an invalidation must not publish the
            // bytes it observed.
            let current = state.epochs.get(&request.key.media).copied().unwrap_or(0);
            if current != request.epoch {
                return Ok(());
            }
            if state.fills.get(&request.key) == Some(&current) {
                return Ok(());
            }
            let Some(index) = self.select_target(&state, request.content_length) else {
                return Ok(());
            };
            state.reserved[index] = state.reserved[index].saturating_add(request.content_length);
            state.fills.insert(request.key.clone(), current);
            (index, current)
        };

        let result = self.write_entry(
            index,
            epoch,
            &request.key,
            &request.filename,
            request.content_length,
            request.source,
        );

        let mut state = self.state.lock().unwrap();
        state.reserved[index] = state.reserved[index].saturating_sub(request.content_length);
        if result.is_ok() {
            state.used[index] = state.used[index].saturating_add(request.content_length);
        }
        if state.fills.get(&request.key) == Some(&epoch) {
            state.fills.remove(&request.key);
        }
        result
    }
}

fn overlaps(first: &Path, second: &Path) -> bool {
    first == second || first.starts_with(second) || second.starts_with(first)
}

fn recover_directory(root: &Path) -> anyhow::Result<()> {
    let trash = root.join(TRASH);
    if trash.exists() {
        std::fs::remove_dir_all(&trash)?;
    }
    // Image entries are process-scoped, so a restart starts cold for images.
    let images = root.join(OBJECTS).join(kind_name(&MediaKind::Image));
    if images.exists() {
        std::fs::remove_dir_all(&images)?;
    }
    // Any temporary file belongs to a fill that did not complete.
    for entry in walkdir::WalkDir::new(root.join(OBJECTS))
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if name.starts_with(".tmp-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn kind_name(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
    }
}

fn key_hash(key: &CacheKey) -> String {
    let generation = match &key.generation {
        super::cache::CacheGeneration::Durable => "durable".to_owned(),
        super::cache::CacheGeneration::Process(process) => format!("process:{process}"),
    };
    digest(&[
        kind_name(&key.media.kind).as_bytes(),
        key.media.id.as_bytes(),
        key.version.0.as_bytes(),
        generation.as_bytes(),
    ])
}

fn id_hash(id: &str) -> String {
    digest(&[id.as_bytes()])
}

fn digest(parts: &[&[u8]]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn entry_matches(name: &str, hash: &str) -> bool {
    name == hash
        || name
            .strip_prefix(hash)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn find_object(directory: &Path, hash: &str) -> AdapterResult<Option<(PathBuf, String)>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage(error)),
    };
    let mut names = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| entry_matches(name, hash))
        .collect::<Vec<_>>();
    names.sort();
    let Some(name) = names.into_iter().next() else {
        return Ok(None);
    };
    let extension = name
        .split_once('.')
        .map(|(_, extension)| extension.to_owned())
        .unwrap_or_default();
    Ok(Some((directory.join(name), extension)))
}

fn open_object(path: &Path, extension: &str, media: &MediaKey) -> AdapterResult<CacheEntry> {
    let file = File::open(path).map_err(storage)?;
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_file() {
        return Err(storage("cache object is not a regular file"));
    }
    let content_type = content_type(media, extension);
    Ok(CacheEntry {
        file,
        content_length: metadata.len(),
        filename: format!("{}.{}", media.id, extension),
        content_type,
    })
}

fn content_type(media: &MediaKey, extension: &str) -> String {
    match media.kind {
        MediaKind::Audio => {
            super::audio_content_type(Path::new(&format!("audio.{extension}"))).to_owned()
        }
        MediaKind::Image => mime_guess::from_ext(extension)
            .first_or_octet_stream()
            .to_string(),
    }
}

fn extension_for(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty() && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "bin".to_owned())
}

fn sync_directory(directory: &Path) {
    if let Ok(file) = File::open(directory) {
        let _ = file.sync_all();
    }
}

fn storage(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Storage(error.to_string())
}
