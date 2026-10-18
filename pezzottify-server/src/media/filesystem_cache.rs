//! Bounded, ordered filesystem caches for published audio and images.
//!
//! See docs/media-cache.md. Each configured directory owns only disposable
//! copies: authoritative storage remains the source of truth, and a fill is
//! written to a temporary file and renamed into place only after validation.

use super::cache::{CacheEntry, CacheFillRequest, CacheKey, CacheSource, MediaCache};
use super::vault::{AdapterError, AdapterResult, MediaKey, MediaKind};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
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
    pub lru: bool,
}

struct CacheDirectory {
    name: String,
    root: PathBuf,
    max_bytes: u64,
    lru: bool,
}

#[derive(Default)]
struct CacheState {
    used: Vec<u64>,
    reserved: Vec<u64>,
    // Exact allocated paths credited to used, including temporary files.
    // Reservations cover admitted fills before a path is allocated.
    charges: HashMap<PathBuf, (usize, u64)>,
    epochs: HashMap<MediaKey, u64>,
    recent: HashMap<PathBuf, (MediaKey, u64)>,
    clock: u64,
    evicting: HashSet<MediaKey>,
    fills: HashMap<CacheKey, u64>,
    #[cfg(test)]
    fills_started: u64,
}

/// Ordered set of filesystem caches with a shared policy state.
pub struct FilesystemCaches {
    caches: Vec<CacheDirectory>,
    state: Mutex<CacheState>,
    #[cfg(test)]
    after_rename: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    #[cfg(test)]
    fail_removal: std::sync::atomic::AtomicBool,
}

impl FilesystemCaches {
    /// Validate configuration, prepare directories and recover persistent state.
    ///
    /// Cache directories may not overlap media storage or each other: publishing,
    /// invalidation and removal must never be able to touch authoritative media.
    pub fn new(
        settings: Vec<FilesystemCacheSettings>,
        media_root: &Path,
        audio_keys: impl IntoIterator<Item = CacheKey>,
    ) -> anyhow::Result<Self> {
        // The catalog, not an epoch directory's name, establishes restart
        // validity. Keep only the exact current immutable audio revisions.
        let audio_keys = audio_keys.into_iter().collect::<Vec<_>>();
        let identities = audio_keys
            .iter()
            .map(|key| (id_hash(&key.media.id), key.media.clone()))
            .collect::<HashMap<_, _>>();
        let audio_keys = audio_keys
            .into_iter()
            .filter(|key| {
                key.media.kind == MediaKind::Audio
                    && key.generation == super::cache::CacheGeneration::Durable
            })
            .map(|key| (id_hash(&key.media.id), key_hash(&key)))
            .collect::<HashMap<_, _>>();
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
            anyhow::ensure!(
                !std::fs::symlink_metadata(root.join(OBJECTS))?
                    .file_type()
                    .is_symlink(),
                "media cache '{}' objects directory must not be a symlink",
                setting.name
            );
            recover_directory(&root, &audio_keys).map_err(|error| {
                anyhow::anyhow!(
                    "media cache '{}' recovery at {} failed: {error}",
                    setting.name,
                    root.display()
                )
            })?;
            prepared.push((setting.name.clone(), root.clone()));
            caches.push(CacheDirectory {
                name: setting.name,
                root,
                max_bytes: setting.max_bytes,
                lru: setting.lru,
            });
        }

