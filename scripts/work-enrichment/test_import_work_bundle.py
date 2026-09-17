import json,sqlite3,tempfile,unittest
from pathlib import Path
from import_work_bundle import apply
SCHEMA=Path(__file__).resolve().parents[2]/'pezzottify-server/src/enrichment_store/work_graph.sql'
BASE='''CREATE TABLE works_v1(id TEXT PRIMARY KEY,title TEXT,creators_json TEXT,catalog_number TEXT,kind TEXT,identity_key TEXT UNIQUE,created_at INTEGER);CREATE TABLE work_external_ids_v1(provider TEXT,external_id TEXT,work_id TEXT REFERENCES works_v1(id),PRIMARY KEY(provider,external_id),UNIQUE(work_id,provider));CREATE TABLE work_resolutions_v1(track_id TEXT PRIMARY KEY,work_id TEXT REFERENCES works_v1(id),status TEXT,reason TEXT,evidence_json TEXT,evaluated_at INTEGER,enriched_at INTEGER,last_verified_at INTEGER,source_status TEXT);'''
class Import(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name);(self.root/'work_graph.sql').write_bytes(SCHEMA.read_bytes());(self.root/'production-manifest.json').write_text('{}')
  self.db=sqlite3.connect(':memory:');self.db.executescript(BASE);self.cat=sqlite3.connect(':memory:');self.cat.executescript("CREATE TABLE albums(id TEXT);INSERT INTO albums VALUES('album');CREATE TABLE tracks(id TEXT,track_available INTEGER,audio_uri TEXT,duration_ms INTEGER,album_rowid INTEGER);INSERT INTO tracks VALUES('track',1,'audio',1000,1);")
  self.works=[{'mbid':'a','title':'Parent','type':None,'artist_relations':[]},{'mbid':'b','title':'Child','type':None,'artist_relations':[]}]
  self.rels=[{'relationship_numeric_id':1,'source_work_mbid':'a','target_work_mbid':'b','type_mbid':'parts-id','type':'parts','ordering':2,'attributes':[]}]
  self.links=[{'track_id':'track','work':{'musicbrainz_id':'b'},'album_id':'album','source_recording_candidates':['recording'],'album_match_basis':'fingerprint_rules'}];self.tracks={'track':{'catalog_duration_ms':1000,'album_id':'album'}}
 def tearDown(self):self.db.close();self.cat.close();self.temp.cleanup()
 def run_import(self):return apply(self.db,self.cat,self.root,{'source_snapshot':'test'},self.works,self.rels,self.links,self.tracks)
 def test_graph_and_child_track_link(self):
  r=self.run_import();self.assertEqual(r['counts']['track_links_added'],1)
  self.assertEqual(self.db.execute('SELECT ordering FROM work_relationships_v1').fetchone()[0],2)
  self.assertEqual(self.db.execute("SELECT e.external_id FROM work_resolutions_v1 r JOIN work_external_ids_v1 e ON e.work_id=r.work_id").fetchone()[0],'b')
  self.assertEqual(self.run_import()['status'],'already_imported')
 def test_preserve_existing_identity_and_link(self):
  self.db.executescript("INSERT INTO works_v1 VALUES('old','Existing title','[\"Original\"]',NULL,'composition','existing',1);INSERT INTO work_external_ids_v1 VALUES('musicbrainz','b','old');INSERT INTO work_resolutions_v1 VALUES('track','old','linked','original','{}',1,1,1,'old');")
  r=self.run_import();self.assertEqual(r['counts']['existing_links_preserved'],1);self.assertEqual(self.db.execute("SELECT title FROM works_v1 WHERE id='old'").fetchone()[0],'Existing title');self.assertEqual(self.db.execute('SELECT reason FROM work_resolutions_v1').fetchone()[0],'original')
 def test_conflict_is_not_overwritten(self):
  self.db.executescript("INSERT INTO works_v1 VALUES('other','Different','[]',NULL,'song','different',1);INSERT INTO work_resolutions_v1 VALUES('track','other','linked','original','{}',1,1,1,'old');")
  self.assertEqual(self.run_import()['counts']['existing_link_conflict'],1);self.assertEqual(self.db.execute('SELECT work_id FROM work_resolutions_v1').fetchone()[0],'other')
 def test_current_availability_and_fingerprint(self):
  self.cat.execute("UPDATE tracks SET track_available=0");self.assertEqual(self.run_import()['counts']['currently_unavailable_or_missing'],1);self.assertEqual(self.db.execute('SELECT count(*) FROM work_resolutions_v1').fetchone()[0],0)
 def test_changed_catalog_is_held(self):
  self.cat.execute('UPDATE tracks SET duration_ms=1001');self.assertEqual(self.run_import()['counts']['catalog_changed_since_fingerprint'],1)
 def test_transaction_rolls_back(self):
  self.rels[0]['target_work_mbid']='missing'
  with self.assertRaises(KeyError):self.run_import()
  self.assertEqual(self.db.execute('SELECT count(*) FROM works_v1').fetchone()[0],0)
  self.assertIsNone(self.db.execute("SELECT name FROM sqlite_master WHERE name='work_relationships_v1'").fetchone())
if __name__=='__main__':unittest.main()
