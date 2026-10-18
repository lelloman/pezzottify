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
use std::sync::{Mutex, MutexGuard};
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

    fn state(&self) -> MutexGuard<'_, CacheState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Directory holding every generation of one media identity. Invalidation
    /// removes these subdirectories, but only after bumping the epoch so stale
    /// entries are unreachable even when cleanup fails.
    fn object_dir(&self, index: usize, media: &MediaKey) -> PathBuf {
        self.caches[index]
            .root
            .join(OBJECTS)
            .join(kind_name(&media.kind))
            .join(id_hash(&media.id))
    }

    /// Entries are stored under their invalidation epoch. A failed cleanup can
    /// leave old bytes on disk, but lookup only ever opens the current epoch.
    fn object_epoch_dir(&self, index: usize, media: &MediaKey, epoch: u64) -> PathBuf {
        self.object_dir(index, media).join(epoch.to_string())
    }

    fn current_epoch(&self, media: &MediaKey) -> u64 {
        self.state().epochs.get(media).copied().unwrap_or(0)
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
        let directory = self.object_epoch_dir(index, &key.media, epoch);
        std::fs::create_dir_all(&directory).map_err(storage)?;
        let temporary = directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        let mut temporary_file = TemporaryFile::new(temporary.clone());
        let extension = extension_for(filename);
        let destination = directory.join(format!("{}.{}", key_hash(key), extension));

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

        // Only the epoch check needs the shared policy lock: it is the one
        // ordering point with invalidation. Rename and directory fsync are
        // filesystem work and must never block foreground lookups or reads.
        {
            let state = self.state();
            let current = state.epochs.get(&key.media).copied().unwrap_or(0);
            if current != epoch {
                return Err(storage("media invalidated during cache fill"));
            }
        }
        if destination.symlink_metadata().is_ok() {
            return Err(storage("cache object already exists"));
        }
        std::fs::rename(&temporary, &destination).map_err(storage)?;
        temporary_file.disarm();
        sync_directory(&directory);
        Ok(())
    }

    #[cfg(test)]
    pub fn used_bytes(&self, index: usize) -> u64 {
        self.state().used[index]
    }

    #[cfg(test)]
    pub fn fills_started(&self) -> u64 {
        self.state().fills_started
    }

    #[cfg(test)]
    pub fn pending_fills(&self) -> usize {
        self.state().fills.len()
    }

    #[cfg(test)]
    pub fn reserved_bytes(&self, index: usize) -> u64 {
        self.state().reserved[index]
    }

    #[cfg(test)]
    pub fn entry_path(&self, index: usize, key: &CacheKey) -> Option<PathBuf> {
        let directory = self.object_epoch_dir(index, &key.media, self.current_epoch(&key.media));
        let hash = key_hash(key);
        let mut names = std::fs::read_dir(directory)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| entry_matches(name, &hash))
            .collect::<Vec<_>>();
        names.sort();
        names.first().map(|name| {
            self.object_epoch_dir(index, &key.media, self.current_epoch(&key.media))
                .join(name)
        })
    }
}

