#!/bin/bash
#
# Populate item_availability table in search.db from catalog.db
#
# Usage:
#   ./populate-availability.sh --catalog-db PATH --search-db PATH [OPTIONS]
#
# Required:
#   --catalog-db PATH   Path to catalog.db
#   --search-db PATH    Path to search.db
#
# Options:
#   --remote-host HOST  SSH host (e.g., user@host). If not set, runs locally.
#   --dry-run           Show what would be done without making changes
#   --batch-size N      Number of items to process per batch (default: 1000)
#
# The script can be safely interrupted with Ctrl+C and will resume from where it left off.

set -euo pipefail

# Configuration (defaults)
REMOTE_HOST="${REMOTE_HOST:-}"
CATALOG_DB="${CATALOG_DB:-}"
SEARCH_DB="${SEARCH_DB:-}"
BATCH_SIZE=1000
DRY_RUN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --batch-size)
            BATCH_SIZE="$2"
            shift 2
            ;;
        --remote-host)
            REMOTE_HOST="$2"
            shift 2
            ;;
        --catalog-db)
            CATALOG_DB="$2"
            shift 2
            ;;
        --search-db)
            SEARCH_DB="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 --catalog-db PATH --search-db PATH [OPTIONS]"
            echo ""
            echo "Populate item_availability table in search.db from catalog.db"
            echo ""
            echo "Required:"
            echo "  --catalog-db PATH   Path to catalog.db"
            echo "  --search-db PATH    Path to search.db"
            echo ""
            echo "Options:"
            echo "  --remote-host HOST  SSH host (e.g., user@host). If not set, runs locally."
            echo "  --dry-run           Show what would be done without making changes"
            echo "  --batch-size N      Number of items to process per batch (default: 1000)"
            echo ""
            echo "Environment variables:"
            echo "  REMOTE_HOST, CATALOG_DB, SEARCH_DB can also be set via environment"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Validate required arguments
if [[ -z "$CATALOG_DB" ]]; then
    echo "Error: --catalog-db is required"
    exit 1
fi
if [[ -z "$SEARCH_DB" ]]; then
    echo "Error: --search-db is required"
    exit 1
fi

# Cleanup on exit
cleanup() {
    echo ""
    echo "Interrupted. Progress saved. Run again to resume."
    exit 1
}
trap cleanup SIGINT SIGTERM

# Helper to run SQL (locally or remotely)
run_catalog_sql() {
    if [[ -n "$REMOTE_HOST" ]]; then
        ssh "$REMOTE_HOST" "sqlite3 '$CATALOG_DB' \"$1\""
    else
        sqlite3 "$CATALOG_DB" "$1"
    fi
}

run_search_sql() {
    if [[ -n "$REMOTE_HOST" ]]; then
        ssh "$REMOTE_HOST" "sqlite3 '$SEARCH_DB' \"$1\""
    else
        sqlite3 "$SEARCH_DB" "$1"
    fi
}

run_search_sql_batch() {
    if [[ -n "$REMOTE_HOST" ]]; then
        ssh "$REMOTE_HOST" "sqlite3 '$SEARCH_DB'" <<< "$1"
    else
        sqlite3 "$SEARCH_DB" <<< "$1"
    fi
}

echo "=== Populate Availability Script ==="
echo "Remote host: ${REMOTE_HOST:-<local>}"
echo "Catalog DB:  $CATALOG_DB"
echo "Search DB:   $SEARCH_DB"
echo "Batch size:  $BATCH_SIZE"
echo "Dry run:     $DRY_RUN"
echo ""

