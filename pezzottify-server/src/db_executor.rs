//! Bounded, priority-aware execution for synchronous database stores.
//!
//! This module is deliberately independent from the server state. Callers wrap a
//! synchronous store in a [`DbHandle`] and submit closures to a named [`DbLane`].
//! Lanes cap database-specific concurrency while the weighted scheduler prevents
//! background maintenance from starving user-facing work (and vice versa).

use anyhow::Error as AnyhowError;
use std::{
    collections::{HashMap, VecDeque},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::{oneshot, Notify};

const PRIORITY_COUNT: usize = 3;
const WEIGHTED_SCHEDULE: [DbPriority; 13] = [
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Critical,
    DbPriority::Interactive,
    DbPriority::Interactive,
    DbPriority::Interactive,
    DbPriority::Interactive,
    DbPriority::Background,
];

/// Scheduling importance for a database operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DbPriority {
    /// Authentication, session validation, and similarly latency-critical work.
    Critical,
    /// User-triggered reads and mutations.
    Interactive,
    /// Maintenance, enrichment, ingestion, and other asynchronous work.
    Background,
}

impl DbPriority {
    const fn index(self) -> usize {
        match self {
            Self::Critical => 0,
            Self::Interactive => 1,
            Self::Background => 2,
        }
    }
}

/// Independent database concurrency domains.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DbLane {
    CatalogRead,
    CatalogWrite,
    User,
    Server,
    SearchRead,
    SearchWrite,
    Download,
    Ingestion,
    EnrichmentRead,
    EnrichmentWrite,
    Mcp,
    Shows,
}

/// Queue and execution budgets for one priority class.
#[derive(Clone, Copy, Debug)]
pub struct DbPriorityConfig {
    pub queue_capacity: usize,
    pub queue_timeout: Duration,
    pub execution_timeout: Duration,
}

/// Executor sizing and lane-concurrency configuration.
#[derive(Clone, Debug)]
pub struct DbExecutorConfig {
    pub worker_threads: usize,
    pub critical: DbPriorityConfig,
    pub interactive: DbPriorityConfig,
    pub background: DbPriorityConfig,
    pub lane_limits: HashMap<DbLane, usize>,
}

impl DbExecutorConfig {
    fn priority(&self, priority: DbPriority) -> DbPriorityConfig {
        match priority {
            DbPriority::Critical => self.critical,
            DbPriority::Interactive => self.interactive,
            DbPriority::Background => self.background,
        }
    }
}

impl Default for DbExecutorConfig {
    fn default() -> Self {
        let mut lane_limits = HashMap::new();
        lane_limits.insert(DbLane::CatalogRead, 4);
        lane_limits.insert(DbLane::CatalogWrite, 1);
        lane_limits.insert(DbLane::User, 1);
        lane_limits.insert(DbLane::Server, 1);
        lane_limits.insert(DbLane::SearchRead, 2);
        lane_limits.insert(DbLane::SearchWrite, 1);
        lane_limits.insert(DbLane::Download, 1);
        lane_limits.insert(DbLane::Ingestion, 1);
        lane_limits.insert(DbLane::EnrichmentRead, 2);
        lane_limits.insert(DbLane::EnrichmentWrite, 1);
        lane_limits.insert(DbLane::Mcp, 2);
        lane_limits.insert(DbLane::Shows, 1);

        Self {
            worker_threads: 8,
            critical: DbPriorityConfig {
                queue_capacity: 64,
                queue_timeout: Duration::from_secs(1),
                execution_timeout: Duration::from_secs(5),
            },
            interactive: DbPriorityConfig {
                queue_capacity: 256,
                queue_timeout: Duration::from_secs(2),
                execution_timeout: Duration::from_secs(10),
            },
            background: DbPriorityConfig {
                queue_capacity: 64,
                queue_timeout: Duration::from_secs(30),
                execution_timeout: Duration::from_secs(300),
            },
            lane_limits,
        }
    }
}

