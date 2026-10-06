const test = require("node:test"), assert = require("node:assert/strict");
const direct = require("../direct-transport.js"), launch = require("../launch-options.js");
const id = "00000000000000000000000000000007", game = "com.tabula.chess"; // xtask-allow-game-id: existing game-client leaf binding, not platform dispatch.
const scope = "a".repeat(64);
function memoryStorage() { const data=new Map(); return {data,getItem:k=>data.get(k)??null,setItem:(k,v)=>data.set(k,v),removeItem:k=>data.delete(k)}; }
const quickTimers={setTimeout:(fn,ms)=>setTimeout(fn,ms<20000?0:ms),clearTimeout};
const hex = text => Buffer.from(text).toString("hex");
const response = (body, extra={}) => new Response(typeof body === "string" ? body : JSON.stringify(body), {status:200,headers:{"Cache-Control":"no-store","Content-Type":"application/json"},...extra});
function fixture(fetcher, extra={}) {
  const controller = new AbortController();
  return {controller,transport:direct.create({matchId:id,gameId:game,signal:controller.signal,current:()=>true,protocol:"https:",fetcher,storage:memoryStorage(),timers:quickTimers,...extra})};
}
test("public online launch admits no grant, credential or client config", () => {
  const query = "?game=" + game + "&mode=network&seats=2&source=tabula&return_to=" + encodeURIComponent("/games/" + game + "?setup=1") + "&locale=en&match_id=" + id;
  const parsed = launch.resolve(launch.parse(query), () => ({matches:false}));
  assert.equal(parsed.online, true);
  assert.ok(launch.argumentsFor(parsed).includes("--online-match\n" + id));
  for (const extra of ["&binding_id=secret","&clock=fischer","&match_id="+id]) assert.throws(() => launch.parse(query + extra));
  assert.throws(() => launch.parse(query.replace(id, "0".repeat(32))));
});
test("opaque command JSON keeps u128 identity exact", () => {
  const command = '{"command":{"match_id":340282366920938463463374607431768211450}}';
  const body = direct.commandBody(id, command);
  assert.ok(body.includes("340282366920938463463374607431768211450"));
  assert.equal(direct.decodeHex(hex(command), 65536), command);
  assert.throws(() => direct.decodeHex("aaZZ", 10));
  assert.throws(() => direct.commandBody(id, "x".repeat(65536)));
});
test("cookie-only context/grant/attach and opaque command stay document-memory", async () => {
  const calls=[],token="A".repeat(43),grant="A".repeat(100),frames={version:2,frames:[]};
  const f=fixture(async(path,init)=>{
    calls.push({path,init});
    if(path.endsWith("/context"))return response({version:1,disposition:"authenticated",csrf_token:token});
    if(path.endsWith("/grant"))return response({version:2,ready:true,binding_id:grant,seat:0,game_id:game,game_version:"1.0.0"});
    if(path.endsWith("/attach"))return response({version:2,attachment_id:id,operation_scope:scope,seat:0,next_seq:1,frames:[]});
    return response(frames);
  });
  const attached=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-attach.txt")));
  assert.equal(attached.game_id,game);assert.equal(attached.game_version,"1.0.0");
  assert.equal(attached.attachment.attachment_id,id);assert.equal(attached.binding_id,undefined);
  assert.ok(!JSON.stringify(attached).includes(grant));
  await f.transport.file("tabula-online-command/"+hex('{"command":{"match_id":340282366920938463463374607431768211450}}'));
  for(const {path,init} of calls){
    assert.ok(!path.includes(token)&&!path.includes(grant));
    assert.equal(init.credentials,"same-origin");assert.equal(init.mode,"same-origin");
    assert.equal(init.cache,"no-store");assert.equal(init.redirect,"error");
    assert.equal(init.headers.Authorization,undefined);
    if(init.method==="POST")assert.equal(init.headers["X-Tabula-CSRF"],token);
  }
  assert.ok(calls.at(-1).init.body.includes("340282366920938463463374607431768211450"));
  f.transport.retire();await assert.rejects(f.transport.file("tabula-online-poll.txt"));
});
test("wrong package and missing no-store fail before attachment",async()=>{
  for(const wrong of [true,false]){
    const f=fixture(async(path)=>path.endsWith("/context")?response({version:1,disposition:"authenticated",csrf_token:"A".repeat(43)}):wrong?response({version:2,ready:true,binding_id:"A".repeat(100),seat:0,game_id:"org.example.other",game_version:"1.0.0"}):response({},{headers:{"Content-Type":"application/json"}}));
    await assert.rejects(f.transport.file("tabula-online-attach.txt"));
  }
});
test("response size bound rejects before reading announced oversized body",async()=>{
  const f=fixture(async()=>response("{}",{headers:{"Cache-Control":"no-store","Content-Type":"application/json","Content-Length":String(direct.MAX_RESPONSE+1)}}));
  await assert.rejects(f.transport.file("tabula-online-attach.txt"));
});
test("no HTTPS or invalid public routing hint can start online transport",()=>{
  for(const change of [{protocol:"http:"},{matchId:"0".repeat(32)},{matchId:"../auth"}]){
    assert.throws(()=>direct.create({matchId:id,gameId:game,signal:new AbortController().signal,current:()=>true,protocol:"https:",...change}));
  }
});
test("retirement rejects late private completion and concurrent requests stay bounded",async()=>{
  let release,calls=0;
  const f=fixture(async()=>{calls++;return await new Promise(resolve=>release=resolve);});
  const pending=f.transport.file("tabula-online-attach.txt");
  await assert.rejects(f.transport.file("tabula-online-poll.txt"),/already active/);
  f.transport.retire();
  release(response({version:1,disposition:"authenticated",csrf_token:"A".repeat(43)}));
  await assert.rejects(pending,/retired|interrupted/);assert.equal(calls,1);
});
test("unannounced oversized streamed private response is cancelled and reader released",async()=>{
  let released=false,aborted=false;
  const f=fixture(async(_path,init)=>{
    init.signal.addEventListener("abort",()=>aborted=true);let count=0;
    return {status:200,redirected:false,headers:new Headers({"Cache-Control":"no-store","Content-Type":"application/json"}),body:{getReader(){return {async read(){return {done:false,value:new Uint8Array(++count===1?direct.MAX_RESPONSE:1)};},releaseLock(){released=true;}};}}};
  });
  await assert.rejects(f.transport.file("tabula-online-attach.txt"),/budget/);
  assert.equal(released,true);assert.equal(aborted,true);
});
function authFetcher(effect=()=>response({version:2,frames:[]})) {
  return async (path,init) => {
    if(path.endsWith("/context"))return response({version:1,disposition:"authenticated",csrf_token:"A".repeat(43)});
    if(path.endsWith("/grant"))return response({version:2,ready:true,binding_id:"B".repeat(100),seat:0,game_id:game,game_version:"1.0.0"});
    if(path.endsWith("/attach"))return response({version:2,attachment_id:id,operation_scope:scope,seat:0,next_seq:1,frames:[]});
    return effect(path,init);
  };
}
test("status bridge publishes current-generation bounded facts without Fetch",async()=>{
  let seen=null,calls=0;
  const f=fixture(async(...args)=>{calls++;return authFetcher()(...args);},{onStatus:v=>seen=v});
  await f.transport.file("tabula-online-attach.txt");
  const value={generation:0,seat:1,revision:4,status:"Game over / Black wins",connection:"Connected"};
  await f.transport.file("tabula-online-status/"+hex(JSON.stringify(value)));
  assert.equal(seen.status,value.status);assert.equal(calls,3);
  await f.transport.file("tabula-online-status/"+hex(JSON.stringify({...value,generation:99,status:"stale"})));
  assert.equal(seen.status,value.status);
  await assert.rejects(f.transport.file("tabula-online-status/"+hex(JSON.stringify({...value,revision:-1}))));
});

