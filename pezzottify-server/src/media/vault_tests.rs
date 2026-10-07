use super::{tests::Fixture, vault::*, *};
use async_trait::async_trait;
use futures::{stream, StreamExt};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[derive(Default)]
struct MemoryAdapter {
    objects: Mutex<HashMap<String, Vec<u8>>>,
    offline: AtomicBool,
    on_read: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
#[async_trait]
impl VaultAdapter for MemoryAdapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            publish: true,
            delete: true,
            presence: true,
            ranges: false,
        }
    }
    async fn read(&self, request: ReadRequest) -> AdapterResult<AdapterRead> {
        if self.offline.load(Ordering::SeqCst) {
            return Err(AdapterError::Unreachable("device offline".into()));
        }
        let bytes = self
            .objects
            .lock()
            .unwrap()
            .get(&request.locator)
            .cloned()
            .ok_or(AdapterError::Missing)?;
        let hook = self.on_read.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(AdapterRead {
            metadata: ReadMetadata {
                content_length: bytes.len() as u64,
                content_type: "audio/ogg".into(),
                extension: "ogg".into(),
            },
            stream: Box::pin(stream::once(async { Ok(Bytes::from(bytes)) })),
        })
    }
    async fn publish(&self, locator: &str, mut stream: MediaStream) -> AdapterResult<()> {
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.map_err(|e| AdapterError::Storage(e.to_string()))?);
        }
        self.objects.lock().unwrap().insert(locator.into(), bytes);
        Ok(())
    }
    async fn delete(&self, locator: &str) -> AdapterResult<()> {
        if self.offline.load(Ordering::SeqCst) {
            return Err(AdapterError::Unreachable("device offline".into()));
        }
        self.objects.lock().unwrap().remove(locator);
        Ok(())
    }
    async fn presence(&self, locator: &str) -> Presence {
        if self.offline.load(Ordering::SeqCst) {
            Presence::Unreachable("offline".into())
        } else if self.objects.lock().unwrap().contains_key(locator) {
            Presence::Present
        } else {
            Presence::Missing
        }
    }
}
fn fixture() -> Fixture {
    let f = Fixture::new();
    f.manager
        .configure_search(Arc::new(crate::search::NoopSearchVault));
    f
}
fn publish(f: &Fixture, data: &[u8]) -> CopyReceipt {
    let stage = f
        .manager
        .begin_publication("track1", "ogg", Provenance::Proxy { materialized_at: 1 })
        .unwrap();
    std::fs::write(stage.path(), data).unwrap();
    f.manager.commit_publication(stage).unwrap()
}
async fn cache(
    f: &Fixture,
    name: &str,
    authority: &MediaCopy,
    representation: &str,
) -> (MediaCopy, Arc<MemoryAdapter>) {
    let adapter = Arc::new(MemoryAdapter::default());
    let vault = Vault {
        id: VaultId(name.into()),
        role: VaultRole::Cache,
    };
    f.manager
        .register_vault(vault.clone(), adapter.clone())
        .unwrap();
    let id = CopyId(uuid::Uuid::new_v4().to_string());
    let copy = MediaCopy {
        id: id.clone(),
        media: authority.media.clone(),
        version: authority.version.clone(),
        representation: RepresentationId(representation.into()),
        vault: vault.id,
        locator: id.0,
        source: Some(SourceReference {
            vault: authority.vault.clone(),
            media: authority.media.clone(),
            version: authority.version.clone(),
            locator: None,
        }),
        protected: false,
    };
    adapter
        .publish(
            &copy.locator,
            Box::pin(stream::once(async { Ok(Bytes::from_static(b"cached")) })),
        )
        .await
        .unwrap();
    f.manager.register_cache_copy(&copy).unwrap();
    (copy, adapter)
}
#[tokio::test]
async fn replacement_invalidates_all_representations_across_vaults_and_restart() {
    let f = fixture();
    let old = publish(&f, b"old").copy.unwrap();
    let (a, adapter_a) = cache(&f, "device-a", &old, "ogg").await;
    let (b, adapter_b) = cache(&f, "device-b", &old, "mp3").await;
    assert!(f.manager.read_cache_copy(a.id.clone(), None).await.is_ok());
    adapter_b.offline.store(true, Ordering::SeqCst);
    let new = publish(&f, b"new").copy.unwrap();
    assert_ne!(new.version, old.version);
    assert!(matches!(
        f.manager.read_cache_copy(a.id.clone(), None).await,
        Err(AdapterError::Stale)
    ));
    assert!(matches!(
        f.manager.read_cache_copy(b.id.clone(), None).await,
        Err(AdapterError::Stale)
    ));
    assert_eq!(f.manager.cleanup_invalidated_caches().await.unwrap(), 1);
    assert!(adapter_a.objects.lock().unwrap().is_empty());
    assert!(!adapter_b.objects.lock().unwrap().is_empty());
    let restarted = Arc::new(MediaManager::new(
        f.manager.catalog.clone(),
        DbExecutor::new(Default::default()),
    ));
    restarted
        .register_vault(
            Vault {
                id: b.vault.clone(),
                role: VaultRole::Cache,
            },
            adapter_b.clone(),
        )
        .unwrap();
    adapter_b.offline.store(false, Ordering::SeqCst);
    assert!(matches!(
        restarted.read_cache_copy(b.id.clone(), None).await,
        Err(AdapterError::Stale)
    ));
    assert_eq!(restarted.cleanup_invalidated_caches().await.unwrap(), 1);
    assert!(adapter_b.objects.lock().unwrap().is_empty());
    assert!(std::fs::read(f.root.path().join(new.locator)).is_ok());
}
#[tokio::test]
async fn authoritative_deletion_invalidates_caches_but_cache_eviction_preserves_authority() {
    let f = fixture();
    let receipt = publish(&f, b"normal audio");
    let authority = receipt.copy.clone().unwrap();
    assert_eq!(authority.vault.0, LOCAL_AUDIO);
    let (a, _) = cache(&f, "device-a", &authority, "ogg").await;
    let (b, _) = cache(&f, "device-b", &authority, "ogg").await;
    assert_ne!(a.id, b.id);
    assert_eq!(a.representation, b.representation);
    f.manager.evict_cache_copy(a.id.clone()).await.unwrap();
    f.manager.evict_cache_copy(a.id.clone()).await.unwrap();
    assert!(f.manager.authoritative_audio("track1").unwrap().is_some());
    assert!(f.manager.register_cache_copy(&a).is_err());
    assert!(f.manager.read_cache_copy(b.id.clone(), None).await.is_ok());
    assert!(f.manager.remove_copy(&receipt).unwrap());
    assert!(matches!(
        f.manager.read_cache_copy(b.id, None).await,
        Err(AdapterError::Stale)
    ));
}
#[tokio::test]
async fn unsupported_and_unreachable_are_distinct_from_missing() {
    let f = fixture();
    let authority = publish(&f, b"audio").copy.unwrap();
    let (copy, adapter) = cache(&f, "device", &authority, "ogg").await;
    assert!(matches!(
        f.manager
            .read_cache_copy(
                copy.id.clone(),
                Some(ByteRange {
                    start: 0,
                    length: 1
                })
            )
            .await,
        Err(AdapterError::Unsupported(_))
    ));
    adapter.offline.store(true, Ordering::SeqCst);
    assert!(matches!(
        adapter.presence(&copy.locator).await,
        Presence::Unreachable(_)
    ));
    assert!(matches!(
        f.manager.read_cache_copy(copy.id.clone(), None).await,
        Err(AdapterError::Unreachable(_))
    ));
    adapter.offline.store(false, Ordering::SeqCst);
    adapter.objects.lock().unwrap().clear();
    assert_eq!(adapter.presence(&copy.locator).await, Presence::Missing);
    assert!(matches!(
        f.manager.read_cache_copy(copy.id, None).await,
        Err(AdapterError::Missing)
    ));
    let http = adapters::HttpImageAdapter::default();
    assert!(matches!(
        http.delete("anything").await,
        Err(AdapterError::Unsupported("delete"))
    ));
}
#[test]
fn content_representation_and_copy_identity_are_independent_and_legacy_is_protected() {
    let f = fixture();
    assert!(f.manager.authoritative_audio("track1").unwrap().is_none());
    assert!(f
        .manager
        .open_local_audio_blocking("track1")
        .unwrap()
        .is_some());
    let a = publish(&f, b"identical").copy.unwrap();
    let b = publish(&f, b"identical").copy.unwrap();
    assert_eq!(a.representation, b.representation);
    assert_ne!(a.id, b.id);
    assert_ne!(a.version, b.version);
    assert!(a.protected);
    assert!(a.source.is_none());
    let roles = f.manager.vaults();
    assert!(roles
        .iter()
        .any(|v| v.id.0 == LOCAL_AUDIO && v.role == VaultRole::Authoritative));
    assert!(roles
        .iter()
        .any(|v| v.id.0 == LOCAL_IMAGES && v.role == VaultRole::Cache));
    assert!(f
        .manager
        .register_vault(roles[0].clone(), Arc::new(MemoryAdapter::default()))
        .is_err());
}
#[tokio::test]
async fn local_adapter_preserves_ranges_atomic_publication_and_root_confinement() {
    let root = tempfile::tempdir().unwrap();
    let adapter = adapters::FilesystemAdapter::new(root.path().to_owned());
    let data =
        || Box::pin(stream::once(async { Ok(Bytes::from_static(b"012345")) })) as MediaStream;
    adapter.publish("audio/copy.ogg", data()).await.unwrap();
    assert!(adapter.publish("audio/copy.ogg", data()).await.is_err());
    let mut read = adapter
        .read(ReadRequest {
            locator: "audio/copy.ogg".into(),
            range: Some(ByteRange {
                start: 2,
                length: 3,
            }),
            priority: DownloadPriority::Normal,
        })
        .await
        .unwrap();
    assert_eq!(read.metadata.content_length, 3);
    assert_eq!(
        read.stream.next().await.unwrap().unwrap(),
        Bytes::from_static(b"234")
    );
    assert!(adapter.publish("../escape", data()).await.is_err());
    adapter.delete("audio/copy.ogg").await.unwrap();
    adapter.delete("audio/copy.ogg").await.unwrap();
    assert_eq!(adapter.presence("audio/copy.ogg").await, Presence::Missing);
}

