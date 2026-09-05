#![allow(dead_code)]

use axum::{
    extract::{MatchedPath, State},
    http::{Extensions, StatusCode},
    response::IntoResponse,
};
use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::db_executor::{DbLane, DbPriority};

use super::filesystem_work::FilesystemWorkPool;

/// Metric name prefix for all Pezzottify metrics
const PREFIX: &str = "pezzottify";

/// Service name for homelab storage metrics
const SERVICE_NAME: &str = "pezzottify";
pub const UNMATCHED_ROUTE_LABEL: &str = "<unmatched>";

lazy_static! {
    // Global Prometheus registry
    pub static ref REGISTRY: Registry = Registry::new();

    // HTTP Request Metrics
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_http_requests_total"), "Total number of HTTP requests"),
        &["method", "path", "status"]
    ).expect("Failed to create http_requests_total metric");

    pub static ref HTTP_REQUEST_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_http_request_duration_seconds"),
            "HTTP request duration in seconds"
        )
        .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        &["method", "path"]
    ).expect("Failed to create http_request_duration_seconds metric");

    // Authentication Metrics
    pub static ref AUTH_LOGIN_ATTEMPTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_auth_login_attempts_total"), "Total login attempts"),
        &["status"]
    ).expect("Failed to create auth_login_attempts_total metric");

    pub static ref AUTH_LOGIN_DURATION_SECONDS: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            format!("{PREFIX}_auth_login_duration_seconds"),
            "Login request duration in seconds"
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0])
    ).expect("Failed to create auth_login_duration_seconds metric");

    pub static ref AUTH_ACTIVE_SESSIONS: Gauge = Gauge::new(
        format!("{PREFIX}_auth_active_sessions"),
        "Number of active authentication sessions"
    ).expect("Failed to create auth_active_sessions metric");

    // Rate Limiting Metrics
    pub static ref RATE_LIMIT_HITS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_rate_limit_hits_total"), "Rate limit violations"),
        &["endpoint", "identifier_type"]
    ).expect("Failed to create rate_limit_hits_total metric");

    // Database Metrics
    pub static ref DB_QUERY_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_db_query_duration_seconds"),
            "Database query duration in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        &["operation"]
    ).expect("Failed to create db_query_duration_seconds metric");

    pub static ref DB_CONNECTION_ERRORS_TOTAL: Counter = Counter::new(
        format!("{PREFIX}_db_connection_errors_total"),
        "Total database connection errors"
    ).expect("Failed to create db_connection_errors_total metric");

    pub static ref DB_EXECUTOR_QUEUE_WAIT_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_db_executor_queue_wait_seconds"),
            "Time database operations spend waiting for an executor worker"
        ).buckets(vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 30.0]),
        &["lane", "priority"]
    ).expect("Failed to create db_executor_queue_wait_seconds metric");

    pub static ref DB_EXECUTOR_EXECUTION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_db_executor_execution_seconds"),
            "Time database operations spend executing on a worker"
        ).buckets(vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0, 300.0]),
        &["lane", "priority"]
    ).expect("Failed to create db_executor_execution_seconds metric");

    pub static ref DB_EXECUTOR_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(
            format!("{PREFIX}_db_executor_operations_total"),
            "Database executor operations by outcome"
        ),
        &["lane", "priority", "outcome"]
    ).expect("Failed to create db_executor_operations_total metric");

    pub static ref DB_EXECUTOR_QUEUED: GaugeVec = GaugeVec::new(
        Opts::new(
            format!("{PREFIX}_db_executor_queued"),
            "Database operations currently queued by priority"
        ),
        &["priority"]
    ).expect("Failed to create db_executor_queued metric");

    pub static ref DB_EXECUTOR_ACTIVE: GaugeVec = GaugeVec::new(
        Opts::new(
            format!("{PREFIX}_db_executor_active"),
            "Database operations currently executing by lane"
        ),
        &["lane"]
    ).expect("Failed to create db_executor_active metric");

    pub static ref BLOCKING_WORK_QUEUE_WAIT_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_blocking_work_queue_wait_seconds"),
            "Time bounded blocking work spends waiting for capacity"
        ).buckets(vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]),
        &["pool"]
    ).expect("Failed to create blocking_work_queue_wait_seconds metric");

    pub static ref BLOCKING_WORK_EXECUTION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_blocking_work_execution_seconds"),
            "Time bounded blocking work spends executing"
        ).buckets(vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0, 60.0]),
        &["pool"]
    ).expect("Failed to create blocking_work_execution_seconds metric");

    pub static ref BLOCKING_WORK_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(
            format!("{PREFIX}_blocking_work_operations_total"),
            "Bounded blocking work operations by outcome"
        ),
        &["pool", "outcome"]
    ).expect("Failed to create blocking_work_operations_total metric");

    pub static ref BLOCKING_WORK_WAITING: GaugeVec = GaugeVec::new(
        Opts::new(
            format!("{PREFIX}_blocking_work_waiting"),
            "Bounded blocking operations currently waiting for capacity"
        ),
        &["pool"]
    ).expect("Failed to create blocking_work_waiting metric");

    pub static ref BLOCKING_WORK_ACTIVE: GaugeVec = GaugeVec::new(
        Opts::new(
            format!("{PREFIX}_blocking_work_active"),
            "Bounded blocking operations currently executing"
        ),
        &["pool"]
    ).expect("Failed to create blocking_work_active metric");

    // Catalog Metrics
    pub static ref CATALOG_ITEMS_TOTAL: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_catalog_items_total"), "Total items in catalog"),
        &["type"]
    ).expect("Failed to create catalog_items_total metric");

    pub static ref CATALOG_SIZE_BYTES: Gauge = Gauge::new(
        format!("{PREFIX}_catalog_size_bytes"),
        "Catalog size in bytes"
    ).expect("Failed to create catalog_size_bytes metric");

    // Error Metrics
    pub static ref ERRORS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_errors_total"), "Total errors by type and endpoint"),
        &["error_type", "endpoint"]
    ).expect("Failed to create errors_total metric");

    // Process Metrics (memory/CPU will be added later if needed)
    pub static ref PROCESS_MEMORY_BYTES: Gauge = Gauge::new(
        format!("{PREFIX}_process_memory_bytes"),
        "Process memory usage in bytes"
    ).expect("Failed to create process_memory_bytes metric");

    // Bandwidth Metrics
    pub static ref BANDWIDTH_BYTES_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_bandwidth_bytes_total"), "Total bytes transferred by endpoint category"),
        &["endpoint_category", "direction"]
    ).expect("Failed to create bandwidth_bytes_total metric");

    pub static ref BANDWIDTH_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_bandwidth_requests_total"), "Total requests by endpoint category"),
        &["endpoint_category"]
    ).expect("Failed to create bandwidth_requests_total metric");

    // Listening Stats Metrics
    pub static ref LISTENING_EVENTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_listening_events_total"), "Total listening events recorded"),
        &["client_type", "completed"]
    ).expect("Failed to create listening_events_total metric");

    pub static ref LISTENING_DURATION_SECONDS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_listening_duration_seconds_total"), "Total listening duration in seconds"),
        &["client_type"]
    ).expect("Failed to create listening_duration_seconds_total metric");

    // Changelog Batch Metrics
    pub static ref CHANGELOG_STALE_BATCHES: Gauge = Gauge::new(
        format!("{PREFIX}_changelog_stale_batches"),
        "Number of changelog batches that have been open longer than the stale threshold"
    ).expect("Failed to create changelog_stale_batches metric");

    pub static ref CHANGELOG_STALE_BATCH_CHECKS_TOTAL: Counter = Counter::new(
        format!("{PREFIX}_changelog_stale_batch_checks_total"),
        "Total number of stale batch checks performed"
    ).expect("Failed to create changelog_stale_batch_checks_total metric");

    // Downloader Metrics
    pub static ref DOWNLOADER_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_downloader_requests_total"), "Total requests to downloader service"),
        &["operation", "status"]
    ).expect("Failed to create downloader_requests_total metric");

    pub static ref DOWNLOADER_REQUEST_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_downloader_request_duration_seconds"),
            "Downloader request duration in seconds"
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0]),
        &["operation"]
    ).expect("Failed to create downloader_request_duration_seconds metric");

    pub static ref DOWNLOADER_ERRORS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_downloader_errors_total"), "Total downloader errors by type"),
        &["operation", "error_type"]
    ).expect("Failed to create downloader_errors_total metric");

    pub static ref DOWNLOADER_BYTES_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_downloader_bytes_total"), "Total bytes downloaded from downloader service"),
        &["content_type"]
    ).expect("Failed to create downloader_bytes_total metric");

    // Download Queue Metrics
    pub static ref DOWNLOAD_QUEUE_STALE_IN_PROGRESS: Gauge = Gauge::new(
        format!("{PREFIX}_download_queue_stale_in_progress"),
        "Number of download queue items stuck in IN_PROGRESS state longer than threshold"
    ).expect("Failed to create download_queue_stale_in_progress metric");

    pub static ref DOWNLOAD_QUEUE_SIZE: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_download_queue_size"), "Current download queue size by status and priority"),
        &["status", "priority"]
    ).expect("Failed to create download_queue_size metric");

    pub static ref DOWNLOAD_PROCESSED_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_download_processed_total"), "Total processed downloads by content type and result"),
        &["content_type", "result"]
    ).expect("Failed to create download_processed_total metric");

    pub static ref DOWNLOAD_PROCESSING_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_download_processing_duration_seconds"),
            "Download processing duration in seconds"
        )
        .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]),
        &["content_type"]
    ).expect("Failed to create download_processing_duration_seconds metric");

    pub static ref DOWNLOAD_CAPACITY_USED: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_download_capacity_used"), "Download capacity usage by period"),
        &["period"]
    ).expect("Failed to create download_capacity_used metric");

    pub static ref DOWNLOAD_USER_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_download_user_requests_total"), "Total user download requests by type"),
        &["request_type"]
    ).expect("Failed to create download_user_requests_total metric");

    pub static ref DOWNLOAD_AUDIT_EVENTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_download_audit_events_total"), "Total download audit events by type"),
        &["event_type"]
    ).expect("Failed to create download_audit_events_total metric");

    // Download Throttle Metrics
    pub static ref DOWNLOAD_THROTTLE_BYTES: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_download_throttle_bytes"), "Current throttle bytes usage by period"),
        &["period"]
    ).expect("Failed to create download_throttle_bytes metric");

    pub static ref DOWNLOAD_THROTTLE_LIMIT_BYTES: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_download_throttle_limit_bytes"), "Throttle limit by period"),
        &["period"]
    ).expect("Failed to create download_throttle_limit_bytes metric");

    pub static ref DOWNLOAD_THROTTLE_IS_THROTTLED: Gauge = Gauge::new(
        format!("{PREFIX}_download_throttle_is_throttled"),
        "Whether downloads are currently throttled (1 = yes, 0 = no)"
    ).expect("Failed to create download_throttle_is_throttled metric");

    // Corruption Handler Metrics
    pub static ref CORRUPTION_HANDLER_LEVEL: Gauge = Gauge::new(
        format!("{PREFIX}_corruption_handler_level"),
        "Current corruption handler escalation level (0 = base)"
    ).expect("Failed to create corruption_handler_level metric");

    pub static ref CORRUPTION_HANDLER_IN_COOLDOWN: Gauge = Gauge::new(
        format!("{PREFIX}_corruption_handler_in_cooldown"),
        "Whether corruption handler is in cooldown (1 = yes, 0 = no)"
    ).expect("Failed to create corruption_handler_in_cooldown metric");

    pub static ref CORRUPTION_HANDLER_COOLDOWN_SECS: Gauge = Gauge::new(
        format!("{PREFIX}_corruption_handler_cooldown_remaining_secs"),
        "Remaining cooldown time in seconds (0 if not in cooldown)"
    ).expect("Failed to create corruption_handler_cooldown_remaining_secs metric");

    pub static ref CORRUPTION_HANDLER_RESTARTS_TOTAL: Counter = Counter::new(
        format!("{PREFIX}_corruption_handler_restarts_total"),
        "Total downloader restarts triggered by corruption handler"
    ).expect("Failed to create corruption_handler_restarts_total metric");

    pub static ref CORRUPTION_HANDLER_CORRUPTIONS_TOTAL: Counter = Counter::new(
        format!("{PREFIX}_corruption_handler_corruptions_total"),
        "Total corruption events (ffprobe failures) detected"
    ).expect("Failed to create corruption_handler_corruptions_total metric");

    // Background Job Metrics
    pub static ref BACKGROUND_JOB_EXECUTIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_background_job_executions_total"), "Total background job executions"),
        &["job_id", "status"]
    ).expect("Failed to create background_job_executions_total metric");

    pub static ref BACKGROUND_JOB_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_background_job_duration_seconds"),
            "Background job execution duration in seconds"
        )
        .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0]),
        &["job_id"]
    ).expect("Failed to create background_job_duration_seconds metric");

    pub static ref BACKGROUND_JOB_RUNNING: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_background_job_running"), "Whether a background job is currently running"),
        &["job_id"]
    ).expect("Failed to create background_job_running metric");

    pub static ref BACKGROUND_JOB_QUEUED: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_background_job_queued"), "Background jobs waiting for scheduler capacity"),
        &["resource_class"]
    ).expect("Failed to create background_job_queued metric");

    pub static ref BACKGROUND_JOB_ACTIVE: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_background_job_active"), "Background jobs actively executing"),
        &["resource_class"]
    ).expect("Failed to create background_job_active metric");

    pub static ref BACKGROUND_JOB_QUEUE_WAIT_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            format!("{PREFIX}_background_job_queue_wait_seconds"),
            "Time background jobs wait for global and resource-class capacity"
        )
        .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0, 60.0]),
        &["resource_class"]
    ).expect("Failed to create background_job_queue_wait_seconds metric");

    pub static ref BACKGROUND_JOB_CIRCUIT_OPEN: GaugeVec = GaugeVec::new(
        Opts::new(format!("{PREFIX}_background_job_circuit_open"), "Whether a job circuit breaker is currently open"),
        &["job_id"]
    ).expect("Failed to create background_job_circuit_open metric");

    pub static ref BACKGROUND_JOB_CIRCUIT_TRIPS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_background_job_circuit_trips_total"), "Background job circuit breaker trips"),
        &["job_id"]
    ).expect("Failed to create background_job_circuit_trips_total metric");

    // Homelab Storage Metrics (standardized format for monitoring)
    pub static ref HOMELAB_STORAGE_BYTES: GaugeVec = GaugeVec::new(
        Opts::new("homelab_storage_bytes", "Storage usage in bytes"),
        &["service", "path"]
    ).expect("Failed to create homelab_storage_bytes metric");
}

