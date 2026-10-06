#!/usr/bin/env python3
"""Persist deterministic genre artwork selections. Dry-run by default.

python3 scripts/genre-artwork/backfill.py /path/catalog.db --output plan.json
python3 scripts/genre-artwork/backfill.py /path/catalog.db --apply --output applied.json

Apply requires the genre_artwork table from catalog migration v11.
The additive DDL may also be preseeded before deployment; never change user_version
outside the server migration runner. Existing choices are preserved unless --reselect is explicitly requested.
Selections are unique by entity and image URL. Use --overrides for editorial choices.
Candidate image URLs must exist in catalog metadata; the existing image endpoint
validates/downloads their bytes on demand. This job does not contact image CDNs.
Run again after catalog imports to fill new genres. No work runs per page request.
"""
import argparse
import json
from pathlib import Path
import sqlite3
import time


def select_artwork(db, existing=None, overrides=None):
    existing = existing or {}
    overrides = overrides or {}
    db.execute("""CREATE TEMP TABLE available_artist_tracks AS
        SELECT ta.artist_rowid, COUNT(DISTINCT t.rowid) AS available_tracks
        FROM tracks t JOIN track_artists ta ON ta.track_rowid = t.rowid
        WHERE t.track_available = 1 GROUP BY ta.artist_rowid""")
    db.execute('CREATE UNIQUE INDEX temp.available_artist_idx ON available_artist_tracks(artist_rowid)')
    images = {}
    def image_urls(key):
        if key not in images:
            kind, entity_id = key
            if kind not in ('artist', 'album'):
                images[key] = set()
            else:
                images[key] = {r[0] for r in db.execute(
                    f"SELECT i.url FROM {kind}_images i JOIN {kind}s e ON e.rowid=i.{kind}_rowid WHERE e.id=? AND TRIM(i.url) <> ''", (entity_id,))}
        return images[key]
    candidates = {}
    rows = db.execute("""SELECT ag.genre, a.id, a.name, COALESCE(c.available_tracks,0), a.popularity
        FROM artist_genres ag JOIN artists a ON a.rowid=ag.artist_rowid
        LEFT JOIN available_artist_tracks c ON c.artist_rowid=a.rowid
        WHERE EXISTS (SELECT 1 FROM artist_images i WHERE i.artist_rowid=a.rowid AND TRIM(i.url) <> '')""")
    tags = {}
    for genre, entity_id, name, count, popularity in rows:
        tags.setdefault(entity_id, set()).add(genre)
        candidates.setdefault(genre, []).append(dict(genre=genre, entity_type='artist',
            entity_id=entity_id, name=name, available_tracks=count, popularity=popularity))
    all_genres = [r[0] for r in db.execute('SELECT DISTINCT genre FROM artist_genres ORDER BY genre')]
    used_entities = {(c['entity_type'], c['entity_id']) for c in existing.values()}
    used_urls = set().union(*(image_urls(key) for key in used_entities)) if used_entities else set()
    choices = {}

    def reserve(candidate):
        key = (candidate['entity_type'], candidate['entity_id'])
        urls = image_urls(key)
        if not urls or key in used_entities or urls & used_urls:
            return False
        used_entities.add(key)
        used_urls.update(urls)
        choices[candidate['genre']] = candidate
        return True

    def rank(c):
        # Availability first; track count is capped so huge archives cannot dominate.
        # More genre tags reduce specificity. This is a heuristic, not semantic truth.
        score = c['popularity'] - 3 * (len(tags[c['entity_id']]) - 1) + min(c['available_tracks'], 20) / 4
        return (-bool(c['available_tracks']), -score, c['entity_id'])

    for genre, entity_id in sorted(overrides.items()):
        if genre in existing:
            if existing[genre]['entity_id'] != entity_id:
                raise ValueError('Use --reselect to change an existing override')
            continue
        candidate = next((c for c in candidates.get(genre, []) if c['entity_id'] == entity_id), None)
        if not candidate or not reserve(candidate):
            raise ValueError(f'Invalid or duplicate artwork override: {genre} / {entity_id}')
    missing = []
    # Give narrowly populated genres first choice, leaving broad genres alternatives.
    for genre in sorted(all_genres, key=lambda g: (len(candidates.get(g, [])), g)):
        if genre in existing or genre in choices:
            continue
        if any(reserve(c) for c in sorted(candidates.get(genre, []), key=rank)):
            continue
        rows = db.execute("""SELECT al.id, al.name, COUNT(DISTINCT t.rowid) AS available_tracks
            FROM artist_genres ag JOIN track_artists ta ON ta.artist_rowid=ag.artist_rowid
            JOIN tracks t ON t.rowid=ta.track_rowid JOIN albums al ON al.rowid=t.album_rowid
            WHERE ag.genre=? AND t.track_available=1
            GROUP BY al.rowid ORDER BY al.popularity DESC, available_tracks DESC, al.id""", (genre,))
        if not any(reserve(dict(genre=genre, entity_type='album', entity_id=row[0], name=row[1], available_tracks=row[2])) for row in rows):
            missing.append(genre)
    return choices, sorted(missing)


