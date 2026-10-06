import importlib.util
from pathlib import Path
import sqlite3
import unittest
spec=importlib.util.spec_from_file_location('opera',Path(__file__).with_name('opera_recordings.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

class OperaEvidenceTest(unittest.TestCase):
    def test_no_album_inheritance_and_linked_parts_only(self):
        db=sqlite3.connect(':memory:')
        db.executescript('''
          CREATE TABLE entity_tags_v1(entity_type TEXT,entity_id TEXT,tag TEXT,confidence REAL);
          CREATE TABLE work_source_evidence_v1(work_id TEXT,evidence_json TEXT);
          CREATE TABLE works_v1(id TEXT,kind TEXT);
          CREATE TABLE work_relationships_v1(source_work_id TEXT,target_work_id TEXT,relationship_type TEXT);
          CREATE TABLE work_resolutions_v1(track_id TEXT,work_id TEXT,status TEXT);
          CREATE TABLE track_enrichment_v1(track_id TEXT,form TEXT);
          INSERT INTO entity_tags_v1 VALUES ('album','beethoven-9','opera',1),('artist','conductor','opera',1),
            ('track','aria','opera',0.95),('track','transcription','opera transcription',1),('track','uncertain','opera',0.5);
          INSERT INTO work_source_evidence_v1 VALUES ('opera','{"type":"Opera"}');
          INSERT INTO work_relationships_v1 VALUES ('opera','act','parts'),('opera','unrelated','other version');
          INSERT INTO work_resolutions_v1 VALUES ('linked','act','linked'),('unresolved','act','unresolved'),('other','unrelated','linked');
          INSERT INTO track_enrichment_v1 VALUES ('concerto','piano concerto');
        ''')
        evidence,excluded=module.opera_evidence(db)
        self.assertEqual(set(evidence),{('track','aria'),('track','linked')})
        self.assertIn('concerto',excluded)
        db.close()

if __name__=='__main__': unittest.main()
