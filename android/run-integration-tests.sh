#!/bin/bash

# Script to run integration tests for the Android remoteapi module
# This script sets up a test catalog-server instance with Docker and runs integration tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CATALOG_SERVER_DIR="$PROJECT_ROOT/catalog-server"

# Test data directories - use timestamp to avoid Docker caching issues
TEST_DATA_DIR="$HOME/.pezzottify-integration-test-$(date +%s)"
TEST_DB_DIR="$TEST_DATA_DIR/db"
TEST_MEDIA_DIR="$TEST_DATA_DIR/media"
TEST_CATALOG_DB="$TEST_DB_DIR/catalog.db"
TEST_USER_DB="$TEST_DB_DIR/user.db"

# Docker configuration
DOCKER_IMAGE="pezzottify-catalog-server"
CONTAINER_NAME="pezzottify-integration-test"

# Mock downloader configuration
MOCK_DOWNLOADER_PORT=8090
MOCK_DOWNLOADER_PID=""

# Test data IDs (artist IDs use R prefix as per test expectations)
ARTIST_ID="R5a2EaR3hamoenG9rDuVn8j"
ALBUM_ID="1999"
TRACK_ID="track1999"
IMAGE_ID="ab6761610000e5eb4fcd6f21e60024ae48c3d244"

echo "🧪 Setting up Pezzottify integration test environment..."

# Clean up any existing test data from previous runs
echo "🧹 Cleaning up old test data..."
rm -rf "$HOME"/.pezzottify-integration-test-* 2>/dev/null || true

# Create new test directories
mkdir -p "$TEST_DB_DIR"
mkdir -p "$TEST_MEDIA_DIR/images"
mkdir -p "$TEST_MEDIA_DIR/albums/$ALBUM_ID"

# Create a minimal test image (1x1 JPEG)
echo "📁 Creating test media files..."
printf '\xff\xd8\xff\xe0\x00\x10\x4a\x46\x49\x46\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xff\xdb\x00\x43\x00\x08\x06\x06\x07\x06\x05\x08\x07\x07\x07\x09\x09\x08\x0a\x0c\x14\x0d\x0c\x0b\x0b\x0c\x19\x12\x13\x0f\x14\x1d\x1a\x1f\x1e\x1d\x1a\x1c\x1c\x20\x24\x2e\x27\x20\x22\x2c\x23\x1c\x1c\x28\x37\x29\x2c\x30\x31\x34\x34\x34\x1f\x27\x39\x3d\x38\x32\x3c\x2e\x33\x34\x32\xff\xc0\x00\x0b\x08\x00\x01\x00\x01\x01\x01\x11\x00\xff\xc4\x00\x14\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03\xff\xc4\x00\x14\x10\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\xff\xda\x00\x08\x01\x01\x00\x00\x3f\x00\x07\xff\xd9' > "$TEST_MEDIA_DIR/images/$IMAGE_ID"

# Create a minimal audio file (just needs to exist)
echo "dummy audio content" > "$TEST_MEDIA_DIR/albums/$ALBUM_ID/track_$TRACK_ID.flac"

# Create SQLite catalog database with test data
echo "💾 Creating SQLite catalog database..."
sqlite3 "$TEST_CATALOG_DB" << 'EOSQL'
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Create schema (matching catalog-server schema v0)
-- Uses rowid-based schema with Spotify-style IDs

CREATE TABLE artists (
    rowid INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    followers_total INTEGER NOT NULL,
    popularity INTEGER NOT NULL
);

CREATE TABLE albums (
    rowid INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    album_type TEXT NOT NULL,
    external_id_upc TEXT,
    external_id_amgid TEXT,
    label TEXT NOT NULL,
    popularity INTEGER NOT NULL,
    release_date TEXT NOT NULL,
    release_date_precision TEXT NOT NULL
);

