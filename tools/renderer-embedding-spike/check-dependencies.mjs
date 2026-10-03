// npm counterpart of deny.toml's permissive third-party license policy, tool-only.
import {readFileSync,readdirSync,writeFileSync} from 'node:fs';import {dirname,join} from 'node:path';import {fileURLToPath} from 'node:url';import assert from 'node:assert/strict';
const root=dirname(fileURLToPath(import.meta.url));const lock=JSON.parse(readFileSync(join(root,'package-lock.json')));const allowed=new Set(['MIT','BSD-3-Clause','ISC']);
assert.equal(lock.packages[''].dependencies['pixi.js'],'8.22.0');const packages=[];let notices='Isolated issue-60 tool dependency notices. Production graphs are unchanged.\n\n';
for(const [path,pin]of Object.entries(lock.packages)){
 if(!path)continue;assert.match(pin.integrity,/^sha512-/);assert.match(pin.resolved,/^https:\/\/registry\.npmjs\.org\//);
 const folder=join(root,path),manifest=JSON.parse(readFileSync(join(folder,'package.json')));assert.equal(manifest.version,pin.version);assert.ok(allowed.has(manifest.license),`${manifest.name} license ${manifest.license}`);
 const files=readdirSync(folder).filter(name=>/^(licen[sc]e|copying|notice)([.-]|$)/i.test(name)).sort();
 const supplementary=manifest.name==='@pixi/colord'?'licenses/colord-MIT.txt':null;assert.ok(files.length>0||supplementary,`${manifest.name} missing distributed license`);
 packages.push({name:manifest.name,version:manifest.version,license:manifest.license,resolved:pin.resolved,integrity:pin.integrity,license_files:files,supplementary_notice:supplementary});notices+=`=== ${manifest.name}@${manifest.version} (${manifest.license}) ===\n`;for(const file of files)notices+=readFileSync(join(folder,file),'utf8')+'\n';if(supplementary)notices+=readFileSync(join(root,supplementary),'utf8')+'\n';
}
packages.sort((a,b)=>a.name.localeCompare(b.name));writeFileSync(join(root,'dependency-inventory.json'),JSON.stringify({schema_version:1,scope:'isolated issue-60 tool only',packages},null,2)+'\n');writeFileSync(join(root,'licenses/npm-NOTICES.txt'),notices);
console.log(`PASS: ${packages.length} exact lock pins with registry SRI, allowed licenses, and retained notices`);
