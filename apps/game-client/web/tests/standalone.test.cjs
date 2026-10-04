"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const launch = require("../launch-options.js");
const source = (name) => fs.readFileSync(path.join(__dirname, "..", name), "utf8");
const media = (preferences = []) => (query) => ({matches:preferences.includes(query),addEventListener(){}});
const tick = () => new Promise((resolve) => setImmediate(resolve));
function element(id) {
  return {id,hidden:false,value:"",textContent:"",dataset:{},listeners:{},attributes:{},focus(){this.focused=true;},setAttribute(key,value){this.attributes[key]=value;},removeAttribute(key){delete this.attributes[key];},addEventListener(type,fn){(this.listeners[type] ??= []).push(fn);},dispatch(type,event={}){for(const fn of this.listeners[type]??[])fn(event);},showModal(){this.open=true;},close(){this.open=false;this.dispatch("close");}};
}
function dom(html) {
  const elements = new Map([...html.matchAll(/id="([^"]+)"/g)].map((match) => [match[1],element(match[1])]));
  const translations = [...html.matchAll(/data-i18n="([^"]+)"/g)].map((match) => ({dataset:{i18n:match[1]},textContent:""}));
  const start = element("start");
  const document={documentElement:{dataset:{}},getElementById:id=>elements.get(id),querySelector:()=>start,querySelectorAll:()=>translations};
  for (const item of [...elements.values(), start]) item.focus=()=>{item.focused=true;document.activeElement=item;};
  return {elements,start,document};
}
test("defaults are local Fischer 5+2 and bounded arguments", () => {
  const config = launch.resolve(launch.parse(""),media());
  assert.equal(config.clock,"fischer");
  assert.equal(config.initialMs,300000);
  assert.equal(config.incrementMs,2000);
  assert.equal(config.locale,"vi");
  assert.match(launch.argumentsFor(config),/^--game\nchess\n--skip-setup\n--clock\nfischer/);
  assert.equal(launch.argumentsFor(config).split("\n").length,11);
});
test("Bronstein and untimed produce only their applicable clock args", () => {
  const bronstein = launch.resolve(launch.parse("clock=bronstein&initial-ms=1000&delay-ms=60000"),media());
  assert.match(launch.argumentsFor(bronstein),/--delay-ms\n60000/);
  assert.doesNotMatch(launch.argumentsFor(bronstein),/increment/);
  const untimed = launch.resolve(launch.parse("clock=untimed"),media());
  assert.doesNotMatch(launch.argumentsFor(untimed),/initial-ms|increment-ms|delay-ms/);
});
test("registry namespaced setup arguments share the same bounds", () => {
  const config=launch.parse("chess.clock=bronstein&chess.initial-ms=10800000&chess.delay-ms=0");
  assert.equal(config.initialMs,10800000);
  assert.equal(config.delayMs,0);
  assert.deepEqual(launch.parse(launch.query(config)),config);
});
test("unknown, duplicate, conflicting, credential, mode and hostile numeric values fail closed", () => {
  for(const query of ["token=secret","game=tiles","mode=online","clock=ai","theme=purple","motion=full","locale=fr","initial-ms=-1","initial-ms=1e9","initial-ms=0","initial-ms=10800001","increment-ms=60001","delay-ms=2.5","clock=fischer&clock=untimed","clock=fischer&chess.clock=bronstein",`clock=${"x".repeat(4096)}`,"initial-ms=999999999999999999999"])assert.throws(()=>launch.parse(query),undefined,query);
});
test("system dark/high contrast and reduced motion reach runtime args", () => {
  const config=launch.resolve(launch.parse(""),media(["(prefers-color-scheme: dark)","(prefers-contrast: more)","(prefers-reduced-motion: reduce)"]));
  assert.equal(config.resolvedTheme,"hc-dark");
  assert.match(launch.argumentsFor(config),/--theme\nhc-dark/);
  assert.match(launch.argumentsFor(config),/--reduced-motion$/);
  assert.equal(launch.resolve(launch.parse("theme=light"),media(["(forced-colors: active)"])).resolvedTheme,"light");
});
function setupHarness(search="") {
  const mock=dom(source("index.html"));
  const navigation=[];
  const window=element("window");
  const context=vm.createContext({document:mock.document,location:{search,assign:url=>navigation.push(url)},TabulaLaunch:launch,matchMedia:media(),window,URLSearchParams,Number,Error,Math});
  vm.runInContext(source("setup.js"),context);
  return {...mock,navigation,window};
}
test("setup submits exactly once, carries edited config and recovers on Back", () => {
  const mock=setupHarness("locale=en");
  mock.elements.get("clock").value="bronstein";
  mock.elements.get("initial").value="10";
  mock.elements.get("adjustment").value="3";
  mock.elements.get("setup").dispatch("input");
  const event={preventDefault(){}};
  mock.elements.get("setup").dispatch("submit",event);
  mock.elements.get("setup").dispatch("submit",event);
  assert.equal(mock.navigation.length,1);
  const config=launch.parse(mock.navigation[0].split("?")[1]);
  assert.equal(config.initialMs,600000);
  assert.equal(config.delayMs,3000);
  assert.equal(mock.start.disabled,true);
  mock.window.dispatch("pageshow");
  assert.equal(mock.start.disabled,false);
  mock.elements.get("setup").dispatch("submit",event);
  assert.equal(mock.navigation.length,2);
});
test("untimed removes and disables timed fields; invalid values cannot navigate", () => {
  const mock=setupHarness();
  mock.elements.get("initial").value="0";
  mock.elements.get("setup").dispatch("submit",{preventDefault(){}});
  assert.equal(mock.navigation.length,0);
  assert.equal(mock.elements.get("setup-error").hidden,false);
  mock.elements.get("clock").value="untimed";
  mock.elements.get("setup").dispatch("input");
  assert.equal(mock.elements.get("initial").disabled,true);
  assert.equal(mock.elements.get("clock-fields").hidden,true);
  mock.elements.get("setup").dispatch("submit",{preventDefault(){}});
  assert.equal(mock.navigation.length,1);
});
// VM mocks exercise host admission/navigation, not a real browser or WASM
// rules execution. Deferred work deliberately ignores abort to test stale gates.
async function runtimeHarness({search="locale=en",httpStatus=200,missingImport=false,throwFrame=false,pinned=false,deferStage,pathname="/standalone/play.html",responseHeaders={},streamChunks,versionMismatch=false,missingFrame=false,navigationThrows=0}={}) {
  const mock=dom(source("play.html"));
  mock.elements.get("runtime-error").hidden=true;
  const events=element("window");
  const navigation=[];
  const callbacks=[];
  const loaded=[];
  const fetches=[];
  const calls={compile:0,instantiate:0,main:0,cancelReader:0,releaseReader:0,navigation:0};
  function navigate(target){calls.navigation++;if(navigationThrows-->0)throw new Error("Navigation blocked");navigation.push(target);}
  let release;
  const gate=new Promise(resolve=>{release=resolve;});
  let frames=0;
  const rustKeys=[];
  const focusChanges=[];
  const inputState={held:false,dragging:false};
  class Memory {}
  const rawExports={memory:new Memory(),table:{kind:"fake-table"},crate_version:()=>versionMismatch?999:2,main(){calls.main++;},focus(value){focusChanges.push(value);if(!value){inputState.held=false;inputState.dragging=false;}},frame(){if(throwFrame)throw new Error("frame panic");frames++;},key_down:(key)=>rustKeys.push(key),key_press(){},file_loaded:id=>loaded.push(id)};
  if(missingFrame)delete rawExports.frame;
  const imports={env:{fs_load_file:()=>999}};
  const context=vm.createContext({
    document:mock.document,
    location:{search,pathname,href:`https://tabula.test${pathname}`,origin:"https://tabula.test",assign:navigate,reload:()=>navigate("reload")},
    matchMedia:media(),TabulaLaunch:launch,URL,URLSearchParams,AbortController,TextEncoder,Uint8Array,Error,Number,Map,
    console:{error(){},warn(){},log(){}},window:events,plugins:[],version:2,wasm_memory:null,wasm_exports:null,
    FS:{unique_id:1,loaded_files:{}},UTF8ToString:(name)=>name,importObject:imports,animation_frame_timeout:1,
    cancelAnimationFrame(){},clearTimeout(){},
    setTimeout(fn,delay){if(delay>1000){callbacks.push(fn);return 1;}Promise.resolve().then(fn);return 2;},
    fetch:async(url,options)=>{
      fetches.push({url,options});
      const isArtifact=url==="tabula-game-client.wasm";
      if((isArtifact&&deferStage==="fetch")||(!isArtifact&&deferStage==="asset"))await gate;
      let chunk=0;
      return {ok:httpStatus===200,status:httpStatus,headers:{get:key=>responseHeaders[key]??null},
        body:streamChunks?{getReader:()=>({
          async read(){if(deferStage==="stream")await gate;return chunk<streamChunks.length?{done:false,value:streamChunks[chunk++]}:{done:true};},
          async cancel(){calls.cancelReader++;},releaseLock(){calls.releaseReader++;}
        })}:null,
        async arrayBuffer(){if(deferStage==="body")await gate;return new Uint8Array([0,97,115,109,1,0,0,0]).buffer;}
      };
    },
    WebAssembly:{Memory,
      compile:async()=>{calls.compile++;if(deferStage==="compile")await gate;return {};},
      Module:{imports:()=>missingImport?[{module:"env",name:"missing"}]:[]},
      instantiate:async()=>{calls.instantiate++;if(deferStage==="instantiate")await gate;return {exports:rawExports};}
    },
    miniquad_add_plugin(plugin){context.plugins.push(plugin);},
    register_plugins(plugins){for(const p of plugins)p.register_plugin(imports);},init_plugins(){},
    animation(){context.wasm_exports?.frame?.();}
  });
  if(pinned) {
    mock.document.querySelector=()=>mock.elements.get("glcanvas");
    mock.document.addEventListener=()=>{};
    mock.document.hasFocus=()=>true;
    mock.document.visibilityState="visible";
    events.requestAnimationFrame=()=>1;events.cancelAnimationFrame=()=>{};
    vm.runInContext(source("mq_js_bundle.js"),context);
  }
  mock.document.hasFocus=()=>true;mock.document.visibilityState="visible";
  vm.runInContext(source("bootstrap.js"),context);
  for(let i=0;i<5;i++)await tick();
  return {...mock,context,events,imports,navigation,callbacks,loaded,rustKeys,focusChanges,inputState,fetches,calls,release,rawExports,frames:()=>frames};
}
async function admitBoard(mock) {
  mock.imports.env.fs_load_file("tabula-ready.txt",16);
  await tick();
  mock.context.animation();
}
test("download and initial async frames never falsely declare board readiness", async () => {
  const mock=await runtimeHarness();
  mock.context.animation();
  assert.equal(mock.elements.get("loader").hidden,false);
  const id=mock.imports.env.fs_load_file("tabula-launch.txt",17);
  await tick();
  assert.ok(mock.loaded.includes(id));
  assert.match(new TextDecoder().decode(mock.context.FS.loaded_files[id]),/--skip-setup/);
  const assetId=mock.imports.env.fs_load_file("other-asset.png",15);
  await tick();
  assert.ok(mock.loaded.includes(assetId));
  assert.equal(mock.context.FS.loaded_files[assetId].byteLength,8);
  const readyId=mock.imports.env.fs_load_file("tabula-ready.txt",16);
  await tick();
  assert.ok(mock.loaded.includes(readyId));
  mock.context.animation();
  assert.equal(mock.elements.get("loader").hidden,true);
  assert.equal(mock.elements.get("glcanvas").focused,true);
  mock.imports.env.fs_load_file("tabula-ready.txt",16);
  await tick();mock.context.animation();
  assert.equal(mock.elements.get("runtime-error").hidden,true);
});
test("HTTP failure, unknown imports and startup timeout expose recovery", async () => {
  for(const options of [{httpStatus:404},{missingImport:true},{search:"mode=online"}]) {
    const mock=await runtimeHarness(options);
    assert.equal(mock.elements.get("runtime-error").hidden,false);
    assert.equal(mock.elements.get("loader").hidden,true);
    assert.equal(mock.elements.get("error-back").focused,true);
    assert.ok(mock.elements.get("error-detail").textContent.length>0);
  }
  const mock=await runtimeHarness();mock.callbacks[0]();
  assert.equal(mock.elements.get("runtime-error").hidden,false);
});
test("frame panic and context loss never silently remount or retain ready state", async () => {
  const mock=await runtimeHarness({throwFrame:true});
  mock.context.animation();
  assert.equal(mock.elements.get("runtime-error").hidden,false);
  assert.equal(mock.navigation.length,0);
  const context=await runtimeHarness();
  let prevented=false;
  context.elements.get("glcanvas").dispatch("webglcontextlost",{preventDefault(){prevented=true;}});
  assert.equal(prevented,true);
  assert.equal(context.elements.get("runtime-error").hidden,false);
});
test("leave requires a safe dialog, cancel restores focus, retry is explicit reload", async () => {
  const mock=await runtimeHarness();
  mock.imports.env.fs_load_file("tabula-ready.txt",16);await tick();mock.context.animation();
  mock.elements.get("leave").dispatch("click");
  assert.equal(mock.elements.get("leave-dialog").open,true);
  assert.equal(mock.navigation.length,0);
  mock.elements.get("stay").dispatch("click");
  assert.equal(mock.elements.get("leave-dialog").open,false);
  assert.equal(mock.elements.get("glcanvas").focused,true);
  mock.elements.get("confirm-leave").dispatch("click");
  assert.match(mock.navigation[0],/^index.html\?/);
  const retry=await runtimeHarness({httpStatus:404});retry.elements.get("retry").dispatch("click");
  assert.deepEqual(retry.navigation,["reload"]);
});
test("ordinary Tab reaches pinned canvas listener; Shift+Tab explicitly reaches host Leave", async () => {
  const mock=await runtimeHarness({pinned:true});
  const canvas=mock.elements.get("glcanvas");
  mock.context.importObject.env.run_animation_loop(false);
  const pinnedKeydown=canvas.onkeydown;
  assert.equal(typeof pinnedKeydown,"function");
  const dispatch=(shiftKey)=>{
    let stopped=false;let prevented=false;
    const event={code:"Tab",shiftKey,stopImmediatePropagation(){stopped=true;},preventDefault(){prevented=true;}};
    canvas.dispatch("keydown",event);
    if(!stopped)pinnedKeydown(event);
    return {stopped,prevented};
  };
  assert.deepEqual(dispatch(false),{stopped:false,prevented:true});
  assert.deepEqual(mock.rustKeys,[258]);
  assert.deepEqual(dispatch(true),{stopped:true,prevented:true});
  assert.deepEqual(mock.rustKeys,[258]);
  assert.equal(mock.elements.get("leave").focused,true);
  let escapeStopped=false;
  canvas.dispatch("keydown",{code:"Escape",shiftKey:false,stopImmediatePropagation(){escapeStopped=true;}});
  assert.equal(escapeStopped,false);
});

test("canvas blur clears held-key/drag state before dialog controls and return re-focuses", async () => {
  const mock=await runtimeHarness();
  const canvas=mock.elements.get("glcanvas");
  canvas.dispatch("blur");
  assert.deepEqual(mock.focusChanges,[]); // No calls before admitted readiness.
  mock.imports.env.fs_load_file("tabula-ready.txt",16);await tick();mock.context.animation();
  mock.inputState.held=true;mock.inputState.dragging=true;
  canvas.dispatch("blur"); // Browser blur from Shift+Tab or showModal.
  mock.elements.get("leave").dispatch("click");
  assert.equal(mock.inputState.held,false);
  assert.equal(mock.inputState.dragging,false);
  assert.equal(mock.focusChanges.at(-1),false);
  mock.elements.get("stay").dispatch("click");
  canvas.dispatch("focus");
  assert.equal(mock.focusChanges.at(-1),true);
  mock.document.visibilityState="hidden";
  canvas.dispatch("focus");
  assert.equal(mock.focusChanges.at(-1),false);
});

test("documents allow zoom, keep board separate and do not load remote scripts", () => {
  for(const name of ["index.html","play.html"]) {
    const html=source(name);
    assert.doesNotMatch(html,/maximum-scale|user-scalable=no|<iframe|<script[^>]*src="https?:/);
  }
  assert.doesNotMatch(source("index.html"),/<canvas|mq_js_bundle/);
  assert.match(source("play.html"),/id="glcanvas" tabindex="0"/);
  assert.match(source("index.html"),/value="ai" disabled/);
  assert.match(source("index.html"),/value="online" disabled/);
});

const integrated=(clock="fischer",extra={})=>new URLSearchParams({game:"com.tabula.chess",mode:"local",seats:"2",clock,source:"tabula",return_to:"/games/com.tabula.chess?setup=1",locale:"en",...(clock==="fischer"?{initial_ms:"300000",increment_ms:"2000"}:clock==="bronstein"?{initial_ms:"300000",delay_ms:"2000"}:{}),...extra}).toString();
test("integrated registry handoffs round-trip and transmit no shell metadata to Rust",()=>{
  for(const clock of ["untimed","fischer","bronstein"]){
    const config=launch.resolve(launch.parse(integrated(clock)),media());
    assert.equal(config.source,"tabula");
    assert.equal(config.returnTo,"/games/com.tabula.chess?setup=1");
    assert.deepEqual(launch.parse(launch.query(config)),launch.parse(integrated(clock)));
    const args=launch.argumentsFor(config);
    assert.match(args,/--game\nchess\n--skip-setup/);
    assert.doesNotMatch(args,/tabula|return|source|seats|mode|locale|\/games/);
  }
});
test("integrated configuration rejects unsupported modes, wrong identities, incomplete and irrelevant clock fields",()=>{
  for(const extra of [{mode:"online"},{mode:"ai"},{mode:"rated"},{seats:"3"},{seats:"02"},{game:"chess"},{game:"com.tabula.tiles"},{locale:"fr"},{clock:"ai"},{delay_ms:"2000"},{"initial-ms":"300000"},{"chess.clock":"fischer"},{initial_ms:"0"},{initial_ms:"10800001"},{increment_ms:"60001"},{initial_ms:"1e6"},{token:"secret"}])assert.throws(()=>launch.parse(integrated("fischer",extra)),undefined,JSON.stringify(extra));
  for(const key of ["game","mode","seats","clock","locale","initial_ms","increment_ms","source","return_to"]){
    const query=new URLSearchParams(integrated());query.delete(key);
    assert.throws(()=>launch.parse(query.toString()),undefined,key);
  }
  assert.throws(()=>launch.parse(integrated("untimed",{initial_ms:"300000"})));
  assert.throws(()=>launch.parse(integrated("bronstein",{increment_ms:"2000"})));
  for(const clock of ["fischer","bronstein"]){
    const config=launch.parse(integrated(clock,{initial_ms:"10800000",[clock==="fischer"?"increment_ms":"delay_ms"]:"60000"}));
    assert.equal(config.initialMs,10800000);
  }
});
test("return metadata admits only the exact paired same-origin shell setup destination",()=>{
  for(const path of ["https://evil.test/games/com.tabula.chess?setup=1","//evil.test","/games/com.tabula.chess","/games/com.tabula.chess?setup=1&token=x","/games/com.tabula.chess?setup=1#x","/games/../settings?setup=1","/games/com.tabula.tiles?setup=1","/games/%63om.tabula.chess?setup=1","\\\\evil.test","javascript:alert(1)","/games/com.tabula.chess?setup=01"]){
    assert.throws(()=>launch.parse(integrated("fischer",{return_to:path})),undefined,path);
  }
  for(const suffix of ["&source=tabula","&return_to=%2Fgames%2Fcom.tabula.chess%3Fsetup%3D1","&clock=fischer"]){
    assert.throws(()=>launch.parse(integrated()+suffix));
  }
  assert.throws(()=>launch.navigation("source=external&return_to=/games/com.tabula.chess?setup=1"));
});
test("safe shell return remains usable during loading, bad gameplay config, and retired runtime failure",async()=>{
  for(const options of [{deferStage:"fetch"},{httpStatus:404},{missingImport:true},{versionMismatch:true},{missingFrame:true},{search:integrated("fischer",{mode:"online"})}]){
    const mock=await runtimeHarness({search:integrated(),pathname:"/play/local/index.html",...options});
    assert.equal(mock.elements.get("cancel-load").href,"/games/com.tabula.chess?setup=1");
    assert.equal(mock.elements.get("error-back").href,"/games/com.tabula.chess?setup=1");
    assert.equal(mock.elements.get("error-back").textContent,"Return to Tabula");
    mock.elements.get(options.deferStage?"cancel-load":"error-back").dispatch("click",{preventDefault(){}});
    assert.deepEqual(mock.navigation,["/games/com.tabula.chess?setup=1"]);
    mock.release();await tick();
    assert.equal(mock.calls.main,0);
  }
  const hostile=await runtimeHarness({search:integrated("fischer",{return_to:"https://evil.test"})});
  assert.equal(hostile.elements.get("error-back").href,"/games");
  assert.equal(hostile.calls.compile,0);
});
test("active host return confirms once, keeps integrated launch on retry, and returns directly to shell",async()=>{
  const mock=await runtimeHarness({search:integrated(),pathname:"/play/local/index.html"});
  await admitBoard(mock);
  assert.equal(mock.elements.get("leave").textContent,"Return to Tabula");
  assert.match(mock.elements.get("help-detail").textContent,/Clocks keep running/);
  mock.elements.get("leave").dispatch("click");
  assert.equal(mock.elements.get("leave-dialog").open,true);
  assert.deepEqual(mock.navigation,[]);
  mock.elements.get("confirm-leave").dispatch("click");
  mock.elements.get("confirm-leave").dispatch("click");
  assert.deepEqual(mock.navigation,["/games/com.tabula.chess?setup=1"]);
  const retry=await runtimeHarness({search:integrated(),httpStatus:404});
  retry.elements.get("retry").dispatch("click");retry.elements.get("retry").dispatch("click");
  assert.deepEqual(retry.navigation,["reload"]);
  assert.equal(retry.context.location.search,integrated());
});
test("every asynchronous startup stage is stale after loading cancel or actual Back/close",async()=>{
  for(const stage of ["fetch","body","stream","compile","instantiate"]){
    for(const close of [false,true]){
      const mock=await runtimeHarness({search:integrated(),deferStage:stage,...(stage==="stream"?{streamChunks:[new Uint8Array([0,97,115,109,1,0,0,0])]}:{})});
      const before={...mock.calls};
      if(close)mock.events.dispatch("pagehide",{persisted:true});
      else mock.elements.get("cancel-load").dispatch("click",{preventDefault(){}});
      assert.equal(mock.fetches[0].options?.signal?.aborted,true,`${stage}: cancelled download must abort its signal`);
      mock.release();for(let i=0;i<5;i++)await tick();
      assert.equal(mock.calls.main,0,`${stage} called main after disposal`);
      assert.equal(mock.calls.instantiate,before.instantiate,`${stage} began a stale instance`);
      assert.equal(mock.elements.get("runtime-error").hidden,true);
      assert.equal(mock.context.wasm_memory,null);
      if(stage==="stream")assert.equal(mock.calls.cancelReader,1);
    }
  }
});
test("canceled unload warning retains game; actual pagehide retires input, callbacks and buffers; BFCache reloads fresh",async()=>{
  const mock=await runtimeHarness({deferStage:"asset"});
  await admitBoard(mock);
  let prevented=false;
  mock.events.dispatch("beforeunload",{preventDefault(){prevented=true;}});
  assert.equal(prevented,true);
  const before=mock.frames();mock.context.animation();assert.equal(mock.frames(),before+1);
  const pendingId=mock.imports.env.fs_load_file("other-asset.png",15);
  const loadedBefore=mock.loaded.length;
  mock.inputState.held=true;mock.inputState.dragging=true;
  mock.events.dispatch("pagehide",{persisted:true});
  assert.equal(mock.inputState.held,false);assert.equal(mock.inputState.dragging,false);
  assert.deepEqual(Object.keys(mock.context.FS.loaded_files),[]);
  assert.equal(mock.context.wasm_exports.table,null);
  assert.equal(mock.fetches.at(-1).options.signal.aborted,true);
  const retiredFrames=mock.frames();
  mock.context.animation();mock.context.wasm_exports.frame();mock.context.wasm_exports.key_down(258);
  assert.equal(mock.frames(),retiredFrames);assert.deepEqual(mock.rustKeys,[]);
  mock.release();await tick();await tick();
  assert.equal(mock.loaded.length,loadedBefore);
  assert.equal(mock.context.FS.loaded_files[pendingId],undefined);
  mock.events.dispatch("pageshow",{persisted:false});assert.deepEqual(mock.navigation,[]);
  mock.events.dispatch("pageshow",{persisted:true});assert.deepEqual(mock.navigation,["reload"]);
});
test("duplicate script starts one instance, and late readiness cannot resurrect failed or closed documents",async()=>{
  const mock=await runtimeHarness();
  vm.runInContext(source("bootstrap.js"),mock.context);
  await tick();
  assert.equal(mock.fetches.length,1);assert.equal(mock.calls.compile,1);assert.equal(mock.calls.instantiate,1);assert.equal(mock.calls.main,1);
  const readyId=mock.imports.env.fs_load_file("tabula-ready.txt",16);
  mock.events.dispatch("pagehide");await tick();mock.context.animation();
  assert.equal(mock.loaded.includes(readyId),false);
  assert.equal(mock.elements.get("loader").hidden,false);
  const failure=await runtimeHarness();
  const stale= failure.imports.env.fs_load_file("tabula-ready.txt",16);
  failure.callbacks[0]();await tick();failure.context.animation();
  assert.equal(failure.loaded.includes(stale),false);
  assert.equal(failure.elements.get("runtime-error").hidden,false);
  assert.equal(failure.elements.get("glcanvas").tabIndex,-1);
});
test("pinned focus callbacks cannot revive input while host dialogs are open, but frames continue",async()=>{
  const mock=await runtimeHarness();await admitBoard(mock);
  mock.inputState.held=true;mock.inputState.dragging=true;
  mock.elements.get("help").dispatch("click");
  mock.context.wasm_exports.focus(true); // Pinned visibility/window callback.
  assert.equal(mock.focusChanges.at(-1),false);
  assert.equal(mock.inputState.held,false);assert.equal(mock.inputState.dragging,false);
  const before=mock.frames();mock.context.animation();assert.equal(mock.frames(),before+1);
  mock.elements.get("close-help").dispatch("click");assert.equal(mock.focusChanges.at(-1),true);
  mock.document.visibilityState="hidden";mock.context.wasm_exports.focus(true);assert.equal(mock.focusChanges.at(-1),false);
});
test("oversized, empty or interrupted streamed download fails boundedly and releases reader",async()=>{
  const advertised=await runtimeHarness({responseHeaders:{"Content-Length":String(64*1024*1024+1)}});
  assert.equal(advertised.elements.get("runtime-error").hidden,false);assert.equal(advertised.calls.compile,0);
  const oversized=await runtimeHarness({streamChunks:[{byteLength:64*1024*1024+1}]});
  assert.equal(oversized.elements.get("runtime-error").hidden,false);assert.equal(oversized.calls.cancelReader,1);assert.equal(oversized.calls.releaseReader,1);
  const empty=await runtimeHarness({streamChunks:[]});
  assert.equal(empty.elements.get("runtime-error").hidden,false);assert.equal(empty.calls.compile,0);assert.equal(empty.calls.releaseReader,1);
  const compressed=await runtimeHarness({responseHeaders:{"Content-Length":"8","Content-Encoding":"gzip"},streamChunks:[new Uint8Array([0,97,115,109,1,0,0,0])]});
  assert.equal(compressed.calls.main,1);assert.equal(compressed.elements.get("load-progress").attributes.value,undefined);
});

test("failed document navigation exposes recovery without reviving the retired runtime",async()=>{
  for(const action of ["return","retry","bfcache"]){
    const mock=await runtimeHarness({search:integrated(),navigationThrows:1});await admitBoard(mock);
    if(action==="return"){
      mock.elements.get("leave").dispatch("click");mock.elements.get("confirm-leave").dispatch("click");
    }else if(action==="retry")mock.elements.get("retry").dispatch("click");
    else{mock.events.dispatch("pagehide",{persisted:true});mock.events.dispatch("pageshow",{persisted:true});}
    assert.equal(mock.elements.get("runtime-error").hidden,false,action);
    assert.equal(Boolean(mock.elements.get("leave-dialog").open),false);
    assert.equal(mock.elements.get("glcanvas").tabIndex,-1);
    assert.match(mock.elements.get("error-detail").textContent,/Navigation blocked/);
    assert.equal(mock.elements.get("error-back").focused,true);
    const frames=mock.frames();mock.context.animation();assert.equal(mock.frames(),frames);
    assert.equal(mock.context.wasm_memory,null);
    mock.elements.get("error-back").dispatch("click",{preventDefault(){}});
    assert.deepEqual(mock.navigation,["/games/com.tabula.chess?setup=1"]);
    assert.equal(mock.calls.navigation,2);
  }
});

test("runtime failure closes modal dialogs before focusing usable recovery",async()=>{
  const mock=await runtimeHarness();await admitBoard(mock);
  mock.elements.get("help").dispatch("click");
  assert.equal(mock.elements.get("help-dialog").open,true);
  mock.events.dispatch("error",{error:new Error("Runtime stopped")});
  assert.equal(mock.elements.get("help-dialog").open,false);
  assert.equal(mock.elements.get("error-back").focused,true);
  assert.equal(mock.elements.get("runtime-error").hidden,false);
});
test("a synchronous failure during a frame cannot resurrect acknowledged readiness",async()=>{
  const mock=await runtimeHarness();
  mock.imports.env.fs_load_file("tabula-ready.txt",16);await tick();
  mock.rawExports.frame=()=>mock.events.dispatch("error",{error:new Error("Runtime stopped during frame")});
  mock.context.animation();
  assert.equal(mock.elements.get("runtime-error").hidden,false);
  assert.equal(mock.elements.get("glcanvas").tabIndex,-1);
  assert.equal(mock.context.wasm_memory,null);
});

test("oversized query fails before game work and preserves a fixed integrated shell return",async()=>{
  const mock=await runtimeHarness({search:"source=tabula&"+"x".repeat(4096),pathname:"/play/local/index.html"});
  assert.equal(mock.calls.compile,0);assert.equal(mock.fetches.length,0);
  assert.equal(mock.elements.get("runtime-error").hidden,false);
  assert.equal(mock.elements.get("error-back").href,"/games");
});
test("preserved colocated standalone gameplay returns to its standalone setup entry",async()=>{
  const mock=await runtimeHarness({search:"locale=en",pathname:"/play/local/play.html"});await admitBoard(mock);
  mock.elements.get("leave").dispatch("click");mock.elements.get("confirm-leave").dispatch("click");
  assert.match(mock.navigation[0],/^standalone.html\?/);
  assert.equal(launch.parse(mock.navigation[0].split("?")[1]).locale,"en");
});

test("integrated index and directory entries require complete handoff metadata without default autorun",async()=>{
  for(const pathname of ["/play/local/","/play/local/index.html"]){
    for(const search of ["","locale=en","game=chess&mode=hot-seat","source=tabula"]){
      const mock=await runtimeHarness({pathname,search});
      assert.equal(mock.elements.get("runtime-error").hidden,false,`${pathname}?${search}`);
      assert.equal(mock.elements.get("error-back").href,"/games");
      assert.equal(mock.fetches.length,0);assert.equal(mock.calls.main,0);
    }
    const valid=await runtimeHarness({pathname,search:integrated("untimed")});
    assert.equal(valid.calls.main,1);assert.equal(valid.elements.get("runtime-error").hidden,true);
  }
  for(const pathname of ["/standalone/play.html","/play/local/play.html"]){
    const standalone=await runtimeHarness({pathname,search:""});
    assert.equal(standalone.calls.main,1);assert.equal(standalone.elements.get("runtime-error").hidden,true);
  }
});
