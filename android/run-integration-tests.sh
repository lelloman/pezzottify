#!/bin/bash

# Script to run integration tests for the Android remoteapi module
# This script sets up a test catalog-server instance with Docker and runs integration tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CATALOG_SERVER_DIR="$PROJECT_ROOT/catalog-server"

# Test data directories - use timestamp to avoid Docker caching issues
# Use a directory outside /tmp to avoid potential Docker mount issues
TEST_DATA_DIR="$HOME/.pezzottify-integration-test-$(date +%s)"
TEST_CATALOG_DIR="$TEST_DATA_DIR/catalog"
TEST_DB_DIR="$TEST_DATA_DIR/db"
TEST_DB_PATH="$TEST_DB_DIR/user.db"

# Docker configuration
DOCKER_IMAGE="pezzottify-catalog-server"
CONTAINER_NAME="pezzottify-integration-test"

echo "🧪 Setting up Pezzottify integration test environment..."

# Clean up any existing test data from previous runs
echo "🧹 Cleaning up old test data..."
rm -rf "$HOME"/.pezzottify-integration-test-* 2>/dev/null || true

# Create new test directories
mkdir -p "$TEST_CATALOG_DIR"/{albums,artists,images}
mkdir -p "$TEST_DB_DIR"

# Create minimal test catalog data
echo "📁 Creating test catalog data..."

# Create Prince artist (note: file must be named artist_<id>.json, ID without R prefix)
cat > "$TEST_CATALOG_DIR/artists/artist_5a2EaR3hamoenG9rDuVn8j.json" << 'EOF'
{
  "id": "5a2EaR3hamoenG9rDuVn8j",
  "name": "Prince",
  "genre": ["Funk"],
  "imageId": "ab6761610000e5eb4fcd6f21e60024ae48c3d244",
  "related": [],
  "portraits": [],
  "activity_periods": [],
  "portrait_group": []
}
EOF

# Create a test album (albums are directories with album_<id>/album_<id>.json structure)
mkdir -p "$TEST_CATALOG_DIR/albums/album_1999"
cat > "$TEST_CATALOG_DIR/albums/album_1999/album_1999.json" << 'EOF'
{
  "id": "1999",
  "name": "1999",
  "imageId": "ab6761610000e5eb4fcd6f21e60024ae48c3d244",
  "discs": [
    {
      "name": "",
      "number": 1,
      "tracks": ["1999"]
    }
  ],
  "album_type": "ALBUM",
  "artists_ids": ["5a2EaR3hamoenG9rDuVn8j"],
  "label": "",
  "date": 0,
  "genres": [],
  "covers": [],
  "related": [],
  "cover_group": [],
  "original_title": "",
  "version_title": "",
  "type_str": ""
}
EOF

# Create track metadata JSON (must match artist ID: 5a2EaR3hamoenG9rDuVn8j)
cat > "$TEST_CATALOG_DIR/albums/album_1999/track_1999.json" << 'EOF'
{
  "id": "1999",
  "name": "1999",
  "album_id": "1999",
  "artists_ids": ["5a2EaR3hamoenG9rDuVn8j"],
  "number": 1,
  "disc_number": 1,
  "duration": 378,
  "is_explicit": false,
  "files": {},
  "alternatives": [],
  "tags": [],
  "earliest_live_timestamp": 0,
  "has_lyrics": false,
  "language_of_performance": [],
  "original_title": "1999",
  "version_title": "",
  "artists_with_role": []
}
EOF

# Create a minimal audio file (just needs to exist, any non-.json extension works)
echo "dummy audio" > "$TEST_CATALOG_DIR/albums/album_1999/track_1999.flac"

# Create a minimal test image (1x1 JPEG)
printf '\xff\xd8\xff\xe0\x00\x10\x4a\x46\x49\x46\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xff\xdb\x00\x43\x00\x08\x06\x06\x07\x06\x05\x08\x07\x07\x07\x09\x09\x08\x0a\x0c\x14\x0d\x0c\x0b\x0b\x0c\x19\x12\x13\x0f\x14\x1d\x1a\x1f\x1e\x1d\x1a\x1c\x1c\x20\x24\x2e\x27\x20\x22\x2c\x23\x1c\x1c\x28\x37\x29\x2c\x30\x31\x34\x34\x34\x1f\x27\x39\x3d\x38\x32\x3c\x2e\x33\x34\x32\xff\xc0\x00\x0b\x08\x00\x01\x00\x01\x01\x01\x11\x00\xff\xc4\x00\x14\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03\xff\xc4\x00\x14\x10\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\xff\xda\x00\x08\x01\x01\x00\x00\x3f\x00\x07\xff\xd9' > "$TEST_CATALOG_DIR/images/ab6761610000e5eb4fcd6f21e60024ae48c3d244"


# Build Docker image if it doesn't exist
echo "🐳 Building Docker image..."
cd "$CATALOG_SERVER_DIR"
if ! docker image inspect "$DOCKER_IMAGE" &> /dev/null; then
    echo "Building $DOCKER_IMAGE..."
    docker build -t "$DOCKER_IMAGE" .
else
    echo "Docker image $DOCKER_IMAGE already exists, skipping build..."
fi

# Create test database with user
echo "💾 Creating test database..."
cd "$CATALOG_SERVER_DIR"
cargo build --release --bin cli-auth 2>&1 | grep -E "(Compiling|Finished|error)" || true

# Use cli-auth to create the test user (interactive mode)
echo "✅ Creating test user 'android-test' with password 'asdasd'..."
{
    echo "add-user android-test"
    echo "add-login android-test asdasd"
    echo "add-role android-test Regular"
    echo "exit"
} | ./target/release/cli-auth "$TEST_DB_PATH" > /dev/null

echo "✅ Test database created with user 'android-test'"

# Stop and remove any existing container
echo "🛑 Stopping any existing test container..."
docker stop "$CONTAINER_NAME" 2>/dev/null || true
docker rm "$CONTAINER_NAME" 2>/dev/null || true

# Start catalog-server container
echo "🚀 Starting catalog-server container..."
docker run -d \
    --name "$CONTAINER_NAME" \
    -p 3002:3001 \
    -v "$TEST_CATALOG_DIR:/data/catalog" \
    -v "$TEST_DB_DIR:/data/db" \
    "$DOCKER_IMAGE"

# Wait for server to be ready
echo "⏳ Waiting for catalog-server to be ready..."
for i in {1..30}; do
    if curl -s -f --head http://localhost:3002/ &> /dev/null; then
        echo "✅ Server is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Server failed to start in time"
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
