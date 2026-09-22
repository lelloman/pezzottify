//! Real HTTP -> application scheduler -> shared primitives -> blocking job -> SQLite.
//! Each test owns loopback listeners and temporary databases. Gates synchronize
//! assertions; timeouts are safety bounds, never substitutes for job completion.
#[allow(dead_code)]
mod common;
use common::{TestClient, TestServer};
use pezzottify_server::background_jobs::*;
use pezzottify_server::server_store::JobRun;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

struct Probe {
    id: &'static str,
    schedule: JobSchedule,
    policy: JobExecutionPolicy,
    behavior: ShutdownBehavior,
    immediate: bool,
    released: AtomicBool,
    ignore_cancel: bool,
    started: AtomicUsize,
    saw_cancel: AtomicBool,
    outcome: AtomicUsize, // 0 success, 1 application error, 2 panic
    params: Mutex<Option<Value>>,
}
impl Probe {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            schedule: JobSchedule::Manual,
            policy: JobExecutionPolicy::default(),
            behavior: ShutdownBehavior::Cancellable,
            immediate: true,
            released: AtomicBool::new(true),
            ignore_cancel: false,
            started: AtomicUsize::new(0),
            saw_cancel: AtomicBool::new(false),
            outcome: AtomicUsize::new(0),
            params: Mutex::new(None),
        }
    }
    fn blocked(id: &'static str) -> Self {
        Self {
            released: AtomicBool::new(false),
            ..Self::new(id)
        }
    }
    fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
    }
    fn count(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }
}
impl BackgroundJob for Probe {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.id
    }
    fn description(&self) -> &'static str {
        "E2E controlled blocking job"
    }
    fn schedule(&self) -> JobSchedule {
        self.schedule.clone()
    }
    fn execution_policy(&self) -> JobExecutionPolicy {
        self.policy
    }
    fn shutdown_behavior(&self) -> ShutdownBehavior {
        self.behavior
    }
    fn run_on_startup(&self) -> bool {
        self.immediate
    }
    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(12);
        while !self.released.load(Ordering::SeqCst) {
            if ctx.is_cancelled() {
                self.saw_cancel.store(true, Ordering::SeqCst);
                if !self.ignore_cancel {
                    return Err(JobError::Cancelled);
                }
            }
            assert!(Instant::now() < deadline, "E2E job gate was not released");
            std::thread::sleep(Duration::from_millis(2));
        }
        match self.outcome.load(Ordering::SeqCst) {
            1 => Err(JobError::ExecutionFailed("E2E failure".into())),
            2 => panic!("E2E panic"),
            _ => Ok(()),
        }
    }
    fn execute_with_params(&self, ctx: &JobContext, params: Option<Value>) -> Result<(), JobError> {
        *self.params.lock().unwrap() = params;
        self.execute(ctx)
    }
}
async fn wait(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("observable job condition not reached");
}
async fn history(server: &TestServer, id: &str, count: usize) -> Vec<JobRun> {
    wait(|| {
        let rows = server.server_store.get_job_history(id, 100).unwrap();
        rows.len() >= count && rows.iter().all(|row| row.finished_at.is_some())
    })
    .await;
    server.server_store.get_job_history(id, 100).unwrap()
}
async fn spawn(jobs: &[Arc<Probe>]) -> (TestServer, TestClient) {
    let mut builder = TestServer::builder();
    for job in jobs {
        builder = builder.with_scheduler_job(job.clone());
    }
    let server = builder.spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;
    (server, client)
}
async fn trigger(client: &TestClient, id: &str) {
    assert_eq!(client.admin_trigger_job(id).await.status(), 202);
}
async fn pause(client: &TestClient, suffix: &str, paused: bool, cancel: bool) {
    let response = client
        .client
        .put(format!("{}/v1/admin/jobs/{suffix}", client.base_url))
        .json(&json!({"paused": paused, "cancel_running": cancel}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
}
async fn cancel(client: &TestClient, id: &str) -> reqwest::Response {
    client
        .client
        .post(format!("{}/v1/admin/jobs/{id}/cancel", client.base_url))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn manual_payload_history_and_immediate_retrigger_round_trip() {
    let job = Arc::new(Probe::new("manual"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    for attempt in 1..=6 {
        let payload = json!({"attempt": attempt, "nested": {"x": [1,2,3]}});
        let response = client
            .client
            .post(format!("{}/v1/admin/jobs/manual/trigger", client.base_url))
            .json(&json!({"params": payload}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 202);
        let rows = history(&server, "manual", attempt).await;
        assert!(rows
            .iter()
            .all(|row| row.status.as_str() == "completed" && row.triggered_by == "manual"));
        assert_eq!(*job.params.lock().unwrap(), Some(payload));
    }
    assert_eq!(job.count(), 6);
    let response = client.admin_get_job_history("manual", 10).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["history"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    server.drain_scheduler().await;
}

#[tokio::test]
async fn overlapping_http_triggers_execute_exactly_once() {
    let job = Arc::new(Probe::blocked("unique"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    let responses =
        futures::future::join_all((0..12).map(|_| client.admin_trigger_job("unique"))).await;
    assert_eq!(responses.iter().filter(|r| r.status() == 202).count(), 1);
    assert_eq!(responses.iter().filter(|r| r.status() == 409).count(), 11);
    wait(|| job.count() == 1).await;
    job.release();
    assert_eq!(history(&server, "unique", 1).await.len(), 1);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn cancellation_finishes_history_and_releases_overlap_slot() {
    let job = Arc::new(Probe::blocked("cancel"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    trigger(&client, "cancel").await;
    wait(|| job.count() == 1).await;
    assert_eq!(cancel(&client, "cancel").await.status(), 202);
    let rows = history(&server, "cancel", 1).await;
    assert_eq!(rows[0].error_message.as_deref(), Some("Cancelled"));
    assert!(job.saw_cancel.load(Ordering::SeqCst));
    assert_eq!(cancel(&client, "cancel").await.status(), 409);
    assert_eq!(cancel(&client, "missing").await.status(), 404);
    job.release();
    trigger(&client, "cancel").await;
    assert_eq!(
        history(&server, "cancel", 2).await[0].status.as_str(),
        "completed"
    );
    server.drain_scheduler().await;
}

#[tokio::test]
async fn global_and_class_capacity_and_queue_expiry_are_enforced() {
    let mut first = Probe::blocked("first");
    first.policy.resource_class = JobResourceClass::CpuBound;
    let mut queued = Probe::new("queued");
    queued.policy = JobExecutionPolicy::new(JobResourceClass::CpuBound)
        .with_queue_timeout(Duration::from_millis(200))
        .with_circuit_breaker(1, Duration::from_secs(30));
    let first = Arc::new(first);
    let queued = Arc::new(queued);
    let other = Arc::new(Probe::new("other"));
    let (mut server, client) = spawn(&[first.clone(), queued.clone(), other.clone()]).await;
    trigger(&client, "first").await;
    wait(|| first.count() == 1).await;
    trigger(&client, "queued").await;
    trigger(&client, "other").await;
    assert_eq!(
        history(&server, "other", 1).await[0].status.as_str(),
        "completed"
    );
    assert_eq!(
        history(&server, "queued", 1).await[0]
            .error_message
            .as_deref(),
        Some("Queue timeout")
    );
    assert_eq!(queued.count(), 0);
    first.release();
    history(&server, "first", 1).await;
    // Expiry is not a circuit failure and must not poison future admission.
    trigger(&client, "queued").await;
    history(&server, "queued", 2).await;
    assert_eq!(queued.count(), 1);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn global_limit_serializes_different_resource_classes() {
    let first = Arc::new(Probe::blocked("global_a"));
    let mut second = Probe::new("global_b");
    second.policy.resource_class = JobResourceClass::Lightweight;
    let second = Arc::new(second);
    let mut server = TestServer::builder()
        .with_scheduler_job(first.clone())
        .with_scheduler_job(second.clone())
        .with_scheduler_config(JobSchedulerConfig {
            max_concurrent_jobs: 1,
            ..Default::default()
        })
        .spawn()
        .await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;
    trigger(&client, "global_a").await;
    wait(|| first.count() == 1).await;
    trigger(&client, "global_b").await;
    // Use a completed command as a barrier, then inspect accepted/running history.
    pause(&client, "controls/classes/cpu_bound", true, false).await;
    assert_eq!(second.count(), 0);
    assert!(
        server.server_store.get_job_history("global_b", 1).unwrap()[0]
            .finished_at
            .is_none()
    );
    first.release();
    history(&server, "global_b", 1).await;
    assert_eq!(second.count(), 1);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn runtime_expiry_keeps_blocking_work_owned_and_capacity_reserved() {
    let mut slow = Probe::blocked("overrun");
    slow.ignore_cancel = true;
    slow.policy = JobExecutionPolicy::new(JobResourceClass::CpuBound)
        .with_max_runtime(Duration::from_millis(100));
    let slow = Arc::new(slow);
    let mut next = Probe::new("after_overrun");
    next.policy.resource_class = JobResourceClass::CpuBound;
    let next = Arc::new(next);
    let (mut server, client) = spawn(&[slow.clone(), next.clone()]).await;
    trigger(&client, "overrun").await;
    wait(|| slow.saw_cancel.load(Ordering::SeqCst)).await;
    trigger(&client, "after_overrun").await;
    assert_eq!(client.admin_trigger_job("overrun").await.status(), 409);
    assert_eq!(next.count(), 0);
    assert!(
        server.server_store.get_job_history("overrun", 1).unwrap()[0]
            .finished_at
            .is_none()
    );
    slow.release();
    assert_eq!(
        history(&server, "overrun", 1).await[0]
            .error_message
            .as_deref(),
        Some("Job timed out")
    );
    history(&server, "after_overrun", 1).await;
    server.drain_scheduler().await;
}

#[tokio::test]
async fn errors_and_panics_are_persisted_without_stopping_scheduler() {
    let bad = Arc::new(Probe::new("bad"));
    let good = Arc::new(Probe::new("good"));
    let (mut server, client) = spawn(&[bad.clone(), good.clone()]).await;
    for mode in 1..=2 {
        bad.outcome.store(mode, Ordering::SeqCst);
        trigger(&client, "bad").await;
        let rows = history(&server, "bad", mode).await;
        assert_eq!(rows[0].status.as_str(), "failed");
        assert!(rows[0]
            .error_message
            .as_ref()
            .unwrap()
            .contains(if mode == 1 {
                "E2E failure"
            } else {
                "E2E panic"
            }));
        trigger(&client, "good").await;
        history(&server, "good", mode).await;
    }
    assert!(!server.scheduler_finished());
    server.drain_scheduler().await;
}

#[tokio::test]
async fn pause_scopes_compose_and_keep_existing_wire_format() {
    let mut a = Probe::new("cpu");
    a.policy.resource_class = JobResourceClass::CpuBound;
    let a = Arc::new(a);
    let b = Arc::new(Probe::new("general"));
    let (mut server, client) = spawn(&[a.clone(), b.clone()]).await;
    pause(&client, "controls/classes/cpu_bound", true, false).await;
    assert_eq!(client.admin_trigger_job("cpu").await.status(), 409);
    trigger(&client, "general").await;
    history(&server, "general", 1).await;
    pause(&client, "general/pause", true, false).await;
    pause(&client, "controls/global", true, false).await;
    pause(&client, "controls/global", false, false).await;
    assert_eq!(client.admin_trigger_job("general").await.status(), 409);
    assert_eq!(client.admin_trigger_job("cpu").await.status(), 409);
    let persisted: Value = serde_json::from_str(
        &server
            .server_store
            .get_state("background_jobs.pause_state.v1")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        persisted,
        json!({"global_paused": false, "paused_jobs": ["general"], "paused_resource_classes": ["cpu_bound"]})
    );
    pause(&client, "controls/classes/cpu_bound", false, false).await;
    pause(&client, "general/pause", false, false).await;
    trigger(&client, "cpu").await;
    trigger(&client, "general").await;
    history(&server, "cpu", 1).await;
    history(&server, "general", 2).await;
    server.drain_scheduler().await;
}

#[tokio::test]
async fn pause_cancel_respects_wait_for_completion_jobs() {
    let cancellable = Arc::new(Probe::blocked("cancellable"));
    let mut wait_job = Probe::blocked("wait");
    wait_job.behavior = ShutdownBehavior::WaitForCompletion;
    let wait_job = Arc::new(wait_job);
    let (mut server, client) = spawn(&[cancellable.clone(), wait_job.clone()]).await;
    trigger(&client, "cancellable").await;
    trigger(&client, "wait").await;
    wait(|| cancellable.count() == 1 && wait_job.count() == 1).await;
    assert_eq!(cancel(&client, "wait").await.status(), 500); // established API contract
    pause(&client, "controls/global", true, true).await;
    history(&server, "cancellable", 1).await;
    assert!(!wait_job.saw_cancel.load(Ordering::SeqCst));
    assert!(server.server_store.get_job_history("wait", 1).unwrap()[0]
        .finished_at
        .is_none());
    wait_job.release();
    history(&server, "wait", 1).await;
    server.drain_scheduler().await;
}

#[tokio::test]
async fn shutdown_waits_for_uncooperative_blocking_execution_and_history() {
    let mut job = Probe::blocked("drain");
    job.ignore_cancel = true;
    let job = Arc::new(job);
    let (mut server, client) = spawn(&[job.clone()]).await;
    trigger(&client, "drain").await;
    wait(|| job.count() == 1).await;
    server.request_scheduler_shutdown();
    wait(|| job.saw_cancel.load(Ordering::SeqCst)).await;
    assert!(!server.scheduler_finished());
    assert!(server.server_store.get_job_history("drain", 1).unwrap()[0]
        .finished_at
        .is_none());
    job.release();
    server.drain_scheduler().await;
    assert_eq!(
        server.server_store.get_job_history("drain", 1).unwrap()[0]
            .status
            .as_str(),
        "completed"
    );
}

#[tokio::test]
async fn circuit_threshold_rejection_cooldown_and_successful_recovery() {
    let mut job = Probe::new("breaker");
    job.policy = JobExecutionPolicy::default().with_circuit_breaker(2, Duration::from_millis(400));
    let job = Arc::new(job);
    job.outcome.store(1, Ordering::SeqCst);
    let (mut server, client) = spawn(&[job.clone()]).await;
    for count in 1..=2 {
        trigger(&client, "breaker").await;
        history(&server, "breaker", count).await;
    }
    assert_eq!(client.admin_trigger_job("breaker").await.status(), 503);
    assert_eq!(job.count(), 2);
    job.outcome.store(0, Ordering::SeqCst);
    let state: Value = serde_json::from_str(
        &server
            .server_store
            .get_state("background_jobs.circuit_breakers.v1")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let until = state["jobs"]["breaker"]["open_until_millis"]
        .as_i64()
        .unwrap();
    wait(|| chrono::Utc::now().timestamp_millis() > until).await;
    trigger(&client, "breaker").await;
    history(&server, "breaker", 3).await;
    trigger(&client, "breaker").await;
    history(&server, "breaker", 4).await;
    let state: Value = serde_json::from_str(
        &server
            .server_store
            .get_state("background_jobs.circuit_breakers.v1")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state["jobs"], json!({}));
    server.drain_scheduler().await;
}

#[tokio::test]
async fn restart_restores_pause_circuit_and_marks_abandoned_history_failed() {
    let mut bad = Probe::new("persistent_breaker");
    bad.policy = JobExecutionPolicy::default().with_circuit_breaker(1, Duration::from_secs(30));
    let bad = Arc::new(bad);
    bad.outcome.store(1, Ordering::SeqCst);
    let paused = Arc::new(Probe::new("persistent_pause"));
    let (mut first, client) = spawn(&[bad.clone(), paused.clone()]).await;
    trigger(&client, bad.id).await;
    history(&first, bad.id, 1).await;
    pause(&client, "persistent_pause/pause", true, false).await;
    first.drain_scheduler().await;
    first
        .server_store
        .record_job_start("abandoned", "manual")
        .unwrap();
    let mut second = TestServer::builder()
        .with_scheduler_store(first.reopen_server_store())
        .with_scheduler_job(bad.clone())
        .with_scheduler_job(paused.clone())
        .spawn()
        .await;
    let client = TestClient::authenticated_admin(second.base_url.clone()).await;
    assert_eq!(client.admin_trigger_job(bad.id).await.status(), 503);
    assert_eq!(client.admin_trigger_job(paused.id).await.status(), 409);
    assert_eq!(
        history(&second, "abandoned", 1).await[0].status.as_str(),
        "failed"
    );
    assert_eq!(bad.count(), 1);
    assert_eq!(paused.count(), 0);
    pause(&client, "persistent_pause/pause", false, false).await;
    trigger(&client, paused.id).await;
    history(&second, paused.id, 1).await;
    second.drain_scheduler().await;
}

#[tokio::test]
async fn hooks_record_trigger_and_do_not_overlap_or_bypass_pause() {
    let mut job = Probe::blocked("hook");
    job.schedule = JobSchedule::Hook(HookEvent::OnCatalogChange);
    let job = Arc::new(job);
    let (mut server, client) = spawn(&[job.clone()]).await;
    server.emit_hook(HookEvent::OnCatalogChange).await;
    wait(|| job.count() == 1).await;
    server.emit_hook(HookEvent::OnCatalogChange).await;
    pause(&client, "hook/pause", true, false).await;
    job.release();
    let rows = history(&server, "hook", 1).await;
    assert_eq!(rows[0].triggered_by, "hook:OnCatalogChange");
    server.emit_hook(HookEvent::OnCatalogChange).await;
    // The command acknowledgement is a barrier: the biased scheduler drains
    // already-buffered hook events before handling commands.
    pause(&client, "controls/classes/cpu_bound", false, false).await;
    assert_eq!(job.count(), 1);
    pause(&client, "hook/pause", false, false).await;
    server.emit_hook(HookEvent::OnUserCreated).await;
    pause(&client, "controls/classes/cpu_bound", false, false).await;
    assert_eq!(job.count(), 1);
    server.emit_hook(HookEvent::OnCatalogChange).await;
    history(&server, "hook", 2).await;
    server.drain_scheduler().await;
    assert_eq!(job.count(), 2);
}

#[tokio::test]
async fn interval_recurrence_wakes_on_completion_and_never_overlaps() {
    let mut job = Probe::blocked("interval");
    job.schedule = JobSchedule::Interval(Duration::from_millis(150));
    let job = Arc::new(job);
    let (mut server, client) = spawn(&[job.clone()]).await;
    wait(|| job.count() == 1).await;
    assert_eq!(client.admin_trigger_job("interval").await.status(), 409);
    job.release();
    wait(|| job.count() >= 2).await;
    pause(&client, "interval/pause", true, false).await;
    history(&server, "interval", 2).await;
    let schedule = server
        .server_store
        .get_schedule_state("interval")
        .unwrap()
        .unwrap();
    assert!(schedule.last_run_at.is_some());
    assert!(schedule.next_run_at > schedule.last_run_at.unwrap());
    let rows = server
        .server_store
        .get_job_history("interval", 100)
        .unwrap();
    assert!(rows.iter().all(|row| row.triggered_by == "schedule"));
    for pair in rows.windows(2) {
        assert!(pair[0].started_at >= pair[1].finished_at.unwrap());
    }
    server.drain_scheduler().await;
}

#[tokio::test]
async fn deferred_jittered_schedule_and_manual_reset_survive_restart() {
    let mut job = Probe::new("deferred");
    job.immediate = false;
    job.schedule = JobSchedule::JitteredInterval {
        interval: Duration::from_secs(60),
        jitter: Duration::from_secs(5),
    };
    let job = Arc::new(job);
    let before = chrono::Utc::now();
    let (mut first, client) = spawn(&[job.clone()]).await;
    assert_eq!(job.count(), 0);
    let initial = first
        .server_store
        .get_schedule_state(job.id)
        .unwrap()
        .unwrap();
    assert!(initial.next_run_at >= before + chrono::Duration::seconds(60));
    assert!(initial.next_run_at <= chrono::Utc::now() + chrono::Duration::seconds(65));
    trigger(&client, job.id).await;
    history(&first, job.id, 1).await;
    wait(|| {
        first
            .server_store
            .get_schedule_state(job.id)
            .unwrap()
            .unwrap()
            .last_run_at
            .is_some()
    })
    .await;
    let completed = first
        .server_store
        .get_schedule_state(job.id)
        .unwrap()
        .unwrap();
    assert!(
        (60..=65).contains(&(completed.next_run_at - completed.last_run_at.unwrap()).num_seconds())
    );
    first.drain_scheduler().await;
    let mut second = TestServer::builder()
        .with_scheduler_store(first.reopen_server_store())
        .with_scheduler_job(job.clone())
        .spawn()
        .await;
    let restored = second
        .server_store
        .get_schedule_state(job.id)
        .unwrap()
        .unwrap();
    assert_eq!(restored.next_run_at, completed.next_run_at);
    assert_eq!(job.count(), 1);
    second.drain_scheduler().await;
}

#[tokio::test]
async fn rejected_history_insert_never_executes_and_recovers_after_storage_repair() {
    let job = Arc::new(Probe::new("history_failure"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    let db = rusqlite::Connection::open(server.server_db_path()).unwrap();
    db.execute_batch("CREATE TRIGGER reject_start BEFORE INSERT ON job_runs BEGIN SELECT RAISE(FAIL, 'E2E history failure'); END;").unwrap();
    // Existing API acknowledges the trigger even when its subsequent history
    // write fails. Preserve that response, but never execute unrecorded work.
    trigger(&client, job.id).await;
    assert_eq!(job.count(), 0);
    assert!(server
        .server_store
        .get_job_history(job.id, 10)
        .unwrap()
        .is_empty());
    db.execute_batch("DROP TRIGGER reject_start;").unwrap();
    trigger(&client, job.id).await;
    history(&server, job.id, 1).await;
    assert_eq!(job.count(), 1);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn failed_pause_write_does_not_change_live_admission() {
    let job = Arc::new(Probe::new("pause_failure"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    let db = rusqlite::Connection::open(server.server_db_path()).unwrap();
    db.execute_batch("CREATE TRIGGER reject_pause BEFORE INSERT ON server_state WHEN NEW.key = 'background_jobs.pause_state.v1' BEGIN SELECT RAISE(FAIL, 'E2E pause failure'); END;").unwrap();
    assert_eq!(
        client
            .admin_set_global_job_pause(true, false)
            .await
            .status(),
        500
    );
    assert_eq!(
        client
            .admin_get_job_controls()
            .await
            .json::<Value>()
            .await
            .unwrap()["global_paused"],
        false
    );
    trigger(&client, job.id).await;
    history(&server, job.id, 1).await;
    db.execute_batch("DROP TRIGGER reject_pause;").unwrap();
    pause(&client, "controls/global", true, false).await;
    assert_eq!(client.admin_trigger_job(job.id).await.status(), 409);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn cancelling_queued_job_never_calls_blocking_factory_or_trips_circuit() {
    let mut holding = Probe::blocked("holding");
    holding.policy.resource_class = JobResourceClass::CpuBound;
    let holding = Arc::new(holding);
    let mut queued = Probe::new("cancel_queued");
    queued.policy = JobExecutionPolicy::new(JobResourceClass::CpuBound)
        .with_circuit_breaker(1, Duration::from_secs(30));
    let queued = Arc::new(queued);
    let (mut server, client) = spawn(&[holding.clone(), queued.clone()]).await;
    trigger(&client, holding.id).await;
    wait(|| holding.count() == 1).await;
    trigger(&client, queued.id).await;
    assert_eq!(cancel(&client, queued.id).await.status(), 202);
    assert_eq!(
        history(&server, queued.id, 1).await[0]
            .error_message
            .as_deref(),
        Some("Cancelled")
    );
    assert_eq!(queued.count(), 0);
    holding.release();
    history(&server, holding.id, 1).await;
    trigger(&client, queued.id).await;
    history(&server, queued.id, 2).await;
    assert_eq!(queued.count(), 1);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn panic_and_runtime_timeout_each_trip_breaker_without_automatic_retry() {
    let mut panic_job = Probe::new("panic_breaker");
    panic_job.policy =
        JobExecutionPolicy::default().with_circuit_breaker(1, Duration::from_secs(30));
    let panic_job = Arc::new(panic_job);
    panic_job.outcome.store(2, Ordering::SeqCst);
    let mut timed = Probe::blocked("timeout_breaker");
    timed.policy = JobExecutionPolicy::default()
        .with_max_runtime(Duration::from_millis(50))
        .with_circuit_breaker(1, Duration::from_secs(30));
    let timed = Arc::new(timed);
    let (mut server, client) = spawn(&[panic_job.clone(), timed.clone()]).await;
    for job in [&panic_job, &timed] {
        trigger(&client, job.id).await;
        history(&server, job.id, 1).await;
        assert_eq!(client.admin_trigger_job(job.id).await.status(), 503);
        assert_eq!(job.count(), 1);
    }
    server.drain_scheduler().await;
}

#[tokio::test]
async fn combined_startup_hook_and_deferred_interval_keep_trigger_provenance() {
    let mut job = Probe::new("combined");
    job.immediate = false;
    job.schedule = JobSchedule::Combined {
        cron: None,
        interval: Some(Duration::from_secs(60)),
        hooks: vec![HookEvent::OnStartup, HookEvent::OnDownloadComplete],
    };
    let job = Arc::new(job);
    let (mut server, _client) = spawn(&[job.clone()]).await;
    assert_eq!(
        history(&server, job.id, 1).await[0].triggered_by,
        "hook:OnStartup"
    );
    server.emit_hook(HookEvent::OnDownloadComplete).await;
    assert_eq!(
        history(&server, job.id, 2).await[0].triggered_by,
        "hook:OnDownloadComplete"
    );
    assert_eq!(job.count(), 2);
    server.drain_scheduler().await;
}

#[tokio::test]
async fn old_due_schedule_runs_once_then_reschedules_without_replaying_backlog() {
    let mut first = TestServer::builder().with_scheduler().spawn().await;
    first.drain_scheduler().await;
    let old = chrono::Utc::now() - chrono::Duration::days(2);
    first
        .server_store
        .update_schedule_state(&pezzottify_server::server_store::JobScheduleState {
            job_id: "overdue".into(),
            next_run_at: old,
            last_run_at: Some(old),
        })
        .unwrap();
    let mut job = Probe::new("overdue");
    job.immediate = false;
    job.schedule = JobSchedule::Interval(Duration::from_secs(60));
    let job = Arc::new(job);
    let mut server = TestServer::builder()
        .with_scheduler_store(first.reopen_server_store())
        .with_scheduler_job(job.clone())
        .spawn()
        .await;
    history(&server, job.id, 1).await;
    wait(|| {
        server
            .server_store
            .get_schedule_state(job.id)
            .unwrap()
            .unwrap()
            .next_run_at
            > chrono::Utc::now()
    })
    .await;
    server.drain_scheduler().await;
    assert_eq!(job.count(), 1);
}

#[tokio::test]
async fn authentication_and_unknown_pause_scopes_cannot_mutate_controls() {
    let job = Arc::new(Probe::new("protected"));
    let (mut server, client) = spawn(&[job.clone()]).await;
    let anonymous = TestClient::new(server.base_url.clone());
    let regular = TestClient::authenticated(server.base_url.clone()).await;
    for (caller, status) in [(&anonymous, 401), (&regular, 403)] {
        assert_eq!(caller.admin_trigger_job(job.id).await.status(), status);
        assert_eq!(cancel(caller, job.id).await.status(), status);
        assert_eq!(
            caller.admin_set_global_job_pause(true, true).await.status(),
            status
        );
    }
    for (suffix, status) in [("unknown/pause", 404), ("controls/classes/unknown", 400)] {
        let response = client
            .client
            .put(format!("{}/v1/admin/jobs/{suffix}", client.base_url))
            .json(&json!({"paused": true, "cancel_running": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    assert_eq!(job.count(), 0);
    trigger(&client, job.id).await;
    history(&server, job.id, 1).await;
    server.drain_scheduler().await;
}

#[tokio::test]
async fn lowered_circuit_threshold_accepts_old_closed_state_without_panicking() {
    let mut first = TestServer::builder().with_scheduler().spawn().await;
    first.drain_scheduler().await;
    first
        .server_store
        .set_state(
            "background_jobs.circuit_breakers.v1",
            &json!({
                "jobs": {"changed_policy": {"consecutive_failures": 4, "open_until_millis": null}}
            })
            .to_string(),
        )
        .unwrap();
    let mut job = Probe::new("changed_policy");
    job.policy = JobExecutionPolicy::default().with_circuit_breaker(2, Duration::from_secs(30));
    let job = Arc::new(job);
    job.outcome.store(1, Ordering::SeqCst);
    let mut server = TestServer::builder()
        .with_scheduler_store(first.reopen_server_store())
        .with_scheduler_job(job.clone())
        .spawn()
        .await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;
    trigger(&client, job.id).await;
    history(&server, job.id, 1).await;
    assert_eq!(client.admin_trigger_job(job.id).await.status(), 503);
    assert_eq!(job.count(), 1);
    server.drain_scheduler().await;
}
