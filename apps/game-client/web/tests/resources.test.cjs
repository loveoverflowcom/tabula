"use strict";
const {test:runTest} = require("node:test");
const test = (name, body) => runTest(name, {timeout:5000}, body);
const assert = require("node:assert/strict");
const vm = require("node:vm");
const {sha256, deferred, until, manifestFor, assetAlias, runtimeFiles, mockStorage, resourceHarness, webcrypto} = require("./resource-harness.cjs");
const MiB = 1024 * 1024;
const prefix = "https://tabula.test/.tabula-public-runtime-v1/";
const indexKey = `${prefix}index`;
const keyFor = (bytes, extension = "wasm") => `${prefix}${sha256(bytes)}.${extension}`;
const bytesFor = (count = 1) => Object.fromEntries(Array.from({length:count}, (_, index) => [`file-${index}.png`, new Uint8Array([index + 1])]));
const payloads = (storage) => [...storage.entries.keys()].filter((key) => key !== indexKey);

test("mocked cold/warm cache fetches only requested manifest inventory and re-verifies hits", async () => {
  const storage = mockStorage();
  const cold = resourceHarness({storage});
  assert.equal(cold.fetches.length, 0);
  assert.equal(storage.entries.size, 0);
  const aliases = ["tabula-game-client.wasm", "assets/OpenSans-Regular.ttf", assetAlias];
  const networkProgress = [];
  for (const alias of aliases) {
    const bytes = await cold.loader.load(alias, {onProgress:(event) => networkProgress.push(event)});
    assert.deepEqual(bytes, runtimeFiles()[alias]);
  }
  assert.equal(cold.fetches.length, 3);
  assert.equal(cold.digests.length, 3);
  assert.equal(payloads(storage).length, 3);
  for (const {url, options} of cold.fetches) {
    assert.match(url, /\/play\/local\/resources\/[0-9a-f]{64}\.(wasm|ttf|png)$/);
    assert.equal(options.credentials, "omit");
    assert.equal(options.cache, "no-store");
    assert.equal(options.redirect, "error");
    assert.equal(options.mode, "same-origin");
  }
  assert.ok(networkProgress.every((event) => event.source === "network"));
  const warm = resourceHarness({storage});
  const cachedProgress = [];
  for (const alias of aliases) await warm.loader.load(alias, {onProgress:(event) => cachedProgress.push(event)});
  assert.equal(warm.fetches.length, 0);
  assert.equal(warm.digests.length, 3, "cache hits must cross SHA-256 verification again");
  assert.ok(cachedProgress.some((event) => event.phase === "cache-hit"));
  assert.ok(cachedProgress.every((event) => event.source === "cache"));
});

test("same alias and identical manifest URL share one pending Promise; settled promises are released", async () => {
  const gate = deferred();
  const bytes = new Uint8Array([1,2,3]);
  const mock = resourceHarness({files:{"first.png":bytes, "second.png":bytes}, fetch:async () => { await gate.promise; return new Response(bytes); }});
  const first = mock.loader.load("first.png");
  const second = mock.loader.load("second.png");
  assert.equal(first, second);
  await until(() => mock.fetches.length === 1);
  gate.resolve();
  assert.deepEqual(await first, bytes);
  await mock.loader.load("first.png");
  assert.equal(mock.fetches.length, 2, "uncached settled bytes must not stay in a promise map");
});

test("four loads and 64 MiB aggregate declared size admit bounded concurrent work and release on abort", async () => {
  const gate = deferred();
  const files = bytesFor(5);
  const mock = resourceHarness({files, fetch:async (_url, _options, bytes) => { await gate.promise; return new Response(bytes); }});
  const pending = Object.keys(files).slice(0, 4).map((name) => mock.loader.load(name));
  await assert.rejects(mock.loader.load("file-4.png"), /in-flight budget/);
  await until(() => mock.fetches.length === 4);
  gate.resolve();
  await Promise.all(pending);
  await mock.loader.load("file-4.png");
  assert.equal(mock.fetches.length, 5);

  const cappedFiles = bytesFor(3);
  const manifest = manifestFor(cappedFiles);
  manifest.files["file-0.png"].bytes = 32 * MiB;
  manifest.files["file-1.png"].bytes = 32 * MiB;
  const hold = deferred();
  const capped = resourceHarness({files:cappedFiles, manifest, fetch:async () => { await hold.promise; return new Response(new Uint8Array()); }});
  const first = capped.loader.load("file-0.png");
  const second = capped.loader.load("file-1.png");
  const firstRejection = assert.rejects(first, {name:"AbortError"});
  const secondRejection = assert.rejects(second, {name:"AbortError"});
  await assert.rejects(capped.loader.load("file-2.png"), /in-flight budget/);
  await until(() => capped.fetches.length === 2);
  capped.controller.abort();
  hold.resolve();
  await Promise.all([firstRejection, secondRejection]);
  assert.equal(capped.fetches.length, 2);
});

