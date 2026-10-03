import test from 'node:test';import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import {webcrypto,createHash} from 'node:crypto';
import {verifiedBytes,inspectPng} from '../pixi-backend.mjs';
const source=readFileSync(new URL('../../../assets/packs/tiles/tiles@1x.png',import.meta.url));
const file={name:'atlas',path:'assets/tiles.png',bytes:source.byteLength,sha256:createHash('sha256').update(source).digest('hex')};
const fetchBytes=bytes=>async()=>new Response(bytes);
test('current authored atlas bytes verify and decoded allocation bounds match PNG dimensions',async()=>{const bytes=await verifiedBytes(file,'http://localhost:8760/',{fetch:fetchBytes(source),crypto:webcrypto});const dimensions=inspectPng(bytes);assert.equal(dimensions.width,432);assert.equal(dimensions.height,288);});
test('asset digest size and unsafe paths reject before image decode',async()=>{
 for(const path of ['../token','/token','http://localhost:8760/token','assets/../../token','assets/%2e%2e/token','assets/tile.png?secret=1','assets/tile.png#hash'])await assert.rejects(verifiedBytes({...file,path},'http://localhost:8760/',{fetch:fetchBytes(source),crypto:webcrypto}));
 await assert.rejects(verifiedBytes({...file,sha256:'0'.repeat(64)},'http://localhost:8760/',{fetch:fetchBytes(source),crypto:webcrypto}),{code:'asset_integrity'});
 await assert.rejects(verifiedBytes({...file,bytes:file.bytes-1},'http://localhost:8760/',{fetch:fetchBytes(source),crypto:webcrypto}),{code:'asset_size'});
});
test('PNG hostile header oversized pixels corrupt signature and descriptor mismatch reject predecode',()=>{
 const bytes=source.buffer.slice(source.byteOffset,source.byteOffset+source.byteLength);const huge=bytes.slice(0);new DataView(huge).setUint32(16,100000);assert.throws(()=>inspectPng(huge),{code:'asset_pixel_limit'});const corrupt=bytes.slice(0);new Uint8Array(corrupt)[0]=0;assert.throws(()=>inspectPng(corrupt),{code:'asset_png_header'});assert.throws(()=>inspectPng(bytes,{width:1,height:1,name:'bad'}),{code:'asset_dimensions'});
});
