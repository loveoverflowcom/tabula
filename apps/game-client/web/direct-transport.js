/* Same-origin HTTPS Fetch/storage adaptation. Rust owns command identity,
 * sequencing, retry decisions and projections (ADR-0031; I-5/I-12). */
(function(root) {
  "use strict";
  const VERSION = 2, MAX_RESPONSE = 2097152, MAX_REQUEST = 65536;
  const PENDING_TTL = 600000, MAX_ATTEMPTS = 6;
  function decodeHex(hex, max) {
    if (typeof hex !== "string" || hex.length > max * 2 || hex.length % 2 || !/^[0-9a-f]*$/.test(hex)) throw new Error("Invalid bounded runtime request");
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    return new TextDecoder("utf-8", {fatal:true}).decode(bytes);
  }
  // Never parse/stringify opaque commands: a u128 JSON identity would round.
  function commandBody(attachment, command) {
    if (!/^[0-9a-f]{32}$/.test(attachment) || typeof command !== "string" || new TextEncoder().encode(command).length > MAX_REQUEST) throw new Error("Invalid bounded command");
    const body = '{"version":2,"attachment_id":' + JSON.stringify(attachment) + ',"command":' + command + '}';
    if (new TextEncoder().encode(body).length > MAX_REQUEST) throw new Error("Online command exceeds request budget");
    return body;
  }
  function privateJson(bytes) {
    try { return JSON.parse(new TextDecoder("utf-8", {fatal:true}).decode(bytes)); }
    catch (_) { throw new Error("Online response is incompatible"); }
  }
  class Retryable extends Error {}
  class AuthorityDenied extends Error {}
  function create({matchId, gameId, signal, current, onWaiting, onStatus, onRecovering, onUnavailable, fetcher=root.fetch, protocol=root.location?.protocol, timers=root, storage, now=()=>Date.now(), random=()=>Math.random(), lifecycle=root}) {
    if (protocol !== "https:" || !/^[0-9a-f]{32}$/.test(matchId) || /^0+$/.test(matchId) || typeof gameId !== "string" || gameId.length > 128) throw new Error("Online play needs a trusted HTTPS document");
    if (storage === undefined) { try { storage = root.sessionStorage; } catch (_) { storage = null; } }
    let csrf = null, attachment = null, operationScope = null, gameVersion = null, retired = false, busy = false, generation = 0;
    const cancellations = new Set(), requestCancellations = new Set(), key = "tabula.pending.v2." + matchId;
    const active = () => !retired && !signal.aborted && current();
    const encode = value => new TextEncoder().encode(JSON.stringify(value));
    let pending = null, pendingUnknown = false;
    function clearPending() { pending = null; pendingUnknown = false; try { storage?.removeItem(key); } catch (_) {} }
    function loadPending() {
      let raw;
      try { raw = storage?.getItem(key); } catch (_) { return; }
      if (raw === null || raw === undefined) return;
      try {
        if (raw.length > MAX_REQUEST * 2 + 2048) throw new Error();
        const p = JSON.parse(raw);
        if (Object.keys(p).some(k => !["version","match_id","game_id","game_version","operation_scope","command","expires_at","unknown"].includes(k)) || p.version !== VERSION || p.match_id !== matchId || p.game_id !== gameId || typeof p.game_version !== "string" || p.game_version.length > 64 || !/^[0-9a-f]{64}$/.test(p.operation_scope) || !Number.isSafeInteger(p.expires_at) || (p.unknown !== true && (typeof p.command !== "string" || new TextEncoder().encode(p.command).length > MAX_REQUEST))) throw new Error();
        if (p.unknown === true || p.expires_at <= now() || p.expires_at > now() + PENDING_TTL) { pendingUnknown = true; pending = null; }
        else pending = p;
      } catch (_) { pendingUnknown = true; pending = null; }
    }
    loadPending();
    function remember(command) {
      if (!operationScope || !gameVersion || pendingUnknown) throw new Error("An earlier move has an unknown result");
      if (pending && (pending.command !== command || pending.operation_scope !== operationScope)) throw new Error("An online move is already pending");
      const value = pending || {version:VERSION,match_id:matchId,game_id:gameId,game_version:gameVersion,operation_scope:operationScope,command,expires_at:now()+PENDING_TTL};
      // If browser storage is inaccessible, do not claim refresh-safe submission.
      if (!storage) throw new Error("Safe online move recovery is unavailable");
      try { storage.setItem(key, JSON.stringify(value)); } catch (_) { throw new Error("Safe online move recovery is unavailable"); }
      pending = value;
    }
    function markUnknown() {
      pending = null; pendingUnknown = true;
      try { storage?.setItem(key, JSON.stringify({version:VERSION,match_id:matchId,game_id:gameId,game_version:gameVersion,operation_scope:operationScope,command:null,expires_at:now()+PENDING_TTL,unknown:true})); } catch (_) {}
    }
    function retire() {
      retired = true; generation++; csrf = null; attachment = null; operationScope = null;
      for (const cancel of [...cancellations]) cancel();
      cancellations.clear();
      lifecycle?.removeEventListener?.("offline", interrupt);
      lifecycle?.document?.removeEventListener?.("visibilitychange", visibility);
    }
    function unavailable() {
      const unknown = Boolean(pending || pendingUnknown);
      try { if (active()) onUnavailable?.({unknown}); }
      finally { clearPending(); retire(); }
    }
    function conceal() { if (active()) onRecovering?.({state:pending || pendingUnknown ? "unknown-result" : "recovering"}); }
    function interrupt() {
      if (!active()) return;
      conceal(); generation++; csrf = null; attachment = null;
      for (const cancel of [...requestCancellations]) cancel();
    }
    function visibility() { interrupt(); }
    lifecycle?.addEventListener?.("offline", interrupt);
    lifecycle?.document?.addEventListener?.("visibilitychange", visibility);
    const endpoint = operation => "/api/v1/matches/" + matchId + "/" + operation;
    async function request(path, body=null) {
      if (!active()) throw new Error("Online document retired");
      const epoch = generation, abort = new AbortController(); let reader;
      const valid = () => active() && epoch === generation;
      const cancel = () => { abort.abort(); try { Promise.resolve(reader?.cancel()).catch(() => {}); } catch (_) {} };
      cancellations.add(cancel); requestCancellations.add(cancel);
      async function wait(promise) {
        let rejectAbort;
        const interrupted = new Promise((_, reject) => { rejectAbort = () => reject(new Retryable("Online request interrupted")); });
        abort.signal.addEventListener("abort", rejectAbort, {once:true});
        try { if (abort.signal.aborted) throw new Retryable("Online request interrupted"); return await Promise.race([promise, interrupted]); }
        finally { abort.signal.removeEventListener("abort", rejectAbort); }
      }
      signal.addEventListener("abort", cancel, {once:true});
      const timeout = timers.setTimeout(cancel, path.endsWith("/command") ? 30000 : 20000);
      try {
        const response = await wait(Promise.resolve().then(() => fetcher(path, {
          method:body === null ? "GET" : "POST", mode:"same-origin", credentials:"same-origin", cache:"no-store", redirect:"error", signal:abort.signal,
          headers:body === null ? {"Accept":"application/json"} : {"Accept":"application/json","Content-Type":"application/json","X-Tabula-CSRF":csrf}, ...(body === null ? {} : {body})
        }))).catch(() => { throw new Retryable("Online request interrupted"); });
        if (!valid()) throw new Retryable("Online request retired");
        if ([401,403].includes(response.status)) throw new AuthorityDenied("Online authority is unavailable");
        if ([409,429,500,502,503,504].includes(response.status)) throw new Retryable("Online service is recovering");
        if (response.redirected || response.status !== 200 || !response.headers.get("Cache-Control")?.split(",").some(v => v.trim().toLowerCase() === "no-store") || response.headers.get("Content-Type")?.split(";")[0].trim() !== "application/json") throw new Error("Online authority could not be confirmed");
        const claimed = response.headers.get("Content-Length");
        if (claimed !== null && (!/^\d+$/.test(claimed) || Number(claimed) > MAX_RESPONSE)) throw new Error("Online response exceeds budget");
        if (!response.body) throw new Retryable("Online response body missing");
        reader = response.body.getReader(); const chunks = []; let size = 0;
        try {
          for (;;) {
            const {done,value} = await wait(Promise.resolve().then(() => reader.read())).catch(() => { throw new Retryable("Online response body interrupted"); });
            if (done) break;
            if (!(value instanceof Uint8Array) || value.length > MAX_RESPONSE-size) throw new Error("Online response exceeds budget");
            size += value.length; chunks.push(value);
          }
        } finally { reader.releaseLock(); }
        if (!valid()) throw new Retryable("Online request retired");
        const bytes = new Uint8Array(size); let at = 0;
        for (const chunk of chunks) { bytes.set(chunk,at); at += chunk.length; }
        return bytes;
      } catch (error) { cancel(); throw error; }
      finally { cancellations.delete(cancel); requestCancellations.delete(cancel); timers.clearTimeout(timeout); signal.removeEventListener("abort", cancel); }
    }
    function pause(delay) {
      return new Promise((resolve,reject) => {
        const cancel = () => { cancellations.delete(cancel); signal.removeEventListener("abort",cancel); timers.clearTimeout(timer); reject(new Error("Online document retired")); };
        const timer = timers.setTimeout(() => { cancellations.delete(cancel); signal.removeEventListener("abort",cancel); resolve(); },delay);
        cancellations.add(cancel); signal.addEventListener("abort",cancel,{once:true});
      });
    }
    async function attach() {
      const context = privateJson(await request("/api/v1/auth/context"));
      if (context.version !== 1 || context.disposition !== "authenticated" || typeof context.csrf_token !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(context.csrf_token)) throw new AuthorityDenied("Sign in to open the online board");
      csrf = context.csrf_token;
      const waitingDeadline = now()+PENDING_TTL;
      for (;;) {
        const grant = privateJson(await request(endpoint("grant"), '{"version":2}'));
        if (grant.version !== VERSION || grant.game_id !== gameId || typeof grant.game_version !== "string" || !Number.isInteger(grant.seat) || grant.seat < 0 || grant.seat > 7) throw new Error("Online game binding is incompatible");
        if (grant.ready === false && grant.binding_id === null) {
          if (!active()) throw new Error("Online document retired");
          if (now() >= waitingDeadline) throw new Error("The online match is not ready");
          onWaiting?.(); await pause(500); continue;
        }
        if (grant.ready !== true || typeof grant.binding_id !== "string" || grant.binding_id.length < 64 || grant.binding_id.length > 2048) throw new Error("Online grant is incompatible");
        const bytes = await request(endpoint("attach"), JSON.stringify({version:VERSION,binding_id:grant.binding_id}));
        const result = privateJson(bytes);
        if (result.version !== VERSION || result.seat !== grant.seat || !/^[0-9a-f]{32}$/.test(result.attachment_id) || !/^[0-9a-f]{64}$/.test(result.operation_scope) || !Number.isSafeInteger(result.next_seq) || result.next_seq < 1) throw new Error("Online attachment is incompatible");
        attachment = result.attachment_id; operationScope = result.operation_scope; gameVersion = grant.game_version;
        if (pending && pending.expires_at <= now()) { pending = null; pendingUnknown = true; }
        // Pending text stays opaque. Rust validates binding/sequence before any retry.
        const pendingJson = JSON.stringify(pending ? {operation_scope:pending.operation_scope,command:pending.command} : null);
        const bootstrap = new TextEncoder().encode('{"game_id":'+JSON.stringify(gameId)+',"game_version":'+JSON.stringify(gameVersion)+',"attachment":'+new TextDecoder().decode(bytes)+',"pending":'+pendingJson+',"pending_unknown":'+JSON.stringify(pendingUnknown)+',"transport_generation":'+generation+'}');
        if (bootstrap.length > MAX_RESPONSE) throw new Error("Online attachment exceeds budget");
        return bootstrap;
      }
    }
    async function recover(error, initial=false) {
      if (!(error instanceof Retryable) || !active()) throw error;
      conceal(); generation++; csrf = null; attachment = null;
      for (let attempt=0; attempt<MAX_ATTEMPTS; attempt++) {
        await pause(Math.floor(Math.min(8000,500*2**attempt)*(0.5+Math.max(0,Math.min(1,random()))*0.5)));
        if (!active()) throw new Error("Online document retired");
        try {
          const bootstrap = await attach();
          return initial ? bootstrap : new TextEncoder().encode('{"transport":"resync","bootstrap":'+new TextDecoder().decode(bootstrap)+'}');
        } catch (next) { if (!(next instanceof Retryable)) throw next; csrf = null; attachment = null; }
      }
      // Keep the bounded pending hint for an explicit Retry/refresh. No failure inferred.
      throw new Retryable("Online recovery needs an explicit retry");
    }
    async function file(name) {
      let ownsRequest = false;
      try {
        if (!active()) throw new Error("Online document retired");
        if (name === "tabula-online-unresolved.txt") {
          try { if (active()) onUnavailable?.({unknown:true}); } finally { retire(); }
          throw new Error("Online move result remains unknown");
        }
        if (name === "tabula-online-unavailable.txt") { unavailable(); throw new Error("Online document unavailable"); }
        if (name === "tabula-online-conceal.txt") { interrupt(); return encode("ok"); }
        if (name === "tabula-online-settled.txt") { clearPending(); return encode("ok"); }
        if (name === "tabula-online-unknown.txt") { markUnknown(); return encode("ok"); }
        if (name.startsWith("tabula-online-status/")) {
          const value = privateJson(new TextEncoder().encode(decodeHex(name.slice("tabula-online-status/".length),4096)));
          if (!Number.isInteger(value.seat) || value.seat < 0 || value.seat > 7 || !Number.isSafeInteger(value.revision) || value.revision < 0 || typeof value.status !== "string" || value.status.length > 1024 || typeof value.connection !== "string" || value.connection.length > 256 || Object.keys(value).some(k => !["seat","revision","status","connection","generation"].includes(k))) throw new Error("Invalid runtime status");
          // A retired authority cannot be revealed by queued old presenter status.
          if (attachment && value.generation === generation && (!lifecycle.document || lifecycle.document.visibilityState === "visible")) onStatus?.(value); return encode("ok");
        }
        if (busy) throw new Error("An online request is already active");
        busy = true; ownsRequest = true;
        if (name === "tabula-online-attach.txt") {
          if (attachment) throw new Error("Already attached");
          try { return await attach(); } catch (error) { return await recover(error,true); }
        }
        if (name === "tabula-online-recover.txt") return await recover(new Retryable("Online stream needs resync"));
        if (!attachment) return await recover(new Retryable("Online board is not attached"));
        try {
          if (name === "tabula-online-poll.txt") return await request(endpoint("poll"), JSON.stringify({version:VERSION,attachment_id:attachment}));
          if (name.startsWith("tabula-online-command/")) {
            const command = decodeHex(name.slice("tabula-online-command/".length),MAX_REQUEST);
            const body = commandBody(attachment,command); remember(command);
            return await request(endpoint("command"),body);
          }
          throw new Error("Unknown online runtime operation");
        } catch (error) {
          if (!(error instanceof Retryable) || !active()) throw error;
          conceal(); generation++; csrf = null; attachment = null;
          // Hand uncertainty to Rust immediately so it drops the private projection
          // before a later request performs fresh-authority recovery.
          return encode({transport:"recovering"});
        }
      } catch (error) {
        // Backpressure is local and must not retire the in-flight owner.
        if (!ownsRequest && busy && error.message === "An online request is already active") throw error;
        if (error instanceof Retryable || (!(error instanceof AuthorityDenied) && (pending || pendingUnknown))) {
          try { if (active()) onUnavailable?.({unknown:Boolean(pending || pendingUnknown)}); } finally { retire(); }
        } else unavailable();
        throw error;
      } finally { if (ownsRequest) busy = false; }
    }
    return Object.freeze({file,retire});
  }
  const api = Object.freeze({create,decodeHex,commandBody,MAX_REQUEST,MAX_RESPONSE,PENDING_TTL,MAX_ATTEMPTS,VERSION});
  root.TabulaDirectTransport = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : window);