/// Failure returned by a submitted database operation.
#[derive(Debug, Error)]
pub enum DbRunError {
    #[error("database executor queue timed out")]
    QueueTimeout,
    #[error("database operation timed out")]
    ExecutionTimeout,
    #[error("database executor is shutting down")]
    ShuttingDown,
    #[error("database operation panicked: {0}")]
    Panicked(String),
    #[error("database operation failed: {0}")]
    Store(#[source] AnyhowError),
}

type Task = Box<dyn FnOnce() + Send + 'static>;

struct Job {
    lane: DbLane,
    cancelled: Arc<AtomicBool>,
    task: Task,
}

struct QueueState {
    queues: [VecDeque<Job>; PRIORITY_COUNT],
    active_by_lane: HashMap<DbLane, usize>,
    schedule_cursor: usize,
    shutting_down: bool,
}

impl QueueState {
    fn new() -> Self {
        Self {
            queues: std::array::from_fn(|_| VecDeque::new()),
            active_by_lane: HashMap::new(),
            schedule_cursor: 0,
            shutting_down: false,
        }
    }

    fn purge_cancelled(&mut self) -> bool {
        let mut removed = false;
        for queue in &mut self.queues {
            let previous_len = queue.len();
            queue.retain(|job| !job.cancelled.load(Ordering::Acquire));
            removed |= queue.len() != previous_len;
        }
        removed
    }
}

struct Shared {
    state: Mutex<QueueState>,
    work_available: Condvar,
    capacity_available: Notify,
    capacities: [usize; PRIORITY_COUNT],
    lane_limits: HashMap<DbLane, usize>,
}

struct ExecutorInner {
    shared: Arc<Shared>,
    config: DbExecutorConfig,
}

impl Drop for ExecutorInner {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutting_down = true;
        drop(state);
        self.shared.work_available.notify_all();
        self.shared.capacity_available.notify_waiters();
    }
}

/// Cloneable scheduler for blocking database work.
#[derive(Clone)]
pub struct DbExecutor {
    inner: Arc<ExecutorInner>,
}

impl DbExecutor {
    /// Start an executor using the supplied fixed worker and queue limits.
    ///
    /// # Panics
    ///
    /// Panics when a worker count, queue capacity, or lane limit is zero.
    pub fn new(config: DbExecutorConfig) -> Self {
        assert!(config.worker_threads > 0, "worker_threads must be non-zero");
        for priority in [
            DbPriority::Critical,
            DbPriority::Interactive,
            DbPriority::Background,
        ] {
            assert!(
                config.priority(priority).queue_capacity > 0,
                "queue capacities must be non-zero"
            );
        }
        assert!(
            config.lane_limits.values().all(|limit| *limit > 0),
            "lane limits must be non-zero"
        );

        let shared = Arc::new(Shared {
            state: Mutex::new(QueueState::new()),
            work_available: Condvar::new(),
            capacity_available: Notify::new(),
            capacities: [
                config.critical.queue_capacity,
                config.interactive.queue_capacity,
                config.background.queue_capacity,
            ],
            lane_limits: config.lane_limits.clone(),
        });

        for worker_index in 0..config.worker_threads {
            let worker_shared = shared.clone();
            std::thread::Builder::new()
                .name(format!("pezzottify-db-{worker_index}"))
                .spawn(move || worker_loop(worker_shared))
                .expect("failed to start database executor worker");
        }

        Self {
            inner: Arc::new(ExecutorInner { shared, config }),
        }
    }

    fn priority_config(&self, priority: DbPriority) -> DbPriorityConfig {
        self.inner.config.priority(priority)
    }

    fn try_enqueue(&self, priority: DbPriority, job: Job) -> Result<(), EnqueueError> {
        let shared = &self.inner.shared;
        let mut state = shared.state.lock().unwrap();
        if state.shutting_down {
            return Err(EnqueueError::ShuttingDown);
        }
        if state.purge_cancelled() {
            self.inner.shared.capacity_available.notify_waiters();
        }
        let queue = &mut state.queues[priority.index()];
        if queue.len() >= shared.capacities[priority.index()] {
            return Err(EnqueueError::Full(job));
        }
        queue.push_back(job);
        drop(state);
        shared.work_available.notify_one();
        Ok(())
    }

