import test from 'node:test';import assert from 'node:assert/strict';import {createBridge,makeEnvelope,validateEnvelope} from '../bridge.mjs';import {identity,Target} from './helpers.mjs';
test('origin source generation revision and exact envelope fields gate all iframe messages',()=>{
 const source={},options={origin:'http://localhost:8760',source,identity,lastRevision:0};const message=makeEnvelope('pong',identity,1,{nonce:1,sent_ms:10});
 assert.equal(validateEnvelope({origin:options.origin,source,data:message},options).kind,'pong');
 for(const mutate of [e=>{e.origin='http://evil.invalid';},e=>{e.source={};},e=>{e.data.generation=0;},e=>{e.data.revision=0;},e=>{e.data.secret='hidden';},e=>{e.data.payload.secret='hidden';}]){const event={origin:options.origin,source,data:structuredClone(message)};mutate(event);assert.throws(()=>validateEnvelope(event,options));}
});
test('bridge one listener monotonic delivery dispose revokes sends and detached callbacks',()=>{
 const owner=new Target(),sent=[],received=[],errors=[],source={postMessage:(value,origin)=>sent.push({value,origin})};const bridge=createBridge({window:owner,targetWindow:source,origin:'http://localhost:8760',identity,onMessage:value=>received.push(value),onError:error=>errors.push(error)});
 bridge.send('ping',{nonce:1,sent_ms:0});assert.equal(sent[0].origin,'http://localhost:8760');const event={origin:'http://localhost:8760',source,data:makeEnvelope('pong',identity,0,{nonce:1,sent_ms:0})};owner.fire('message',event);owner.fire('message',event);assert.equal(received.length,1);assert.equal(errors[0].code,'stale_revision');assert.equal(owner.count(),1);bridge.dispose();assert.equal(owner.count(),0);owner.fire('message',{...event,data:makeEnvelope('pong',identity,1,{nonce:1,sent_ms:0})});assert.equal(received.length,1);assert.throws(()=>bridge.send('dispose'));assert.equal(bridge.snapshot().listeners,0);
});
test('prototype property names are never accepted as bridge message kinds',()=>{
 const source={},options={origin:'http://localhost:8760',source,identity,lastRevision:0};
 for(const kind of ['constructor','toString','__proto__']){
  const data={channel:'tabula-renderer-spike',protocol:1,kind,session:identity.session_id,generation:identity.generation,revision:1,payload:{token:'must reject'}};
  assert.throws(()=>validateEnvelope({origin:options.origin,source,data},options),error=>error.code==='unknown_kind');
 }
});
test('validated initialization revision rejects replay before the next bridge message',()=>{
 const owner=new Target(),received=[],errors=[],source={postMessage:()=>{}};
 const bridge=createBridge({window:owner,targetWindow:source,origin:'http://localhost:8760',identity,initialIncomingRevision:0,onMessage:value=>received.push(value),onError:error=>errors.push(error)});
 owner.fire('message',{origin:'http://localhost:8760',source,data:makeEnvelope('dispose',identity,0,{})});
 assert.equal(received.length,0);assert.equal(errors[0].code,'stale_revision');
 owner.fire('message',{origin:'http://localhost:8760',source,data:makeEnvelope('dispose',identity,1,{})});
 assert.equal(received.length,1);assert.equal(bridge.snapshot().last_incoming,1);bridge.dispose();
 assert.throws(()=>createBridge({window:owner,targetWindow:source,origin:'http://localhost:8760',identity,initialIncomingRevision:-2,onMessage:()=>{}}));
});
