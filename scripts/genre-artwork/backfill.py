#!/usr/bin/env python3
"""Persist deterministic genre artwork selections. Dry-run by default.

python3 scripts/genre-artwork/backfill.py /path/catalog.db --output plan.json
python3 scripts/genre-artwork/backfill.py /path/catalog.db --apply --output applied.json

Apply requires the genre_artwork table from catalog migration v11.
The additive DDL may also be preseeded before deployment; never change user_version
outside the server migration runner. Existing choices are never overwritten.
Candidate image URLs must exist in catalog metadata; the existing image endpoint
validates/downloads their bytes on demand. This job does not contact image CDNs.
Run again after catalog imports to fill new genres. No work runs per page request.
"""
import argparse
import json
from pathlib import Path
import sqlite3
import time


def select_artwork(db):
    # Count the available subset once, not once per genre or candidate artist.
    db.execute('''CREATE TEMP TABLE available_artist_tracks AS
        SELECT ta.artist_rowid, COUNT(DISTINCT t.rowid) AS available_tracks
        FROM tracks t JOIN track_artists ta ON ta.track_rowid = t.rowid
        WHERE t.track_available = 1 GROUP BY ta.artist_rowid''')
    db.execute('CREATE UNIQUE INDEX temp.available_artist_idx ON available_artist_tracks(artist_rowid)')
    rows = db.execute('''SELECT ag.genre, a.id, a.name,
            COALESCE(c.available_tracks, 0) AS available_tracks, a.popularity
        FROM artist_genres ag JOIN artists a ON a.rowid = ag.artist_rowid
        LEFT JOIN available_artist_tracks c ON c.artist_rowid = a.rowid
        WHERE EXISTS (SELECT 1 FROM artist_images i
            WHERE i.artist_rowid = a.rowid AND TRIM(i.url) <> '')
        ORDER BY ag.genre, available_tracks DESC, a.popularity DESC, a.id''')
    choices = {}
    for genre, entity_id, name, count, popularity in rows:
        choices.setdefault(genre, dict(genre=genre, entity_type='artist',
            entity_id=entity_id, name=name, available_tracks=count))
    all_genres = [r[0] for r in db.execute('SELECT DISTINCT genre FROM artist_genres ORDER BY genre')]
    missing = []
    for genre in all_genres:
        if genre in choices:
            continue
        row = db.execute('''SELECT al.id, al.name, COUNT(DISTINCT t.rowid) AS available_tracks
            FROM artist_genres ag JOIN track_artists ta ON ta.artist_rowid = ag.artist_rowid
            JOIN tracks t ON t.rowid = ta.track_rowid
            JOIN albums al ON al.rowid = t.album_rowid
            WHERE ag.genre = ? AND t.track_available = 1
              AND EXISTS (SELECT 1 FROM album_images i WHERE i.album_rowid = al.rowid AND TRIM(i.url) <> '')
            GROUP BY al.rowid ORDER BY available_tracks DESC, al.popularity DESC, al.id LIMIT 1''', (genre,)).fetchone()
        if row:
            choices[genre] = dict(genre=genre, entity_type='album', entity_id=row[0], name=row[1], available_tracks=row[2])
        else:
            missing.append(genre)
    return choices, missing


def backfill(path, apply=False):
    db = sqlite3.connect(Path(path).resolve().as_uri() + ('?mode=rw' if apply else '?mode=ro'), uri=True, timeout=10)
    try:
        exists = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='genre_artwork'").fetchone()
        if apply and not exists:
            raise RuntimeError('Deploy catalog migration v11 before applying; dry-run can run on the old catalog.')
        # A consistent read snapshot; release it before acquiring the short write transaction.
        db.execute('BEGIN')
        existing = {r[0] for r in db.execute('SELECT genre FROM genre_artwork')} if exists else set()
        choices, missing = select_artwork(db)
        pending = [choice for genre, choice in sorted(choices.items()) if genre not in existing]
        db.commit()
        inserted = 0
        if apply:
            with db:
                for c in pending:
                    inserted += db.execute('''INSERT OR IGNORE INTO genre_artwork
                        (genre, entity_type, entity_id, selected_at) VALUES (?, ?, ?, ?)''',
                        (c['genre'], c['entity_type'], c['entity_id'], int(time.time()))).rowcount
        return dict(mode='applied' if apply else 'dry-run', existing=len(existing),
                    candidates=len(choices), pending=len(pending), inserted=inserted,
                    missing=missing, selections=pending)
    finally:
        db.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('catalog')
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    # Reserve the report before any database writes; never overwrite prior evidence.
    with args.output.open('x') as out:
        report = backfill(args.catalog, args.apply)
        json.dump(report, out, indent=2)
    print(json.dumps({k: v for k, v in report.items() if k != 'selections'}))


if __name__ == '__main__':
    main()