impl FilesystemCaches {
    fn lookup_with_open(
        &self,
        key: &CacheKey,
        mut open: impl FnMut(&Path, &str, &MediaKey) -> AdapterResult<CacheEntry>,
    ) -> AdapterResult<Option<CacheEntry>> {
        let hash = key_hash(key);
        let epoch = self.current_epoch(&key.media);
        let mut last_error = None;
        for index in 0..self.caches.len() {
            let directory = self.object_epoch_dir(index, &key.media, epoch);
            match find_object(&directory, &hash) {
                Ok(Some((path, extension))) => match open(&path, &extension, &key.media) {
                    Ok(entry) => {
                        // The descriptor must predate invalidation. Cleanup may
                        // fail, leaving an old epoch physically openable.
                        if self.current_epoch(&key.media) != epoch {
                            return Ok(None);
                        }
                        return Ok(Some(entry));
                    }
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
}

impl MediaCache for FilesystemCaches {
    fn lookup(&self, key: &CacheKey) -> AdapterResult<Option<CacheEntry>> {
        self.lookup_with_open(key, open_object)
    }

    fn invalidate(&self, media: &MediaKey) {
        // Bumping the epoch under the lock makes every older entry unreachable
        // immediately, even if the filesystem cleanup below cannot run.
        let epoch = {
            let mut state = self.state();
            let epoch = state.epochs.entry(media.clone()).or_insert(0);
            *epoch += 1;
            *epoch
        };
        let mut trash = Vec::new();
        for index in 0..self.caches.len() {
            let directory = self.object_dir(index, media);
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    warn!(
                        cache = self.caches[index].name,
                        media_id = media.id,
                        %error,
                        "Failed to inspect invalidated media cache entries"
                    );
                    continue;
                }
            };
            let current = epoch.to_string();
            let stale = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .filter(|entry| entry.file_name().to_string_lossy() != current)
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            for directory in stale {
                let tombstone = self.caches[index]
                    .root
                    .join(TRASH)
                    .join(uuid::Uuid::new_v4().to_string());
                if let Err(error) = std::fs::create_dir_all(self.caches[index].root.join(TRASH)) {
                    warn!(
                        cache = self.caches[index].name,
                        media_id = media.id,
                        %error,
                        "Failed to prepare media cache trash; stale entries are unreachable"
                    );
                    break;
                }
                match std::fs::rename(&directory, &tombstone) {
                    Ok(()) => trash.push((index, tombstone)),
                    Err(error) => warn!(
                        cache = self.caches[index].name,
                        media_id = media.id,
                        %error,
                        "Failed to clean up invalidated media cache entries; they are unreachable"
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
            let mut state = self.state();
            state.used[index] = state.used[index].saturating_sub(removed);
        }
    }

    fn invalidation_epoch(&self, media: &MediaKey) -> u64 {
        self.current_epoch(media)
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

        let mut reservation = {
            let mut state = self.state();
            #[cfg(test)]
            {
                state.fills_started += 1;
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
            FillReservation {
                caches: self,
                index,
                key: request.key.clone(),
                epoch: current,
                content_length: request.content_length,
                published: false,
            }
        };

        // Another fill can finish between the initial lookup and reservation.
        // Once we own the marker, recheck outside the policy lock before opening
        // the source; completed concurrent work is a successful no-op.
        if self.lookup(&request.key)?.is_some() {
            return Ok(());
        }

        let result = self.write_entry(
            reservation.index,
            reservation.epoch,
            &reservation.key,
            &request.filename,
            request.content_length,
            request.source,
        );
        if result.is_ok() {
            reservation.mark_published();
        }
        drop(reservation);
        result
    }
}

/// Releases a reserved byte budget and the per-key fill marker on every exit,
/// including an unwinding panic from the source or the writer.
struct FillReservation<'a> {
    caches: &'a FilesystemCaches,
    index: usize,
    key: CacheKey,
    epoch: u64,
    content_length: u64,
    published: bool,
}

impl FillReservation<'_> {
    fn mark_published(&mut self) {
        self.published = true;
    }
}

impl Drop for FillReservation<'_> {
    fn drop(&mut self) {
        let mut state = self.caches.state();
        state.reserved[self.index] = state.reserved[self.index].saturating_sub(self.content_length);
        if self.published {
            state.used[self.index] = state.used[self.index].saturating_add(self.content_length);
        }
        if state.fills.get(&self.key) == Some(&self.epoch) {
            state.fills.remove(&self.key);
        }
    }
}

/// Removes a fill's temporary file unless publication succeeded. Also covers a
/// panic escaping `write_entry`, where an explicit error-path cleanup would not
/// run.
struct TemporaryFile {
    path: PathBuf,
    armed: bool,
}

impl TemporaryFile {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
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

#[cfg(test)]
mod lookup_race_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn invalidation_before_open_rejects_old_descriptor_even_when_cleanup_fails() {
        let root = tempfile::tempdir().unwrap();
        let media = root.path().join("media");
        std::fs::create_dir(&media).unwrap();
        let path = root.path().join("cache");
        let caches = FilesystemCaches::new(
            vec![FilesystemCacheSettings {
                name: "ssd".into(),
                path: path.clone(),
                max_bytes: 1024,
            }],
            &media,
        )
        .unwrap();
        let key = CacheKey::image("cover", "https://example.test/cover.jpg", "process");
        caches
            .populate(CacheFillRequest {
                key: key.clone(),
                epoch: 0,
                filename: "cover.jpg".into(),
                content_type: "image/jpeg".into(),
                content_length: 3,
                source: CacheSource::Bytes(Arc::from(&b"old"[..])),
            })
            .unwrap();
        let old_path = caches.entry_path(0, &key).unwrap();
        std::fs::write(path.join(TRASH), b"prevent cleanup").unwrap();

        // Gate precisely between selecting the old epoch and opening it. This
        // also proves invalidation can acquire the state lock during open I/O.
        let hit = caches
            .lookup_with_open(&key, |path, extension, media| {
                caches.invalidate(media);
                assert!(old_path.exists(), "cleanup failure must leave the old file");
                open_object(path, extension, media)
            })
            .unwrap();
        assert!(hit.is_none());
        assert!(caches.lookup(&key).unwrap().is_none());
    }
}
