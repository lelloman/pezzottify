//! Durable, revision-specific publication and removal. Filesystem and catalog
//! commits are separate; the pending journal makes their interruption recoverable.
use super::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, Weak};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Provenance {
    Ingested,
    Proxy { materialized_at: i64 },
    ImageCache,
    Retained,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum Phase {
    Writing,
    Ready,
    Attached,
    Removing,
    Detached,
    Observing,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopyReceipt {
    pub revision: String,
    pub media_id: String,
    pub uri: String,
    pub provenance: Provenance,
    #[serde(default)]
    pub pending_effects: bool,
    previous: Option<String>,
    image: bool,
    pub(super) phase: Phase,
    pub(super) staging: String,
    pub(super) owner: String,
}

/// A local staging lease is the sole path-based exception for converters.
/// Dropping an uncommitted lease aborts it; durable ready operations belong to recovery.
pub struct StagedMedia {
    pub(crate) record: CopyReceipt,
    root: PathBuf,
    pub(super) handed_off: bool,
}
impl StagedMedia {
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.record.staging)
    }
}
impl Drop for StagedMedia {
    fn drop(&mut self) {
        if !self.handed_off {
            let _ = std::fs::remove_file(self.path());
            let _ = std::fs::remove_file(pending_path(&self.root, &self.record.revision));
        }
    }
}

pub(super) struct Effects {
    pub search: Arc<dyn crate::search::SearchVault>,
    pub server: Option<Arc<dyn crate::server_store::ServerStore>>,
}

pub(super) fn mutation_lock(root: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let key = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap();
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}
fn process_epoch() -> &'static str {
    static EPOCH: OnceLock<String> = OnceLock::new();
    EPOCH
        .get_or_init(|| uuid::Uuid::new_v4().to_string())
        .as_str()
}
pub(super) fn pending_path(root: &Path, revision: &str) -> PathBuf {
    root.join(".media/pending").join(format!("{revision}.json"))
}
pub(super) fn copy_path(root: &Path, revision: &str) -> PathBuf {
    root.join(".media/copies").join(format!("{revision}.json"))
}
fn image_pointer(root: &Path, id: &str) -> PathBuf {
    root.join(".media/images").join(format!("{id}.json"))
}
fn validate_id(id: &str) -> Result<()> {
    anyhow::ensure!(
        !id.trim().is_empty()
            && id != "."
            && id != ".."
            && !id
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':')),
        "invalid media identity"
    );
    Ok(())
}
fn prepare_directory(root: &Path, relative: &str) -> Result<()> {
    let mut path = root.to_owned();
    for part in local::normalized_media_identifier(relative)?.components() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "managed directory is not a real directory"
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut builder = std::fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(&path)?;
                sync_directory(path.parent().context("managed directory has no parent")?)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}