        let mut used = vec![0u64; caches.len()];
        let mut charges = HashMap::new();
        for (index, cache) in caches.iter().enumerate() {
            for entry in walkdir::WalkDir::new(cache.root.join(OBJECTS)) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let bytes = entry.metadata()?.len();
                    used[index] = used[index].saturating_add(bytes);
                    charges.insert(entry.path().to_owned(), (index, bytes));
                }
            }
        }
        let mut recent = HashMap::new();
        for path in charges.keys() {
            // Identity is recovered from the catalog hash map below lazily on
            // lookup; eviction also needs never-read recovered objects.
            let identity = path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned());
            if let Some(identity) = identity {
                if let Some(media) = identities.get(&identity) {
                    recent.insert(path.clone(), (media.clone(), 0));
                }
            }
        }
        let reserved = vec![0; caches.len()];
        let instance = Self {
            caches,
            #[cfg(test)]
            after_rename: Mutex::new(None),
            #[cfg(test)]
            fail_removal: std::sync::atomic::AtomicBool::new(false),
            state: Mutex::new(CacheState {
                used,
                reserved,
                charges,
                recent,
                ..Default::default()
            }),
        };
        drop(instance.state());
        Ok(instance)
    }

    fn remove_tombstone(&self, path: &Path) -> std::io::Result<()> {
        #[cfg(test)]
        if self.fail_removal.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::other("injected tombstone removal failure"));
        }
        std::fs::remove_dir_all(path)
    }

    pub fn names(&self) -> Vec<&str> {
        self.caches
            .iter()
            .map(|cache| cache.name.as_str())
            .collect()
    }

    fn state(&self) -> StateGuard<'_> {
        StateGuard {
            caches: self,
            state: self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
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

    fn touch(&self, path: &Path, media: &MediaKey) {
        let mut state = self.state();
        if state.charges.contains_key(path) {
            state.clock = state.clock.saturating_add(1);
            let clock = state.clock;
            state.recent.insert(path.to_owned(), (media.clone(), clock));
        }
    }

    /// Claim candidates in memory, then try an exclusive descriptor lock and
    /// unlink outside the policy mutex. Readers retain shared descriptor locks,
    /// so neither their bytes nor their allocation are reclaimed early.
    fn make_room(&self, bytes: u64) {
        let candidates = {
            let mut state = self.state();
            if self.select_target(&state, bytes).is_some() {
                return;
            }
            let mut candidates = state
                .recent
                .iter()
                .filter_map(|(path, (media, stamp))| {
                    let (index, _) = state.charges.get(path)?;
                    let epoch = state.epochs.get(media).copied().unwrap_or(0);
                    // Failed invalidation may have moved this allocation to
                    // trash while keeping its original ledger path. Absence at
                    // that path is not evidence that the bytes were freed.
                    if !path.starts_with(self.object_epoch_dir(*index, media, epoch)) {
                        return None;
                    }
                    if !self.caches[*index].lru
                        || bytes > self.caches[*index].max_bytes
                        || state.evicting.contains(media)
                        || state.fills.keys().any(|key| &key.media == media)
                    {
                        return None;
                    }
                    Some((
                        *stamp,
                        path.clone(),
                        media.clone(),
                        *index,
                        state.epochs.get(media).copied().unwrap_or(0),
                    ))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
            // One candidate per identity avoids overlapping invalidation claims.
            candidates.retain(|(_, _, media, _, _)| state.evicting.insert(media.clone()));
            candidates
        };
        let _claims = EvictionClaims {
            caches: self,
            media: candidates
                .iter()
                .map(|(_, _, media, _, _)| media.clone())
                .collect(),
        };
        for (_, path, media, index, epoch) in candidates {
            if self.select_target(&self.state(), bytes).is_some() {
                break;
            }
            let result = (|| -> std::io::Result<bool> {
                let file = match File::open(&path) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
                    Err(error) => return Err(error),
                };
                if file.try_lock().is_err() {
                    return Ok(false);
                }
                #[cfg(test)]
                if self.fail_removal.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(std::io::Error::other("injected eviction removal failure"));
                }
                std::fs::remove_file(&path)?;
                Ok(true)
            })();
            match result {
                Ok(true) => {
                    let mut state = self.state();
                    if state.epochs.get(&media).copied().unwrap_or(0) == epoch {
                        release_charge(&mut state, &path);
                        event(&self.caches[index].name, "eviction");
                    }
                }
                Ok(false) => event(&self.caches[index].name, "eviction_busy"),
                Err(error) => {
                    event(&self.caches[index].name, "eviction_failed");
                    warn!(cache = self.caches[index].name, media_id = media.id, %error,
                        "Media cache eviction failed; allocation retained");
                }
            }
        }
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
        reservation: &mut FillReservation<'_>,
        filename: &str,
        content_length: u64,
        source: CacheSource,
    ) -> AdapterResult<()> {
        let index = reservation.index;
        let epoch = reservation.epoch;
        let key = &reservation.key;
        let directory = self.object_epoch_dir(index, &key.media, epoch);
        std::fs::create_dir_all(&directory).map_err(storage)?;
        let temporary = directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        let extension = extension_for(filename);

        // Assign the admitted bytes to this exact temporary path before I/O.
        // Cleanup can then settle temporary and complete objects independently,
        // including retained trash after failed removal.
        let mut publication = PublicationCharge {
            caches: self,
            path: temporary.clone(),
            temporary: temporary.clone(),
            media: key.media.clone(),
            epoch,
            published: false,
        };
        {
            let mut state = self.state();
            if state.epochs.get(&key.media).copied().unwrap_or(0) != epoch {
                return Err(storage("media invalidated during cache fill"));
            }
            state.reserved[index] -= content_length;
            reservation.settled = true;
            state.used[index] = state.used[index].saturating_add(content_length);
            state
                .charges
                .insert(temporary.clone(), (index, content_length));
        }

        let mut temporary_file = TemporaryFile::new(temporary.clone());
        let mut reader = source.open().map_err(storage)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(storage)?;
        let copied =
            std::io::copy(&mut reader.by_ref().take(content_length), &mut file).map_err(storage)?;
        let mut extra = [0u8; 1];
        if copied != content_length || reader.read(&mut extra).map_err(storage)? != 0 {
            return Err(storage("source length changed during cache fill"));
        }
        file.flush().map_err(storage)?;
        file.sync_all().map_err(storage)?;
        drop(file);

        let checksum = file_checksum(&mut File::open(&temporary).map_err(storage)?)?;
        let destination = directory.join(format!("{}.{checksum}.{extension}", key_hash(key)));
        if destination.symlink_metadata().is_ok() {
            return Err(storage("cache object already exists"));
        }
        // Move the existing charge to the destination before rename, ordered
        // with invalidation; no bytes are credited twice at publication.
        // Only the epoch check needs the shared policy lock: it is the one
        // ordering point with invalidation. Rename and directory fsync are
        // filesystem work and must never block foreground lookups or reads.
        {
            let mut state = self.state();
            let current = state.epochs.get(&key.media).copied().unwrap_or(0);
            if current != epoch {
                return Err(storage("media invalidated during cache fill"));
            }
            if let Some(charge) = state.charges.remove(&temporary) {
                state.charges.insert(destination.clone(), charge);
            }
            publication.path = destination.clone();
        }
        std::fs::rename(&temporary, &destination).map_err(storage)?;
        #[cfg(test)]
        {
            let hook = self.after_rename.lock().unwrap().take();
            if let Some(hook) = hook {
                hook();
            }
        }
        publication.published = true;
        temporary_file.disarm();
        sync_directory(&directory);
        self.touch(&destination, &key.media);
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
                        self.touch(&path, &key.media);
                        return Ok(Some(entry));
                    }
                    Err(AdapterError::Missing | AdapterError::Stale) => {}
                    Err(error) => {
                        warn!(cache = self.caches[index].name, media_id = key.media.id, %error,
                            "Invalid or unreadable media cache entry; invalidating");
                        self.invalidate(&key.media);
                        last_error = Some(error);
                    }
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
            let stale = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .parse::<u64>()
                        .is_ok_and(|old| old < epoch)
                })
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
                    Ok(()) => trash.push((index, directory, tombstone)),
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
        for (index, directory, tombstone) in trash {
            if let Err(error) = self.remove_tombstone(&tombstone) {
                warn!(
                    cache = self.caches[index].name,
                    media_id = media.id,
                    %error,
                    "Failed to remove invalidated media cache entries"
                );
                continue;
            }
            let mut state = self.state();
            let removed = state
                .charges
                .keys()
                .filter(|path| path.starts_with(&directory))
                .cloned()
                .collect::<Vec<_>>();
            for path in removed {
                release_charge(&mut state, &path);
            }
        }
    }

    fn invalidation_epoch(&self, media: &MediaKey) -> u64 {
        self.current_epoch(media)
    }

    fn populate(&self, request: CacheFillRequest) -> AdapterResult<()> {
        if request.content_length == 0 {
            event("all", "fill_skipped_empty");
            return Ok(());
        }
        // A completed fill needs no second source read. This also keeps retries
        // and concurrent readers from reopening authoritative storage.
        if self.lookup(&request.key)?.is_some() {
            event("all", "fill_skipped_present");
            return Ok(());
        }

        self.make_room(request.content_length);
        let mut reservation = {
            let mut state = self.state();
            #[cfg(test)]
            {
                state.fills_started += 1;
            }
            // A read that began before an invalidation must not publish the
            // bytes it observed.
            let current = state.epochs.get(&request.key.media).copied().unwrap_or(0);
            if current != request.epoch || state.evicting.contains(&request.key.media) {
                event("all", "fill_skipped_invalidated");
                return Ok(());
            }
            if state.fills.get(&request.key) == Some(&current) {
                event("all", "fill_skipped_duplicate");
                return Ok(());
            }
            let Some(index) = self.select_target(&state, request.content_length) else {
                event("all", "fill_skipped_capacity");
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
                settled: false,
                succeeded: false,
            }
        };

        // Another fill can finish between the initial lookup and reservation.
        // Once we own the marker, recheck outside the policy lock before opening
        // the source; completed concurrent work is a successful no-op.
        if self.lookup(&request.key)?.is_some() {
            event("all", "fill_skipped_present");
            reservation.succeeded = true;
            return Ok(());
        }

        let result = self.write_entry(
            &mut reservation,
            &request.filename,
            request.content_length,
            request.source,
        );
        reservation.succeeded = result.is_ok();
        drop(reservation);
        result
    }
}