# Step 1: Create item_availability table if it doesn't exist
echo "[1/4] Checking/creating item_availability table..."
TABLE_EXISTS=$(run_search_sql "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='item_availability'")
if [[ "$TABLE_EXISTS" == "0" ]]; then
    if [[ "$DRY_RUN" == "true" ]]; then
        echo "  [DRY-RUN] Would create item_availability table"
    else
        echo "  Creating item_availability table..."
        run_search_sql_batch "
            CREATE TABLE IF NOT EXISTS item_availability (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                is_available INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_availability_lookup
                ON item_availability(item_id, item_type, is_available);
        "
        echo "  Table created."
    fi
else
    echo "  Table already exists."
fi

# Step 2: Get counts
echo ""
echo "[2/4] Analyzing data..."
SEARCH_COUNT=$(run_search_sql "SELECT COUNT(*) FROM search_index")
AVAIL_COUNT=$(run_search_sql "SELECT COUNT(*) FROM item_availability" 2>/dev/null || echo "0")
echo "  Items in search_index: $SEARCH_COUNT"
echo "  Items in item_availability: $AVAIL_COUNT"

# Count by type
echo "  Breakdown by type in search_index:"
ARTISTS=$(run_search_sql "SELECT COUNT(*) FROM search_index WHERE item_type = 'artist'")
ALBUMS=$(run_search_sql "SELECT COUNT(*) FROM search_index WHERE item_type = 'album'")
TRACKS=$(run_search_sql "SELECT COUNT(*) FROM search_index WHERE item_type = 'track'")
echo "    Artists: $ARTISTS"
echo "    Albums:  $ALBUMS"
echo "    Tracks:  $TRACKS"

# Check what's already processed
PROCESSED_ARTISTS=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE item_type = 'artist'" 2>/dev/null || echo "0")
PROCESSED_ALBUMS=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE item_type = 'album'" 2>/dev/null || echo "0")
PROCESSED_TRACKS=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE item_type = 'track'" 2>/dev/null || echo "0")

echo ""
echo "  Already processed:"
echo "    Artists: $PROCESSED_ARTISTS / $ARTISTS"
echo "    Albums:  $PROCESSED_ALBUMS / $ALBUMS"
echo "    Tracks:  $PROCESSED_TRACKS / $TRACKS"

# Step 3: Process each type
echo ""
echo "[3/4] Processing availability data..."

process_type() {
    local item_type=$1
    local catalog_table=$2
    local available_column=$3
    local total=$4
    local processed=$5

    if [[ "$processed" -ge "$total" ]]; then
        echo "  [$item_type] Already complete ($total items)"
        return
    fi

    local remaining=$((total - processed))
    echo "  [$item_type] Processing $remaining remaining items..."

    local offset=$processed
    local batch_num=0

    while [[ $offset -lt $total ]]; do
        batch_num=$((batch_num + 1))
        local current_batch=$((offset + BATCH_SIZE > total ? total - offset : BATCH_SIZE))
        local progress_pct=$((offset * 100 / total))

        printf "\r    Progress: %d/%d (%d%%) - Batch %d" "$offset" "$total" "$progress_pct" "$batch_num"

        if [[ "$DRY_RUN" == "true" ]]; then
            # In dry run, just show what we'd do
            offset=$((offset + BATCH_SIZE))
            continue
        fi

        # Get batch of item IDs from search_index
        local ids
        ids=$(run_search_sql "SELECT item_id FROM search_index WHERE item_type = '$item_type' LIMIT $BATCH_SIZE OFFSET $offset")

        if [[ -z "$ids" ]]; then
            break
        fi

        # Build IN clause for catalog query
        local in_clause=""
        while IFS= read -r id; do
            if [[ -n "$id" ]]; then
                if [[ -n "$in_clause" ]]; then
                    in_clause="$in_clause,'$id'"
                else
                    in_clause="'$id'"
                fi
            fi
        done <<< "$ids"

        if [[ -z "$in_clause" ]]; then
            break
        fi

        # Query availability from catalog
        local avail_data
        if [[ "$item_type" == "album" ]]; then
            # Albums: available if album_availability != 'missing'
            avail_data=$(run_catalog_sql "SELECT id, CASE WHEN album_availability != 'missing' THEN 1 ELSE 0 END FROM $catalog_table WHERE id IN ($in_clause)")
        else
            # Artists/Tracks: use the boolean column directly
            avail_data=$(run_catalog_sql "SELECT id, $available_column FROM $catalog_table WHERE id IN ($in_clause)")
        fi

        # Build INSERT statements
        local insert_sql="BEGIN TRANSACTION;"
        while IFS='|' read -r id is_avail; do
            if [[ -n "$id" ]]; then
                insert_sql="$insert_sql
INSERT OR REPLACE INTO item_availability (item_id, item_type, is_available) VALUES ('$id', '$item_type', ${is_avail:-0});"
            fi
        done <<< "$avail_data"
        insert_sql="$insert_sql
COMMIT;"

        # Execute batch insert
        run_search_sql_batch "$insert_sql"

        offset=$((offset + BATCH_SIZE))
    done

    echo ""
    echo "  [$item_type] Complete!"
}

# Process artists
process_type "artist" "artists" "artist_available" "$ARTISTS" "$PROCESSED_ARTISTS"

# Process albums
process_type "album" "albums" "album_availability" "$ALBUMS" "$PROCESSED_ALBUMS"

# Process tracks
process_type "track" "tracks" "track_available" "$TRACKS" "$PROCESSED_TRACKS"

# Step 4: Summary
echo ""
echo "[4/4] Summary..."
FINAL_COUNT=$(run_search_sql "SELECT COUNT(*) FROM item_availability" 2>/dev/null || echo "0")
AVAILABLE_COUNT=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE is_available = 1" 2>/dev/null || echo "0")

echo "  Total items in item_availability: $FINAL_COUNT"
echo "  Available items: $AVAILABLE_COUNT"

echo ""
echo "  Breakdown:"
for type in artist album track; do
    total=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE item_type = '$type'" 2>/dev/null || echo "0")
    avail=$(run_search_sql "SELECT COUNT(*) FROM item_availability WHERE item_type = '$type' AND is_available = 1" 2>/dev/null || echo "0")
    echo "    $type: $avail / $total available"
done

echo ""
echo "=== Done ==="
