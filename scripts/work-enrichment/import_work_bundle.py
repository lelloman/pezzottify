"""Offline, transactional Work graph import. Dry-run defaults to a SQLite backup copy."""
import argparse,collections,hashlib,json,sqlite3,tempfile,time,uuid
from pathlib import Path

def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(4*1024*1024),b''):h.update(chunk)
 return h.hexdigest()
def load_bundle(root):
 manifest=json.load(open(root/'production-manifest.json'))
 for name,sha in manifest['files'].items():
  if Path(name).name!=name:raise ValueError('Bundle paths must be simple filenames')
  if digest(root/name)!=sha:raise ValueError('Bundle checksum mismatch: '+name)
 works=[json.loads(l) for l in open(root/'works.jsonl')];rels=[json.loads(l) for l in open(root/'relationships.jsonl')];links=json.load(open(root/'staged-link-batch.json'))['links'];tracks={r['track_id']:r for r in map(json.loads,open(root/'track-evidence.jsonl'))}
 ids={w['mbid'] for w in works};assert len(ids)==len(works)
 for wid in ids:uuid.UUID(wid)
 assert len({r['relationship_numeric_id'] for r in rels})==len(rels)
 for r in rels:assert r['source_work_mbid'] in ids and r['target_work_mbid'] in ids;uuid.UUID(r['type_mbid'])
 assert len({r['track_id'] for r in links})==len(links)
 for l in links:
  assert l['work']['musicbrainz_id'] in ids and not l['review_flags'] and l['album_match_basis']=='fingerprint_rules' and l['import_creator_claims'] is False
  assert l['track_id'] in tracks and tracks[l['track_id']]['work_mbid']==l['work']['musicbrainz_id']
 return manifest,works,rels,links,tracks

