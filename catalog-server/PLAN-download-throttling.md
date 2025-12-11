# Implementation Plan: Download Manager Throttling & Corruption Handler

## Overview

Add two new mechanisms to the download manager:
1. **Bandwidth Throttling** - Limit download rate to respect hidden downloader service limits
2. **Corruption Handler** - Detect file corruption patterns, trigger downloader restart, and manage cooldown

---

## Part 1: Bandwidth Throttling

### Goal
Prevent overwhelming the downloader service with a configurable MB/min and MB/hour limit.

### Design: Trait-based abstraction

```rust
// New file: catalog-server/src/download_manager/throttle.rs

#[async_trait]
pub trait DownloadThrottler: Send + Sync {
    /// Check if we can download right now based on historical usage.
    /// Returns Ok(()) if allowed, Err(wait_duration) if throttled.
    async fn check_bandwidth(&self) -> Result<(), Duration>;

    /// Record that we downloaded `bytes`. Called after successful download.
    async fn record_download(&self, bytes: u64);

    /// Reset state (for testing or manual override).
    async fn reset(&self);
}
```

### Implementation: SlidingWindowThrottler

Tracks bytes downloaded in sliding time windows.

**State:**
- `downloads: Mutex<VecDeque<(Instant, u64)>>` - timestamped download records
- `max_bytes_per_minute: u64`
- `max_bytes_per_hour: u64`

**Logic:**
- `check_bandwidth()`:
  - Prune entries older than 1 hour
  - Sum bytes in last minute, check against limit
  - Sum bytes in last hour, check against limit
  - If either exceeded, return Err with wait duration
  - No estimation needed - just checks historical usage
- `record_download(bytes)`: Push `(Instant::now(), bytes)` to deque (called after download with actual bytes)

### Configuration additions

```rust
// In DownloadManagerSettings
pub throttle_max_mb_per_minute: u64,  // default: 20
pub throttle_max_mb_per_hour: u64,    // default: 1500
pub throttle_enabled: bool,           // default: true
```

### Integration point

In `job_processor.rs`, before calling download:
```rust
// Check historical bandwidth usage (no estimation needed)
if let Err(wait) = self.throttler.check_bandwidth().await {
    // Skip this tick, will retry next cycle
    return Ok(None);
}
// ... do download ...
// Record actual bytes after download completes
self.throttler.record_download(actual_bytes).await;
```

### Files to create/modify

| File | Action |
|------|--------|
| `throttle.rs` | **CREATE** - Trait + SlidingWindowThrottler impl |
| `mod.rs` | MODIFY - Export throttle module |
| `models.rs` | MODIFY - Add config fields to DownloadManagerSettings |
| `manager.rs` | MODIFY - Initialize throttler, pass to processor |
| `job_processor.rs` | MODIFY - Check throttle before download, record after |

---

## Part 2: Corruption Handler

### Goal
Detect when downloader is producing corrupted files and trigger a restart with "springy" cooldown.

**What counts as corruption:**
- Corruption = ffprobe failure specifically (file exists but is invalid/unplayable)
- Network errors, disk errors, download failures → handled by existing retry logic
- ffprobe failure indicates the external service is broken (failed decryption) or poisoning us → back off

### Design: CorruptionHandler struct

```rust
// New file: catalog-server/src/download_manager/corruption_handler.rs

pub struct CorruptionHandler {
    // Sliding window of recent results (true = success, false = corruption)
    recent_results: Mutex<VecDeque<bool>>,

    // Cooldown state
    state: Mutex<CooldownState>,

    // Mutex to prevent concurrent restart attempts
    restart_in_progress: Mutex<bool>,

    // Config
    config: CorruptionHandlerConfig,
}

struct CooldownState {
    current_level: u32,                   // 0 = base, 1 = 2x, 2 = 4x, etc.
    last_restart_at: Option<Instant>,
    successes_since_last_level_change: u32,
}

pub struct CorruptionHandlerConfig {
    pub window_size: usize,               // 4
    pub failure_threshold: usize,         // 2 (50% = restart)
    pub base_cooldown_secs: u64,          // 600 (10 min)
    pub max_cooldown_secs: u64,           // 7200 (2 hours)
    pub cooldown_multiplier: f64,         // 2.0
    pub successes_to_deescalate: u32,     // 10 (successful downloads to decrease level by 1)
}
```

