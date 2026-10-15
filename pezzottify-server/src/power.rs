//! Optional local activity admission. A guard survives until the work really ends.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Client {
    runtime: tokio::runtime::Handle,
    http: reqwest::Client,
    token: String,
    service: String,
    instance: String,
    initialized: AtomicBool,
    healthy: AtomicBool,
    sequence: AtomicU64,
    schedules_synced: AtomicBool,
    worker_synced: AtomicBool,
    media_probe: AtomicBool,
    wanted_epoch: AtomicU64,
    checked_epoch: AtomicU64,
    probe_notify: tokio::sync::Notify,
}

static CLIENT: OnceLock<Arc<Client>> = OnceLock::new();

impl Client {
    fn endpoint(&self, suffix: &str) -> String {
        format!("http://localhost/services/{}{}", self.service, suffix)
    }

    async fn call(
        &self,
        method: reqwest::Method,
        suffix: &str,
        body: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.http
            .request(method, self.endpoint(suffix))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn report(&self) -> anyhow::Result<()> {
        self.call(reqwest::Method::POST, "/activity", serde_json::json!({
            "ready": self.initialized.load(Ordering::Acquire) && self.healthy.load(Ordering::Acquire) && self.schedules_synced.load(Ordering::Acquire) && self.worker_synced.load(Ordering::Acquire),
            "instance": self.instance,
        })).await
    }
}

pub struct Guard {
    client: Arc<Client>,
    key: String,
    renewal: Option<tokio::task::JoinHandle<()>>,
    runtime: tokio::runtime::Handle,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(task) = self.renewal.take() {
            task.abort();
        }
        let client = self.client.clone();
        let key = self.key.clone();
        self.runtime.spawn(async move {
            for attempt in 0..3 {
                if client
                    .call(
                        reqwest::Method::DELETE,
                        &format!("/leases/{key}"),
                        serde_json::Value::Null,
                    )
                    .await
                    .is_ok()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1 + attempt)).await;
            }
            tracing::error!(
                "Activity release failed; node agent retains the guard until reconciliation"
            );
        });
    }
}

pub fn enabled() -> bool {
    CLIENT.get().is_some()
}

pub async fn initialize() -> anyhow::Result<Option<Guard>> {
    let Some(socket) = std::env::var_os("POWER_AGENT_SOCKET") else {
        return Ok(None);
    };
    let service = std::env::var("POWER_SERVICE")?;
    anyhow::ensure!(
        !service.is_empty()
            && service.len() <= 100
            && service
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)),
        "Invalid POWER_SERVICE"
    );
    let token = std::fs::read_to_string(std::env::var("POWER_TOKEN_FILE")?)?
        .trim()
        .to_owned();
    anyhow::ensure!(token.len() >= 32, "Invalid power token");
    let client = Arc::new(Client {
        runtime: tokio::runtime::Handle::current(),
        http: reqwest::Client::builder()
            .unix_socket(std::path::PathBuf::from(socket))
            .timeout(Duration::from_secs(5))
            .build()?,
        token,
        service,
        instance: format!(
            "{}.{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ),
        initialized: AtomicBool::new(false),
        healthy: AtomicBool::new(true),
        sequence: AtomicU64::new(0),
        schedules_synced: AtomicBool::new(
            std::env::var("POWER_REQUIRE_SCHEDULES").as_deref() != Ok("1"),
        ),
        worker_synced: AtomicBool::new(std::env::var("ALBUM_WORKER_ENABLED").as_deref() != Ok("1")),
        media_probe: AtomicBool::new(false),
        wanted_epoch: AtomicU64::new(0),
        checked_epoch: AtomicU64::new(0),
        probe_notify: tokio::sync::Notify::new(),
    });
    client.report().await?;
    anyhow::ensure!(
        CLIENT.set(client.clone()).is_ok(),
        "Power client already initialized"
    );
    let startup = acquire(true).await?;
    tokio::spawn(async move {
        loop {
            if let Err(error) = client.report().await {
                tracing::warn!(%error, "Activity heartbeat unavailable");
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
    Ok(startup)
}

pub async fn acquire(demand: bool) -> anyhow::Result<Option<Guard>> {
    let Some(client) = CLIENT.get() else {
        return Ok(None);
    };
    acquire_for(client, demand).await.map(Some)
}

async fn acquire_for(client: &Arc<Client>, demand: bool) -> anyhow::Result<Guard> {
    let key = format!(
        "{}.{}",
        client.instance,
        client.sequence.fetch_add(1, Ordering::Relaxed)
    );
    let body = serde_json::json!({"kind": if demand { "demand" } else { "work" }, "wake": false});
    let mut guard = Guard {
        client: client.clone(),
        key: key.clone(),
        renewal: None,
        // Background jobs can enter a temporary runtime. Guard renewal and
        // release must survive that runtime, including on errors/cancellation.
        runtime: client.runtime.clone(),
    };
    client
        .call(
            reqwest::Method::PUT,
            &format!("/leases/{key}"),
            body.clone(),
        )
        .await?;
    let renew_client = client.clone();
    let renew_key = key.clone();
    let renewal = client.runtime.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Err(error) = renew_client
                .call(
                    reqwest::Method::PUT,
                    &format!("/leases/{renew_key}"),
                    body.clone(),
                )
                .await
            {
                // Local guards never expire into "idle". The host inhibitor
                // remains even if a controller lease expires during an outage.
                tracing::warn!(%error, "Activity renewal unavailable");
            }
        }
    });
    guard.renewal = Some(renewal);
    Ok(guard)
}

pub async fn initialized() -> anyhow::Result<()> {
    if let Some(client) = CLIENT.get() {
        client.initialized.store(true, Ordering::Release);
        client.report().await?;
    }
    Ok(())
}

pub fn set_healthy(healthy: bool) {
    if let Some(client) = CLIENT.get() {
        client.healthy.store(healthy, Ordering::Release);
    }
}

pub(crate) async fn admission_open() -> bool {
    let Some(client) = CLIENT.get() else {
        return true;
    };
    match client
        .http
        .get(client.endpoint(""))
        .bearer_auth(&client.token)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response
            .json::<serde_json::Value>()
            .await
            .is_ok_and(|value| value["fenced"].as_bool() == Some(false)),
        _ => false,
    }
}