/// Initialize all metrics and register them with the Prometheus registry
pub fn init_metrics() {
    // Register all metrics - ignore errors if already registered (for tests)
    let _ = REGISTRY.register(Box::new(HTTP_REQUESTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(HTTP_REQUEST_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(AUTH_LOGIN_ATTEMPTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(AUTH_LOGIN_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(AUTH_ACTIVE_SESSIONS.clone()));
    let _ = REGISTRY.register(Box::new(RATE_LIMIT_HITS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DB_QUERY_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(DB_CONNECTION_ERRORS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DB_EXECUTOR_QUEUE_WAIT_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(DB_EXECUTOR_EXECUTION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(DB_EXECUTOR_OPERATIONS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DB_EXECUTOR_QUEUED.clone()));
    let _ = REGISTRY.register(Box::new(DB_EXECUTOR_ACTIVE.clone()));
    let _ = REGISTRY.register(Box::new(BLOCKING_WORK_QUEUE_WAIT_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(BLOCKING_WORK_EXECUTION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(BLOCKING_WORK_OPERATIONS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(BLOCKING_WORK_WAITING.clone()));
    let _ = REGISTRY.register(Box::new(BLOCKING_WORK_ACTIVE.clone()));
    let _ = REGISTRY.register(Box::new(CATALOG_ITEMS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(CATALOG_SIZE_BYTES.clone()));
    let _ = REGISTRY.register(Box::new(ERRORS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(PROCESS_MEMORY_BYTES.clone()));
    let _ = REGISTRY.register(Box::new(BANDWIDTH_BYTES_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(BANDWIDTH_REQUESTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(LISTENING_EVENTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(LISTENING_DURATION_SECONDS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(CHANGELOG_STALE_BATCHES.clone()));
    let _ = REGISTRY.register(Box::new(CHANGELOG_STALE_BATCH_CHECKS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOADER_REQUESTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOADER_REQUEST_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOADER_ERRORS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOADER_BYTES_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_QUEUE_STALE_IN_PROGRESS.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_QUEUE_SIZE.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_PROCESSED_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_PROCESSING_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_CAPACITY_USED.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_USER_REQUESTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_AUDIT_EVENTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_THROTTLE_BYTES.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_THROTTLE_LIMIT_BYTES.clone()));
    let _ = REGISTRY.register(Box::new(DOWNLOAD_THROTTLE_IS_THROTTLED.clone()));
    let _ = REGISTRY.register(Box::new(CORRUPTION_HANDLER_LEVEL.clone()));
    let _ = REGISTRY.register(Box::new(CORRUPTION_HANDLER_IN_COOLDOWN.clone()));
    let _ = REGISTRY.register(Box::new(CORRUPTION_HANDLER_COOLDOWN_SECS.clone()));
    let _ = REGISTRY.register(Box::new(CORRUPTION_HANDLER_RESTARTS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(CORRUPTION_HANDLER_CORRUPTIONS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_EXECUTIONS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_RUNNING.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_QUEUED.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_ACTIVE.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_QUEUE_WAIT_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_CIRCUIT_OPEN.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_CIRCUIT_TRIPS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(HOMELAB_STORAGE_BYTES.clone()));

    tracing::info!("Metrics system initialized successfully");
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutorOutcome {
    Success,
    StoreError,
    QueueTimeout,
    ExecutionTimeout,
    ShuttingDown,
    Panicked,
}

impl ExecutorOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::StoreError => "store_error",
            Self::QueueTimeout => "queue_timeout",
            Self::ExecutionTimeout => "execution_timeout",
            Self::ShuttingDown => "shutting_down",
            Self::Panicked => "panicked",
        }
    }
}

pub(crate) fn db_executor_enqueued(priority: DbPriority) {
    DB_EXECUTOR_QUEUED
        .with_label_values(&[priority.label()])
        .inc();
}

pub(crate) fn db_executor_started(lane: DbLane, priority: DbPriority, queue_wait: Duration) {
    DB_EXECUTOR_QUEUED
        .with_label_values(&[priority.label()])
        .dec();
    DB_EXECUTOR_ACTIVE.with_label_values(&[lane.label()]).inc();
    DB_EXECUTOR_QUEUE_WAIT_SECONDS
        .with_label_values(&[lane.label(), priority.label()])
        .observe(queue_wait.as_secs_f64());
}

pub(crate) fn db_executor_cancelled(priority: DbPriority) {
    DB_EXECUTOR_QUEUED
        .with_label_values(&[priority.label()])
        .dec();
}

pub(crate) fn db_executor_execution_finished(
    lane: DbLane,
    priority: DbPriority,
    execution: Duration,
) {
    DB_EXECUTOR_ACTIVE.with_label_values(&[lane.label()]).dec();
    DB_EXECUTOR_EXECUTION_SECONDS
        .with_label_values(&[lane.label(), priority.label()])
        .observe(execution.as_secs_f64());
}

pub(crate) fn record_db_executor_outcome(
    lane: DbLane,
    priority: DbPriority,
    outcome: ExecutorOutcome,
) {
    DB_EXECUTOR_OPERATIONS_TOTAL
        .with_label_values(&[lane.label(), priority.label(), outcome.label()])
        .inc();
}

pub(crate) fn blocking_work_waiting(pool: &'static str, waiting: bool) {
    let metric = BLOCKING_WORK_WAITING.with_label_values(&[pool]);
    if waiting {
        metric.inc();
    } else {
        metric.dec();
    }
}

pub(crate) fn blocking_work_started(pool: &'static str, queue_wait: Duration) {
    BLOCKING_WORK_ACTIVE.with_label_values(&[pool]).inc();
    BLOCKING_WORK_QUEUE_WAIT_SECONDS
        .with_label_values(&[pool])
        .observe(queue_wait.as_secs_f64());
}

pub(crate) fn blocking_work_finished(pool: &'static str, execution: Duration) {
    BLOCKING_WORK_ACTIVE.with_label_values(&[pool]).dec();
    BLOCKING_WORK_EXECUTION_SECONDS
        .with_label_values(&[pool])
        .observe(execution.as_secs_f64());
}

pub(crate) fn record_blocking_work_outcome(pool: &'static str, outcome: ExecutorOutcome) {
    BLOCKING_WORK_OPERATIONS_TOTAL
        .with_label_values(&[pool, outcome.label()])
        .inc();
}

/// Initialize catalog-specific metrics
pub fn init_catalog_metrics(num_artists: usize, num_albums: usize, num_tracks: usize) {
    CATALOG_ITEMS_TOTAL
        .with_label_values(&["artist"])
        .set(num_artists as f64);

    CATALOG_ITEMS_TOTAL
        .with_label_values(&["album"])
        .set(num_albums as f64);

    CATALOG_ITEMS_TOTAL
        .with_label_values(&["track"])
        .set(num_tracks as f64);

    tracing::info!(
        "Catalog metrics initialized: {} artists, {} albums, {} tracks",
        num_artists,
        num_albums,
        num_tracks
    );
}

/// Record an HTTP request
pub fn record_http_request(method: &str, route: &str, status: u16, duration: Duration) {
    let method = http_method_label(method);
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, route, &status.to_string()])
        .inc();

    HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[method, route])
        .observe(duration.as_secs_f64());
}

/// Return only Axum's bounded route template, never a raw request URI.
pub fn request_route_label(extensions: &Extensions) -> &str {
    extensions
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(UNMATCHED_ROUTE_LABEL)
}

/// Keep arbitrary HTTP extension methods from creating attacker-controlled labels.
fn http_method_label(method: &str) -> &str {
    match method {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" | "CONNECT" | "TRACE" => {
            method
        }
        _ => "OTHER",
    }
}

/// Record a login attempt
pub fn record_login_attempt(status: &str, duration: Duration) {
    AUTH_LOGIN_ATTEMPTS_TOTAL.with_label_values(&[status]).inc();

    AUTH_LOGIN_DURATION_SECONDS.observe(duration.as_secs_f64());
}

/// Update active sessions count
pub fn set_active_sessions(count: i64) {
    AUTH_ACTIVE_SESSIONS.set(count as f64);
}

/// Record a rate limit hit
pub fn record_rate_limit_hit(endpoint: &str, identifier_type: &str) {
    RATE_LIMIT_HITS_TOTAL
        .with_label_values(&[endpoint, identifier_type])
        .inc();
}

/// Record a database query
pub fn record_db_query(operation: &str, duration: Duration) {
    DB_QUERY_DURATION_SECONDS
        .with_label_values(&[operation])
        .observe(duration.as_secs_f64());
}

/// Record a database connection error
pub fn record_db_connection_error() {
    DB_CONNECTION_ERRORS_TOTAL.inc();
}

/// Record an error
pub fn record_error(error_type: &str, endpoint: &str) {
    ERRORS_TOTAL
        .with_label_values(&[error_type, endpoint])
        .inc();
}

/// Categorize an endpoint path into a high-level category for bandwidth tracking
pub fn categorize_endpoint(path: &str) -> &'static str {
    if path.starts_with("/v1/content/stream") || path.starts_with("/v1/playback") {
        "stream"
    } else if path.starts_with("/v1/content/image") {
        "image"
    } else if path.starts_with("/v1/content") || path.starts_with("/v1/catalog") {
        "catalog"
    } else if path.starts_with("/v1/search") {
        "search"
    } else if path.starts_with("/v1/auth") {
        "auth"
    } else if path.starts_with("/v1/user") {
        "user"
    } else if path.starts_with("/v1/admin") {
        "admin"
    } else {
        "other"
    }
}

/// Record bandwidth usage for a request/response
pub fn record_bandwidth(endpoint_category: &str, response_bytes: u64) {
    BANDWIDTH_BYTES_TOTAL
        .with_label_values(&[endpoint_category, "response"])
        .inc_by(response_bytes as f64);

    BANDWIDTH_REQUESTS_TOTAL
        .with_label_values(&[endpoint_category])
        .inc();
}

/// Record a listening event
pub fn record_listening_event(client_type: Option<&str>, completed: bool, duration_seconds: u32) {
    let client_type_str = client_type.unwrap_or("unknown");
    let completed_str = if completed { "true" } else { "false" };

    LISTENING_EVENTS_TOTAL
        .with_label_values(&[client_type_str, completed_str])
        .inc();

    LISTENING_DURATION_SECONDS_TOTAL
        .with_label_values(&[client_type_str])
        .inc_by(duration_seconds as f64);
}

/// Record a successful downloader request
pub fn record_downloader_request(operation: &str, duration: Duration) {
    DOWNLOADER_REQUESTS_TOTAL
        .with_label_values(&[operation, "success"])
        .inc();

    DOWNLOADER_REQUEST_DURATION_SECONDS
        .with_label_values(&[operation])
        .observe(duration.as_secs_f64());
}

/// Record a failed downloader request
pub fn record_downloader_error(operation: &str, error_type: &str) {
    DOWNLOADER_REQUESTS_TOTAL
        .with_label_values(&[operation, "error"])
        .inc();

    DOWNLOADER_ERRORS_TOTAL
        .with_label_values(&[operation, error_type])
        .inc();
}

/// Record bytes downloaded from the downloader service
pub fn record_downloader_bytes(content_type: &str, bytes: u64) {
    DOWNLOADER_BYTES_TOTAL
        .with_label_values(&[content_type])
        .inc_by(bytes as f64);
}

/// Set the count of stale in-progress download queue items
pub fn set_download_stale_in_progress(count: usize) {
    DOWNLOAD_QUEUE_STALE_IN_PROGRESS.set(count as f64);
}

/// Set the download queue size for a specific status and priority
pub fn set_download_queue_size(status: &str, priority: u8, count: usize) {
    DOWNLOAD_QUEUE_SIZE
        .with_label_values(&[status, &priority.to_string()])
        .set(count as f64);
}

/// Record a processed download
pub fn record_download_processed(content_type: &str, result: &str, duration: Duration) {
    DOWNLOAD_PROCESSED_TOTAL
        .with_label_values(&[content_type, result])
        .inc();

    DOWNLOAD_PROCESSING_DURATION_SECONDS
        .with_label_values(&[content_type])
        .observe(duration.as_secs_f64());
}

/// Set the download capacity usage for a period
pub fn set_download_capacity_used(period: &str, count: usize) {
    DOWNLOAD_CAPACITY_USED
        .with_label_values(&[period])
        .set(count as f64);
}

/// Record a user download request
pub fn record_download_user_request(request_type: &str) {
    DOWNLOAD_USER_REQUESTS_TOTAL
        .with_label_values(&[request_type])
        .inc();
}

/// Record a download audit event
pub fn record_download_audit_event(event_type: &str) {
    DOWNLOAD_AUDIT_EVENTS_TOTAL
        .with_label_values(&[event_type])
        .inc();
}

/// Update throttle metrics with current stats
pub fn update_throttle_metrics(
    bytes_last_minute: u64,
    bytes_last_hour: u64,
    max_bytes_per_minute: u64,
    max_bytes_per_hour: u64,
    is_throttled: bool,
) {
    DOWNLOAD_THROTTLE_BYTES
        .with_label_values(&["minute"])
        .set(bytes_last_minute as f64);
    DOWNLOAD_THROTTLE_BYTES
        .with_label_values(&["hour"])
        .set(bytes_last_hour as f64);
    DOWNLOAD_THROTTLE_LIMIT_BYTES
        .with_label_values(&["minute"])
        .set(max_bytes_per_minute as f64);
    DOWNLOAD_THROTTLE_LIMIT_BYTES
        .with_label_values(&["hour"])
        .set(max_bytes_per_hour as f64);
    DOWNLOAD_THROTTLE_IS_THROTTLED.set(if is_throttled { 1.0 } else { 0.0 });
}

/// Update corruption handler metrics with current state
pub fn update_corruption_handler_metrics(
    level: u32,
    in_cooldown: bool,
    cooldown_remaining_secs: u64,
) {
    CORRUPTION_HANDLER_LEVEL.set(level as f64);
    CORRUPTION_HANDLER_IN_COOLDOWN.set(if in_cooldown { 1.0 } else { 0.0 });
    CORRUPTION_HANDLER_COOLDOWN_SECS.set(cooldown_remaining_secs as f64);
}

/// Record a corruption event (ffprobe failure)
pub fn record_corruption_event() {
    CORRUPTION_HANDLER_CORRUPTIONS_TOTAL.inc();
}

/// Record a downloader restart triggered by corruption handler
pub fn record_corruption_handler_restart() {
    CORRUPTION_HANDLER_RESTARTS_TOTAL.inc();
}

/// Record a background job execution
pub fn record_background_job_execution(job_id: &str, status: &str, duration: Duration) {
    BACKGROUND_JOB_EXECUTIONS_TOTAL
        .with_label_values(&[job_id, status])
        .inc();

    BACKGROUND_JOB_DURATION_SECONDS
        .with_label_values(&[job_id])
        .observe(duration.as_secs_f64());
}

/// Set whether a background job is currently running
pub fn set_background_job_running(job_id: &str, running: bool) {
    BACKGROUND_JOB_RUNNING
        .with_label_values(&[job_id])
        .set(if running { 1.0 } else { 0.0 });
}

pub(crate) fn background_job_waiting(resource_class: &str, waiting: bool) {
    let metric = BACKGROUND_JOB_QUEUED.with_label_values(&[resource_class]);
    if waiting {
        metric.inc();
    } else {
        metric.dec();
    }
}

pub(crate) fn background_job_started(resource_class: &str, queue_wait: Duration) {
    BACKGROUND_JOB_QUEUE_WAIT_SECONDS
        .with_label_values(&[resource_class])
        .observe(queue_wait.as_secs_f64());
    BACKGROUND_JOB_ACTIVE
        .with_label_values(&[resource_class])
        .inc();
}

pub(crate) fn background_job_finished(resource_class: &str) {
    BACKGROUND_JOB_ACTIVE
        .with_label_values(&[resource_class])
        .dec();
}

pub(crate) fn set_background_job_circuit_open(job_id: &str, open: bool) {
    BACKGROUND_JOB_CIRCUIT_OPEN
        .with_label_values(&[job_id])
        .set(if open { 1.0 } else { 0.0 });
}

pub(crate) fn record_background_job_circuit_trip(job_id: &str) {
    BACKGROUND_JOB_CIRCUIT_TRIPS_TOTAL
        .with_label_values(&[job_id])
        .inc();
}

/// Update process memory usage
async fn update_memory_usage(filesystem_work: &FilesystemWorkPool) {
    // Get current process memory usage
    #[cfg(target_os = "linux")]
    {
        if let Ok(Ok(status)) = filesystem_work
            .read(std::path::PathBuf::from("/proc/self/status"))
            .await
        {
            for line in String::from_utf8_lossy(&status).lines() {
                if line.starts_with("VmRSS:") {
                    // Parse the RSS (Resident Set Size) in kB
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<f64>() {
                            // Convert kB to bytes
                            PROCESS_MEMORY_BYTES.set(kb * 1024.0);
                            return;
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    let _ = filesystem_work;

    // Fallback for non-Linux systems or if reading fails
    // We'll just not update the metric
}

/// Handler for the /metrics endpoint
pub(super) async fn metrics_handler(
    State(filesystem_work): State<FilesystemWorkPool>,
) -> impl IntoResponse {
    let request_started = Instant::now();

    // Update memory usage before returning metrics
    update_memory_usage(&filesystem_work).await;

    let encoder = TextEncoder::new();
    let gather_started = Instant::now();
    let metric_families = REGISTRY.gather();
    let gather_elapsed = gather_started.elapsed();

    let mut buffer = vec![];
    let encode_started = Instant::now();
    let result = encoder.encode(&metric_families, &mut buffer);
    let encode_elapsed = encode_started.elapsed();
    let request_elapsed = request_started.elapsed();
    if request_elapsed >= Duration::from_secs(1) {
        tracing::warn!(
            total_ms = request_elapsed.as_millis() as u64,
            gather_ms = gather_elapsed.as_millis() as u64,
            encode_ms = encode_elapsed.as_millis() as u64,
            metric_families = metric_families.len(),
            response_bytes = buffer.len(),
            "Slow Prometheus metrics response"
        );
    }

    match result {
        Ok(()) => {
            let response = String::from_utf8(buffer).unwrap_or_else(|_| String::from(""));
            (StatusCode::OK, response)
        }
        Err(e) => {
            tracing::error!("Failed to encode metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to encode metrics: {}", e),
            )
        }
    }
}

// =============================================================================
// Storage Metrics (Homelab Standard)
// =============================================================================

/// Storage metrics breakdown for the service
#[derive(Debug, Default)]
pub struct StorageMetrics {
    /// Total storage for the service (root "/")
    pub total: u64,
    /// Database storage breakdown
    pub db_total: u64,
    pub db_catalog: u64,
    pub db_user: u64,
    pub db_server: u64,
    pub db_download_queue: u64,
    pub db_search: u64,
    /// Catalog/media storage breakdown
    pub catalog_total: u64,
    pub catalog_audio: u64,
    pub catalog_images: u64,
}

/// Get the size of a single file
fn get_file_size(path: &Path) -> u64 {
    path.metadata().map(|m| m.len()).unwrap_or(0)
}

/// Calculate storage metrics for the service
///
/// # Arguments
/// * `db_dir` - Path to the database directory containing SQLite files
/// * `media_path` - Path to the media directory containing audio and images
pub fn calculate_storage_metrics(db_dir: &Path, media_path: &Path) -> StorageMetrics {
    // Database files
    let db_catalog = get_file_size(&db_dir.join("catalog.db"));
    let db_user = get_file_size(&db_dir.join("user.db"));
    let db_server = get_file_size(&db_dir.join("server.db"));
    let db_download_queue = get_file_size(&db_dir.join("download_queue.db"));
    let db_search = get_file_size(&db_dir.join("search.db"));
    let db_total = db_catalog + db_user + db_server + db_download_queue + db_search;

    // Catalog/media directories
    let catalog_audio = crate::media::directory_size(&media_path.join("audio"));
    let catalog_images = crate::media::directory_size(&media_path.join("images"));
    let catalog_total = catalog_audio + catalog_images;

    // Total
    let total = db_total + catalog_total;

    StorageMetrics {
        total,
        db_total,
        db_catalog,
        db_user,
        db_server,
        db_download_queue,
        db_search,
        catalog_total,
        catalog_audio,
        catalog_images,
    }
}

/// Update the homelab storage metrics with current values
pub fn update_storage_metrics(db_dir: &Path, media_path: &Path) {
    let metrics = calculate_storage_metrics(db_dir, media_path);

    // Root total
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/"])
        .set(metrics.total as f64);

    // Database breakdown
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db"])
        .set(metrics.db_total as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db/catalog"])
        .set(metrics.db_catalog as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db/user"])
        .set(metrics.db_user as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db/server"])
        .set(metrics.db_server as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db/download_queue"])
        .set(metrics.db_download_queue as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/db/search"])
        .set(metrics.db_search as f64);

    // Catalog/media breakdown
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/catalog"])
        .set(metrics.catalog_total as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/catalog/audio"])
        .set(metrics.catalog_audio as f64);
    HOMELAB_STORAGE_BYTES
        .with_label_values(&[SERVICE_NAME, "/catalog/images"])
        .set(metrics.catalog_images as f64);

    tracing::debug!(
        "Storage metrics updated: total={}, db={}, catalog={}",
        metrics.total,
        metrics.db_total,
        metrics.catalog_total
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn metrics_handler_stays_responsive_when_filesystem_capacity_is_exhausted() {
        let pool = crate::server::filesystem_work::FilesystemWorkPool::with_limits(
            1,
            Duration::from_millis(20),
            Duration::from_secs(1),
        );
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let blocker_pool = pool.clone();
        let blocker_gate = gate.clone();
        let blocker = tokio::spawn(async move {
            blocker_pool
                .run(move || {
                    started_tx.send(()).unwrap();
                    let (lock, condvar) = &*blocker_gate;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = condvar.wait(released).unwrap();
                    }
                })
                .await
        });
        started_rx.await.unwrap();

        let started = Instant::now();
        let response = metrics_handler(axum::extract::State(pool))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(started.elapsed() < Duration::from_millis(200));

        let (lock, condvar) = &*gate;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        blocker.await.unwrap().unwrap();
    }

    #[test]
    fn executor_observability_metrics_are_registered() {
        init_metrics();
        db_executor_enqueued(DbPriority::Interactive);
        db_executor_started(
            DbLane::CatalogRead,
            DbPriority::Interactive,
            Duration::from_millis(2),
        );
        db_executor_execution_finished(
            DbLane::CatalogRead,
            DbPriority::Interactive,
            Duration::from_millis(3),
        );
        record_db_executor_outcome(
            DbLane::CatalogRead,
            DbPriority::Interactive,
            ExecutorOutcome::Success,
        );
        blocking_work_waiting("password", true);
        blocking_work_waiting("password", false);
        blocking_work_started("password", Duration::from_millis(2));
        blocking_work_finished("password", Duration::from_millis(3));
        record_blocking_work_outcome("password", ExecutorOutcome::Success);
        background_job_waiting("io_bound", true);
        background_job_waiting("io_bound", false);
        background_job_started("io_bound", Duration::from_millis(4));
        background_job_finished("io_bound");
        set_background_job_circuit_open("test_job", true);
        set_background_job_circuit_open("test_job", false);
        record_background_job_circuit_trip("test_job");

        let metric_names = REGISTRY
            .gather()
            .into_iter()
            .map(|family| family.get_name().to_owned())
            .collect::<std::collections::HashSet<_>>();
        for expected in [
            "pezzottify_db_executor_queue_wait_seconds",
            "pezzottify_db_executor_execution_seconds",
            "pezzottify_db_executor_operations_total",
            "pezzottify_db_executor_queued",
            "pezzottify_db_executor_active",
            "pezzottify_blocking_work_queue_wait_seconds",
            "pezzottify_blocking_work_execution_seconds",
            "pezzottify_blocking_work_operations_total",
            "pezzottify_blocking_work_waiting",
            "pezzottify_blocking_work_active",
            "pezzottify_background_job_queued",
            "pezzottify_background_job_active",
            "pezzottify_background_job_queue_wait_seconds",
            "pezzottify_background_job_circuit_open",
            "pezzottify_background_job_circuit_trips_total",
        ] {
            assert!(metric_names.contains(expected), "missing metric {expected}");
        }
    }

    #[test]
    fn test_metrics_initialization() {
        // This test ensures metrics can be initialized without panic
        init_metrics();

        // Verify we can gather metrics
        let metric_families = REGISTRY.gather();
        assert!(!metric_families.is_empty(), "Metrics should be registered");
    }

    #[test]
    fn test_record_http_request() {
        // Ensure metrics are initialized
        init_metrics();

        // Record a sample request
        record_http_request(
            "GET",
            "/v1/content/track/123",
            200,
            Duration::from_millis(50),
        );

        // Verify the counter was incremented
        let metrics = REGISTRY.gather();
        let http_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_http_requests_total");

        assert!(http_metrics.is_some(), "HTTP request metrics should exist");
    }

    #[test]
    fn non_standard_http_methods_share_one_bounded_label() {
        init_metrics();
        let route = "/__cardinality_test__/{id}";
        record_http_request("ATTACKER-METHOD-ONE", route, 200, Duration::ZERO);
        record_http_request("ATTACKER-METHOD-TWO", route, 200, Duration::ZERO);

        let matching_metrics: Vec<_> = REGISTRY
            .gather()
            .into_iter()
            .filter(|family| family.get_name() == "pezzottify_http_requests_total")
            .flat_map(|family| family.get_metric().to_vec())
            .filter(|metric| {
                metric
                    .get_label()
                    .iter()
                    .any(|label| label.get_name() == "path" && label.get_value() == route)
            })
            .collect();

        assert_eq!(matching_metrics.len(), 1);
        assert!(matching_metrics[0]
            .get_label()
            .iter()
            .any(|label| label.get_name() == "method" && label.get_value() == "OTHER"));
        assert_eq!(matching_metrics[0].get_counter().get_value(), 2.0);
    }

    #[test]
    fn test_record_login_attempt() {
        // Ensure metrics are initialized
        init_metrics();

        record_login_attempt("success", Duration::from_secs(1));
        record_login_attempt("failure", Duration::from_millis(500));

        // Verify metrics were recorded
        let metrics = REGISTRY.gather();
        let login_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_auth_login_attempts_total");

        assert!(login_metrics.is_some(), "Login metrics should exist");
    }

    #[test]
    fn test_record_rate_limit_hit() {
        // Ensure metrics are initialized
        init_metrics();

        record_rate_limit_hit("/v1/auth/login", "ip");

        let metrics = REGISTRY.gather();
        let rate_limit_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_rate_limit_hits_total");

        assert!(
            rate_limit_metrics.is_some(),
            "Rate limit metrics should exist"
        );
    }

    #[test]
    fn test_catalog_metrics() {
        // Ensure metrics are initialized
        init_metrics();

        init_catalog_metrics(100, 500, 2000);

        let metrics = REGISTRY.gather();
        let catalog_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_catalog_items_total");

        assert!(catalog_metrics.is_some(), "Catalog metrics should exist");
    }

    #[test]
    fn test_db_query_recording() {
        // Ensure metrics are initialized
        init_metrics();

        record_db_query("read", Duration::from_millis(10));
        record_db_query("write", Duration::from_millis(50));

        let metrics = REGISTRY.gather();
        let db_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_db_query_duration_seconds");

        assert!(db_metrics.is_some(), "DB query metrics should exist");
    }

    #[test]
    fn test_categorize_endpoint() {
        // Stream endpoints
        assert_eq!(categorize_endpoint("/v1/content/stream/track123"), "stream");
        assert_eq!(categorize_endpoint("/v1/playback/queue"), "stream");

        // Image endpoints
        assert_eq!(categorize_endpoint("/v1/content/image/abc123"), "image");

        // Catalog endpoints
        assert_eq!(categorize_endpoint("/v1/content/track/123"), "catalog");
        assert_eq!(categorize_endpoint("/v1/content/album/456"), "catalog");
        assert_eq!(categorize_endpoint("/v1/content/artist/789"), "catalog");
        assert_eq!(categorize_endpoint("/v1/catalog/artists"), "catalog");

        // Search endpoints
        assert_eq!(categorize_endpoint("/v1/search/query"), "search");

        // Auth endpoints
        assert_eq!(categorize_endpoint("/v1/auth/login"), "auth");
        assert_eq!(categorize_endpoint("/v1/auth/logout"), "auth");

        // User endpoints
        assert_eq!(categorize_endpoint("/v1/user/playlists"), "user");
        assert_eq!(categorize_endpoint("/v1/user/liked"), "user");

        // Admin endpoints
        assert_eq!(categorize_endpoint("/v1/admin/users"), "admin");

        // Other endpoints
        assert_eq!(categorize_endpoint("/"), "other");
        assert_eq!(categorize_endpoint("/health"), "other");
        assert_eq!(categorize_endpoint("/metrics"), "other");
    }

    #[test]
    fn test_record_bandwidth() {
        // Ensure metrics are initialized
        init_metrics();

        // Record bandwidth for two bounded endpoint categories.
        record_bandwidth("stream", 1024 * 1024);
        record_bandwidth("catalog", 512);

        // Verify metrics exist
        let metrics = REGISTRY.gather();
        let bandwidth_bytes = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_bandwidth_bytes_total");
        assert!(
            bandwidth_bytes.is_some(),
            "Bandwidth bytes metric should exist"
        );

        let bandwidth_requests = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_bandwidth_requests_total");
        assert!(
            bandwidth_requests.is_some(),
            "Bandwidth requests metric should exist"
        );
        for family in [bandwidth_bytes.unwrap(), bandwidth_requests.unwrap()] {
            assert!(family
                .get_metric()
                .iter()
                .flat_map(|metric| metric.get_label())
                .all(|label| label.get_name() != "user_id"));
        }
    }

    #[test]
    fn test_record_listening_event() {
        // Ensure metrics are initialized
        init_metrics();

        // Record a completed listening event
        record_listening_event(Some("android"), true, 180);

        // Record an incomplete listening event
        record_listening_event(Some("web"), false, 45);

        // Record without client type
        record_listening_event(None, true, 200);

        // Verify metrics exist
        let metrics = REGISTRY.gather();
        let listening_events = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_listening_events_total");
        assert!(
            listening_events.is_some(),
            "Listening events metric should exist"
        );

        let listening_duration = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_listening_duration_seconds_total");
        assert!(
            listening_duration.is_some(),
            "Listening duration metric should exist"
        );
    }

    #[test]
    fn test_record_downloader_request() {
        // Ensure metrics are initialized
        init_metrics();

        // Record a successful request
        record_downloader_request("get_artist", Duration::from_millis(500));

        // Record an error
        record_downloader_error("get_album", "connection");

        // Record bytes downloaded
        record_downloader_bytes("audio", 1024 * 1024);
        record_downloader_bytes("image", 50000);

        // Verify metrics exist
        let metrics = REGISTRY.gather();
        let requests = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_downloader_requests_total");
        assert!(
            requests.is_some(),
            "Downloader requests metric should exist"
        );

        let duration = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_downloader_request_duration_seconds");
        assert!(
            duration.is_some(),
            "Downloader duration metric should exist"
        );

        let errors = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_downloader_errors_total");
        assert!(errors.is_some(), "Downloader errors metric should exist");

        let bytes = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_downloader_bytes_total");
        assert!(bytes.is_some(), "Downloader bytes metric should exist");
    }

    #[test]
    fn test_record_background_job_execution() {
        // Ensure metrics are initialized
        init_metrics();

        // Record a successful job execution
        record_background_job_execution("test_job", "success", Duration::from_secs(5));

        // Record a failed job execution
        record_background_job_execution("test_job", "failed", Duration::from_secs(2));

        // Verify metrics exist
        let metrics = REGISTRY.gather();
        let executions = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_background_job_executions_total");
        assert!(
            executions.is_some(),
            "Background job executions metric should exist"
        );

        let duration = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_background_job_duration_seconds");
        assert!(
            duration.is_some(),
            "Background job duration metric should exist"
        );
    }

    #[test]
    fn test_set_background_job_running() {
        // Ensure metrics are initialized
        init_metrics();

        // Set job as running
        set_background_job_running("test_job", true);

        // Set job as not running
        set_background_job_running("test_job", false);

        // Verify metric exists
        let metrics = REGISTRY.gather();
        let running = metrics
            .iter()
            .find(|m| m.get_name() == "pezzottify_background_job_running");
        assert!(
            running.is_some(),
            "Background job running metric should exist"
        );
    }

    #[test]
    fn test_calculate_storage_metrics() {
        use std::io::Write;
        use tempfile::TempDir;

        // Create temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let db_dir = temp_dir.path().join("db");
        let media_path = temp_dir.path().join("media");
        std::fs::create_dir_all(&db_dir).unwrap();
        std::fs::create_dir_all(media_path.join("audio")).unwrap();
        std::fs::create_dir_all(media_path.join("images")).unwrap();

        // Create some test files with known sizes
        let mut catalog_db = std::fs::File::create(db_dir.join("catalog.db")).unwrap();
        catalog_db.write_all(&[0u8; 1000]).unwrap(); // 1KB

        let mut user_db = std::fs::File::create(db_dir.join("user.db")).unwrap();
        user_db.write_all(&[0u8; 500]).unwrap(); // 500 bytes

        let mut audio_file = std::fs::File::create(media_path.join("audio/track1.ogg")).unwrap();
        audio_file.write_all(&[0u8; 2000]).unwrap(); // 2KB

        let mut image_file = std::fs::File::create(media_path.join("images/cover1.jpg")).unwrap();
        image_file.write_all(&[0u8; 800]).unwrap(); // 800 bytes

        // Calculate storage metrics
        let metrics = calculate_storage_metrics(&db_dir, &media_path);

        // Verify database metrics
        assert_eq!(metrics.db_catalog, 1000);
        assert_eq!(metrics.db_user, 500);
        assert_eq!(metrics.db_server, 0); // file doesn't exist
        assert_eq!(metrics.db_download_queue, 0);
        assert_eq!(metrics.db_search, 0);
        assert_eq!(metrics.db_total, 1500);

        // Verify catalog metrics
        assert_eq!(metrics.catalog_audio, 2000);
        assert_eq!(metrics.catalog_images, 800);
        assert_eq!(metrics.catalog_total, 2800);

        // Verify total
        assert_eq!(metrics.total, 4300);
    }

    #[test]
    fn test_update_storage_metrics() {
        use std::io::Write;
        use tempfile::TempDir;

        init_metrics();

        // Create temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let db_dir = temp_dir.path().join("db");
        let media_path = temp_dir.path().join("media");
        std::fs::create_dir_all(&db_dir).unwrap();
        std::fs::create_dir_all(media_path.join("audio")).unwrap();
        std::fs::create_dir_all(media_path.join("images")).unwrap();

        // Create a test database file
        let mut catalog_db = std::fs::File::create(db_dir.join("catalog.db")).unwrap();
        catalog_db.write_all(&[0u8; 1000]).unwrap();

        // Update storage metrics
        update_storage_metrics(&db_dir, &media_path);

        // Verify metrics were registered
        let metrics = REGISTRY.gather();
        let storage_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "homelab_storage_bytes");
        assert!(storage_metrics.is_some(), "Storage metrics should exist");
    }
}
