#!/bin/bash
# Script to recalculate and fix album_availability based on track audio presence
#
# Usage: ./fix-album-availability.sh /path/to/catalog.db [--dry-run]
#
# The script calculates availability for each album:
# - complete: all tracks have audio_uri set
# - partial: some tracks have audio_uri, some don't
# - missing: no tracks have audio_uri (or album has no tracks)

set -e

DB_PATH="$1"
DRY_RUN="$2"

if [ -z "$DB_PATH" ]; then
    echo "Usage: $0 /path/to/catalog.db [--dry-run]"
    exit 1
fi

if [ ! -f "$DB_PATH" ]; then
    echo "Error: Database file not found: $DB_PATH"
    exit 1
fi

echo "Database: $DB_PATH"
echo ""

# First, show current state
echo "=== Current album availability distribution ==="
sqlite3 "$DB_PATH" "SELECT album_availability, COUNT(*) as count FROM albums GROUP BY album_availability ORDER BY album_availability;"
echo ""

# Calculate what the availability SHOULD be
echo "=== Calculating correct availability based on tracks ==="

# Create a temp table with calculated availability
CALC_QUERY="
WITH album_stats AS (
    SELECT
        a.rowid as album_rowid,
        a.id as album_id,
        a.name as album_name,
        a.album_availability as current_availability,
        COUNT(t.rowid) as total_tracks,
        SUM(CASE WHEN t.audio_uri IS NOT NULL AND t.audio_uri != '' THEN 1 ELSE 0 END) as tracks_with_audio
    FROM albums a
    LEFT JOIN tracks t ON t.album_rowid = a.rowid
    GROUP BY a.rowid
),
calculated AS (
    SELECT
        album_rowid,
        album_id,
        album_name,
        current_availability,
        total_tracks,
        tracks_with_audio,
        CASE
            WHEN total_tracks = 0 THEN 'missing'
            WHEN tracks_with_audio = total_tracks THEN 'complete'
            WHEN tracks_with_audio > 0 THEN 'partial'
            ELSE 'missing'
        END as correct_availability
    FROM album_stats
)
SELECT * FROM calculated WHERE current_availability != correct_availability;
"

echo "Albums with incorrect availability:"
sqlite3 -header -column "$DB_PATH" "$CALC_QUERY" | head -50

INCORRECT_COUNT=$(sqlite3 "$DB_PATH" "
WITH album_stats AS (
    SELECT
        a.rowid as album_rowid,
        a.album_availability as current_availability,
        COUNT(t.rowid) as total_tracks,
        SUM(CASE WHEN t.audio_uri IS NOT NULL AND t.audio_uri != '' THEN 1 ELSE 0 END) as tracks_with_audio
    FROM albums a
    LEFT JOIN tracks t ON t.album_rowid = a.rowid
    GROUP BY a.rowid
),
calculated AS (
    SELECT
        album_rowid,
        current_availability,
        CASE
            WHEN total_tracks = 0 THEN 'missing'
            WHEN tracks_with_audio = total_tracks THEN 'complete'
            WHEN tracks_with_audio > 0 THEN 'partial'
            ELSE 'missing'
        END as correct_availability
    FROM album_stats
)
SELECT COUNT(*) FROM calculated WHERE current_availability != correct_availability;
")

echo ""
echo "Total albums needing update: $INCORRECT_COUNT"
echo ""

if [ "$DRY_RUN" == "--dry-run" ]; then
    echo "Dry run mode - no changes made."
    echo ""
    echo "=== What the distribution would look like after fix ==="
    sqlite3 "$DB_PATH" "
    WITH album_stats AS (
        SELECT
            a.rowid as album_rowid,
            COUNT(t.rowid) as total_tracks,
            SUM(CASE WHEN t.audio_uri IS NOT NULL AND t.audio_uri != '' THEN 1 ELSE 0 END) as tracks_with_audio
        FROM albums a
        LEFT JOIN tracks t ON t.album_rowid = a.rowid
        GROUP BY a.rowid
    ),
    calculated AS (
        SELECT
            CASE
                WHEN total_tracks = 0 THEN 'missing'
                WHEN tracks_with_audio = total_tracks THEN 'complete'
                WHEN tracks_with_audio > 0 THEN 'partial'
                ELSE 'missing'
            END as availability
        FROM album_stats
    )
    SELECT availability, COUNT(*) as count FROM calculated GROUP BY availability ORDER BY availability;
    "
    exit 0
fi

echo "Updating album availability..."

sqlite3 "$DB_PATH" "
UPDATE albums
SET album_availability = (
    SELECT CASE
        WHEN COUNT(t.rowid) = 0 THEN 'missing'
        WHEN SUM(CASE WHEN t.audio_uri IS NOT NULL AND t.audio_uri != '' THEN 1 ELSE 0 END) = COUNT(t.rowid) THEN 'complete'
        WHEN SUM(CASE WHEN t.audio_uri IS NOT NULL AND t.audio_uri != '' THEN 1 ELSE 0 END) > 0 THEN 'partial'
        ELSE 'missing'
    END
    FROM tracks t
    WHERE t.album_rowid = albums.rowid
);
"

echo "Done!"
echo ""
echo "=== New album availability distribution ==="
sqlite3 "$DB_PATH" "SELECT album_availability, COUNT(*) as count FROM albums GROUP BY album_availability ORDER BY album_availability;"
