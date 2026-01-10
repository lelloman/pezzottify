#!/usr/bin/env python3
"""
Script to fix album_availability based on actual audio files on the server.

Runs locally, SSHes to server for data, persists state locally for resume capability.

Usage:
    python fix-album-availability.py --host USER@HOST --db-path /path/to/catalog.db --audio-path /path/to/audio

Example:
    python fix-album-availability.py --host lelloman@192.168.1.101 --db-path /home/lelloman/homelab-data/pezzottify/catalog.db --audio-path /home/lelloman/homelab-data/pezzottify/audio
"""

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Optional


# Local cache directory
CACHE_DIR = Path(__file__).parent / ".availability-fix-cache"


@dataclass
class AlbumData:
    id: str
    name: str
    current_availability: str
    total_tracks: int
    track_ids: list[str]


def run_ssh(host: str, cmd: str, allow_partial_output: bool = False) -> str:
    """Run a command on the remote host via SSH."""
    result = subprocess.run(
        ["ssh", host, cmd],
        capture_output=True,
        text=True,
        timeout=300,
    )
    if result.returncode != 0:
        # Some commands like 'find' may return non-zero but still have valid output
        if allow_partial_output and result.stdout.strip():
            print(f"  Warning: Command returned exit code {result.returncode}, but got output")
            return result.stdout
        error_msg = result.stderr or f"Exit code {result.returncode}, stdout: {result.stdout[:500] if result.stdout else '(empty)'}"
        raise RuntimeError(f"SSH command failed: {error_msg}")
    return result.stdout


def run_ssh_query(host: str, db_path: str, query: str) -> list[tuple]:
    """Run a SQLite query on the remote host."""
    # Escape quotes in query
    escaped_query = query.replace("'", "'\"'\"'")
    cmd = f"sqlite3 -separator '|' '{db_path}' '{escaped_query}'"
    output = run_ssh(host, cmd)
    rows = []
    for line in output.strip().split("\n"):
        if line:
            rows.append(tuple(line.split("|")))
    return rows


def load_cache(name: str) -> Optional[dict]:
    """Load a cache file if it exists."""
    cache_file = CACHE_DIR / f"{name}.json"
    if cache_file.exists():
        with open(cache_file) as f:
            return json.load(f)
    return None


def save_cache(name: str, data: dict):
    """Save data to a cache file."""
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    cache_file = CACHE_DIR / f"{name}.json"
    with open(cache_file, "w") as f:
        json.dump(data, f, indent=2)
    print(f"  Cached to {cache_file}")


def step1_collect_audio_files(host: str, audio_path: str) -> set[str]:
    """Step 1: Collect all audio file IDs from the server."""
    print("\n=== Step 1: Collecting audio files from server ===")

    cached = load_cache("audio_file_ids")
    if cached:
        audio_ids = set(cached)
        print(f"  Loaded {len(audio_ids)} audio file IDs from cache")
        return audio_ids

    print(f"  Scanning {audio_path} on {host}...")

    # Find all .ogg files and extract their IDs (filename without extension)
    cmd = f"find '{audio_path}' -name '*.ogg' -type f 2>/dev/null"
    output = run_ssh(host, cmd, allow_partial_output=True)

    audio_ids = set()
    for line in output.strip().split("\n"):
        if line:
            # Extract ID from path like: audio/2U/OG/me/2UOGmejRXbAkgUmSPaTt5L.ogg
            filename = os.path.basename(line)
            audio_id = filename.replace(".ogg", "")
            audio_ids.add(audio_id)

    print(f"  Found {len(audio_ids)} audio files")
    save_cache("audio_file_ids", list(audio_ids))
    return audio_ids


def step2_build_track_mapping(host: str, db_path: str, audio_ids: set[str]) -> dict[str, str]:
    """Step 2: Build mapping of track_id -> album_id (only for tracks with audio)."""
    print("\n=== Step 2: Building track -> album mapping ===")

    cached = load_cache("track_to_album")
    if cached:
        print(f"  Loaded {len(cached)} track mappings from cache")
        return cached

    print(f"  Querying tracks that have audio files ({len(audio_ids)} IDs)...")

    # Query in batches to avoid command line length limits
    track_to_album = {}
    audio_list = list(audio_ids)
    batch_size = 500

    for i in range(0, len(audio_list), batch_size):
        batch = audio_list[i:i + batch_size]
        ids_str = ",".join(f"'{tid}'" for tid in batch)
        query = f"""
            SELECT t.id, a.id
            FROM tracks t
            JOIN albums a ON t.album_rowid = a.rowid
            WHERE t.id IN ({ids_str})
        """
        rows = run_ssh_query(host, db_path, query)
        for track_id, album_id in rows:
            track_to_album[track_id] = album_id

        if (i + batch_size) % 2000 == 0 or i + batch_size >= len(audio_list):
            print(f"  Processed {min(i + batch_size, len(audio_list))}/{len(audio_list)} audio IDs...")

    print(f"  Found {len(track_to_album)} tracks with audio")
    save_cache("track_to_album", track_to_album)
    return track_to_album


