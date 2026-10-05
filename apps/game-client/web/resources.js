/* Bounded local-host public-file delivery, not the future pack/CDN service.
 * ADR-0030 / doc 04 §12: Rust still verifies the pack's BLAKE3 contract. */
(function () {
  "use strict";
  if (window.TabulaResources) return;
  const FILE_LIMIT = 64 * 1024 * 1024;
  const INVENTORY_LIMIT = 150 * 1024 * 1024;
  const FILE_COUNT_LIMIT = 32;
  const LOAD_COUNT_LIMIT = 4;
  const INDEX_LIMIT = 32 * 1024;
  // Reserve one entry and INDEX_LIMIT bytes for recency metadata, so the whole
  // host-owned cache fits 32 entries / 150 MiB, including its bounded index.
  const CACHE_LIMIT = 150 * 1024 * 1024 - INDEX_LIMIT;
  const CACHE_COUNT_LIMIT = 31;
  const CACHE_NAME = "tabula-public-runtime-v1";
  const BUDGET_LOCK = `${CACHE_NAME}:budget`;
  const HASH = /^[0-9a-f]{64}$/;
  const RESOURCE = /^resources\/([0-9a-f]{64})\.([a-z0-9]{1,12})$/;
  const VIRTUAL = new Set(["tabula-launch.txt", "tabula-ready.txt", "tabula-concealed.txt"]);

  function record(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const prototype = Object.getPrototypeOf(value);
    return prototype === null || prototype === Object.prototype;
  }
  function fields(value, allowed) {
    if (!record(value)) throw new Error("Invalid resource manifest object");
    const keys = Reflect.ownKeys(value);
    if (keys.length !== allowed.length || keys.some((key) => !allowed.includes(key))) throw new Error("Unknown or missing resource manifest field");
    for (const key of keys) if (!Object.hasOwn(Object.getOwnPropertyDescriptor(value, key), "value")) throw new Error("Resource manifest accessors are not allowed");
  }
  function canonicalAlias(name) {
    return typeof name === "string" && name.length > 0 && name.length <= 240 &&
      !VIRTUAL.has(name) && name.split("/").every((part) => part !== "." && part !== ".." && /^[A-Za-z0-9_@.-]+$/.test(part));
  }
  function validateManifest(manifest) {
    fields(manifest, ["schema", "files"]);
    if (manifest.schema !== 1 || !record(manifest.files)) throw new Error("Unsupported resource manifest schema");
    const aliases = Reflect.ownKeys(manifest.files);
    if (!aliases.length || aliases.length > FILE_COUNT_LIMIT) throw new Error("Resource manifest file count exceeds the host limit");
    const files = new Map();
    const urls = new Map();
    let total = 0;
    for (const alias of aliases) {
      if (!canonicalAlias(alias) || !Object.hasOwn(Object.getOwnPropertyDescriptor(manifest.files, alias), "value")) throw new Error("Invalid resource alias");
      const raw = manifest.files[alias];
      fields(raw, ["url", "bytes", "sha256"]);
      const path = typeof raw.url === "string" ? RESOURCE.exec(raw.url) : null;
      if (!path || !HASH.test(raw.sha256) || path[1] !== raw.sha256 || !alias.endsWith(`.${path[2]}`)) throw new Error("Resource URL must contain its exact SHA-256 and extension");
      if (!Number.isSafeInteger(raw.bytes) || raw.bytes < 1 || raw.bytes > FILE_LIMIT) throw new Error("Resource size exceeds the 64 MiB host limit");
      total += raw.bytes;
      if (total > INVENTORY_LIMIT) throw new Error("Resource manifest inventory exceeds the 150 MiB host limit");
      const url = new URL(raw.url, location.href);
      if (!/^https?:$/.test(url.protocol) || url.origin !== location.origin || url.username || url.password || url.search || url.hash) throw new Error("Game resources must be canonical same-origin URLs");
      const existing = urls.get(url.href);
      if (existing && (existing.bytes !== raw.bytes || existing.sha256 !== raw.sha256)) throw new Error("Conflicting resource URL declarations");
      const entry = Object.freeze({url:url.href, bytes:raw.bytes, sha256:raw.sha256, extension:path[2]});
      urls.set(url.href, entry);
      files.set(alias, entry);
    }
    return files;
  }

  function create(manifest, {signal, current = () => true} = {}) {
    const files = validateManifest(manifest);
    if (!globalThis.crypto?.subtle?.digest) throw new Error("Verified game resources require WebCrypto SHA-256 in a secure browser context");
    const inFlight = new Map();
    let reservedBytes = 0;
    let persistence = Boolean(globalThis.caches?.open && globalThis.navigator?.locks?.request && typeof Response === "function");
    const prefix = `${location.origin}/.tabula-public-runtime-v1/`;
    const indexKey = `${prefix}index`;
    const cacheKey = (entry) => `${prefix}${entry.sha256}.${entry.extension}`;
    function check() {
      if (signal?.aborted || !current()) {
        const error = new Error("The game resource request was canceled");
        error.name = "AbortError";
        throw error;
      }
    }
    function notify(operation, event) {
      check();
      for (const listener of operation.listeners) listener(event);
    }
    function cancelBody(response) {
      try { response.body?.cancel()?.catch(() => {}); } catch (_) {}
    }
    // Fetch bodies and cached bodies use one preallocated bounded buffer.
    // Streaming support is required: arrayBuffer cannot enforce an allocation
    // bound before a hostile or decompressed response has been fully buffered.
    async function read(response, limit, exact, progress) {
      check();
      if (typeof response.body?.getReader !== "function") {
        cancelBody(response);
        throw new Error("Streaming resource responses are required for bounded game loading");
      }
      const advertised = response.headers.get("Content-Length");
      if (advertised !== null && (!/^[0-9]+$/.test(advertised) || !Number.isSafeInteger(Number(advertised)) || Number(advertised) > FILE_LIMIT || (!response.headers.get("Content-Encoding") && (exact ? Number(advertised) !== limit : Number(advertised) > limit)))) {
        cancelBody(response);
        throw new Error("Resource response length does not match its manifest");
      }
      const reader = response.body.getReader();
      const bytes = new Uint8Array(limit);
      let received = 0;
      let canceled = false;
      function cancel() {
        if (canceled) return;
        canceled = true;
        try { reader.cancel()?.catch(() => {}); } catch (_) {}
      }
      signal?.addEventListener("abort", cancel, {once:true});
      try {
        check();
        for (;;) {
          const {done, value} = await reader.read();
          check();
          if (done) break;
          if (value.byteLength > limit - received) throw new Error("Resource download exceeds its declared size");
          if (!(value instanceof Uint8Array)) throw new Error("Invalid resource download chunk");
          bytes.set(value, received);
          received += value.byteLength;
          progress?.(received);
        }
        if (exact && received !== limit) throw new Error("Resource response length does not match its manifest");
        return exact ? bytes : bytes.subarray(0, received);
      } catch (error) {
        cancel();
        throw error;
      } finally {
        signal?.removeEventListener("abort", cancel);
        reader.releaseLock();
      }
    }
    async function verify(bytes, entry) {
      check();
      if (bytes.byteLength !== entry.bytes) throw new Error("Resource response length does not match its manifest");
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      check();
      const hash = Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
      if (hash !== entry.sha256) throw new Error("Resource SHA-256 integrity check failed");
      return bytes;
    }
    async function download(entry, operation) {
      check();
      notify(operation, {source:"network", phase:"download", received:0, total:entry.bytes});
      const response = await fetch(entry.url, {signal, credentials:"omit", cache:"no-store", redirect:"error", mode:"same-origin"});
      check();
      if (!response.ok) { cancelBody(response); throw new Error(`Game resource download failed: HTTP ${response.status}`); }
      if (response.url && response.url !== entry.url) { cancelBody(response); throw new Error("Game resource redirects are not allowed"); }
      const bytes = await read(response, entry.bytes, true, (received) => notify(operation, {source:"network", phase:"download", received, total:entry.bytes}));
      check();
      notify(operation, {source:"network", phase:"verify", received:entry.bytes, total:entry.bytes});
      return verify(bytes, entry);
    }

    function payloadKey(key) {
      if (typeof key !== "string" || !key.startsWith(prefix)) return null;
      return /^([0-9a-f]{64})\.([a-z0-9]{1,12})$/.exec(key.slice(prefix.length));
    }
    function validIndex(raw) {
      try {
        fields(raw, ["schema", "entries"]);
        if (raw.schema !== 1 || !Array.isArray(raw.entries) || raw.entries.length > CACHE_COUNT_LIMIT) return null;
        const seen = new Set();
        for (const entry of raw.entries) {
          fields(entry, ["key", "bytes", "sha256", "used"]);
          const key = payloadKey(entry.key);
          if (!key || entry.sha256 !== key[1] || !Number.isSafeInteger(entry.bytes) || entry.bytes < 1 || entry.bytes > FILE_LIMIT || !Number.isSafeInteger(entry.used) || entry.used < 0 || seen.has(entry.key)) return null;
          seen.add(entry.key);
        }
        return raw.entries;
      } catch (_) { return null; }
    }
    async function remove(cache, key) {
      check();
      await cache.delete(key);
      check();
    }
    // Reconcile orphaned writes/index corruption under the shared budget lock.
    // Index metadata is bounded and contains only public hashes/sizes/recency.
    async function inventory(cache) {
      const indexResponse = await cache.match(indexKey);
      check();
      let prior = null;
      if (indexResponse) {
        try {
          const bytes = await read(indexResponse, INDEX_LIMIT, false);
          check();
          prior = validIndex(JSON.parse(new TextDecoder().decode(bytes)));
        } catch (_) { check(); }
      }
      const old = new Map((prior ?? []).map((entry) => [entry.key, entry]));
      const requests = await cache.keys();
      check();
      const entries = [];
      for (const request of requests) {
        const key = typeof request === "string" ? request : request.url;
        if (key === indexKey) continue;
        const path = payloadKey(key);
        if (!path) { await remove(cache, key); continue; }
        const response = await cache.match(key);
        check();
        const size = response?.headers.get("Content-Length");
        const hash = response?.headers.get("X-Tabula-SHA256");
        if (response) cancelBody(response);
        if (!response || !/^[1-9][0-9]*$/.test(size ?? "") || !Number.isSafeInteger(Number(size)) || Number(size) > FILE_LIMIT || hash !== path[1]) { await remove(cache, key); continue; }
        const previous = old.get(key);
        entries.push({key, bytes:Number(size), sha256:hash, used:previous?.bytes === Number(size) && previous.sha256 === hash ? previous.used : 0});
      }
      entries.sort((a, b) => a.used - b.used || a.key.localeCompare(b.key));
      let total = entries.reduce((sum, entry) => sum + entry.bytes, 0);
      while (entries.length > CACHE_COUNT_LIMIT || total > CACHE_LIMIT) {
        const evicted = entries.shift();
        await remove(cache, evicted.key);
        total -= evicted.bytes;
      }
      // Renumber instead of depending on wall-clock order or overflowing recency.
      entries.forEach((entry, index) => { entry.used = index; });
      return entries;
    }
    async function saveIndex(cache, entries) {
      check();
      const body = JSON.stringify({schema:1, entries});
      if (new TextEncoder().encode(body).byteLength > INDEX_LIMIT) throw new Error("Cache index exceeds its metadata limit");
      await cache.put(indexKey, new Response(body, {headers:{"Content-Type":"application/json"}}));
      check();
    }
    async function cacheAction(action) {
      if (!persistence) return null;
      try {
        const result = await navigator.locks.request(BUDGET_LOCK, {mode:"exclusive", signal}, async () => {
          check();
          const cache = await caches.open(CACHE_NAME);
          check();
          return action(cache);
        });
        check();
        return result;
      } catch (_) {
        check();
        // Cache/lock denial and quota must never turn valid gameplay into failure.
        persistence = false;
        return null;
      }
    }
    async function cached(entry, operation) {
      return cacheAction(async (cache) => {
        const entries = await inventory(cache);
        check();
        const key = cacheKey(entry);
        const known = entries.find((item) => item.key === key);
        if (!known) { await saveIndex(cache, entries); return null; }
        let bytes;
        try {
          const response = await cache.match(key);
          check();
          if (!response || known.bytes !== entry.bytes || known.sha256 !== entry.sha256) throw new Error("Cached resource metadata changed");
          notify(operation, {source:"cache", phase:"verify", received:0, total:entry.bytes});
          bytes = await read(response, entry.bytes, true);
          check();
          await verify(bytes, entry);
          check();
        } catch (_) {
          check();
          await remove(cache, key);
          await saveIndex(cache, entries.filter((item) => item.key !== key));
          return null;
        }
        known.used = entries.length;
        try { await saveIndex(cache, entries); }
        catch (_) { check(); persistence = false; }
        check();
        notify(operation, {source:"cache", phase:"cache-hit", received:entry.bytes, total:entry.bytes});
        return bytes;
      });
    }
    async function store(entry, bytes) {
      await cacheAction(async (cache) => {
        let entries = await inventory(cache);
        check();
        const key = cacheKey(entry);
        entries = entries.filter((item) => item.key !== key);
        let total = entries.reduce((sum, item) => sum + item.bytes, 0);
        while (entries.length >= CACHE_COUNT_LIMIT || total + entry.bytes > CACHE_LIMIT) {
          const evicted = entries.shift();
          await remove(cache, evicted.key);
          total -= evicted.bytes;
        }
        check();
        await cache.put(key, new Response(bytes, {headers:{"Content-Type":"application/octet-stream", "Content-Length":String(entry.bytes), "X-Tabula-SHA256":entry.sha256}}));
        // Cache.put has no AbortSignal. Compensate a write completed after abort
        // while still holding its lock; never deliver those bytes to the host.
        if (signal?.aborted || !current()) {
          try { await cache.delete(key); } catch (_) {}
          check();
        }
        check();
        entries.push({key, bytes:entry.bytes, sha256:entry.sha256, used:entries.length});
        await saveIndex(cache, entries);
        return true;
      });
      check();
    }
    async function resolve(entry, operation) {
      async function load() {
        const hit = await cached(entry, operation);
        check();
        if (hit) return hit;
        const bytes = await download(entry, operation);
        check();
        await store(entry, bytes);
        check();
        return bytes;
      }
      if (persistence) {
        let entered = false;
        try {
          const bytes = await navigator.locks.request(`${CACHE_NAME}:resource:${cacheKey(entry)}`, {mode:"exclusive", signal}, async () => { entered = true; check(); return load(); });
          check();
          return bytes;
        } catch (error) {
          check();
          if (entered) throw error;
          persistence = false;
        }
      }
      return load();
    }
    function load(alias, {onProgress} = {}) {
      try {
        check();
        const entry = files.get(alias);
        if (!entry) throw new Error("Game resource is not listed in the manifest");
        let operation = inFlight.get(entry.url);
        if (operation) {
          if (onProgress) operation.listeners.add(onProgress);
          return operation.promise;
        }
        if (inFlight.size >= LOAD_COUNT_LIMIT || reservedBytes + entry.bytes > FILE_LIMIT) throw new Error("Game resource in-flight budget exceeds four loads or 64 MiB");
        operation = {listeners:new Set(onProgress ? [onProgress] : []), promise:null};
        reservedBytes += entry.bytes;
        inFlight.set(entry.url, operation);
        operation.promise = resolve(entry, operation).finally(() => {
          if (inFlight.get(entry.url) === operation) inFlight.delete(entry.url);
          reservedBytes -= entry.bytes;
          operation.listeners.clear();
        });
        return operation.promise;
      } catch (error) { return Promise.reject(error); }
    }
    return Object.freeze({load});
  }
  window.TabulaResources = Object.freeze({create});
})();