test("401/403, malformed JSON and body failure terminally conceal without exposing bodies",async()=>{
  for(const scenario of [401,403,"json","body"]){
    let concealed=0,status=0;
    const f=fixture(async()=>{
      if(typeof scenario==="number") return response("private-body",{status:scenario});
      if(scenario==="json") return response("{private-grant");
      return {status:200,redirected:false,headers:new Headers({"Cache-Control":"no-store","Content-Type":"application/json"}),body:{getReader(){return {async read(){throw new Error("private-body");},releaseLock(){}};}}};
    },{onUnavailable(){concealed++;},onStatus(){status++;}});
    await assert.rejects(f.transport.file("tabula-online-attach.txt"),error=>!error.message.includes("private-"));
    assert.equal(concealed,1);
    await assert.rejects(f.transport.file("tabula-online-status/"+hex(JSON.stringify({seat:0,revision:0,status:"old",connection:"old"}))),/retired/);
    assert.equal(status,0); assert.equal(concealed,1);
  }
});
test("deadline aborts an uncooperative command, retains unknown intent and retires late success",async()=>{
  let timeout,resolve,recovering=0,aborted=false;
  const f=fixture(authFetcher((_path,init)=>{init.signal.addEventListener("abort",()=>aborted=true);return new Promise(ok=>resolve=ok);}),{onRecovering(){recovering++;},timers:{setTimeout(fn,ms){if(ms===30000)timeout=fn;return 1;},clearTimeout(){}}});
  await f.transport.file("tabula-online-attach.txt");
  const pending=f.transport.file("tabula-online-command/"+hex('{"seq":1}'));
  await Promise.resolve();timeout();
  assert.deepEqual(JSON.parse(new TextDecoder().decode(await pending)),{transport:"recovering"});
  assert.equal(aborted,true);assert.equal(recovering,1);
  resolve(response({version:2,frames:[]}));await Promise.resolve();
  f.transport.retire();
});

