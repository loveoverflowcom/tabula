/* Same-origin HTTPS/document-memory Fetch glue (ADR-0041). No board, rules,
 * state folding or protocol sequencing: Rust owns typed commands/projections. */
(function(root) {
  "use strict";
  const MAX_RESPONSE = 2097152, MAX_REQUEST = 65536;
  function decodeHex(hex, max) {
    if (typeof hex !== "string" || hex.length > max * 2 || hex.length % 2 || !/^[0-9a-f]*$/.test(hex)) throw new Error("Invalid bounded runtime request");
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    return new TextDecoder("utf-8", {fatal:true}).decode(bytes);
  }
  // Keep the opaque u128 JSON identity exact: parse/stringify could round it.
  // The server validates the Rust-created ClientEnvelope.
  function commandBody(attachment, command) {
    if (!/^[0-9a-f]{32}$/.test(attachment) || typeof command !== "string" || new TextEncoder().encode(command).length > MAX_REQUEST) throw new Error("Invalid bounded command");
    const body = '{"version":1,"attachment_id":' + JSON.stringify(attachment) + ',"command":' + command + '}';
    if (new TextEncoder().encode(body).length > MAX_REQUEST) throw new Error("Online command exceeds request budget");
    return body;
  }
  function privateJson(bytes) {
    try { return JSON.parse(new TextDecoder("utf-8", {fatal:true}).decode(bytes)); }
    catch (_) { throw new Error("Online response is incompatible"); }
  }
  function create({matchId, gameId, signal, current, onWaiting, onStatus, onUnavailable, fetcher=root.fetch, protocol=root.location?.protocol, timers=root}) {
    if (protocol !== "https:" || !/^[0-9a-f]{32}$/.test(matchId) || /^0+$/.test(matchId) || typeof gameId !== "string" || gameId.length > 128) throw new Error("Online play needs a trusted HTTPS document");
    let csrf = null, attachment = null, retired = false, busy = false;
    const cancellations = new Set();
    const active = () => !retired && !signal.aborted && current();
    function retire() {
      retired = true; csrf = null; attachment = null;
      for (const cancel of cancellations) cancel();
      cancellations.clear();
    }
    function unavailable() {
      // The host hides the authorized surface synchronously, before retirement.
      try { if (active()) onUnavailable?.(); }
      finally { retire(); }
    }
    const endpoint = (operation) => "/api/v1/matches/" + matchId + "/" + operation;
    async function request(path, body=null) {
      if (!active()) throw new Error("Online document retired");
      const abort = new AbortController(); let reader;
      const cancel = () => {
        abort.abort();
        try { Promise.resolve(reader?.cancel()).catch(() => {}); } catch (_) {}
      };
      cancellations.add(cancel);
      async function wait(promise) {
        let rejectAbort;
        const interrupted = new Promise((_, reject) => { rejectAbort = () => reject(new Error("Online request interrupted")); });
        abort.signal.addEventListener("abort", rejectAbort, {once:true});
        try {
          if (abort.signal.aborted) throw new Error("Online request interrupted");
          return await Promise.race([promise, interrupted]);
        } finally { abort.signal.removeEventListener("abort", rejectAbort); }
      }
      signal.addEventListener("abort", cancel, {once:true});
      const timeout = timers.setTimeout(cancel, path.endsWith("/command") ? 30000 : 20000);
      try {
        const response = await wait(Promise.resolve().then(() => fetcher(path, {
          method:body === null ? "GET" : "POST", mode:"same-origin",
          credentials:"same-origin", cache:"no-store", redirect:"error", signal:abort.signal,
          headers:body === null ? {"Accept":"application/json"} : {"Accept":"application/json","Content-Type":"application/json","X-Tabula-CSRF":csrf},
          ...(body === null ? {} : {body})
        }))).catch(() => { throw new Error("Online request interrupted"); });
        if (!active()) throw new Error("Online document retired");
        if (response.redirected || response.status !== 200 || !response.headers.get("Cache-Control")?.split(",").some(v => v.trim().toLowerCase() === "no-store") || response.headers.get("Content-Type")?.split(";")[0].trim() !== "application/json") throw new Error("Online authority could not be confirmed");
        const claimed = response.headers.get("Content-Length");
        if (claimed !== null && (!/^\d+$/.test(claimed) || Number(claimed) > MAX_RESPONSE)) throw new Error("Online response exceeds budget");
        if (!response.body) throw new Error("Online response body missing");
        reader = response.body.getReader(); const chunks = []; let size = 0;
        try {
          for (;;) {
            const {done, value} = await wait(Promise.resolve().then(() => reader.read())).catch(() => { throw new Error("Online response body interrupted"); });
            if (done) break;
            if (!(value instanceof Uint8Array) || value.length > MAX_RESPONSE - size) { abort.abort(); throw new Error("Online response exceeds budget"); }
            size += value.length; chunks.push(value);
          }
        } finally { reader.releaseLock(); }
        if (!active()) throw new Error("Online document retired");
        const bytes = new Uint8Array(size); let at = 0;
        for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
        return bytes;
      } catch (error) { cancel(); throw error; }
      finally { cancellations.delete(cancel); timers.clearTimeout(timeout); signal.removeEventListener("abort", cancel); }
    }
    async function attach() {
      const context = privateJson(await request("/api/v1/auth/context"));
      if (context.version !== 1 || context.disposition !== "authenticated" || typeof context.csrf_token !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(context.csrf_token)) throw new Error("Sign in to open the online board");
      csrf = context.csrf_token;
      for (;;) {
        const grant = privateJson(await request(endpoint("grant"), '{"version":1}'));
        if (grant.version !== 1 || grant.game_id !== gameId || typeof grant.game_version !== "string" || !Number.isInteger(grant.seat) || grant.seat < 0 || grant.seat > 7) throw new Error("Online game binding is incompatible");
        if (grant.ready === false && grant.binding_id === null) {
          if (!active()) throw new Error("Online document retired");
          onWaiting?.();
          await new Promise((resolve, reject) => {
            const cancel = () => { cancellations.delete(cancel); signal.removeEventListener("abort", cancel); timers.clearTimeout(timer); reject(new Error("Online document retired")); };
            const timer = timers.setTimeout(() => { cancellations.delete(cancel); signal.removeEventListener("abort", cancel); resolve(); }, 500);
            cancellations.add(cancel);
            signal.addEventListener("abort", cancel, {once:true});
          });
          continue;
        }
        if (grant.ready !== true || typeof grant.binding_id !== "string" || grant.binding_id.length < 64 || grant.binding_id.length > 2048) throw new Error("Online grant is incompatible");
        const bytes = await request(endpoint("attach"), JSON.stringify({version:1, binding_id:grant.binding_id}));
        const result = privateJson(bytes);
        if (result.version !== 1 || result.seat !== grant.seat || !/^[0-9a-f]{32}$/.test(result.attachment_id) || !Number.isSafeInteger(result.next_seq) || result.next_seq < 1) throw new Error("Online attachment is incompatible");
        attachment = result.attachment_id;
        const bootstrap = new TextEncoder().encode('{"game_id":' + JSON.stringify(grant.game_id) + ',"game_version":' + JSON.stringify(grant.game_version) + ',"attachment":' + new TextDecoder().decode(bytes) + '}');
        if (bootstrap.length > MAX_RESPONSE) throw new Error("Online attachment exceeds budget");
        return bootstrap;
      }
    }
    async function file(name) {
      let ownsRequest = false;
      try {
        if (!active()) throw new Error("Online document retired");
        if (name === "tabula-online-unavailable.txt") {
          unavailable();
          throw new Error("Online document unavailable");
        }
        if (name.startsWith("tabula-online-status/")) {
          const value = privateJson(new TextEncoder().encode(decodeHex(name.slice("tabula-online-status/".length), 4096)));
          if (!Number.isInteger(value.seat) || value.seat < 0 || value.seat > 7 || !Number.isSafeInteger(value.revision) || value.revision < 0 || typeof value.status !== "string" || value.status.length > 1024 || typeof value.connection !== "string" || value.connection.length > 256 || Object.keys(value).some(key => !["seat","revision","status","connection"].includes(key))) throw new Error("Invalid runtime status");
          if (!active()) throw new Error("Online document retired");
          onStatus?.(value); return new TextEncoder().encode("ok");
        }
        if (busy) throw new Error("An online request is already active");
        busy = true; ownsRequest = true;
        if (name === "tabula-online-attach.txt") { if (attachment) throw new Error("Already attached"); return await attach(); }
        if (!attachment) throw new Error("Online board is not attached");
        if (name === "tabula-online-poll.txt") return await request(endpoint("poll"), JSON.stringify({version:1,attachment_id:attachment}));
        if (name.startsWith("tabula-online-command/")) return await request(endpoint("command"), commandBody(attachment, decodeHex(name.slice("tabula-online-command/".length), MAX_REQUEST)));
        throw new Error("Unknown online runtime operation");
      } catch (error) { unavailable(); throw error; }
      finally { if (ownsRequest) busy = false; }
    }
    return Object.freeze({file, retire});
  }
  const api = Object.freeze({create, decodeHex, commandBody, MAX_REQUEST, MAX_RESPONSE});
  root.TabulaDirectTransport = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : window);