pub(super) fn save(path: &Path, record: &CopyReceipt) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(record)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        sync_directory(path.parent().context("journal has no parent")?)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
pub(super) fn read_record(path: &Path) -> Result<CopyReceipt> {
    let record: CopyReceipt = serde_json::from_slice(&std::fs::read(path)?)?;
    validate_id(&record.media_id)?;
    uuid::Uuid::parse_str(&record.revision)?;
    if record.phase != Phase::Observing {
        let extension = Path::new(&record.uri)
            .extension()
            .and_then(|s| s.to_str())
            .context("copy has no extension")?;
        anyhow::ensure!(
            extension.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid extension"
        );
        let expected = format!(
            "{}/.managed/{}.{}.{}",
            if record.image { "images" } else { "audio" },
            record.media_id,
            record.revision,
            extension
        );
        anyhow::ensure!(
            record.uri == expected
                && record.staging == format!(".media/staging/{}.{}", record.revision, extension),
            "invalid journal locator"
        );
    }
    Ok(record)
}
fn unlink(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_directory(path.parent().context("file has no parent")?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

impl MediaManager {
    pub fn configure_effects(
        &self,
        search: Arc<dyn crate::search::SearchVault>,
        server: Arc<dyn crate::server_store::ServerStore>,
    ) {
        let _ = self.effects.set(Effects {
            search,
            server: Some(server),
        });
    }
    pub(crate) fn configure_search(&self, search: Arc<dyn crate::search::SearchVault>) {
        let _ = self.effects.set(Effects {
            search,
            server: None,
        });
    }
    fn current_uri(&self, record: &CopyReceipt) -> Result<Option<String>> {
        if record.image {
            return match read_record(&image_pointer(&self.root, &record.media_id)) {
                Ok(record) => Ok(Some(record.uri)),
                Err(error)
                    if error
                        .downcast_ref::<io::Error>()
                        .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
                {
                    Ok(None)
                }
                Err(error) => Err(error),
            };
        }
        Ok(self
            .catalog
            .get_track(&record.media_id)?
            .and_then(|track| track.audio_uri))
    }
    pub fn begin_publication(
        &self,
        id: &str,
        extension: &str,
        provenance: Provenance,
    ) -> Result<StagedMedia> {
        validate_id(id)?;
        anyhow::ensure!(
            !extension.is_empty() && extension.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid media extension"
        );
        let _guard = self.mutations.lock().unwrap();
        for path in [
            ".media/staging",
            ".media/pending",
            ".media/copies",
            ".media/images",
            "audio/.managed",
            "images/.managed",
        ] {
            prepare_directory(&self.root, path)?;
        }
        let revision = uuid::Uuid::new_v4().to_string();
        let image = provenance == Provenance::ImageCache;
        let mut record = CopyReceipt {
            uri: format!(
                "{}/.managed/{id}.{revision}.{extension}",
                if image { "images" } else { "audio" }
            ),
            staging: format!(".media/staging/{revision}.{extension}"),
            pending_effects: false,
            owner: process_epoch().to_owned(),
            revision,
            media_id: id.to_owned(),
            provenance,
            previous: None,
            image,
            phase: Phase::Writing,
        };
        if !image {
            anyhow::ensure!(
                self.catalog.get_track(id)?.is_some(),
                "track no longer exists"
            );
        }
        record.previous = self.current_uri(&record)?;
        save(&pending_path(&self.root, &record.revision), &record)?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(&record.staging))?;
        Ok(StagedMedia {
            record,
            root: self.root.clone(),
            handed_off: false,
        })
    }
    pub fn commit_publication(&self, mut staged: StagedMedia) -> Result<CopyReceipt> {
        let _guard = self.mutations.lock().unwrap();
        anyhow::ensure!(
            staged.root == self.root,
            "staging belongs to another manager"
        );
        let (file, _) = local::open_media_file_beneath(&self.root, &staged.record.staging)?;
        anyhow::ensure!(file.metadata()?.len() > 0, "empty publication");
        if staged.record.image {
            let bytes = std::fs::read(staged.path())?;
            anyhow::ensure!(
                infer::get(&bytes).is_some_and(|kind| kind.mime_type().starts_with("image/")),
                "invalid image"
            );
        }
        file.sync_all()?;
        anyhow::ensure!(
            self.current_uri(&staged.record)? == staged.record.previous,
            "media revision changed during preparation"
        );
        staged.record.phase = Phase::Ready;
        save(
            &pending_path(&self.root, &staged.record.revision),
            &staged.record,
        )?;
        staged.handed_off = true;
        if let Err(error) = self.finish_publication(&mut staged.record) {
            if staged.record.phase != Phase::Attached
                || self.current_uri(&staged.record)? != Some(staged.record.uri.clone())
            {
                return Err(error);
            }
            staged.record.pending_effects = true;
            warn!(revision = %staged.record.revision, %error, "Media committed with pending recovery work");
        }
        Ok(staged.record.clone())
    }
    fn finish_publication(&self, record: &mut CopyReceipt) -> Result<()> {
        let pending = pending_path(&self.root, &record.revision);
        if record.phase == Phase::Ready {
            if self.current_uri(record)? != Some(record.uri.clone()) {
                if self.current_uri(record)? != record.previous {
                    // This generation never became current. Clean only its owned paths.
                    unlink(&self.root.join(&record.staging))?;
                    unlink(&self.root.join(&record.uri))?;
                    unlink(&pending)?;
                    anyhow::bail!("superseded publication");
                }
                let destination = self.root.join(&record.uri);
                if !destination.exists() {
                    std::fs::rename(self.root.join(&record.staging), &destination)?;
                    sync_directory(destination.parent().unwrap())?;
                    sync_directory(self.root.join(&record.staging).parent().unwrap())?;
                }
                local::open_media_file_beneath(&self.root, &record.uri)?;
                if record.image {
                    save(&image_pointer(&self.root, &record.media_id), record)?;
                } else {
                    anyhow::ensure!(
                        self.catalog.compare_exchange_audio(
                            &record.media_id,
                            record.previous.as_deref(),
                            Some(&record.uri)
                        )?,
                        "catalog attachment changed"
                    );
                }
            }
            record.phase = Phase::Attached;
            save(&copy_path(&self.root, &record.revision), record)?;
            save(&pending, record)?;
        }
        if self.current_uri(record)? == Some(record.uri.clone()) {
            self.apply_effects(record, true)?;
        }
        unlink(&self.root.join(&record.staging))?;
        unlink(&pending)?;
        Ok(())
    }
    fn apply_effects(&self, record: &CopyReceipt, present: bool) -> Result<()> {
        if record.image {
            return Ok(());
        }
        let effects = self
            .effects
            .get()
            .context("media secondary stores not configured; operation remains pending")?;
        use crate::search::{HashedItemType, SearchIndexItem};
        let Some(resolved) = self.catalog.get_resolved_track(&record.media_id)? else {
            return Ok(());
        };
        let mut available = Vec::new();
        let mut missing = Vec::new();
        if present {
            available.push(SearchIndexItem {
                id: record.media_id.clone(),
                name: resolved.track.name.clone(),
                item_type: HashedItemType::Track,
                additional_text: resolved
                    .artists
                    .iter()
                    .map(|a| format!("artist:{}", a.artist.name))
                    .chain(std::iter::once(format!("album:{}", resolved.album.name)))
                    .collect(),
            });
        } else {
            missing.push((record.media_id.clone(), HashedItemType::Track));
        }
        if let Some(album) = self.catalog.get_resolved_album(&resolved.album.id)? {
            if album.album.album_availability != crate::catalog_store::AlbumAvailability::Missing {
                available.push(SearchIndexItem {
                    id: album.album.id.clone(),
                    name: album.album.name,
                    item_type: HashedItemType::Album,
                    additional_text: album
                        .artists
                        .iter()
                        .map(|a| format!("artist:{}", a.name))
                        .collect(),
                });
            } else {
                missing.push((album.album.id, HashedItemType::Album));
            }
            for artist in album.artists {
                if artist.available {
                    available.push(SearchIndexItem {
                        id: artist.id,
                        name: artist.name,
                        item_type: HashedItemType::Artist,
                        additional_text: artist
                            .genres
                            .iter()
                            .map(|g| format!("extra:{g}"))
                            .collect(),
                    });
                } else {
                    missing.push((artist.id, HashedItemType::Artist));
                }
            }
        }
        effects.search.publish_newly_available(&available)?;
        effects.search.unpublish_proxy_items(&missing)?;
        if record.phase != Phase::Observing {
            if let Some(server) = &effects.server {
                if present {
                    match record.provenance {
                        Provenance::Proxy { materialized_at } => server
                            .record_proxy_materialization(&record.media_id, materialized_at)?,
                        _ => {
                            server.delete_proxy_materialization(&record.media_id)?;
                        }
                    }
                } else {
                    server.delete_proxy_materialization(&record.media_id)?;
                }
            }
        }
        Ok(())
    }
    pub fn proxy_copy(&self, id: &str, materialized_at: i64) -> Result<Option<CopyReceipt>> {
        let _guard = self.mutations.lock().unwrap();
        let Some(uri) = self
            .catalog
            .get_track(id)?
            .and_then(|track| track.audio_uri)
        else {
            return Ok(None);
        };
        // Only manager-created immutable generations establish ownership. Legacy
        // filename prefixes and old retention records are never deletion authority.
        let Some(revision) = Path::new(&uri)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('.').next())
        else {
            return Ok(None);
        };
        if uuid::Uuid::parse_str(revision).is_err() {
            return Ok(None);
        }
        let record = read_record(&copy_path(&self.root, revision))?;
        Ok((record.uri == uri
            && record.media_id == id
            && record.provenance == Provenance::Proxy { materialized_at })
        .then_some(record))
    }
    pub fn retain_copy(&self, receipt: &CopyReceipt) -> Result<()> {
        uuid::Uuid::parse_str(&receipt.revision)?;
        let _guard = self.mutations.lock().unwrap();
        let mut record = read_record(&copy_path(&self.root, &receipt.revision))?;
        anyhow::ensure!(
            record.uri == receipt.uri && self.current_uri(&record)? == Some(record.uri.clone()),
            "stale retention decision"
        );
        record.provenance = Provenance::Retained;
        save(&copy_path(&self.root, &record.revision), &record)?;
        if let Some(server) = self
            .effects
            .get()
            .and_then(|effects| effects.server.as_ref())
        {
            server.delete_proxy_materialization(&record.media_id)?;
        }
        Ok(())
    }
    pub fn remove_copy(&self, receipt: &CopyReceipt) -> Result<bool> {
        uuid::Uuid::parse_str(&receipt.revision)?;
        let _guard = self.mutations.lock().unwrap();
        let mut record = match read_record(&copy_path(&self.root, &receipt.revision)) {
            Ok(record) => record,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                return Ok(false)
            }
            Err(error) => return Err(error),
        };
        if record.uri != receipt.uri || record.media_id != receipt.media_id {
            return Ok(false);
        }
        if !matches!(
            record.provenance,
            Provenance::Proxy { .. } | Provenance::ImageCache
        ) || self.current_uri(&record)? != Some(record.uri.clone())
        {
            return Ok(false);
        }
        record.phase = Phase::Removing;
        save(&pending_path(&self.root, &record.revision), &record)?;
        self.finish_removal(&mut record)?;
        Ok(true)
    }
    fn finish_removal(&self, record: &mut CopyReceipt) -> Result<()> {
        if record.phase == Phase::Removing {
            if record.image {
                if self.current_uri(record)? == Some(record.uri.clone()) {
                    unlink(&image_pointer(&self.root, &record.media_id))?;
                }
            } else {
                self.catalog
                    .compare_exchange_audio(&record.media_id, Some(&record.uri), None)?;
            }
            record.phase = Phase::Detached;
            save(&pending_path(&self.root, &record.revision), record)?;
        }
        // Never remove a newer revision; this locator is unique and manager-owned.
        if self.current_uri(record)?.is_none() {
            self.apply_effects(record, false)?;
        }
        unlink(&self.root.join(&record.uri))?;
        unlink(&copy_path(&self.root, &record.revision))?;
        unlink(&pending_path(&self.root, &record.revision))
    }
    pub fn recover(&self, cancelled: &(dyn Fn() -> bool + Send + Sync)) -> Result<usize> {
        let _guard = self.mutations.lock().unwrap();
        let directory = self.root.join(".media/pending");
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        let mut paths = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        let cursor = self.recovery_cursor.lock().unwrap().clone();
        let start = cursor
            .as_ref()
            .map(|cursor| {
                paths.partition_point(|path| path.to_string_lossy().as_ref() <= cursor.as_str())
            })
            .unwrap_or(0);
        if start == paths.len() {
            *self.recovery_cursor.lock().unwrap() = None;
        }
        let start = if start == paths.len() { 0 } else { start };
        let mut recovered = 0;
        let mut failure = None;
        for path in paths.into_iter().skip(start).take(1000) {
            anyhow::ensure!(!cancelled(), "cancelled");
            *self.recovery_cursor.lock().unwrap() = Some(path.to_string_lossy().into_owned());
            let outcome = (|| -> Result<bool> {
                let mut record = read_record(&path)?;
                anyhow::ensure!(
                    path.file_stem().and_then(|s| s.to_str()) == Some(record.revision.as_str()),
                    "journal identity mismatch"
                );
                match record.phase {
                    Phase::Writing => {
                        if record.owner == process_epoch() {
                            return Ok(false);
                        }
                        unlink(&self.root.join(&record.staging))?;
                        unlink(&path)?;
                    }
                    Phase::Observing => {
                        self.finish_observation(&record)?;
                    }
                    Phase::Ready | Phase::Attached => self.finish_publication(&mut record)?,
                    Phase::Removing | Phase::Detached => self.finish_removal(&mut record)?,
                }
                Ok(true)
            })();
            match outcome {
                Ok(true) => recovered += 1,
                Ok(false) => {}
                Err(_) if !path.exists() => recovered += 1, // superseded owned intent was safely aborted
                Err(error) => {
                    warn!(%error, path = %path.display(), "Pending media operation could not recover");
                    if failure.is_none() {
                        failure = Some(error);
                    }
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(recovered)
    }
    pub(super) fn observe_missing_batch(
        &self,
        observations: &[(String, Option<String>, bool)],
        cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<crate::catalog_store::AvailabilityRefreshResult> {
        prepare_directory(&self.root, ".media/pending")?;
        let mut records = Vec::new();
        for (id, uri, _) in observations {
            anyhow::ensure!(!cancelled(), "cancelled");
            let record = CopyReceipt {
                revision: uuid::Uuid::new_v4().to_string(),
                media_id: id.to_owned(),
                uri: uri.clone().unwrap_or_default(),
                previous: uri.clone(),
                image: false,
                phase: Phase::Observing,
                staging: String::new(),
                provenance: Provenance::Retained,
                pending_effects: false,
                owner: process_epoch().to_owned(),
            };
            save(&pending_path(&self.root, &record.revision), &record)?;
            records.push(record);
        }
        let refreshed = self
            .catalog
            .apply_media_observations(observations, cancelled)?;
        for record in records {
            if self.current_uri(&record)? == record.previous {
                self.apply_effects(&record, false)?;
            }
            unlink(&pending_path(&self.root, &record.revision))?;
        }
        Ok(refreshed)
    }
    fn finish_observation(
        &self,
        record: &CopyReceipt,
    ) -> Result<crate::catalog_store::AvailabilityRefreshResult> {
        let mut result = crate::catalog_store::AvailabilityRefreshResult::default();
        if self.current_uri(record)? == record.previous {
            let present = match super::probe(&self.root, record.previous.as_deref()) {
                super::MediaPresence::Present => true,
                super::MediaPresence::Missing => false,
                super::MediaPresence::Unknown(error) => anyhow::bail!(error),
            };
            result = self.catalog.apply_media_observations(
                &[(record.media_id.clone(), record.previous.clone(), present)],
                &|| false,
            )?;
            self.apply_effects(record, present)?;
        }
        unlink(&pending_path(&self.root, &record.revision))?;
        Ok(result)
    }
    pub(super) fn image_path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        match read_record(&image_pointer(&self.root, id)) {
            Ok(record) => Ok(self.root.join(record.uri)),
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                Ok(self.root.join("images").join(format!("{id}.jpg")))
            }
            Err(error) => Err(error),
        }
    }
}
