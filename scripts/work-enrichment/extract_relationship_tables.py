"""Extract missing Work relationship tables from an existing MusicBrainz archive."""
import argparse,json,shutil,tarfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--workspace',type=Path,default=Path('.local-work-enrichment'));args=p.parse_args()
 raw=args.workspace/'available-catalog/raw';names={'l_work_work','link_attribute_credit','link_attribute_text_value'}
 missing={n for n in names if not (raw/n).exists()}
 if not missing:return
 with tarfile.open(args.workspace/'20260912-002318/mbdump.tar.bz2','r|bz2') as archive:
  for member in archive:
   if member.name not in {'mbdump/'+n for n in missing}:continue
   name=Path(member.name).name;dest=raw/name;tmp=raw/(name+'.partial')
   print(json.dumps({'extracting':name,'bytes':member.size}),flush=True)
   with archive.extractfile(member) as src,tmp.open('wb') as out:shutil.copyfileobj(src,out,4*1024*1024)
   assert tmp.stat().st_size==member.size;tmp.replace(dest);missing.remove(name)
   if not missing:break
 if missing:raise RuntimeError('Tables absent from archive: '+str(sorted(missing)))
 print('Work relationship tables extracted',flush=True)
if __name__=='__main__':main()
