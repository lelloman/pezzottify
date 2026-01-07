//! Shared constants for end-to-end tests
//!
//! This module contains all constants used across the test suite.
//! When test data changes (user credentials, catalog IDs, etc.),
//! update only this file.

// ============================================================================
// Test User Credentials
// ============================================================================

/// Regular test user handle
pub const TEST_USER: &str = "testuser";

/// Regular test user password
pub const TEST_PASS: &str = "testpass123";

/// Admin test user handle
pub const ADMIN_USER: &str = "admin";

/// Admin test user password
pub const ADMIN_PASS: &str = "adminpass123";

// ============================================================================
// Test Catalog IDs
// ============================================================================

/// Artist ID for "The Test Band"
pub const ARTIST_1_ID: &str = "test_artist_001";

/// Artist ID for "Jazz Ensemble"
pub const ARTIST_2_ID: &str = "test_artist_002";

/// Album ID for "First Album" by The Test Band
pub const ALBUM_1_ID: &str = "test_album_001";

/// Album ID for "Jazz Collection" by Jazz Ensemble
pub const ALBUM_2_ID: &str = "test_album_002";

/// Track ID for "Opening Track" on First Album
pub const TRACK_1_ID: &str = "test_track_001";

/// Track ID for "Middle Track" on First Album
pub const TRACK_2_ID: &str = "test_track_002";

/// Track ID for "Closing Track" on First Album
pub const TRACK_3_ID: &str = "test_track_003";

/// Track ID for "Smooth Jazz" on Jazz Collection
pub const TRACK_4_ID: &str = "test_track_004";

/// Track ID for "Upbeat Jazz" on Jazz Collection
pub const TRACK_5_ID: &str = "test_track_005";

// ============================================================================
// Test Catalog Metadata
// ============================================================================

/// Artist 1 name
pub const ARTIST_1_NAME: &str = "The Test Band";

/// Artist 2 name
pub const ARTIST_2_NAME: &str = "Jazz Ensemble";

/// Album 1 title
pub const ALBUM_1_TITLE: &str = "First Album";

/// Album 2 title
pub const ALBUM_2_TITLE: &str = "Jazz Collection";

/// Track 1 title
pub const TRACK_1_TITLE: &str = "Opening Track";

/// Track 2 title
pub const TRACK_2_TITLE: &str = "Middle Track";

/// Track 3 title
pub const TRACK_3_TITLE: &str = "Closing Track";

/// Track 4 title
pub const TRACK_4_TITLE: &str = "Smooth Jazz";

/// Track 5 title
pub const TRACK_5_TITLE: &str = "Upbeat Jazz";

// ============================================================================
// Test Timeouts and Configuration
// ============================================================================

/// Maximum time to wait for server to become ready (milliseconds)
pub const SERVER_READY_TIMEOUT_MS: u64 = 5000;

/// Timeout for individual HTTP requests (seconds)
pub const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Polling interval when waiting for server ready (milliseconds)
pub const SERVER_READY_POLL_INTERVAL_MS: u64 = 50;

// ============================================================================
// Test File Sizes (approximate, for validation)
// ============================================================================

/// Expected size of test audio file (bytes)
pub const TEST_AUDIO_SIZE_BYTES: usize = 8400; // ~8.2 KB

/// Expected size of test image file (bytes)
pub const TEST_IMAGE_SIZE_BYTES: usize = 224;
