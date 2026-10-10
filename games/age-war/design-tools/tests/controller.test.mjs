import test from 'node:test';
import assert from 'node:assert/strict';
import {PresentationController as Core} from '../core/controller.mjs';
const sample = (_rig, clip, time) => ({x:Math.min(time, clip.duration_ms), opacity:1});
class PresentationController extends Core { constructor(rig, clip) { super(rig, clip, sample); } }
const rig = {unit_id:'test', family:'synthetic', skills:[], d01_attack:{windup_ticks:1,recovery_ticks:1,projectile_speed_q_per_tick:0}};
const clip = (markers=[], loop=false) => ({id:'idle',duration_ms:100,loop,markers});
const marker = time_ms => ({time_ms,kind:'cue',authority:true});
test('zero-time marker emits once on first positive advance',()=>{const c=new PresentationController(rig,clip([marker(0)]));c.advance(0);assert.equal(c.cues.length,0);c.advance(1);assert.equal(c.cues.length,1);assert.equal(c.cues[0].authority,false);c.advance(0);c.advance(900);assert.equal(c.cues.length,1);});
test('loop markers include every elapsed cycle',()=>{const c=new PresentationController(rig,clip([marker(50)],true));c.advance(251);assert.equal(c.cues.length,3);});
test('sparse and dense loop advances have equal ordered non-authoritative cues',()=>{const a=new PresentationController(rig,clip([marker(75),marker(0),marker(25)],true)),b=new PresentationController(rig,clip([marker(75),marker(0),marker(25)],true));a.advance(351);for(let i=0;i<351;i++)b.advance(1);assert.deepEqual(a.cues,b.cues);assert(a.cues.every(x=>x.authority===false));});
test('endpoint marker emits once and no duplicate at equal time',()=>{const c=new PresentationController(rig,clip([marker(100)]));c.advance(100);c.advance(0);c.advance(1);assert.equal(c.cues.length,1);});
test('interrupt clears old cues and permits new phase-start marker',()=>{const c=new PresentationController(rig,clip([marker(0)]));c.advance(1);c.interrupt(clip([marker(0)]));assert.equal(c.cues.length,0);c.advance(1);assert.equal(c.cues.length,1);assert.equal(c.cues[0].generation,1);});
test('reset clears cues and readonly sampling emits none',()=>{const c=new PresentationController(rig,clip([marker(0)]));c.poseAt(100);assert.equal(c.cues.length,0);c.advance(1);c.reset();assert.equal(c.cues.length,0);c.advance(1);assert.equal(c.cues.length,1);});
for(const speed of [.5,1,2])test(`loop cadence independent at speed ${speed}`,()=>{const a=new PresentationController(rig,clip([marker(0),marker(50)],true)),b=new PresentationController(rig,clip([marker(0),marker(50)],true));a.speed=b.speed=speed;a.advance(400/speed);for(let i=0;i<400;i++)b.advance(1/speed);assert.deepEqual(a.cues,b.cues);});
for(const delta of [-1,NaN,Infinity])test(`invalid delta ${delta} is transactional`,()=>{const c=new PresentationController(rig,clip([marker(0)]));assert.throws(()=>c.advance(delta));assert.equal(c.time_ms,0);assert.equal(c.cues.length,0);});

for(const speed of [-1,0,NaN,Infinity])test(`invalid speed ${speed} is transactional`,()=>{const c=new PresentationController(rig,clip([marker(0)]));c.speed=speed;assert.throws(()=>c.advance(1));assert.equal(c.time_ms,0);assert.deepEqual(c.cues,[]);});
test('excessive loop catch-up fails before mutating',()=>{const c=new PresentationController(rig,clip([marker(0)],true));assert.throws(()=>c.advance(1e6));assert.equal(c.time_ms,0);assert.equal(c.started,false);assert.deepEqual(c.cues,[]);});
test('overflow and malformed target fail before mutating',()=>{const c=new PresentationController(rig,clip());assert.throws(()=>c.advance(Number.MAX_VALUE));assert.throws(()=>c.interrupt(clip([marker(-1)])));assert.equal(c.time_ms,0);assert.equal(c.generation,0);});
