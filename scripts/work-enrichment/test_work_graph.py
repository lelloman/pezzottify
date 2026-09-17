import sqlite3,unittest
from extract_work_graph import closure,decode
class Graph(unittest.TestCase):
 def graph(self,edges):
  db=sqlite3.connect(':memory:');db.execute('CREATE TABLE edges(source INTEGER,target INTEGER)');db.executemany('INSERT INTO edges VALUES(?,?)',edges);return db
 def test_parent_siblings_and_ancestors(self):
  db=self.graph([(1,2),(1,3),(0,1),(9,10)]);self.assertEqual(closure(db,{2}),{'0','1','2','3'})
 def test_cycles_and_self_links(self):
  db=self.graph([(1,2),(2,3),(3,1),(3,3)]);self.assertEqual(closure(db,{1}),{'1','2','3'})
 def test_isolated_seed_preserved(self):self.assertEqual(closure(self.graph([(1,2)]),{5}),{'5'})
 def test_multiple_components(self):self.assertEqual(closure(self.graph([(1,2),(3,4),(8,9)]),{1,4}),{'1','2','3','4'})
 def test_direction_not_rewritten(self):
  db=self.graph([(1,2)]);closure(db,{2});self.assertEqual(db.execute('SELECT * FROM edges').fetchall(),[(1,2)])
 def test_decode(self):self.assertIsNone(decode(r'\N'));self.assertEqual(decode(r'a\tb'), 'a\tb')
if __name__=='__main__':unittest.main()