CREATE TABLE tracks (
    rowid INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    album_rowid INTEGER NOT NULL,
    track_number INTEGER NOT NULL,
    external_id_isrc TEXT,
    popularity INTEGER NOT NULL,
    disc_number INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    explicit INTEGER NOT NULL,
    language TEXT
);

CREATE TABLE track_artists (
    track_rowid INTEGER NOT NULL,
    artist_rowid INTEGER NOT NULL,
    role INTEGER
);

CREATE TABLE artist_albums (
    artist_rowid INTEGER NOT NULL,
    album_rowid INTEGER NOT NULL,
    is_appears_on INTEGER NOT NULL,
    is_implicit_appears_on INTEGER NOT NULL,
    index_in_album INTEGER
);

CREATE TABLE artist_genres (
    artist_rowid INTEGER NOT NULL,
    genre TEXT NOT NULL
);

CREATE TABLE album_images (
    album_rowid INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    url TEXT NOT NULL
);

CREATE TABLE artist_images (
    artist_rowid INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    url TEXT NOT NULL
);

-- Create indices
CREATE INDEX idx_artists_id ON artists(id);
CREATE INDEX idx_albums_id ON albums(id);
CREATE INDEX idx_tracks_id ON tracks(id);
CREATE INDEX idx_tracks_album ON tracks(album_rowid);
CREATE INDEX idx_tracks_isrc ON tracks(external_id_isrc);
CREATE INDEX idx_track_artists_track ON track_artists(track_rowid);
CREATE INDEX idx_track_artists_artist ON track_artists(artist_rowid);
CREATE INDEX idx_artist_albums_artist ON artist_albums(artist_rowid);
CREATE INDEX idx_artist_albums_album ON artist_albums(album_rowid);
CREATE INDEX idx_artist_genres_artist ON artist_genres(artist_rowid);
CREATE INDEX idx_album_images_album ON album_images(album_rowid);
CREATE INDEX idx_artist_images_artist ON artist_images(artist_rowid);

-- Schema version: BASE_DB_VERSION (99999) + schema_version (0) = 99999
PRAGMA user_version = 99999;
EOSQL

# Insert test data using the IDs defined above
# Note: rowid values are assigned explicitly for predictable foreign keys
sqlite3 "$TEST_CATALOG_DB" << EOSQL
-- Insert test artist (Prince) - rowid 1
INSERT INTO artists (rowid, id, name, followers_total, popularity)
VALUES (1, '$ARTIST_ID', 'Prince', 5000000, 85);

-- Insert test album (1999) - rowid 1
INSERT INTO albums (rowid, id, name, album_type, label, popularity, release_date, release_date_precision)
VALUES (1, '$ALBUM_ID', '1999', 'album', 'Warner Bros.', 80, '1982-10-27', 'day');

-- Insert second test album (Purple Rain) - rowid 2
INSERT INTO albums (rowid, id, name, album_type, label, popularity, release_date, release_date_precision)
VALUES (2, 'purple-rain-album-id', 'Purple Rain', 'album', 'Warner Bros.', 90, '1984-06-25', 'day');

-- Insert third test album (Sign o the Times) - rowid 3
INSERT INTO albums (rowid, id, name, album_type, label, popularity, release_date, release_date_precision)
VALUES (3, 'sign-o-the-times-id', 'Sign o the Times', 'album', 'Paisley Park', 85, '1987-03-30', 'day');

