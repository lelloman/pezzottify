import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('backfill', Path(__file__).with_name('backfill.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
DDL = Path(__file__).resolve().parents[2] / 'pezzottify-server/src/catalog_store/genre_artwork.sql'


class BackfillTest(unittest.TestCase):
    def test_ranking_fallback_dry_run_and_preserved_choices(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'catalog.db'
            db = sqlite3.connect(path)
            db.executescript('''
                CREATE TABLE artists(rowid INTEGER PRIMARY KEY,id TEXT,name TEXT,popularity INTEGER);
                CREATE TABLE albums(rowid INTEGER PRIMARY KEY,id TEXT,name TEXT,popularity INTEGER);
                CREATE TABLE tracks(rowid INTEGER PRIMARY KEY,album_rowid INTEGER,track_available INTEGER);
                CREATE TABLE track_artists(track_rowid INTEGER,artist_rowid INTEGER);
                CREATE TABLE artist_genres(artist_rowid INTEGER,genre TEXT);
                CREATE TABLE artist_images(artist_rowid INTEGER,url TEXT);
                CREATE TABLE album_images(album_rowid INTEGER,url TEXT);
                INSERT INTO artists VALUES (1,'a','A',100),(2,'b','B',10),(3,'c','C',20),(4,'d','D',90);
                INSERT INTO albums VALUES (1,'album','Album',10);
                INSERT INTO tracks VALUES (1,1,1),(2,1,1),(3,1,0);
                INSERT INTO track_artists VALUES (1,1),(1,2),(2,2),(3,1),(1,3);
                INSERT INTO artist_genres VALUES (1,'jazz'),(2,'jazz'),(3,'rock'),(4,'missing'),(1,'unavailable');
                INSERT INTO artist_images VALUES (1,'https://a'),(2,'https://b');
                INSERT INTO album_images VALUES (1,'https://album');
            ''')
            db.close()
            report = module.backfill(path)
            selected = {c['genre']: c for c in report['selections']}
            self.assertEqual(selected['jazz']['entity_id'], 'b')
            self.assertEqual(selected['rock']['entity_type'], 'album')
            self.assertEqual(report['missing'], ['missing'])
            with self.assertRaisesRegex(RuntimeError, 'migration'):
                module.backfill(path, True)
            db = sqlite3.connect(path)
            self.assertIsNone(db.execute("SELECT 1 FROM sqlite_master WHERE name='genre_artwork'").fetchone())
            db.executescript(DDL.read_text())
            db.close()
            applied = module.backfill(path, True)
            self.assertEqual(applied['inserted'], 3)
            db = sqlite3.connect(path)
            db.execute("UPDATE genre_artwork SET entity_id='manual-override' WHERE genre='jazz'")
            db.commit()
            before = db.execute('SELECT * FROM genre_artwork ORDER BY genre').fetchall()
            self.assertEqual(module.backfill(path, True)['inserted'], 0)
            self.assertEqual(before, db.execute('SELECT * FROM genre_artwork ORDER BY genre').fetchall())
            db.close()


if __name__ == '__main__':
    unittest.main()