struct StateGuard<'a> {
    caches: &'a FilesystemCaches,
    state: MutexGuard<'a, CacheState>,
}
impl std::ops::Deref for StateGuard<'_> {
    type Target = CacheState;
    fn deref(&self) -> &CacheState {
        &self.state
    }
}
impl std::ops::DerefMut for StateGuard<'_> {
    fn deref_mut(&mut self) -> &mut CacheState {
        &mut self.state
    }
}
impl Drop for StateGuard<'_> {
    fn drop(&mut self) {
        for (index, cache) in self.caches.caches.iter().enumerate() {
            for (kind, bytes) in [
                ("allocated", self.state.used[index]),
                ("reserved", self.state.reserved[index]),
                ("budget", cache.max_bytes),
            ] {
                crate::server::metrics::MEDIA_CACHE_BYTES
                    .with_label_values(&[&cache.name, kind])
                    .set(bytes as f64);
            }
        }
    }
}

struct EvictionClaims<'a> {
    caches: &'a FilesystemCaches,
    media: Vec<MediaKey>,
}
impl Drop for EvictionClaims<'_> {
    fn drop(&mut self) {
        let mut state = self.caches.state();
        for media in &self.media {
            state.evicting.remove(media);
        }
    }
}

fn file_checksum(file: &mut File) -> AdapterResult<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn event(cache: &str, outcome: &str) {
    crate::server::metrics::MEDIA_CACHE_EVENTS
        .with_label_values(&[cache, outcome])
        .inc();
}

