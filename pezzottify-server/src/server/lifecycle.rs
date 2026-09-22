//! Application-owned upgraded connections, request tasks, and maintenance.
use std::future::Future;

use simple_server::lifecycle::{Lifecycle, Shutdown};
use simple_server::tasks::{AdmissionError, WorkGuard, WorkTracker};

/// Shared ownership for tasks whose results are handled at their call sites.
#[derive(Clone, Default)]
pub struct RuntimeWork(WorkTracker);

impl RuntimeWork {
    pub fn token(&self) -> Result<WorkGuard, AdmissionError> {
        self.0.try_acquire("HTTP upgrade")
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        match self.0.try_acquire("application task") {
            Ok(guard) => {
                tokio::spawn(async move {
                    let _guard = guard;
                    future.await;
                });
            }
            Err(error) => tracing::warn!(%error, "Application task rejected during drain"),
        }
    }

    fn close(&self) {
        self.0.close();
    }
    async fn wait(&self) {
        self.0.wait().await;
    }
}

#[derive(Clone, Default)]
pub struct RuntimeTasks {
    pub shutdown: Shutdown,
    pub tasks: RuntimeWork,
}

impl RuntimeTasks {
    pub fn new(shutdown: Shutdown) -> Self {
        Self {
            shutdown,
            tasks: RuntimeWork::default(),
        }
    }

    /// Run only after HTTP services have drained: upgrade callbacks already own
    /// a token, including upgrades that have not completed their handshake yet.
    pub async fn drain(self) -> std::io::Result<()> {
        self.tasks.close();
        self.tasks.wait().await;
        Ok(())
    }
}

pub(super) fn maintenance(
    lifecycle: &mut Option<&mut Lifecycle<'static>>,
    name: &'static str,
    future: impl Future<Output = std::io::Result<()>> + Send + 'static,
) -> anyhow::Result<()> {
    if let Some(lifecycle) = lifecycle {
        lifecycle.service(name, future)?;
    } else {
        // Router-only callers retain their existing runtime-owned maintenance.
        tokio::spawn(future);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple_server::lifecycle::{ShutdownOptions, ShutdownReason};
    use std::{io, time::Duration};

    #[tokio::test]
    async fn drain_waits_for_upgrade_reserved_before_http_finishes() {
        let mut lifecycle = Lifecycle::new(ShutdownOptions {
            grace_period: Duration::from_secs(1),
        });
        let tasks = RuntimeTasks::new(lifecycle.shutdown());
        // As in on_upgrade, reserve ownership before the callback starts.
        let token = tasks.tasks.token().unwrap();
        let shutdown = lifecycle.shutdown();
        lifecycle
            .service("http", async move {
                shutdown.requested().await;
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let drained = lifecycle.run(
            async { Ok::<_, io::Error>(ShutdownReason::Requested) },
            tasks.drain(),
        );
        tokio::pin!(drained);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut drained)
                .await
                .is_err()
        );
        drop(token);
        assert!(drained.await.is_ok());
    }

    #[tokio::test]
    async fn outstanding_application_work_is_bounded_by_shutdown_deadline() {
        let mut lifecycle = Lifecycle::new(ShutdownOptions {
            grace_period: Duration::from_millis(20),
        });
        let tasks = RuntimeTasks::new(lifecycle.shutdown());
        let _token = tasks.tasks.token().unwrap();
        let shutdown = lifecycle.shutdown();
        lifecycle
            .service("http", async move {
                shutdown.requested().await;
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let error = lifecycle
            .run(
                async { Ok::<_, io::Error>(ShutdownReason::Requested) },
                tasks.drain(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:?}").contains("Cleanup"));
    }
    #[tokio::test]
    async fn closed_runtime_work_rejects_late_upgrades_and_spawns() {
        let tasks = RuntimeTasks::default();
        tasks.tasks.close();
        assert!(matches!(tasks.tasks.token(), Err(AdmissionError::Closed)));
        let (sent, received) = tokio::sync::oneshot::channel();
        tasks.tasks.spawn(async move {
            let _ = sent.send(());
        });
        assert!(received.await.is_err(), "rejected task must never run");
        tasks.drain().await.unwrap();
    }

    #[tokio::test]
    async fn panicking_request_task_releases_its_shared_work_guard() {
        let tasks = RuntimeTasks::default();
        tasks.tasks.spawn(async {
            panic!("request task panic");
        });
        tokio::time::timeout(Duration::from_secs(1), tasks.drain())
            .await
            .unwrap()
            .unwrap();
    }
}
