//! Mixed-workload latency and failure budgets for user-facing backend paths.
#![allow(dead_code)] // The shared integration-test harness exposes more helpers than this crate uses.

mod common;

use common::{TestClient, TestServer, TEST_PASS, TEST_USER, TRACK_1_ID};
use reqwest::StatusCode;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

const AUTH_REQUESTS: usize = 4;
const REQUESTS_PER_INTERACTIVE_PATH: usize = 8;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const P95_BUDGET: Duration = Duration::from_secs(2);
const TOTAL_BUDGET: Duration = Duration::from_secs(10);

struct Sample {
    workload: &'static str,
    elapsed: Duration,
}

fn p95(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    let index = (samples.len() * 95).div_ceil(100).saturating_sub(1);
    samples[index]
}

#[tokio::test]
async fn mixed_user_workload_stays_within_latency_and_failure_budget() {
    let server = TestServer::builder()
        .with_available_catalog()
        .with_ingestion()
        .spawn()
        .await;
    let admin = Arc::new(TestClient::authenticated_admin(server.base_url.clone()).await);
    let task_count = AUTH_REQUESTS + 3 * REQUESTS_PER_INTERACTIVE_PATH;
    let barrier = Arc::new(tokio::sync::Barrier::new(task_count + 1));
    let mut tasks = Vec::with_capacity(task_count);

    for index in 0..AUTH_REQUESTS {
        let base_url = server.base_url.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let started = Instant::now();
            let client = TestClient::new(base_url);
            let response = tokio::time::timeout(
                REQUEST_TIMEOUT,
                client.login_with_device(
                    TEST_USER,
                    TEST_PASS,
                    &format!("mixed-workload-login-{index}"),
                ),
            )
            .await
            .expect("login exceeded its request timeout");
            assert_eq!(response.status(), StatusCode::CREATED);
            Sample {
                workload: "authentication",
                elapsed: started.elapsed(),
            }
        }));
    }

    for _ in 0..REQUESTS_PER_INTERACTIVE_PATH {
        let stream_admin = admin.clone();
        let stream_barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            stream_barrier.wait().await;
            let started = Instant::now();
            let response = tokio::time::timeout(
                REQUEST_TIMEOUT,
                stream_admin.stream_track_with_range(TRACK_1_ID, "bytes=0-31"),
            )
            .await
            .expect("streaming exceeded its request timeout");
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.bytes().await.unwrap().len(), 32);
            Sample {
                workload: "streaming",
                elapsed: started.elapsed(),
            }
        }));

        let sync_admin = admin.clone();
        let sync_barrier = barrier.clone();
        let base_url = server.base_url.clone();
        tasks.push(tokio::spawn(async move {
            sync_barrier.wait().await;
            let started = Instant::now();
            let response = tokio::time::timeout(
                REQUEST_TIMEOUT,
                sync_admin
                    .client
                    .get(format!("{base_url}/v1/sync/state"))
                    .send(),
            )
            .await
            .expect("sync exceeded its request timeout")
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let _: serde_json::Value = response.json().await.unwrap();
            Sample {
                workload: "synchronization",
                elapsed: started.elapsed(),
            }
        }));

        let ingestion_admin = admin.clone();
        let ingestion_barrier = barrier.clone();
        let base_url = server.base_url.clone();
        tasks.push(tokio::spawn(async move {
            ingestion_barrier.wait().await;
            let started = Instant::now();
            let response = tokio::time::timeout(
                REQUEST_TIMEOUT,
                ingestion_admin
                    .client
                    .get(format!("{base_url}/v1/ingestion/my-jobs?limit=1"))
                    .send(),
            )
            .await
            .expect("ingestion exceeded its request timeout")
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.json::<serde_json::Value>().await.unwrap(),
                serde_json::json!([])
            );
            Sample {
                workload: "ingestion",
                elapsed: started.elapsed(),
            }
        }));
    }

    let suite_started = Instant::now();
    barrier.wait().await;
    let mut by_workload: HashMap<&'static str, Vec<Duration>> = HashMap::new();
    for task in tasks {
        let sample = task.await.expect("mixed workload task panicked");
        by_workload
            .entry(sample.workload)
            .or_default()
            .push(sample.elapsed);
    }

    assert!(suite_started.elapsed() <= TOTAL_BUDGET);
    for (workload, samples) in &mut by_workload {
        let observed = p95(samples);
        assert!(
            observed <= P95_BUDGET,
            "{workload} p95 {observed:?} exceeded {P95_BUDGET:?}"
        );
    }
    assert_eq!(by_workload.len(), 4);
}
