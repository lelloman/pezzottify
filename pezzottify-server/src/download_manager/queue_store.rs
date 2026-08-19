//! Download queue storage and persistence.
//!
//! Provides SQLite-backed storage for download queue items and related data.

use super::models::*;
use super::schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

/// Trait for download queue storage operations.
///
/// Provides methods for managing download queue items, tracking activity,
/// rate limiting, and audit logging.
pub trait DownloadQueueStore: Send + Sync {
    // === Queue Management ===

    /// Add a new item to the download queue.
    fn enqueue(&self, item: QueueItem) -> Result<()>;

    /// Get a queue item by ID.
    fn get_item(&self, id: &str) -> Result<Option<QueueItem>>;

    /// Delete a queue item by ID. Returns true if item was deleted.
    fn delete_item(&self, id: &str) -> Result<bool>;

    /// Get the next pending item to process (by priority, then age).
    fn get_next_pending(&self) -> Result<Option<QueueItem>>;

    /// List queue items for a specific user.
    fn list_by_user(
        &self,
        user_id: &str,
        status: Option<QueueStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    /// List all queue items with optional status filter.
    /// If `exclude_completed` is true, completed items are excluded from results.
    /// If `top_level_only` is true, only items with parent_id IS NULL are returned.
    fn list_all(
        &self,
        status: Option<QueueStatus>,
        exclude_completed: bool,
        top_level_only: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    /// Get the queue position for an item (1-based, among pending items).
    fn get_queue_position(&self, id: &str) -> Result<Option<usize>>;

    // === State Transitions (atomic) ===

    /// Atomically claim an item for processing (PENDING → IN_PROGRESS).
    /// Returns true if claimed, false if already claimed or not pending.
    fn claim_for_processing(&self, id: &str) -> Result<bool>;

    /// Mark an item as completed with metrics.
    fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;

    /// Mark an item for retry with next retry time and error details.
    fn mark_retry_waiting(&self, id: &str, next_retry_at: i64, error: &DownloadError)
        -> Result<()>;

    /// Mark an item as permanently failed.
    fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()>;

    // === Parent-Child Management ===

    /// Create child items for a parent (e.g., tracks for an album).
    fn create_children(&self, parent_id: &str, children: Vec<QueueItem>) -> Result<()>;

    /// Get all children of a parent item.
    fn get_children(&self, parent_id: &str) -> Result<Vec<QueueItem>>;

    /// Delete all children of a parent item. Returns the number of children deleted.
    fn delete_children(&self, parent_id: &str) -> Result<usize>;

    /// Get download progress for a parent item based on children status.
    fn get_children_progress(&self, parent_id: &str) -> Result<DownloadProgress>;

    /// Check if all children are in terminal state and return new parent status.
    /// Returns Some(status) if parent should be updated, None otherwise.
    fn check_parent_completion(&self, parent_id: &str) -> Result<Option<QueueStatus>>;

    /// Get top-level user requests (parent_id IS NULL only).
    fn get_user_requests(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    // === Retry Handling ===

    /// Get items ready for retry (next_retry_at <= now).
    fn get_retry_ready(&self) -> Result<Vec<QueueItem>>;

    /// Promote a retry-waiting item back to pending.
    fn promote_retry_to_pending(&self, id: &str) -> Result<()>;

    /// Reset any non-completed item back to pending (for admin retry).
    /// Clears error info and resets retry state.
    fn reset_to_pending(&self, id: &str) -> Result<()>;

    // === Duplicate/Existence Checks ===

    /// Find a queue item by content type and ID.
    fn find_by_content(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<Option<QueueItem>>;

    /// Check if content exists in the queue (any status).
    fn is_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool>;

    /// Check if content is in the active queue (non-terminal status).
    fn is_in_active_queue(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<bool>;

    /// Find all pending/active queue items for a content type and ID.
    /// Returns items with status PENDING, IN_PROGRESS, or RETRY_WAITING.
    fn find_pending_by_content(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<Vec<QueueItem>>;

    // === User Rate Limiting ===

    /// Get rate limit status for a user.
    fn get_user_stats(&self, user_id: &str) -> Result<UserLimitStatus>;

    /// Increment user's daily request count.
    fn increment_user_requests(&self, user_id: &str) -> Result<()>;

    /// Reset daily request counts for all users. Returns number of users reset.
    fn reset_daily_user_stats(&self) -> Result<usize>;

    // === Activity Tracking ===

    /// Record download activity for capacity tracking.
    fn record_activity(
        &self,
        content_type: DownloadContentType,
        bytes: u64,
        success: bool,
    ) -> Result<()>;

    /// Get activity log entries since a timestamp.
    fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>>;

    /// Get hourly download counts.
    fn get_hourly_counts(&self) -> Result<HourlyCounts>;

    /// Get daily download counts.
    fn get_daily_counts(&self) -> Result<DailyCounts>;

    /// Get download statistics history aggregated by period.
    /// - Hourly: last 48 hours (default if no custom range)
    /// - Daily: last 30 days (default if no custom range)
    /// - Weekly: last 12 weeks (default if no custom range)
    ///
    /// If `since` is provided, uses it as the start time instead of the period default.
    /// If `until` is provided, uses it as the end time (otherwise no upper bound).
    fn get_stats_history(
        &self,
        period: StatsPeriod,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<DownloadStatsHistory>;

    // === Statistics ===

    /// Get overall queue statistics.
    fn get_queue_stats(&self) -> Result<QueueStats>;

    /// Get failed items for review/retry.
    fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;

    /// Get items stuck in IN_PROGRESS state (for alerting).
    fn get_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<Vec<QueueItem>>;

    // === Audit Logging ===

    /// Log an audit event.
    fn log_audit_event(&self, event: AuditLogEntry) -> Result<()>;

    /// Get audit log entries with filtering. Returns (entries, total_count).
    fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;

    /// Get all audit entries for a specific queue item.
    fn get_audit_for_item(&self, queue_item_id: &str) -> Result<Vec<AuditLogEntry>>;

    /// Get audit entries for a user with time range. Returns (entries, total_count).
    fn get_audit_for_user(
        &self,
        user_id: &str,
        since: Option<i64>,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<AuditLogEntry>, usize)>;

    /// Clean up old audit entries. Returns number of entries deleted.
    fn cleanup_old_audit_entries(&self, older_than: i64) -> Result<usize>;
}

/// SQLite-backed download queue store.
///
/// Stores download queue items, activity logs, user rate limits, and audit entries.
pub struct SqliteDownloadQueueStore {
    conn: Arc<Mutex<Connection>>,
}

include!("sqlite_queue.rs");
include!("sqlite_queue_adapter.rs");
