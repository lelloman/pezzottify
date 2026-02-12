#!/usr/bin/env python3
"""
Import script for migrating old JSON-on-filesystem catalog to SQLite.

This script scans the old pezzottify-catalog directory structure and imports
artists, albums, tracks, and images into the new SQLite catalog database.

Directory structure expected:
    pezzottify-catalog/
    ├── artists/
    │   └── artist_<id>.json
    ├── albums/
    │   └── album_<id>/
    │       ├── album_<id>.json
    │       └── track_<id>.json (with .OGG_VORBIS_320, etc. audio files)
    └── images/
        └── <image_id> (no extension)

Usage:
    python import_catalog.py <old_catalog_path> <new_db_path>
"""

import argparse
import json
import os
import sqlite3
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# Database version (matches Rust code's BASE_DB_VERSION + schema version)
# BASE_DB_VERSION = 99999, schema version = 2
DB_VERSION = 99999 + 2  # 100001

# Schema SQL for catalog database (version 2 - with changelog tables)
SCHEMA_SQL = """
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Core tables
CREATE TABLE IF NOT EXISTS artists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    genres TEXT,
    activity_periods TEXT,
    display_image_id TEXT REFERENCES images(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    uri TEXT NOT NULL,
    size TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS albums (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    album_type TEXT NOT NULL,
    label TEXT,
    release_date INTEGER,
    genres TEXT,
    original_title TEXT,
    version_title TEXT,
    display_image_id TEXT REFERENCES images(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS tracks (
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

CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
CREATE INDEX IF NOT EXISTS idx_tracks_disc_track ON tracks(album_id, disc_number, track_number);

-- Relationship tables
CREATE TABLE IF NOT EXISTS album_artists (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    UNIQUE(album_id, artist_id)
);

CREATE INDEX IF NOT EXISTS idx_album_artists_artist ON album_artists(artist_id);

CREATE TABLE IF NOT EXISTS track_artists (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(track_id, artist_id, role)
);

CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist_id);

CREATE TABLE IF NOT EXISTS related_artists (
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    related_artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    UNIQUE(artist_id, related_artist_id)
);

CREATE TABLE IF NOT EXISTS artist_images (
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(artist_id, image_id, image_type)
);

CREATE INDEX IF NOT EXISTS idx_artist_images_artist ON artist_images(artist_id);

CREATE TABLE IF NOT EXISTS album_images (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL,
    position INTEGER NOT NULL,
    UNIQUE(album_id, image_id, image_type)
);

CREATE INDEX IF NOT EXISTS idx_album_images_album ON album_images(album_id);

-- Changelog tables (version 2)
CREATE TABLE IF NOT EXISTS catalog_batches (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    is_open INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    closed_at INTEGER,
    last_activity_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS catalog_change_log (
    id INTEGER PRIMARY KEY,
    batch_id TEXT NOT NULL REFERENCES catalog_batches(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    field_changes TEXT NOT NULL,
    entity_snapshot TEXT NOT NULL,
    display_summary TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_changelog_batch ON catalog_change_log(batch_id);
CREATE INDEX IF NOT EXISTS idx_changelog_entity ON catalog_change_log(entity_type, entity_id);
"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Import old JSON catalog to SQLite database"
    )
    parser.add_argument(
        "catalog_path",
        type=Path,
        help="Path to the old pezzottify-catalog directory",
    )
    parser.add_argument(
        "db_path",
        type=Path,
        help="Path for the new SQLite database (will be created if doesn't exist)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be imported without making changes",
    )
    return parser.parse_args()


class CatalogImporter:
    """Imports catalog data from JSON files to SQLite."""

    def __init__(self, catalog_path: Path, db_path: Path, dry_run: bool = False):
        self.catalog_path = catalog_path
        self.db_path = db_path
        self.dry_run = dry_run
        self.conn: Optional[sqlite3.Connection] = None

        # Counters
        self.artists_imported = 0
        self.albums_imported = 0
        self.tracks_imported = 0
        self.images_imported = 0
        self.relations_imported = 0
        self.errors: List[str] = []

    def connect(self):
        """Connect to the SQLite database and create schema if needed."""
        if self.dry_run:
            print(f"[DRY RUN] Would connect to {self.db_path}")
            return

        is_new_db = not self.db_path.exists()
        self.conn = sqlite3.connect(self.db_path)
        self.conn.execute("PRAGMA foreign_keys = ON")

        if is_new_db:
            print(f"Creating new database with schema...")
            self.conn.executescript(SCHEMA_SQL)
            self.conn.execute(f"PRAGMA user_version = {DB_VERSION}")
            self.conn.commit()
            print(f"Database created with version {DB_VERSION}")
        else:
            # Check version
            version = self.conn.execute("PRAGMA user_version").fetchone()[0]
            print(f"Existing database version: {version}")

    def close(self):
        """Close the database connection."""
        if self.conn:
            self.conn.close()

    def import_all(self):
        """Run the full import process."""
        print(f"Importing catalog from {self.catalog_path} to {self.db_path}")
        print()

        self.connect()

        try:
            # Import in order of dependencies
            self.import_images()
            self.import_artists()
            self.import_albums()
            self.import_tracks()
            self.import_relations()

            if not self.dry_run:
                self.conn.commit()

            self.print_summary()
        except Exception as e:
            print(f"Error during import: {e}")
            if self.conn:
                self.conn.rollback()
            raise
        finally:
            self.close()

    def import_images(self):
        """Import images from the images directory."""
        images_dir = self.catalog_path / "images"
        if not images_dir.exists():
            print("No images directory found, skipping images")
            return

        print("Importing images...")

        # Images are stored as flat files with their ID as filename
        for image_file in images_dir.iterdir():
            if image_file.is_dir():
                # Skip subdirectories like "artists"
                continue

            image_id = image_file.name
            # Calculate relative URI for the image
            relative_uri = f"images/{image_id}"

            # We don't have size metadata from the file, so use defaults
            self._insert_image(image_id, relative_uri, "DEFAULT", 0, 0)

    def import_artists(self):
        """Import artists from the artists directory."""
        artists_dir = self.catalog_path / "artists"
        if not artists_dir.exists():
            print("No artists directory found")
            return

        print("Importing artists...")

        for artist_file in artists_dir.glob("artist_*.json"):
            try:
                with open(artist_file) as f:
                    data = json.load(f)

                self._insert_artist(data)
                self._insert_artist_images(data)

            except Exception as e:
                self.errors.append(f"Error importing artist {artist_file}: {e}")

    def import_albums(self):
        """Import albums from the albums directory."""
        albums_dir = self.catalog_path / "albums"
        if not albums_dir.exists():
            print("No albums directory found")
            return

        print("Importing albums...")

        for album_dir in albums_dir.iterdir():
            if not album_dir.is_dir():
                continue

            album_id = album_dir.name.replace("album_", "")
            album_json = album_dir / f"album_{album_id}.json"

            if not album_json.exists():
                self.errors.append(f"No album.json found in {album_dir}")
                continue

            try:
                with open(album_json) as f:
                    data = json.load(f)

                self._insert_album(data)
                self._insert_album_images(data)
                self._insert_album_artists(data)

            except Exception as e:
                self.errors.append(f"Error importing album {album_json}: {e}")

    def import_tracks(self):
        """Import tracks from album directories."""
        albums_dir = self.catalog_path / "albums"
        if not albums_dir.exists():
            return

        print("Importing tracks...")

        for album_dir in albums_dir.iterdir():
            if not album_dir.is_dir():
                continue

            album_id = album_dir.name.replace("album_", "")

            for track_file in album_dir.glob("track_*.json"):
                try:
                    with open(track_file) as f:
                        data = json.load(f)

                    # Find the audio file
                    audio_uri = self._find_audio_file(track_file, data)

                    self._insert_track(data, audio_uri)
                    self._insert_track_artists(data)

                except Exception as e:
                    self.errors.append(f"Error importing track {track_file}: {e}")

    def import_relations(self):
        """Import relationships between entities."""
        artists_dir = self.catalog_path / "artists"
        if not artists_dir.exists():
            return

        print("Importing relationships...")

        for artist_file in artists_dir.glob("artist_*.json"):
            try:
                with open(artist_file) as f:
                    data = json.load(f)

                self._insert_related_artists(data)

            except Exception as e:
                self.errors.append(f"Error importing relations for {artist_file}: {e}")

    def _find_audio_file(self, track_json: Path, data: Dict) -> str:
        """Find the audio file for a track."""
        track_id = data.get("id", "")
        album_id = data.get("album_id", "")

        # Look for audio files with different format extensions
        formats = ["OGG_VORBIS_320", "OGG_VORBIS_160", "MP3_320", "FLAC"]

        for fmt in formats:
            audio_file = track_json.parent / f"track_{track_id}.{fmt}"
            if audio_file.exists():
                # Return relative URI
                ext = "ogg" if fmt.startswith("OGG") else fmt.split("_")[0].lower()
                return f"albums/album_{album_id}/track_{track_id}.{fmt}"

        # Fallback to first available format from files dict
        files = data.get("files", {})
        if files:
            for fmt in files.keys():
                audio_file = track_json.parent / f"track_{track_id}.{fmt}"
                if audio_file.exists():
                    return f"albums/album_{album_id}/track_{track_id}.{fmt}"

        return ""

    def _insert_image(self, image_id: str, uri: str, size: str, width: int, height: int):
        """Insert an image into the database, updating if new data has better dimensions."""
        if self.dry_run:
            print(f"  Would insert image: {image_id}")
            self.images_imported += 1
            return

        try:
            # First try to insert
            self.conn.execute(
                "INSERT OR IGNORE INTO images (id, uri, size, width, height) VALUES (?, ?, ?, ?, ?)",
                (image_id, uri, size, width, height),
            )
            # If image exists and new data has better dimensions, update it
            if width > 0 and height > 0:
                self.conn.execute(
                    "UPDATE images SET size = ?, width = ?, height = ? WHERE id = ? AND width * height < ? * ?",
                    (size, width, height, image_id, width, height),
                )
            self.images_imported += 1
        except sqlite3.Error as e:
            self.errors.append(f"Error inserting image {image_id}: {e}")

    def _insert_artist(self, data: Dict):
        """Insert an artist into the database."""
        artist_id = data.get("id", "")
        name = data.get("name", "")
        genres = json.dumps(data.get("genre", []))

        # Handle activity periods
        activity_periods = []
        for ap in data.get("activity_periods", []):
            if "Decade" in ap:
                activity_periods.append({"type": "decade", "decade": ap["Decade"]})
            elif "Timespan" in ap:
                ts = ap["Timespan"]
                activity_periods.append({
                    "type": "timespan",
                    "start_year": ts.get("start_year"),
                    "end_year": ts.get("end_year"),
                })

        activity_periods_json = json.dumps(activity_periods)

        if self.dry_run:
            print(f"  Would insert artist: {artist_id} - {name}")
            self.artists_imported += 1
            return

        try:
            self.conn.execute(
                "INSERT OR IGNORE INTO artists (id, name, genres, activity_periods) VALUES (?, ?, ?, ?)",
                (artist_id, name, genres, activity_periods_json),
            )
            self.artists_imported += 1
        except sqlite3.Error as e:
            self.errors.append(f"Error inserting artist {artist_id}: {e}")

    def _insert_artist_images(self, data: Dict):
        """Insert artist image relationships."""
        artist_id = data.get("id", "")

        # Use portrait_group if available, otherwise portraits
        images = data.get("portrait_group", []) or data.get("portraits", [])

        # Track the biggest image by pixel area
        biggest_image_id = None
        biggest_area = 0

        for idx, img in enumerate(images):
            image_id = img.get("id", "")
            if not image_id:
                continue

            width = img.get("width", 0)
            height = img.get("height", 0)
            area = width * height

            # Insert image first
            self._insert_image(
                image_id,
                f"images/{image_id}",
                img.get("size", "DEFAULT"),
                width,
                height,
            )

            # Track biggest image
            if area > biggest_area:
                biggest_area = area
                biggest_image_id = image_id

            if self.dry_run:
                continue

            try:
                self.conn.execute(
                    "INSERT OR IGNORE INTO artist_images (artist_id, image_id, image_type, position) VALUES (?, ?, ?, ?)",
                    (artist_id, image_id, "portrait", idx),
                )
            except sqlite3.Error as e:
                self.errors.append(f"Error linking artist {artist_id} to image {image_id}: {e}")

        # Set display_image_id to the biggest image
        if biggest_image_id and not self.dry_run:
            try:
                self.conn.execute(
                    "UPDATE artists SET display_image_id = ? WHERE id = ?",
                    (biggest_image_id, artist_id),
                )
            except sqlite3.Error as e:
                self.errors.append(f"Error setting display image for artist {artist_id}: {e}")

    def _insert_album(self, data: Dict):
        """Insert an album into the database."""
        album_id = data.get("id", "")
        name = data.get("name", "")
        album_type = data.get("album_type", "ALBUM")
        label = data.get("label")
        release_date = data.get("date")
        genres = json.dumps(data.get("genres", []))
        original_title = data.get("original_title")
        version_title = data.get("version_title") or None

        if self.dry_run:
            print(f"  Would insert album: {album_id} - {name}")
            self.albums_imported += 1
            return

        try:
            self.conn.execute(
                """INSERT OR IGNORE INTO albums
                   (id, name, album_type, label, release_date, genres, original_title, version_title)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
                (album_id, name, album_type, label, release_date, genres, original_title, version_title),
            )
            self.albums_imported += 1
        except sqlite3.Error as e:
            self.errors.append(f"Error inserting album {album_id}: {e}")

    def _insert_album_images(self, data: Dict):
        """Insert album image relationships."""
        album_id = data.get("id", "")

        # Use cover_group if available, otherwise covers
        images = data.get("cover_group", []) or data.get("covers", [])

        # Track the biggest image by pixel area
        biggest_image_id = None
        biggest_area = 0

        for idx, img in enumerate(images):
            image_id = img.get("id", "")
            if not image_id:
                continue

            width = img.get("width", 0)
            height = img.get("height", 0)
            area = width * height

            # Insert image first
            self._insert_image(
                image_id,
                f"images/{image_id}",
                img.get("size", "DEFAULT"),
                width,
                height,
            )

            # Track biggest image
            if area > biggest_area:
                biggest_area = area
                biggest_image_id = image_id

            if self.dry_run:
                continue

            try:
                self.conn.execute(
                    "INSERT OR IGNORE INTO album_images (album_id, image_id, image_type, position) VALUES (?, ?, ?, ?)",
                    (album_id, image_id, "cover", idx),
                )
            except sqlite3.Error as e:
                self.errors.append(f"Error linking album {album_id} to image {image_id}: {e}")

        # Set display_image_id to the biggest image
        if biggest_image_id and not self.dry_run:
            try:
                self.conn.execute(
                    "UPDATE albums SET display_image_id = ? WHERE id = ?",
                    (biggest_image_id, album_id),
                )
            except sqlite3.Error as e:
                self.errors.append(f"Error setting display image for album {album_id}: {e}")

    def _insert_album_artists(self, data: Dict):
        """Insert album-artist relationships."""
        album_id = data.get("id", "")
        artists_ids = data.get("artists_ids", [])

        for idx, artist_id in enumerate(artists_ids):
            if self.dry_run:
                continue

            try:
                self.conn.execute(
                    "INSERT OR IGNORE INTO album_artists (album_id, artist_id, position) VALUES (?, ?, ?)",
                    (album_id, artist_id, idx),
                )
                self.relations_imported += 1
            except sqlite3.Error as e:
                self.errors.append(f"Error linking album {album_id} to artist {artist_id}: {e}")

    def _insert_track(self, data: Dict, audio_uri: str):
        """Insert a track into the database."""
        track_id = data.get("id", "")
        name = data.get("name", "")
        album_id = data.get("album_id", "")
        disc_number = data.get("disc_number", 1)
        track_number = data.get("number", 1)
        duration_ms = data.get("duration", 0)
        duration_secs = duration_ms // 1000 if duration_ms else None
        is_explicit = data.get("is_explicit", False)
        tags = json.dumps(data.get("tags", []))
        has_lyrics = data.get("has_lyrics", False)
        languages = json.dumps(data.get("language_of_performance", []))
        original_title = data.get("original_title")
        version_title = data.get("version_title") or None

        # Determine format from audio_uri
        fmt = "MP3_320"  # default
        if audio_uri:
            if "OGG_VORBIS_320" in audio_uri:
                fmt = "OGG_VORBIS_320"
            elif "OGG_VORBIS_160" in audio_uri:
                fmt = "OGG_VORBIS_160"
            elif "FLAC" in audio_uri:
                fmt = "FLAC"

        if self.dry_run:
            print(f"  Would insert track: {track_id} - {name}")
            self.tracks_imported += 1
            return

        try:
            self.conn.execute(
                """INSERT OR IGNORE INTO tracks
                   (id, name, album_id, disc_number, track_number, duration_secs,
                    is_explicit, audio_uri, format, tags, has_lyrics, languages,
                    original_title, version_title)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (track_id, name, album_id, disc_number, track_number, duration_secs,
                 is_explicit, audio_uri, fmt, tags, has_lyrics, languages,
                 original_title, version_title),
            )
            self.tracks_imported += 1
        except sqlite3.Error as e:
            self.errors.append(f"Error inserting track {track_id}: {e}")

    def _insert_track_artists(self, data: Dict):
        """Insert track-artist relationships."""
        track_id = data.get("id", "")

        for idx, awr in enumerate(data.get("artists_with_role", [])):
            artist_id = awr.get("artist_id", "")
            role = awr.get("role", "ARTIST_ROLE_MAIN_ARTIST")

            # Map downloader role to catalog role
            role_map = {
                "ARTIST_ROLE_MAIN_ARTIST": "MAIN_ARTIST",
                "ARTIST_ROLE_FEATURED_ARTIST": "FEATURED_ARTIST",
                "ARTIST_ROLE_REMIXER": "REMIXER",
                "ARTIST_ROLE_COMPOSER": "COMPOSER",
                "ARTIST_ROLE_CONDUCTOR": "CONDUCTOR",
                "ARTIST_ROLE_ORCHESTRA": "ORCHESTRA",
                "ARTIST_ROLE_ACTOR": "ACTOR",
            }
            role = role_map.get(role, "UNKNOWN")

            if self.dry_run:
                continue

            try:
                self.conn.execute(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role, position) VALUES (?, ?, ?, ?)",
                    (track_id, artist_id, role, idx),
                )
                self.relations_imported += 1
            except sqlite3.Error as e:
                self.errors.append(f"Error linking track {track_id} to artist {artist_id}: {e}")

    def _insert_related_artists(self, data: Dict):
        """Insert related artist relationships."""
        artist_id = data.get("id", "")
        related = data.get("related", [])

        for related_id in related:
            if self.dry_run:
                continue

            try:
                self.conn.execute(
                    "INSERT OR IGNORE INTO related_artists (artist_id, related_artist_id) VALUES (?, ?)",
                    (artist_id, related_id),
                )
                self.relations_imported += 1
            except sqlite3.Error as e:
                # Ignore foreign key errors - related artist might not exist yet
                if "FOREIGN KEY" not in str(e):
                    self.errors.append(f"Error linking artist {artist_id} to related {related_id}: {e}")

    def print_summary(self):
        """Print import summary."""
        print()
        print("=" * 50)
        print("Import Summary")
        print("=" * 50)
        print(f"  Artists imported: {self.artists_imported}")
        print(f"  Albums imported:  {self.albums_imported}")
        print(f"  Tracks imported:  {self.tracks_imported}")
        print(f"  Images imported:  {self.images_imported}")
        print(f"  Relations:        {self.relations_imported}")
        print()

        if self.errors:
            print(f"Errors encountered: {len(self.errors)}")
            for error in self.errors[:10]:  # Show first 10 errors
                print(f"  - {error}")
            if len(self.errors) > 10:
                print(f"  ... and {len(self.errors) - 10} more errors")


def main():
    args = parse_args()

    if not args.catalog_path.exists():
        print(f"Error: Catalog path does not exist: {args.catalog_path}")
        sys.exit(1)

    importer = CatalogImporter(args.catalog_path, args.db_path, args.dry_run)
    importer.import_all()


if __name__ == "__main__":
    main()