test("Rust unavailable notification and malformed status retire transport irreversibly",async()=>{
  for(const operation of ["tabula-online-unavailable.txt","tabula-online-status/"+hex("{malformed"),"tabula-online-status/"+hex(JSON.stringify({seat:0,revision:0,status:"x",connection:"x",secret:"no"}))]){
    let concealed=0,status=0;
    const f=fixture(()=>{throw new Error("no Fetch");},{onUnavailable(){concealed++;},onStatus(){status++;}});
    await assert.rejects(f.transport.file(operation));
    assert.equal(concealed,1);
    await assert.rejects(f.transport.file("tabula-online-status/"+hex(JSON.stringify({seat:0,revision:0,status:"old",connection:"old"}))));
    assert.equal(status,0);
  }
});

test("pending refresh hint has no credential/grant/projection and preserves opaque identity",async()=>{
  const storage=memoryStorage(),key="tabula.pending.v2."+id;
  const command='{"seq":1,"command":{"match_id":340282366920938463463374607431768211450}}';
  const first=fixture(authFetcher(()=>{throw new Error("network");}),{storage});
  await first.transport.file("tabula-online-attach.txt");
  assert.deepEqual(JSON.parse(new TextDecoder().decode(await first.transport.file("tabula-online-command/"+hex(command)))),{transport:"recovering"});
  const saved=JSON.parse(storage.getItem(key));
  assert.equal(saved.command,command);assert.equal(saved.operation_scope,scope);
  for(const forbidden of ["csrf_token","binding_id","view","attachment_id","credential"])assert.equal(saved[forbidden],undefined);
  assert.ok(!storage.getItem(key).includes("A".repeat(43))&&!storage.getItem(key).includes("B".repeat(100)));
  first.transport.retire();
  const second=fixture(authFetcher(),{storage});
  const restored=JSON.parse(new TextDecoder().decode(await second.transport.file("tabula-online-attach.txt")));
  assert.equal(restored.pending.command,command);assert.equal(restored.pending.operation_scope,scope);
  await second.transport.file("tabula-online-settled.txt");assert.equal(storage.getItem(key),null);
});
test("recovery obtains fresh context/grant and never replays a command in JavaScript",async()=>{
  const paths=[];let failures=0;
  const f=fixture(async(path,init)=>{paths.push(path);return authFetcher(()=>{if(failures++===0)throw new Error("drop");return response({version:2,frames:[]});})(path,init);});
  await f.transport.file("tabula-online-attach.txt");
  await f.transport.file("tabula-online-command/"+hex('{"seq":1}'));
  const fresh=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-recover.txt")));
  assert.equal(fresh.transport,"resync");assert.equal(fresh.bootstrap.pending.command,'{"seq":1}');
  assert.equal(paths.filter(p=>p.endsWith("/command")).length,1);
  assert.equal(paths.filter(p=>p.endsWith("/context")).length,2);
  assert.equal(paths.filter(p=>p.endsWith("/grant")).length,2);
});
test("expired/malformed pending storage is unknown and never supplies replay bytes",async()=>{
  for(const raw of ["{bad",JSON.stringify({version:2,match_id:id,game_id:game,game_version:"1.0.0",operation_scope:scope,command:'{"seq":1}',expires_at:1})]){
    const storage=memoryStorage();storage.setItem("tabula.pending.v2."+id,raw);
    const f=fixture(authFetcher(),{storage});
    const fresh=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-attach.txt")));
    assert.equal(fresh.pending_unknown,true);assert.equal(fresh.pending,null);
  }
});
test("inaccessible storage blocks submission before any command transmission",async()=>{
  let sent=0;
  const f=fixture(authFetcher(()=>{sent++;return response({version:2,frames:[]});}),{storage:{getItem:()=>null,setItem(){throw new Error("quota");},removeItem(){}}});
  await f.transport.file("tabula-online-attach.txt");
  await assert.rejects(f.transport.file("tabula-online-command/"+hex('{"seq":1}')),/recovery/);
  assert.equal(sent,0);
});
test("six failed fresh-authority attempts stop with an explicit retry and keep the unknown hint",async()=>{
  const storage=memoryStorage();let requests=0,unknown=false;
  const f=fixture(authFetcher(()=>{throw new Error("drop");}),{storage,onUnavailable:v=>unknown=v.unknown});
  await f.transport.file("tabula-online-attach.txt");
  await f.transport.file("tabula-online-command/"+hex('{"seq":1}'));
  // Replace context after initial send failure; only six bounded context retries run.
  const g=fixture(()=>{requests++;throw new Error("offline");},{storage,onUnavailable:v=>unknown=v.unknown});
  await assert.rejects(g.transport.file("tabula-online-attach.txt"),/explicit retry/);
  assert.equal(requests,7);assert.equal(unknown,true);
  assert.ok(storage.getItem("tabula.pending.v2."+id));
  f.transport.retire();
});
test("Rust capacity resync conceals immediately and preserves original intent without HTTP",async()=>{
  let calls=0,recovering=0,seen=0;
  const storage=memoryStorage();
  const f=fixture(async(...args)=>{calls++;return authFetcher()(...args);},{storage,onRecovering(){recovering++;},onStatus(){seen++;}});
  await f.transport.file("tabula-online-attach.txt");
  await f.transport.file("tabula-online-command/"+hex('{"seq":1}'));
  const before=calls;
  await f.transport.file("tabula-online-conceal.txt");
  await f.transport.file("tabula-online-status/"+hex(JSON.stringify({generation:0,seat:0,revision:0,status:"old",connection:"old"})));
  assert.equal(calls,before);assert.equal(recovering,1);assert.equal(seen,0);
  assert.equal(JSON.parse(storage.getItem("tabula.pending.v2."+id)).command,'{"seq":1}');
  f.transport.retire();
});