def step3_build_album_data(host: str, db_path: str, track_to_album: dict[str, str]) -> dict[str, AlbumData]:
    """Step 3: Build album data (only for albums that have tracks with audio)."""
    print("\n=== Step 3: Building album data ===")

    cached = load_cache("albums")
    if cached:
        albums = {k: AlbumData(**v) for k, v in cached.items()}
        print(f"  Loaded {len(albums)} albums from cache")
        return albums

    # Get unique album IDs from tracks with audio
    album_ids_with_audio = set(track_to_album.values())
    print(f"  Found {len(album_ids_with_audio)} albums with at least one audio file")

    # Query album info and track counts for these albums
    albums: dict[str, AlbumData] = {}
    album_list = list(album_ids_with_audio)
    batch_size = 200

    for i in range(0, len(album_list), batch_size):
        batch = album_list[i:i + batch_size]
        ids_str = ",".join(f"'{aid}'" for aid in batch)

        # Get album info + total track count in one query
        query = f"""
            SELECT a.id, a.name, a.album_availability, COUNT(t.rowid) as total_tracks
            FROM albums a
            LEFT JOIN tracks t ON t.album_rowid = a.rowid
            WHERE a.id IN ({ids_str})
            GROUP BY a.id
        """
        rows = run_ssh_query(host, db_path, query)

        for album_id, name, availability, total_tracks in rows:
            albums[album_id] = AlbumData(
                id=album_id,
                name=name,
                current_availability=availability,
                total_tracks=int(total_tracks),
                track_ids=[],  # We don't need to store all track IDs
            )

        if (i + batch_size) % 1000 == 0 or i + batch_size >= len(album_list):
            print(f"  Processed {min(i + batch_size, len(album_list))}/{len(album_list)} albums...")

    # Now populate track_ids only for tracks that have audio (from track_to_album)
    for track_id, album_id in track_to_album.items():
        if album_id in albums:
            albums[album_id].track_ids.append(track_id)

    print(f"  Built data for {len(albums)} albums")

    # Save cache
    cache_data = {k: asdict(v) for k, v in albums.items()}
    save_cache("albums", cache_data)

    return albums


def step4_compute_availability(
    audio_ids: set[str],
    albums: dict[str, AlbumData],
) -> dict[str, str]:
    """Step 4: Compute correct availability for each album."""
    print("\n=== Step 4: Computing availability ===")

    cached = load_cache("computed_availability")
    if cached:
        print(f"  Loaded {len(cached)} computed availabilities from cache")
        return cached

    computed: dict[str, str] = {}

    for i, (album_id, album) in enumerate(albums.items()):
        if album.total_tracks == 0:
            computed[album_id] = "missing"
            continue

        tracks_with_audio = sum(1 for tid in album.track_ids if tid in audio_ids)

        if tracks_with_audio == album.total_tracks:
            computed[album_id] = "complete"
        elif tracks_with_audio > 0:
            computed[album_id] = "partial"
        else:
            computed[album_id] = "missing"

        if (i + 1) % 1000 == 0:
            print(f"  Processed {i + 1}/{len(albums)} albums...")

    print(f"  Computed availability for {len(computed)} albums")
    save_cache("computed_availability", computed)

    return computed


def step5_find_albums_to_update(
    albums: dict[str, AlbumData],
    computed: dict[str, str],
) -> list[tuple[str, str, str]]:
    """Step 5: Find albums that need updating."""
    print("\n=== Step 5: Finding albums to update ===")

    to_update = []
    for album_id, correct in computed.items():
        album = albums.get(album_id)
        if album and album.current_availability != correct:
            to_update.append((album_id, album.current_availability, correct))

    print(f"  Found {len(to_update)} albums needing update")

    # Show distribution
    from collections import Counter
    changes = Counter((old, new) for _, old, new in to_update)
    print("\n  Changes breakdown:")
    for (old, new), count in sorted(changes.items()):
        print(f"    {old} -> {new}: {count}")

    # Show some examples
    print("\n  Examples (first 10):")
    for album_id, old, new in to_update[:10]:
        album = albums[album_id]
        name = album.name[:40] if len(album.name) > 40 else album.name
        print(f"    {name:<40} {old:>8} -> {new}")

    return to_update