/// Releases a reserved byte budget and the per-key fill marker on every exit,
/// including an unwinding panic from the source or the writer.
struct FillReservation<'a> {
    caches: &'a FilesystemCaches,
    index: usize,
    key: CacheKey,
    epoch: u64,
    content_length: u64,
    settled: bool,
    succeeded: bool,
}

impl Drop for FillReservation<'_> {
    fn drop(&mut self) {
        event(
            &self.caches.caches[self.index].name,
            if self.succeeded {
                "fill_completed"
            } else {
                "fill_failed"
            },
        );
        let mut state = self.caches.state();
        if !self.settled {
            state.reserved[self.index] -= self.content_length;
        }
        if state.fills.get(&self.key) == Some(&self.epoch) {
            state.fills.remove(&self.key);
        }
    }
}

// Failed publication releases only its own precredited charge, and only once
// absence is confirmed. Files left behind by errors/unwind stay conservatively
// charged. Successful invalidation also removes from this same ledger.
struct PublicationCharge<'a> {
    caches: &'a FilesystemCaches,
    path: PathBuf,
    temporary: PathBuf,
    media: MediaKey,
    epoch: u64,
    published: bool,
}

impl Drop for PublicationCharge<'_> {
    fn drop(&mut self) {
        if !self.published
            && self
                .temporary
                .symlink_metadata()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            && self
                .path
                .symlink_metadata()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            let mut state = self.caches.state();
            // Once invalidated, cleanup owns settlement: the file may have
            // moved to trash and still occupy disk even though this path is gone.
            if state.epochs.get(&self.media).copied().unwrap_or(0) == self.epoch {
                release_charge(&mut state, &self.path);
            }
        }
    }
}