test("window focus-only resume conceals old authority before fresh attach and ignores dialog focus", async()=>{
  const events=new Map(), document={visibilityState:"visible",addEventListener(){},removeEventListener(){}};
  const lifecycle={document,addEventListener:(name,fn)=>events.set(name,fn),removeEventListener:(name)=>events.delete(name)};
  let concealed=0, seen=0, calls=0;
  const f=fixture(async(...args)=>{calls++;return authFetcher()(...args);},{lifecycle,onRecovering(){concealed++;},onStatus(){seen++;}});
  await f.transport.file("tabula-online-attach.txt");
  const status=generation=>"tabula-online-status/"+hex(JSON.stringify({seat:0,revision:0,status:"current",connection:"Connected",generation}));
  events.get("focus")({target:{tagName:"DIALOG"}});
  assert.equal(concealed,0);
  await f.transport.file(status(0));assert.equal(seen,1);
  events.get("blur")({target:lifecycle});assert.equal(concealed,1);
  await f.transport.file(status(0));assert.equal(seen,1);
  events.get("focus")({target:lifecycle});assert.equal(concealed,2);
  await f.transport.file(status(0));assert.equal(seen,1);
  const fresh=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-poll.txt")));
  assert.equal(fresh.transport,"resync");assert.equal(calls,6);
  await f.transport.file(status(fresh.bootstrap.transport_generation));assert.equal(seen,2);
  f.transport.retire();assert.equal(events.has("focus"),false);assert.equal(events.has("blur"),false);
});

