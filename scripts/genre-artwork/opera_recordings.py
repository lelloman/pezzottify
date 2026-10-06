#!/usr/bin/env python3
"""Materialize conservative Opera membership from enrichment, never artist tags.

Usage: opera_recordings.py catalog.db enrichment.db --output report.json [--apply]
Run after enrichment updates. --apply requires catalog schema v12 tables.
The report preserves previous rows for rollback. Unknown recordings stay out.
"""
import argparse
import json
import re
from pathlib import Path
import sqlite3


def opera_evidence(db):
    evidence = {}
    # Only explicit opera tags; 'opera transcription' is not sufficient.
    tags = ('opera', 'italian opera', 'french opera', 'german opera', 'russian opera', 'opera buffa', 'opera seria')
    for kind, entity_id, tag, confidence in db.execute("SELECT entity_type,entity_id,tag,confidence FROM entity_tags_v1 WHERE entity_type='track'"):
        if tag.strip().lower() in tags and (confidence or 0) >= 0.9:
            evidence[(kind, entity_id)] = f'{kind} genre tag: {tag} (confidence {confidence})'
    # A source work explicitly typed as opera, including its linked parts.
    rows = db.execute("""WITH RECURSIVE opera_work(id) AS (
        SELECT work_id FROM work_source_evidence_v1
        WHERE lower(COALESCE(json_extract(evidence_json,'$.type.name'), json_extract(evidence_json,'$.type'),''))='opera'
        UNION SELECT id FROM works_v1 WHERE lower(kind)='opera'
        UNION SELECT r.target_work_id FROM work_relationships_v1 r JOIN opera_work o ON o.id=r.source_work_id
              WHERE r.relationship_type='parts'
    ) SELECT r.track_id FROM work_resolutions_v1 r JOIN opera_work o ON o.id=r.work_id WHERE r.status='linked'""")
    for (track_id,) in rows:
        evidence[('track', track_id)] = 'linked recording of an opera work or its part'
    # Contradictory explicit forms beat even album-level positive evidence.
    excluded = {track_id for track_id, form in db.execute('SELECT track_id,form FROM track_enrichment_v1')
                if any(word in (form or '').lower() for word in ('concerto', 'symphony', 'sonata'))}
    return evidence, excluded


def run(catalog, enrichment, apply=False):
    source=sqlite3.connect(Path(enrichment).resolve().as_uri()+'?mode=ro',uri=True)
    try:
        evidence, excluded=opera_evidence(source)
    finally:
        source.close()
    db=sqlite3.connect(Path(catalog).resolve().as_uri()+('?mode=rw' if apply else '?mode=ro'),uri=True,timeout=10)
    try:
        selected={}
        for (kind, entity_id), reason in evidence.items():
            query = "SELECT t.id,t.name FROM tracks t WHERE t.id=? AND t.track_available=1" if kind=='track' else "SELECT t.id,t.name FROM tracks t JOIN albums a ON a.rowid=t.album_rowid WHERE a.id=? AND t.track_available=1"
            for track_id, name in db.execute(query,(entity_id,)):
                if track_id not in excluded and not re.search(r'\b(concerto|symphony|sonata)\b', name, re.IGNORECASE):
                    selected.setdefault(track_id, dict(track_id=track_id,name=name,evidence=reason))
        exists=db.execute("SELECT 1 FROM sqlite_master WHERE name='genre_recordings'").fetchone()
        previous=db.execute("SELECT genre,track_id,evidence FROM genre_recordings WHERE genre='opera'").fetchall() if exists else []
        if apply:
            if not exists:
                raise RuntimeError('Apply catalog migration v12 before writing membership')
            with db:
                db.execute("INSERT OR IGNORE INTO genre_recording_policy VALUES ('opera')")
                db.execute("DELETE FROM genre_recordings WHERE genre='opera'")
                db.executemany("INSERT INTO genre_recordings VALUES ('opera',?,?)",[(c['track_id'],c['evidence']) for c in selected.values()])
        return dict(applied=apply,count=len(selected),previous=previous,selections=sorted(selected.values(),key=lambda c:c['track_id']))
    finally:
        db.close()


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('catalog');parser.add_argument('enrichment')
    parser.add_argument('--apply',action='store_true');parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    with args.output.open('x') as out:
        report=run(args.catalog,args.enrichment,args.apply)
        json.dump(report,out,indent=2)
    print(json.dumps({k:v for k,v in report.items() if k not in ('selections','previous')}))