def backfill(path, apply=False, reselect=False, overrides=None):
    db = sqlite3.connect(Path(path).resolve().as_uri() + ('?mode=rw' if apply else '?mode=ro'), uri=True, timeout=10)
    try:
        exists = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='genre_artwork'").fetchone()
        if apply and not exists:
            raise RuntimeError('Deploy catalog migration v11 before applying; dry-run can run on the old catalog.')
        db.execute('BEGIN')
        previous = [dict(zip(('genre', 'entity_type', 'entity_id', 'selected_at'), r))
                    for r in db.execute('SELECT genre, entity_type, entity_id, selected_at FROM genre_artwork ORDER BY genre')] if exists else []
        preserved = {} if reselect else {c['genre']: c for c in previous}
        choices, missing = select_artwork(db, preserved, overrides)
        pending = [choice for genre, choice in sorted(choices.items())]
        db.commit()
        inserted = 0
        if apply:
            db.execute('BEGIN IMMEDIATE')
            try:
                current = [dict(zip(('genre', 'entity_type', 'entity_id', 'selected_at'), r))
                           for r in db.execute('SELECT genre, entity_type, entity_id, selected_at FROM genre_artwork ORDER BY genre')]
                if current != previous:
                    raise RuntimeError('Artwork choices changed during planning; rerun to avoid overwriting them.')
                if reselect:
                    db.execute('DELETE FROM genre_artwork')
                for c in pending:
                    inserted += db.execute("""INSERT INTO genre_artwork
                        (genre, entity_type, entity_id, selected_at) VALUES (?, ?, ?, ?)""",
                        (c['genre'], c['entity_type'], c['entity_id'], int(time.time()))).rowcount
                db.commit()
            except Exception:
                db.rollback()
                raise
        return dict(mode='applied' if apply else 'dry-run', reselect=reselect, existing=len(previous),
                    candidates=len(choices), pending=len(pending), inserted=inserted,
                    missing=missing, selections=pending, previous=previous)
    finally:
        db.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('catalog')
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reselect', action='store_true', help='Replace all prior choices; report includes rollback rows')
    parser.add_argument('--overrides', type=Path, help='JSON object mapping genre names to artist IDs')
    args = parser.parse_args()
    # Reserve the report before any database writes; never overwrite prior evidence.
    with args.output.open('x') as out:
        report = backfill(args.catalog, args.apply, args.reselect,
                          json.loads(args.overrides.read_text()) if args.overrides else None)
        json.dump(report, out, indent=2)
    print(json.dumps({k: v for k, v in report.items() if k not in ('selections', 'previous')}))


if __name__ == '__main__':
    main()