fn release_charge(state: &mut CacheState, path: &Path) {
    state.recent.remove(path);
    if let Some((index, bytes)) = state.charges.remove(path) {
        state.used[index] = state.used[index].saturating_sub(bytes);
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

fn recover_directory(root: &Path, audio_keys: &HashMap<String, String>) -> anyhow::Result<()> {
    let trash = root.join(TRASH);
    if trash.symlink_metadata().is_ok() {
        remove_cached_path(&trash)?;
    }
    // Image entries are process-scoped, so a restart starts cold for images.
    let images = root.join(OBJECTS).join(kind_name(&MediaKind::Image));
    if images.symlink_metadata().is_ok() {
        remove_cached_path(&images)?;
    }
    let audio = root.join(OBJECTS).join(kind_name(&MediaKind::Audio));
    match audio.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            remove_cached_path(&audio)?;
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    for identity in std::fs::read_dir(&audio)? {
        let identity = identity?;
        let name = identity.file_name();
        let expected = audio_keys.get(name.to_string_lossy().as_ref());
        if !identity.file_type()?.is_dir() || expected.is_none() {
            remove_cached_path(&identity.path())?;
            continue;
        }
        let expected = expected.unwrap();
        let directory = identity.path();
        let canonical = directory.join("0");
        // Epochs only order concurrent work within one process. On restart,
        // rebase exact catalog-version matches to zero, independent of epoch
        // magnitude. Also accept the original flat layout; all other files,
        // partial fills and obsolete versions are disposable.
        let mut candidates = Vec::new();
        for entry in walkdir::WalkDir::new(&directory).min_depth(1) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let relative = entry.path().strip_prefix(&directory)?;
                let parts = relative.components().collect::<Vec<_>>();
                let known_layout = parts.len() == 1
                    || (parts.len() == 2
                        && parts[0]
                            .as_os_str()
                            .to_string_lossy()
                            .parse::<u64>()
                            .is_ok());
                let name = entry.file_name().to_string_lossy();
                let valid_name = name.strip_prefix(expected).is_some_and(|suffix| {
                    suffix.strip_prefix('.').is_some_and(|suffix| {
                        suffix.split_once('.').is_some_and(|(hash, extension)| {
                            hash.len() == 64
                                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                                && !extension.is_empty()
                                && extension.bytes().all(|b| b.is_ascii_alphanumeric())
                        })
                    })
                });
                if known_layout && valid_name {
                    candidates.push(entry.path().to_owned());
                } else {
                    std::fs::remove_file(entry.path())?;
                }
            } else if entry.file_type().is_symlink() {
                std::fs::remove_file(entry.path())?;
            }
        }
        std::fs::create_dir_all(&canonical)?;
        candidates.sort();
        for path in candidates {
            let destination = canonical.join(path.file_name().unwrap());
            if path != destination {
                if destination.exists() {
                    // Both copies have the same immutable catalog revision.
                    std::fs::remove_file(path)?;
                } else {
                    std::fs::rename(path, destination)?;
                }
            }
        }
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.path() != canonical {
                remove_cached_path(&entry.path())?;
            }
        }
        sync_directory(&canonical);
        sync_directory(&directory);
    }
    Ok(())
}

fn remove_cached_path(path: &Path) -> std::io::Result<()> {
    if path.symlink_metadata()?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
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
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_owned())
        .unwrap_or_default();
    Ok(Some((directory.join(name), extension)))
}

fn open_object(path: &Path, extension: &str, media: &MediaKey) -> AdapterResult<CacheEntry> {
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AdapterError::Missing
        } else {
            storage(error)
        }
    })?;
    file.try_lock_shared().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => AdapterError::Stale,
        error => storage(error),
    })?;
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_file() {
        return Err(storage("cache object is not a regular file"));
    }
    let expected = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.split('.').nth(1))
        .ok_or_else(|| storage("missing cache checksum"))?;
    if file_checksum(&mut file)? != expected {
        return Err(storage("cache checksum mismatch"));
    }
    use std::io::Seek;
    file.rewind().map_err(storage)?;
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
                lru: false,
            }],
            &media,
            std::iter::empty(),
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

#[cfg(test)]
mod accounting_tests {
    use super::*;
    use std::io::{Cursor, Read};
    use std::sync::Arc;

    const SIZE: u64 = 8192;

    fn setup() -> (tempfile::TempDir, Arc<FilesystemCaches>) {
        let root = tempfile::tempdir().unwrap();
        let media = root.path().join("media");
        std::fs::create_dir(&media).unwrap();
        let caches = Arc::new(
            FilesystemCaches::new(
                vec![FilesystemCacheSettings {
                    name: "ssd".into(),
                    path: root.path().join("cache"),
                    max_bytes: SIZE * 2,
                    lru: false,
                }],
                &media,
                std::iter::empty(),
            )
            .unwrap(),
        );
        (root, caches)
    }

    fn key(id: &str) -> CacheKey {
        CacheKey::image(id, "https://example.test/cover.jpg", "process")
    }

    fn request(id: &str) -> CacheFillRequest {
        CacheFillRequest {
            key: key(id),
            epoch: 0,
            filename: "cover.jpg".into(),
            content_type: "image/jpeg".into(),
            content_length: SIZE,
            source: CacheSource::Bytes(Arc::from(vec![7; SIZE as usize])),
        }
    }

    fn complete_bytes(caches: &FilesystemCaches) -> u64 {
        walkdir::WalkDir::new(&caches.caches[0].root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| {
                entry.file_type().is_file()
                    && !entry.file_name().to_string_lossy().starts_with(".tmp-")
            })
            .map(|entry| entry.metadata().unwrap().len())
            .sum()
    }

