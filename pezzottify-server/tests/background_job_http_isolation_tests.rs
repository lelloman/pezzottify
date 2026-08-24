//! Synthetic mixed-load contract for scheduler/API resource isolation.
#![allow(dead_code)]

mod common;

use common::{TestClient, TestServer, TRACK_1_ID};
use pezzottify_server::background_jobs::{
    BackgroundJob, JobContext, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule,
};
use reqwest::StatusCode;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

struct SyntheticCpuJob {
    started: Arc<AtomicBool>,
}

impl BackgroundJob for SyntheticCpuJob {
    fn id(&self) -> &'static str {
        "synthetic_cpu_load"
    }

    fn name(&self) -> &'static str {
        "Synthetic CPU Load"
    }

    fn description(&self) -> &'static str {
        "Exercises HTTP responsiveness while bounded background work is active"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Manual
    }

    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::CpuBound).with_max_runtime(Duration::from_secs(5))
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        self.started.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            for value in 0..25_000 {
                std::hint::black_box(value * value);
            }
            std::thread::yield_now();
        }
        Ok(())
    }
}

#[tokio::test]
async fn user_facing_http_stays_responsive_during_cpu_bound_job() {
    let started = Arc::new(AtomicBool::new(false));
    let server = TestServer::builder()
        .with_available_catalog()
        .with_scheduler_job(Arc::new(SyntheticCpuJob {
            started: started.clone(),
        }))
        .spawn()
        .await;
    let admin = Arc::new(TestClient::authenticated_admin(server.base_url.clone()).await);

    assert_eq!(
        admin.admin_trigger_job("synthetic_cpu_load").await.status(),
        StatusCode::ACCEPTED
    );
    for _ in 0..100 {
        if started.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(started.load(Ordering::SeqCst));

    let mut requests = Vec::new();
    for _ in 0..12 {
        let client = admin.clone();
        requests.push(tokio::spawn(async move {
            let request_started = Instant::now();
            let response = client
                .stream_track_with_range(TRACK_1_ID, "bytes=0-31")
                .await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.bytes().await.unwrap().len(), 32);
            request_started.elapsed()
        }));
    }

    for request in requests {
        let elapsed = tokio::time::timeout(Duration::from_secs(3), request)
            .await
            .expect("streaming timed out during bounded background work")
            .unwrap();
        assert!(elapsed < Duration::from_secs(2), "request took {elapsed:?}");
    }
}
