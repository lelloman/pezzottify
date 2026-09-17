"""Add a complete connected MusicBrainz Work graph to a local album extraction.
No production writes. Related Works never imply extra catalog track links.
"""
import argparse,collections,hashlib,json,re,sqlite3,time
from pathlib import Path
ESC=re.compile(r'\\([0-7]{1,3}|x[0-9a-fA-F]{1,2}|.)')
def decode(s):
 if s==r'\N':return None
 def replace(m):
  c=m[1]
  if c[0] in '01234567':return chr(int(c,8))
  if c.startswith('x') and len(c)>1:return chr(int(c[1:],16))
  return {'b':'\b','f':'\f','n':'\n','r':'\r','t':'\t','v':'\v','\\':'\\'}.get(c,c)
 return ESC.sub(replace,s)
def scan(raw,name,columns,key=None,allowed=None):
 print('Scanning '+name,flush=True)
 for n,line in enumerate((raw/name).open(),1):
  row=line.rstrip('\n').split('\t')
  if len(row)!=columns:raise ValueError(f'{name}:{n}: column count {len(row)} != {columns}')
  if allowed is None or row[key] in allowed:yield [decode(v) for v in row]
def closure(db,seeds):
 """Both-direction reachability; preserve the original edge direction separately."""
 db.executescript('CREATE TEMP TABLE seen(id INTEGER PRIMARY KEY); CREATE TEMP TABLE frontier(id INTEGER PRIMARY KEY); CREATE TEMP TABLE next_frontier(id INTEGER PRIMARY KEY);')
 db.executemany('INSERT INTO seen VALUES(?)',[(int(i),) for i in seeds]);db.execute('INSERT INTO frontier SELECT id FROM seen')
 while db.execute('SELECT EXISTS(SELECT 1 FROM frontier)').fetchone()[0]:
  db.execute('DELETE FROM next_frontier')
  for a,b in [('source','target'),('target','source')]:
   db.execute(f'INSERT OR IGNORE INTO next_frontier SELECT e.{b} FROM edges e JOIN frontier f ON e.{a}=f.id LEFT JOIN seen s ON s.id=e.{b} WHERE s.id IS NULL')
  db.execute('INSERT OR IGNORE INTO seen SELECT id FROM next_frontier');db.execute('DELETE FROM frontier');db.execute('INSERT INTO frontier SELECT id FROM next_frontier')
 return {str(r[0]) for r in db.execute('SELECT id FROM seen')}
