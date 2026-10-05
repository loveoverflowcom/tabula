const test = require("node:test"), assert = require("node:assert/strict");
const direct = require("../direct-transport.js"), launch = require("../launch-options.js");
const id = "00000000000000000000000000000007", game = "com.tabula.chess"; // xtask-allow-game-id: existing game-client leaf binding, not platform dispatch.
const hex = text => Buffer.from(text).toString("hex");
const response = (body, extra={}) => new Response(typeof body === "string" ? body : JSON.stringify(body), {status:200,headers:{"Cache-Control":"no-store","Content-Type":"application/json"},...extra});
function fixture(fetcher) {
  const controller = new AbortController();
  return {controller,transport:direct.create({matchId:id,gameId:game,signal:controller.signal,current:()=>true,protocol:"https:",fetcher})};
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
  const calls=[],token="A".repeat(43),grant="A".repeat(100),frames={version:1,frames:[]};
  const f=fixture(async(path,init)=>{
    calls.push({path,init});
    if(path.endsWith("/context"))return response({version:1,disposition:"authenticated",csrf_token:token});
    if(path.endsWith("/grant"))return response({version:1,ready:true,binding_id:grant,seat:0,game_id:game,game_version:"1.0.0"});
    if(path.endsWith("/attach"))return response({version:1,attachment_id:id,seat:0,next_seq:1,frames:[]});
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
    const f=fixture(async(path)=>path.endsWith("/context")?response({version:1,disposition:"authenticated",csrf_token:"A".repeat(43)}):wrong?response({version:1,ready:true,binding_id:"A".repeat(100),seat:0,game_id:"org.example.other",game_version:"1.0.0"}):response({},{headers:{"Content-Type":"application/json"}}));
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
  await assert.rejects(pending,/retired/);assert.equal(calls,1);
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
test("status bridge publishes only bounded presenter facts and never performs Fetch",async()=>{
  let seen=null;
  const t=direct.create({matchId:id,gameId:game,signal:new AbortController().signal,current:()=>true,protocol:"https:",fetcher:()=>{throw new Error("unexpected HTTP");},onStatus:value=>seen=value});
  await t.file("tabula-online-status/"+hex(JSON.stringify({seat:1,revision:4,status:"Game over / Black wins",connection:"Connected"})));
  assert.equal(seen.status,"Game over / Black wins");assert.equal(seen.seat,1);
  await assert.rejects(t.file("tabula-online-status/"+hex(JSON.stringify({seat:0,revision:-1,status:"x",connection:"x"}))));
});