pub async fn ready() -> bool {
    let Some(client) = CLIENT.get() else {
        return false;
    };
    if !client.initialized.load(Ordering::Acquire)
        || !client.healthy.load(Ordering::Acquire)
        || !client.schedules_synced.load(Ordering::Acquire)
        || !client.worker_synced.load(Ordering::Acquire)
    {
        return false;
    }
    match client
        .http
        .get(client.endpoint(""))
        .bearer_auth(&client.token)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let Some(value) = response.json::<serde_json::Value>().await.ok() else {
                return false;
            };
            let epoch = value["resume_epoch"].as_u64().unwrap_or(0);
            if client.media_probe.load(Ordering::Acquire)
                && epoch != client.checked_epoch.load(Ordering::Acquire)
            {
                client.wanted_epoch.store(epoch, Ordering::Release);
                client.probe_notify.notify_one();
                return false;
            }
            value["ready"].as_bool().unwrap_or(false)
        }
        _ => false,
    }
}

pub async fn schedule(key: &str, revision: i64, run_at: Option<String>) -> anyhow::Result<()> {
    if let Some(client) = CLIENT.get() {
        let method = if run_at.is_some() {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        };
        client
            .call(
                method,
                &format!("/wakeups/{key}"),
                serde_json::json!({"revision":revision,"run_at":run_at}),
            )
            .await?;
    }
    Ok(())
}

pub async fn schedules_synced(value: bool) {
    if let Some(client) = CLIENT.get() {
        client.schedules_synced.store(value, Ordering::Release);
        if let Err(error) = client.report().await {
            tracing::warn!(%error, "Schedule readiness publication failed");
        }
    }
}

pub(crate) async fn worker_synced(value: bool) {
    if let Some(client) = CLIENT.get() {
        client.worker_synced.store(value, Ordering::Release);
        if let Err(error) = client.report().await {
            tracing::warn!(%error, "Worker readiness publication failed");
        }
    }
}

pub async fn schedule_revision(key: &str) -> anyhow::Result<i64> {
    let Some(client) = CLIENT.get() else {
        return Ok(0);
    };
    let response = client
        .http
        .get(client.endpoint(&format!("/wakeups/{key}")))
        .bearer_auth(&client.token)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(0);
    }
    let value: serde_json::Value = response.error_for_status()?.json().await?;
    value["revision"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("Invalid wake revision"))
}