    async fn enqueue_async(
        &self,
        priority: DbPriority,
        mut job: Job,
        deadline: Instant,
    ) -> Result<(), DbRunError> {
        loop {
            let notified = self.inner.shared.capacity_available.notified();
            match self.try_enqueue(priority, job) {
                Ok(()) => return Ok(()),
                Err(EnqueueError::ShuttingDown) => return Err(DbRunError::ShuttingDown),
                Err(EnqueueError::Full(returned)) => job = returned,
            }

            if Instant::now() >= deadline {
                return Err(DbRunError::QueueTimeout);
            }
            if tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), notified)
                .await
                .is_err()
            {
                return Err(DbRunError::QueueTimeout);
            }
        }
    }

    fn enqueue_blocking(
        &self,
        priority: DbPriority,
        mut job: Job,
        deadline: Instant,
    ) -> Result<(), DbRunError> {
        loop {
            match self.try_enqueue(priority, job) {
                Ok(()) => return Ok(()),
                Err(EnqueueError::ShuttingDown) => return Err(DbRunError::ShuttingDown),
                Err(EnqueueError::Full(returned)) => job = returned,
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(DbRunError::QueueTimeout);
            }
            let state = self.inner.shared.state.lock().unwrap();
            let (_state, wait) = self
                .inner
                .shared
                .work_available
                .wait_timeout(state, deadline.saturating_duration_since(now))
                .unwrap();
            if wait.timed_out() && Instant::now() >= deadline {
                return Err(DbRunError::QueueTimeout);
            }
        }
    }
}

enum EnqueueError {
    Full(Job),
    ShuttingDown,
}

/// A typed store bound to one executor lane.
pub struct DbHandle<S: ?Sized> {
    store: Arc<S>,
    executor: DbExecutor,
    lane: DbLane,
}

impl<S: ?Sized> Clone for DbHandle<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            executor: self.executor.clone(),
            lane: self.lane,
        }
    }
}

impl<S> DbHandle<S>
where
    S: ?Sized + Send + Sync + 'static,
{
    pub fn new(store: Arc<S>, executor: DbExecutor, lane: DbLane) -> Self {
        Self {
            store,
            executor,
            lane,
        }
    }

    /// Run synchronous store code without blocking an async runtime worker.
    pub async fn run<T, F>(&self, priority: DbPriority, operation: F) -> Result<T, DbRunError>
    where
        T: Send + 'static,
        F: FnOnce(&S) -> Result<T, AnyhowError> + Send + 'static,
    {
        let config = self.executor.priority_config(priority);
        let queue_deadline = Instant::now() + config.queue_timeout;
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = cancelled.clone();
        let store = self.store.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let (result_tx, result_rx) = oneshot::channel();
        let task = Box::new(move || {
            let _ = started_tx.send(());
            if task_cancelled.load(Ordering::Acquire) {
                return;
            }
            let result = catch_unwind(AssertUnwindSafe(|| operation(store.as_ref())))
                .map_err(panic_message)
                .and_then(|result| result.map_err(DbRunError::Store));
            let _ = result_tx.send(result);
        });

        self.executor
            .enqueue_async(
                priority,
                Job {
                    lane: self.lane,
                    cancelled: cancelled.clone(),
                    task,
                },
                queue_deadline,
            )
            .await?;

        match tokio::time::timeout_at(tokio::time::Instant::from_std(queue_deadline), started_rx)
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(DbRunError::ShuttingDown),
            Err(_) => {
                cancelled.store(true, Ordering::Release);
                self.executor.inner.shared.work_available.notify_all();
                return Err(DbRunError::QueueTimeout);
            }
        }

        match tokio::time::timeout(config.execution_timeout, result_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(DbRunError::ShuttingDown),
            Err(_) => {
                cancelled.store(true, Ordering::Release);
                Err(DbRunError::ExecutionTimeout)
            }
        }
    }

    /// Run through the same scheduler from a synchronous caller.
    pub fn run_blocking<T, F>(&self, priority: DbPriority, operation: F) -> Result<T, DbRunError>
    where
        T: Send + 'static,
        F: FnOnce(&S) -> Result<T, AnyhowError> + Send + 'static,
    {
        let config = self.executor.priority_config(priority);
        let queue_deadline = Instant::now() + config.queue_timeout;
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = cancelled.clone();
        let store = self.store.clone();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
        let task = Box::new(move || {
            let _ = started_tx.send(());
            if task_cancelled.load(Ordering::Acquire) {
                return;
            }
            let result = catch_unwind(AssertUnwindSafe(|| operation(store.as_ref())))
                .map_err(panic_message)
                .and_then(|result| result.map_err(DbRunError::Store));
            let _ = result_tx.send(result);
        });

        self.executor.enqueue_blocking(
            priority,
            Job {
                lane: self.lane,
                cancelled: cancelled.clone(),
                task,
            },
            queue_deadline,
        )?;

        let remaining = queue_deadline.saturating_duration_since(Instant::now());
        if started_rx.recv_timeout(remaining).is_err() {
            cancelled.store(true, Ordering::Release);
            self.executor.inner.shared.work_available.notify_all();
            return Err(DbRunError::QueueTimeout);
        }

        match result_rx.recv_timeout(config.execution_timeout) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                cancelled.store(true, Ordering::Release);
                Err(DbRunError::ExecutionTimeout)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(DbRunError::ShuttingDown),
        }
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> DbRunError {
    let message = payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic payload".to_owned());
    DbRunError::Panicked(message)
}

