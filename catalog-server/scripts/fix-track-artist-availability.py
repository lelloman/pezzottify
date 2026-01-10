#!/usr/bin/env python3
"""
Script to set track_available and artist_available based on actual audio files.

After running the schema migration (version 2), all tracks and artists default
to unavailable (0). This script:
1. Finds all audio files (.ogg) on the server
2. Sets track_available = 1 for tracks with audio files
3. Sets artist_available = 1 for artists with at least one available track

Usage:
    python fix-track-artist-availability.py --host USER@HOST --db-path /path/to/catalog.db --audio-path /path/to/audio

Example:
    python fix-track-artist-availability.py --host lelloman@192.168.1.101 --db-path /home/lelloman/homelab-data/pezzottify/catalog.db --audio-path /home/lelloman/homelab-data/pezzottify/audio
"""

import argparse
import subprocess
import os


def run_ssh(host: str, cmd: str, allow_partial_output: bool = False) -> str:
    """Run a command on the remote host via SSH."""
    result = subprocess.run(
        ["ssh", host, cmd],
        capture_output=True,
        text=True,
        timeout=300,
    )
    if result.returncode != 0:
        if allow_partial_output and result.stdout.strip():
            print(f"  Warning: Command returned exit code {result.returncode}, but got output")
            return result.stdout
        error_msg = result.stderr or f"Exit code {result.returncode}"
        raise RuntimeError(f"SSH command failed: {error_msg}")
    return result.stdout


def run_ssh_query(host: str, db_path: str, query: str) -> list[tuple]:
    """Run a SQLite query on the remote host."""
    escaped_query = query.replace("'", "'\"'\"'")
    cmd = f"sqlite3 -separator '|' '{db_path}' '{escaped_query}'"
    output = run_ssh(host, cmd)
    rows = []
    for line in output.strip().split("\n"):
        if line:
            rows.append(tuple(line.split("|")))
    return rows


def step1_collect_audio_files(host: str, audio_path: str) -> set[str]:
    """Step 1: Collect all audio file IDs from the server."""
    print("\n=== Step 1: Collecting audio files from server ===")
    print(f"  Scanning {audio_path} on {host}...")

    cmd = f"find '{audio_path}' -name '*.ogg' -type f 2>/dev/null"
    output = run_ssh(host, cmd, allow_partial_output=True)

    audio_ids = set()
    for line in output.strip().split("\n"):
        if line:
            filename = os.path.basename(line)
            audio_id = filename.replace(".ogg", "")
            audio_ids.add(audio_id)

    print(f"  Found {len(audio_ids)} audio files")
    return audio_ids


def step2_update_tracks(host: str, db_path: str, audio_ids: set[str], dry_run: bool):
    """Step 2: Set track_available = 1 for tracks with audio."""
    print("\n=== Step 2: Updating track availability ===")

    # First, reset all to 0 (in case of re-runs)
    if not dry_run:
        print("  Resetting all tracks to unavailable...")
        run_ssh(host, f"sqlite3 '{db_path}' \"UPDATE tracks SET track_available = 0\"")

    # Update in batches
    audio_list = list(audio_ids)
    batch_size = 500
    total_updated = 0

    for i in range(0, len(audio_list), batch_size):
        batch = audio_list[i:i + batch_size]
        ids_str = ",".join(f"'{tid}'" for tid in batch)

        if dry_run:
            # Count how many would be updated
            query = f"SELECT COUNT(*) FROM tracks WHERE id IN ({ids_str})"
            rows = run_ssh_query(host, db_path, query)
            total_updated += int(rows[0][0]) if rows else 0
        else:
            sql = f"UPDATE tracks SET track_available = 1 WHERE id IN ({ids_str})"
            run_ssh(host, f"sqlite3 '{db_path}' \"{sql}\"")
            total_updated += len(batch)

        if (i + batch_size) % 2000 == 0 or i + batch_size >= len(audio_list):
            print(f"  Processed {min(i + batch_size, len(audio_list))}/{len(audio_list)} audio IDs...")

    if dry_run:
        print(f"  Would update {total_updated} tracks to available")
    else:
        # Verify the update
        rows = run_ssh_query(host, db_path, "SELECT COUNT(*) FROM tracks WHERE track_available = 1")
        count = int(rows[0][0]) if rows else 0
        print(f"  Updated {count} tracks to available")


def step3_update_artists(host: str, db_path: str, dry_run: bool):
    """Step 3: Set artist_available = 1 for artists with at least one available track."""
    print("\n=== Step 3: Updating artist availability ===")

    if dry_run:
        # Count artists that would be updated
        query = """
            SELECT COUNT(DISTINCT ar.rowid)
            FROM artists ar
            JOIN track_artists ta ON ta.artist_rowid = ar.rowid
            JOIN tracks t ON t.rowid = ta.track_rowid
            WHERE t.track_available = 1
        """
        rows = run_ssh_query(host, db_path, query)
        count = int(rows[0][0]) if rows else 0
        print(f"  Would update {count} artists to available")
    else:
        # Reset all artists to unavailable
        print("  Resetting all artists to unavailable...")
        run_ssh(host, f"sqlite3 '{db_path}' \"UPDATE artists SET artist_available = 0\"")

        # Update artists with at least one available track
        print("  Setting artists with available tracks...")
        sql = """
            UPDATE artists SET artist_available = 1
            WHERE rowid IN (
                SELECT DISTINCT ar.rowid
                FROM artists ar
                JOIN track_artists ta ON ta.artist_rowid = ar.rowid
                JOIN tracks t ON t.rowid = ta.track_rowid
                WHERE t.track_available = 1
            )
        """
        run_ssh(host, f"sqlite3 '{db_path}' \"{sql}\"")

        # Verify
        rows = run_ssh_query(host, db_path, "SELECT COUNT(*) FROM artists WHERE artist_available = 1")
        count = int(rows[0][0]) if rows else 0
        print(f"  Updated {count} artists to available")


def main():
    parser = argparse.ArgumentParser(description="Fix track and artist availability based on audio files")
    parser.add_argument("--host", required=True, help="SSH host (user@host)")
    parser.add_argument("--db-path", required=True, help="Path to catalog.db on remote host")
    parser.add_argument("--audio-path", required=True, help="Path to audio directory on remote host")
    parser.add_argument("--dry-run", action="store_true", help="Don't apply changes")

    args = parser.parse_args()

    print(f"Host: {args.host}")
    print(f"Database: {args.db_path}")
    print(f"Audio path: {args.audio_path}")
    if args.dry_run:
        print("DRY RUN MODE - no changes will be made")

    # Step 1: Collect audio files
    audio_ids = step1_collect_audio_files(args.host, args.audio_path)

    # Step 2: Update tracks
    step2_update_tracks(args.host, args.db_path, audio_ids, args.dry_run)

    # Step 3: Update artists
    step3_update_artists(args.host, args.db_path, args.dry_run)

    print("\n=== Done! ===")


if __name__ == "__main__":
    main()