test("unknown, missing, corrupt, empty, truncated and oversized resources fail without trusted delivery", async () => {
  const files = {"image.png":new Uint8Array([1,2,3])};
  const unknown = resourceHarness({files});
  for (const name of ["other.png", "https://tabula.test/image.png", "resources/image.png", "../image.png", "tabula-launch.txt", "tabula-ready.txt"]) {
    await assert.rejects(unknown.loader.load(name), /not listed/);
  }
  assert.equal(unknown.fetches.length, 0);
  for (const [name, response, expected] of [
    ["HTTP", new Response("", {status:404}), /HTTP 404/],
    ["corrupt", new Response(new Uint8Array([3,2,1])), /integrity/],
    ["empty", new Response(new Uint8Array()), /length/],
    ["truncated", new Response(new Uint8Array([1,2])), /length/],
    ["oversized", new Response(new Uint8Array([1,2,3,4])), /declared size/]
  ]) {
    const storage = mockStorage();
    const mock = resourceHarness({files, storage, fetch:async () => response.clone()});
    await assert.rejects(mock.loader.load("image.png"), expected, name);
    assert.equal(payloads(storage).length, 0, `${name} entered the verified cache`);
  }
});

test("failed pending resource releases its coalescing entry so a later explicit request can retry", async () => {
  let attempts = 0;
  const files = {"image.png":new Uint8Array([1,2,3])};
  const mock = resourceHarness({files, fetch:async (_url, _options, bytes) => ++attempts === 1 ? new Response("", {status:503}) : new Response(bytes)});
  await assert.rejects(mock.loader.load("image.png"), /HTTP 503/);
  assert.deepEqual(await mock.loader.load("image.png"), files["image.png"]);
  assert.equal(mock.fetches.length, 2);
});

test("mocked corrupt cached bytes are evicted and get exactly one fresh verified fetch", async () => {
  for (const damaged of [new Uint8Array([9,9,9]), new Uint8Array([1,2]), new Uint8Array([1,2,3,4])]) {
    const bytes = new Uint8Array([1,2,3]);
    const files = {"image.png":bytes};
    const storage = mockStorage();
    await resourceHarness({files, storage}).loader.load("image.png");
    const key = keyFor(bytes, "png");
    storage.entries.set(key, new Response(damaged, {headers:{"Content-Length":"3", "X-Tabula-SHA256":sha256(bytes)}}));
    const fresh = resourceHarness({files, storage});
    assert.deepEqual(await fresh.loader.load("image.png"), bytes);
    assert.equal(fresh.fetches.length, 1);
    assert.ok(storage.changes.some((change) => change.action === "delete" && change.key === key));
    const warm = resourceHarness({files, storage});
    await warm.loader.load("image.png");
    assert.equal(warm.fetches.length, 0);
  }
  const bytes = new Uint8Array([1,2,3]);
  const files = {"image.png":bytes};
  const storage = mockStorage();
  await resourceHarness({files, storage}).loader.load("image.png");
  storage.entries.set(keyFor(bytes, "png"), new Response(new Uint8Array([9,9,9]), {headers:{"Content-Length":"3", "X-Tabula-SHA256":sha256(bytes)}}));
  const corruptNetwork = resourceHarness({files, storage, fetch:async () => new Response(new Uint8Array([8,8,8]))});
  await assert.rejects(corruptNetwork.loader.load("image.png"), /integrity/);
  assert.equal(corruptNetwork.fetches.length, 1);
  assert.equal(payloads(storage).length, 0);
});

test("content versions have distinct cache keys and never reuse stale bytes under the same alias", async () => {
  const storage = mockStorage();
  const oldFiles = {"image.png":new Uint8Array([1,2,3])};
  const newFiles = {"image.png":new Uint8Array([4,5,6])};
  await resourceHarness({files:oldFiles, storage}).loader.load("image.png");
  const changed = resourceHarness({files:newFiles, storage});
  assert.deepEqual(await changed.loader.load("image.png"), newFiles["image.png"]);
  assert.equal(changed.fetches.length, 1);
  assert.equal(payloads(storage).length, 2);
  const warm = resourceHarness({files:newFiles, storage});
  assert.deepEqual(await warm.loader.load("image.png"), newFiles["image.png"]);
  assert.equal(warm.fetches.length, 0);
});

