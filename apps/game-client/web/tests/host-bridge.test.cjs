"use strict";
const {test:runTest} = require("node:test");
const test = (name, body) => runTest(name, {timeout:5000}, body);
const assert = require("node:assert/strict");
const bridge = require("../host-bridge.js");
const vectors = require("./bridge-vectors.json");

test("every host→page vector decodes exactly as specified or is rejected", () => {
  for (const vector of vectors.hostToPage) {
    const result = bridge.decode(vector.text);
    if (vector.accept) {
      assert.equal(result.ok, true, vector.name);
      assert.deepEqual(JSON.parse(JSON.stringify(result.message)), vector.message, vector.name);
    } else assert.equal(result.ok, false, vector.name);
  }
});
test("every page→host vector is produced byte-for-byte by encode, and invalid ones never encode", () => {
  for (const vector of vectors.pageToHost.filter((entry) => entry.accept && !entry.decodeOnly)) assert.equal(bridge.encode(vector.message), vector.text, vector.name);
  for (const vector of vectors.pageToHost.filter((entry) => !entry.accept && !entry.hostOnly)) {
    let parsed;
    try { parsed = JSON.parse(vector.text); } catch (_) { continue; } // hostOnly vectors are ones the page would never emit as-is (encode normalizes or truncates them) but the strict host grammar must still reject; not even JSON: nothing for encode to accept
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) continue;
    const {v, ...message} = parsed;
    if (v === 1 && vector.name !== "duplicate key") assert.throws(() => bridge.encode(message), /Invalid host bridge message/, vector.name);
  }
});
test("oversized and malformed host input is rejected without throwing", () => {
  const big = JSON.stringify({v:1, type:"suspend", gen:1, pad:"x".repeat(bridge.MAX_BYTES)});
  assert.deepEqual(bridge.decode(big), {ok:false, error:"oversized"});
  for (const text of [undefined, null, 7, {}, "{", "null", "[]", '{"v":1}']) assert.equal(bridge.decode(text).ok, false);
  const accessor = Object.defineProperty({}, "type", {get() { throw new Error("must not run"); }});
  assert.equal(bridge.decode(JSON.stringify(accessor)).ok, false);
});
test("encode replaces lone surrogates so the host only sees well-formed text", () => {
  const text = bridge.encode({type:"failed", gen:3, code:"runtime", detail:"a\ud800b\udc00c😀"});
  assert.equal(JSON.parse(text).detail, "a\ufffdb\ufffdc😀");
});
test("encode truncates failure detail to 200 code points and the message stays bounded", () => {
  const text = bridge.encode({type:"failed", gen:3, code:"runtime", detail:"é".repeat(5000)});
  assert.ok(new TextEncoder().encode(text).byteLength <= bridge.MAX_BYTES);
  assert.equal([...JSON.parse(text).detail].length, 200);
});

function port(answer) {
  const sent = [];
  const native = {postMessage(text) { sent.push(JSON.parse(text)); if (answer) Promise.resolve().then(() => answer(native, JSON.parse(text))); }, onmessage: null};
  return {native, sent};
}
const timers = () => { const pending = []; return {pending, setTimeout(fn) { pending.push(fn); return pending.length; }, clearTimeout(id) { pending[id - 1] = null; }}; };
const init = (over = {}) => JSON.stringify({v:1, type:"init", gen:5, capabilities:["keep-awake"], preferences:{theme:"light", motion:"system", locale:"vi"}, ...over});

test("handshake resolves once; a second init and an out-of-generation message are dropped", async () => {
  const dropped = [];
  const {native, sent} = port((n) => n.onmessage({data:init()}));
  const events = [];
  const attached = bridge.attach(native, {dropped:(why) => dropped.push(why), suspend:() => events.push("suspend")}, {timers:timers()});
  const answer = await attached.handshake();
  assert.equal(answer.gen, 5);
  assert.deepEqual(sent, [{v:1, type:"hello"}]);
  native.onmessage({data:init({gen:6})});
  native.onmessage({data:JSON.stringify({v:1, type:"suspend", gen:6})});
  native.onmessage({data:JSON.stringify({v:1, type:"suspend", gen:5})});
  native.onmessage({data:"garbage"});
  assert.deepEqual(dropped, ["duplicate-init", "stale-generation", "malformed"]);
  assert.deepEqual(events, ["suspend"]);
});
test("nothing is sent before init and nothing after dispose", async () => {
  const {native, sent} = port((n) => n.onmessage({data:init()}));
  const attached = bridge.attach(native, {}, {timers:timers()});
  assert.equal(attached.exit(), false);
  assert.equal(attached.ready(10), false);
  await attached.handshake();
  assert.equal(attached.exit(), true);
  native.onmessage({data:JSON.stringify({v:1, type:"dispose", gen:5})});
  assert.equal(attached.disposed, true);
  const count = sent.length;
  assert.equal(attached.exit(), false);
  assert.equal(attached.ready(10), false);
  assert.equal(attached.failed("runtime", "late"), false);
  assert.deepEqual(await attached.service("keep-awake", true), {ok:false, code:"denied"});
  assert.equal(sent.length, count);
});
test("handshake rejects on silence and on an unavailable port", async () => {
  const clock = timers();
  const silent = bridge.attach(port().native, {}, {timers:clock});
  const waiting = silent.handshake();
  clock.pending[0]();
  await assert.rejects(waiting, /did not answer/);
  const broken = bridge.attach({postMessage() { throw new Error("gone"); }, onmessage:null}, {}, {timers:timers()});
  await assert.rejects(broken.handshake(), /unavailable/);
});
test("services are granted per capability, ids correlate replies, and unknown replies are dropped", async () => {
  const dropped = [];
  const {native, sent} = port((n, message) => {
    if (message.type === "hello") n.onmessage({data:init()});
    if (message.type === "service") n.onmessage({data:JSON.stringify({v:1, type:"reply", gen:5, id:message.id, ok:true, code:"ok"})});
  });
  const attached = bridge.attach(native, {dropped:(why) => dropped.push(why)}, {timers:timers()});
  await attached.handshake();
  assert.deepEqual(await attached.service("keep-awake", true), {type:"reply", gen:5, id:1, ok:true, code:"ok"});
  assert.deepEqual(await attached.service("microphone", true), {ok:false, code:"denied"});
  assert.equal(sent.filter((message) => message.type === "service").length, 1, "an ungranted service never reaches the host");
  native.onmessage({data:JSON.stringify({v:1, type:"reply", gen:5, id:42, ok:true, code:"ok"})});
  assert.deepEqual(dropped, ["unknown-reply"]);
});
test("a host that never replies cannot make requests pile up without bound", async () => {
  const {native} = port((n, message) => { if (message.type === "hello") n.onmessage({data:init()}); });
  const attached = bridge.attach(native, {}, {timers:timers()});
  await attached.handshake();
  for (let i = 0; i < 4; i++) attached.service("keep-awake", true);
  assert.deepEqual(await attached.service("keep-awake", true), {ok:false, code:"unsupported"});
});