def step6_apply_updates(
    host: str,
    db_path: str,
    to_update: list[tuple[str, str, str]],
    dry_run: bool = True,
):
    """Step 6: Apply updates to the database."""
    print("\n=== Step 6: Applying updates ===")

    if dry_run:
        print("  DRY RUN - no changes will be made")
        print(f"  Would update {len(to_update)} albums")
        return

    # Load progress
    progress_file = CACHE_DIR / "update_progress.json"
    updated_ids = set()
    if progress_file.exists():
        with open(progress_file) as f:
            updated_ids = set(json.load(f))
        print(f"  Resuming: {len(updated_ids)} already updated")

    # Filter out already updated
    remaining = [(aid, old, new) for aid, old, new in to_update if aid not in updated_ids]
    print(f"  {len(remaining)} albums remaining to update")

    if not remaining:
        print("  All updates already applied!")
        return

    # Update in batches
    batch_size = 100
    for i in range(0, len(remaining), batch_size):
        batch = remaining[i:i + batch_size]

        # Build UPDATE statements
        updates = []
        for album_id, _, new_availability in batch:
            # Escape single quotes in album_id
            safe_id = album_id.replace("'", "''")
            updates.append(
                f"UPDATE albums SET album_availability = '{new_availability}' WHERE id = '{safe_id}';"
            )

        sql = " ".join(updates)
        cmd = f"sqlite3 '{db_path}' \"{sql}\""

        try:
            run_ssh(host, cmd)

            # Record progress
            for album_id, _, _ in batch:
                updated_ids.add(album_id)

            with open(progress_file, "w") as f:
                json.dump(list(updated_ids), f)

            print(f"  Updated {len(updated_ids)}/{len(to_update)} albums")

        except Exception as e:
            print(f"  ERROR updating batch: {e}")
            print(f"  Progress saved. Re-run to continue.")
            raise

    print(f"  Successfully updated {len(to_update)} albums!")


def main():
    parser = argparse.ArgumentParser(description="Fix album availability based on audio files")
    parser.add_argument("--host", required=True, help="SSH host (user@host)")
    parser.add_argument("--db-path", required=True, help="Path to catalog.db on remote host")
    parser.add_argument("--audio-path", required=True, help="Path to audio directory on remote host")
    parser.add_argument("--dry-run", action="store_true", help="Don't apply changes")
    parser.add_argument("--clear-cache", action="store_true", help="Clear local cache and start fresh")
    parser.add_argument("--skip-to-step", type=int, help="Skip to a specific step (for debugging)")

    args = parser.parse_args()

    print(f"Host: {args.host}")
    print(f"Database: {args.db_path}")
    print(f"Audio path: {args.audio_path}")
    print(f"Cache dir: {CACHE_DIR}")

    if args.clear_cache and CACHE_DIR.exists():
        import shutil
        shutil.rmtree(CACHE_DIR)
        print("Cache cleared!")

    try:
        # Step 1: Collect audio files
        if not args.skip_to_step or args.skip_to_step <= 1:
            audio_ids = step1_collect_audio_files(args.host, args.audio_path)
        else:
            audio_ids = set(load_cache("audio_file_ids") or [])

        # Step 2: Build track -> album mapping (only for tracks with audio)
        if not args.skip_to_step or args.skip_to_step <= 2:
            track_to_album = step2_build_track_mapping(args.host, args.db_path, audio_ids)
        else:
            track_to_album = load_cache("track_to_album") or {}

        # Step 3: Build album data (only for albums with audio)
        if not args.skip_to_step or args.skip_to_step <= 3:
            albums = step3_build_album_data(args.host, args.db_path, track_to_album)
        else:
            cached = load_cache("albums") or {}
            albums = {k: AlbumData(**v) for k, v in cached.items()}

        # Step 4: Compute availability
        if not args.skip_to_step or args.skip_to_step <= 4:
            computed = step4_compute_availability(audio_ids, albums)
        else:
            computed = load_cache("computed_availability") or {}

        # Step 5: Find albums to update
        to_update = step5_find_albums_to_update(albums, computed)

        # Step 6: Apply updates
        step6_apply_updates(args.host, args.db_path, to_update, dry_run=args.dry_run)

        print("\n=== Done! ===")

    except KeyboardInterrupt:
        print("\n\nInterrupted! Progress saved. Re-run to continue.")
        sys.exit(1)
    except Exception as e:
        print(f"\n\nError: {e}")
        print("Progress saved. Re-run to continue.")
        raise


if __name__ == "__main__":
    main()