test("manifest schema, fields, sizes, count, total and canonical hash URLs reject hostile inventory", () => {
  const files = {"image.png":new Uint8Array([1,2,3])};
  const base = manifestFor(files);
  const invalid = [];
  for (const schema of [0, 2, "1", null]) invalid.push({...base, schema});
  invalid.push({...base, unknown:true}, {schema:1, files:[]}, {schema:1, files:{}}, {schema:1, files:bytesFor(33)});
  for (const bytes of [0, -1, 1.5, "3", 64 * MiB + 1, null]) invalid.push({schema:1, files:{"image.png":{...base.files["image.png"], bytes}}});
  for (const url of ["https://evil.test/resource.png", "https://tabula.test/resource.png", "/resources/"+base.files["image.png"].sha256+".png", "resources/../resource.png", "resources/%2e%2e/resource.png", base.files["image.png"].url+"?x=1", base.files["image.png"].url+"#x", "resources/"+"f".repeat(64)+".png", "resources/"+base.files["image.png"].sha256.toUpperCase()+".png", "resources/"+base.files["image.png"].sha256+".ttf", "https://user:password@tabula.test/resource.png"]) {
    invalid.push({schema:1, files:{"image.png":{...base.files["image.png"], url}}});
  }
  for (const alias of ["../image.png", "image/../image.png", "/image.png", "image//image.png", "image\\image.png", "image.png?x=1", "image.png#x", "image%2epng", "tabula-launch.txt", "tabula-ready.txt"]) invalid.push({schema:1, files:{[alias]:base.files["image.png"]}});
  invalid.push({schema:1, files:{"image.png":{...base.files["image.png"], unknown:true}}});
  invalid.push({schema:1, files:{"image.png":{...base.files["image.png"], sha256:"0".repeat(64)}}});
  const tooMany = manifestFor(bytesFor(33));
  invalid.push(tooMany);
  const tooLarge = manifestFor(bytesFor(3));
  for (const entry of Object.values(tooLarge.files)) entry.bytes = 64 * MiB;
  invalid.push(tooLarge);
  const conflict = manifestFor({"one.png":files["image.png"], "two.png":files["image.png"]});
  conflict.files["two.png"].bytes = 2;
  invalid.push(conflict);
  for (const manifest of invalid) assert.throws(() => resourceHarness({files, manifest}), undefined, JSON.stringify(manifest));
  const valid = resourceHarness({files});
  for (const setup of ["window.TabulaResourceManifest.extra=true", "Object.defineProperty(window.TabulaResourceManifest.files, 'image.png', {get(){return {};}})", "window.TabulaResourceManifest.files=Object.create({})", "window.TabulaResourceManifest[Symbol('extra')]=1"]) {
    assert.throws(() => vm.runInContext(`${setup}; window.TabulaResources.create(window.TabulaResourceManifest);`, valid.context));
    vm.runInContext(`window.TabulaResourceManifest=${JSON.stringify(base)}`, valid.context);
  }
});

test("validated inventory is detached from later raw-manifest mutation", async () => {
  const files = {"image.png":new Uint8Array([1,2,3])};
  const mock = resourceHarness({files});
  mock.window.TabulaResourceManifest.files["image.png"].url = "https://evil.test/changed.png";
  mock.window.TabulaResourceManifest.files["image.png"].bytes = 99;
  assert.deepEqual(await mock.loader.load("image.png"), files["image.png"]);
  assert.equal(new URL(mock.fetches[0].url).origin, "https://tabula.test");
});

test("mocked unsupported, denied and quota-limited cache/locks fall back to verified network", async () => {
  for (const settings of [undefined, {storage:mockStorage(), cacheUnsupported:true}, {storage:mockStorage(), locksUnsupported:true}, {storage:mockStorage({openDenied:true})}, {storage:mockStorage({lockDenied:true})}, {storage:mockStorage({quota:true})}]) {
    const mock = resourceHarness(settings);
    const bytes = await mock.loader.load("tabula-game-client.wasm");
    assert.deepEqual(bytes, runtimeFiles()["tabula-game-client.wasm"]);
    assert.equal(mock.fetches.length, 1);
    assert.equal(mock.digests.length, 1);
  }
  const storage = mockStorage();
  await resourceHarness({storage}).loader.load("tabula-game-client.wasm");
  storage.setQuota(true);
  const hit = resourceHarness({storage});
  await hit.loader.load("tabula-game-client.wasm");
  assert.equal(hit.fetches.length, 0, "an index update failure must not discard verified hit bytes");
});

