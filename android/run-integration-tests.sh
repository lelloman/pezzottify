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
CREATE TABLE artists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    genres TEXT,
    activity_periods TEXT
);

CREATE TABLE albums (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    album_type TEXT NOT NULL,
    label TEXT,
    release_date INTEGER,
    genres TEXT,
    original_title TEXT,
    version_title TEXT
);

CREATE TABLE images (
    id TEXT PRIMARY KEY,
    uri TEXT NOT NULL,
    size TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL
);

CREATE TABLE tracks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    disc_number INTEGER NOT NULL DEFAULT 1,
    track_number INTEGER NOT NULL,
    duration_secs INTEGER,
    is_explicit INTEGER NOT NULL DEFAULT 0,
    audio_uri TEXT NOT NULL,
    format TEXT NOT NULL,
    tags TEXT,
    has_lyrics INTEGER NOT NULL DEFAULT 0,
    languages TEXT,
    original_title TEXT,
    version_title TEXT
);

CREATE TABLE album_artists (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    UNIQUE(album_id, artist_id)
);

CREATE TABLE track_artists (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(track_id, artist_id, role)
);

CREATE TABLE related_artists (
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    related_artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    UNIQUE(artist_id, related_artist_id)
);

CREATE TABLE artist_images (
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(artist_id, image_id, image_type)
);

CREATE TABLE album_images (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(album_id, image_id, image_type)
);

-- Create indices
CREATE INDEX idx_tracks_album ON tracks(album_id);
CREATE INDEX idx_tracks_disc_track ON tracks(album_id, disc_number, track_number);
CREATE INDEX idx_album_artists_artist ON album_artists(artist_id);
CREATE INDEX idx_track_artists_artist ON track_artists(artist_id);
CREATE INDEX idx_artist_images_artist ON artist_images(artist_id);
CREATE INDEX idx_album_images_album ON album_images(album_id);

-- Schema version: BASE_DB_VERSION (99999) + schema_version (0) = 99999
PRAGMA user_version = 99999;
EOSQL

# Insert test data using the IDs defined above
sqlite3 "$TEST_CATALOG_DB" << EOSQL
-- Insert test artist (Prince)
INSERT INTO artists (id, name, genres, activity_periods)
VALUES ('$ARTIST_ID', 'Prince', '["Funk", "Pop", "Rock"]', '[{"Timespan":{"start_year":1976,"end_year":2016}}]');

-- Insert test album (1999)
INSERT INTO albums (id, name, album_type, label, release_date, genres)
VALUES ('$ALBUM_ID', '1999', 'ALBUM', 'Warner Bros.', 404524800, '["Funk", "Pop"]');

-- Insert test image
INSERT INTO images (id, uri, size, width, height)
VALUES ('$IMAGE_ID', 'images/$IMAGE_ID', 'DEFAULT', 300, 300);

-- Insert test track
INSERT INTO tracks (id, name, album_id, disc_number, track_number, duration_secs, is_explicit, audio_uri, format, tags, has_lyrics, languages)
VALUES ('$TRACK_ID', '1999', '$ALBUM_ID', 1, 1, 378, 0, 'albums/$ALBUM_ID/track_$TRACK_ID.flac', 'FLAC', '[]', 0, '["en"]');

-- Insert relationships
INSERT INTO album_artists (album_id, artist_id, position) VALUES ('$ALBUM_ID', '$ARTIST_ID', 0);
INSERT INTO track_artists (track_id, artist_id, role, position) VALUES ('$TRACK_ID', '$ARTIST_ID', 'MAIN_ARTIST', 0);
INSERT INTO artist_images (artist_id, image_id, image_type, position) VALUES ('$ARTIST_ID', '$IMAGE_ID', 'portrait', 0);
INSERT INTO album_images (album_id, image_id, image_type, position) VALUES ('$ALBUM_ID', '$IMAGE_ID', 'cover', 0);
EOSQL

echo "✅ Catalog database created with test data"

# Build Docker image if it doesn't exist or is outdated
echo "🐳 Building Docker image..."
cd "$PROJECT_ROOT"
if ! docker image inspect "$DOCKER_IMAGE" &> /dev/null; then
    echo "Building $DOCKER_IMAGE..."
    docker build -t "$DOCKER_IMAGE" -f catalog-server/Dockerfile .
else
    echo "Docker image $DOCKER_IMAGE already exists, skipping build..."
    echo "   (Run 'docker rmi $DOCKER_IMAGE' to force rebuild)"
fi

# Create test user database
echo "💾 Creating test user database..."
cd "$CATALOG_SERVER_DIR"
cargo build --release --bin cli-auth 2>&1 | grep -E "(Compiling|Finished|error)" || true

# Use cli-auth to create the test user
echo "✅ Creating test user 'android-test' with password 'asdasd'..."
{
    echo "add-user android-test"
    echo "add-login android-test asdasd"
    echo "add-role android-test Regular"
    echo "exit"
} | ./target/release/cli-auth "$TEST_USER_DB" > /dev/null

echo "✅ User database created with user 'android-test'"

# Stop and remove any existing container
echo "🛑 Stopping any existing test container..."
docker stop "$CONTAINER_NAME" 2>/dev/null || true
docker rm "$CONTAINER_NAME" 2>/dev/null || true

# Start catalog-server container
# The server expects: catalog-server <catalog-db> <user-db> --media-path <path>
echo "🚀 Starting catalog-server container..."
docker run -d \
    --name "$CONTAINER_NAME" \
    -p 3002:3001 \
    -v "$TEST_DB_DIR:/data/db" \
    -v "$TEST_MEDIA_DIR:/data/media" \
    "$DOCKER_IMAGE"

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
        docker stop "$CONTAINER_NAME"
        docker rm "$CONTAINER_NAME"
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
docker stop "$CONTAINER_NAME" > /dev/null
docker rm "$CONTAINER_NAME" > /dev/null
rm -rf "$TEST_DATA_DIR"

echo ""
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "✅ Integration tests passed!"
    exit 0
else
    echo "❌ Integration tests failed with exit code $TEST_EXIT_CODE"
    exit $TEST_EXIT_CODE
fi