#[tokio::test]
async fn replacement_during_cache_open_is_rejected_and_availability_is_not_deletion() {
    let f = fixture();
    let authority = publish(&f, b"old").copy.unwrap();
    let (copy, adapter) = cache(&f, "device", &authority, "ogg").await;
    // Persisted availability alone is not an authoritative content change. In
    // particular an unknown physical observation must not evict dependent media.
    rusqlite::Connection::open(f.root.path().join("catalog.db"))
        .unwrap()
        .execute("UPDATE tracks SET track_available=0 WHERE id='track1'", [])
        .unwrap();
    assert!(f
        .manager
        .read_cache_copy(copy.id.clone(), None)
        .await
        .is_ok());
    let manager = f.manager.clone();
    *adapter.on_read.lock().unwrap() = Some(Box::new(move || {
        let stage = manager
            .begin_publication("track1", "ogg", Provenance::Ingested)
            .unwrap();
        std::fs::write(stage.path(), b"replacement").unwrap();
        manager.commit_publication(stage).unwrap();
    }));
    assert!(matches!(
        f.manager.read_cache_copy(copy.id, None).await,
        Err(AdapterError::Stale)
    ));
}

#[tokio::test]
async fn explicit_authoritative_deletion_handles_ingested_media_and_stale_requests() {
    let f = fixture();
    let old = publish(&f, b"old").copy.unwrap();
    let stage = f
        .manager
        .begin_publication("track1", "ogg", Provenance::Ingested)
        .unwrap();
    std::fs::write(stage.path(), b"ingested").unwrap();
    let receipt = f.manager.commit_publication(stage).unwrap();
    let current = receipt.copy.clone().unwrap();
    let (copy, _) = cache(&f, "device", &current, "ogg").await;
    assert!(!f.manager.remove_copy(&receipt).unwrap());
    assert!(!f.manager.delete_authoritative_audio(&old).unwrap());
    assert!(f
        .manager
        .read_cache_copy(copy.id.clone(), None)
        .await
        .is_ok());
    assert!(f.manager.delete_authoritative_audio(&current).unwrap());
    assert!(!f.manager.delete_authoritative_audio(&current).unwrap());
    assert!(matches!(
        f.manager.read_cache_copy(copy.id, None).await,
        Err(AdapterError::Stale)
    ));
}

#[tokio::test]
async fn absent_filesystem_root_is_unreachable_not_missing_content() {
    let root = tempfile::tempdir().unwrap();
    let adapter = adapters::FilesystemAdapter::new(root.path().join("offline-volume"));
    assert!(matches!(
        adapter.presence("audio/track.ogg").await,
        Presence::Unreachable(_)
    ));
    assert!(matches!(
        adapter
            .read(ReadRequest {
                locator: "audio/track.ogg".into(),
                range: None,
                priority: DownloadPriority::Normal
            })
            .await,
        Err(AdapterError::Unreachable(_))
    ));
}