-- Insert test track - rowid 1
INSERT INTO tracks (rowid, id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
VALUES (1, '$TRACK_ID', '1999', 1, 1, 75, 1, 378000, 0);

-- Insert second test track - rowid 2
INSERT INTO tracks (rowid, id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
VALUES (2, 'purple-rain-track-id', 'Purple Rain', 2, 1, 95, 1, 520000, 0);

-- Insert third test track - rowid 3
INSERT INTO tracks (rowid, id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
VALUES (3, 'sign-o-times-track-id', 'Sign o the Times', 3, 1, 80, 1, 290000, 0);

-- Insert relationships using rowid references
INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album) VALUES (1, 1, 0, 0, 0);
INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album) VALUES (1, 2, 0, 0, 0);
INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album) VALUES (1, 3, 0, 0, 0);
INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (1, 1, 0);
INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (2, 1, 0);
INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (3, 1, 0);
INSERT INTO artist_genres (artist_rowid, genre) VALUES (1, 'Funk');
INSERT INTO artist_genres (artist_rowid, genre) VALUES (1, 'Pop');
INSERT INTO artist_genres (artist_rowid, genre) VALUES (1, 'Rock');
INSERT INTO artist_images (artist_rowid, width, height, url) VALUES (1, 300, 300, 'https://example.com/prince.jpg');
INSERT INTO album_images (album_rowid, width, height, url) VALUES (1, 300, 300, 'https://example.com/1999.jpg');
INSERT INTO album_images (album_rowid, width, height, url) VALUES (2, 300, 300, 'https://example.com/purple-rain.jpg');
INSERT INTO album_images (album_rowid, width, height, url) VALUES (3, 300, 300, 'https://example.com/sign-o-times.jpg');
EOSQL

echo "✅ Catalog database created with test data"

# Build Docker image - always rebuild to ensure we have the latest code
# This is important because schema changes will cause "database version too new" errors
echo "🐳 Building Docker image..."
cd "$PROJECT_ROOT"

# Get the latest commit hash of catalog-server
LATEST_COMMIT=$(git log -1 --format=%H -- catalog-server/)

# Check if image exists and has the correct commit label
EXISTING_COMMIT=$(docker image inspect "$DOCKER_IMAGE" --format '{{index .Config.Labels "catalog-server.commit"}}' 2>/dev/null || echo "")

if [ "$EXISTING_COMMIT" = "$LATEST_COMMIT" ]; then
    echo "Docker image $DOCKER_IMAGE is up to date (commit: ${LATEST_COMMIT:0:8})"
else
    if [ -n "$EXISTING_COMMIT" ]; then
        echo "Docker image is outdated (has: ${EXISTING_COMMIT:0:8}, need: ${LATEST_COMMIT:0:8})"
        echo "Removing old image..."
        docker rm -f $(docker ps -aq --filter "ancestor=$DOCKER_IMAGE") 2>/dev/null || true
        docker rmi "$DOCKER_IMAGE" 2>/dev/null || true
    else
        echo "Building $DOCKER_IMAGE..."
    fi
    docker build -t "$DOCKER_IMAGE" --label "catalog-server.commit=$LATEST_COMMIT" -f catalog-server/Dockerfile .
fi

# Create test user database
echo "💾 Creating test user database..."
cd "$CATALOG_SERVER_DIR"
cargo build --release --bin cli-auth 2>&1 | grep -E "(Compiling|Finished|error)" || true

# Use cli-auth to create the test user with both Admin and Regular roles
# Admin: RequestContent, DownloadManagerAdmin, etc.
# Regular: LikeContent, OwnPlaylists
echo "✅ Creating test user 'android-test' with password 'asdasd'..."
{
    echo "add-user android-test"
    echo "add-login android-test asdasd"
    echo "add-role android-test Admin"
    echo "add-role android-test Regular"
    echo "exit"
} | ./target/release/cli-auth "$TEST_USER_DB" > /dev/null

echo "✅ User database created with user 'android-test'"

# Stop and remove any existing container
echo "🛑 Stopping any existing test container..."
docker stop "$CONTAINER_NAME" 2>/dev/null || true
docker rm "$CONTAINER_NAME" 2>/dev/null || true

# Kill any existing mock downloader
pkill -f "mock_downloader.py" 2>/dev/null || true

