#!/usr/bin/env python3
"""
Script to set track_available and artist_available based on actual audio files.

Usage:
    python fix-track-artist-availability.py --host USER@HOST --db-path /path/to/catalog.db --audio-path /path/to/audio
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

CACHE_DIR = Path(__file__).parent / ".track-artist-availability-cache"
BATCH_SIZE = 200


def run_ssh(host: str, cmd: str, timeout: int = 120) -> tuple[int, str, str]:
    """Run a command on the remote host via SSH."""
    result = subprocess.run(
        ["ssh", host, cmd],
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    return result.returncode, result.stdout, result.stderr


def load_cache(name: str):
    cache_file = CACHE_DIR / f"{name}.json"
    if cache_file.exists():
        with open(cache_file) as f:
            return json.load(f)
    return None


def save_cache(name: str, data):
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    cache_file = CACHE_DIR / f"{name}.json"
    with open(cache_file, "w") as f:
        json.dump(data, f)


def clear_cache():
    if CACHE_DIR.exists():
        import shutil
        shutil.rmtree(CACHE_DIR)
        print(f"Cache cleared: {CACHE_DIR}")


def main():
    parser = argparse.ArgumentParser(description="Fix track and artist availability")
    parser.add_argument("--host", required=True, help="SSH host (user@host)")
    parser.add_argument("--db-path", required=True, help="Path to catalog.db on remote host")
    parser.add_argument("--audio-path", required=True, help="Path to audio directory on remote host")
    parser.add_argument("--dry-run", action="store_true", help="Don't apply changes")
    parser.add_argument("--clear-cache", action="store_true", help="Clear cache and start fresh")

    args = parser.parse_args()

    if args.clear_cache:
        clear_cache()

    print(f"Host: {args.host}")
    print(f"Database: {args.db_path}")
    print(f"Audio path: {args.audio_path}")
    if args.dry_run:
        print("DRY RUN MODE")
    print()

    # Step 1: Find audio files (cached)
    print("=== Step 1: Finding audio files ===")
    audio_ids = load_cache("audio_ids")
    if audio_ids is not None:
        print(f"  Loaded {len(audio_ids)} audio IDs from cache")
    else:
        print("  Scanning server for audio files...")
        find_cmd = f"find '{args.audio_path}' -name '*.ogg' -type f 2>/dev/null | xargs -I{{}} basename {{}} .ogg"
        code, stdout, stderr = run_ssh(args.host, find_cmd, timeout=300)
        if code != 0:
            print(f"Error: {stderr}")
            sys.exit(1)
        audio_ids = [line.strip() for line in stdout.strip().split("\n") if line.strip()]
        print(f"  Found {len(audio_ids)} audio files")
        save_cache("audio_ids", audio_ids)

    if not audio_ids:
        print("No audio files found.")
        sys.exit(0)

    # Step 2: Update tracks in batches
    print(f"\n=== Step 2: Updating tracks ({len(audio_ids)} total) ===")

    # Load progress
    progress = load_cache("progress") or {"tracks_done": 0, "tracks_updated": 0}
    start_idx = progress["tracks_done"]
    tracks_updated = progress["tracks_updated"]

    if start_idx > 0:
        print(f"  Resuming from batch {start_idx // BATCH_SIZE + 1}")

    if args.dry_run:
        # Count how many tracks would be updated
        total = 0
        for i in range(0, len(audio_ids), BATCH_SIZE):
            batch = audio_ids[i:i + BATCH_SIZE]
            ids_str = ",".join(f"'{tid}'" for tid in batch)
            query = f"SELECT COUNT(*) FROM tracks WHERE id IN ({ids_str})"
            code, stdout, _ = run_ssh(args.host, f"sqlite3 '{args.db_path}' \"{query}\"")
            if code == 0:
                total += int(stdout.strip())
            print(f"\r  Checking... {min(i + BATCH_SIZE, len(audio_ids))}/{len(audio_ids)}", end="", flush=True)
        print(f"\n  Would update {total} tracks to available")
    else:
        for i in range(start_idx, len(audio_ids), BATCH_SIZE):
            batch = audio_ids[i:i + BATCH_SIZE]
            ids_str = ",".join(f"'{tid}'" for tid in batch)

            sql = f"UPDATE tracks SET track_available = 1 WHERE id IN ({ids_str})"
            code, stdout, stderr = run_ssh(args.host, f"sqlite3 '{args.db_path}' \"{sql}\"")

            if code != 0:
                print(f"\n  Error at batch {i // BATCH_SIZE + 1}: {stderr}")
                save_cache("progress", {"tracks_done": i, "tracks_updated": tracks_updated})
                sys.exit(1)

            tracks_updated += len(batch)
            progress_pct = (i + len(batch)) / len(audio_ids) * 100
            print(f"\r  Progress: {i + len(batch)}/{len(audio_ids)} ({progress_pct:.1f}%)", end="", flush=True)

            # Save progress periodically
            if (i // BATCH_SIZE) % 10 == 0:
                save_cache("progress", {"tracks_done": i + len(batch), "tracks_updated": tracks_updated})

        print()
        # Verify
        code, stdout, _ = run_ssh(args.host, f"sqlite3 '{args.db_path}' 'SELECT COUNT(*) FROM tracks WHERE track_available = 1'")
        print(f"  Done. {stdout.strip()} tracks now available")
        save_cache("progress", {"tracks_done": len(audio_ids), "tracks_updated": tracks_updated})

    # Step 3: Update artists
    print("\n=== Step 3: Updating artists ===")

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
        count_sql = """
            SELECT COUNT(DISTINCT ar.rowid)
            FROM artists ar
            JOIN track_artists ta ON ta.artist_rowid = ar.rowid
            JOIN tracks t ON t.rowid = ta.track_rowid
            WHERE t.track_available = 1
        """
        code, stdout, _ = run_ssh(args.host, f"sqlite3 '{args.db_path}' \"{count_sql}\"", timeout=300)
        print(f"  Would update {stdout.strip()} artists to available")
    else:
        print("  Updating artists (this may take a moment)...")
        code, stdout, stderr = run_ssh(args.host, f"sqlite3 '{args.db_path}' \"{artist_sql}\"", timeout=600)
        if code != 0:
            print(f"  Error: {stderr}")
            sys.exit(1)

        code, stdout, _ = run_ssh(args.host, f"sqlite3 '{args.db_path}' 'SELECT COUNT(*) FROM artists WHERE artist_available = 1'")
        print(f"  Done. {stdout.strip()} artists now available")

    print("\n=== Done! ===")


if __name__ == "__main__":
    main()
