#!/bin/bash
set -euo pipefail

DB_DIR="${DB_DIR:-/data/db}"
MEDIA_DIR="${MEDIA_DIR:-/data/media}"
SEED_DIR="/seed"

echo "=== E2E Seed Entrypoint ==="
echo "DB_DIR=$DB_DIR"
echo "MEDIA_DIR=$MEDIA_DIR"

# Create directories
mkdir -p "$DB_DIR"
mkdir -p "$MEDIA_DIR/audio"
mkdir -p "$MEDIA_DIR/images"

# Create catalog database using Python seed script
echo "--- Creating catalog.db ---"
python3 "$SEED_DIR/seed_catalog.py" "$DB_DIR/catalog.db"

# Copy test media files
echo "--- Copying test media files ---"
# Audio files use track IDs as filenames (.ogg extension, content is the test mp3)
for track_id in test_track_001 test_track_002 test_track_003 test_track_004 test_track_005; do
    cp "$SEED_DIR/test_media/test-audio.mp3" "$MEDIA_DIR/audio/${track_id}.ogg"
done

# Image files use album IDs as filenames
for album_id in test_album_001 test_album_002; do
    cp "$SEED_DIR/test_media/test-image.jpg" "$MEDIA_DIR/images/${album_id}.jpg"
done

echo "--- Creating users via cli-auth ---"
# Create testuser (Regular role) and admin (Admin + Regular roles)
{
    echo "add-user testuser"
    echo "add-login testuser testpass123"
    echo "add-role testuser Regular"
    echo "add-user admin"
    echo "add-login admin adminpass123"
    echo "add-role admin Admin"
    echo "add-role admin Regular"
    echo "exit"
} | cli-auth --db-dir "$DB_DIR"

echo "--- Linking OIDC subjects ---"
{
    echo "link-oidc testuser testuser"
    echo "link-oidc admin admin"
    echo "exit"
} | cli-auth --db-dir "$DB_DIR"

echo "--- Verifying seed data ---"
echo "Files in DB_DIR:"
ls -la "$DB_DIR/"
echo "Files in MEDIA_DIR/audio:"
ls -la "$MEDIA_DIR/audio/"
echo "Files in MEDIA_DIR/images:"
ls -la "$MEDIA_DIR/images/"

echo "=== Seed complete ==="