### API

```rust
impl CorruptionHandler {
    /// Record a download result (success or corruption).
    /// - If success: increments success counter, may de-escalate level
    /// - If corruption: adds to sliding window, may return RestartNeeded
    pub async fn record_result(&self, success: bool) -> WatchdogAction;

    /// Called after a restart has been triggered and completed.
    /// Escalates the cooldown level and resets counters.
    pub async fn record_restart(&self);

    /// Check if we're currently in cooldown.
    pub async fn is_in_cooldown(&self) -> bool;

    /// Get current cooldown duration based on level (for logging/metrics).
    pub fn current_cooldown_duration(&self) -> Duration;

    /// Get current state for diagnostics/admin API.
    pub async fn get_state(&self) -> WatchdogState;

    /// Try to acquire restart lock. Returns guard if successful, None if restart in progress.
    pub async fn try_lock_restart(&self) -> Option<RestartGuard>;

    /// Admin reset: immediately resets level to 0 and clears cooldown.
    pub async fn admin_reset(&self);
}

pub enum WatchdogAction {
    Continue,
    RestartNeeded,
}

pub struct WatchdogState {
    pub current_level: u32,
    pub successes_since_last_level_change: u32,
    pub in_cooldown: bool,
    pub cooldown_remaining: Option<Duration>,
    pub recent_results: Vec<bool>,  // for visibility into sliding window
}
```

### Cooldown level logic

**Cooldown duration formula:**
```
duration = min(base_cooldown * 2^level, max_cooldown)

level 0 → 10 min
level 1 → 20 min
level 2 → 40 min
level 3 → 80 min
level 4+ → 120 min (capped at max_cooldown)
```

**Escalation (on restart):**
- `record_restart()` is called after restart completes
- `current_level += 1`
- `successes_since_last_level_change = 0`
- `last_restart_at = Instant::now()`
- Clear sliding window (fresh start)

**De-escalation (on successful downloads):**
- Each successful download: `successes_since_last_level_change += 1`
- When `successes_since_last_level_change >= successes_to_deescalate` (default: 10):
  - If `current_level > 0`: `current_level -= 1`
  - `successes_since_last_level_change = 0`

**Example scenario:**
```
1. System starts at level 0
2. Corruption detected (2 of 4 bad) → RestartNeeded returned
3. Caller triggers restart, calls record_restart() → level becomes 1, cooldown 20 min
4. After cooldown, processing resumes
5. 10 successful downloads → level drops to 0
6. Another corruption spike → restart, level becomes 1, cooldown 20 min
7. More corruption during processing → restart, level becomes 2, cooldown 40 min
8. 10 successes → level 1
9. 10 more successes → level 0
```

### Restart flow

1. `record_result(false)` called after ffprobe failure
2. Check sliding window: if failures >= threshold → return `RestartNeeded`
3. Caller (job_processor) sees `RestartNeeded`:
   - Acquire restart mutex (skip if another restart already in progress)
   - Call `downloader_client.restart().await` (fire and forget - no polling)
   - Call `record_restart()` to escalate level and start cooldown
   - Release mutex
4. During cooldown, `is_in_cooldown()` returns true → processor skips work
5. When cooldown expires, processing resumes. If downloader still broken, we'll detect corruption again and re-escalate

### DownloaderClient addition

```rust
// In downloader_client.rs
pub async fn restart(&self) -> Result<()> {
    let url = format!("{}/restart", self.base_url);
    let response = self.client.post(&url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("Restart request failed: {}", response.status()));
    }
    Ok(())
}
```

### Configuration additions

```rust
// In DownloadManagerSettings
pub corruption_window_size: usize,           // default: 4
pub corruption_failure_threshold: usize,     // default: 2
pub corruption_base_cooldown_secs: u64,      // default: 600 (10 min)
pub corruption_max_cooldown_secs: u64,       // default: 7200 (2 hours)
pub corruption_cooldown_multiplier: f64,     // default: 2.0
pub corruption_successes_to_deescalate: u32, // default: 10
```

### Files to create/modify

| File | Action |
|------|--------|
| `corruption_handler.rs` | **CREATE** - CorruptionHandler struct and logic |
| `mod.rs` | MODIFY - Export corruption_handler module |
| `models.rs` | MODIFY - Add config fields |
| `downloader_client.rs` | MODIFY - Add restart() method |
| `manager.rs` | MODIFY - Initialize watchdog, pass to processor |
| `job_processor.rs` | MODIFY - Record results, check cooldown, handle restart |