def apply(db,catalog,root,manifest,works,rels,links,tracks):
 now=int(time.time());counts=collections.Counter();skips=[];source=manifest['source_snapshot'];batch_id=digest(root/'production-manifest.json')
 db.execute('PRAGMA foreign_keys=ON');db.execute('PRAGMA busy_timeout=10000')
 # DDL and data form a single transaction. executescript includes BEGIN explicitly.
 db.executescript('BEGIN IMMEDIATE;\n'+(root/'work_graph.sql').read_text())
 try:
  if db.execute('SELECT 1 FROM work_import_batches_v1 WHERE id=?',(batch_id,)).fetchone():
   db.rollback();return {'status':'already_imported','batch_id':batch_id}
  identities={external:wid for external,wid in db.execute("SELECT external_id,work_id FROM work_external_ids_v1 WHERE provider='musicbrainz'")}
  for w in works:
   mbid=w['mbid'];wid=identities.get(mbid)
   if wid is None:
    wid=str(uuid.uuid5(uuid.NAMESPACE_URL,'https://musicbrainz.org/work/'+mbid))
    names=sorted({r['artist']['name'] for r in w['artist_relations'] if r['type'] in ('composer','writer','lyricist','librettist')})
    db.execute('INSERT INTO works_v1(id,title,creators_json,catalog_number,kind,identity_key,created_at) VALUES(?,?,?,?,?,?,?)',(wid,w['title'],json.dumps(names,ensure_ascii=False),None,'song' if w['type']=='Song' else 'composition','musicbrainz:'+mbid,now))
    db.execute("INSERT INTO work_external_ids_v1 VALUES('musicbrainz',?,?)",(mbid,wid));identities[mbid]=wid;counts['works_added']+=1
   else:counts['existing_works_preserved']+=1
   db.execute('INSERT INTO work_source_evidence_v1 VALUES(?,?,?,?,?,?) ON CONFLICT(provider,external_id) DO UPDATE SET source_snapshot=excluded.source_snapshot,evidence_json=excluded.evidence_json,imported_at=excluded.imported_at',('musicbrainz',mbid,wid,source,json.dumps(w,ensure_ascii=False),now))
  for r in rels:
   rid=str(r['relationship_numeric_id']);old=db.execute("SELECT source_work_id,target_work_id,relationship_type_id FROM work_relationships_v1 WHERE provider='musicbrainz' AND external_id=?",(rid,)).fetchone()
   identity=(identities[r['source_work_mbid']],identities[r['target_work_mbid']],r['type_mbid'])
   if old and tuple(old)!=identity:raise ValueError('Existing relationship identity conflict: '+rid)
   db.execute('INSERT INTO work_relationships_v1 VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(provider,external_id) DO UPDATE SET ordering=excluded.ordering,evidence_json=excluded.evidence_json,source_snapshot=excluded.source_snapshot,imported_at=excluded.imported_at',('musicbrainz',rid,identity[0],identity[1],r['type'],identity[2],r['ordering'],json.dumps(r,ensure_ascii=False),source,now));counts['relationships_upserted']+=1
  for link in links:
   tid=link['track_id'];snapshot=tracks[tid];wmbid=link['work']['musicbrainz_id'];wid=identities[wmbid]
   current=catalog.execute('SELECT t.track_available,t.audio_uri,t.duration_ms,a.id FROM tracks t JOIN albums a ON a.rowid=t.album_rowid WHERE t.id=?',(tid,)).fetchone()
   reason=None
   if current is None or current[0]!=1 or current[1] is None:reason='currently_unavailable_or_missing'
   elif current[2]!=snapshot['catalog_duration_ms'] or current[3]!=snapshot['album_id']:reason='catalog_changed_since_fingerprint'
   existing=db.execute('SELECT work_id FROM work_resolutions_v1 WHERE track_id=?',(tid,)).fetchone()
   if existing and existing[0]:
    if existing[0]==wid:counts['existing_links_preserved']+=1;continue
    reason='existing_link_conflict'
   if reason:counts[reason]+=1;skips.append({'track_id':tid,'reason':reason});continue
   evidence={'prompt_version':'offline-album-work-import-v1','selected_source':{'musicbrainz_id':wmbid,'url':'https://musicbrainz.org/work/'+wmbid},'source_snapshot':source,'batch_id':batch_id,'album_id':link['album_id'],'source_recording_candidates':link['source_recording_candidates'],'album_match_basis':link['album_match_basis'],'creator_claims_imported_to_catalog':False}
   db.execute("INSERT INTO work_resolutions_v1(track_id,work_id,status,reason,evidence_json,evaluated_at,enriched_at,last_verified_at,source_status) VALUES(?,?,'linked',?,?,?,?,?,'musicbrainz_supported_v1') ON CONFLICT(track_id) DO UPDATE SET work_id=excluded.work_id,status=excluded.status,reason=excluded.reason,evidence_json=excluded.evidence_json,evaluated_at=excluded.evaluated_at,enriched_at=excluded.enriched_at,last_verified_at=excluded.last_verified_at,source_status=excluded.source_status WHERE work_resolutions_v1.work_id IS NULL",(tid,wid,'Offline album fingerprint and unanimous recording-to-Work evidence',json.dumps(evidence),now,now,now));counts['track_links_added']+=1
  assert not db.execute('PRAGMA foreign_key_check').fetchall()
  report={'status':'imported','batch_id':batch_id,'counts':dict(counts),'skipped':skips,'input':{'works':len(works),'relationships':len(rels),'track_links':len(links)},'catalog_credits_modified':False}
  db.execute('INSERT INTO work_import_batches_v1 VALUES(?,?,?,?)',(batch_id,now,json.dumps(manifest),json.dumps(report)));db.commit();return report
 except BaseException:db.rollback();raise

def main():
 p=argparse.ArgumentParser();p.add_argument('--bundle',type=Path,required=True);p.add_argument('--db',type=Path,required=True);p.add_argument('--catalog',type=Path,required=True);p.add_argument('--report',type=Path,required=True);p.add_argument('--apply',action='store_true');p.add_argument('--backup',type=Path);a=p.parse_args()
 data=load_bundle(a.bundle);catalog=sqlite3.connect(f'file:{a.catalog}?mode=ro',uri=True);catalog.execute('BEGIN')
 started=time.time()
 with tempfile.TemporaryDirectory(prefix='work-import-') as temp:
  if a.apply:
   if not a.backup:raise ValueError('--apply requires a backup path')
   if a.backup.exists():raise ValueError('Backup path exists; refusing overwrite')
   db=sqlite3.connect(a.db);backup=sqlite3.connect(a.backup);db.backup(backup);assert backup.execute('PRAGMA integrity_check').fetchone()[0]=='ok';backup.close()
  else:
   src=sqlite3.connect(f'file:{a.db}?mode=ro',uri=True);db=sqlite3.connect(Path(temp)/'dry-run.sqlite');src.backup(db);src.close()
  report=apply(db,catalog,a.bundle,*data);assert db.execute('PRAGMA integrity_check').fetchone()[0]=='ok';db.close()
  report.update(mode='apply' if a.apply else 'dry_run',seconds=round(time.time()-started,2),backup=str(a.backup) if a.apply else None)
  a.report.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='skipped'},indent=2))
 catalog.close()
if __name__=='__main__':main()
