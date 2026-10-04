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
  return {elements,start,document:{documentElement:{dataset:{}},getElementById:id=>elements.get(id),querySelector:()=>start,querySelectorAll:()=>translations}};
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
async function runtimeHarness({search="locale=en",httpStatus=200,missingImport=false,throwFrame=false,pinned=false}={}) {
  const mock=dom(source("play.html"));
  mock.elements.get("runtime-error").hidden=true;
  const events=element("window");
  const navigation=[];
  const callbacks=[];
  const loaded=[];
  let frames=0;
  const rustKeys=[];
  const focusChanges=[];
  const inputState={held:false,dragging:false};
  class Memory {}
  const imports={env:{fs_load_file:()=>999}};
  const context=vm.createContext({document:mock.document,location:{search,assign:url=>navigation.push(url),reload:()=>navigation.push("reload")},matchMedia:media(),TabulaLaunch:launch,URLSearchParams,TextEncoder,Uint8Array,Error,Number,Map,console:{error(){},warn(){},log(){}},window:events,plugins:[],version:2,wasm_memory:null,wasm_exports:null,FS:{unique_id:1,loaded_files:{}},UTF8ToString:(name)=>name,importObject:imports,animation_frame_timeout:1,cancelAnimationFrame(){},clearTimeout(){},setTimeout(fn,delay){if(delay>1000){callbacks.push(fn);return 1;}Promise.resolve().then(fn);return 2;},fetch:async()=>({ok:httpStatus===200,status:httpStatus,headers:{get:()=>null},body:null,arrayBuffer:async()=>new Uint8Array([0,97,115,109,1,0,0,0]).buffer}),WebAssembly:{Memory,compile:async()=>({}),Module:{imports:()=>missingImport?[{module:"env",name:"missing"}]:[]},instantiate:async()=>({exports:{memory:new Memory(),crate_version:()=>2,main(){},focus(value){focusChanges.push(value);if(!value){inputState.held=false;inputState.dragging=false;}},frame(){if(throwFrame)throw new Error("frame panic");frames++;},key_down:(key)=>rustKeys.push(key),key_press(){},file_loaded:id=>loaded.push(id)}})},miniquad_add_plugin(plugin){context.plugins.push(plugin);},register_plugins(plugins){for(const p of plugins)p.register_plugin(imports);},init_plugins(){},animation(){context.wasm_exports.frame();}});
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
  return {...mock,context,events,imports,navigation,callbacks,loaded,rustKeys,focusChanges,inputState,frames:()=>frames};
}
test("download and initial async frames never falsely declare board readiness", async () => {
  const mock=await runtimeHarness();
  mock.context.animation();
  assert.equal(mock.elements.get("loader").hidden,false);
  const id=mock.imports.env.fs_load_file("tabula-launch.txt",17);
  await tick();
  assert.ok(mock.loaded.includes(id));
  assert.match(new TextDecoder().decode(mock.context.FS.loaded_files[id]),/--skip-setup/);
  assert.equal(mock.imports.env.fs_load_file("other-asset.png",15),999);
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