---

## Part 2.5: State Persistence

### Goal
Persist watchdog state across server restarts so we don't lose escalation context.

### Design: Key-value table in server.db

```sql
-- New table in server.db
CREATE TABLE IF NOT EXISTS server_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Watchdog state storage

Store as JSON under key `corruption_handler_state`:
```json
{
  "current_level": 2,
  "successes_since_last_level_change": 5,
  "last_restart_at_unix": 1702345678
}
```

### Integration
- On startup: Load state from server.db, restore `current_level` and `successes_since_last_level_change`
- `last_restart_at` converted from unix timestamp to Instant relative to current time
- On `record_restart()` and level changes: Persist updated state
- Throttle data (sliding window) is NOT persisted - it's transient and resets on restart

### Files to modify

| File | Action |
|------|--------|
| `server_store.rs` | MODIFY - Add `get_state`/`set_state` methods |
| `corruption_handler.rs` | MODIFY - Load/save state via ServerStore |
| `manager.rs` | MODIFY - Pass ServerStore to watchdog on init |

---

## Part 3: Integration in Job Processor

### Updated process_next() flow

```rust
async fn process_next(&self) -> Result<Option<ProcessingResult>> {
    // 1. Check corruption watchdog cooldown
    if self.corruption_handler.is_in_cooldown().await {
        return Ok(None);  // Skip, still cooling down
    }

    // 2. Check bandwidth throttle (historical usage, no estimation)
    if let Err(_wait) = self.throttler.check_bandwidth().await {
        return Ok(None);  // Skip, bandwidth limit reached
    }

    // 3. Check global capacity (existing)
    // 4. Get next pending item (existing)
    // 5. Claim for processing (existing)

    // 6. Execute download
    let result = self.execute_download(&item).await;

    // 7. Record actual bytes with throttler
    if let Ok(ref r) = result {
        self.throttler.record_download(r.bytes_downloaded).await;
    }

    // 8. Record with corruption watchdog (only ffprobe failures count as corruption)
    let success = result.as_ref().map(|r| !r.was_corrupted).unwrap_or(true);  // non-corruption failures don't count
    let action = self.corruption_handler.record_result(success).await;

    // 9. Handle restart if needed
    if matches!(action, WatchdogAction::RestartNeeded) {
        self.trigger_downloader_restart().await?;
    }

    // 10. Handle result (existing mark_completed/mark_failed logic)
    ...
}

async fn trigger_downloader_restart(&self) -> Result<()> {
    // Acquire restart mutex - skip if restart already in progress
    let mut restart_guard = match self.corruption_handler.try_lock_restart().await {
        Some(guard) => guard,
        None => {
            log::debug!("Restart already in progress, skipping");
            return Ok(());
        }
    };

    log::warn!("Corruption threshold exceeded, restarting downloader...");

    // Fire restart request (don't wait for confirmation)
    if let Err(e) = self.downloader_client.restart().await {
        log::error!("Failed to send restart request: {}", e);
    }

    // Record restart to escalate level and start cooldown
    self.corruption_handler.record_restart().await;

    Ok(())
}
```

---

## Part 4: Prometheus Metrics

### Throttle metrics

```rust
// Gauge: current bytes in the per-minute window
download_throttle_bytes_last_minute: Gauge

// Gauge: current bytes in the per-hour window
download_throttle_bytes_last_hour: Gauge

// Counter: number of times processing was skipped due to throttle
download_throttle_skip_count: Counter
```

### Corruption watchdog metrics

```rust
// Gauge: current cooldown level (0, 1, 2, ...)
download_corruption_cooldown_level: Gauge

// Gauge: successes since last level change
download_corruption_successes_since_change: Gauge

// Gauge: 1 if in cooldown, 0 otherwise
download_corruption_in_cooldown: Gauge

// Counter: total restarts triggered
download_corruption_restart_count: Counter

