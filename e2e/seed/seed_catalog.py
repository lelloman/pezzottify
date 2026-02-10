"""Creates a seeded catalog.db for E2E tests.

Schema matches catalog-server/src/catalog_store/schema.rs at version 4.
Data mirrors catalog-server/tests/common/fixtures.rs.
"""

import sqlite3
import sys

# Constants matching catalog-server/tests/common/constants.rs
ARTIST_1_ID = "test_artist_001"
ARTIST_2_ID = "test_artist_002"
ALBUM_1_ID = "test_album_001"
ALBUM_2_ID = "test_album_002"
TRACK_1_ID = "test_track_001"
TRACK_2_ID = "test_track_002"
TRACK_3_ID = "test_track_003"
TRACK_4_ID = "test_track_004"
TRACK_5_ID = "test_track_005"

ARTIST_1_NAME = "The Test Band"
ARTIST_2_NAME = "Jazz Ensemble"
ALBUM_1_TITLE = "First Album"
ALBUM_2_TITLE = "Jazz Collection"
TRACK_1_TITLE = "Opening Track"
TRACK_2_TITLE = "Middle Track"
TRACK_3_TITLE = "Closing Track"
TRACK_4_TITLE = "Smooth Jazz"
TRACK_5_TITLE = "Upbeat Jazz"

# BASE_DB_VERSION from sqlite_persistence/versioned_schema.rs
BASE_DB_VERSION = 99999
SCHEMA_VERSION = 4


def create_schema(conn: sqlite3.Connection) -> None:
    """Create catalog schema at version 4 (latest)."""
    conn.executescript("""
        CREATE TABLE artists (
            rowid INTEGER PRIMARY KEY,
            id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            followers_total INTEGER NOT NULL,
            popularity INTEGER NOT NULL,
            artist_available INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_artists_id ON artists(id);
        CREATE INDEX idx_artists_available ON artists(artist_available);

        CREATE TABLE albums (
            rowid INTEGER PRIMARY KEY,
            id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            album_type TEXT NOT NULL,
            external_id_upc TEXT,
            external_id_amgid TEXT,
            label TEXT NOT NULL,
            popularity INTEGER NOT NULL,
            release_date TEXT NOT NULL,
            release_date_precision TEXT NOT NULL,
            album_availability TEXT NOT NULL DEFAULT 'missing',
            track_count INTEGER,
            total_duration_ms INTEGER
        );
        CREATE INDEX idx_albums_id ON albums(id);
        CREATE INDEX idx_albums_upc ON albums(external_id_upc);
        CREATE INDEX idx_albums_availability ON albums(album_availability);
        CREATE INDEX idx_album_fingerprint ON albums(track_count, total_duration_ms);

        CREATE TABLE tracks (
            rowid INTEGER PRIMARY KEY,
            id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            album_rowid INTEGER NOT NULL,
            track_number INTEGER NOT NULL,
            external_id_isrc TEXT,
            popularity INTEGER NOT NULL,
            disc_number INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL,
            explicit INTEGER NOT NULL,
            language TEXT,
            audio_uri TEXT,
            track_available INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_tracks_id ON tracks(id);
        CREATE INDEX idx_tracks_album ON tracks(album_rowid);
        CREATE INDEX idx_tracks_isrc ON tracks(external_id_isrc);
        CREATE INDEX idx_tracks_available ON tracks(track_available);

        CREATE TABLE track_artists (
            track_rowid INTEGER NOT NULL,
            artist_rowid INTEGER NOT NULL,
            role INTEGER
        );
        CREATE INDEX idx_track_artists_track ON track_artists(track_rowid);
        CREATE INDEX idx_track_artists_artist ON track_artists(artist_rowid);

        CREATE TABLE artist_albums (
            artist_rowid INTEGER NOT NULL,
            album_rowid INTEGER NOT NULL,
            is_appears_on INTEGER NOT NULL,
            is_implicit_appears_on INTEGER NOT NULL,
            index_in_album INTEGER,
            UNIQUE(artist_rowid, album_rowid, is_appears_on)
        );
        CREATE INDEX idx_artist_albums_artist ON artist_albums(artist_rowid);
        CREATE INDEX idx_artist_albums_album ON artist_albums(album_rowid);
        CREATE UNIQUE INDEX idx_artist_albums_unique ON artist_albums(artist_rowid, album_rowid, is_appears_on);

        CREATE TABLE artist_genres (
            artist_rowid INTEGER NOT NULL,
            genre TEXT NOT NULL
        );
        CREATE INDEX idx_artist_genres_artist ON artist_genres(artist_rowid);

        CREATE TABLE album_images (
            album_rowid INTEGER NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            url TEXT NOT NULL
        );
        CREATE INDEX idx_album_images_album ON album_images(album_rowid);

        CREATE TABLE artist_images (
            artist_rowid INTEGER NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            url TEXT NOT NULL
        );
        CREATE INDEX idx_artist_images_artist ON artist_images(artist_rowid);
    """)


