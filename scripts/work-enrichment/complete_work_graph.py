"""Required graph stage for the one-off local Work extraction pipeline."""
import argparse,hashlib,json,sqlite3,subprocess,sys
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--workspace',type=Path,default=Path('.local-work-enrichment'));a=p.parse_args();root=a.workspace.resolve();base=root/'album-data-extraction';out=base/'work-graph';scripts=Path(__file__).resolve().parent
 subprocess.run([sys.executable,str(scripts/'extract_relationship_tables.py'),'--workspace',str(root)],check=True)
 if (out/'COMPLETE').exists():
  m=json.load(open(out/'manifest.json'))
  if m['seeds_sha256']!=hashlib.sha256((base/'works.jsonl').read_bytes()).hexdigest():raise RuntimeError('Seed Works changed; regenerate the graph in a fresh extraction')
 else:subprocess.run([sys.executable,str(scripts/'extract_work_graph.py'),'--workspace',str(root)],check=True)
 s=json.load(open(out/'summary.json'));db=sqlite3.connect(f'file:{base}/evidence.sqlite?mode=ro',uri=True)
 assert db.execute('SELECT count(*) FROM works').fetchone()[0]==s['total_works']
 assert db.execute('SELECT count(*) FROM work_relationships').fetchone()[0]==s['work_relationships']
 assert not db.execute('PRAGMA foreign_key_check').fetchall();db.close()
 report=f'''# Work graph extraction

Started with **{s['seed_works']:,} recording-linked Works** and followed all Work-to-Work relationships in both directions until no additional Works were reachable.

- **{s['total_works']:,} total Works**, including **{s['additional_related_works']:,} related Works**.
- **{s['work_relationships']:,} typed relationships**, preserving source/target direction, order, dates, attributes, text values and credited names.
- Complete endpoint records and Work-level artist relations are included. Related Works do not create catalog track links.

## Relationship types

| Type | Count |
|---|---:|
'''
 for kind,n in sorted(s['relationship_types'].items()):report+=f'| {kind} | {n:,} |\n'
 report+='''
## Files and import requirements

`works.jsonl` is the expanded Work set. `relationships.jsonl` contains directed edges. `graph.sqlite` supports indexed queries. `import-bundle.json` requires both nodes and relationships along with the existing track links; it is not a flat Work-only batch. The main `../evidence.sqlite` also contains all expanded Works and the `work_relationships` table. Original `../works.jsonl` remains the immutable recording-linked seed set.

Production currently has no Work-relationship table or graph importer. The bundle is staged locally; production graph storage/import must be implemented before applying it. No production writes occurred. Parts, arrangements, revisions and quotations remain distinct typed relations: they are not merged into a single identity or all treated as parent/child links. Source ordering (including zeros) is preserved as supplied, not inferred from titles.

## Chopin Op. 10

See `chopin-op10.json` for the parent Work and its actual directed parts links. This is source relationship evidence, not title-based inference.

## Remaining metadata limits

Work aliases, external URLs, ISWCs and catalogue-number attribute tables were not part of this stage. Creator/arranger claims remain Work-level source relationships, not recording-level assertions. The graph is a connected closure, which may include related compositions without a performance in the available catalog.
'''
 (out/'REPORT.md').write_text(report)
 summary=json.load(open(base/'summary.json'));summary['work_graph']=s;summary['total_works_including_related']=s['total_works'];(base/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 reportpath=base/'REPORT.md';text=reportpath.read_text();marker='# Work relationships now included'
 if marker not in text:
  text=marker+'\n\nThe graph stage is complete: **'+f"{s['total_works']:,} Works and {s['work_relationships']:,} Work-to-Work relationships"+'**. See [work-graph/REPORT.md](work-graph/REPORT.md) and the graph-aware [import bundle](work-graph/import-bundle.json). Track links remain attached to the performed Work, not its parent. The original extraction report below describes the recording-linked seed set. Its Work-hierarchy limitation is superseded by this stage.\n\n---\n\n'+text
  reportpath.write_text(text)
 m=json.load(open(base/'manifest.json'));m['limitations']=[x for x in m['limitations'] if not x.startswith('No Work hierarchy')];m['limitations'].append('Work graph included; aliases, external URLs, ISWCs and catalogue-number attributes still not extracted.')
 m['work_graph_summary']=s;m['pipeline_scripts']={str(f.relative_to(scripts.parent.parent)):hashlib.sha256(f.read_bytes()).hexdigest() for f in scripts.glob('*.py')}
 for name in ('evidence.sqlite','summary.json','REPORT.md','extract.py'):m['files'][name]=hashlib.sha256((base/name).read_bytes()).hexdigest()
 (base/'manifest.json').write_text(json.dumps(m,indent=2)+'\n')
 gm=json.load(open(out/'manifest.json'));gm['files']['REPORT.md']=hashlib.sha256((out/'REPORT.md').read_bytes()).hexdigest();(out/'manifest.json').write_text(json.dumps(gm,indent=2)+'\n')
 statepath=root/'RUN-STATE.json'
 if statepath.exists():
  state=json.load(open(statepath));state['work_graph_extraction']={'status':'complete','report':'album-data-extraction/work-graph/REPORT.md','summary':s};statepath.write_text(json.dumps(state,indent=2)+'\n')
 print(json.dumps(s,indent=2))
if __name__=='__main__':main()
