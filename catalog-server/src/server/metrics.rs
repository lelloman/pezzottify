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
    AUTH_LOGIN_ATTEMPTS_TOTAL
        .with_label_values(&[status])
        .inc();

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
        record_http_request("GET", "/v1/content/track/123", 200, Duration::from_millis(50));

        // Verify the counter was incremented
        let metrics = REGISTRY.gather();
        let http_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "http_requests_total");

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
            .find(|m| m.get_name() == "auth_login_attempts_total");

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
            .find(|m| m.get_name() == "rate_limit_hits_total");

        assert!(rate_limit_metrics.is_some(), "Rate limit metrics should exist");
    }

    #[test]
    fn test_catalog_metrics() {
        // Ensure metrics are initialized
        init_metrics();

        init_catalog_metrics(100, 500, 2000);

        let metrics = REGISTRY.gather();
        let catalog_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "catalog_items_total");

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
            .find(|m| m.get_name() == "db_query_duration_seconds");

        assert!(db_metrics.is_some(), "DB query metrics should exist");
    }
}