def seed_data(conn: sqlite3.Connection) -> None:
    """Insert test catalog data matching fixtures.rs."""
    # Insert artists
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity, artist_available) VALUES (?, ?, 0, 50, 1)",
        (ARTIST_1_ID, ARTIST_1_NAME),
    )
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity, artist_available) VALUES (?, ?, 0, 50, 1)",
        (ARTIST_2_ID, ARTIST_2_NAME),
    )

    # Get artist rowids
    artist1_rowid = conn.execute(
        "SELECT rowid FROM artists WHERE id = ?", (ARTIST_1_ID,)
    ).fetchone()[0]
    artist2_rowid = conn.execute(
        "SELECT rowid FROM artists WHERE id = ?", (ARTIST_2_ID,)
    ).fetchone()[0]

    # Insert albums
    conn.execute(
        "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability) "
        "VALUES (?, ?, 'album', '', 50, '2023', 'year', 'complete')",
        (ALBUM_1_ID, ALBUM_1_TITLE),
    )
    conn.execute(
        "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability) "
        "VALUES (?, ?, 'album', '', 50, '2023', 'year', 'complete')",
        (ALBUM_2_ID, ALBUM_2_TITLE),
    )

    # Get album rowids
    album1_rowid = conn.execute(
        "SELECT rowid FROM albums WHERE id = ?", (ALBUM_1_ID,)
    ).fetchone()[0]
    album2_rowid = conn.execute(
        "SELECT rowid FROM albums WHERE id = ?", (ALBUM_2_ID,)
    ).fetchone()[0]

    # Link artists to albums
    conn.execute(
        "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album) "
        "VALUES (?, ?, 0, 0, 0)",
        (artist1_rowid, album1_rowid),
    )
    conn.execute(
        "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album) "
        "VALUES (?, ?, 0, 0, 0)",
        (artist2_rowid, album2_rowid),
    )

    # Insert tracks for album 1
    tracks_album1 = [
        (TRACK_1_ID, TRACK_1_TITLE, 240000),
        (TRACK_2_ID, TRACK_2_TITLE, 180000),
        (TRACK_3_ID, TRACK_3_TITLE, 210000),
    ]
    for i, (track_id, name, duration_ms) in enumerate(tracks_album1):
        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, disc_number, track_number, duration_ms, explicit, popularity, audio_uri, track_available) "
            "VALUES (?, ?, ?, 1, ?, ?, 0, 50, ?, 1)",
            (track_id, name, album1_rowid, i + 1, duration_ms, f"audio/{track_id}.ogg"),
        )
        track_rowid = conn.execute(
            "SELECT rowid FROM tracks WHERE id = ?", (track_id,)
        ).fetchone()[0]
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?, ?, 0)",
            (track_rowid, artist1_rowid),
        )

    # Insert tracks for album 2
    tracks_album2 = [
        (TRACK_4_ID, TRACK_4_TITLE, 200000),
        (TRACK_5_ID, TRACK_5_TITLE, 160000),
    ]
    for i, (track_id, name, duration_ms) in enumerate(tracks_album2):
        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, disc_number, track_number, duration_ms, explicit, popularity, audio_uri, track_available) "
            "VALUES (?, ?, ?, 1, ?, ?, 0, 50, ?, 1)",
            (track_id, name, album2_rowid, i + 1, duration_ms, f"audio/{track_id}.ogg"),
        )
        track_rowid = conn.execute(
            "SELECT rowid FROM tracks WHERE id = ?", (track_id,)
        ).fetchone()[0]
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?, ?, 0)",
            (track_rowid, artist2_rowid),
        )

    # Update album fingerprint columns (track_count, total_duration_ms)
    conn.execute(
        "UPDATE albums SET "
        "track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_rowid = albums.rowid), "
        "total_duration_ms = (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE tracks.album_rowid = albums.rowid)"
    )

    # Insert album images
    conn.execute(
        "INSERT INTO album_images (album_rowid, url, width, height) VALUES (?, ?, 300, 300)",
        (album1_rowid, "https://example.com/image-1.jpg"),
    )
    conn.execute(
        "INSERT INTO album_images (album_rowid, url, width, height) VALUES (?, ?, 300, 300)",
        (album2_rowid, "https://example.com/image-2.jpg"),
    )

    # Set PRAGMA user_version = BASE_DB_VERSION + SCHEMA_VERSION
    conn.execute(f"PRAGMA user_version = {BASE_DB_VERSION + SCHEMA_VERSION}")


def main() -> None:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <output_path>", file=sys.stderr)
        sys.exit(1)

    db_path = sys.argv[1]
    conn = sqlite3.connect(db_path)
    try:
        create_schema(conn)
        seed_data(conn)
        conn.commit()
        print(f"Created catalog database at {db_path}")
    finally:
        conn.close()


if __name__ == "__main__":
    main()
