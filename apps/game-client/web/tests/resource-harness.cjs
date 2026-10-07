"use strict";
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const {createHash, webcrypto} = require("node:crypto");
const source = (name) => fs.readFileSync(path.join(__dirname, "..", name), "utf8");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const tick = () => new Promise((resolve) => setImmediate(resolve));
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return {promise, resolve, reject};
}
async function until(predicate, description = "mocked operation", milliseconds = 2000) {
  const end = Date.now() + milliseconds;
  while (!predicate()) {
    if (Date.now() >= end) throw new Error(`Timed out waiting for ${description}`);
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
}
function manifestFor(files) {
  return {schema:1, files:Object.fromEntries(Object.entries(files).map(([alias, bytes]) => {
    const hash = sha256(bytes);
    const extension = alias.split(".").at(-1);
    return [alias, {url:`resources/${hash}.${extension}`, bytes:bytes.byteLength, sha256:hash}];
  }))};
}
const fixturePaths = [...fs.readFileSync(path.join(__dirname, "../../../../games/chess/assets/fixture.pack.toml"), "utf8").matchAll(/^path = "([^"]+)"$/gm)].map((match) => match[1]);
const assetAlias = fixturePaths[2];
function runtimeFiles() {
  const wasm = new Uint8Array([0,97,115,109,1,0,0,0]);
  const files = {"tabula-game-client.wasm":wasm};
  for (const [index, name] of ["assets/OpenSans-Regular.ttf", "assets/OpenSans-Semibold.ttf", "assets/NotoSerif-Bold.ttf", ...fixturePaths].entries()) {
    files[name] = new Uint8Array([index+1,97,115,109,1,0,0,0]);
  }
  return files;
}
// Deliberately mocked persistence and locks. These are not browser storage,
// disk-use, cold/warm timing, quota, BFCache, or rendered-runtime measurements.
function mockStorage({quota = false, openDenied = false, lockDenied = false, beforePut, beforeMatch} = {}) {
  const entries = new Map();
  const changes = [];
  const pending = new Map();
  let quotaEnabled = quota;
  const keyOf = (key) => typeof key === "string" ? key : key.url;
  const cache = {
    async match(key) {
      key = keyOf(key);
      if (beforeMatch) await beforeMatch(key);
      return entries.get(key)?.clone();
    },
    async keys() { return [...entries.keys()].map((url) => ({url})); },
    async put(key, response) {
      key = keyOf(key);
      if (beforePut) await beforePut(key);
      if (quotaEnabled) throw Object.assign(new Error("Storage quota denied"), {name:"QuotaExceededError"});
      entries.set(key, response.clone());
      changes.push({action:"put", key});
    },
    async delete(key) {
      key = keyOf(key);
      changes.push({action:"delete", key});
      return entries.delete(key);
    }
  };
  const caches = {async open(name) { if (openDenied) throw new Error("CacheStorage denied"); if (name !== "tabula-public-runtime-v1") throw new Error("Wrong cache namespace"); return cache; }};
  const locks = {
    async request(name, options, callback) {
      if (lockDenied) throw new Error("Web Locks denied");
      const prior = pending.get(name) ?? Promise.resolve();
      const own = deferred();
      pending.set(name, own.promise);
      try {
        await prior;
        if (options.signal?.aborted) throw Object.assign(new Error("Lock aborted"), {name:"AbortError"});
        return await callback({name});
      } finally {
        own.resolve();
        if (pending.get(name) === own.promise) pending.delete(name);
      }
    }
  };
  return {cache, caches, locks, entries, changes, setQuota(value) { quotaEnabled = value; }};
}
function resourceHarness({files = runtimeFiles(), manifest = manifestFor(files), storage, locksUnsupported = false, cacheUnsupported = false, fetch:fetchOverride, digest, current, pathname = "/play/local/index.html"} = {}) {
  const controller = new AbortController();
  const fetches = [];
  const digests = [];
  const window = {};
  const location = {href:`https://tabula.test${pathname}`, origin:"https://tabula.test"};
  const data = new Map(Object.entries(files).map(([alias, bytes]) => [new URL(manifest.files[alias]?.url ?? "invalid", location.href).href, bytes]));
  const context = vm.createContext({window, location, URL, AbortController, TextEncoder, TextDecoder, Uint8Array, Response,
    crypto:{subtle:{async digest(algorithm, bytes) { digests.push({algorithm, bytes:bytes.byteLength}); return digest ? digest(algorithm, bytes) : webcrypto.subtle.digest(algorithm, bytes); }}},
    caches:cacheUnsupported ? undefined : storage?.caches,
    navigator:{locks:locksUnsupported ? undefined : storage?.locks},
    fetch:async (url, options) => {
      fetches.push({url, options});
      if (fetchOverride) return fetchOverride(url, options, data.get(url));
      const bytes = data.get(url);
      return bytes ? new Response(bytes, {headers:{"Content-Length":String(bytes.byteLength)}}) : new Response("", {status:404});
    }
  });
  vm.runInContext(`window.TabulaResourceManifest = ${JSON.stringify(manifest)};\n${source("resources.js")}`, context);
  const loader = window.TabulaResources.create(window.TabulaResourceManifest, {signal:controller.signal, current});
  return {loader, controller, fetches, digests, context, window, data, storage};
}
module.exports = {source, sha256, tick, deferred, until, manifestFor, fixturePaths, assetAlias, runtimeFiles, mockStorage, resourceHarness, webcrypto};