fn worker_loop(shared: Arc<Shared>) {
    while let Some(job) = take_next_job(&shared) {
        let lane = job.lane;
        (job.task)();

        let mut state = shared.state.lock().unwrap();
        let active = state
            .active_by_lane
            .get_mut(&lane)
            .expect("dispatched lane must be active");
        *active -= 1;
        if *active == 0 {
            state.active_by_lane.remove(&lane);
        }
        drop(state);
        shared.work_available.notify_all();
    }
}

fn take_next_job(shared: &Shared) -> Option<Job> {
    let mut state = shared.state.lock().unwrap();
    loop {
        if state.purge_cancelled() {
            shared.capacity_available.notify_waiters();
        }

        for offset in 0..WEIGHTED_SCHEDULE.len() {
            let schedule_index = (state.schedule_cursor + offset) % WEIGHTED_SCHEDULE.len();
            let priority = WEIGHTED_SCHEDULE[schedule_index];
            let queue_index = priority.index();
            let eligible_position = state.queues[queue_index].iter().position(|job| {
                let active = state.active_by_lane.get(&job.lane).copied().unwrap_or(0);
                let limit = shared.lane_limits.get(&job.lane).copied().unwrap_or(1);
                active < limit
            });
            if let Some(position) = eligible_position {
                let job = state.queues[queue_index]
                    .remove(position)
                    .expect("eligible queue position must exist");
                *state.active_by_lane.entry(job.lane).or_insert(0) += 1;
                state.schedule_cursor = (schedule_index + 1) % WEIGHTED_SCHEDULE.len();
                drop(state);
                shared.capacity_available.notify_one();
                shared.work_available.notify_all();
                return Some(job);
            }
        }

        if state.shutting_down {
            return None;
        }
        state = shared.work_available.wait(state).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_config(worker_threads: usize) -> DbExecutorConfig {
        let budget = DbPriorityConfig {
            queue_capacity: 64,
            queue_timeout: Duration::from_millis(250),
            execution_timeout: Duration::from_millis(250),
        };
        DbExecutorConfig {
            worker_threads,
            critical: budget,
            interactive: budget,
            background: budget,
            ..DbExecutorConfig::default()
        }
    }

    #[tokio::test]
    async fn async_handle_returns_values_and_typed_store_errors() {
        let executor = DbExecutor::new(test_config(1));
        let handle = DbHandle::new(Arc::new(41_u32), executor, DbLane::User);

        assert_eq!(
            handle
                .run(DbPriority::Interactive, |value| Ok(*value + 1))
                .await
                .unwrap(),
            42
        );
        let error = handle
            .run::<(), _>(DbPriority::Interactive, |_| anyhow::bail!("store failure"))
            .await
            .unwrap_err();
        assert!(matches!(error, DbRunError::Store(_)));
        assert!(error.to_string().contains("store failure"));
    }

    #[test]
    fn blocking_handle_uses_the_same_executor() {
        let executor = DbExecutor::new(test_config(1));
        let handle = DbHandle::new(Arc::new(21_u32), executor, DbLane::User);
        let result = handle
            .run_blocking(DbPriority::Critical, |value| Ok(*value * 2))
            .unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn panics_are_isolated_and_reported() {
        let executor = DbExecutor::new(test_config(1));
        let handle = DbHandle::new(Arc::new(()), executor, DbLane::User);
        let error = handle
            .run::<(), _>(DbPriority::Critical, |_| panic!("boom"))
            .await
            .unwrap_err();
        assert!(matches!(error, DbRunError::Panicked(message) if message == "boom"));

        assert_eq!(
            handle.run(DbPriority::Critical, |_| Ok(42)).await.unwrap(),
            42
        );
    }

    #[tokio::test]
    async fn execution_timeout_does_not_block_the_async_runtime() {
        let mut config = test_config(1);
        config.interactive.execution_timeout = Duration::from_millis(20);
        let executor = DbExecutor::new(config);
        let handle = DbHandle::new(Arc::new(()), executor, DbLane::User);

        let heartbeat = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            42
        });
        let error = handle
            .run(DbPriority::Interactive, |_| {
                std::thread::sleep(Duration::from_millis(80));
                Ok(())
            })
            .await
            .unwrap_err();
        assert!(matches!(error, DbRunError::ExecutionTimeout));
        assert_eq!(heartbeat.await.unwrap(), 42);
    }

    #[tokio::test]
    async fn bounded_queue_times_out_when_lane_cannot_drain() {
        let mut config = test_config(2);
        config.interactive.queue_capacity = 1;
        config.interactive.queue_timeout = Duration::from_millis(30);
        config.interactive.execution_timeout = Duration::from_secs(1);
        let executor = DbExecutor::new(config);
        let handle = DbHandle::new(Arc::new(()), executor, DbLane::User);
        let release = Arc::new((Mutex::new(false), Condvar::new()));

        let blocker_release = release.clone();
        let blocker_handle = handle.clone();
        let blocker = tokio::spawn(async move {
            blocker_handle
                .run(DbPriority::Interactive, move |_| {
                    let (lock, condvar) = &*blocker_release;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = condvar.wait(released).unwrap();
                    }
                    Ok(())
                })
                .await
        });
        tokio::time::sleep(Duration::from_millis(10)).await;

        let queued_handle = handle.clone();
        let queued =
            tokio::spawn(
                async move { queued_handle.run(DbPriority::Interactive, |_| Ok(())).await },
            );
        tokio::time::sleep(Duration::from_millis(5)).await;
        let error = handle
            .run(DbPriority::Interactive, |_| Ok(()))
            .await
            .unwrap_err();
        assert!(matches!(error, DbRunError::QueueTimeout));

        let (lock, condvar) = &*release;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        blocker.await.unwrap().unwrap();
        assert!(matches!(
            queued.await.unwrap().unwrap_err(),
            DbRunError::QueueTimeout
        ));
    }

    #[tokio::test]
    async fn lane_limits_serialize_one_store_and_parallelize_distinct_lanes() {
        let mut config = test_config(4);
        config.lane_limits.insert(DbLane::CatalogRead, 2);
        config.lane_limits.insert(DbLane::User, 1);
        let executor = DbExecutor::new(config);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let run_pair = |lane| {
            let handle = DbHandle::new(Arc::new(()), executor.clone(), lane);
            let active = active.clone();
            let peak = peak.clone();
            async move {
                let mut tasks = Vec::new();
                for _ in 0..2 {
                    let handle = handle.clone();
                    let active = active.clone();
                    let peak = peak.clone();
                    tasks.push(tokio::spawn(async move {
                        handle
                            .run(DbPriority::Interactive, move |_| {
                                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                                peak.fetch_max(now, Ordering::SeqCst);
                                std::thread::sleep(Duration::from_millis(30));
                                active.fetch_sub(1, Ordering::SeqCst);
                                Ok(())
                            })
                            .await
                            .unwrap();
                    }));
                }
                for task in tasks {
                    task.await.unwrap();
                }
            }
        };

        run_pair(DbLane::User).await;
        assert_eq!(peak.swap(0, Ordering::SeqCst), 1);
        run_pair(DbLane::CatalogRead).await;
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn critical_work_overtakes_queued_background_work() {
        let executor = DbExecutor::new(test_config(1));
        let handle = DbHandle::new(Arc::new(()), executor, DbLane::User);
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let order = Arc::new(Mutex::new(Vec::new()));

        let blocker_release = release.clone();
        let blocker_handle = handle.clone();
        let blocker = tokio::spawn(async move {
            blocker_handle
                .run(DbPriority::Critical, move |_| {
                    let (lock, condvar) = &*blocker_release;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = condvar.wait(released).unwrap();
                    }
                    Ok(())
                })
                .await
                .unwrap();
        });
        tokio::time::sleep(Duration::from_millis(10)).await;

        let background_order = order.clone();
        let background_handle = handle.clone();
        let background = tokio::spawn(async move {
            background_handle
                .run(DbPriority::Background, move |_| {
                    background_order.lock().unwrap().push("background");
                    Ok(())
                })
                .await
                .unwrap();
        });
        let critical_order = order.clone();
        let critical_handle = handle.clone();
        let critical = tokio::spawn(async move {
            critical_handle
                .run(DbPriority::Critical, move |_| {
                    critical_order.lock().unwrap().push("critical");
                    Ok(())
                })
                .await
                .unwrap();
        });
        tokio::time::sleep(Duration::from_millis(10)).await;

        let (lock, condvar) = &*release;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        blocker.await.unwrap();
        critical.await.unwrap();
        background.await.unwrap();
        assert_eq!(*order.lock().unwrap(), ["critical", "background"]);
    }

    #[tokio::test]
    async fn weighted_schedule_runs_background_under_critical_load() {
        let mut config = test_config(1);
        config.critical.queue_capacity = 32;
        config.background.queue_capacity = 4;
        let executor = DbExecutor::new(config);
        let handle = DbHandle::new(Arc::new(()), executor, DbLane::User);
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let order = Arc::new(Mutex::new(Vec::new()));

        let blocker_release = release.clone();
        let blocker_handle = handle.clone();
        let blocker = tokio::spawn(async move {
            blocker_handle
                .run(DbPriority::Critical, move |_| {
                    let (lock, condvar) = &*blocker_release;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = condvar.wait(released).unwrap();
                    }
                    Ok(())
                })
                .await
                .unwrap();
        });
        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut tasks = Vec::new();
        for index in 0..16 {
            let handle = handle.clone();
            let order = order.clone();
            tasks.push(tokio::spawn(async move {
                handle
                    .run(DbPriority::Critical, move |_| {
                        order.lock().unwrap().push(format!("critical-{index}"));
                        Ok(())
                    })
                    .await
                    .unwrap();
            }));
        }
        let background_handle = handle.clone();
        let background_order = order.clone();
        tasks.push(tokio::spawn(async move {
            background_handle
                .run(DbPriority::Background, move |_| {
                    background_order
                        .lock()
                        .unwrap()
                        .push("background".to_owned());
                    Ok(())
                })
                .await
                .unwrap();
        }));
        tokio::time::sleep(Duration::from_millis(10)).await;

        let (lock, condvar) = &*release;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        blocker.await.unwrap();
        for task in tasks {
            task.await.unwrap();
        }

        let order = order.lock().unwrap();
        let background_position = order
            .iter()
            .position(|item| item == "background")
            .expect("background job must execute");
        assert!(background_position < 13, "execution order was {order:?}");
    }
}
