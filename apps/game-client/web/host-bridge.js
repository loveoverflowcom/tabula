/* Typed lifecycle/preferences/service bridge to a native mobile GameHost (ADR-0033).
 *
 * This is a small control channel, never a state channel: it carries lifecycle
 * events, host preferences and capability-gated service requests. Canonical
 * state, projections, render lists and credentials never cross it (I-5/I-6/I-10).
 *
 * Wire: one JSON object per message, UTF-8 text of at most MAX_BYTES, schema
 * version 1. The Kotlin/Swift decoders in mobile/ implement the same table; the
 * shared vectors in mobile/shared/src/commonTest/.../BridgeVectors.kt and
 * tests/host-bridge.test.cjs pin both sides to the same bytes. */
(function (root) {
  "use strict";
  const PROTOCOL = 1;
  const MAX_BYTES = 4096;
  const MAX_DETAIL = 200;
  const THEMES = ["system", "light", "dark", "hc-light", "hc-dark"];
  const MOTIONS = ["system", "reduced"];
  const LOCALES = ["vi", "en"];
  const CAPABILITIES = ["keep-awake"];
  const FAILURE_CODES = ["runtime", "graphics-context", "timeout", "bridge"];
  const HELLO_TIMEOUT_MS = 5000;

  function plain(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const prototype = Object.getPrototypeOf(value);
    return prototype === null || prototype === Object.prototype;
  }
  // Exactly these data fields: no extras, no accessors, no inherited surprises.
  function exact(value, keys) {
    if (!plain(value)) return false;
    const own = Reflect.ownKeys(value);
    return own.length === keys.length && own.every((key) => keys.includes(key) && Object.hasOwn(Object.getOwnPropertyDescriptor(value, key), "value"));
  }
  const generation = (value) => Number.isSafeInteger(value) && value >= 1 && value <= 0x7fffffff;
  const textBytes = (text) => new TextEncoder().encode(text).byteLength;

  /** Host → page. Returns {ok, message} or {ok:false, error}; never throws. */
  function decode(text) {
    try {
      if (typeof text !== "string" || textBytes(text) > MAX_BYTES) return {ok:false, error:"oversized"};
      const raw = JSON.parse(text);
      if (!plain(raw) || raw.v !== PROTOCOL || typeof raw.type !== "string") return {ok:false, error:"schema"};
      switch (raw.type) {
        case "init":
          if (!exact(raw, ["v", "type", "gen", "capabilities", "preferences"]) || !generation(raw.gen)) return {ok:false, error:"schema"};
          if (!Array.isArray(raw.capabilities) || raw.capabilities.length > CAPABILITIES.length || new Set(raw.capabilities).size !== raw.capabilities.length || raw.capabilities.some((name) => !CAPABILITIES.includes(name))) return {ok:false, error:"schema"};
          if (!exact(raw.preferences, ["theme", "motion", "locale"]) || !THEMES.includes(raw.preferences.theme) || !MOTIONS.includes(raw.preferences.motion) || !LOCALES.includes(raw.preferences.locale)) return {ok:false, error:"schema"};
          return {ok:true, message:Object.freeze({type:"init", gen:raw.gen, capabilities:Object.freeze([...raw.capabilities]), preferences:Object.freeze({...raw.preferences})})};
        case "suspend": case "resume": case "dispose": case "back-requested":
          if (!exact(raw, ["v", "type", "gen"]) || !generation(raw.gen)) return {ok:false, error:"schema"};
          return {ok:true, message:Object.freeze({type:raw.type, gen:raw.gen})};
        case "reply":
          if (!exact(raw, ["v", "type", "gen", "id", "ok", "code"]) || !generation(raw.gen) || !Number.isSafeInteger(raw.id) || raw.id < 1 || typeof raw.ok !== "boolean" || !["ok", "denied", "unsupported"].includes(raw.code)) return {ok:false, error:"schema"};
          return {ok:true, message:Object.freeze({type:"reply", gen:raw.gen, id:raw.id, ok:raw.ok, code:raw.code})};
        default:
          return {ok:false, error:"unknown-type"};
      }
    } catch (_) { return {ok:false, error:"malformed"}; }
  }

  /** Page → host. Throws on a message that breaks the schema, so a bug here is loud in tests. */
  function encode(message) {
    const out = {v:PROTOCOL, ...message};
    const bad = (why) => { throw new Error(`Invalid host bridge message: ${why}`); };
    switch (message.type) {
      case "hello": if (!exact(message, ["type"])) bad("hello"); break;
      case "ready":
        if (!exact(message, ["type", "gen", "bootMs"]) || !generation(message.gen) || !Number.isSafeInteger(message.bootMs) || message.bootMs < 0 || message.bootMs > 3600000) bad("ready");
        break;
      case "failed":
        if (!exact(message, ["type", "gen", "code", "detail"]) || !generation(message.gen) || !FAILURE_CODES.includes(message.code) || typeof message.detail !== "string") bad("failed");
        // Code points, with any lone surrogate replaced, so the host never sees ill-formed text.
        out.detail = [...message.detail].slice(0, MAX_DETAIL).map((point) => point.length === 1 && /[\ud800-\udfff]/.test(point) ? "\ufffd" : point).join("");
        break;
      case "exit": if (!exact(message, ["type", "gen"]) || !generation(message.gen)) bad("exit"); break;
      case "service":
        if (!exact(message, ["type", "gen", "id", "name", "enabled"]) || !generation(message.gen) || !Number.isSafeInteger(message.id) || message.id < 1 || !CAPABILITIES.includes(message.name) || typeof message.enabled !== "boolean") bad("service");
        break;
      default: bad("type");
    }
    const text = JSON.stringify(out);
    if (textBytes(text) > MAX_BYTES) bad("oversized");
    return text;
  }

  /**
   * One bridge per document. `native` is the origin-restricted port the host
   * injected: `postMessage(text)` out, `onmessage({data})` in. Hostile or stale
   * input is dropped, never thrown into the game loop.
   */
  function attach(native, handlers = {}, {timers = globalThis} = {}) {
    let generationId = null;
    let disposed = false;
    let initialized = false;
    let granted = [];
    let nextService = 1;
    const pending = new Map();
    let helloTimer = null;
    let resolveInit;
    let rejectInit;
    const init = new Promise((resolve, reject) => { resolveInit = resolve; rejectInit = reject; });
    init.catch(() => {});

    function send(message) {
      if (disposed) return false;
      try { native.postMessage(encode(message)); return true; } catch (_) { return false; }
    }
    function receive(event) {
      const decoded = decode(event?.data);
      if (!decoded.ok) { handlers.dropped?.(decoded.error); return; }
      const message = decoded.message;
      if (disposed) { handlers.dropped?.("disposed"); return; }
      if (message.type === "init") {
        if (initialized) { handlers.dropped?.("duplicate-init"); return; }
        initialized = true;
        generationId = message.gen;
        granted = message.capabilities;
        timers.clearTimeout(helloTimer);
        resolveInit(message);
        return;
      }
      if (!initialized || message.gen !== generationId) { handlers.dropped?.("stale-generation"); return; }
      switch (message.type) {
        case "reply": {
          const resolve = pending.get(message.id);
          if (!resolve) { handlers.dropped?.("unknown-reply"); return; }
          pending.delete(message.id);
          resolve(message);
          return;
        }
        case "dispose":
          disposed = true;
          pending.clear();
          handlers.dispose?.();
          return;
        default:
          handlers[message.type === "back-requested" ? "backRequested" : message.type]?.();
      }
    }
    native.onmessage = receive;
    return Object.freeze({
      /** Resolves with the host's init (generation, capabilities, preferences) or rejects on silence. */
      handshake() {
        helloTimer = timers.setTimeout(() => rejectInit(new Error("The mobile host did not answer")), HELLO_TIMEOUT_MS);
        if (!send({type:"hello"})) rejectInit(new Error("The mobile host bridge is unavailable"));
        return init;
      },
      ready: (bootMs) => initialized && send({type:"ready", gen:generationId, bootMs:Math.max(0, Math.min(3600000, Math.round(bootMs)))}),
      failed: (code, detail) => initialized && send({type:"failed", gen:generationId, code:FAILURE_CODES.includes(code) ? code : "runtime", detail:String(detail ?? "")}),
      exit: () => initialized && send({type:"exit", gen:generationId}),
      /** Capability-gated: an ungranted service is refused here and again by the host. */
      service(name, enabled) {
        if (!initialized || disposed || !granted.includes(name)) return Promise.resolve({ok:false, code:"denied"});
        if (pending.size >= 4) return Promise.resolve({ok:false, code:"unsupported"});
        const id = nextService++;
        return new Promise((resolve) => {
          pending.set(id, resolve);
          if (!send({type:"service", gen:generationId, id, name, enabled:Boolean(enabled)})) { pending.delete(id); resolve({ok:false, code:"unsupported"}); }
        });
      },
      get generation() { return generationId; },
      get disposed() { return disposed; },
      get capabilities() { return granted; },
    });
  }

  const api = Object.freeze({PROTOCOL, MAX_BYTES, decode, encode, attach});
  root.TabulaHostBridge = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : window);