    fn assert_capacity(caches: &FilesystemCaches, expected: u64) {
        assert_eq!(caches.used_bytes(0), expected);
        assert_eq!(complete_bytes(caches), expected);
        assert_eq!(caches.reserved_bytes(0), 0);
        assert_eq!(caches.pending_fills(), 0);
        assert!(expected <= SIZE * 2);
    }

    struct InvalidateOnEof {
        bytes: Cursor<Vec<u8>>,
        caches: Arc<FilesystemCaches>,
    }

    impl Read for InvalidateOnEof {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.bytes.position() == SIZE {
                // copy has written B's entire temporary file, but not published.
                assert_eq!(self.caches.used_bytes(0), SIZE * 2);
                assert_eq!(self.caches.reserved_bytes(0), 0);
                self.caches.invalidate(&key("B").media);
            }
            self.bytes.read(buffer)
        }
    }

    #[test]
    fn invalidating_temporary_fill_never_debits_another_entry() {
        let (_root, caches) = setup();
        caches.populate(request("A")).unwrap();
        let mut b = request("B");
        let source_cache = caches.clone();
        b.source = CacheSource::Open(Box::new(move || {
            Ok(Box::new(InvalidateOnEof {
                bytes: Cursor::new(vec![7; SIZE as usize]),
                caches: source_cache,
            }))
        }));
        assert!(caches.populate(b).is_err());
        assert_capacity(&caches, SIZE);
        caches.populate(request("C")).unwrap();
        caches.populate(request("D")).unwrap();
        assert_capacity(&caches, SIZE * 2);
        assert!(caches.lookup(&key("A")).unwrap().is_some());
        assert!(caches.lookup(&key("C")).unwrap().is_some());
        assert!(caches.lookup(&key("D")).unwrap().is_none());
    }

    #[test]
    fn growing_source_cannot_write_past_its_admitted_budget() {
        struct GrowingReader {
            caches: Arc<FilesystemCaches>,
            remaining: usize,
        }
        impl Read for GrowingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let size = buffer.len().min(self.remaining);
                self.remaining -= size;
                buffer[..size].fill(7);
                let physical: u64 = walkdir::WalkDir::new(&self.caches.caches[0].root)
                    .into_iter()
                    .map(Result::unwrap)
                    .filter(|entry| entry.file_type().is_file())
                    .map(|entry| entry.metadata().unwrap().len())
                    .sum();
                assert!(physical <= SIZE, "source wrote past its allocation");
                Ok(size)
            }
        }
        let (_root, caches) = setup();
        let mut a = request("A");
        let source_cache = caches.clone();
        a.source = CacheSource::Open(Box::new(move || {
            Ok(Box::new(GrowingReader {
                caches: source_cache,
                remaining: SIZE as usize * 4,
            }))
        }));
        assert!(caches.populate(a).is_err());
        assert_capacity(&caches, 0);
        caches.populate(request("A")).unwrap();
        assert_capacity(&caches, SIZE);
    }

    #[test]
    fn retained_temporary_trash_stays_charged_after_source_unwind() {
        struct InvalidateAndPanic {
            bytes: Cursor<Vec<u8>>,
            caches: Arc<FilesystemCaches>,
        }
        impl Read for InvalidateAndPanic {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.bytes.position() == SIZE {
                    self.caches.invalidate(&key("B").media);
                    panic!("source unwind after temporary file moved to trash");
                }
                self.bytes.read(buffer)
            }
        }
        let (_root, caches) = setup();
        caches.populate(request("A")).unwrap();
        caches
            .fail_removal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let source_cache = caches.clone();
        let mut b = request("B");
        b.source = CacheSource::Open(Box::new(move || {
            Ok(Box::new(InvalidateAndPanic {
                bytes: Cursor::new(vec![7; SIZE as usize]),
                caches: source_cache,
            }))
        }));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| caches.populate(b))).is_err()
        );
        assert_eq!(caches.used_bytes(0), SIZE * 2);
        assert_eq!(caches.reserved_bytes(0), 0);
        assert_eq!(caches.pending_fills(), 0);
        assert_eq!(complete_bytes(&caches), SIZE);
        let physical: u64 = walkdir::WalkDir::new(&caches.caches[0].root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.metadata().unwrap().len())
            .sum();
        assert_eq!(physical, SIZE * 2);
        caches.populate(request("C")).unwrap();
        assert!(caches.lookup(&key("C")).unwrap().is_none());
        assert!(caches.lookup(&key("A")).unwrap().is_some());
        assert!(caches.lookup(&key("B")).unwrap().is_none());
    }

    #[test]
    fn failed_tombstone_removal_remains_charged_through_unwind_until_restart() {
        let (root, caches) = setup();
        caches.populate(request("A")).unwrap();
        caches
            .fail_removal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let weak = Arc::downgrade(&caches);
        *caches.after_rename.lock().unwrap() = Some(Box::new(move || {
            weak.upgrade().unwrap().invalidate(&key("B").media);
            panic!("unwind with published bytes retained in trash");
        }));
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || caches.populate(request("B"))
        ))
        .is_err());
        assert_capacity(&caches, SIZE * 2);
        assert!(std::fs::read_dir(caches.caches[0].root.join(TRASH))
            .unwrap()
            .next()
            .is_some());
        caches.populate(request("C")).unwrap();
        assert!(caches.lookup(&key("C")).unwrap().is_none());
        assert_capacity(&caches, SIZE * 2);
        drop(caches);
        let restarted = FilesystemCaches::new(
            vec![FilesystemCacheSettings {
                name: "ssd".into(),
                path: root.path().join("cache"),
                max_bytes: SIZE * 2,
                lru: false,
            }],
            &root.path().join("media"),
            std::iter::empty(),
        )
        .unwrap();
        // Both image entries and retained trash are discarded on restart.
        assert_capacity(&restarted, 0);
        restarted.populate(request("C")).unwrap();
        assert_capacity(&restarted, SIZE);
    }

    #[test]
    fn invalidation_after_rename_settles_exact_charge_even_on_unwind_or_cleanup_failure() {
        for fail_cleanup in [false, true] {
            for unwind in [false, true] {
                let (_root, caches) = setup();
                caches.populate(request("A")).unwrap();
                if fail_cleanup {
                    std::fs::write(caches.caches[0].root.join(TRASH), []).unwrap();
                }
                let weak = Arc::downgrade(&caches);
                *caches.after_rename.lock().unwrap() = Some(Box::new(move || {
                    let caches = weak.upgrade().unwrap();
                    caches.invalidate(&key("B").media);
                    if unwind {
                        panic!("unwind after rename and invalidation");
                    }
                }));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    caches.populate(request("B"))
                }));
                assert_eq!(result.is_err(), unwind);
                if !unwind {
                    result.unwrap().unwrap();
                }
                let expected = if fail_cleanup { SIZE * 2 } else { SIZE };
                assert_capacity(&caches, expected);
                assert!(caches.lookup(&key("B")).unwrap().is_none());
                caches.populate(request("C")).unwrap();
                caches.populate(request("D")).unwrap();
                assert_capacity(&caches, SIZE * 2);
                assert_eq!(caches.lookup(&key("C")).unwrap().is_some(), !fail_cleanup);
                assert!(caches.lookup(&key("D")).unwrap().is_none());
                // A later cleanup releases the retained exact B charge once.
                if fail_cleanup {
                    std::fs::remove_file(caches.caches[0].root.join(TRASH)).unwrap();
                    caches.invalidate(&key("B").media);
                    assert_capacity(&caches, SIZE);
                    caches.invalidate(&key("B").media);
                    assert_capacity(&caches, SIZE);
                    caches.populate(request("D")).unwrap();
                    assert_capacity(&caches, SIZE * 2);
                }
            }
        }
    }
}