def main():
 p=argparse.ArgumentParser();p.add_argument('--workspace',type=Path,default=Path('.local-work-enrichment'));a=p.parse_args();root=a.workspace.resolve();base=root/'album-data-extraction';raw=root/'available-catalog/raw';out=base/'work-graph';out.mkdir(exist_ok=True)
 if (out/'COMPLETE').exists():raise RuntimeError('Work graph already completed; refusing to replace it')
 assert (raw/'SCHEMA_SEQUENCE').read_text().strip()=='31'
 start=time.time();seeds={str(w['numeric_id']):w for w in map(json.loads,(base/'works.jsonl').open())};seedids=set(seeds)
 db=sqlite3.connect(out/'graph.sqlite');db.executescript('CREATE TABLE edges(id INTEGER PRIMARY KEY,source INTEGER,target INTEGER,link INTEGER,ordering INTEGER,data_json TEXT);CREATE INDEX edge_source ON edges(source);CREATE INDEX edge_target ON edges(target);')
 batch=[]
 for r in scan(raw,'l_work_work',9):
  batch.append((int(r[0]),int(r[2]),int(r[3]),int(r[1]),int(r[6]),json.dumps(r)))
  if len(batch)==10000:db.executemany('INSERT INTO edges VALUES(?,?,?,?,?,?)',batch);batch=[]
 db.executemany('INSERT INTO edges VALUES(?,?,?,?,?,?)',batch);db.commit()
 allcount=db.execute('SELECT count(*) FROM edges').fetchone()[0];ids=closure(db,seedids)
 # Keep only entire connected components seeded by recording-linked Works.
 db.execute('DELETE FROM edges WHERE source NOT IN (SELECT id FROM seen)');db.commit()
 assert db.execute('SELECT count(*) FROM edges WHERE target NOT IN (SELECT id FROM seen)').fetchone()[0]==0
 edges=[json.loads(r[0]) for r in db.execute('SELECT data_json FROM edges ORDER BY id')]
 print(json.dumps({'seed_works':len(seeds),'closure_works':len(ids),'relationships':len(edges),'all_dump_relationships':allcount}),flush=True)
 works={r[0]:r for r in scan(raw,'work',7,0,ids)};assert set(works)==ids
 artistrels=list(scan(raw,'l_artist_work',9,3,ids));artistids={r[2] for r in artistrels};artists={r[0]:r for r in scan(raw,'artist',19,0,artistids)};assert set(artists)==artistids
 linkids={r[1] for r in edges+artistrels};links={r[0]:r for r in scan(raw,'link',11,0,linkids)};assert set(links)==linkids
 types={r[0]:r for r in scan(raw,'link_type',16)};attrtypes={r[0]:r for r in scan(raw,'link_attribute_type',8)};worktypes={r[0]:r[1] for r in scan(raw,'work_type',6)}
 attributes=collections.defaultdict(list)
 for r in scan(raw,'link_attribute',3,0,linkids):attributes[r[0]].append(r)
 credits={(r[0],r[1]):r[2] for r in scan(raw,'link_attribute_credit',3,0,linkids)}
 values={(r[0],r[1]):r[2] for r in scan(raw,'link_attribute_text_value',3,0,linkids)}
 def rel(r,entitytypes):
  link=links[r[1]];typ=types[link[1]];assert typ[4:6]==entitytypes
  attrs=[]
  for ar in attributes[r[1]]:
   at=attrtypes[ar[1]];attrs.append({'musicbrainz_id':at[4],'name':at[5],'credited_as':credits.get((r[1],ar[1])),'text_value':values.get((r[1],ar[1]))})
  return {'relationship_numeric_id':int(r[0]),'link_numeric_id':int(r[1]),'type':typ[6],'type_mbid':typ[3],'forward_phrase':typ[8],'reverse_phrase':typ[9],'ordering':int(r[6]),'attributes':attrs,'begin_date':link[2:5],'end_date':link[5:8],'ended':link[10]=='t','entity0_credit':r[7],'entity1_credit':r[8],'source_snapshot':'20260912-002318'}
 creators=collections.defaultdict(list)
 for r in artistrels:
  ar=artists[r[2]];creators[r[3]].append({**rel(r,['artist','work']),'artist':{'mbid':ar[1],'name':ar[2]}})
 payload={wid:{'numeric_id':int(wid),'mbid':w[1],'title':w[2],'type':worktypes.get(w[3]),'comment':w[4],'artist_relations':creators[wid],'recording_linked_seed':wid in seeds} for wid,w in works.items()}
 relations=[{**rel(r,['work','work']),'source_work_mbid':works[r[2]][1],'target_work_mbid':works[r[3]][1]} for r in edges]
 def jsonl(name,rows):
  with (out/name).open('w') as f:
   for r in rows:f.write(json.dumps(r,ensure_ascii=False)+'\n')
 jsonl('works.jsonl',(payload[k] for k in sorted(payload,key=int)));jsonl('relationships.jsonl',relations)
 # A separately downloadable graph remains complete even before production gains graph storage.
 db.executescript('CREATE TABLE works(mbid TEXT PRIMARY KEY,seed INTEGER NOT NULL,data_json TEXT);CREATE TABLE work_relationships(id INTEGER PRIMARY KEY,source_work_mbid TEXT NOT NULL REFERENCES works(mbid),target_work_mbid TEXT NOT NULL REFERENCES works(mbid),type TEXT NOT NULL,ordering INTEGER NOT NULL,data_json TEXT);CREATE INDEX relationship_source ON work_relationships(source_work_mbid);CREATE INDEX relationship_target ON work_relationships(target_work_mbid);')
 db.executemany('INSERT INTO works VALUES(?,?,?)',[(w['mbid'],int(w['recording_linked_seed']),json.dumps(w,ensure_ascii=False)) for w in payload.values()]);db.executemany('INSERT INTO work_relationships VALUES(?,?,?,?,?,?)',[(r['relationship_numeric_id'],r['source_work_mbid'],r['target_work_mbid'],r['type'],r['ordering'],json.dumps(r,ensure_ascii=False)) for r in relations]);db.commit()
 assert not db.execute('PRAGMA foreign_key_check').fetchall();assert db.execute('PRAGMA integrity_check').fetchone()[0]=='ok'
 # Augment the local extraction database transactionally; do not change track proposals.
 main_db=sqlite3.connect(base/'evidence.sqlite');original_tracks=main_db.execute('SELECT count(*) FROM tracks').fetchone()[0];main_db.execute('PRAGMA foreign_keys=ON');main_db.executescript('CREATE TABLE IF NOT EXISTS work_relationships(id INTEGER PRIMARY KEY,source_work_mbid TEXT NOT NULL REFERENCES works(mbid),target_work_mbid TEXT NOT NULL REFERENCES works(mbid),type TEXT NOT NULL,ordering INTEGER NOT NULL,data_json TEXT);CREATE INDEX IF NOT EXISTS work_relationship_source ON work_relationships(source_work_mbid);CREATE INDEX IF NOT EXISTS work_relationship_target ON work_relationships(target_work_mbid);CREATE TABLE IF NOT EXISTS work_graph_seeds(mbid TEXT PRIMARY KEY REFERENCES works(mbid));')
 with main_db:
  main_db.executemany('INSERT INTO works VALUES(?,?) ON CONFLICT(mbid) DO UPDATE SET data_json=excluded.data_json',[(w['mbid'],json.dumps(w,ensure_ascii=False)) for w in payload.values()]);main_db.executemany('INSERT INTO work_relationships VALUES(?,?,?,?,?,?)',[(r['relationship_numeric_id'],r['source_work_mbid'],r['target_work_mbid'],r['type'],r['ordering'],json.dumps(r,ensure_ascii=False)) for r in relations]);main_db.executemany('INSERT INTO work_graph_seeds VALUES(?)',[(w['mbid'],) for w in seeds.values()])
 assert not main_db.execute('PRAGMA foreign_key_check').fetchall();assert main_db.execute('SELECT count(*) FROM tracks').fetchone()[0]==original_tracks;main_db.close()
 # Verify Op. 10 against actual graph edges, not title-inferred ancestry.
 bymbid={w['mbid']:w for w in payload.values()}
 etudes=[w for w in seeds.values() if re.search(r'\bop\.?\s*10\b',w['title'],re.I) and any('chopin' in c['artist']['name'].lower() for c in w['artist_relations'])]
 parents=collections.defaultdict(list)
 for r in relations:
  if r['type']=='parts' and r['target_work_mbid'] in {w['mbid'] for w in etudes}:parents[r['source_work_mbid']].append(r)
 chopin=[{'parent':bymbid[pid],'parts':[{'relationship':r,'work':bymbid[r['target_work_mbid']]} for r in sorted(rs,key=lambda r:(r['ordering'],r['relationship_numeric_id']))]} for pid,rs in parents.items()]
 (out/'chopin-op10.json').write_text(json.dumps(chopin,ensure_ascii=False,indent=2)+'\n')
 summary={'seed_works':len(seeds),'total_works':len(payload),'additional_related_works':len(payload)-len(seeds),'work_relationships':len(relations),'relationship_types':dict(collections.Counter(r['type'] for r in relations)),'scope':'Complete bidirectional connected closure across all Work-to-Work relationship types; relation directions and types preserved.','chopin_op10_individual_seed_works':len(etudes),'chopin_op10_parents':len(parents),'chopin_op10_parts_links':sum(map(len,parents.values())),'seconds':round(time.time()-start,2),'production_writes':False}
 (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2),flush=True)
 # The bundle explicitly includes both nodes and typed edges; no parent replaces a performed child.
 (out/'import-bundle.json').write_text(json.dumps({'status':'local_only_requires_graph_aware_importer','schema_version':1,'works':'works.jsonl','work_relationships':'relationships.jsonl','track_links':'../staged-link-batch.json','review_links':'../review-links.jsonl','source_snapshot':'20260912-002318','semantics':'Preserve source/target direction, relationship type, ordering and attributes. Related Works do not imply performed tracks. Never merge arrangement/revision with its source.'},indent=2)+'\n')
 db.close()
 manifest={'source_snapshot':'20260912-002318','schema_sequence':31,'seeds_sha256':hashlib.sha256((base/'works.jsonl').read_bytes()).hexdigest(),'source_tables':{n:hashlib.sha256((raw/n).read_bytes()).hexdigest() for n in ['l_work_work','link_attribute_credit','link_attribute_text_value']},'files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.iterdir() if p.is_file() and p.name not in ('manifest.json','COMPLETE')}}
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(out/'COMPLETE').write_text('Work graph extraction and integrity checks complete.\n')
if __name__=='__main__':main()