test("mocked shared Web Locks coalesce identical resource loads across documents", async () => {
  const storage = mockStorage();
  const gate = deferred();
  const cold = resourceHarness({storage, fetch:async (_url, _options, bytes) => { await gate.promise; return new Response(bytes); }});
  const warm = resourceHarness({storage});
  const first = cold.loader.load("tabula-game-client.wasm");
  await until(() => cold.fetches.length === 1);
  const second = warm.loader.load("tabula-game-client.wasm");
  assert.equal(warm.fetches.length, 0);
  gate.resolve();
  const [one, two] = await Promise.all([first, second]);
  assert.deepEqual(one, two);
  assert.equal(warm.fetches.length, 0);
  assert.equal(warm.digests.length, 1);
});

test("mocked LRU bounds total cache entries including its metadata and refreshes file recency", async () => {
  const files = bytesFor(32);
  const storage = mockStorage();
  const mock = resourceHarness({files, storage});
  for (let index = 0; index < 31; index++) await mock.loader.load(`file-${index}.png`);
  await mock.loader.load("file-0.png");
  await mock.loader.load("file-31.png");
  assert.equal(storage.entries.size, 32);
  assert.equal(payloads(storage).length, 31);
  assert.ok(storage.entries.has(keyFor(files["file-0.png"], "png")));
  assert.ok(!storage.entries.has(keyFor(files["file-1.png"], "png")));
  assert.ok(storage.entries.has(keyFor(files["file-31.png"], "png")));
  const index = await storage.entries.get(indexKey).clone().json();
  assert.equal(index.entries.length, 31);
  assert.ok(new TextEncoder().encode(JSON.stringify(index)).byteLength <= 32 * 1024);
});

test("mocked declared-payload accounting reserves metadata inside 150 MiB and evicts before insertion", async () => {
  const storage = mockStorage();
  const sizes = [64 * MiB, 64 * MiB, 22 * MiB - 64 * 1024];
  for (const [index, size] of sizes.entries()) {
    const hash = "abc"[index].repeat(64);
    // Sparse mocked persisted bodies model budget accounting, not real disk use.
    storage.entries.set(`${prefix}${hash}.png`, new Response(new Uint8Array([1]), {headers:{"Content-Length":String(size), "X-Tabula-SHA256":hash}}));
  }
  const files = {"new.png":new Uint8Array(128 * 1024).fill(7)};
  await resourceHarness({files, storage}).loader.load("new.png");
  assert.ok(!storage.entries.has(`${prefix}${"a".repeat(64)}.png`));
  const indexResponse = storage.entries.get(indexKey).clone();
  const indexBytes = new Uint8Array(await indexResponse.arrayBuffer());
  const index = JSON.parse(new TextDecoder().decode(indexBytes));
  assert.ok(index.entries.reduce((sum, entry) => sum + entry.bytes, 0) + indexBytes.byteLength <= 150 * MiB);
});

test("mocked broken LRU index and orphan entries reconcile without fetching unrelated resources", async () => {
  const storage = mockStorage();
  await resourceHarness({storage}).loader.load("tabula-game-client.wasm");
  storage.entries.set(indexKey, new Response("not json"));
  storage.entries.set(`${prefix}untrusted`, new Response("private or unrelated"));
  const warm = resourceHarness({storage});
  await warm.loader.load("tabula-game-client.wasm");
  assert.equal(warm.fetches.length, 0);
  assert.ok(!storage.entries.has(`${prefix}untrusted`));
  assert.equal((await storage.entries.get(indexKey).clone().json()).entries.length, 1);
});

test("aborts reject at pending fetch and digest boundaries before caching or late delivery", async () => {
  for (const stage of ["fetch", "digest"]) {
    const storage = mockStorage();
    const gate = deferred();
    let entered = false;
    const mock = resourceHarness({storage,
      fetch:stage === "fetch" ? async (_url, _options, bytes) => { entered = true; await gate.promise; return new Response(bytes); } : undefined,
      digest:stage === "digest" ? async (algorithm, bytes) => { entered = true; await gate.promise; return webcrypto.subtle.digest(algorithm, bytes); } : undefined
    });
    const promise = mock.loader.load("tabula-game-client.wasm");
    const rejection = assert.rejects(promise, {name:"AbortError"});
    await until(() => entered, stage);
    mock.controller.abort();
    gate.resolve();
    await rejection;
    assert.equal(payloads(storage).length, 0);
  }
});