// Counter: total corrupted files detected
download_corruption_detected_count: Counter
```

### Files to modify

| File | Action |
|------|--------|
| `job_processor.rs` | MODIFY - Update metrics after throttle/watchdog operations |

---

## Part 5: Admin API Endpoints

### New endpoints

```
GET /v1/admin/download/throttle
```
Returns current throttle state:
```json
{
  "enabled": true,
  "max_mb_per_minute": 20,
  "max_mb_per_hour": 1500,
  "current_mb_last_minute": 12.5,
  "current_mb_last_hour": 450.2,
  "is_throttled": false
}
```

```
GET /v1/admin/download/corruption-watchdog
```
Returns current corruption watchdog state:
```json
{
  "current_level": 1,
  "successes_since_last_level_change": 7,
  "successes_to_deescalate": 10,
  "in_cooldown": false,
  "cooldown_remaining_secs": null,
  "current_cooldown_duration_secs": 1200,
  "recent_results": [true, true, false, true],
  "window_size": 4,
  "failure_threshold": 2
}
```

```
POST /v1/admin/download/corruption-watchdog/reset
```
Admin reset: immediately resets level to 0 AND clears any active cooldown. Returns new state.
This is for manual intervention when an admin knows the situation is resolved.

### Files to create/modify

| File | Action |
|------|--------|
| `catalog-server/src/server/admin_download.rs` | **CREATE** - New admin endpoints |
| `catalog-server/src/server/mod.rs` | MODIFY - Register new routes |

---

## Part 6: Admin Panel (Web Frontend)

### Location
Add to existing admin panel, likely under a "Downloads" or "System" section.

### Throttle widget
- Display current MB/min and MB/hour usage as progress bars
- Show limits (20 MB/min, 1500 MB/hour)
- Visual indicator when throttled

### Corruption watchdog widget
- Display current level (0-4+) with color coding (green/yellow/orange/red)
- Progress bar: successes toward de-escalation (e.g., "7/10 to level down")
- Cooldown indicator with countdown timer when active
- Recent results as small icons (✓/✗) showing sliding window
- "Reset" button (calls POST /reset endpoint)

### Files to create/modify

| File | Action |
|------|--------|
| `web/src/views/AdminDownloadsView.vue` | **CREATE** or extend existing admin view |
| `web/src/store/admin.js` | MODIFY - Add API calls for new endpoints |
| `web/src/router/index.js` | MODIFY - Add route if new view |

---

## Summary of All Changes

### New files (4)
1. `catalog-server/src/download_manager/throttle.rs` - Throttler trait + SlidingWindowThrottler
2. `catalog-server/src/download_manager/corruption_handler.rs` - CorruptionHandler
3. `catalog-server/src/server/admin_download.rs` - Admin API endpoints
4. `web/src/views/AdminDownloadsView.vue` - Admin panel UI (or integrate into existing)

### Modified files - Backend (7)
1. `catalog-server/src/download_manager/mod.rs` - exports
2. `catalog-server/src/download_manager/models.rs` - config fields
3. `catalog-server/src/download_manager/downloader_client.rs` - restart()
4. `catalog-server/src/download_manager/manager.rs` - init throttler & watchdog, expose state
5. `catalog-server/src/download_manager/job_processor.rs` - integration + metrics
6. `catalog-server/src/server/mod.rs` - register admin routes
7. `catalog-server/src/server_store.rs` - add get_state/set_state for key-value persistence

### Modified files - Frontend (2-3)
1. `web/src/store/admin.js` - API calls
2. `web/src/router/index.js` - routing (if needed)
3. Existing admin component or new view

### Tests to add
- Unit tests for SlidingWindowThrottler
- Unit tests for CorruptionHandler (escalation, de-escalation, threshold detection)
- Integration test for restart flow (mock downloader)

---

## Part 7: Retry Policy Changes

### Current values
```
max_retries: 5
initial_backoff_secs: 60
max_backoff_secs: 3600
backoff_multiplier: 2.0
```

Sequence: 1 min → 2 min → 4 min → 8 min → 16 min (5 attempts)

### New values
```
max_retries: 8
initial_backoff_secs: 60
max_backoff_secs: 3600
backoff_multiplier: 2.5
```

Sequence: 1 min → 2.5 min → 6.25 min → 15.6 min → 39 min → 60 min (capped) → 60 min → 60 min (8 attempts)

### Files to modify

| File | Action |
|------|--------|
| `retry_policy.rs` | MODIFY - Update defaults |
| `models.rs` | MODIFY - Update DownloadManagerSettings defaults |
