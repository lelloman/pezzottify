#!/usr/bin/env python3
"""
Script to set track_available and artist_available based on actual audio files.

Runs everything on the server via SSH:
1. Lists audio files and extracts track IDs (cached locally)
2. Uploads IDs to server and updates tracks
3. Updates artists based on their linked tracks

Usage:
    python fix-track-artist-availability.py --host USER@HOST --db-path /path/to/catalog.db --audio-path /path/to/audio

Example:
    python fix-track-artist-availability.py --host lelloman@192.168.1.101 --db-path /home/lelloman/homelab-data/pezzottify/catalog.db --audio-path /home/lelloman/homelab-data/pezzottify/audio
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

# Local cache directory (next to this script)
CACHE_DIR = Path(__file__).parent / ".track-artist-availability-cache"


def run_ssh(host: str, cmd: str) -> tuple[int, str, str]:
    """Run a command on the remote host via SSH. Returns (returncode, stdout, stderr)."""
    result = subprocess.run(
        ["ssh", host, cmd],
        capture_output=True,
        text=True,
        timeout=600,
    )
    return result.returncode, result.stdout, result.stderr


def load_cache(name: str) -> list[str] | None:
    """Load a cache file if it exists."""
    cache_file = CACHE_DIR / f"{name}.json"
    if cache_file.exists():
        with open(cache_file) as f:
            return json.load(f)
    return None


def save_cache(name: str, data: list[str]):
    """Save data to a cache file."""
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    cache_file = CACHE_DIR / f"{name}.json"
    with open(cache_file, "w") as f:
        json.dump(data, f)
    print(f"  Cached to {cache_file}")


def clear_cache():
    """Clear the cache directory."""
    if CACHE_DIR.exists():
        import shutil
        shutil.rmtree(CACHE_DIR)
        print(f"Cache cleared: {CACHE_DIR}")


def main():
    parser = argparse.ArgumentParser(description="Fix track and artist availability based on audio files")
    parser.add_argument("--host", required=True, help="SSH host (user@host)")
    parser.add_argument("--db-path", required=True, help="Path to catalog.db on remote host")
    parser.add_argument("--audio-path", required=True, help="Path to audio directory on remote host")
    parser.add_argument("--dry-run", action="store_true", help="Don't apply changes, just show what would happen")
    parser.add_argument("--clear-cache", action="store_true", help="Clear local cache and start fresh")

    args = parser.parse_args()

    if args.clear_cache:
        clear_cache()

    print(f"Host: {args.host}")
    print(f"Database: {args.db_path}")
    print(f"Audio path: {args.audio_path}")
    if args.dry_run:
        print("DRY RUN MODE")
    print()

    # Step 1: Find audio files (use cache if available)
    print("=== Step 1: Finding audio files on server ===")

    audio_ids = load_cache("audio_ids")
    if audio_ids is not None:
        print(f"  Loaded {len(audio_ids)} audio IDs from cache")
    else:
        find_cmd = f"find '{args.audio_path}' -name '*.ogg' -type f 2>/dev/null | xargs -I{{}} basename {{}} .ogg"
        code, stdout, stderr = run_ssh(args.host, find_cmd)
        if code != 0:
            print(f"Error finding audio files: {stderr}")
            sys.exit(1)

        audio_ids = [line.strip() for line in stdout.strip().split("\n") if line.strip()]
        print(f"  Found {len(audio_ids)} audio files")
        save_cache("audio_ids", audio_ids)

    if len(audio_ids) == 0:
        print("No audio files found, nothing to update.")
        sys.exit(0)

    # Upload IDs to server via scp
    print(f"  Uploading {len(audio_ids)} IDs to server...")
    local_tmp = CACHE_DIR / "audio_ids.txt"
    local_tmp.write_text("\n".join(audio_ids))

    tmp_file = "/tmp/audio_track_ids.txt"
    result = subprocess.run(
        ["scp", str(local_tmp), f"{args.host}:{tmp_file}"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        print(f"Error uploading IDs to server: {result.stderr}")
        sys.exit(1)

    # Step 2: Update tracks
    print("\n=== Step 2: Updating track availability ===")

    if args.dry_run:
        # Use temp table approach to count
        count_cmd = f"""
sqlite3 '{args.db_path}' <<'EOF'
CREATE TEMP TABLE audio_ids(id TEXT);
.mode csv
.import {tmp_file} audio_ids
SELECT COUNT(*) FROM tracks WHERE id IN (SELECT id FROM audio_ids);
EOF
"""
        code, stdout, stderr = run_ssh(args.host, count_cmd)
        count = stdout.strip() if code == 0 else "?"
        print(f"  Would update {count} tracks to available")
    else:
        # Reset all tracks to unavailable
        run_ssh(args.host, f"sqlite3 '{args.db_path}' 'UPDATE tracks SET track_available = 0'")

        # Create temp table, import IDs, and update tracks
        update_cmd = f"""
sqlite3 '{args.db_path}' <<'EOF'
CREATE TEMP TABLE audio_ids(id TEXT);
.mode csv
.import {tmp_file} audio_ids
UPDATE tracks SET track_available = 1 WHERE id IN (SELECT id FROM audio_ids);
SELECT 'Updated ' || changes() || ' tracks';
EOF
"""
        code, stdout, stderr = run_ssh(args.host, update_cmd)
        if code != 0:
            print(f"Error updating tracks: {stderr}")
            sys.exit(1)
        print(f"  {stdout.strip()}")

        # Verify
        code, stdout, _ = run_ssh(args.host, f"sqlite3 '{args.db_path}' 'SELECT COUNT(*) FROM tracks WHERE track_available = 1'")
        print(f"  Verified: {stdout.strip()} tracks now available")

    # Step 3: Update artists based on their tracks
    print("\n=== Step 3: Updating artist availability ===")

    artist_sql = """
        UPDATE artists SET artist_available = 1
        WHERE rowid IN (
            SELECT DISTINCT ar.rowid
            FROM artists ar
            JOIN track_artists ta ON ta.artist_rowid = ar.rowid
            JOIN tracks t ON t.rowid = ta.track_rowid
            WHERE t.track_available = 1
        )
    """

    if args.dry_run:
        # In dry-run, tracks aren't updated yet, so join with temp table instead
        count_cmd = f"""
sqlite3 '{args.db_path}' <<'EOF'
CREATE TEMP TABLE audio_ids(id TEXT);
.mode csv
.import {tmp_file} audio_ids
SELECT COUNT(DISTINCT ar.rowid)
FROM artists ar
JOIN track_artists ta ON ta.artist_rowid = ar.rowid
JOIN tracks t ON t.rowid = ta.track_rowid
WHERE t.id IN (SELECT id FROM audio_ids);
EOF
"""
        code, stdout, _ = run_ssh(args.host, count_cmd)
        print(f"  Would update {stdout.strip()} artists to available")
    else:
        update_cmd = f"""
            sqlite3 '{args.db_path}' "UPDATE artists SET artist_available = 0"
            sqlite3 '{args.db_path}' "{artist_sql}"
            sqlite3 '{args.db_path}' "SELECT COUNT(*) FROM artists WHERE artist_available = 1"
        """
        code, stdout, stderr = run_ssh(args.host, update_cmd)
        if code != 0:
            print(f"Error updating artists: {stderr}")
            sys.exit(1)
        print(f"  Updated {stdout.strip()} artists to available")

    # Cleanup
    run_ssh(args.host, f"rm -f {tmp_file}")

    print("\n=== Done! ===")


if __name__ == "__main__":
    main()
