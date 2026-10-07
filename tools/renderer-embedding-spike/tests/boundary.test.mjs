import test from 'node:test';import assert from 'node:assert/strict';
import {validateFrame,validateView,validateViewport,validatePreferences} from '../boundary.mjs';
import {fixture,contract,identity,envelope,frame} from './helpers.mjs';
test('all actual Rust fixture frames validate against Rust generated descriptor',()=>{
  let frames=0;for(const scenario of fixture.scenarios)for(const f of scenario.frames){validateFrame(f,contract,fixture.assets,fixture.themes);frames++;}assert.ok(frames>=10);assert.ok(fixture.scenarios.length>=5);
});
test('view boundary rejects canonical fields and unknown nested draw fields',()=>{
  for(const mutate of [v=>{v.state={secret:'x'};},v=>{v.frame.secret='x';},v=>{v.frame.camera.seed='x';},v=>{v.frame.commands[0].hidden_hand=['x'];},v=>{v.schema_version=2;},v=>{v.generation=0;},v=>{v.session_id='other';}]){const v=envelope();mutate(v);assert.throws(()=>validateView(v,identity,-1,contract,fixture.assets,fixture.themes));}
});
test('revision replay is rejected and caller mutation cannot change accepted view',()=>{const v=envelope(3);const accepted=validateView(v,identity,2,contract,fixture.assets,fixture.themes);assert.throws(()=>validateView(v,identity,3,contract));v.frame.commands[0].rect[0]=Infinity;assert.notEqual(accepted.frame.commands[0].rect[0],Infinity);});
test('numeric hostile input missing assets and scope cross closes are rejected',()=>{
  for(const mutate of [v=>{v.camera.zoom=NaN;},v=>{v.commands[0].rect[2]=-1;},v=>{v.commands[0].border.color[0]=256;},v=>{v.commands.push({kind:'pop_transform',layer:0,z:0});},v=>{v.commands=[{kind:'push_clip',layer:0,z:0,rect:[0,0,1,1]},{kind:'push_transform',layer:0,z:0,matrix:[1,0,0,1,0,0]},{kind:'pop_clip',layer:0,z:0},{kind:'pop_transform',layer:0,z:0}];},v=>{v.commands.push({kind:'sprite',layer:0,z:0,asset:'unknown',rect:[0,0,1,1],tint:[255,255,255,255],rotation:0,pivot:[0,0]});}]){const v=structuredClone(frame);mutate(v);assert.throws(()=>validateFrame(v,contract,fixture.assets,fixture.themes));}
});
test('viewport dpi preference limits reject hostile values',()=>{for(const dpi of [0,NaN,Infinity,4.01])assert.throws(()=>validateViewport({width:900,height:720,dpi}));assert.throws(()=>validatePreferences({reduced_motion:true,audio_enabled:false,volume:1.01,modal:false}));});