fn media_probe(root: &std::path::Path, expected: &str, identity: &str) -> anyhow::Result<()> {
    use std::io::Write;
    anyhow::ensure!(
        std::fs::read_to_string(root.join(".export-id"))?.trim() == expected,
        "Media export identity mismatch"
    );
    let managed = root.join(".media");
    let metadata = std::fs::symlink_metadata(&managed)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Invalid managed media directory"
    );
    // A unique write + fsync confirms server-side I/O rather than an NFS cache
    // hit. Only one probe may run, including during an indefinitely blocked I/O.
    let path = managed.join(format!(".health-{identity}"));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(b"pezzottify media health\n")?;
        file.sync_all()?;
        std::fs::remove_file(&path)?;
        std::fs::File::open(&managed)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

pub async fn watch_media(root: std::path::PathBuf) -> anyhow::Result<()> {
    let Some(client) = CLIENT.get() else {
        return Ok(());
    };
    let expected = std::env::var("POWER_MEDIA_EXPORT_ID")?;
    let client = client.clone();
    client.media_probe.store(true, Ordering::Release);
    client.healthy.store(false, Ordering::Release);
    let first_activity = acquire(false).await?;
    let first_root = root.clone();
    let first_expected = expected.clone();
    let first_identity = format!("{}.first", client.instance);
    tokio::task::spawn_blocking(move || {
        let _activity = first_activity;
        media_probe(&first_root, &first_expected, &first_identity)
    })
    .await??;
    client.healthy.store(true, Ordering::Release);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = client.probe_notify.notified() => {},
            }
            let Ok(activity) = acquire(false).await else {
                continue;
            };
            let wanted = client.wanted_epoch.load(Ordering::Acquire);
            let check_root = root.clone();
            let check_expected = expected.clone();
            let identity = format!(
                "{}.{}",
                client.instance,
                client.sequence.fetch_add(1, Ordering::Relaxed)
            );
            let mut check = tokio::task::spawn_blocking(move || {
                let _activity = activity;
                media_probe(&check_root, &check_expected, &identity)
            });
            let result = match tokio::time::timeout(Duration::from_secs(2), &mut check).await {
                Ok(result) => result,
                Err(_) => {
                    client.healthy.store(false, Ordering::Release);
                    let _ = client.report().await;
                    // Do not spawn another blocked NFS operation or release the
                    // guard before the original syscall actually finishes.
                    check.await
                }
            };
            let healthy = matches!(result, Ok(Ok(())));
            client.healthy.store(healthy, Ordering::Release);
            if healthy {
                client.checked_epoch.store(wanted, Ordering::Release);
            }
            let _ = client.report().await;
        }
    });
    Ok(())
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn guard_release_survives_background_job_runtime_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("agent.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let (released_tx, released_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut released_tx = Some(released_tx);
            loop {
                let (mut connection, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let size = connection.read(&mut request).await.unwrap();
                let is_release = request[..size].starts_with(b"DELETE ");
                connection
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
                if is_release {
                    released_tx.take().unwrap().send(()).unwrap();
                    break;
                }
            }
        });
        let client = Arc::new(Client {
            runtime: tokio::runtime::Handle::current(),
            http: reqwest::Client::builder()
                .unix_socket(socket)
                .build()
                .unwrap(),
            token: "test-token".into(),
            service: "test".into(),
            instance: "test".into(),
            initialized: AtomicBool::new(true),
            healthy: AtomicBool::new(true),
            sequence: AtomicU64::new(0),
            schedules_synced: AtomicBool::new(true),
            worker_synced: AtomicBool::new(true),
            media_probe: AtomicBool::new(false),
            wanted_epoch: AtomicU64::new(0),
            checked_epoch: AtomicU64::new(0),
            probe_notify: tokio::sync::Notify::new(),
        });
        tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                let _guard = acquire_for(&client, false).await.unwrap();
                // No further yield: the job's runtime ends as the guard drops.
            });
            drop(runtime);
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(3), released_rx)
            .await
            .unwrap()
            .unwrap();
    }
}
