#!/usr/bin/env python3
"""Static emitted Werewolf inventory, not browser network/performance evidence."""
import argparse,gzip,hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--wasm',type=Path,required=True);p.add_argument('--tree',type=Path,required=True);p.add_argument('--bundle',type=Path,required=True);p.add_argument('--receipt',type=Path,required=True)
a=p.parse_args();b=a.wasm.read_bytes();tree=a.tree.read_text()
assert b.startswith(b'\0asm\x01\0\0\0');assert len(b)<=1_250_000
compressed=gzip.compress(b,compresslevel=9,mtime=0);assert len(compressed)<=500_000
assert 'tabula-game-werewolf ' in tree
for name in ['tabula-game-chess ','tabula-game-tiles ','leptos ']:assert name not in tree,name
absent=[]
for folder in ['assets/fonts','assets/packs/chess','assets/packs/tiles','assets/packs/werewolf']:
 for f in sorted((ROOT/folder).iterdir()):
  if f.suffix in ['.png','.ttf']:
   assert f.read_bytes() not in b,f;absent.append(str(f.relative_to(ROOT)))
text=(a.bundle/'resource-manifest.js').read_text();manifest=json.loads(text.removeprefix('window.TabulaResourceManifest=').rstrip(';\n'));files=manifest['files'];assert manifest['schema']==1 and len(files)==18
rows=[]
for alias,entry in files.items():
 payload=(a.bundle/entry['url']).read_bytes();assert len(payload)==entry['bytes'];assert hashlib.sha256(payload).hexdigest()==entry['sha256'];rows.append({'alias':alias,'bytes':len(payload),'sha256':entry['sha256']})
setup=(a.bundle/'index.html').read_text();assert not re.search(r'<(?:img|link)[^>]+(?:png|wasm)',setup);assert 'glcanvas' not in setup
pack_rows=[row for row in rows if row['alias'].startswith('werewolf/')];assert len(pack_rows)==14
assert sum(row['bytes'] for row in pack_rows)<=2*1024*1024
result={'evidence':'static emitted selected-game inventory, not real browser request waterfall or runtime performance','wasm':{'bytes':len(b),'gzip9_bytes':len(compressed),'sha256':hashlib.sha256(b).hexdigest(),'selected_normal_graph':'PASS Werewolf only, no Chess/Tiles/Leptos','external_payloads_absent':absent},'runtime_payloads':rows,'encoded_pack_bytes_all_densities':sum(row['bytes'] for row in pack_rows),'setup_game_art_or_wasm_references':0}
a.receipt.parent.mkdir(parents=True,exist_ok=True);a.receipt.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result['wasm'],indent=2));print('PASS static Werewolf emitted budgets and complete content hashes')