# Start mock downloader service
echo "🔧 Starting mock downloader service on port $MOCK_DOWNLOADER_PORT..."
python3 "$SCRIPT_DIR/mock_downloader.py" "$MOCK_DOWNLOADER_PORT" &
MOCK_DOWNLOADER_PID=$!
sleep 1

# Verify mock downloader is running
if ! curl -s -f "http://localhost:$MOCK_DOWNLOADER_PORT/health" &> /dev/null; then
    echo "❌ Mock downloader failed to start"
    kill $MOCK_DOWNLOADER_PID 2>/dev/null || true
    rm -rf "$TEST_DATA_DIR"
    exit 1
fi
echo "✅ Mock downloader is ready!"

# Start catalog-server container with downloader URL
# The container runs on the host network to access the mock downloader
echo "🚀 Starting catalog-server container..."
docker run -d \
    --name "$CONTAINER_NAME" \
    --network host \
    -v "$TEST_DB_DIR:/data/db" \
    -v "$TEST_MEDIA_DIR:/data/media" \
    -e RUST_LOG=info \
    "$DOCKER_IMAGE" \
    catalog-server \
    --db-dir /data/db \
    --media-path /data/media \
    --port 3002 \
    --content-cache-age-sec=60 \
    --logging-level path \
    --downloader-url "http://localhost:$MOCK_DOWNLOADER_PORT"

# Wait for server to be ready
echo "⏳ Waiting for catalog-server to be ready..."
for i in {1..30}; do
    if curl -s -f http://localhost:3002/ &> /dev/null; then
        echo "✅ Server is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Server failed to start in time"
        echo "Container logs:"
        docker logs "$CONTAINER_NAME"
        docker stop "$CONTAINER_NAME" 2>/dev/null || true
        docker rm "$CONTAINER_NAME" 2>/dev/null || true
        if [ -n "$MOCK_DOWNLOADER_PID" ]; then
            kill $MOCK_DOWNLOADER_PID 2>/dev/null || true
        fi
        pkill -f "mock_downloader.py" 2>/dev/null || true
        rm -rf "$TEST_DATA_DIR"
        exit 1
    fi
    sleep 1
done

# Temporarily move integration test to test source set
echo "🧪 Preparing to run integration tests..."
INTEGRATION_TEST_SRC="$SCRIPT_DIR/remoteapi/src/integrationTest/java/com/lelloman/pezzottify/android/remoteapi/internal/RemoteApiClientImplTest.kt"
TEST_DEST_DIR="$SCRIPT_DIR/remoteapi/src/test/java/com/lelloman/pezzottify/android/remoteapi/internal"
TEST_DEST="$TEST_DEST_DIR/RemoteApiClientImplTest.kt"

# Move test to regular test directory
mkdir -p "$TEST_DEST_DIR"
cp "$INTEGRATION_TEST_SRC" "$TEST_DEST"

# Run tests
echo "🧪 Running integration tests..."
cd "$SCRIPT_DIR"
./gradlew :remoteapi:testDebugUnitTest --tests "RemoteApiClientImplTest" -q
TEST_EXIT_CODE=$?

# Remove the test file from test directory
rm -f "$TEST_DEST"

# Cleanup
echo "🧹 Cleaning up..."
docker stop "$CONTAINER_NAME" > /dev/null 2>&1 || true
docker rm "$CONTAINER_NAME" > /dev/null 2>&1 || true
if [ -n "$MOCK_DOWNLOADER_PID" ]; then
    kill $MOCK_DOWNLOADER_PID 2>/dev/null || true
fi
pkill -f "mock_downloader.py" 2>/dev/null || true
# Some files may be created by Docker with different ownership, use sudo if available
rm -rf "$TEST_DATA_DIR" 2>/dev/null || sudo rm -rf "$TEST_DATA_DIR" 2>/dev/null || true

echo ""
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "✅ Integration tests passed!"
    exit 0
else
    echo "❌ Integration tests failed with exit code $TEST_EXIT_CODE"
    exit $TEST_EXIT_CODE
fi