#[cfg(test)]
mod eviction_tests {
    use super::*;
    use std::io::Read;
    use std::sync::Arc;

    fn setup() -> (tempfile::TempDir, Arc<FilesystemCaches>) {
        let root = tempfile::tempdir().unwrap();
        let media = root.path().join("source");
        std::fs::create_dir(&media).unwrap();
        std::fs::write(media.join("original"), b"untouched").unwrap();
        let caches = Arc::new(
            FilesystemCaches::new(
                vec![FilesystemCacheSettings {
                    name: "lru-test".into(),
                    path: root.path().join("cache"),
                    max_bytes: 16,
                    lru: true,
                }],
                &media,
                [],
            )
            .unwrap(),
        );
        (root, caches)
    }
    fn key(id: &str) -> CacheKey {
        CacheKey::image(id, "https://example.test/image.jpg", "test")
    }
    fn fill(caches: &FilesystemCaches, id: &str) {
        caches
            .populate(CacheFillRequest {
                key: key(id),
                epoch: caches.current_epoch(&key(id).media),
                filename: "image.jpg".into(),
                content_type: "image/jpeg".into(),
                content_length: 8,
                source: CacheSource::Bytes(Arc::from(&b"12345678"[..])),
            })
            .unwrap();
    }

    #[test]
    fn lru_preserves_recent_entries_active_readers_and_originals() {
        let (root, caches) = setup();
        fill(&caches, "a");
        fill(&caches, "b");
        drop(caches.lookup(&key("a")).unwrap()); // b is now least recent
        fill(&caches, "c");
        assert!(caches.lookup(&key("b")).unwrap().is_none());
        let mut a = caches.lookup(&key("a")).unwrap().unwrap();
        let c = caches.lookup(&key("c")).unwrap().unwrap();
        fill(&caches, "d"); // both readers held: no allocation can be freed
        assert!(caches.lookup(&key("d")).unwrap().is_none());
        assert_eq!(caches.used_bytes(0), 16);
        let mut bytes = Vec::new();
        a.file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"12345678");
        drop(a);
        drop(c);
        fill(&caches, "d");
        assert!(caches.lookup(&key("d")).unwrap().is_some());
        assert_eq!(
            std::fs::read(root.path().join("source/original")).unwrap(),
            b"untouched"
        );
        assert_eq!(caches.reserved_bytes(0), 0);
    }

    #[test]
    fn concurrent_eviction_fills_and_reads_keep_capacity_bounded() {
        let (_root, caches) = setup();
        let threads = (0..8)
            .map(|thread| {
                let caches = caches.clone();
                std::thread::spawn(move || {
                    for iteration in 0..40 {
                        let id = format!("{thread}-{iteration}");
                        fill(&caches, &id);
                        let _reader = caches.lookup(&key(&id)).unwrap();
                        let state = caches.state();
                        assert!(state.used[0] + state.reserved[0] <= 16);
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(caches.pending_fills(), 0);
        assert_eq!(caches.reserved_bytes(0), 0);
        let physical: u64 = walkdir::WalkDir::new(&caches.caches[0].root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.metadata().unwrap().len())
            .sum();
        assert_eq!(physical, caches.used_bytes(0));
    }

    #[test]
    fn eviction_never_credits_missing_paths_whose_bytes_remain_in_trash() {
        let (_root, caches) = setup();
        fill(&caches, "a");
        fill(&caches, "b");
        caches
            .fail_removal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        caches.invalidate(&key("a").media);
        assert_eq!(caches.used_bytes(0), 16);
        caches
            .fail_removal
            .store(false, std::sync::atomic::Ordering::SeqCst);
        // B can be evicted; A remains charged in trash. A's absent original
        // pathname must never be credited as though its bytes were unlinked.
        fill(&caches, "c");
        assert!(caches.lookup(&key("c")).unwrap().is_some());
        assert_eq!(caches.used_bytes(0), 16);
        let physical: u64 = walkdir::WalkDir::new(&caches.caches[0].root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.metadata().unwrap().len())
            .sum();
        assert_eq!(physical, caches.used_bytes(0));
    }

    #[test]
    fn eviction_failure_retains_charge_and_is_retriable() {
        let (_root, caches) = setup();
        fill(&caches, "a");
        fill(&caches, "b");
        caches
            .fail_removal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        fill(&caches, "c");
        assert_eq!(caches.used_bytes(0), 16);
        assert!(caches.state().evicting.is_empty());
        assert!(caches.lookup(&key("c")).unwrap().is_none());
        caches
            .fail_removal
            .store(false, std::sync::atomic::Ordering::SeqCst);
        fill(&caches, "c");
        assert!(caches.lookup(&key("c")).unwrap().is_some());
        assert_eq!(caches.used_bytes(0), 16);
    }

    #[test]
    fn same_length_corruption_is_rejected_and_reclaimed() {
        let (_root, caches) = setup();
        fill(&caches, "a");
        let path = caches.entry_path(0, &key("a")).unwrap();
        std::fs::write(path, b"corrupt!").unwrap();
        assert!(caches.lookup(&key("a")).is_err());
        assert_eq!(caches.used_bytes(0), 0);
        fill(&caches, "a");
        assert!(caches.lookup(&key("a")).unwrap().is_some());
    }

    #[test]
    fn missing_cache_object_is_reclaimed_under_pressure() {
        let (_root, caches) = setup();
        fill(&caches, "a");
        fill(&caches, "b");
        std::fs::remove_file(caches.entry_path(0, &key("a")).unwrap()).unwrap();
        fill(&caches, "c");
        assert!(caches.lookup(&key("c")).unwrap().is_some());
        assert_eq!(caches.used_bytes(0), 16);
    }
}
