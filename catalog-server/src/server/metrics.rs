#![allow(dead_code)]

use axum::{http::StatusCode, response::IntoResponse};
use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::time::Duration;

/// Metric name prefix for all Pezzottify metrics
const PREFIX: &str = "pezzottify";

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
        Opts::new(format!("{PREFIX}_bandwidth_bytes_total"), "Total bytes transferred"),
        &["user_id", "endpoint_category", "direction"]
    ).expect("Failed to create bandwidth_bytes_total metric");

    pub static ref BANDWIDTH_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(format!("{PREFIX}_bandwidth_requests_total"), "Total requests by user and endpoint category"),
        &["user_id", "endpoint_category"]
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
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_EXECUTIONS_TOTAL.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_DURATION_SECONDS.clone()));
    let _ = REGISTRY.register(Box::new(BACKGROUND_JOB_RUNNING.clone()));

    tracing::info!("Metrics system initialized successfully");
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
pub fn record_http_request(method: &str, path: &str, status: u16, duration: Duration) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, path, &status.to_string()])
        .inc();

    HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[method, path])
        .observe(duration.as_secs_f64());
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
pub fn record_bandwidth(user_id: Option<usize>, endpoint_category: &str, response_bytes: u64) {
    let user_id_str = user_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    BANDWIDTH_BYTES_TOTAL
        .with_label_values(&[&user_id_str, endpoint_category, "response"])
        .inc_by(response_bytes as f64);

    BANDWIDTH_REQUESTS_TOTAL
        .with_label_values(&[&user_id_str, endpoint_category])
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

/// Update process memory usage
pub fn update_memory_usage() {
    // Get current process memory usage
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
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

    // Fallback for non-Linux systems or if reading fails
    // We'll just not update the metric
}

/// Handler for the /metrics endpoint
pub async fn metrics_handler() -> impl IntoResponse {
    // Update memory usage before returning metrics
    update_memory_usage();

    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();

    let mut buffer = vec![];
    match encoder.encode(&metric_families, &mut buffer) {
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

#[cfg(test)]
mod tests {
    use super::*;

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

        // Record bandwidth for authenticated user
        record_bandwidth(Some(42), "stream", 1024 * 1024);

        // Record bandwidth for anonymous user
        record_bandwidth(None, "catalog", 512);

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
}