test("retired local attachment409 preserves exact pending intent until fresh same-scope Rust resync",async()=>{
  const storage=memoryStorage(),key="tabula.pending.v2."+id,paths=[];
  const command='{"seq":1,"command":{"match_id":340282366920938463463374607431768211450}}';
  const f=fixture(async(path,init)=>{paths.push(path);return authFetcher(()=>response({code:"reattach_required"},{status:409}))(path,init);},{storage});
  await f.transport.file("tabula-online-attach.txt");
  assert.deepEqual(JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-command/"+hex(command)))),{transport:"recovering"});
  assert.equal(JSON.parse(storage.getItem(key)).command,command);
  const fresh=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-recover.txt")));
  assert.equal(fresh.transport,"resync");assert.equal(fresh.bootstrap.pending.operation_scope,scope);
  assert.equal(fresh.bootstrap.pending.command,command);
  assert.equal(paths.filter(path=>path.endsWith("/command")).length,1,"JavaScript must never replay pending intent");
  f.transport.retire();
});

test("verified stale-CSRF403 keeps pending intent and revalidates context before any Rust replay",async()=>{
  const paths=[],storage=memoryStorage(),command='{"seq":1}',newToken="C".repeat(43);let contexts=0;
  const f=fixture(async(path,init)=>{
    paths.push({path,init});
    if(path.endsWith("/context"))return response({version:1,disposition:"authenticated",csrf_token:contexts++===0?"A".repeat(43):newToken});
    if(path.endsWith("/command"))return response('{"code":"request_rejected"}',{status:403,headers:{"Cache-Control":"no-store","Content-Type":"application/problem+json"}});
    return authFetcher()(path,init);
  },{storage});
  await f.transport.file("tabula-online-attach.txt");
  assert.deepEqual(JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-command/"+hex(command)))),{transport:"recovering"});
  const fresh=JSON.parse(new TextDecoder().decode(await f.transport.file("tabula-online-recover.txt")));
  assert.equal(fresh.bootstrap.pending.command,command);assert.equal(fresh.bootstrap.pending.operation_scope,scope);
  assert.equal(paths.filter(call=>call.path.endsWith("/command")).length,1);
  assert.equal(paths.at(-1).init.headers["X-Tabula-CSRF"],newToken);
  f.transport.retire();
});
test("nonexact or untrusted403 and genuine401 cannot become automatic context recovery",async()=>{
  const invalid=[
    ['{"code":"request_rejected","code":"request_rejected"}',403,true],
    ['{"code":"request_rejected","secret":"private-body"}',403,true],
    ['{"code":"match_unavailable"}',403,true],
    ['\uFEFF{"code":"request_rejected"}',403,true],
    ['\u000b{"code":"request_rejected"}',403,true],
    ['{"code":"request_rejected"}',403,false],
    ['{"code":"request_rejected"}',401,true],
    ['{"code":"request_rejected"}'+" ".repeat(1024),403,true],
  ];
  for(const [body,status,noStore] of invalid){
    const f=fixture(authFetcher(()=>response(body,{status,headers:{"Content-Type":"application/problem+json",...(noStore?{"Cache-Control":"no-store"}:{})}})));
    await f.transport.file("tabula-online-attach.txt");
    await assert.rejects(f.transport.file("tabula-online-command/"+hex('{"seq":1}')),error=>!error.message.includes("private-body"));
    await assert.rejects(f.transport.file("tabula-online-recover.txt"),/retired/);
  }
});
