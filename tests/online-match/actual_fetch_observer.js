/* Test-only observation of authentic fetch bytes; never a transport or authority. */
(() => {
  "use strict";
  const original = window.fetch, NativeRequest = window.Request;
  const nativeBody = Object.getOwnPropertyDescriptor(NativeRequest.prototype, "body").get;
  const nativeUrl = Object.getOwnPropertyDescriptor(NativeRequest.prototype, "url").get;
  const MAX_RECORDS = 64, MAX_BODY = 2097152, MAX_TOTAL = 8388608;
  const MAX_REQUEST = 131072, READ_MS = 5000;
  function observationEpoch() {
    return {documentId:crypto.randomUUID(),records:new Map(),readers:new Set(),timers:new Set(),requestBodies:new WeakMap(),
      next:0,retained:0,capturedRequests:0,capturedRequestBytes:0,disposed:false,fatal:null};
  }
  let currentEpoch = observationEpoch();
  const resultError = error => ({ok:false,error});
  const interested = url => {
    try {
      const value = new URL(url, location.href);
      return value.origin === "https://localhost:9443" && !value.search && !value.hash
        && (value.pathname === "/api/v1/matches" || value.pathname === "/api/v1/matches/join"
          || /^\/api\/v1\/matches\/[0-9a-f]{32}\/(attach|command)$/.test(value.pathname));
    } catch (_) { return false; }
  };
  function fail(record, error, requestFailure = false) {
    const epoch = record.epoch;
    if (!record.error) record.error = error;
    if (record.bytes) { epoch.retained -= record.bytes; record.bytes = 0; }
    record.text = null;
    record.responseCancel?.();
    record.notify?.();
    if (requestFailure || error === "document_disposed") {
      record.fingerprint_error = error;
      record.requestCancel?.(); record.stop?.();
    }
  }
  function dispose(epoch = currentEpoch) {
    if (epoch.disposed) return;
    epoch.disposed = true;
    for (const record of epoch.records.values()) fail(record, "document_disposed");
    for (const reader of epoch.readers) { try { Promise.resolve(reader.cancel()).catch(() => {}); } catch (_) {} }
    for (const timer of epoch.timers) clearTimeout(timer);
    epoch.readers.clear(); epoch.timers.clear(); epoch.records.clear(); epoch.retained = 0;
    epoch.requestBodies = new WeakMap();
    epoch.capturedRequests = 0; epoch.capturedRequestBytes = 0;
  }
  function bodyOption(init) {
    if (init == null) return {known:true,value:undefined};
    if (typeof init !== "object" && typeof init !== "function") return {known:false};
    const descriptor = Object.getOwnPropertyDescriptor(init, "body");
    if (!descriptor) return "body" in init ? {known:false} : {known:true,value:undefined};
    return Object.hasOwn(descriptor, "value") ? {known:true,value:descriptor.value} : {known:false};
  }
  function bodySource(input, init, epoch) {
    const option = bodyOption(init);
    if (!option.known) return {error:"request_body_unsupported"};
    if (option.value != null) {
      if (typeof option.value !== "string") return {error:"request_body_unsupported"};
      if (option.value.length > MAX_REQUEST) return {error:"request_body_limit"};
      return {text:option.value};
    }
    if (!(input instanceof NativeRequest)) return {text:""};
    const inherited = epoch.requestBodies.get(input);
    if (inherited) return inherited;
    return nativeBody.call(input) === null ? {text:""} : {error:"request_body_unsupported"};
  }
  // A Request clone tees its upload and makes Chromium omit authentic post data
  // from the real Request event. Retain only bounded plain constructor inputs;
  // never read or clone an upload to reconstruct bytes missing from that event.
  window.Request = new Proxy(NativeRequest, {
    construct(target, args, newTarget) {
      const epoch = currentEpoch;
      let source;
      try { source = bodySource(args[0], args[1], epoch); }
      catch (_) { source = {error:"request_body_unsupported"}; }
      const request = Reflect.construct(target, args, newTarget);
      try {
        if (!epoch.disposed && !epoch.fatal && interested(nativeUrl.call(request))) {
          if (epoch.capturedRequests >= MAX_RECORDS) epoch.fatal = "record_count_limit";
          else {
            let size = 0;
            if (!source.error) {
              size = new TextEncoder().encode(source.text).length;
              if (size > MAX_REQUEST) { source = {error:"request_body_limit"}; size = 0; }
            }
            if (size > MAX_TOTAL - epoch.capturedRequestBytes) epoch.fatal = "total_body_limit";
            else {
              epoch.capturedRequests += 1; epoch.capturedRequestBytes += size;
              epoch.requestBodies.set(request, source);
            }
          }
        }
      } catch (_) { epoch.fatal = "request_descriptor_failed"; }
      return request;
    }
  });
  async function boundedBytes(stream, limit, record, responseBody) {
    const epoch = record.epoch;
    if (!stream) return new Uint8Array();
    const reader = stream.getReader(), chunks = [];
    epoch.readers.add(reader);
    let size = 0, timer, rejectCancel;
    const canceled = new Promise((_, reject) => { rejectCancel = reject; });
    const cancel = () => { rejectCancel(new Error("observation_canceled")); try { Promise.resolve(reader.cancel()).catch(() => {}); } catch (_) {} };
    const cancelKey = responseBody ? "responseCancel" : "requestCancel";
    record[cancelKey] = cancel;
    timer = setTimeout(() => { fail(record, "body_read_deadline", !responseBody); cancel(); }, READ_MS);
    epoch.timers.add(timer);
    try {
      for (;;) {
        if (epoch.disposed || (responseBody ? record.error : record.fingerprint_error)) throw new Error("observation_canceled");
        const part = await Promise.race([reader.read(), canceled]);
        if (epoch.disposed || (responseBody ? record.error : record.fingerprint_error)) throw new Error("observation_canceled");
        if (part.done) break;
        if (!(part.value instanceof Uint8Array) || part.value.length > limit - size) {
          fail(record, responseBody ? "response_body_limit" : "request_body_limit", !responseBody);
          throw new Error("observation_limit");
        }
        if (responseBody && part.value.length > MAX_TOTAL - epoch.retained) {
          epoch.fatal = "total_body_limit"; fail(record, epoch.fatal); throw new Error("observation_limit");
        }
        size += part.value.length;
        if (responseBody) { epoch.retained += part.value.length; record.bytes += part.value.length; }
        chunks.push(part.value);
      }
      if (epoch.disposed || (responseBody ? record.error : record.fingerprint_error)) throw new Error("observation_canceled");
      const bytes = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
      return bytes;
    } finally {
      clearTimeout(timer); epoch.timers.delete(timer); epoch.readers.delete(reader);
      if (record[cancelKey] === cancel) record[cancelKey] = null;
      chunks.length = 0;
      try { reader.releaseLock(); } catch (_) {}
    }
  }
  async function fingerprint(input, init, record) {
    const epoch = record.epoch;
    let bytes;
    try {
      const source = bodySource(input, init, epoch);
      if (source.error) { fail(record, source.error, true); return; }
      bytes = new TextEncoder().encode(source.text);
      if (bytes.length > MAX_REQUEST) { fail(record, "request_body_limit", true); return; }
      let timer;
      const deadline = new Promise(resolve => {
        timer = setTimeout(() => { fail(record, "request_fingerprint_deadline", true); resolve(null); }, READ_MS);
        epoch.timers.add(timer);
      });
      let hashed;
      try { hashed = await Promise.race([crypto.subtle.digest("SHA-256", bytes), deadline, record.closed]); }
      finally { clearTimeout(timer); epoch.timers.delete(timer); }
      if (hashed === null) return;
      const digest = new Uint8Array(hashed);
      if (!epoch.disposed) record.body_sha256 = Array.from(digest, value => value.toString(16).padStart(2, "0")).join("");
    } catch (_) { fail(record, "request_body_read_failed", true); }
  }
  async function observeResponse(response, record) {
    const epoch = record.epoch;
    if (epoch.disposed) { fail(record, "document_disposed"); return; }
    try {
      if (response.redirected || response.url !== record.url) { fail(record, "response_redirected"); return; }
      const length = response.headers.get("Content-Length");
      if (length !== null && (!/^[0-9]{1,10}$/.test(length) || Number(length) > MAX_BODY)) {
        fail(record, "response_body_limit"); return;
      }
      const media = (response.headers.get("Content-Type") || "").split(";")[0].trim().toLowerCase();
      record.metadata = {
        status:response.status, url:response.url,
        content_type:["application/json","application/problem+json"].includes(media) ? media : "other",
        no_store:(response.headers.get("Cache-Control") || "").split(",").some(value => value.trim().toLowerCase() === "no-store"),
        content_length:length
      };
      const clone = response.clone();
      const bytes = await boundedBytes(clone.body, MAX_BODY, record, true);
      if (length !== null && bytes.length !== Number(length)) { fail(record, "response_body_truncated"); return; }
      if (!epoch.disposed && !record.error) record.text = new TextDecoder("utf-8", {fatal:true,ignoreBOM:true}).decode(bytes);
    } catch (_) { fail(record, "response_body_read_failed"); }
  }
  function wrappedFetch(...args) {
    const epoch = currentEpoch;
    let record = null;
    try {
      const input = args[0], init = args[1];
      const url = input instanceof NativeRequest ? input.url : new URL(String(input), location.href).href;
      if (!epoch.disposed && interested(url)) {
        if (epoch.records.size >= MAX_RECORDS) epoch.fatal = "record_count_limit";
        else {
          record = {epoch,id:++epoch.next,url,method:String(init?.method ?? (input instanceof NativeRequest ? input.method : "GET")).toUpperCase(),
            call_started_at:performance.timeOrigin+performance.now(),
            body_sha256:null,owner:null,bound_watermark:null,metadata:null,text:null,bytes:0,error:null,
            fingerprint_error:null,requestCancel:null,responseCancel:null};
          epoch.records.set(record.id, record);
          record.headers = new Promise(resolve => { record.notify = resolve; });
          record.closed = new Promise(resolve => { record.stop = () => resolve(null); });
          record.fingerprint = fingerprint(input, init, record);
        }
      }
    } catch (_) { epoch.fatal = "request_descriptor_failed"; }
    // No fingerprint/read/binding await can delay or replace the original call.
    let promise;
    try { promise = Reflect.apply(original, this, args); }
    catch (error) { if (record) fail(record, "original_fetch_failed"); throw error; }
    if (record) promise.then(response => {
      record.response = observeResponse(response, record);
      record.notify();
    }, () => { fail(record, "original_fetch_failed"); }).catch(() => { fail(record, "response_observation_failed"); });
    return promise;
  }
  const controller = {
    version:1,
    identity() { return {document:currentEpoch.documentId,disposed:currentEpoch.disposed,fatal:currentEpoch.fatal}; },
    async bind(expected, owner) {
      const epoch = currentEpoch;
      if (epoch.disposed) return resultError("document_disposed");
      if (epoch.fatal) return resultError(epoch.fatal);
      const matching = () => [...epoch.records.values()].filter(record => record.method === expected.method && record.url === expected.url);
      let candidates = matching();
      for (;;) {
        await Promise.all(candidates.map(record => record.fingerprint));
        const current = matching();
        if (current.length === candidates.length) { candidates = current; break; }
        candidates = current;
      }
      if (epoch.disposed) return resultError("document_disposed");
      if (epoch.fatal) return resultError(epoch.fatal);
      if (candidates.some(record => record.body_sha256 === null)) return resultError("request_fingerprint_unavailable");
      const exact = candidates.filter(record => record.body_sha256 === expected.body_sha256);
      const owned = exact.filter(record => record.owner === owner);
      if (owned.length === 1) {
        if (exact.some(record => record.owner === null && record.id <= owned[0].bound_watermark)) return resultError("ambiguous_request");
        return {ok:true,id:owned[0].id,document:epoch.documentId};
      }
      if (owned.length > 1) return resultError("ambiguous_request");
      // Pending and failed records remain candidates; success cannot hide a retry.
      const unbound = exact.filter(record => record.owner === null);
      if (unbound.length !== 1) return resultError(unbound.length ? "ambiguous_request" : "request_not_observed");
      const record = unbound[0];
      // An older identical capture is excluded only by a real request-event
      // binding that happened before this fetch invocation existed.
      if (exact.some(other => other !== record && other.bound_watermark >= record.id)) return resultError("ambiguous_request");
      record.owner = owner; record.bound_watermark = epoch.next;
      return {ok:true,id:record.id,document:epoch.documentId};
    },
    async read({id, owner, document:expectedDocument, metadata, request_started_at}) {
      const epoch = currentEpoch;
      if (epoch.disposed) return resultError("document_disposed");
      if (epoch.fatal) return resultError(epoch.fatal);
      if (expectedDocument !== epoch.documentId) return resultError("document_mismatch");
      const record = epoch.records.get(id);
      if (!record || record.owner !== owner) return resultError("request_owner_mismatch");
      if (!Number.isFinite(request_started_at) || request_started_at < record.call_started_at) return resultError("request_document_mismatch");
      if (!record.response && !record.error) {
        let timer;
        await Promise.race([record.headers, new Promise(resolve => {
          timer = setTimeout(() => { fail(record, "response_observation_deadline"); resolve(); }, READ_MS);
          epoch.timers.add(timer);
        })]);
        clearTimeout(timer); epoch.timers.delete(timer);
      }
      if (!record.response) return resultError(record.error || "response_not_observed");
      await record.response;
      if (epoch.disposed) return resultError("document_disposed");
      if (epoch.fatal) return resultError(epoch.fatal);
      if (record.error) return resultError(record.error);
      const keys = ["status","url","content_type","no_store","content_length"];
      if (!record.metadata || Object.keys(metadata).length !== keys.length || keys.some(key => record.metadata[key] !== metadata[key])) return resultError("response_metadata_mismatch");
      if (typeof record.text !== "string") return resultError("response_body_unavailable");
      return {ok:true,text:record.text};
    },
    retireBody({id, owner, document:expectedDocument}) {
      const epoch = currentEpoch;
      if (epoch.disposed) return resultError("document_disposed");
      if (expectedDocument !== epoch.documentId) return resultError("document_mismatch");
      const record = epoch.records.get(id);
      if (!record || record.owner !== owner) return resultError("request_owner_mismatch");
      // Python's specific-Response cache owns these same authentic bytes after
      // transfer. Keep their charge until disposal so combined retention stays bounded.
      record.text = null;
      return {ok:true};
    },
    dispose
  };
  Object.defineProperty(window, "__tabulaActualFetchObserver", {value:controller,configurable:false,writable:false});
  window.fetch = wrappedFetch;
  window.addEventListener("pagehide", () => dispose(), {capture:true});
  window.addEventListener("pageshow", event => {
    if (!event.persisted) return;
    // BFCache restores this JavaScript heap. Retire every old observation before
    // accepting fresh fetches; late continuations keep their retired epoch.
    dispose();
    currentEpoch = observationEpoch();
  }, {capture:true});
})();