test("aborted streamed reads cancel and release their reader without accepting late chunks", async () => {
  const gate = deferred();
  let reads = 0;
  let canceled = 0;
  let released = 0;
  const mock = resourceHarness({fetch:async () => ({ok:true, headers:new Headers({"Content-Length":"8"}), body:{getReader:() => ({
    async read() { reads++; await gate.promise; return {done:false, value:runtimeFiles()["tabula-game-client.wasm"]}; },
    async cancel() { canceled++; }, releaseLock() { released++; }
  })}})});
  const promise = mock.loader.load("tabula-game-client.wasm");
  const rejection = assert.rejects(promise, {name:"AbortError"});
  await until(() => reads === 1);
  mock.controller.abort();
  gate.resolve();
  await rejection;
  assert.equal(canceled, 1);
  assert.equal(released, 1);
  assert.equal(mock.digests.length, 0);
});

test("mocked late cache hits and cached-hit verification cannot deliver after cancellation", async () => {
  for (const stage of ["match", "digest"]) {
    const storage = mockStorage();
    await resourceHarness({storage}).loader.load("tabula-game-client.wasm");
    const gate = deferred();
    let entered = false;
    const key = keyFor(runtimeFiles()["tabula-game-client.wasm"]);
    if (stage === "match") {
      const match = storage.cache.match;
      let matches = 0;
      storage.cache.match = async (requested) => {
        if (requested === key && ++matches === 2) { entered = true; await gate.promise; }
        return match(requested);
      };
    }
    const mock = resourceHarness({storage, digest:stage === "digest" ? async (algorithm, bytes) => { entered = true; await gate.promise; return webcrypto.subtle.digest(algorithm, bytes); } : undefined});
    const promise = mock.loader.load("tabula-game-client.wasm");
    const rejection = assert.rejects(promise, {name:"AbortError"});
    await until(() => entered, `cached ${stage}`);
    mock.controller.abort();
    gate.resolve();
    await rejection;
    assert.equal(mock.fetches.length, 0);
    assert.ok(storage.entries.has(key), "cancellation should retain previously verified public bytes");
  }
});

test("mocked cache.put finishing after abort is compensated before releasing the shared lock", async () => {
  const gate = deferred();
  let entered = false;
  const storage = mockStorage({beforePut:async (key) => { if (key.endsWith(".wasm")) { entered = true; await gate.promise; } }});
  const mock = resourceHarness({storage});
  const promise = mock.loader.load("tabula-game-client.wasm");
  const rejection = assert.rejects(promise, {name:"AbortError"});
  await until(() => entered, "cache payload write");
  mock.controller.abort();
  gate.resolve();
  await rejection;
  assert.equal(payloads(storage).length, 0);
});

test("non-streaming network responses fail closed without invoking arrayBuffer", async () => {
  let buffered = 0;
  const storage = mockStorage();
  const mock = resourceHarness({storage, fetch:async () => ({
    ok:true, status:200, headers:new Headers({"Content-Length":"8"}), body:null,
    async arrayBuffer() { buffered++; return runtimeFiles()["tabula-game-client.wasm"].buffer; }
  })});
  await assert.rejects(mock.loader.load("tabula-game-client.wasm"), /Streaming resource responses are required/);
  assert.equal(buffered, 0);
  assert.equal(mock.digests.length, 0);
  assert.equal(payloads(storage).length, 0);
});

test("non-streaming cached responses are evicted before a fresh streamed verified load", async () => {
  let buffered = 0;
  const bytes = runtimeFiles()["tabula-game-client.wasm"];
  const key = keyFor(bytes);
  const storage = mockStorage();
  await resourceHarness({storage}).loader.load("tabula-game-client.wasm");
  const match = storage.cache.match;
  storage.cache.match = async (requested) => requested === key ? {
    headers:new Headers({"Content-Length":"8", "X-Tabula-SHA256":sha256(bytes)}), body:null,
    async arrayBuffer() { buffered++; return bytes.buffer; }
  } : match(requested);
  const mock = resourceHarness({storage});
  assert.deepEqual(await mock.loader.load("tabula-game-client.wasm"), bytes);
  assert.equal(buffered, 0);
  assert.equal(mock.fetches.length, 1);
  assert.equal(mock.digests.length, 1);
  assert.ok(storage.changes.some((change) => change.action === "delete" && change.key === key));
});
